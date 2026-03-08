# Drafts 整理索引

更新时间：2026-03-09

## 目录原则

- 这里只保留**仍适合未来继续阅读**的草案；
- 已完成、已过时、会误导后续实现的内容，直接删除；
- 历史过程统一以 `editing-history/` 为准，不再在 drafts 中重复保留。

## 当前保留文件

| 文件                                | 状态          | 建议                                                       |
| ----------------------------------- | ------------- | ---------------------------------------------------------- |
| `function-schema-dual-track-rfc.md` | Active        | 已收敛为当前 schema 约定说明，涉及函数 schema 时优先参考。 |
| `runtime-traits-plan.md`            | Active        | runtime traits 主设计文档。                                |
| `register-platform-api-rfc.md`      | Active        | host capability / register API 规范草案。                  |
| `language-theory-evolution-plan.md` | Review-needed | 偏理论路线图，阅读时需区分愿景与已落地内容。               |
| `optional-record-macro-plan.md`     | Review-needed | 小范围提案，尚未进入稳定实现。                             |
| `project-modernization-roadmap.md`  | Review-needed | 工程路线图可参考，但不要当作语法或行为文档。               |

## 已执行的清理

- 删除了旧的类型标注/泛型函数草案，避免继续传播过时 `hint-fn`、旧函数类型 DSL 与迁移期描述；
- 删除了旧审查报告和 archived 下的历史快照，避免大模型后续检索到过时语义；
- 空的 `archived/` 目录不再作为阅读入口使用。

## 后续维护规则

1. 若文档里的语法示例已经不符合当前实现，优先修正；
2. 若文档主要价值只剩“历史过程”，优先删除并让位给 `editing-history/`；
3. 若文档保留，至少应保证示例语法与当前代码库一致。
