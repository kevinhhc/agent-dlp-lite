//! A bounded decision journal; replacements never produce history backups.
use crate::{engine::Report, event::Event, policy::CompiledPack, storage};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub const CAPACITY: usize = 100;
const MAX_BYTES: usize = 8 * 1024 * 1024;
pub fn default_path() -> Result<PathBuf, String> {
    let home = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("USERPROFILE"))
    } else {
        std::env::var_os("HOME")
    }
    .ok_or("cannot locate user history directory")?;
    Ok(PathBuf::from(home).join(".agent-dlp-lite/history.jsonl"))
}
pub fn redacted(text: &str, policy: &CompiledPack, limit: usize) -> String {
    let mut value = text.to_owned();
    for (rule, re) in &policy.rules {
        value = re
            .replace_all(&value, format!("[REDACTED:{}]", rule.id).as_str())
            .into_owned();
    }
    value
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(limit)
        .collect()
}
pub fn record(
    event: &Event,
    policy: &CompiledPack,
    report: &Report,
    source: &str,
    reason: &str,
) -> Value {
    let mut value = serde_json::to_value(report).expect("report serialization");
    let object = value.as_object_mut().expect("report object");
    object.insert("record_version".into(), json!(1));
    object.insert("detector_version".into(), json!(env!("CARGO_PKG_VERSION")));
    object.insert(
        "policy_sha256".into(),
        json!(format!(
            "{:x}",
            Sha256::digest(policy.export().unwrap_or_default().as_bytes())
        )),
    );
    object.insert(
        "timestamp_unix_ms".into(),
        json!(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64),
    );
    object.insert(
        "decision".into(),
        json!(if report.action == crate::policy::Action::Deny {
            "deny"
        } else {
            "allow"
        }),
    );
    object.insert("decision_source".into(), json!(source));
    object.insert("reason".into(), json!(redacted(reason, policy, 256)));
    object.insert(
        "preview".into(),
        json!(redacted(&event.texts().join("\n"), policy, 400)),
    );
    if let Some(name) = &event.tool_name {
        object.insert("tool".into(), json!(redacted(name, policy, 80)));
    }
    value
}
fn locked<T>(path: &Path, operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if fs::symlink_metadata(parent).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("history directory must not be a symlink".into());
    }
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(parent)
        .map_err(|_| "cannot create history directory")?;
    let lock_path = path.with_file_name(format!(
        "{}.lock",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let lock = storage::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|_| "cannot open history lock")?;
    if !lock
        .metadata()
        .map_err(|_| "cannot stat history lock")?
        .is_file()
    {
        return Err("invalid history lock".into());
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if lock.try_lock().is_ok() {
            break;
        }
        if Instant::now() >= deadline {
            return Err("history lock timed out".into());
        }
        thread::sleep(Duration::from_millis(5));
    }
    let result = operation();
    let _ = lock.unlock();
    result
}
fn load(path: &Path) -> Result<VecDeque<Value>, String> {
    if fs::symlink_metadata(path).is_err() {
        return Ok(VecDeque::new());
    }
    let text = storage::read_text(path, MAX_BYTES)?;
    let mut records = VecDeque::new();
    for line in text.lines().filter(|s| !s.trim().is_empty()) {
        let value: Value = serde_json::from_str(line).map_err(|_| "invalid history data")?;
        if !value.is_object() {
            return Err("invalid history record".into());
        }
        records.push_back(value);
        if records.len() > CAPACITY {
            records.pop_front();
        }
    }
    Ok(records)
}
pub fn append(path: &Path, record: Value) -> Result<(), String> {
    locked(path, || {
        let mut records = load(path)?;
        records.push_back(record);
        while records.len() > CAPACITY {
            records.pop_front();
        }
        let mut text = String::new();
        for record in records {
            text.push_str(&serde_json::to_string(&record).map_err(|_| "cannot encode history")?);
            text.push('\n');
        }
        if text.len() > MAX_BYTES {
            return Err("history exceeds byte limit".into());
        }
        let temp = path.with_file_name(format!(
            "{}.agent-dlp-tmp",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
        if fs::symlink_metadata(&temp).is_ok() {
            fs::remove_file(&temp).map_err(|_| "cannot clear history staging file")?;
        }
        let result = storage::create(&temp, &text)
            .and_then(|_| fs::rename(&temp, path).map_err(|_| "cannot replace history".into()));
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    })
}
pub fn list(path: &Path) -> Result<Vec<Value>, String> {
    locked(path, || Ok(load(path)?.into_iter().collect()))
}
