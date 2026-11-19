//! Bounded, conservative lexical inspection. Never executes inspected code.
use crate::{policy::Limits, storage};
use regex::Regex;
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
};

#[derive(Default)]
pub struct Inspection {
    pub texts: Vec<String>,
    pub incomplete: bool,
    pub files: usize,
}

fn script_extension(s: &str) -> bool {
    [".py", ".js", ".mjs", ".cjs", ".sh", ".bash"]
        .iter()
        .any(|ext| s.ends_with(ext))
}
fn interpreter(s: &str) -> bool {
    let base = s
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(s)
        .trim_end_matches(".exe");
    [
        "python", "python3", "python2", "node", "nodejs", "bash", "sh", "zsh",
    ]
    .contains(&base)
        || base.starts_with("python3.")
}

pub fn inspect(command: &str, cwd: Option<&Path>, limits: &Limits) -> Inspection {
    let mut result = Inspection {
        texts: vec![command.into()],
        ..Default::default()
    };
    let Some(tokens) = shlex::split(command) else {
        result.incomplete = true;
        return result;
    };
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    let mut inline = Vec::new();
    let mut expects_script = false;
    for (i, token) in tokens.iter().enumerate() {
        let t = token.trim_matches([';', '|', '&']);
        if interpreter(t) {
            expects_script = true;
            continue;
        }
        if expects_script && ["-c", "-lc", "-ic", "-xc", "-e", "--eval"].contains(&t) {
            if let Some(code) = tokens.get(i + 1) {
                inline.push(code.clone());
            } else {
                result.incomplete = true;
            }
            expects_script = false;
            continue;
        }
        if expects_script && ["-m", "-", "--", "--import", "--require", "-r"].contains(&t) {
            result.incomplete = true;
            expects_script = false;
            continue;
        }
        if expects_script && !t.starts_with('-') {
            if !tokens
                .get(i.wrapping_sub(1))
                .is_some_and(|s| ["-c", "-lc", "-ic", "-xc", "-e", "--eval"].contains(&s.as_str()))
            {
                queue.push_back((PathBuf::from(t), 0));
            }
            expects_script = false;
        } else if script_extension(t) && !t.contains("://") && !t.contains(['\n', ' ', '=']) {
            queue.push_back((PathBuf::from(t), 0));
        }
    }
    // Unsupported execution cannot be mistaken for a completed script inspection.
    if command.contains("<<")
        || command.contains("<(")
        || command.contains("$('")
        || (command.contains("cd ") && !queue.is_empty())
        || expects_script
    {
        result.incomplete = true;
    }
    if queue.is_empty() && inline.is_empty() {
        return result;
    }
    let imports = Regex::new(r#"(?m)(?:\brequire\s*\(\s*|\bfrom\s+|\bimport\s*\(\s*)["'](\.[^"']+)["']|(?:^|[;\n])\s*(?:source|\.)\s+["']?([^\s;"']+)|^\s*(?:from|import)\s+([A-Za-z_][\w.]*)"#).expect("static regex");
    let dynamic =
        Regex::new(r"\b(?:eval|exec|__import__)\s*\(|\bimportlib\b|\b(?:base64|b64decode)\b")
            .expect("static regex");
    let root = cwd.and_then(|p| p.canonicalize().ok());
    if (!queue.is_empty() || !inline.is_empty()) && root.is_none() {
        result.incomplete = true;
    }
    for code in inline {
        if let Some(parts) = shlex::split(&code) {
            for part in parts {
                if script_extension(&part)
                    && !part.contains([' ', '\n', '='])
                    && !part.contains("://")
                {
                    queue.push_back((PathBuf::from(part), 0));
                }
            }
        }
        if dynamic.is_match(&code) {
            result.incomplete = true;
        }
        if let Some(root) = &root {
            dependencies(&code, root, 0, &imports, &mut queue, &mut result);
        }
        result.texts.push(code);
    }
    let Some(root) = root else {
        return result;
    };
    let mut seen = HashSet::new();
    let mut total = 0;
    while let Some((path, depth)) = queue.pop_front() {
        let joined = root.join(path);
        let Ok(path) = joined.canonicalize() else {
            result.incomplete = true;
            continue;
        };
        if !path.starts_with(&root) {
            result.incomplete = true;
            continue;
        }
        if !seen.insert(path.clone()) {
            continue;
        }
        if result.files >= limits.max_scripts || depth > limits.max_depth {
            result.incomplete = true;
            break;
        }
        let remaining = limits.max_total_script_bytes.saturating_sub(total);
        let text = match storage::read_text(&path, limits.max_script_bytes.min(remaining)) {
            Ok(s) => s,
            Err(_) => {
                result.incomplete = true;
                continue;
            }
        };
        total += text.len();
        result.files += 1;
        if dynamic.is_match(&text) {
            result.incomplete = true;
        }
        dependencies(
            &text,
            path.parent().unwrap_or(&root),
            depth + 1,
            &imports,
            &mut queue,
            &mut result,
        );
        result.texts.push(text);
    }
    result
}

fn dependencies(
    text: &str,
    base: &Path,
    depth: usize,
    re: &Regex,
    queue: &mut VecDeque<(PathBuf, usize)>,
    result: &mut Inspection,
) {
    for cap in re.captures_iter(text) {
        if let Some(m) = cap.get(1).or_else(|| cap.get(2)) {
            let path = base.join(m.as_str());
            if path.is_file() {
                queue.push_back((path, depth));
            } else if path.extension().is_none() && path.with_extension("js").is_file() {
                queue.push_back((path.with_extension("js"), depth));
            } else {
                result.incomplete = true;
            }
        } else if let Some(m) = cap.get(3) {
            // Third party/stdlib Python modules are not recursively scanned.
            let local = base.join(m.as_str().replace('.', "/")).with_extension("py");
            let package = base.join(m.as_str().replace('.', "/")).join("__init__.py");
            if local.is_file() {
                queue.push_back((local, depth));
            } else if package.is_file() {
                queue.push_back((package, depth));
            }
        }
    }
}
