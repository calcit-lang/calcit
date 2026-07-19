# RFC 整理索引

更新时间：2026-06-29

## 目录原则

- 这里只保留**仍适合未来继续阅读**的草案；
- 已完成、已过时、会误导后续实现的内容，直接删除；
- 历史过程统一以 `editing-history/` 为准，不再在 rfc 中重复保留。

## 当前保留文件

| 文件                                                | 状态          | 建议                                                                                      |
| --------------------------------------------------- | ------------- | ----------------------------------------------------------------------------------------- |
| `03-05-function-schema-dual-track-rfc.md`           | Active        | 已收敛为当前 schema 约定说明，涉及函数 schema 时优先参考。                                |
| `02-04-runtime-traits-plan.md`                      | Active        | runtime traits 主设计文档。                                                               |
| `02-17-register-platform-api-rfc.md`                | Active        | host capability / register API 规范草案。                                                 |
| `02-18-language-theory-evolution-plan.md`           | Review-needed | 偏理论路线图，阅读时需区分愿景与已落地内容。                                              |
| `02-23-optional-record-macro-plan.md`               | Review-needed | 小范围提案，尚未进入稳定实现。                                                            |
| `02-14-project-modernization-roadmap.md`            | Review-needed | 工程路线图可参考，但不要当作语法或行为文档。                                              |
| `03-16-runtime-boundary-refactor-plan.md`           | Review-needed | 运行时边界重构方案。                                                                      |
| `03-18-query-def-tree-show-chunked-display-plan.md` | Review-needed | query/tree show 分块展示方案。                                                            |
| `04-13-call-arg-literal-rewrite-rfc.md`             | Active        | 调用参数字面量重写优化提案。                                                              |
| `04-13-type-slot-mechanism-rfc.md`                  | Active        | Type slot 机制提案。                                                                      |
| `04-15-match-syntax-rfc.md`                         | Active        | match 语法改进提案。                                                                      |
| `04-15-type-directed-optimization-catalog.md`       | Active        | 基于 `&record:nth` 经验，系统梳理 Record/Tuple/Scope 等类型导向优化机会。                 |
| `04-15-wasm-compilation-feasibility.md`             | Active        | WASM 编译三条路径（解释器→WASM / AOT 子集 / WASM GC）的可行性评估。                       |
| `04-16-wasm-data-structures.md`                     | Active        | WASM codegen 中 Tag/Record/Tuple 等数据结构的内存布局与编译策略。                         |
| `05-31-generic-where-bounds-mfs.md`                 | Active        | 函数 schema 泛型 `:where` 约束的最小功能规格，先作为主链路开发基线。                      |
| `06-15-effects-graph-rfc.md`                        | Draft         | `cr analyze effects-graph`：State/Transform/Effect 语义分解图与类型驱动 effect 标注路线。 |
| `06-29-cr-exec-cli-builtins-rfc.md`                 | **Active**    | `cr exec` + `calcit.cli/*` 内建函数：绕过 Shell 转义的 Cirru 函数调用方案。               |
| `07-06-semantic-tree-navigation-rfc.md`             | Draft         | 语义化树形导航与编辑：路径标注、多候选交互、锚点搜索替换、结构化查询语言。                |
| `07-19-type-introspection-consistency-rfc.md`       | Implemented   | 类型自省一致性改进：`&methods-of` 支持裸类型定义、`Enum` Display 补 variants、`to-pairs`/`keys` 类型签名修正（第 4 项可选新增 proc 延后）。 |

## 已执行的清理

- 删除了旧的类型标注/泛型函数草案，避免继续传播过时 `hint-fn`、旧函数类型 DSL 与迁移期描述；
- 删除了旧审查报告和 archived 下的历史快照，避免大模型后续检索到过时语义；
- 空的 `archived/` 目录不再作为阅读入口使用。

## 后续维护规则

1. 若文档里的语法示例已经不符合当前实现，优先修正；
2. 若文档主要价值只剩“历史过程”，优先删除并让位给 `editing-history/`；
3. 若文档保留，至少应保证示例语法与当前代码库一致。
