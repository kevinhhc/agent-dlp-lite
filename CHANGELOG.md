# Changelog

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
