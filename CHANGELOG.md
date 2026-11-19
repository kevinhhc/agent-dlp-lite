# Changelog

## 0.2.2 — private candidate

- Native Codex confirmation through a local stdio MCP hook and a nonempty decision/reason form.
- Same Rust executable serves MCP; no external runtime or browser is required by the default integration.
- Independent command guard records warnings if MCP is unavailable; confirm falls back to warning continuation on unsupported UI, protocol errors or timeout. Explicit denial/cancellation and existing deny rules block.
- Bounded latest-100 history deduplicates companion/native outcomes when host invocation IDs are present; human feedback is distinguished from automatic continuation.
- Explicit --mcp-config installation merges and backs up Codex TOML; uninstall optionally removes the owned MCP entry.
- WorkBuddy retains command-hook warning fallback; its native form support is unverified.

## 0.2.0 — private candidate

- Per-operation local browser confirmation with timeout/unavailable/changed-content denial.
- Default PII/path confirmation; legacy warn remains non-blocking.
- Latest 100 masked alert decisions, concurrency-safe replacement, no rolling backups.
- Decision source, reason, policy fingerprint, history listing/export for future policy review.
- Reinstall hooks on upgrade: 90-second host timeout and explicit interactive flag.


## 0.1.0 — local candidate

- Offline Rust engine and CLI, shared Codex/WorkBuddy hook protocol subset.
- Secret/PII/path/custom regex rules and portable versioned YAML packs.
- Bounded lexical script/dependency inspection with conservative source/sink detection.
- Optional redacted audit, configuration previews, backed-up merging and targeted uninstall.
- Automated tests, offline installers, cross-platform CI and manual candidate artifact builds.
- Chinese/English documentation and Apache-2.0 license.

Desktop end-to-end interception, other-platform runtime checks, signing and GitHub execution remain release acceptance work. No public repository or release has been created.
