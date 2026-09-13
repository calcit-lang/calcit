# 增加类型证明的 Struct 必填字段迁移

## 背景

严格类型模式会拒绝在已知 Struct 上继续使用可选查询 `get`，但旧源码需要在通过严格门禁之前获得可审阅、可原子应用的迁移路径。迁移不能依赖新的统计 analyzer，也不能通过默认值、unwrap、`unsafe-coerce` 或扩大 Dynamic 绕过类型语义。

## 调整

- 增加稳定规则 `required-struct-field-v1`，消费已有 `W_STRUCT_FIELD_OPTIONAL_LOOKUP` 和 preprocess/type-at 类型证据。
- 只处理精确 core `get`、两个参数、静态 tag、单一具名 Struct 与已声明字段，将整个调用替换为 `(:field receiver)`。
- suggestion 暴露 core definition、Struct identity、字段名与声明类型，并用完整 AST fingerprint 和 guarded `tree replace` 进入现有原子 transaction。
- 初次规划临时使用兼容 preprocess 读取旧代码；replacement 后的 staged validation 仍重新执行严格 preprocess，普通编译路径不受影响。
- 为复杂 receiver 保留准确的 source argument location；无法回到精确 source call 的 macro 结果不会产生自动建议。
- Dynamic、运行时 key、未知字段、本地 shadow 和 `.unwrap-or`/`.or-else` 业务默认路径保持不改写。

## 验证分层

- 通过 Calcit CLI 在 fixture definition 的 `:tests` 中加入 before/after 与 must-not 场景，覆盖简单/复杂 Struct receiver、Dynamic、运行时 key、未知字段、fallback 和 shadow。
- Rust 集成测试只负责 CLI 进程、revision、fingerprint、staged preprocess、原子写入与幂等边界；迁移后的业务结果由 Calcit `:tests` 验证。
- Agent interface 增加 Struct/field evidence 的 JSON 契约场景。

## 生态迁移证明

- 在维护中的 JS 应用 `calcit-lang/phoenix-script` 临时副本加入具名 Struct、旧式 `get` 和 definition `:tests`，preview 展示 Struct/field 证据，按 revision 原子应用后再次 preview 为零建议。
- 在共享库 `respo-message.calcit` 临时副本重复同一流程；两个副本都通过 0.14 默认严格诊断的 `--check-only`、目标 Calcit 测试、JS generation 和生成模块加载。
- `phoenix-script` 的完整 smoke build/test 通过；`respo-message.calcit` 的 compile、原有 unit tests 和 Vite production build 通过。Vite 保留一个既有的缺失 export warning，与注入定义和迁移结果无关。
- `--strict-types` 额外执行全项目零类型债务质量门；两个上游项目已有 `unsafe-coerce` 或未完整 schema 债务，因此该额外 gate 不作为本次局部迁移结果。普通严格诊断检查在迁移后通过，原仓库工作树保持干净。
