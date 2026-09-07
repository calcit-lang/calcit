# Resolve checked-call type slots / 解引用 checked-call 类型槽

- Resolve bound `TypeSlot` receivers before selecting a shared collection
  contract, with cycle detection that leaves open or cyclic slots on the
  compatibility path.
- 将已绑定的 `TypeSlot` receiver 解引用后再选择共享集合 contract，并通过循环
  检测让未绑定或循环类型槽继续走兼容路径。
- Exercise strict `get` lowering through a bound generic list receiver so the
  contract and optional-access lowering stay aligned.
- 通过绑定为泛型列表 receiver 的严格 `get` 覆盖 lowering，确保 contract 与
  optional-access lowering 保持一致。

## Validation / 验证

- `cargo test checked_call_contract`
- `cargo test strict_types_reject_concrete_unsupported_optional_access_receivers`
