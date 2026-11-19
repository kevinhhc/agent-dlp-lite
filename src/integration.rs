use clap::ValueEnum;
use serde_json::{json, Value};
use std::path::Path;

const MARKER: &str = " --managed-by agent-dlp-lite";
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Agent {
    Codex,
    Workbuddy,
}
impl Agent {
    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Workbuddy => "workbuddy",
        }
    }
}

pub fn quote_path(path: &Path) -> Result<String, String> {
    let s = path.to_str().ok_or("path must be UTF-8")?;
    if s.chars().any(char::is_control) {
        return Err("control character in path".into());
    }
    #[cfg(windows)]
    {
        if s.contains(['%', '!', '"']) {
            return Err("unsupported Windows shell character in path".into());
        }
        Ok(format!("\"{s}\""))
    }
    #[cfg(not(windows))]
    {
        shlex::try_quote(s)
            .map(|s| s.into_owned())
            .map_err(|_| "cannot quote path".into())
    }
}

pub fn configuration(agent: Agent, executable: &Path, policy: &Path) -> Result<Value, String> {
    configuration_with_history(agent, executable, policy, &crate::history::default_path()?)
}
pub fn configuration_with_history(
    agent: Agent,
    executable: &Path,
    policy: &Path,
    history: &Path,
) -> Result<Value, String> {
    let command = format!(
        "{} --policy {} --history-file {} hook --agent {}{}{}",
        quote_path(executable)?,
        quote_path(policy)?,
        quote_path(history)?,
        agent.name(),
        if matches!(agent, Agent::Codex) {
            " --native-companion"
        } else {
            ""
        },
        MARKER
    );
    let mut pre = vec![json!({"type":"command","command":command,"timeout":5})];
    let mut prompt = pre.clone();
    if matches!(agent, Agent::Codex) {
        for (hooks, event, fields) in [
            (
                &mut pre,
                "PreToolUse",
                json!({"tool_name":"${tool_name}","tool_input":"${tool_input}","tool_use_id":"${tool_use_id}"}),
            ),
            (
                &mut prompt,
                "UserPromptSubmit",
                json!({"prompt":"${prompt}"}),
            ),
        ] {
            let mut input = json!({"hook_event_name":event,"session_id":"${session_id}","turn_id":"${turn_id}","cwd":"${cwd}"});
            input
                .as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            hooks.push(json!({"type":"mcp_tool","server":crate::mcp::server_name(),"tool":"inspect","input":{"event":input},"timeout":90,"statusMessage":"Agent DLP Lite: checking sensitive content"}));
        }
    }
    Ok(json!({"hooks": {
        "PreToolUse": [{"matcher":"*", "hooks":pre}],
        "UserPromptSubmit": [{"hooks":prompt}]
    }}))
}

fn owned(h: &Value) -> bool {
    (h.get("type").and_then(Value::as_str) == Some("mcp_tool")
        && h.get("server").and_then(Value::as_str) == Some(crate::mcp::server_name())
        && h.get("tool").and_then(Value::as_str) == Some("inspect"))
        || h.get("type").and_then(Value::as_str) == Some("command")
            && h.get("command")
                .and_then(Value::as_str)
                .is_some_and(|c| c.ends_with(MARKER))
}

pub fn remove(root: &mut Value) -> Result<usize, String> {
    if !root.is_object() {
        return Err("settings root must be an object".into());
    }
    let Some(hooks) = root.get_mut("hooks") else {
        return Ok(0);
    };
    let hooks = hooks.as_object_mut().ok_or("hooks must be an object")?;
    let mut removed = 0;
    for groups in hooks.values_mut() {
        let groups = groups.as_array_mut().ok_or("hook event must be an array")?;
        for group in groups.iter_mut() {
            let handlers = group
                .get_mut("hooks")
                .and_then(Value::as_array_mut)
                .ok_or("invalid hook group")?;
            let len = handlers.len();
            handlers.retain(|h| !owned(h));
            removed += len - handlers.len();
        }
        groups.retain(|g| g["hooks"].as_array().is_none_or(|a| !a.is_empty()));
    }
    Ok(removed)
}

pub fn merge(root: &mut Value, generated: Value) -> Result<(), String> {
    remove(root)?;
    let map = root
        .as_object_mut()
        .ok_or("settings root must be an object")?;
    let hooks = map
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("hooks must be object")?;
    for (name, groups) in generated["hooks"]
        .as_object()
        .ok_or("invalid generated hooks")?
    {
        let existing = hooks
            .entry(name)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or("hook event must be array")?;
        existing.extend(groups.as_array().ok_or("invalid generated groups")?.clone());
    }
    Ok(())
}

pub fn count(root: &Value) -> usize {
    root.get("hooks")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|o| o.values())
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|g| g.get("hooks").and_then(Value::as_array))
        .flatten()
        .filter(|h| owned(h))
        .count()
}
