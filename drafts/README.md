# Drafts 整理索引

更新时间：2026-02-16

## 使用约定

- **Active**：仍在推进，优先参考。
- **Review-needed**：内容可用，但包含阶段性假设或部分已实现项，使用前需对照当前代码。
- **Archived**：历史记录，仅用于追溯，不作为当前决策依据。

## 文档状态

| 文件                                    | 状态          | 建议                                                                           |
| --------------------------------------- | ------------- | ------------------------------------------------------------------------------ |
| `assert-types-plan.md`                  | Active        | 类型标注主计划文档，持续维护。                                                 |
| `assert-types.md`                       | Review-needed | 技术背景较完整，但篇幅大且部分描述偏阶段性，引用时建议抽取成专题文档。         |
| `generics-struct-fn-proc-plan.md`       | Active        | 泛型 struct/fn/proc 设计草案，适合继续拆任务推进。                             |
| `runtime-traits-plan.md`                | Active        | runtime traits 主设计文档，当前阶段的核心参考。                                |
| `project-modernization-roadmap.md`      | Review-needed | 里程碑记录详尽，但包含大量“已完成”条目，建议后续拆分为 `done` 与 `next` 两段。 |
| `macro-trait-improvements-checklist.md` | Archived      | 本轮 checklist 基本完成，建议保留存档，不再继续追加。                          |
| `record-struct-and-enum-plan.md`        | Review-needed | 迁移计划多项已完成，可作为变更历史，但需和当前语义再次对齐。                   |
| `record-struct-and-enum-rfc.md`         | Archived      | RFC 历史提案，保留用于背景追溯。                                               |
| `last-session.md`                       | Archived      | 会话快照，存在历史上下文（含旧语法/阶段结论）。                                |

## 下一步整理建议

1. 把 `Review-needed` 文档按“已落地 / 待落地”拆分，减少阅读噪音。
2. 将 `Archived` 文档在标题处统一加“归档”标识（目前已处理 `last-session.md`）。
3. 当某项草案进入实现阶段时，追加“最后验证日期 + 对应测试命令”。
