# 断言表达式的已知矛盾检查

## 决策

#1538 的第一步已有局部变量矛盾检查，但字面量和调用结果绕过了同一 TypeProof 检查。发布版 0.28.0-alpha.3 中，`assert-type |hello 'Number` 的预处理通过，native 仅在运行时失败。JS/WASM 不应因独立 lowering 而接受已知错误。

将现有方向性 proof 检查移到表达式与 local 分支之前，复用 `resolve_type_value`，只拒绝确定的 Mismatch。NeedsBoundary、Dynamic 和推断失败的迁移政策保持不变；不添加开关、规则入口、类型关系或消费者豁免。局部 scope refinement 仍仅适用于 local，不给任意表达式制造 scope 证明。

## 验证

在 `calcit.core/assert-type` 的 definition `:tests` 增加字面量、调用结果和 List payload 正例。已有 CLI/backend 脚本复用这些测试，native 与真实 JS 执行全部正例，WASM 执行已支持的 scalar subset。严格负例同时覆盖 local、字面量、调用结果及 List/Option payload，要求四种入口在预处理阶段返回同一个诊断 code 与位置。

这只是 #1538 的有界补漏，不关闭 Dynamic 参数/返回值、erased callback 或 #1542 的传播问题，也不提前开启全面默认门禁。

全量回归还发现既有跨 namespace 查询 fixture 用特殊的 core `Option` 构造写法，却断言成另一个 namespace 的同名 enum；旧检查把错误标签当成推断证据。正例改为明确调用本来期望的 qualified constructor，预期 nominal identity 不变，并增加 Option → Result 的负例以保留不同名义类型不可互换的契约。没有放宽 proof 关系来迁就错误 fixture。
