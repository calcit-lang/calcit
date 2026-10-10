# 嵌套 decoder 的查询源码关联

关联 #1906、#1456、#1553。

原 `check-parse-boundary.mjs` 将 definition 的闭合 decoder 成功与拒绝
`:tests` 原样放进同一入口。对这些嵌套调用运行 `query type-at` 时，源码
坐标遍历会先命中展开后保留坐标的匿名 Enum 类型表达式，而非原调用。
原回归新增的证据检查因此失败：`try-parse-cirru-edn-as` 被显示为精确
`Enum`，没有对应的 `ResultOps`，但顶层 WASI fixture 仍显示具体 Result。

复用已有唯一 source-expression trace：查询普通调用时，如果坐标命中的
展开节点与原调用 head 不同，就从 trace 取得真正的调用证据。不能把
存在同一坐标当成调用身份的证明，也不凭名称重建 decoder 结果类型。
追踪用于源码的符号/导入/方法及 reader 已解析的 primitive/syntax head；Tag 字段
调用继续使用现有展开证据。原 Agent `sum-point` 字段查询曾揭示扩大到
所有 head 会丢失 Number 证据，修复因此保留该边界及原断言，不改预期。
这不修改语言类型规则、运行时 decoder、源码 AST 或公开查询 envelope。

回归保留原四份成功/拒绝调用及运行时断言，用 search 定位，再检查
type-at 的精确证据和结构化 Result 方法合同；不比较展示字符串，不把
宽 builtin 声明冻结为业务返回类型。原 native/JS、formatter WASM 及
unsupported 检查继续执行；查询前后 Snapshot 字节必须不变。

验证使用 cargo fmt/build/Clippy、现有解析边界、Agent 与 query-derived
契约脚本。具体完成结果、最新 CI 与审查记录写在 PR；完整家族和真实
消费者仍按父任务独立验收，不从这四个调用推断全部 core 已冻结。
