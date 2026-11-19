# 策略迁移

统一包包含规则、动作、Source/Sink 和资源限制，不包含 Agent 安装路径、账户或二进制。`schema_version: 1` 是兼容边界；未知版本拒绝导入。

```sh
agent-dlp --policy policy-pack.yaml policy export --output shared.yaml
agent-dlp policy import shared.yaml --output imported.yaml
agent-dlp --policy imported.yaml policy validate
agent-dlp --policy imported.yaml integration --agent codex
agent-dlp --policy imported.yaml integration --agent workbuddy
```

导入默认不覆盖；`--force` 覆盖前备份。导出/导入经过反序列化与序列化，语义保留，注释与排版不保留；初始化模板仍带注释。

规则本身与 Agent 无关，但宿主事件覆盖和传入工具名可能不同。相同输入得到相同结论，不等于两个宿主能观察到相同事件。业务规则应优先使用统一事件和文本特征；包含操作系统路径的正则要同时考虑 `/` 与 `\`。

分享前审核正则中是否写入真实内部标识或敏感值。策略导入不是代码执行，仍属于安全配置变更：来自不可信来源的策略可能禁用检测、产生误报或增加资源消耗。本版不提供策略签名、远程分发、自动更新、远程 include 或目标可信度判断。
