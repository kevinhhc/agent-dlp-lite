use agent_dlp_lite::{
    engine,
    event::Event,
    history, mcp,
    policy::{Action, CompiledPack, DEFAULT_POLICY},
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};
fn test_policy() -> String {
    DEFAULT_POLICY.replace("\r\n", "\n").replace("rules:\n", "rules:\n  - id: TEST-CONFIRM\n    kind: custom\n    pattern: DLP_SYNTHETIC_MARKER\n    action: confirm\n")
}
struct Client {
    child: Child,
    stdin: ChildStdin,
    incoming: Receiver<Value>,
    dir: tempfile::TempDir,
}
impl Client {
    fn new(capabilities: Value) -> Self {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("policy.yaml"), test_policy()).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_agent-dlp"))
            .args([
                "--policy",
                dir.path().join("policy.yaml").to_str().unwrap(),
                "--history-file",
                dir.path().join("history.jsonl").to_str().unwrap(),
                "mcp",
                "--confirmation-timeout",
                "1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, incoming) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                if tx
                    .send(serde_json::from_str(&line).expect("only JSON-RPC on stdout"))
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut c = Self {
            child,
            stdin,
            incoming,
            dir,
        };
        c.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":capabilities,"clientInfo":{"name":"protocol-test","version":"1"}}}));
        let init = c.next();
        assert_eq!(init["result"]["serverInfo"]["version"], "0.2.2");
        assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
        c.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        c
    }
    fn send(&mut self, v: Value) {
        writeln!(self.stdin, "{v}").unwrap();
        self.stdin.flush().unwrap();
    }
    fn next(&self) -> Value {
        self.incoming
            .recv_timeout(Duration::from_secs(8))
            .expect("MCP response")
    }
    fn call(&mut self, event: Value) {
        self.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"inspect","arguments":{"event":event}}}));
    }
    fn answer(&mut self, id: Value, decision: &str) {
        self.send(json!({"jsonrpc":"2.0","id":id,"result":{"action":"accept","content":{"decision":decision,"reason":"synthetic test"}}}));
    }
    fn records(&self) -> Vec<Value> {
        history::list(&self.dir.path().join("history.jsonl")).unwrap()
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn prompt() -> Value {
    json!({"hook_event_name":"UserPromptSubmit","session_id":"test-session","turn_id":"test-turn","prompt":"DLP_SYNTHETIC_MARKER"})
}
fn native() -> Client {
    Client::new(json!({"elicitation":{"form":{}}}))
}
fn output(message: &Value) -> &Value {
    assert_eq!(message["id"], 2);
    assert_eq!(message["result"]["isError"], false);
    &message["result"]["structuredContent"]
}

#[test]
fn native_form_waits_for_explicit_choice_and_masks_preview() {
    let mut c = native();
    c.call(prompt());
    let form = c.next();
    assert_eq!(form["method"], "elicitation/create");
    assert_eq!(form["params"]["mode"], "form");
    assert_eq!(
        form["params"]["requestedSchema"]["required"],
        json!(["decision"])
    );
    assert!(!form.to_string().contains("DLP_SYNTHETIC_MARKER"));
    assert!(c.incoming.recv_timeout(Duration::from_millis(60)).is_err());
    c.answer(form["id"].clone(), "allow_once");
    assert_eq!(output(&c.next()), &json!({}));
    let r = c.records();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["decision"], "allow");
    assert_eq!(r[0]["decision_source"], "user");
    assert_eq!(r[0]["human_feedback"], true);
}
#[test]
fn native_explicit_deny_blocks_prompt() {
    let mut c = native();
    c.call(prompt());
    let form = c.next();
    c.answer(form["id"].clone(), "deny");
    assert_eq!(output(&c.next())["decision"], "block");
    assert_eq!(c.records()[0]["decision"], "deny");
}
#[test]
fn native_decline_and_cancel_block() {
    for action in ["decline", "cancel"] {
        let mut c = native();
        c.call(prompt());
        let form = c.next();
        c.send(json!({"jsonrpc":"2.0","id":form["id"],"result":{"action":action}}));
        assert_eq!(output(&c.next())["decision"], "block");
        assert_eq!(c.records()[0]["decision"], "deny");
    }
}
#[test]
fn lack_of_form_capability_warns_continues_and_is_not_feedback() {
    for capabilities in [json!({}), json!({"elicitation":{"url":{}}})] {
        let mut c = Client::new(capabilities);
        c.call(prompt());
        let reply = c.next();
        assert!(output(&reply)["decision"].is_null());
        assert!(output(&reply)["systemMessage"]
            .as_str()
            .unwrap()
            .contains("unavailable"));
        let r = c.records();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["decision"], "allow");
        assert_eq!(r[0]["human_feedback"], false);
    }
}
#[test]
fn legacy_empty_capability_means_form_support() {
    let mut c = Client::new(json!({"elicitation":{}}));
    c.call(prompt());
    let form = c.next();
    assert_eq!(form["method"], "elicitation/create");
    c.answer(form["id"].clone(), "deny");
    assert_eq!(output(&c.next())["decision"], "block");
}
#[test]
fn empty_accept_cannot_be_mistaken_for_user_approval() {
    let mut c = native();
    c.call(prompt());
    let form = c.next();
    c.send(json!({"jsonrpc":"2.0","id":form["id"],"result":{"action":"accept","content":{}}}));
    assert!(output(&c.next())["decision"].is_null());
    assert_eq!(c.records()[0]["human_feedback"], false);
    assert_eq!(c.records()[0]["decision_source"], "protocol_error");
}
#[test]
fn unsupported_elicitation_error_is_a_warning_continuation() {
    let mut c = native();
    c.call(prompt());
    let form = c.next();
    c.send(
        json!({"jsonrpc":"2.0","id":form["id"],"error":{"code":-32601,"message":"unsupported"}}),
    );
    assert!(output(&c.next())["decision"].is_null());
    assert_eq!(c.records()[0]["decision_source"], "unavailable");
}
#[test]
fn timeout_cancels_form_and_records_nonhuman_continuation() {
    let mut c = native();
    c.call(prompt());
    let _form = c.next();
    let cancellation = c.next();
    assert_eq!(cancellation["method"], "notifications/cancelled");
    assert!(output(&c.next())["decision"].is_null());
    assert_eq!(c.records()[0]["decision_source"], "timeout");
    assert_eq!(c.records()[0]["human_feedback"], false);
}
#[test]
fn unrelated_response_and_ping_cannot_approve_operation() {
    let mut c = native();
    c.call(prompt());
    let form = c.next();
    c.answer(json!("wrong-id"), "allow_once");
    c.send(json!({"jsonrpc":"2.0","id":9,"method":"ping"}));
    assert_eq!(c.next()["id"], 9);
    c.answer(form["id"].clone(), "deny");
    assert_eq!(output(&c.next())["decision"], "block");
}
#[test]
fn policy_change_during_native_confirmation_denies_even_after_allow() {
    let mut c = native();
    c.call(prompt());
    let form = c.next();
    fs::write(
        c.dir.path().join("policy.yaml"),
        test_policy().replace("action: confirm", "action: deny"),
    )
    .unwrap();
    c.answer(form["id"].clone(), "allow_once");
    assert_eq!(output(&c.next())["decision"], "block");
    assert_eq!(c.records()[0]["decision_source"], "changed");
}
#[test]
fn hard_deny_never_requests_confirmation_and_clean_input_has_no_history() {
    let mut c = native();
    let mut event = prompt();
    event["prompt"] = json!("ordinary public task");
    c.call(event.clone());
    assert_eq!(output(&c.next()), &json!({}));
    assert!(c.records().is_empty());
    event["prompt"] = json!(format!("AKIA{}", "Z".repeat(16)));
    c.call(event);
    assert_eq!(output(&c.next())["decision"], "block");
    assert_eq!(c.records().len(), 1);
}
#[test]
fn native_pretool_deny_has_correct_host_shape() {
    let mut c = native();
    c.call(json!({"hook_event_name":"PreToolUse","session_id":"s","turn_id":"t","tool_use_id":"u","tool_name":"Bash","cwd":c.dir.path(),"tool_input":{"command":"echo DLP_SYNTHETIC_MARKER"}}));
    let form = c.next();
    c.answer(form["id"].clone(), "deny");
    assert_eq!(
        output(&c.next())["hookSpecificOutput"]["permissionDecision"],
        "deny"
    );
}
#[test]
fn companion_and_native_are_deduplicated_in_both_write_orders() {
    let policy = CompiledPack::parse(&test_policy()).unwrap();
    let event = Event::parse(&prompt().to_string()).unwrap();
    let mut report = engine::evaluate(&event, &policy);
    report.action = Action::Warn;
    let baseline = history::record(&event, &policy, &report, "companion", "");
    report.action = Action::Deny;
    let user = history::record(&event, &policy, &report, "user", "test");
    for reverse in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("history.jsonl");
        let pair = if reverse {
            [user.clone(), baseline.clone()]
        } else {
            [baseline.clone(), user.clone()]
        };
        for r in pair {
            history::append(&p, r).unwrap();
        }
        let records = history::list(&p).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["decision_source"], "user");
        assert!(!records[0].to_string().contains("test-session"));
    }
}
#[test]
fn toml_merge_preserves_user_configuration_and_refuses_name_collision() {
    let p = std::path::Path::new("/trusted/agent-dlp");
    let config = mcp::config(p, p, p).unwrap();
    let original =
        "# preserve comment\nmodel = 'example'\n[mcp_servers.other]\ncommand = 'existing'\n";
    let text = mcp::merge_config(original, config.clone()).unwrap();
    assert!(text.contains("# preserve comment"));
    assert!(text.contains("[mcp_servers.other]"));
    assert!(text.contains("--managed-by"));
    assert!(mcp::merge_config(&text, config.clone()).is_ok());
    assert!(mcp::merge_config(
        "[mcp_servers.agent-dlp-lite]\ncommand='another'\n",
        config.clone()
    )
    .is_err());
    assert!(mcp::merge_config("{invalid", config).is_err());
}

#[test]
fn automatic_review_is_not_recorded_as_human_feedback() {
    for action in ["accept", "decline"] {
        let mut c = native();
        c.call(prompt());
        let form = c.next();
        c.send(json!({"jsonrpc":"2.0","id":form["id"],"result":{"action":action,"content":{"decision":"allow_once"},"_meta":{"approvals_reviewer":"auto_review"}}}));
        let reply = c.next();
        if action == "decline" {
            assert_eq!(output(&reply)["decision"], "block");
        } else {
            assert!(output(&reply)["decision"].is_null());
        }
        assert_eq!(c.records()[0]["human_feedback"], false);
    }
}

#[test]
fn cancellation_of_parent_call_blocks_and_records_cancellation() {
    let mut c = native();
    c.call(prompt());
    let _form = c.next();
    c.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":2}}));
    assert_eq!(output(&c.next())["decision"], "block");
    assert_eq!(c.records()[0]["decision_source"], "cancelled");
}

#[test]
fn failed_history_returns_valid_block_instead_of_fail_open_tool_error() {
    let mut c = native();
    fs::create_dir(c.dir.path().join("history.jsonl")).unwrap();
    c.call(prompt());
    assert_eq!(output(&c.next())["decision"], "block");
}

#[test]
fn mcp_config_removal_preserves_unrelated_servers() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("config.toml");
    let executable = d.path().join("agent-dlp");
    fs::write(
        &path,
        mcp::merge_config(
            "[mcp_servers.other]\ncommand='keep'\n",
            mcp::config(&executable, &executable, &executable).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(mcp::remove_config(&path).unwrap());
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("[mcp_servers.other]"));
    assert!(!text.contains("[mcp_servers.agent-dlp-lite]"));
    assert!(!mcp::remove_config(&path).unwrap());
}
