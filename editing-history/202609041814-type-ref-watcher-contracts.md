# Type Ref watcher contracts / 收紧 Ref watcher 类型契约

- Align native `add-watch` with the public `Ref<T>, Tag, Fn(T, T) -> Unit`
  contract instead of erasing all three arguments to Dynamic.
- Require a Tag key for both `add-watch` and `remove-watch`; remove the
  unrelated public key generic from `remove-watch`.
- Substitute already-bound proc type variables before diagnostics so callback
  errors name the concrete Ref payload type.
- Migrate the bundled watcher callback to explicit Unit, add negative
  key/callback fixtures, and document the contract.

- 将 native `add-watch` 与公开的 `Ref<T>, Tag, Fn(T, T) -> Unit` 契约对齐，
  不再把三个参数都擦除为 Dynamic。
- `add-watch` 与 `remove-watch` 均要求 Tag key，并移除 `remove-watch` 无关的
  public key 泛型。
- diagnostic 生成前替换已绑定的 proc 类型变量，使 callback 错误展示具体 Ref
  payload 类型。
- 将 bundled watcher callback 迁移为显式 Unit，增加 key/callback 负向 fixture，
  并补充文档。

## Validation / 验证

- targeted proc-signature and type-fail regression tests
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -- --test-threads=1` (all passed)
- `yarn check-all` (all passed; core quality inventory unchanged at
  278 schema-Dynamic / 184 unresolved / 134 not-full)
