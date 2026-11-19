# v0.2.2：本地 MCP 表单确认与最近 100 条记录

## 默认行为

Codex 使用本地 STDIO MCP Hook 调用同一个 Rust 二进制的 mcp 子命令。confirm（ask 别名）命中后，向支持 form elicitation 的客户端发送结构化表单，字段为本次选择和可选原因；只有有效的 allow_once 或 deny 才是明确的表单反馈。默认不打开浏览器、不监听网络端口、不调用外部 API。Codex 管理一个随连接运行的本地 MCP 子进程，无需 Rust、Python 或 Node.js 运行环境。

| 情形 | 结果 | 记录来源 |
| --- | --- | --- |
| 表单选择 allow_once | 仅放行当前调用，保留宿主其他权限检查 | user，human_feedback=true |
| 表单选择 deny | 拒绝当前调用 | user，human_feedback=true |
| 客户端 decline / cancel | 拒绝；不能确定 decline 一定来自人工 | declined / cancelled，非人工样本 |
| 不支持表单 / 返回错误 / 无效或空选择 | 告警、记录、继续 | unavailable / protocol_error |
| 60 秒未收到有效选择 | 取消等待，告警、记录、继续 | timeout |
| 确认期间策略或已检查脚本发生变化、检查不完整 | 拒绝 | changed / incomplete |
| 任意规则 deny | 拒绝，不请求放行 | policy |
| 历史不可写或检查失败 | 返回阻断决策 | 不伪造成功记录 |

confirm 是需要判断的告警；deny 保持硬拒绝，优先级为 deny > confirm > warn > allow。默认包中 PII/敏感路径为 confirm，Secret 和潜在外传为 deny。旧包 warn 仍只告警并继续，不自动改写策略。

## 接入 Codex

先升级稳定目录中的二进制和策略，再安装两个配置文件：

~~~sh
./agent-dlp --policy policy-pack.yaml install --agent codex --settings "${CODEX_HOME:-$HOME/.codex}/hooks.json" --mcp-config "${CODEX_HOME:-$HOME/.codex}/config.toml"
~~~

安装器保留其他 Hook、MCP 服务器和 TOML 注释，修改前备份。两份文件先校验再写入，但不是跨文件事务；若第二份写入失败，检查错误和备份后修复，不要假定整体已回滚。重启/重新加载客户端，按宿主要求信任更新后的 Hook，并检查 agent-dlp-lite MCP 已连接。安装和 status 都不证明原生 UI 已通过实际验收。

生成的每个 Codex 事件同时包含：

- 5 秒命令 Hook：独立检查、对 confirm 返回警告并记录默认继续；deny 仍拒绝。即使 MCP 未连接，仍有记录路径。
- 90 秒 MCP Hook：调用 inspect，等待原生表单并返回有效阻断或非阻断结果。内部等待上限 60 秒，不因宿主声称暂停超时而无限等待。

宿主等待同步 Hook 结束后才应执行操作。两个处理器可能并发运行，不能把配置顺序当作执行顺序。有宿主调用 ID 时，两条路径更新同一记录；迟到的命令 Hook 不会覆盖 MCP 结果。客户端缺少调用 ID 时可能出现重复记录，但总保留量仍为 100。

MCP Hook 自身错误、缺失服务器可能被宿主默认放行，因此命令 Hook 必须一起保留。若旧客户端连 mcp_tool 配置都不支持，使用 WorkBuddy 式命令告警配置，勿声称已启用原生确认。不要绕过组织禁止交互的策略；客户端明确 decline/cancel 仍拒绝。

## WorkBuddy 和浏览器兼容模式

WorkBuddy 默认生成命令 Hook：confirm 告警、记录并继续，deny 阻断。其桌面版本的原生 MCP 表单接入尚未验证，不自动生成 Codex 格式的原生处理器。

旧版浏览器方式仅保留为显式 hook --interactive-confirmation 兼容选项；它仍采用 v0.2.0 的超时/不可用拒绝语义，不能与原生默认降级语义混用。自动生成的新配置不包含此选项。

## 最近 100 条决定

默认路径：macOS/Linux 为 ~/.agent-dlp-lite/history.jsonl；Windows 为 LOCALAPPDATA 下的 .agent-dlp-lite/history.jsonl，不可用时使用 USERPROFILE。可用全局 --history-file PATH 指定可信位置；安装生成的命令及 MCP 参数固定同一绝对路径。旧 hook --audit PATH 兼容。

只记录执行前告警；无告警放行、scan 和 PostToolUse 不写入。每次写入/更新后最多保留 100 条，删除更早条目，不生成历史备份。文件锁保护并发写入，使用同目录临时文件原子替换。导出副本不参与自动清理。

字段包括时间、版本、策略 SHA-256、规则、最终决定、decision_source、human_feedback、脱敏上下文（400 字符内）及原因（截断）。宿主调用 ID 只作为 operation_id 哈希的输入，不保存原值。确认等待期间可能先出现 companion 记录，表示未获人工批准的默认继续；最终结果到达后更新。

只有 human_feedback=true 且 decision_source=user 的有效表单选择适合未来人工反馈分析。companion、timeout、unavailable、protocol_error、declined、cancelled、automatic_review、changed、incomplete 和 policy 不能冒充人工许可/拒绝样本；自动审批标记不会当成人工反馈。不自动训练模型或修改策略。

~~~sh
./agent-dlp history list
./agent-dlp history export --output alert-decisions.jsonl
~~~

仅按已启用规则脱敏，未知敏感文本仍可能留在摘要或用户原因中。不要在原因中填写真实秘密；保管历史及导出文件。Unix 新历史文件 0600、新目录 0700；Windows 使用用户目录 ACL。

## 验收边界

已实现协议与进程测试不等于特定桌面版已完成原生 UI 验收。必须在实际客户端验证：原生表单出现；未选择前无工具副作用；允许/拒绝/取消正确；自动审批和完整访问模式不会产生伪人工记录；断连/无表单/超时按表格降级；历史去重和 100 条保留正确。Codex 与 WorkBuddy 分别验收。

参考：[Codex Hooks](https://learn.chatgpt.com/docs/hooks#mcp-tool-hooks)、[App Server](https://learn.chatgpt.com/docs/app-server)、[MCP Elicitation](https://modelcontextprotocol.io/specification/2025-11-25/client/elicitation)。
