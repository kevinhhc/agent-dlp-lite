# 安装、接入与卸载

## 环境

最终用户只需操作系统对应的二进制，以及支持命令 Hook 的 Agent 版本。无需开发语言运行时、外部网络服务或管理员权限。交互确认需要默认浏览器；Linux 图形桌面需要系统提供 xdg-open。源码构建需要 Rust 1.100+；Python 仅用于开发基准。[v0.2.0 下载入口](https://github.com/kevinhhc/agent-dlp-lite/actions/runs/44503897249) 已提供五种构建成功的候选包：Windows x64 MSVC（静态 CRT）、macOS ARM64/Intel、Linux x64/ARM64 musl。下载时确认工作流版本与包内 artifact-manifest.json 一致，选择成功的 v0.2.0 构建。用户应先运行对应包的 `self-test`，再配置桌面 Hook。

Linux musl 目标减少 glibc 兼容问题。macOS 候选版没有开发者签名/公证；发行前还要验证最低操作系统版本、签名和下载隔离行为。Windows 10/11 是否可运行须在对应机器验收，不能只依据编译目标保证。

## 离线安装

macOS/Linux：下载或本机构建后，核验来自可信渠道的 SHA-256，再执行：

```sh
sh scripts/install.sh --binary /path/to/agent-dlp --sha256 EXPECTED_64_HEX_HASH --prefix "$HOME/.local/bin"
"$HOME/.local/bin/agent-dlp" self-test
```

将 `EXPECTED_64_HEX_HASH` 替换为可信清单中的值。安装脚本不下载、不修改 PATH、不注册 Hook。默认拒绝覆盖；更新用 `--force`，会保留旧二进制副本。可以直接复制单二进制，无须使用脚本。

Windows PowerShell：

```powershell
powershell -File scripts/install.ps1 -BinaryPath .\agent-dlp.exe -Sha256 EXPECTED_64_HEX_HASH -InstallDir "$env:LOCALAPPDATA\AgentDlpLite\bin"
& "$env:LOCALAPPDATA\AgentDlpLite\bin\agent-dlp.exe" self-test
```

更新使用 `-Force`。若企业限制脚本执行，按企业要求签名脚本，或手动校验后复制二进制。不要求用户改变系统执行策略。

## 策略与配置

在自己拥有的可信目录中运行 `agent-dlp init --output policy-pack.yaml`。该命令不会覆盖已有文件。策略中包含识别规则、安全动作、Source/Sink 模式和资源限制。两个 Agent 可共用此文件；不要把策略保存在可被不可信项目修改的目录。

先用 `integration --agent codex` 或 `integration --agent workbuddy` 预览，命令前加 `--policy /absolute/path/to/policy-pack.yaml`。再用 `install --agent AGENT --settings PATH` 显式指定宿主实际加载的文件。安装程序不读取账户、不自动推断 WorkBuddy 目录，也不改变系统策略。

已有文件先完整备份至同目录 `*.agent-dlp-backup-*`，然后同目录临时文件加原子替换；检测到修改竞争时拒绝写入。编辑安装配置时应关闭其他配置编辑器；这不是跨进程配置事务。所有其他字段与非本工具 Hook 尽量保持；JSON 格式缩进可能重排。严格 JSON 之外的输入拒绝修改。

接入成功需要三个证据：配置文件已写入、宿主实际加载该配置、合成风险事件确实阻断。[验收过程](compatibility.md) 缺一不可。`status` 只报告第一项，且不会检查二进制是否已经被移动。

## 卸载与恢复

```sh
agent-dlp uninstall --settings /absolute/path/to/settings.json
```

卸载仅移除命令末尾带本工具管理标记的处理器，保留用户后续增加的其他配置，并在修改前再次备份。不要手工给其他工具添加相同管理标记。验证配置后删除二进制，按需保留或删除策略、审计与备份。

需要恢复时，先保存当前配置，人工比较备份并恢复必要内容；直接覆盖旧备份可能丢失后续修改。备份没有自动保留期，需要使用者清理。

## 从 v0.1 升级至 v0.2

1. 下载 v0.2.0 对应平台包并校验摘要；替换已安装二进制前保留旧副本，再执行 self-test。
2. 保留并审核自己的策略。旧 warn 仍提醒后继续；需要人工确认的规则改为 action: confirm，或采用新包默认策略中的确认规则。init 不覆盖旧文件，安装也不自动替换策略。
3. 使用新二进制、策略绝对路径重新 install 到原宿主实际配置文件，生成 --interactive-confirmation、固定 --history-file 和 90 秒 timeout。旧 5 秒 Hook 遇到 confirm 会直接拒绝并提示重新安装。
4. 重新加载宿主，用合成样例分别验证允许、拒绝、超时和历史记录；详见[确认与记录](confirmation-history.md)。

历史默认启用，只保留最近 100 条告警决定；旧版 --audit 路径仍可使用，但不会自动合并到默认路径。导出的副本和配置备份需要用户自行管理。卸载 Hook 不删除这些数据。
