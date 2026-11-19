//! Bounded JSON-RPC over stdio. No browser, listener, remote API or model sampling.
use crate::{
    confirmation, engine,
    event::Event,
    history,
    policy::{Action, CompiledPack},
    storage,
};
use serde_json::{json, Value};
use std::{
    io::{self, BufRead, Read, Write},
    path::Path,
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

const MAX_RPC_BYTES: usize = 1024 * 1024;
const SERVER: &str = "agent-dlp-lite";
pub fn server_name() -> &'static str {
    SERVER
}

fn send(out: &mut impl Write, value: Value) -> Result<(), String> {
    writeln!(out, "{value}")
        .and_then(|_| out.flush())
        .map_err(|_| "cannot write MCP response".into())
}
fn result(out: &mut impl Write, id: Value, value: Value) -> Result<(), String> {
    send(out, json!({"jsonrpc":"2.0", "id":id, "result":value}))
}
fn error(out: &mut impl Write, id: Value, code: i32, message: &str) -> Result<(), String> {
    send(
        out,
        json!({"jsonrpc":"2.0", "id":id, "error":{"code":code,"message":message}}),
    )
}
fn tool_result(value: Value) -> Value {
    json!({"content":[{"type":"text","text":value.to_string()}], "structuredContent":value, "isError":false})
}
fn supports_form(capabilities: &Value) -> bool {
    capabilities
        .get("elicitation")
        .and_then(Value::as_object)
        .is_some_and(|e| e.is_empty() || e.get("form").is_some_and(Value::is_object))
}
fn outcome(allowed: bool, source: &'static str, reason: &str) -> confirmation::Outcome {
    confirmation::Outcome {
        allowed,
        source,
        reason: reason.into(),
    }
}
/// Missing/invalid UI is a warning continuation, never explicit human feedback.
pub fn is_fallback(source: &str) -> bool {
    matches!(source, "unavailable" | "timeout" | "protocol_error")
}
pub fn parse_choice(response: &Value) -> confirmation::Outcome {
    if response.get("error").is_some() {
        return outcome(
            false,
            "unavailable",
            "client confirmation is unavailable; warning continuation",
        );
    }
    let automated = ["_meta", "meta"]
        .iter()
        .any(|key| response["result"][key]["approvals_reviewer"] == "auto_review");
    if automated {
        return if response["result"]["action"] == "accept" {
            outcome(
                false,
                "protocol_error",
                "automatic approval is not a user choice; warning continuation",
            )
        } else {
            outcome(
                false,
                "automatic_review",
                "automatic reviewer declined confirmation",
            )
        };
    }
    match response["result"]["action"].as_str() {
        // A bare decline may also come from client policy: do not label it human feedback.
        Some("decline") => outcome(
            false,
            "declined",
            "confirmation declined by user or client policy",
        ),
        Some("cancel") => outcome(false, "cancelled", "user cancelled confirmation"),
        Some("accept") => {
            let content = &response["result"]["content"];
            let reason = match content.get("reason") {
                None => "",
                Some(Value::String(s)) if s.chars().count() <= 200 => s,
                _ => {
                    return outcome(
                        false,
                        "protocol_error",
                        "invalid confirmation reason; warning continuation",
                    )
                }
            };
            match content["decision"].as_str() {
                Some("allow_once") => outcome(true, "user", reason),
                Some("deny") => outcome(false, "user", reason),
                _ => outcome(
                    false,
                    "protocol_error",
                    "no explicit valid choice; warning continuation",
                ),
            }
        }
        _ => outcome(
            false,
            "protocol_error",
            "invalid confirmation response; warning continuation",
        ),
    }
}
fn ask(
    input: &Receiver<Result<Value, String>>,
    out: &mut impl Write,
    counter: &mut u64,
    parent_id: &Value,
    summary: &str,
    preview: &str,
    timeout: Duration,
) -> confirmation::Outcome {
    *counter += 1;
    let id = json!(format!("agent-dlp-confirm-{}", *counter));
    let request = json!({"jsonrpc":"2.0","id":id,"method":"elicitation/create","params":{
        "mode":"form", "message":format!("Agent DLP Lite：{summary}\n{preview}\n请选择本次操作是否继续。不会修改策略或批准其他权限。"),
        "requestedSchema":{"type":"object","properties":{
            "decision":{"type":"string","title":"本次选择 / Decision","enum":["deny","allow_once"]},
            "reason":{"type":"string","title":"原因（可选，请勿填写真实秘密）","maxLength":200}
        },"required":["decision"]}
    }});
    if send(out, request).is_err() {
        return outcome(
            false,
            "unavailable",
            "cannot request confirmation; warning continuation",
        );
    }
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match input.recv_timeout(remaining) {
            Ok(Ok(message)) => {
                if message["jsonrpc"] == "2.0"
                    && message.get("method").is_none()
                    && message.get("id") == Some(&id)
                {
                    return parse_choice(&message);
                }
                match message["method"].as_str() {
                    Some("ping") => {
                        let _ = result(out, message["id"].clone(), json!({}));
                    }
                    Some("notifications/cancelled") => {
                        // Ignore cancellation for unrelated requests.
                        if message["params"]["requestId"] == id
                            || &message["params"]["requestId"] == parent_id
                        {
                            return outcome(false, "cancelled", "confirmation was cancelled");
                        }
                    }
                    Some(_) if message.get("id").is_some() => {
                        let _ = error(
                            out,
                            message["id"].clone(),
                            -32000,
                            "server busy awaiting user confirmation",
                        );
                    }
                    _ => (),
                }
            }
            Ok(Err(_)) => {
                return outcome(
                    false,
                    "protocol_error",
                    "invalid MCP input; warning continuation",
                )
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(_) => {
                return outcome(
                    false,
                    "unavailable",
                    "MCP connection closed; warning continuation",
                )
            }
        }
    }
    let _ = send(
        out,
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":id,"reason":"confirmation timed out"}}),
    );
    outcome(
        false,
        "timeout",
        "confirmation timed out; warning continuation",
    )
}

fn inspect(
    raw: &Value,
    policy_path: Option<&Path>,
    history_path: &Path,
    request: impl FnOnce(&str, &str, Duration) -> confirmation::Outcome,
) -> Result<Value, String> {
    let policy = CompiledPack::load(policy_path)?;
    let text = serde_json::to_string(raw).map_err(|_| "invalid event")?;
    if text.len() > policy.pack.limits.max_event_bytes {
        return Err("event exceeds byte limit".into());
    }
    let event = Event::parse(&text)?;
    let mut report = engine::evaluate(&event, &policy);
    if report.action == Action::Allow || event.hook_event_name == "PostToolUse" {
        return Ok(report.hook_output());
    }
    let mut source = "policy";
    let mut reason = String::new();
    if report.action == Action::Confirm {
        // Persist fallback before requesting UI. The independent command hook covers server startup failures.
        let mut baseline = report.clone();
        baseline.action = Action::Warn;
        history::append(
            history_path,
            history::record(
                &event,
                &policy,
                &baseline,
                "companion",
                "native confirmation pending or unavailable; not human approval",
            ),
        )?;
        let answer = confirmation::decide(
            &event,
            &policy,
            policy_path,
            &report.reason(),
            true,
            request,
        );
        report.action = if is_fallback(answer.source) {
            Action::Warn
        } else if answer.allowed {
            Action::Allow
        } else {
            Action::Deny
        };
        source = answer.source;
        reason = answer.reason;
    }
    history::append(
        history_path,
        history::record(&event, &policy, &report, source, &reason),
    )?;
    let mut output = report.hook_output();
    if source != "policy" && report.action != Action::Allow {
        output["systemMessage"] = json!(format!(
            "Agent DLP Lite: {source}; {}",
            history::redacted(&reason, &policy, 256)
        ));
    }
    Ok(output)
}

pub fn serve(
    policy_path: Option<&Path>,
    history_path: &Path,
    timeout: Duration,
) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel::<Result<Value, String>>(32);
    thread::spawn(move || {
        let mut reader = io::stdin().lock();
        loop {
            let mut line = Vec::new();
            match (&mut reader)
                .take((MAX_RPC_BYTES + 1) as u64)
                .read_until(b'\n', &mut line)
            {
                Ok(0) | Err(_) => break,
                Ok(_) if line.len() > MAX_RPC_BYTES => {
                    let _ = tx.send(Err("RPC exceeds byte limit".into()));
                    break;
                }
                Ok(_) => {
                    let message =
                        serde_json::from_slice(&line).map_err(|_| "invalid JSON-RPC".into());
                    if tx.send(message).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let mut out = io::stdout().lock();
    let mut initialized = false;
    let mut forms = false;
    let mut counter = 0;
    while let Ok(message) = rx.recv() {
        let message = match message {
            Ok(v) => v,
            Err(_) => {
                error(
                    &mut out,
                    Value::Null,
                    -32700,
                    "invalid or oversized JSON-RPC",
                )?;
                continue;
            }
        };
        let Some(method) = message["method"].as_str() else {
            continue;
        }; // Includes stale responses after a timeout.
        let Some(id) = message.get("id").cloned() else {
            continue;
        };
        if message["jsonrpc"] != "2.0" || !(id.is_string() || id.is_number()) {
            error(&mut out, Value::Null, -32600, "invalid JSON-RPC request")?;
            continue;
        }
        match method {
            "initialize" => {
                if initialized {
                    error(&mut out, id, -32600, "already initialized")?;
                    continue;
                }
                let requested = message["params"]["protocolVersion"].as_str().unwrap_or("");
                let version = match requested {
                    "2024-11-05" | "2025-03-26" | "2025-06-18" | "2025-11-25" => requested,
                    _ => "2025-11-25",
                };
                forms = supports_form(&message["params"]["capabilities"]);
                initialized = true;
                result(
                    &mut out,
                    id,
                    json!({"protocolVersion":version,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":SERVER,"version":env!("CARGO_PKG_VERSION")}}),
                )?;
            }
            "ping" => result(&mut out, id, json!({}))?,
            _ if !initialized => error(&mut out, id, -32002, "initialize first")?,
            "tools/list" => result(
                &mut out,
                id,
                json!({"tools":[{
                    "name":"inspect","description":"Inspect a local Codex hook event and request a per-operation DLP decision when supported.",
                    "inputSchema":{"type":"object","properties":{"event":{"type":"object"}},"required":["event"],"additionalProperties":false},
                    "annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}
                }]}),
            )?,
            "tools/call" => {
                if message["params"]["name"] != "inspect" {
                    error(&mut out, id, -32602, "unknown tool")?;
                    continue;
                }
                let output = inspect(
                    &message["params"]["arguments"]["event"],
                    policy_path,
                    history_path,
                    |summary, preview, _| {
                        if forms {
                            ask(&rx, &mut out, &mut counter, &id, summary, preview, timeout)
                        } else {
                            outcome(
                                false,
                                "unavailable",
                                "client lacks form elicitation; warning continuation",
                            )
                        }
                    },
                )
                .unwrap_or_else(|_| {
                    let message = "Agent DLP Lite: inspection or history failed; operation denied";
                    json!({"decision":"block","reason":message,"systemMessage":message})
                });
                result(&mut out, id, tool_result(output))?;
            }
            _ => error(&mut out, id, -32601, "method not found")?,
        }
    }
    Ok(())
}

pub fn config(
    executable: &Path,
    policy: &Path,
    history: &Path,
) -> Result<toml_edit::Table, String> {
    fn as_text(p: &Path) -> Result<&str, &str> {
        p.to_str()
            .filter(|s| !s.chars().any(char::is_control))
            .ok_or("invalid configuration path")
    }
    let mut table = toml_edit::Table::new();
    table["command"] = toml_edit::value(as_text(executable)?);
    let mut args = toml_edit::Array::new();
    for arg in [
        "--policy",
        as_text(policy)?,
        "--history-file",
        as_text(history)?,
        "mcp",
        "--managed-by",
        "agent-dlp-lite",
    ] {
        args.push(arg);
    }
    table["args"] = toml_edit::value(args);
    table["startup_timeout_sec"] = toml_edit::value(10);
    table["tool_timeout_sec"] = toml_edit::value(90);
    Ok(table)
}
fn owned(table: &toml_edit::Item) -> bool {
    table
        .get("args")
        .and_then(toml_edit::Item::as_array)
        .is_some_and(|a| {
            a.len() >= 3
                && a.get(a.len() - 3).and_then(toml_edit::Value::as_str) == Some("mcp")
                && a.get(a.len() - 2).and_then(toml_edit::Value::as_str) == Some("--managed-by")
                && a.get(a.len() - 1).and_then(toml_edit::Value::as_str) == Some(SERVER)
        })
}
pub fn merge_config(text: &str, table: toml_edit::Table) -> Result<String, String> {
    let mut doc = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| "MCP config must be valid TOML")?;
    if doc.get("mcp_servers").is_some_and(|v| !v.is_table()) {
        return Err("mcp_servers must be a table".into());
    }
    if let Some(old) = doc.get("mcp_servers").and_then(|v| v.get(SERVER)) {
        if !owned(old) {
            return Err("MCP server name belongs to another configuration".into());
        }
    }
    if doc.get("mcp_servers").is_none() {
        doc["mcp_servers"] = toml_edit::Item::Table(toml_edit::Table::new());
    }
    doc["mcp_servers"][SERVER] = toml_edit::Item::Table(table);
    Ok(doc.to_string())
}
pub fn remove_config(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    let text = storage::read_text(path, 1024 * 1024)?;
    let mut doc = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| "MCP config must be valid TOML")?;
    if doc
        .get("mcp_servers")
        .and_then(|v| v.get(SERVER))
        .is_some_and(owned)
    {
        doc["mcp_servers"]
            .as_table_mut()
            .ok_or("invalid mcp_servers")?
            .remove(SERVER);
        storage::replace_with_backup(path, &doc.to_string())?;
        return Ok(true);
    }
    Ok(false)
}
