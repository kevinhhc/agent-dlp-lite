# v0.2.2 安装、升级与卸载

## 环境与下载

用户只需对应操作系统的预编译二进制。默认 Codex 接入需要支持 MCP tool hooks 和 form elicitation 的客户端版本；无原生表单能力时 confirm 告警记录后继续。无需安装 Rust、Python、Node.js 或外部服务。MCP 是由客户端管理的本地子进程，不是系统后台服务。

[候选构建工作流](https://github.com/kevinhhc/agent-dlp-lite/actions/workflows/build-artifacts.yml) 提供 Windows x64 MSVC、macOS ARM64/Intel、Linux x64/ARM64 musl。选择成功的 v0.2.2 运行；私有仓库需要账号权限，产物保留 90 天。核对 manifest 中的版本、目标、源码提交以及可信清单的 SHA-256。

macOS 首次下载的未签名候选包可能被 Gatekeeper 阻止。确认来自本仓库并验证校验值后，在系统设置 → 隐私与安全性中找到 agent-dlp 的拦截提示，选择仍要打开，再运行 self-test。此步骤仅批准该文件；候选版没有 Developer ID 签名/Apple 公证。[Apple 说明](https://support.apple.com/en-us/102445)。

~~~sh
shasum -a 256 -c SHA256SUMS
./agent-dlp --version
./agent-dlp self-test
~~~

Linux 可使用 sha256sum -c SHA256SUMS。各系统最低版本和最终用户机器仍需验收；musl 构建不能代替操作系统兼容性测试。

## 放到稳定目录

macOS/Linux 安装脚本仅复制与验证，不下载、不改 PATH、不自动注册 Hook。将占位值替换为来自可信清单的校验值：

~~~sh
sh scripts/install.sh --binary /path/to/agent-dlp --sha256 EXPECTED_64_HEX_HASH --prefix "$HOME/.local/bin"
~~~

默认拒绝覆盖；更新使用 --force 并保留旧二进制副本。也可手动校验后复制。Windows PowerShell 对应：

~~~powershell
powershell -File scripts/install.ps1 -BinaryPath .\agent-dlp.exe -Sha256 EXPECTED_64_HEX_HASH -InstallDir "$env:LOCALAPPDATA\AgentDlpLite\bin"
~~~

不要求改变企业脚本执行策略。候选版未签名，按企业要求处理。

## Codex 原生 MCP 接入

将二进制和策略放到可信固定路径。以下从解压目录调用；实际安装后使用固定二进制完整路径：

~~~sh
./agent-dlp --policy policy-pack.yaml policy validate
./agent-dlp --policy policy-pack.yaml integration --agent codex
./agent-dlp --policy policy-pack.yaml integration --agent codex --mcp
./agent-dlp --policy policy-pack.yaml install --agent codex --settings "${CODEX_HOME:-$HOME/.codex}/hooks.json" --mcp-config "${CODEX_HOME:-$HOME/.codex}/config.toml"
~~~

Windows 使用 .\agent-dlp.exe，显式传入用户 Codex 目录中的 hooks.json 和 config.toml。

安装生成绝对二进制、策略和历史路径。命令 Hook 与 MCP Hook 都要保留：前者独立告警记录和执行 deny，后者提供原生交互。MCP 服务启动命令是同一二进制的 mcp 子命令。已有两份配置先备份，JSON/TOML 无效或同名非本工具服务器则拒绝接管。保留其他配置、TOML 注释和其他 Hook；JSON 格式可能重排。

两份文件先校验再写入，但不是跨文件事务。中途失败时可能只完成 MCP 配置写入；终端显示错误及备份位置，应检查两份文件再重试。备份可能含原配置敏感内容，无自动保留期，由用户清理。

重新加载/重启客户端，信任修改后的 Hook，确认 agent-dlp-lite MCP 已连接。status 只统计注册项，不检查 MCP 实际就绪和 UI 支持。通过[桌面验收](compatibility.md)验证操作确实等到用户选择后才执行。

## WorkBuddy

~~~sh
./agent-dlp --policy policy-pack.yaml install --agent workbuddy --settings /absolute/path/to/workbuddy/settings.json
~~~

指定实际桌面版本加载的严格 JSON 文件，不猜测配置目录。默认命令 Hook 中 confirm 告警记录后继续、deny 阻断；WorkBuddy 原生表单未验证，不自动写入 Codex MCP 配置。

## 从 v0.2.0 升级

1. 下载/校验 v0.2.2；先替换固定路径的二进制，再 self-test。
2. 保留自定义策略。旧 warn 不自动改成 confirm；需要交互的规则应使用 confirm。init 不覆盖旧策略，安装也不更改策略。
3. Codex 重新 install，并新增 --mcp-config 参数；这会替换本工具旧浏览器 Hook。若只换二进制却保留旧 --interactive-confirmation 参数，仍运行浏览器兼容模式，其超时/不可用仍拒绝。
4. 重新加载并信任 Hook，测试原生允许、拒绝、取消、无表单和超时。原生默认超时 60 秒；不可用/无效/超时告警继续，显式拒绝/取消仍阻断。历史保留 100 条，human_feedback 区分人工选择与默认继续。

## 卸载

~~~sh
./agent-dlp uninstall --settings /absolute/path/to/hooks.json --mcp-config /absolute/path/to/config.toml
~~~

只移除本工具管理标记的命令、保留给本工具的 MCP inspect 处理器和标记的 MCP 服务器条目，保留其他设置。WorkBuddy 不传 --mcp-config。若需卸载 Codex MCP 服务，必须传入与安装相同的配置路径；然后重启客户端，再删除二进制。策略、历史、导出及旧备份不自动删除。
