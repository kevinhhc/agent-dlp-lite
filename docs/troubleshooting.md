# 故障排查

| 现象 | 检查与处理 |
| --- | --- |
| 配置存在但不阻断 | `status` 只表示注册；确认宿主版本、真实配置路径、Hook 信任/开关、重新加载情况，再按兼容性文档实测 |
| WorkBuddy 无反应 | 区分 WorkBuddy 桌面、CodeBuddy IDE、CodeBuddy Code；不要混用配置目录；桌面真实配置位置尚待验证 |
| Hook 启动失败 | 二进制与策略是否移动/删除；绝对路径和执行权限是否正确；macOS 下载隔离、签名要求是否满足 |
| 配置被拒绝 | 只支持严格 JSON；JSONC 注释、错误结构、符号链接不会自动覆盖；保留原文件后人工调整 |
| 策略校验失败 | 版本、未知字段、重复 ID、正则和资源范围；错误不回显输入内容以避免泄露 |
| 误报 | 用 `scan` 查规则 ID，修改对应模式、events、enabled 或 action；allow 不能覆盖 deny；需要人工确认用 confirm，warn 只提醒并继续 |
| INSPECTION-INCOMPLETE | 检查脚本相对 cwd、权限、UTF-8、引用根目录和大小/深度限制；对不支持语法缩小检测预期 |
| scan 退出 2 | 正常代表 deny 或待确认 confirm（scan 不打开确认页）；Hook 模式有效 deny JSON 用退出 0，错误才用退出 2 |
| 速度异常 | 用 release 二进制；限制规则数量和模式长度；在相同机器运行进程级 benchmark，检查审计磁盘和宿主开销 |
| 重复触发 | 查找用户级/项目级/系统级重复注册；同文件重复 install 会替换本工具标记项，跨文件重复需要人工清理 |
| 已卸载仍报错 | 另一个配置层可能也注册了 Hook；定位并逐个 uninstall，之后再删除二进制 |

`compatibility` 展示本工具协议范围，不自动探测或认证桌面端。`self-test` 验证内置默认引擎，不替代自定义策略测试和实际宿主验收。

## v0.2.2 原生确认排查

- 没有原生表单：确认客户端支持 mcp_tool Hook 和 form elicitation；重新 install 时传 --mcp-config，重启并信任 Hook，检查 agent-dlp-lite MCP 已连接。缺少能力时显示告警、记录、继续。
- 仍打开浏览器：旧配置仍含 --interactive-confirmation；用新安装命令替换管理项。
- 60 秒无回复：原生默认告警继续，并记录 timeout；用户取消/拒绝仍阻断。浏览器兼容模式保持超时拒绝，不要混淆。
- 安装要求 MCP 路径：Codex 需要 --settings hooks.json 和 --mcp-config config.toml；WorkBuddy 不需要 MCP 路径。
- 看不到记录：查看与安装相同的 --history-file；无告警、scan、PostToolUse 不写入。
- companion 记录：表示独立告警守卫已记录但尚未得到原生最终结果；不是人工批准。缺少宿主 ID 时可有重复条目，上限仍为 100。
- 历史失败：检查权限、空间、文件损坏和符号链接；当前告警返回阻断，不能静默清空历史。
- macOS 无法验证开发者：先核对来源和 SHA-256，再在系统设置的隐私与安全性中选择仍要打开；候选文件尚未签名/公证。
