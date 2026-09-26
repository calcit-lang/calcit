# 推断 helper 与迁移分析的一致性

## 问题与决策

0.23.0 的封闭 helper 推断已经允许 Respo 的 `next-resource-id!` 删除冗余 schema，但迁移期 `analyze quality` 仍只读取源码标注，误把删除标注当作新增债务（#1392）。恢复冗余 schema 或放宽 baseline 都会掩盖编译器与工具的分歧。

三个既有分析入口统一在内存中使用严格预处理证明的函数契约，再调用原有分析与增量缓存。复用查询入口的程序准备及编译器推断，不新增推断规则、指标或命令。源码中没有 schema 的事实保持不变；显式 Dynamic、无法证明的输入及编译失败不自动升级为已证明契约。

编译缓存不会重放已收集的告警。一次证明失败或产生告警后，必须清理该次编译缓存，再处理下一定义，避免有问题的依赖被后续缓存命中误认成无告警的证明。回归同时检查调用者与依赖的 coverage。

函数退出时也通过作用域 guard 清理编译缓存（包含提前返回），避免后续非严格 `schema-evidence` 分析复用严格模式产物。此处 Rust 测试只检查内部缓存与模式隔离，CLI 回归另外覆盖 schema-evidence 的用户入口；不增加重复的 Rust 语言语义测试。

## 验证边界

- 既有 Calcit definition `:tests` 继续在 native、JS、WASM 验证 helper 与普通方法调用的语义。
- CLI 回归检查完整及重复增量分析、query 一致性、显式 Dynamic/未证明输入的负例，以及分析前后 Snapshot 字节不变。这些是工具协议测试，留在 JS runner，不重复语言语义断言。
- 使用 Respo 删除 helper 标注后的真实 Snapshot 验证原有 quality baseline，不修改预算或恢复标注。
- 执行仓库格式、Clippy、Rust 和全量集成门禁；实际结果记录在关联 PR。
