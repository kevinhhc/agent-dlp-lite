use crate::{
    event::Event,
    policy::{Action, CompiledPack, Kind},
    scripts,
};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub rule_id: String,
    pub category: String,
    pub action: Action,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub event: String,
    pub action: Action,
    pub findings: Vec<Finding>,
    pub inspection_complete: bool,
    pub inspected_files: usize,
    pub enforcement: &'static str,
}

pub fn evaluate(event: &Event, policy: &CompiledPack) -> Report {
    let mut texts = event.texts();
    let mut complete = true;
    let mut files = 0;
    let mut script_texts = Vec::new();
    if event.hook_event_name == "PreToolUse" {
        if let Some(command) = event.command() {
            let inspected =
                scripts::inspect(&command, event.script_cwd().as_deref(), &policy.pack.limits);
            complete = !inspected.incomplete;
            files = inspected.files;
            script_texts = inspected.texts;
            texts.extend(script_texts.iter().cloned());
        } else if event.tool_name.as_deref().is_some_and(|s| {
            [
                "Bash",
                "exec_command",
                "execute_command",
                "shell",
                "shell_command",
            ]
            .contains(&s)
        }) {
            complete = false;
        }
    }
    let mut findings = Vec::new();
    for (rule, re) in &policy.rules {
        if !rule.events.is_empty() && !rule.events.contains(&event.hook_event_name) {
            continue;
        }
        if texts.iter().any(|text| re.is_match(text)) {
            findings.push(Finding {
                rule_id: rule.id.clone(),
                category: match rule.kind {
                    Kind::Secret => "secret",
                    Kind::Pii => "pii",
                    Kind::SensitiveFile => "sensitive_file",
                    Kind::Custom => "custom",
                }
                .into(),
                action: rule.action,
                confidence: "pattern_match".into(),
            });
        }
    }
    // Across the inspected closure: may over-approximate branches and unrelated variables.
    for (flow, source, sink) in &policy.flows {
        if script_texts.iter().any(|s| source.is_match(s))
            && script_texts.iter().any(|s| sink.is_match(s))
        {
            findings.push(Finding {
                rule_id: flow.id.clone(),
                category: "script_flow".into(),
                action: flow.action,
                confidence: "potential".into(),
            });
        }
    }
    if !complete {
        findings.push(Finding {
            rule_id: "INSPECTION-INCOMPLETE".into(),
            category: "coverage".into(),
            action: policy.pack.inspection_failure,
            confidence: "incomplete".into(),
        });
    }
    let action = findings
        .iter()
        .map(|f| f.action)
        .max()
        .unwrap_or(Action::Allow);
    Report {
        schema_version: 1,
        event: event.hook_event_name.clone(),
        action,
        findings,
        inspection_complete: complete,
        inspected_files: files,
        enforcement: if event.hook_event_name == "PostToolUse" {
            "observe_only"
        } else {
            "before_action"
        },
    }
}

impl Report {
    pub fn reason(&self) -> String {
        format!(
            "Agent DLP Lite: {:?}; rules={}",
            self.action,
            self.findings
                .iter()
                .map(|f| f.rule_id.as_str())
                .collect::<Vec<_>>()
                .join(",")
        )
    }
    /// No explicit 'allow': preserve the host's existing permission checks.
    pub fn hook_output(&self) -> Value {
        match (self.event.as_str(), self.action) {
            ("PreToolUse", Action::Deny | Action::Confirm) => json!({"hookSpecificOutput": {
                "hookEventName":"PreToolUse", "permissionDecision":"deny", "permissionDecisionReason":self.reason()
            }}),
            ("UserPromptSubmit", Action::Deny | Action::Confirm) => {
                json!({"decision":"block", "reason":self.reason()})
            }
            (_, Action::Allow) => json!({}),
            _ => json!({"systemMessage": self.reason()}),
        }
    }
}
