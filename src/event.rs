use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

pub const EVENTS: &[&str] = &["PreToolUse", "UserPromptSubmit", "PostToolUse"];

#[derive(Debug, Deserialize)]
pub struct Event {
    pub hook_event_name: String,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_input: Value,
    #[serde(default)]
    pub tool_response: Value,
    #[serde(default)]
    pub prompt: Option<String>,
}

impl Event {
    pub fn parse(text: &str) -> Result<Self, String> {
        let event: Self = serde_json::from_str(text).map_err(|_| "invalid hook event JSON")?;
        if !EVENTS.contains(&event.hook_event_name.as_str()) {
            return Err("unsupported hook event".into());
        }
        if event.hook_event_name == "UserPromptSubmit" {
            if event.prompt.is_none() {
                return Err("prompt is required".into());
            }
        } else if event.tool_name.as_ref().is_none_or(|s| s.is_empty())
            || event.tool_input.is_null()
        {
            return Err("tool_name and tool_input are required".into());
        }
        Ok(event)
    }
    pub fn texts(&self) -> Vec<String> {
        fn walk(v: &Value, out: &mut Vec<String>) {
            match v {
                Value::String(s) => out.push(s.clone()),
                Value::Array(a) => a.iter().for_each(|v| walk(v, out)),
                // Include object keys: some tools transmit user data as keys.
                Value::Object(o) => o.iter().for_each(|(k, v)| {
                    out.push(k.clone());
                    walk(v, out);
                }),
                Value::Number(n) => out.push(n.to_string()),
                _ => (),
            }
        }
        let mut out = Vec::new();
        if let Some(p) = &self.prompt {
            out.push(p.clone());
        }
        walk(&self.tool_input, &mut out);
        if self.hook_event_name == "PostToolUse" {
            walk(&self.tool_response, &mut out);
        }
        out
    }
    pub fn command(&self) -> Option<String> {
        let name = self.tool_name.as_deref()?.to_lowercase();
        if ![
            "bash",
            "shell",
            "shell_command",
            "exec_command",
            "execute_command",
            "run_command",
            "terminal",
        ]
        .contains(&name.as_str())
        {
            return None;
        }
        let v = self
            .tool_input
            .get("command")
            .or_else(|| self.tool_input.get("cmd"))?;
        match v {
            Value::String(s) => Some(s.clone()),
            Value::Array(a) => {
                let args: Option<Vec<&str>> = a.iter().map(Value::as_str).collect();
                args.map(|a| {
                    a.iter()
                        .map(|s| {
                            shlex::try_quote(s)
                                .map(|s| s.into_owned())
                                .unwrap_or_default()
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
            }
            _ => None,
        }
    }
    pub fn script_cwd(&self) -> Option<PathBuf> {
        let base = self.cwd.as_ref()?;
        let sub = self
            .tool_input
            .get("workdir")
            .or_else(|| self.tool_input.get("cwd"))
            .and_then(Value::as_str);
        Some(sub.map_or_else(|| base.clone(), |p| base.join(p)))
    }
}
