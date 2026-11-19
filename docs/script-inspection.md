# 轻量脚本静态检查

P0 使用命令分词、文件解析和正则线索，刻意限定为有界静态检查。没有 Tree-sitter、完整 AST、执行模拟或通用污点分析。不声称证明脚本一定会外传，亦不因没有命中而证明安全。

## 已实现

- 常见工具名：Bash、shell、shell_command、exec_command、execute_command、run_command、terminal（匹配时忽略大小写）。
- 命令字段 `command` 或 `cmd`，字符串或字符串数组。Codex 的工具级 workdir/cwd 优先于事件 cwd。
- `python/python3/python3.x`、`node/nodejs`、`bash/sh/zsh` 文件调用；常见 `.py/.js/.mjs/.cjs/.sh/.bash` 参数。
- `-c`、`-e`、`--eval` 内联代码；内联 shell 中简单脚本文件线索。
- 简单 Python `import helper` / `from helper import x`、包 `__init__.py`，JavaScript 相对 require/import，Shell source；循环引用去重。
- 当前工作目录内的脚本。规范化路径后越出根目录的引用拒绝检查，防止任意跨目录读取。脚本本身不会执行。

例如脚本读取 `.env` 且使用 `requests.post`，会产生 `FLOW-SENSITIVE-NETWORK`。Source 和 Sink 可以在同一已检查引用闭包内的不同文件出现；结果标为 **potential**，因为它们可能处于互斥分支、无关函数或注释中。本版不提供 confirmed 标签。

`flows` 中每项包含唯一 `id`、`source` 正则、`sink` 正则、`action`、可选 `enabled`。两个模式在已检查脚本集合（含原始命令）分别命中即触发。可以按组织场景改模式，导出后两种 Agent 保持相同含义。

## 检查不完整

已识别的脚本不存在、不可读、不是普通文件、非 UTF-8、过大、超过总量/深度/数量限制、越出工作目录，会报告 `INSPECTION-INCOMPLETE`。部分 `eval/exec/__import__/importlib/base64` 模式、模块执行 `-m`、标准输入脚本、here-doc、进程替换、改变目录后执行脚本等也报告不完整。默认拒绝，可用策略改为 warn。

## 仍有盲区

命令分词不是完整 Shell/PowerShell/cmd 解析器。复杂引用、变量拼接、命令别名、解释器参数组合、Windows 未引用反斜线路径、嵌套 eval、Python 相对/动态导入、JS 动态路径和包解析、子进程调用、运行时生成代码、编码/加密、第三方依赖、原生二进制均可能不被识别。PowerShell 专用语法属于后续工作。

不存在的标准库或第三方 Python 模块不当作缺失本地引用处理，也不会遍历虚拟环境或 node_modules。没有解析到的依赖可能完全不进入检查流程；`inspection_complete=true` 只表示本检测子集内未遇到已识别的读取失败，不表示完整程序语义已覆盖。

每次 Hook 重新读取，没有持久缓存。即便如此，检查后的文件仍可在执行前被修改；路径检查与打开也不是抵御本机同权限恶意进程的完整机制。需要严格运行时约束时配合只读工作区、宿主权限和系统 DLP。
