# 安装、接入与卸载

## 环境

最终用户只需操作系统对应的二进制，以及支持命令 Hook 的 Agent 版本。无需开发语言运行时、网络服务或管理员权限。源码构建需要 Rust 1.100+；Python 仅用于开发基准。当前交付含本机 macOS Intel 候选二进制，其他目标由手动工作流准备：Windows x64 MSVC（静态 CRT）、macOS ARM64/Intel、Linux x64/ARM64 musl。其他平台未在本机实际执行。

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
