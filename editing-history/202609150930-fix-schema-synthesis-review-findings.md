# 2026-09-15 09:30 — 修复 schema synthesis review 问题

- `defwasm-import` 没有可用于推导返回类型的函数体；implementation inference 现在直接返回无证据，不再把 module/field 字符串误判为 `String` 返回值。
- 参数类型的调用点证明排除点分 namespace 中的 `test`、`tests`、`example` 与 `examples` 分段，避免样本代码单独收窄公共 schema。
- definition-attached tests/examples 继续只参与 staged validation，不提供参数推导证据。
- 增加集成回归，证明只有测试与示例 namespace 调用时参数 hole 仍保持 unresolved。
