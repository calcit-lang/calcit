# Clarify incremental cache fallback / 澄清增量缓存回退

- Distinguished input-cache reload behavior from definition-cache cold starts in the user and agent documentation.
- Documented every definition-cache cold-start condition: missing or unreadable cache, schema or compiler version mismatch, and context revision changes.
- Clarified that a failed input-cache write does not fail the current analysis and simply leaves that cache unchanged.
- 在用户与 Agent 文档中区分 input cache 源码重载和 definition cache 冷启动。
- 补全 definition cache 冷启动条件：缓存缺失或不可读、schema 或编译器版本不匹配，以及 context revision 变化。
- 明确 input cache 写入失败不会中断本次分析，只是不更新对应缓存。
