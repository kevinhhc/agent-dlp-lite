use agent_dlp_lite::{
    confirmation, engine,
    event::Event,
    history,
    policy::{Action, CompiledPack},
};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    thread,
    time::Duration,
};
fn policy() -> CompiledPack {
    CompiledPack::load(None).unwrap()
}
fn event(prompt: &str) -> Event {
    Event::parse(&json!({"hook_event_name":"UserPromptSubmit","prompt":prompt}).to_string())
        .unwrap()
}
fn send(url: &str, method: &str, body: &str, origin: Option<&str>) -> String {
    let (address, path) = url
        .strip_prefix("http://")
        .unwrap()
        .split_once('/')
        .unwrap();
    let mut socket = TcpStream::connect(address).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let extra = origin
        .map(|s| format!("Origin: {s}\r\n"))
        .unwrap_or_default();
    write!(socket,"{method} /{path} HTTP/1.1\r\nHost: {address}\r\n{extra}Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    response
}
fn run(driver: impl FnOnce(String) + Send + 'static) -> confirmation::Outcome {
    let mut child = None;
    let result = confirmation::serve_with(
        "TEST-001",
        "<script>unsafe()</script>",
        Duration::from_secs(2),
        |url| {
            let url = url.to_owned();
            child = Some(thread::spawn(move || driver(url)));
            Ok(())
        },
    );
    child
        .expect(
            "confirmation listener unavailable; these protocol tests require loopback networking",
        )
        .join()
        .unwrap();
    result
}
#[test]
fn explicit_user_allow() {
    let result = run(|url| {
        let token = url.rsplit('/').next().unwrap();
        assert!(send(
            &url,
            "POST",
            &format!("token={token}&choice=allow&reason=public+example"),
            None
        )
        .contains("200 OK"));
    });
    assert!(result.allowed);
    assert_eq!(result.source, "user");
    assert_eq!(result.reason, "public example");
}
#[test]
fn explicit_user_deny() {
    let result = run(|url| {
        let token = url.rsplit('/').next().unwrap();
        send(&url, "POST", &format!("token={token}&choice=deny"), None);
    });
    assert!(!result.allowed);
    assert_eq!(result.source, "user");
}
#[test]
fn get_never_approves_and_html_escapes() {
    let result = run(|url| {
        let page = send(&url, "GET", "", None);
        assert!(page.contains("&lt;script&gt;"));
        assert!(!page.contains("<script>unsafe"));
        assert!(page.contains("no-store"));
    });
    assert!(!result.allowed);
    assert_eq!(result.source, "timeout");
}
#[test]
fn bad_token_rejected_before_valid_choice() {
    let result = run(|url| {
        assert!(send(&url, "POST", "token=wrong&choice=allow", None).contains("403"));
        let token = url.rsplit('/').next().unwrap();
        send(&url, "POST", &format!("token={token}&choice=deny"), None);
    });
    assert!(!result.allowed);
    assert_eq!(result.source, "user");
}
#[test]
fn cross_origin_rejected() {
    let result = run(|url| {
        let token = url.rsplit('/').next().unwrap();
        assert!(send(
            &url,
            "POST",
            &format!("token={token}&choice=allow"),
            Some("https://example.invalid")
        )
        .contains("403"));
    });
    assert!(!result.allowed);
    assert_eq!(result.source, "timeout");
}
#[test]
fn duplicate_choices_rejected() {
    let result = run(|url| {
        let token = url.rsplit('/').next().unwrap();
        assert!(send(
            &url,
            "POST",
            &format!("token={token}&choice=allow&choice=deny"),
            None
        )
        .contains("403"));
    });
    assert!(!result.allowed);
}
#[test]
fn missing_browser_denies() {
    let result = confirmation::serve_with("test", "", Duration::from_millis(100), |_| {
        Err("no browser".into())
    });
    assert!(!result.allowed);
    assert_eq!(result.source, "unavailable");
}
#[test]
fn independent_requests_require_independent_choices() {
    assert!(
        run(|url| {
            let token = url.rsplit('/').next().unwrap();
            send(&url, "POST", &format!("token={token}&choice=allow"), None);
        })
        .allowed
    );
    assert!(
        !run(|url| {
            send(&url, "GET", "", None);
        })
        .allowed
    );
}
#[test]
fn confirm_has_safe_default_output() {
    let p = policy();
    let e = event("person@example.invalid");
    let report = engine::evaluate(&e, &p);
    assert_eq!(report.action, Action::Confirm);
    assert_eq!(report.hook_output()["decision"], "block");
}
#[test]
fn deny_overrides_confirm() {
    let p = policy();
    let text = format!("person@example.invalid AKIA{}", "Z".repeat(16));
    assert_eq!(engine::evaluate(&event(&text), &p).action, Action::Deny);
}
#[test]
fn legacy_warn_still_works() {
    let mut p = policy().pack;
    for r in &mut p.rules {
        if r.kind == agent_dlp_lite::policy::Kind::Pii {
            r.action = Action::Warn;
        }
    }
    let p = CompiledPack::parse(&serde_saphyr::to_string(&p).unwrap()).unwrap();
    assert_eq!(
        engine::evaluate(&event("person@example.invalid"), &p).action,
        Action::Warn
    );
}
#[test]
fn decision_content_is_masked_before_truncation() {
    let p = policy();
    let text = format!("person@example.invalid AKIA{}", "Z".repeat(16));
    let e = event(&text);
    let r = engine::evaluate(&e, &p);
    let record = history::record(&e, &p, &r, "user", &text);
    let serialized = record.to_string();
    assert!(!serialized.contains("person@example.invalid"));
    assert!(!serialized.contains(&"Z".repeat(16)));
    assert!(serialized.contains("REDACTED"));
}
#[test]
fn fingerprint_changes_with_policy() {
    let p = policy();
    let e = event("person@example.invalid");
    let a = confirmation::fingerprint(&e, &p).unwrap();
    let mut changed = p.pack.clone();
    changed.rules[0].enabled = false;
    let p = CompiledPack::parse(&serde_saphyr::to_string(&changed).unwrap()).unwrap();
    assert_ne!(a, confirmation::fingerprint(&e, &p).unwrap());
}
#[test]
fn fingerprint_changes_with_script_dependency() {
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("main.py"), "import helper").unwrap();
    fs::write(d.path().join("helper.py"), "x=1").unwrap();
    let e=Event::parse(&json!({"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":d.path(),"tool_input":{"command":"python main.py"}}).to_string()).unwrap();
    let p = policy();
    let before = confirmation::fingerprint(&e, &p).unwrap();
    fs::write(d.path().join("helper.py"), "x=2").unwrap();
    assert_ne!(before, confirmation::fingerprint(&e, &p).unwrap());
}
#[test]
fn history_keeps_exactly_latest_100_without_backups() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("history.jsonl");
    for i in 0..137 {
        history::append(&path, json!({"id":i,"decision":"deny"})).unwrap();
    }
    let records = history::list(&path).unwrap();
    assert_eq!(records.len(), 100);
    assert_eq!(records[0]["id"], 37);
    assert_eq!(records[99]["id"], 136);
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 2);
}
#[test]
fn concurrent_history_writes_do_not_lose_decisions() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("history.jsonl");
    let jobs: Vec<_> = (0..24)
        .map(|i| {
            let path = path.clone();
            thread::spawn(move || history::append(&path, json!({"id":i})).unwrap())
        })
        .collect();
    for j in jobs {
        j.join().unwrap();
    }
    let records = history::list(&path).unwrap();
    assert_eq!(records.len(), 24);
}
#[test]
fn corrupt_history_is_not_silently_discarded() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("history.jsonl");
    fs::write(&p, "invalid").unwrap();
    assert!(history::append(&p, json!({"id":1})).is_err());
    assert_eq!(fs::read_to_string(&p).unwrap(), "invalid");
}
#[cfg(unix)]
#[test]
fn history_permissions_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("history.jsonl");
    history::append(&p, json!({"id":1})).unwrap();
    assert_eq!(fs::metadata(p).unwrap().permissions().mode() & 0o777, 0o600);
}
#[test]
fn histories_are_regular_files() {
    let d = tempfile::tempdir().unwrap();
    assert!(history::append(d.path(), json!({"id":1})).is_err());
}
#[test]
fn missing_script_cannot_be_confirmed() {
    let e=Event::parse(&json!({"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":Path::new("."),"tool_input":{"command":"python nonexistent-confirmation-test.py"}}).to_string()).unwrap();
    assert!(confirmation::fingerprint(&e, &policy()).is_err());
}

#[test]
fn approved_unchanged_operation_is_allowed() {
    let p = policy();
    let e = event("person@example.invalid");
    let result = confirmation::decide(&e, &p, None, "test", true, |_, preview, _| {
        assert!(!preview.contains("person@example.invalid"));
        confirmation::Outcome {
            allowed: true,
            source: "user",
            reason: "public example".into(),
        }
    });
    assert!(result.allowed);
    assert_eq!(result.source, "user");
}
#[test]
fn changed_policy_after_user_allow_is_denied() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("policy.yaml");
    let p = policy();
    fs::write(&path, p.export().unwrap()).unwrap();
    let result = confirmation::decide(
        &event("person@example.invalid"),
        &p,
        Some(&path),
        "test",
        true,
        |_, _, _| {
            fs::write(&path, "invalid changed policy").unwrap();
            confirmation::Outcome {
                allowed: true,
                source: "user",
                reason: "example".into(),
            }
        },
    );
    assert!(!result.allowed);
    assert_eq!(result.source, "changed");
}
#[test]
fn changed_script_after_user_allow_is_denied() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("sample.py");
    fs::write(&path, "print('hello')").unwrap();
    let e=Event::parse(&json!({"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":d.path(),"tool_input":{"command":"python sample.py"}}).to_string()).unwrap();
    let result = confirmation::decide(&e, &policy(), None, "test", true, |_, _, _| {
        fs::write(&path, "print('changed')").unwrap();
        confirmation::Outcome {
            allowed: true,
            source: "user",
            reason: "example".into(),
        }
    });
    assert!(!result.allowed);
    assert_eq!(result.source, "changed");
}
#[test]
fn missing_interactive_flag_never_opens_confirmation() {
    let result = confirmation::decide(
        &event("person@example.invalid"),
        &policy(),
        None,
        "test",
        false,
        |_, _, _| panic!("must not open a UI under legacy timeout"),
    );
    assert!(!result.allowed);
    assert_eq!(result.source, "unavailable");
}
