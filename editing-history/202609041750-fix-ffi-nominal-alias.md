# Resolve FFI declarations by nominal name / 按 nominal 名称解析 FFI 声明

- Index local Interface IR Struct/Enum declarations by the resolved nominal
  name instead of the Snapshot binding that stores the definition.
- Support the standard top-level `Foo0 = defstruct Foo` plus
  `Foo = impl-traits Foo0 FooImpl` pattern used by typed modules.
- Reject duplicate nominal declarations as deterministic ambiguity diagnostics
  that name the conflicting source bindings.
- Add unit regressions and validate the fix against the real calcit-regex
  `Regex0`/`Regex` consumer from calcit#634.

- Interface IR 的本地 Struct/Enum declaration 改为按解析后的 nominal 名称建索引，
  不再误用保存定义的 Snapshot binding 名称。
- 支持类型化模块常用的顶层 `Foo0 = defstruct Foo` 加
  `Foo = impl-traits Foo0 FooImpl` 模式。
- 多个 binding 声明同一 nominal 类型时，以包含冲突 source binding 的稳定歧义
  diagnostic 拒绝，不随机选择。
- 添加单元回归，并使用 calcit#634 的真实 calcit-regex `Regex0`/`Regex` 消费者验证。

## Validation / 验证

- `cargo test ffi_interface_ir::tests::` (21 passed)
- rebuilt `cargo build --bin calcit`
- real calcit-regex `ffi export`: no declaration-missing diagnostic; exact
  `declarations.regex.core/Regex.fields.0.type` unsupported resource boundary
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -- --test-threads=1` (all passed)
- `yarn check-all` (all passed)
