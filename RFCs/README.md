# RFC 整理索引

更新时间：2026-08-23

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
| `04-13-type-slot-mechanism-rfc.md`                  | Partial       | Revision 3：已落地无条件擦除与 entry `:type-slots`；环境指纹和 namespaced identity 暂缓。 |
| `04-15-match-syntax-rfc.md`                         | Active        | match 语法改进提案。                                                                      |
| `04-15-type-directed-optimization-catalog.md`       | Active        | 基于 `&record:nth` 经验，系统梳理 Record/Tuple/Scope 等类型导向优化机会。                 |
| `04-15-wasm-compilation-feasibility.md`             | Active        | WASM 编译三条路径（解释器→WASM / AOT 子集 / WASM GC）的可行性评估。                       |
| `04-16-wasm-data-structures.md`                     | Active        | WASM codegen 中 Tag/Record/Tuple 等数据结构的内存布局与编译策略。                         |
| `05-31-generic-where-bounds-mfs.md`                 | Active        | 函数 schema 泛型 `:where` 约束的最小功能规格，先作为主链路开发基线。                      |
| `06-15-effects-graph-rfc.md`                        | Draft         | `calcit analyze effects-graph`：State/Transform/Effect 语义分解图与类型驱动 effect 标注路线。 |
| `06-29-cr-exec-cli-builtins-rfc.md`                 | **Active**    | `calcit exec` + `calcit.cli/*` 内建函数：绕过 Shell 转义的 Cirru 函数调用方案。               |
| `07-06-semantic-tree-navigation-rfc.md`             | Draft         | 语义化树形导航与编辑：路径标注、多候选交互、锚点搜索替换、结构化查询语言。                |
| `07-19-doc-knowledge-index-rfc.md`                  | Draft         | Markdown/Calcit snapshot 的知识节点、关系索引与用户级增量缓存方案。                       |
| `07-19-type-introspection-consistency-rfc.md`       | Implemented   | 类型自省一致性改进：`&methods-of` 支持裸类型定义、`Enum` Display 补 variants、`to-pairs`/`keys` 类型签名修正（第 4 项可选新增 proc 延后）。 |
| `07-26-agent-machine-protocol-rfc.md`               | Draft         | Agent typed result、JSON 协议、definition descriptor 与按需重新解析/可选 daemon 的边界。                                |
| `07-26-static-semantic-analysis-rfc.md`             | Draft         | 静态类型发现、类型证据、动态边界分类与统一结构化诊断。                                                                  |
| `07-26-safe-structured-editing-rfc.md`              | Draft         | revision/fingerprint 前置条件、事务编辑、语义 diff 与受影响范围验证。                                                  |
| `07-26-agent-docs-and-evaluation-rfc.md`            | Draft         | 结构化文档上下文、默认检索范围和 Agent 接口基准。                                                                        |
| `07-28-git-module-store-rfc.md`                     | Draft         | 保持 `deps.cirru` 与 Git 模块路径，以 tag 为最佳实践并使用 pnpm 式全局目录存储；不引入 registry、lockfile、workspace 或多版本。 |
| `07-28-persistent-tree-cursor-rfc.md`               | Draft         | `.calcit/` 本地状态、虚拟 cursor、region/marks/last-query、结构化 clipboard 与 path 迁移。                            |
| `08-14-architecture-scaffold-rfc.md`                | Implemented   | Cirru EDN architecture graph、existing-definition reconciliation、atomic scaffold apply、work items 与多 Agent 分工边界。 |
| `08-14-todo-placeholder-rfc.md`                      | Partial       | compiler-known `todo!`、`W_TODO` 与 native/JS/WASM 中止行为已落地；完整 Never/control-flow inference 后续实现。 |
| `08-04-strict-cirru-edn-decoding-rfc.md`            | Implemented   | Phase 1：`parse-cirru-edn-as` 严格类型化反序列化、无 Dynamic 的 `EdnDecoderGraph`、名义身份与 Native/JS 一致性。    |
| `08-05-systematic-nil-reduction-rfc.md`              | Partial       | 类型驱动减少 nil：先拆分可省略参数与 nullable 值，再迁移至 Option/Result 并逐步收紧 typed code。                  |
| `08-08-cross-backend-host-ffi-contracts-rfc.md`      | Draft         | 统一 JS/native/WASM/WASI 的逻辑 FFI 契约与诊断，ABI transport 保持 backend-specific；首个完整 shape consumer 为 JS/DOM。 |
| `08-18-calcit-typed-js-ffi-boundary-rfc.md`           | Draft         | 在现有 Struct/Enum/Fn/trait 上补齐 JS capability gate 与 target validation；FFI metadata 不进入普通 trait 匹配和泛型推断。 |
| `08-21-setup-calcit-version-and-toolchain-contract-rfc.md` | Draft       | 以 `deps.cirru` 为正常项目的唯一 Calcit 版本来源；只下载 `calcit`，并由 Action 提供 `cr` 兼容链接。 |
| `08-21-type-quality-ci-adoption-rfc.md`               | Draft       | 统一使用原生 `analyze quality` 与按 definition baseline，定义生态 CI 层级并禁止各项目重复实现 JS 汇总脚本。 |
| `08-21-js-ffi-runtime-contract-validation-rfc.md`     | Draft       | 在现有 typed JS FFI 声明之上增加 host guard、decoder、runtime contract tests 与 unsafe evidence。 |
| `08-21-static-type-system-evolution-roadmap.md`       | Draft       | 借鉴 Rust/MoonBit 推进 Unknown/Dynamic 分离、穷尽性、局部推断、trait coherence 与框架类型化。 |
| `08-23-typed-option-query-ergonomics-rfc.md`           | Withdrawn | 生态采用率为零；`get-or` 等六个宏已移除，直接使用查询返回的 `Option` 与 `.unwrap-or`。 |
| `08-23-option-result-binding-macros-rfc.md`             | Partial | 保留已有 Respo 使用的 `option:let`；移除未采用的 `result:let`，Result 链直接使用 `.and-then`。 |

## 已执行的清理

- 删除了旧的类型标注/泛型函数草案，避免继续传播过时 `hint-fn`、旧函数类型 DSL 与迁移期描述；
- 删除了旧审查报告和 archived 下的历史快照，避免大模型后续检索到过时语义；
- 空的 `archived/` 目录不再作为阅读入口使用。

## 后续维护规则

1. 若文档里的语法示例已经不符合当前实现，优先修正；
2. 若文档主要价值只剩“历史过程”，优先删除并让位给 `editing-history/`；
3. 若文档保留，至少应保证示例语法与当前代码库一致。
