//! One ephemeral loopback page per operation; GET, timeout and errors never approve.
use crate::{event::Event, policy::CompiledPack, scripts};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
#[derive(Debug, Clone)]
pub struct Outcome {
    pub allowed: bool,
    pub source: &'static str,
    pub reason: String,
}
fn outcome(allowed: bool, source: &'static str, reason: &str) -> Outcome {
    Outcome {
        allowed,
        source,
        reason: reason.into(),
    }
}
pub fn fingerprint(event: &Event, policy: &CompiledPack) -> Result<Vec<u8>, String> {
    let mut hash = Sha256::new();
    for part in [
        serde_json::to_string(event).map_err(|_| "cannot hash event")?,
        policy.export()?,
    ] {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    if let Some(command) = event.command() {
        let inspection =
            scripts::inspect(&command, event.script_cwd().as_deref(), &policy.pack.limits);
        if inspection.incomplete {
            return Err("script inspection incomplete".into());
        }
        for text in inspection.texts {
            hash.update((text.len() as u64).to_le_bytes());
            hash.update(text.as_bytes());
        }
    }
    Ok(hash.finalize().to_vec())
}
/// Validate the same event, current policy and inspected script closure around the user's choice.
pub fn decide(
    event: &Event,
    policy: &CompiledPack,
    policy_path: Option<&std::path::Path>,
    summary: &str,
    interactive: bool,
    request_impl: impl FnOnce(&str, &str, Duration) -> Outcome,
) -> Outcome {
    if !interactive {
        return outcome(
            false,
            "unavailable",
            "confirmation disabled; reinstall v0.2 hooks with a 90-second timeout",
        );
    }
    let Ok(before) = fingerprint(event, policy) else {
        return outcome(
            false,
            "incomplete",
            "cannot verify script contents for confirmation",
        );
    };
    let mut answer = request_impl(
        summary,
        &crate::history::redacted(&event.texts().join("\n"), policy, 400),
        Duration::from_secs(60),
    );
    if answer.allowed
        && !CompiledPack::load(policy_path)
            .and_then(|p| fingerprint(event, &p))
            .is_ok_and(|after| after == before)
    {
        answer = outcome(
            false,
            "changed",
            "policy or inspected script changed during confirmation",
        );
    }
    answer
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn launch(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(windows)]
    let mut command = {
        let mut c = Command::new("rundll32.exe");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    let mut child = command
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "cannot open browser")?;
    thread::sleep(Duration::from_millis(50));
    match child.try_wait() {
        Ok(Some(status)) if !status.success() => Err("browser opener failed".into()),
        Ok(Some(_)) => Ok(()),
        Ok(None) => {
            thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        Err(_) => Err("cannot inspect browser opener".into()),
    }
}
pub fn request(summary: &str, preview: &str, timeout: Duration) -> Outcome {
    serve_with(summary, preview, timeout, launch)
}
/// Injected opener supports protocol tests without opening the user's real browser.
pub fn serve_with(
    summary: &str,
    preview: &str,
    timeout: Duration,
    opener: impl FnOnce(&str) -> Result<(), String>,
) -> Outcome {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(_) => return outcome(false, "unavailable", "cannot start local confirmation"),
    };
    if listener.set_nonblocking(true).is_err() {
        return outcome(
            false,
            "unavailable",
            "cannot configure confirmation listener",
        );
    }
    let mut random = [0u8; 32];
    if getrandom::fill(&mut random).is_err() {
        return outcome(false, "unavailable", "cannot create confirmation token");
    }
    let token = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let address = listener.local_addr().expect("bound address").to_string();
    let route = format!("/review/{token}");
    let url = format!("http://{address}{route}");
    let html = format!(
        r#"<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>Agent DLP Lite — 确认操作</title><style>body{{font:18px system-ui;max-width:760px;margin:48px auto;padding:24px}}pre{{white-space:pre-wrap;background:#f1f4f8;padding:20px}}button{{padding:12px 24px;margin-right:16px}}textarea{{display:block;width:95%;height:70px;margin:18px 0}}</style><h1>检测到敏感内容</h1><p>{}</p><pre>{}</pre><p>是否允许本次操作？本次选择不会修改安全策略，也不会批准 Agent 的其他权限。未确认将在 {} 秒后拒绝。</p><form method="post" action="{}"><input type="hidden" name="token" value="{}"><label>原因（可选，用于后续策略调整）<textarea name="reason" maxlength="200"></textarea></label><button name="choice" value="deny" autofocus>拒绝本次</button><button name="choice" value="allow">仅允许本次</button></form></html>"#,
        escape(summary),
        escape(preview),
        timeout.as_secs(),
        route,
        token
    );
    let deadline = Instant::now() + timeout;
    if opener(&url).is_err() {
        return outcome(false, "unavailable", "confirmation page unavailable");
    }
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let remaining = deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(250));
                if remaining.is_zero() {
                    break;
                }
                let _ = stream.set_read_timeout(Some(remaining));
                let _ = stream.set_write_timeout(Some(remaining));
                if let Some(answer) = handle(&mut stream, &address, &route, &token, &html) {
                    return answer;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(_) => return outcome(false, "unavailable", "confirmation listener failed"),
        }
    }
    outcome(false, "timeout", "confirmation expired or window closed")
}
fn reply(stream: &mut TcpStream, status: &str, body: &str) {
    let _=write!(stream,"HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Frame-Options: DENY\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; frame-ancestors 'none'\r\nConnection: close\r\n\r\n{body}",body.len());
}
fn decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' => {
                let h = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
                out.push(u8::from_str_radix(h, 16).ok()?);
                i += 2;
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8(out).ok()
}
fn handle(
    stream: &mut TcpStream,
    address: &str,
    route: &str,
    token: &str,
    html: &str,
) -> Option<Outcome> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 1024];
    let mut split = None;
    while bytes.len() < 8192 {
        let n = stream.read(&mut buffer).ok()?;
        if n == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..n]);
        if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            split = Some(i);
            break;
        }
    }
    let i = split?;
    let header = std::str::from_utf8(&bytes[..i]).ok()?;
    let mut lines = header.lines();
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?;
    let path = first.next()?;
    let headers: Vec<_> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let get = |key: &str| {
        headers
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    if path != route
        || get("host") != Some(address)
        || get("origin").is_some_and(|o| o != format!("http://{address}"))
    {
        reply(stream, "403 Forbidden", "Invalid confirmation request");
        return None;
    }
    if method == "GET" {
        reply(stream, "200 OK", html);
        return None;
    }
    if method != "POST"
        || get("transfer-encoding").is_some()
        || get("content-type").is_none_or(|v| !v.starts_with("application/x-www-form-urlencoded"))
    {
        reply(stream, "400 Bad Request", "Invalid form");
        return None;
    }
    let length = get("content-length")?.parse::<usize>().ok()?;
    if length > 2048 {
        return None;
    }
    while bytes.len() < i + 4 + length {
        let n = stream.read(&mut buffer).ok()?;
        if n == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..n]);
        if bytes.len() > 8192 {
            return None;
        }
    }
    let body = std::str::from_utf8(&bytes[i + 4..i + 4 + length]).ok()?;
    let fields: Option<Vec<_>> = body
        .split('&')
        .map(|f| {
            let (k, v) = f.split_once('=')?;
            Some((decode(k)?, decode(v)?))
        })
        .collect();
    let fields = fields?;
    let field = |key: &str| {
        fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    if field("token") != Some(token)
        || fields
            .iter()
            .filter(|(k, _)| k == "choice" || k == "token")
            .count()
            != 2
    {
        reply(stream, "403 Forbidden", "Invalid token");
        return None;
    }
    let allowed = match field("choice") {
        Some("allow") => true,
        Some("deny") => false,
        _ => {
            reply(stream, "400 Bad Request", "Choose allow or deny");
            return None;
        }
    };
    let reason = field("reason")
        .unwrap_or("")
        .chars()
        .take(200)
        .collect::<String>();
    reply(stream,"200 OK","<meta charset=\"utf-8\"><h1>选择已提交</h1><p>可以关闭此页面并返回 Agent。若策略或文件已改变，本次操作仍将拒绝。</p>");
    Some(outcome(allowed, "user", &reason))
}
