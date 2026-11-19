# 兼容性与桌面验收

核对日期：2026-11-19。实现采用两种产品已文档化协议的保守公共子集。**当前状态是协议/本地测试，桌面端实际加载和阻断未完成验收。**

| 宿主/事件 | 已实现行为 | 本地协议验证 | 桌面端实测 |
| --- | --- | --- | --- |
| Codex PreToolUse | 识别、提醒、deny JSON | 已覆盖 | 待验收 |
| Codex UserPromptSubmit | 识别、提醒、block JSON | 已覆盖 | 待验收 |
| WorkBuddy PreToolUse | 公共格式适配 | 已覆盖 | 具体桌面版本与配置待验收 |
| WorkBuddy UserPromptSubmit | 公共格式适配 | 已覆盖 | 具体桌面版本与配置待验收 |
| 两者 PostToolUse | 可手工接入的观察接口 | 引擎已覆盖 | 不默认注册，不承诺撤销 |

Codex 官方说明用户级路径是 `$CODEX_HOME/hooks.json`，默认 `~/.codex/hooks.json`；支持执行前拒绝，但部分工具路径不触发 Hook。WorkBuddy Enterprise 文档包含 CodeBuddy IDE/Code 章节，其 `.codebuddy` 配置不能证明 WorkBuddy 桌面使用相同位置。没有足够证据自动写入 `.workbuddy` 或 `.codebuddy`，因此本版要求显式 `--settings`。

参考来源：

- [OpenAI Hooks 官方文档](https://learn.chatgpt.com/docs/hooks)
- [OpenAI Codex Hooks 源码](https://github.com/openai/codex/tree/main/codex-rs/hooks)
- [腾讯 Hooks 功能文档（目录标为 CodeBuddy IDE）](https://cloud.tencent.com/document/product/1831/134517)
- [腾讯 CodeBuddy Code Hooks 配置参考](https://cloud.tencent.com/document/product/1831/137030)

## 发布前的桌面验收步骤

1. 使用独立临时工作目录、合成数据，记录操作系统、Agent 产品形态与准确版本。确认该版本官方支持的配置位置、Hook 开关、信任流程和退出码含义。
2. 使用稳定二进制和可信策略目录；生成配置预览，在测试配置中保留已有 Hook，再用 install 合并。保存备份。
3. 重启/重新加载宿主。确认宿主自己的 Hook 诊断界面或日志确实加载了注册项。单纯运行 `status` 不算成功。
4. 在策略 rules 中加入匹配 `DLP_TEST_BLOCK_2026` 的 custom deny。提交该合成 Prompt，确认模型请求被该 Hook 按预期阻断；同时检查此时是否已有宿主同步/上传行为，不能据 Hook 消息推断未上传。
5. 在隔离目录让 Agent 执行写入 `DLP_TEST_BLOCK_2026` 的无害命令，例如 `printf DLP_TEST_BLOCK_2026 > marker.txt`。确认 Hook 拒绝且 marker.txt 没有创建；再用普通标记验证正常命令可执行。
6. 创建仅含字面示例 URL、敏感来源线索和网络调用代码的脚本，先通过 `policy test` 验证，真实宿主测试时使用断网或系统隔离，确保脚本不会外传。确认文件调用会被阻断。
7. 分别模拟策略缺失、无权限、脚本超限和 Hook 超时；记录宿主是否继续执行。测试完恢复路径和权限。失败行为未验证时不能声称 fail-closed。
8. 重复安装一次，再添加另一个测试 Hook。uninstall 后验证仅本工具处理器消失，其他配置保留。清理合成文件和测试配置。

将版本、步骤、预期、实际、宿主日志脱敏证据写入验收记录后，才能把矩阵对应行改成已实测。当前不为此自动修改正在运行的用户 Agent 配置。
