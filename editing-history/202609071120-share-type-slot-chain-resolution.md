# Share type-slot chain resolution / 共享类型槽链解引用

- Extract the cycle-safe bound `TypeSlot` traversal into the checked-call
  contract module and reuse it from typed optional-access lowering.
- 将循环安全的已绑定 `TypeSlot` 遍历提取到 checked-call contract 模块，并在
  typed optional-access lowering 中复用。
- Keep contract selection and lowering on one resolution policy so future
  changes cannot silently diverge.
- 让 contract 选择与 lowering 使用同一解引用策略，避免后续修改静默漂移。

## Validation / 验证

- `cargo fmt --check`
- `cargo test checked_call_contract`
- `cargo test strict_types_reject_concrete_unsupported_optional_access_receivers`
