use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};
fn invoke(args: &[&str], input: &str) -> Output {
    let scratch = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_agent-dlp"));
    if !args.contains(&"--history-file") {
        command
            .arg("--history-file")
            .arg(scratch.path().join("history.jsonl"));
    }
    let mut child = command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
fn text(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn value(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap()
}
fn secret_event() -> String {
    json!({"hook_event_name":"UserPromptSubmit","prompt":format!("AKIA{}","Z".repeat(16))})
        .to_string()
}

#[test]
fn adapters_have_identical_decisions() {
    for agent in ["codex", "workbuddy"] {
        let out = invoke(&["hook", "--agent", agent], &secret_event());
        assert!(out.status.success());
        assert_eq!(value(&out)["decision"], "block");
    }
}
#[test]
fn hook_error_exit_two_and_no_data_echo() {
    let out = invoke(&["hook", "--agent", "codex"], "invalid sensitive example");
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("sensitive example"));
}
#[test]
fn missing_explicit_policy_does_not_fall_back() {
    let d = tempfile::tempdir().unwrap();
    let out = invoke(
        &[
            "--policy",
            text(&d.path().join("missing.yaml")),
            "hook",
            "--agent",
            "codex",
        ],
        "{}",
    );
    assert_eq!(out.status.code(), Some(2));
}
#[test]
fn scan_deny_exit_two() {
    assert_eq!(invoke(&["scan"], &secret_event()).status.code(), Some(2));
}
#[test]
fn huge_event_is_rejected_without_echo() {
    let data =
        json!({"hook_event_name":"UserPromptSubmit","prompt":"x".repeat(270000)}).to_string();
    let out = invoke(&["hook", "--agent", "codex"], &data);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
}
#[test]
fn install_is_idempotent_preserves_other_hooks_and_uninstalls() {
    let d = tempfile::tempdir().unwrap();
    let policy = d.path().join("policy.yaml");
    let settings = d.path().join("settings.json");
    assert!(invoke(&["init", "-o", text(&policy)], "").status.success());
    let original = json!({"theme":"dark","hooks":{"PreToolUse":[{"matcher":"*","hooks":[{"type":"command","command":"echo existing"}]}]}});
    fs::write(&settings, serde_json::to_string(&original).unwrap()).unwrap();
    for _ in 0..2 {
        assert!(invoke(
            &[
                "--policy",
                text(&policy),
                "install",
                "--agent",
                "codex",
                "--settings",
                text(&settings)
            ],
            ""
        )
        .status
        .success());
    }
    let status = invoke(&["status", "--settings", text(&settings)], "");
    assert_eq!(value(&status)["registered_handlers"], 2);
    let mut edited: Value = serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    edited["later"] = json!("preserve me");
    fs::write(&settings, edited.to_string()).unwrap();
    assert!(invoke(&["uninstall", "--settings", text(&settings)], "")
        .status
        .success());
    let remaining: Value = serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(remaining["later"], "preserve me");
    assert_eq!(
        remaining["hooks"]["PreToolUse"],
        original["hooks"]["PreToolUse"]
    );
    assert!(fs::read_dir(d.path())
        .unwrap()
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().contains("backup")));
}
#[test]
fn malformed_settings_not_overwritten() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("p.yaml");
    let s = d.path().join("settings.json");
    invoke(&["init", "-o", text(&p)], "");
    fs::write(&s, "{invalid}").unwrap();
    assert!(!invoke(
        &[
            "--policy",
            text(&p),
            "install",
            "--agent",
            "workbuddy",
            "--settings",
            text(&s)
        ],
        ""
    )
    .status
    .success());
    assert_eq!(fs::read_to_string(&s).unwrap(), "{invalid}");
}
#[test]
fn init_refuses_overwrite() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("p.yaml");
    assert!(invoke(&["init", "-o", text(&p)], "").status.success());
    assert!(!invoke(&["init", "-o", text(&p)], "").status.success());
}
#[test]
fn portable_policy_export_import() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("a.yaml");
    let b = d.path().join("b.yaml");
    assert!(invoke(&["policy", "export", "-o", text(&a)], "")
        .status
        .success());
    assert!(invoke(&["policy", "import", text(&a), "-o", text(&b)], "")
        .status
        .success());
    for agent in ["codex", "workbuddy"] {
        assert_eq!(
            value(&invoke(
                &["--policy", text(&b), "hook", "--agent", agent],
                &secret_event()
            ))["decision"],
            "block"
        );
    }
}
#[test]
fn alert_history_is_redacted_and_private() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("audit.jsonl");
    let out = invoke(
        &["hook", "--agent", "codex", "--audit", text(&a)],
        &secret_event(),
    );
    assert!(out.status.success());
    let log = fs::read_to_string(&a).unwrap();
    assert!(!log.contains(&"Z".repeat(16)));
    assert!(!log.contains("prompt"));
    assert_eq!(
        serde_json::from_str::<Value>(log.trim()).unwrap()["action"],
        "deny"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(a).unwrap().permissions().mode() & 0o777, 0o600);
    }
}
#[cfg(unix)]
#[test]
fn refuses_symlink_audit_and_settings() {
    let d = tempfile::tempdir().unwrap();
    let target = d.path().join("target");
    fs::write(&target, "original").unwrap();
    let link = d.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert_eq!(
        invoke(
            &["hook", "--agent", "codex", "--audit", text(&link)],
            &secret_event()
        )
        .status
        .code(),
        Some(0)
    );
    assert_eq!(fs::read_to_string(target).unwrap(), "original");
}
#[test]
fn self_test() {
    assert_eq!(value(&invoke(&["self-test"], ""))["passed"], true);
}
#[test]
fn integration_requires_pinned_policy() {
    assert!(!invoke(&["integration", "--agent", "codex"], "")
        .status
        .success());
}
#[test]
fn shell_safe_install_command() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("policy with spaces.yaml");
    invoke(&["init", "-o", text(&p)], "");
    let out = invoke(
        &["--policy", text(&p), "integration", "--agent", "codex"],
        "",
    );
    assert!(out.status.success());
    let cfg = value(&out);
    let command = cfg["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    #[cfg(not(windows))]
    {
        let parts = shlex::split(command).unwrap();
        assert!(parts.contains(&text(&p.canonicalize().unwrap()).to_owned()));
    }
    assert!(!command.contains("permissionDecision"));
}

#[test]
fn generated_hook_runs_through_host_shell() {
    let directory = tempfile::tempdir().unwrap();
    let policy = directory.path().join("policy with spaces.yaml");
    assert!(invoke(&["init", "-o", text(&policy)], "").status.success());
    let output = invoke(
        &[
            "--history-file",
            text(&directory.path().join("history.jsonl")),
            "--policy",
            text(&policy),
            "integration",
            "--agent",
            "codex",
        ],
        "",
    );
    let config = value(&output);
    let line = config["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    #[cfg(windows)]
    let mut shell = {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new("cmd.exe");
        // cmd uses shell syntax rather than CRT argument escaping.
        command
            .args(["/D", "/S", "/C"])
            .raw_arg(format!("\"{line}\""));
        command
    };
    #[cfg(not(windows))]
    let mut shell = {
        let mut command = Command::new("sh");
        command.args(["-c", line]);
        command
    };
    let mut child = shell
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(secret_event().as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(value(&result)["decision"], "block");
}

#[test]
fn confirmation_disabled_is_denied_and_recorded() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("history.jsonl");
    let event =
        json!({"hook_event_name":"UserPromptSubmit","prompt":"person@example.invalid"}).to_string();
    let out = invoke(
        &["--history-file", text(&path), "hook", "--agent", "codex"],
        &event,
    );
    assert!(out.status.success());
    assert_eq!(value(&out)["decision"], "block");
    let records = value(&invoke(
        &["--history-file", text(&path), "history", "list"],
        "",
    ));
    assert_eq!(records["records"][0]["decision"], "deny");
    assert_eq!(records["records"][0]["decision_source"], "unavailable");
    assert!(!records.to_string().contains("person@example.invalid"));
    let output = d.path().join("export.jsonl");
    assert!(invoke(
        &[
            "--history-file",
            text(&path),
            "history",
            "export",
            "--output",
            text(&output)
        ],
        ""
    )
    .status
    .success());
    assert_eq!(fs::read_to_string(output).unwrap().lines().count(), 1);
}

#[test]
fn ordinary_allow_does_not_write_history() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("history.jsonl");
    let event =
        json!({"hook_event_name":"UserPromptSubmit","prompt":"ordinary public task"}).to_string();
    assert!(invoke(
        &["--history-file", text(&p), "hook", "--agent", "codex"],
        &event
    )
    .status
    .success());
    assert!(!p.exists());
}

#[test]
fn generated_confirmation_configuration_has_sufficient_timeout() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("policy.yaml");
    invoke(&["init", "-o", text(&p)], "");
    let out = invoke(
        &["--policy", text(&p), "integration", "--agent", "codex"],
        "",
    );
    let config = value(&out);
    for event in ["PreToolUse", "UserPromptSubmit"] {
        let hook = &config["hooks"][event][0]["hooks"][0];
        assert_eq!(hook["timeout"], 90);
        assert!(hook["command"]
            .as_str()
            .unwrap()
            .contains("--interactive-confirmation"));
        assert!(hook["command"].as_str().unwrap().contains("--history-file"));
    }
}
