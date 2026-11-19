# v0.2.0：告警确认与最近 100 条决定

## 如何使用

将需要人工判断的规则设为 `action: confirm`（也接受 `ask` 别名）。`deny` 仍是不可由本次确认覆盖的拒绝；优先级为 deny > confirm > warn > allow。

v0.2 默认包将邮箱、手机号、敏感文件路径改为 confirm；Secret 和潜在外传默认 deny。旧策略中的 warn 保持“提醒并继续”，不会被静默改写。自定义策略需自行选择哪些规则使用 confirm。

升级二进制后重新运行 install，以生成带 `--interactive-confirmation`、固定历史路径和 90 秒宿主超时的 Hook。使用旧 Hook 的 5 秒配置时，不会启动等待确认的页面：confirm 会直接拒绝并提示重新安装。

```sh
# 若使用现有自定义策略，先把需要确认的规则 action 改为 confirm。
./agent-dlp --policy policy-pack.yaml install --agent codex --settings "${CODEX_HOME:-$HOME/.codex}/hooks.json"
```

重启/重新加载宿主后验证。WorkBuddy 仍须指定实际加载的 settings.json；桌面端整体拦截效果须分别实测。

## 确认过程

1. 执行前规则命中 confirm，当前 Hook 暂停，并用操作系统的默认浏览器打开本地确认页。
2. 页面展示规则编号和最多 400 字符的脱敏上下文。用户选择“仅允许本次”或“拒绝本次”，可填写原因。
3. 60 秒内未确认、浏览器无法打开或本地服务不可用，返回拒绝。关页会在超时后拒绝。
4. 点击允许后重新读取策略和已检查脚本引用闭包，指纹变化即拒绝。允许仅属于这一次调用，不缓存批准，也不修改策略；Agent 自己的其他权限检查照常进行。

这里的确认页是 Agent DLP Lite 自己提供的临时本地页面，不是宿主的原生 ask。Codex 官方文档说明 PreToolUse 的 permissionDecision=ask 尚不支持并可能使调用继续，因此本工具不会输出该值。PostToolUse 始终只是观察，不弹出事后放行对话框。

服务只监听 127.0.0.1 的临时端口，采用一次性随机路径/令牌，只有有效 POST 表单提交才能作出决定；GET、外站 Origin、错误令牌均不能批准。页面不加载外部资源，敏感内容在生成 HTML 前脱敏并转义。只在告警期间运行，无常驻服务，无外网请求。Linux 图形桌面需要系统提供 xdg-open；无桌面或无浏览器时拒绝。

宿主必须实际尊重配置中的 90 秒超时和拒绝决策。管理员强制更短超时、宿主关闭或绕过 Hook 等情形仍属宿主边界；不能据本地协议测试声称桌面端绝对不可绕过。

## 最近 100 条记录

默认位置：macOS/Linux 为 `~/.agent-dlp-lite/history.jsonl`；Windows 为 `%LOCALAPPDATA%/.agent-dlp-lite/history.jsonl`，不可用时使用 USERPROFILE。可用全局 `--history-file PATH` 指定可信位置，安装生成的命令会固定该绝对路径。旧 `hook --audit PATH` 参数兼容，写入同样的有界格式。

仅记录执行前出现告警的最终放行/拒绝；普通无告警放行、scan 模拟测试、PostToolUse 观察不写入。每次写入后只保留最新 100 条，自动删除更早记录，不生成历史备份。多个 Hook 使用同一日志时通过系统文件锁串行写入，再原子替换文件。记录不可写时拒绝当前告警操作。

每条记录包含：时间、事件类型、工具名称、规则与置信类型、最终动作、决定来源、用户原因、脱敏上下文、检测器版本、策略指纹。决定来源区分 user、policy、timeout、unavailable、changed 和 incomplete，不能把超时/不可用误当作用户明确拒绝。

命中已启用识别规则的内容会被替换为 REDACTED 标记，然后才截断摘要。不会保存原始完整事件、会话文件路径、确认 URL 或令牌。未识别的敏感内容仍可能保留在上下文或用户原因中；记录和导出文件应留在可信位置。Unix 新文件使用 0600，新建历史目录使用 0700；Windows 使用用户目录 ACL。相同用户权限下的恶意程序不在不可绕过边界内。

```sh
./agent-dlp history list
./agent-dlp history export --output alert-decisions.jsonl
# 指定日志位置时，查看/导出也传同一选项。
./agent-dlp --history-file /absolute/path/history.jsonl history list
```

导出最多 100 条且不覆盖已有文件；用户主动导出的副本由用户管理，不参与滚动删除。v0.1 的旧审计对象可读取，但不带完整用户反馈字段。

## 为策略调整准备数据

本版本只收集与导出反馈，不自动放宽规则，也不向外部模型发送记录。后续分析应筛选 `record_version=1` 且 `decision_source=user`，再结合规则 ID、脱敏上下文、原因和策略指纹形成建议。超时、环境不可用、策略硬拒绝不能作为用户反馈训练样本。策略变更仍应审核并用新的正反例验证。

参考：[Codex Hooks](https://learn.chatgpt.com/docs/hooks)。
