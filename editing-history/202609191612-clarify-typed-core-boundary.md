# 明确 typed core 与自动 fix 的边界

- 明确 typed core 首先是一组编译器内部 invariant，不要求立即引入完整的新 AST。
- 区分类型与解析证据、source identity、origin chain 和自动改写安全性，避免把类型正确误当成 source fix 语义等价。
- 记录短期继续增强现有 preprocess 表示的顺序，以及重新评估独立 `CoreExpr`/arena 的具体触发条件。
- 强调独立 typed core AST 具有长期价值，但不应阻塞已有证据足够的确定性迁移规则。
