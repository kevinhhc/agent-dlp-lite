# Agent DLP Lite

本地 Agent DLP，Rust 单二进制。通过 Hook 检查 Prompt、工具参数和有界脚本引用；无需 Python、Node.js、Docker、数据库或 API Key。

**v0.2.2 私有候选版：Codex 本地 MCP Hook + 交互表单。** 默认不打开浏览器；原生 UI 不可用时，对 confirm 告警、记录并继续。已有 deny 规则仍阻断。特定桌面版本的原生 UI 端到端验收尚待完成，不能把协议测试当作客户端认证。

[English](README_EN.md) · [安装与升级](docs/installation.md) · [确认与 100 条记录](docs/confirmation-history.md) · [验证记录](docs/validation.md)

## 预编译下载

打开[候选构建工作流](https://github.com/kevinhhc/agent-dlp-lite/actions/workflows/build-artifacts.yml)，选择标注 **v0.2.2** 且成功的运行，在 Artifacts 下载对应平台。核对包内 artifact-manifest.json 的版本、目标和源码提交。私有仓库需要登录有访问权限的账号；产物保留 90 天。

提供 Windows x64、macOS Apple Silicon/Intel、Linux x64/ARM64 musl 五种目标。包内有二进制、默认策略、安装脚本、文档和完整许可证。候选版未完成开发者签名/公证；macOS 首次运行步骤见[安装说明](docs/installation.md)。

## 快速开始

在解压目录执行（Windows 使用 .\agent-dlp.exe）：

~~~sh
./agent-dlp --version
./agent-dlp self-test
./agent-dlp --policy policy-pack.yaml policy validate
~~~

先把二进制和策略移至稳定、可信目录，再生成配置。移动文件会使已注册路径失效。

## 接入 Codex

~~~sh
./agent-dlp --policy policy-pack.yaml integration --agent codex
./agent-dlp --policy policy-pack.yaml integration --agent codex --mcp
./agent-dlp --policy policy-pack.yaml install --agent codex --settings "${CODEX_HOME:-$HOME/.codex}/hooks.json" --mcp-config "${CODEX_HOME:-$HOME/.codex}/config.toml"
~~~

install 备份并合并 hooks.json 和 config.toml，保留其他处理器与服务器；拒绝接管同名的非本工具 MCP 配置。重新加载客户端并信任更新后的 Hook，确认 agent-dlp-lite MCP 已连接。两个文件不是跨文件事务，错误与备份位置见终端输出。

每个执行前事件包含独立命令告警处理器和原生 MCP 处理器，避免服务器未连接时没有告警记录。MCP 由 Codex 启动同一二进制的 mcp 子命令，通过本地标准输入输出通信，不需要额外运行环境或外网服务。

## 告警和交互

| 动作/情形 | 行为 |
| --- | --- |
| allow | 无告警继续，保留宿主权限检查 |
| warn | 提醒、记录并继续 |
| confirm / ask | 请求选择仅允许本次或拒绝本次，可填原因 |
| 表单不支持、UI 错误、无效回复或 60 秒超时 | 告警、记录、默认继续；不算用户批准 |
| 明确拒绝、取消、策略/脚本在确认期间变化 | 阻断本次 |
| deny | 硬拒绝，不由确认覆盖 |

优先级 deny > confirm > warn > allow。默认 PII/敏感路径为 confirm，Secret 和潜在外传为 deny。旧策略的 warn 不自动改为 confirm。普通命令 Hook 返回 ask 不是受支持的原生审批方式。

历史只保留最近 **100 条**执行前告警决定，超过自动删除，不生成历史备份。human_feedback 与 decision_source 区分人工选择和自动继续；有宿主调用 ID 时命令/MCP 路径去重。仅按启用规则脱敏，未知敏感文本可能仍存在。无自动策略学习或放宽。

~~~sh
./agent-dlp history list
./agent-dlp history export --output alert-decisions.jsonl
~~~

## WorkBuddy

~~~sh
./agent-dlp --policy policy-pack.yaml install --agent workbuddy --settings /absolute/path/to/workbuddy/settings.json
~~~

替换为真实桌面配置路径。WorkBuddy 默认使用命令 Hook：confirm 告警记录后继续，deny 阻断；原生 MCP 表单未验证，不套用 Codex 的配置。不要把 CodeBuddy 产品路径直接当作 WorkBuddy 路径。

## 管理、策略与卸载

~~~sh
./agent-dlp --policy policy-pack.yaml rules list
./agent-dlp --policy policy-pack.yaml policy validate
./agent-dlp --policy policy-pack.yaml policy export --output shared-pack.yaml
./agent-dlp policy import shared-pack.yaml --output imported-pack.yaml
./agent-dlp status --settings /absolute/path/to/hooks.json
./agent-dlp uninstall --settings /absolute/path/to/hooks.json --mcp-config /absolute/path/to/config.toml
~~~

卸载只删除本工具处理器和标记的 MCP 条目，先备份；不删除策略、历史和导出文件。卸载 Hook 后再删除二进制。策略格式、脚本检查和迁移见[规则](docs/detection-rules.md)、[策略](docs/security-policies.md)、[脚本](docs/script-inspection.md)、[迁移](docs/policy-portability.md)。

## 边界与开发

只检查宿主交付的事件，不能撤销已执行操作。Prompt 可能先被宿主同步；静态检查不能覆盖所有动态代码、二进制和工具。宿主跳过 Hook、信任未通过、MCP 不可用及超时行为须实测。该工具不是操作系统网络隔离边界。[隐私](docs/privacy-security.md)、[兼容性验收](docs/compatibility.md)。

源码构建需要 Rust 1.100+；Python 仅用于开发脚本。

~~~sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
./target/release/agent-dlp self-test
python3 scripts/benchmark.py target/release/agent-dlp --samples 200
~~~

普通命令 Hook 与 MCP 会分别检查，保障服务器故障时仍有记录；不把双路径延迟说成单次扫描耗时。性能结果按版本见[验证记录](docs/validation.md)。CI 验证 Windows/macOS/Linux，候选构建生成五平台包，不自动创建 Release 或公开仓库。

许可证 [Apache-2.0](LICENSE)，依赖完整授权文本见 [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES.md)。[贡献](CONTRIBUTING.md)、[安全报告](SECURITY.md)。
