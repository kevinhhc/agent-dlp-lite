# Agent DLP Lite

An offline Rust executable for local agent DLP hooks. No daemon, telemetry, API key, database, or Python/Node runtime is required to run the binary. The detector makes no external network requests. Confirmation temporarily serves a loopback-only page in the default browser.

**0.2.0 precompiled candidates are available for private testing.** Codex and Tencent WorkBuddy share one engine and portable YAML policy. Automated protocol tests are available; actual desktop interception has not been verified on either product. See [compatibility](docs/compatibility.md) and [validation](docs/validation.md). [中文说明](README.md).

## Features

- Secret patterns, email and mainland China mobile-number patterns, sensitive-path clues, custom regular expressions.
- Allow / Warn / Confirm / Deny, rule switches and event scopes, portable policy import/export.
- Bounded inspection of common Shell, Python and JavaScript invocations, inline code and simple local dependencies.
- Conservative source/sink co-occurrence detection of **potential** exfiltration; not full taint analysis.
- Bounded redacted JSONL alert history; backup and non-destructive hook configuration merging.

## New in v0.2.0

`action: confirm` opens a short-lived local browser page for a single allow/deny decision. Timeout, unavailable UI, changed policy or changed inspected scripts deny the operation. Reinstall hooks after upgrading to enable confirmation with a 90-second host timeout. Existing warn policies remain non-blocking. The new default pack requests confirmation for PII and sensitive paths.

Only the latest 100 pre-action alert decisions are retained automatically, including masked context, source, reason, detector version and policy fingerprint. `history list` and `history export --output FILE` support future policy review. No automatic policy modification or external training is performed. See the [confirmation/history guide](docs/confirmation-history.md).

## Download native candidates

Open the successful [v0.2.0 five-platform download run](https://github.com/kevinhhc/agent-dlp-lite/actions/runs/44503897249) and choose your platform under **Artifacts** near the bottom of the page. All five targets built successfully. Future builds appear in the [candidate workflow](https://github.com/kevinhhc/agent-dlp-lite/actions/workflows/build-artifacts.yml). Verify the version and target in artifact-manifest.json. The private repository requires a signed-in account with repository access. Artifacts are retained for 90 days.

Targets: Windows x64, macOS ARM64/Intel, Linux x64/ARM64 musl. Extract the download and its platform archive, read `START_HERE.txt`, and run the included self-test. Packages include the native executable, default policy, installers, documentation and SHA-256 checksums. No language runtime is required. Candidates are unsigned; workflow results establish the actual availability of each target.

## User quick start

Download and extract your platform package, then open a terminal in the extracted `agent-dlp-lite` directory. The executable and `policy-pack.yaml` are included. No Rust, Python or Node.js installation is required.

macOS / Linux:

```sh
./agent-dlp self-test
./agent-dlp --policy policy-pack.yaml policy validate
```

Windows PowerShell:

```powershell
.\agent-dlp.exe self-test
.\agent-dlp.exe --policy policy-pack.yaml policy validate
```

Both commands should exit successfully. Read `START_HERE.txt` and the [installation guide](docs/installation.md), put the executable in a permanent location and add its directory to PATH before configuring hooks below. Use its full path when it is not on PATH.

## Configure hooks

Keep the binary in a permanent location and copy the supplied policy-pack.yaml into a trusted directory. Preview:

```sh
agent-dlp --policy policy-pack.yaml integration --agent codex
agent-dlp --policy policy-pack.yaml integration --agent workbuddy
```

After verifying that the specific desktop version supports the protocol and identifying its settings path:

```sh
agent-dlp --policy policy-pack.yaml install --agent codex --settings /absolute/path/to/hooks.json
agent-dlp --policy policy-pack.yaml install --agent workbuddy --settings /absolute/path/to/settings.json
agent-dlp status --settings /absolute/path/to/settings.json
agent-dlp uninstall --settings /absolute/path/to/settings.json
```

Replace paths. Codex's documented user location is `$CODEX_HOME/hooks.json`, normally `~/.codex/hooks.json`. WorkBuddy desktop settings discovery is pending verification; do not assume CodeBuddy's directory is its location. Installation explicitly requires a settings path, preserves unrelated settings/hooks and backs up existing files. Strict JSON is required; JSONC is unsupported. Restart/reload and perform the [desktop acceptance procedure](docs/compatibility.md). Registration count alone does not prove active protection. Unregister hooks before deleting the binary.

## Policies

Edit `rules` in the generated YAML. Add a custom rule with `id`, `kind: custom`, `pattern`, `action`, optional `enabled`, and optional `events`. See [example](examples/custom-rules.yaml), [rule reference](docs/detection-rules.md) and [policy reference](docs/security-policies.md).

Decisions combine as **deny > confirm > warn > allow**, independent of order. An allow match cannot override another rule's deny. No explicit host permission approval is emitted on clean input. Invalid policies fail rather than falling back to defaults. `--policy` is explicit; a project cannot silently replace the pack through configuration discovery.

```sh
agent-dlp --policy policy-pack.yaml rules list
agent-dlp --policy policy-pack.yaml policy export --output shared.yaml
agent-dlp policy import shared.yaml --output imported.yaml
```

Both adapters can point to the same pack. Unknown fields, unsupported schema versions, duplicate identifiers and invalid regexes are rejected.

## Limits, privacy and verification

Standard detection reports contain rule IDs and decisions without matched text. Decision history also stores a masked, truncated preview and reason; unknown sensitive data may remain because masking follows enabled regex rules. Pre-action alert history is enabled by default and retains at most 100 records; ordinary clean allows and post-action observations are not recorded. `--history-file PATH` selects its location; legacy `hook --audit PATH` remains an alias for a bounded journal. New Unix policy/audit/settings files use mode 0600. Existing permissions and Windows ACLs remain the operator's responsibility. Configuration backups may contain the original settings' sensitive values.

Only host-delivered events are observable. Prompt hooks may run after host synchronization or uploads. PostToolUse is observation only and cannot undo an action. Static analysis is lexical and incomplete: dynamic code, obfuscation, third-party packages, compiled binaries, unrecognized tool names and time-of-check/time-of-use races remain limitations. Recognized unreadable, out-of-root or oversized scripts produce an incomplete finding and deny by default. Hosts may ignore failed/timed-out hooks. This is an application guardrail, not an OS enforcement boundary. See [script inspection](docs/script-inspection.md) and [privacy/security](docs/privacy-security.md).

Latency targets include fresh process startup and policy compilation: ordinary hooks P50 ≤10 ms, P95 ≤30 ms, P99 ≤50 ms; small scripts P95 ≤100 ms; RSS ≤30 MB. See [measured local results](docs/validation.md); targets are not cross-platform guarantees. There is no daemon or persistent scan cache; confirmation temporarily serves a loopback-only browser page.

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
python3 scripts/benchmark.py target/release/agent-dlp --samples 200
```

CI definitions cover Windows/macOS/Linux tests and dependency auditing. A candidate workflow, triggered manually or by packaging updates, prepares five binary targets and checksums, with read-only repository permissions and no release publication. See [GitHub Actions](https://github.com/kevinhhc/agent-dlp-lite/actions) for actual remote run results. Python is for developer benchmarks only.

## Developers: build from source

Contributing or compiling from source requires Rust 1.100+. Run these commands in the source checkout:

```sh
cargo build --release --locked
./target/release/agent-dlp self-test
./target/release/agent-dlp init --output policy-pack.yaml
./target/release/agent-dlp --policy policy-pack.yaml policy validate
./target/release/agent-dlp scan --file tests/fixtures/clean.json
./target/release/agent-dlp policy test --file tests/fixtures/upload.json
```

The final command intentionally exits **2** with a deny report. It does not execute the command in the JSON, read a real `.env`, or contact the example URL. Windows uses `target\release\agent-dlp.exe`.

Install a local binary with `scripts/install.sh --binary PATH --sha256 EXPECTED_HASH --prefix DIRECTORY`, or `scripts/install.ps1 -BinaryPath PATH -Sha256 EXPECTED_HASH -InstallDir DIRECTORY` on Windows. No remote installer or runtime download is used. Obtain hashes from a trusted release manifest when releases become available. See [installation](docs/installation.md).

Licensed under [Apache-2.0](LICENSE). See [contributing](CONTRIBUTING.md) and [security reporting](SECURITY.md).
