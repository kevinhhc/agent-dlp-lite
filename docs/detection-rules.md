# 识别规则参考

完整模板：[policy-pack.yaml](../examples/policy-pack.yaml)。规则片段：[custom-rules.yaml](../examples/custom-rules.yaml)，须追加到完整包的 `rules` 中，不能直接作为完整包使用。

| 字段 | 含义 |
| --- | --- |
| `id` | 全包唯一，1–64 字节 ASCII 字母、数字、`-_.`；会进入脱敏报告 |
| `kind` | `secret` / `pii` / `sensitive_file` / `custom`；作为报告分类 |
| `pattern` | Rust regex 正则，最多 4096 字节；无回溯引用、无 lookaround |
| `action` | `allow` / `warn` / `deny` |
| `enabled` | 默认 true；禁用规则仍校验语法 |
| `events` | 默认空；可指定 `PreToolUse`、`UserPromptSubmit`、`PostToolUse` |

模式不能匹配空字符串，编译大小也有限制。规则与 flows 合计最多 128 条；策略文件最大 256 KiB。未知字段、重复 ID、未知事件和未知动作均拒绝。

工具参数会递归扫描字符串、对象键、数值；Prompt 扫描 `prompt`；PostToolUse 额外扫描 `tool_response`。不扫描 session_id 或 transcript_path，不自动打开会话记录或普通工具引用的所有文件。`sensitive_file` 是路径文本模式，不会因路径命中而读取文件中的真实数据。受支持的脚本会另行读取并检查。

内置 Secret 是确定格式和启发式赋值模式，不是所有厂商密钥库；赋值模式可能匹配示例代码或普通文本。PII 格式规则未做身份真实性验证，不包含姓名、住址或所有证件类型。手机号、邮箱默认 warn，可按组织需求改为 deny。

脱敏报告只包含规则 ID、分类、动作和置信类型，没有值、偏移、代码片段或文件路径。ID 应使用稳定的技术编号，不要把客户姓名、秘密或完整路径放进 ID。配置管理命令可以列出配置本身，不属于脱敏审计接口。

修改后：`agent-dlp --policy YOUR_PACK rules validate`，再用 `policy test --file EVENT_JSON` 验证允许、提醒和拒绝案例。使用合成数据；不要把真实凭据提交到测试目录。
