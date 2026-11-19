# Agent DLP Lite

纯本地、无后台服务的 Agent DLP。Rust 单二进制，通过 Hook 检查 Prompt、工具参数和常见脚本，在宿主支持的执行前事件上返回阻断决策。运行不需要 Rust、Python、Node.js、Docker、数据库或 API Key，不向外网发起请求，无遥测；确认时临时使用本机回环页面。

**状态：0.2.0 预编译候选版已提供下载，当前用于私有测试。** Codex 与腾讯 WorkBuddy 使用相同引擎和 Policy Pack。自动化协议测试通过不代表某个桌面版本已实际执行 Hook；两个桌面端的端到端验收仍须完成。详细状态见 [兼容性](docs/compatibility.md) 和 [验证记录](docs/validation.md)。

[English](README_EN.md) · [安装](docs/installation.md) · [识别规则](docs/detection-rules.md) · [策略](docs/security-policies.md) · [脚本检查](docs/script-inspection.md) · [隐私与边界](docs/privacy-security.md)

## 功能

- Secret：私钥头、AWS Access Key、GitHub Token、常见密钥赋值模式。
- PII：邮箱、中国大陆手机号码格式；默认请求确认，不声称识别所有个人信息。
- 敏感文件：`.env`、凭据文件、私钥路径等文本线索；默认请求确认。
- 自定义正则、事件作用域、规则开关、Allow / Warn / Confirm / Deny。
- Shell、Python、JavaScript 的常见文件调用、内联代码、本地引用；以有界 Source/Sink 线索检测潜在外传。不是完整污点分析。
- YAML 策略包校验、测试、导出、导入，两种适配器共用决策。
- 默认保留最近 100 条脱敏告警决定；安装前备份，卸载只移除本工具标记的 Hook。

## v0.2.0 新增

- `action: confirm`：通过临时本地确认页，由用户决定仅放行本次或拒绝；超时/不可用拒绝。
- 自动保留最近 100 条告警决定，记录脱敏上下文、选择及原因，超过后删除最旧记录。
- `history list` / `history export`：为后续策略调整准备数据，不自动修改规则。
- 升级后须重新安装 Hook（90 秒超时）；旧策略的 warn 仍仅提醒。详见[确认与记录指南](docs/confirmation-history.md)。

## 预编译候选版下载

打开 [v0.2.0 五平台下载](https://github.com/kevinhhc/agent-dlp-lite/actions/runs/44503897249)，在页面底部的 **Artifacts** 区域下载对应平台；五个目标均已构建成功。后续版本见[候选构建工作流](https://github.com/kevinhhc/agent-dlp-lite/actions/workflows/build-artifacts.yml)。核对包内 artifact-manifest.json 的版本与目标。当前仓库为私有，需要登录有仓库访问权限的 GitHub 账号；构建产物保留 90 天。

提供 Windows x64、macOS Apple Silicon/Intel、Linux x64/ARM64（musl）五种目标。解压下载文件及其中的平台压缩包，先阅读 `START_HERE.txt`，再执行自测。包内有单二进制、默认策略、安装脚本、文档和 SHA-256 校验值，运行无需安装开发环境。候选文件未签名/公证；各平台实际构建状态以工作流结果为准。

## 用户快速开始

下载并解压对应平台的文件，在解压后的 `agent-dlp-lite` 目录打开终端。包内已包含可执行文件和 `policy-pack.yaml`，无需安装 Rust、Python 或 Node.js。

macOS / Linux：

```sh
./agent-dlp self-test
./agent-dlp --policy policy-pack.yaml policy validate
```

Windows PowerShell：

```powershell
.\agent-dlp.exe self-test
.\agent-dlp.exe --policy policy-pack.yaml policy validate
```

两条命令均应成功退出。接着阅读包内 `START_HERE.txt` 和 [安装说明](docs/installation.md)，把可执行文件放到稳定目录并加入 PATH，再按下节配置 Hook。未加入 PATH 时，使用可执行文件的完整路径调用。

## 接入 Codex 与 WorkBuddy

先把二进制放到稳定位置；移动或删除二进制会使 Hook 失效。将包内的 `policy-pack.yaml` 保存到可信目录，生成配置预览：

```sh
agent-dlp --policy policy-pack.yaml integration --agent codex
agent-dlp --policy policy-pack.yaml integration --agent workbuddy
```

输出 JSON 中使用绝对二进制、策略及历史路径，启用交互确认并设为 90 秒超时。默认注册 `PreToolUse` 和 `UserPromptSubmit`，不注册事后阻断。

仅在确认目标桌面版本的配置位置和 Hook 支持后合并配置：

```sh
# Codex 用户级位置；CODEX_HOME 自定义时使用对应目录。
agent-dlp --policy policy-pack.yaml install --agent codex --settings "$HOME/.codex/hooks.json"

# WorkBuddy：明确传入该桌面版本实际加载的 settings.json。
agent-dlp --policy policy-pack.yaml install --agent workbuddy --settings /absolute/path/to/workbuddy/settings.json
```

第二条的路径需要替换。WorkBuddy/CodeBuddy 品牌下不同产品的配置位置可能不同；不能直接把 CodeBuddy 的 `~/.codebuddy/settings.json` 当作 WorkBuddy 桌面路径。程序不会猜测或自动修改这些目录。配置合并保留其他字段和其他 Hook，已有文件先备份；不支持 JSONC。重启/重新加载 Agent 后，按 [桌面验收步骤](docs/compatibility.md) 验证，注册数量不代表保护已生效。

```sh
agent-dlp status --settings /absolute/path/to/settings.json
agent-dlp uninstall --settings /absolute/path/to/settings.json
```

先卸载 Hook，再删除二进制。备份可能包含原有配置中的敏感信息，按需要自行保管或清理。

## 修改规则与安全策略

编辑 `policy-pack.yaml` 中的 `rules`。例如追加：

```yaml
  - id: COMPANY-INTERNAL
    kind: custom
    pattern: '\bINTERNAL-[0-9]{6}\b'
    action: deny
    enabled: true
    events: [UserPromptSubmit, PreToolUse]
```

`events` 为空/省略表示适用于全部已支持事件。规则按 **deny > confirm > warn > allow** 合并，顺序不影响结果。`allow` 不能覆盖其他规则的 `deny`；要处理误报，修改命中规则的模式、范围或开关。程序不会向 Agent 返回显式 `permissionDecision: allow`，避免改变宿主原有权限检查。

```sh
agent-dlp --policy policy-pack.yaml rules list
agent-dlp --policy policy-pack.yaml policy validate
agent-dlp --policy policy-pack.yaml policy export --output shared-pack.yaml
agent-dlp policy import shared-pack.yaml --output imported-pack.yaml
```

两个 Agent 指向同一策略文件即可迁移。没有外部 include、远程规则、自动目录发现或脚本执行型规则。详情见 [规则](docs/detection-rules.md)、[策略](docs/security-policies.md)、[迁移](docs/policy-portability.md)。

## 安全边界与性能

Hook 只能处理宿主交付的事件。Prompt Hook 未必早于宿主的上传、同步或遥测；PostToolUse 检查不能撤销已经执行的操作。静态检查可漏掉动态生成代码、混淆、第三方包、二进制程序或其他工具中的命令；检查和执行之间也存在文件被修改的时间窗口。它是应用层防护，应与宿主权限、操作系统隔离及企业 DLP 配合。

已识别脚本读取失败、越出工作目录、超限或部分动态模式会产生 `INSPECTION-INCOMPLETE`，默认拒绝。输入/配置错误在 Hook 模式退出 2，是否阻断取决于宿主。宿主超时、禁用或绕过 Hook 时，本程序无法强制执行安全策略。不要将其描述为绝对不可绕过的系统边界。

性能目标：普通 Hook P50 ≤10 ms、P95 ≤30 ms、P99 ≤50 ms；小脚本 P95 ≤100 ms；RSS ≤30 MB。目标不等于所有平台保证。实际本机数据见 [验证记录](docs/validation.md)。确认时短暂启动本机回环服务；无常驻进程，无持久扫描缓存，每次重新读取脚本，避免旧缓存复用。

## 开发者：从源码构建

参与开发或自行编译时，需要 Rust 1.100+。以下命令在源码目录中执行：

```sh
cargo build --release --locked
./target/release/agent-dlp self-test
./target/release/agent-dlp init --output policy-pack.yaml
./target/release/agent-dlp --policy policy-pack.yaml rules validate
./target/release/agent-dlp --policy policy-pack.yaml scan --file tests/fixtures/clean.json
./target/release/agent-dlp --policy policy-pack.yaml policy test --file tests/fixtures/upload.json
```

最后一条应输出 `action: deny`，退出码为 **2**；只扫描 JSON，不执行其中命令，不读取真实 `.env` 内容，也不连接示例网址。测试数据全部为合成数据。Windows 使用 `target\release\agent-dlp.exe`。

安装本地二进制到指定目录：

```sh
sh scripts/install.sh --binary target/release/agent-dlp \
  --sha256 "$(shasum -a 256 target/release/agent-dlp | cut -d ' ' -f 1)" \
  --prefix "$PWD/local/bin"
./local/bin/agent-dlp self-test
```

这里用本机编译结果的校验值验证复制流程。未来下载发行文件时，应使用来自可信发行渠道的 `SHA256SUMS`，不能把下载文件自算的哈希当作来源证明。Windows 离线安装方式见 [安装说明](docs/installation.md)。

## 开发和质量检查

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
python3 scripts/benchmark.py target/release/agent-dlp --samples 200
```

Python 只用于开发性能测试，不是运行依赖。CI 包含 Windows、macOS、Linux 的质量检查和依赖审计；候选构建工作流生成五种目标的制品及校验值，可手动或在打包脚本更新时触发，不创建 Release、不公开发布。CI 的实际运行结果见 [GitHub Actions](https://github.com/kevinhhc/agent-dlp-lite/actions)。

许可证：[Apache-2.0](LICENSE)。贡献方式见 [CONTRIBUTING](CONTRIBUTING.md)，安全问题见 [SECURITY](SECURITY.md)。
