# Agent DLP Lite

Local agent DLP in one Rust executable. No Python/Node runtime, Docker, database or API key is required.

**v0.2.2 private candidate:** Codex uses a local stdio MCP hook and structured confirmation form. If confirmation is unsupported, unavailable, invalid or times out, confirm alerts warn, record and continue. Existing deny rules still block. Native desktop UI acceptance remains pending; protocol tests do not certify a client version.

[中文](README.md) · [Installation](docs/installation.md) · [Confirmation/history](docs/confirmation-history.md) · [Validation](docs/validation.md)

## Download and run

Open the successful [v0.2.2 five-platform build](https://github.com/kevinhhc/agent-dlp-lite/actions/runs/44522674609) and download your platform under Artifacts. Future builds appear in the [candidate workflow](https://github.com/kevinhhc/agent-dlp-lite/actions/workflows/build-artifacts.yml). Private repository access is required; artifacts expire after 90 days. Verify version, target and source commit in artifact-manifest.json.

Targets: Windows x64, macOS ARM64/Intel, Linux x64/ARM64 musl. Packages contain the binary, policy, installers, documentation, checksums and complete license notices. Candidates are unsigned/unnotarized; see the macOS steps in the installation guide.

~~~sh
./agent-dlp --version
./agent-dlp self-test
./agent-dlp --policy policy-pack.yaml policy validate
~~~

Windows uses .\agent-dlp.exe. Move the executable and policy to stable trusted locations before installing hooks.

## Codex setup

~~~sh
./agent-dlp --policy policy-pack.yaml integration --agent codex
./agent-dlp --policy policy-pack.yaml integration --agent codex --mcp
./agent-dlp --policy policy-pack.yaml install --agent codex --settings /absolute/path/to/hooks.json --mcp-config /absolute/path/to/config.toml
~~~

Codex normally loads these files from its user configuration directory, controlled by CODEX_HOME. Installation backs up and merges both files, preserving unrelated hooks, servers and TOML comments. An existing unrelated server named agent-dlp-lite is not overwritten. The two writes are not a cross-file transaction; inspect errors and backups if interrupted. Reload the client, trust changed hooks and verify the MCP connection.

The same binary runs as a Codex-managed MCP child process. A paired command hook independently warns and records if MCP is unavailable; keep both handlers. They may run concurrently. Normal MCP execution has no browser, listener, external API or model sampling.

## Decisions and feedback

- Priority: deny > confirm > warn > allow. Confirm also accepts the ask policy alias; it does not output an unsupported host permissionDecision=ask.
- A valid form choice allows only this operation or denies it. An optional reason is masked before storage.
- Missing forms, UI errors, invalid choices and a 60-second timeout warn, record and continue. These are not human approval.
- Decline/cancel, changed policy or inspected scripts, incomplete revalidation, and existing deny rules block. Bare client decline is not labeled human feedback because it may originate in client policy.
- History/inspection failures return a blocking decision; the host must actually honor hook results.
- Default PII/path rules use confirm; secrets and potential exfiltration use deny. Existing warn rules remain nonblocking.

Only the latest 100 pre-action alert records are retained automatically, without rolling backups. With host invocation IDs, command/MCP results update one hashed operation record; a late baseline cannot overwrite the final choice. Without IDs duplicate entries may occur, still within 100. Clean allows, scan and PostToolUse do not write history. Exports are separate user-managed copies.

Only human_feedback=true with decision_source=user identifies valid explicit form choices for future analysis. Automatic review and fallback continuation never count as user approval. Masking follows enabled regexes; unknown sensitive text can remain in a preview or reason. No automatic policy learning or external training occurs.

~~~sh
./agent-dlp history list
./agent-dlp history export --output alert-decisions.jsonl
~~~

## WorkBuddy and removal

~~~sh
./agent-dlp --policy policy-pack.yaml install --agent workbuddy --settings /absolute/path/to/settings.json
./agent-dlp uninstall --settings /absolute/path/to/hooks.json --mcp-config /absolute/path/to/config.toml
~~~

WorkBuddy uses command-only warning fallback by default. Native form support and its actual desktop settings path need separate verification. Do not assume a CodeBuddy directory is WorkBuddy's directory. Uninstall removes only owned entries and retains policy/history/exports.

The old browser confirmation is available only through the explicit hook --interactive-confirmation compatibility flag; its timeout/unavailable behavior remains deny. New generated configurations do not use it.

## Limits and development

Only host-delivered events are inspected. Prompt sync may precede hooks; post-action checks cannot undo actions. Lexical script inspection is bounded and incomplete. Host bypasses, untrusted/disabled hooks, tool coverage gaps and file changes after inspection remain limitations. [Security model](docs/privacy-security.md), [acceptance](docs/compatibility.md), [policy portability](docs/policy-portability.md).

Building requires Rust 1.100+. Python is for developer utilities only.

~~~sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
./target/release/agent-dlp self-test
~~~

CI covers Windows/macOS/Linux and dependency audit. Candidate builds produce five targets without publishing a Release or changing repository visibility. Timing data is version-specific; paired command/MCP scans are separate work. [Measured validation](docs/validation.md).

[Apache-2.0](LICENSE); [dependency licenses](THIRD_PARTY_NOTICES.md); [contributing](CONTRIBUTING.md); [security reporting](SECURITY.md).
