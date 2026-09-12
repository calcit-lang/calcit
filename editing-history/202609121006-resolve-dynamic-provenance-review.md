# 收尾 Dynamic provenance 的两条未决 review 意见

## 背景

0.14.9 milestone 的发布门禁要求清零里程碑实现 PR 的未解决 review threads。复查 #974（bounded Dynamic provenance，来自 #940）时仍有两个 CodeRabbit 线程未处理，且都不影响编译语义，但会破坏对外的 JSON 契约或误标诊断来源。

## 修改

- `query type-at` 的 `ContextDiagnostic` 移除 `provenance` 上的 `skip_serializing_if`。文档与 `analyze check-public` 都要求该字段始终为数组，空时也应为 `[]`，不再因省略字段破坏消费者。
- `nearest_dynamic_provenance` 判断原始 `get` 符号时，只在它未被局部绑定（`scope_types`）遮蔽、且不是指向同名外层定义的 self-reference 时，才标记为 `calcit.core/get`；符号解析中 core 先于同命名空间定义，因此同命名空间存在 `get` 不会遮蔽 core `get`。删除把 `Calcit::Local` 当作 core `get` 的分支。
- 新增回归测试：空 provenance 的 `ContextDiagnostic` 序列化后仍包含 `provenance: []`；被局部 `get` 遮蔽或 self-reference 到项目 `get` 的调用都不会产生 `calcit.core/get` 来源，且保持简洁诊断。

## 验证

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`（含新增 `context_diagnostic_always_serializes_provenance_array` 与 `skips_dynamic_provenance_for_shadowed_get_binding`）
- `yarn compile`
- `yarn check-agent-interface`
