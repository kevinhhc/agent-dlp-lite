# 安全策略参考

包结构必须包含 `schema_version: 1`、`name`、`inspection_failure`、`limits`、`rules`、`flows`。严格校验后才使用；显式文件缺失或错误时绝不回退到默认策略。没有 `--policy` 时使用二进制内嵌默认策略，不自动读取当前目录的配置。

每条识别规则自带动作；每条 flow 自带动作。最终取最严格结果：deny > confirm > warn > allow；无命中则 allow。不存在“后面的规则覆盖前面的规则”。需要允许某个例外时，修改对应模式/范围或禁用命中项；本版不提供全局域名白名单，避免复杂脚本因一个可信域名而全部放行。

| 动作 | 执行前事件的行为 |
| --- | --- |
| allow | 返回空 JSON，不提升宿主权限 |
| warn | 返回脱敏 `systemMessage`，不阻断；不要求用户交互 |
| confirm | Codex 原生 MCP 表单；仅批准本次；无 UI/无效/超时告警记录后继续，明确拒绝/取消仍阻断 |
| deny | PreToolUse 返回 `hookSpecificOutput.permissionDecision=deny`；Prompt 返回 `decision=block` |

PostToolUse 始终仅提示，报告保留策略 deny 结论但 `enforcement=observe_only`。不返回伪装的事后阻断。主程序接受 confirm（ask 别名），但v0.2.2 默认使用 MCP 表单，不输出宿主原生 ask；详见 [确认与记录](confirmation-history.md)。

`inspection_failure` 控制已识别脚本检查不完整时的动作，默认 deny。可改 warn，但会让未知内容继续执行；它不是输入/策略解析错误的 fail-open 开关。此类错误始终在 hook 模式退出 2，宿主是否尊重退出码需验收。

默认限制及硬性范围：

| 资源 | 默认 | 允许范围 |
| --- | --- | --- |
| 事件字节 | 256 KiB | 1 KiB–1 MiB |
| 单脚本字节 | 128 KiB | 1 KiB–1 MiB |
| 全部脚本字节 | 512 KiB | 不小于单脚本，最高 4 MiB |
| 脚本数 | 16 | 1–32 |
| 引用深度 | 4 | 1–8 |

所有读取受字节限制，正则受编译大小限制；不启动扫描子进程、不运行脚本。v0.2.2 命令守卫设为 5 秒、MCP 处理器设为 90 秒，原生确认内部等待最多 60 秒后告警继续；这些资源上限，不能替代延迟基准，也不构成强制阻断保证。
