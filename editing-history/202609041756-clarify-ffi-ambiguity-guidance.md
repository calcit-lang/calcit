# Clarify FFI ambiguity guidance / 澄清 FFI 歧义指引

- Correct the duplicate nominal declaration diagnostic so it remains useful
  when the callable schema already uses a namespace-qualified ID.
- Tell authors to keep one local declaration source per nominal type, while
  retaining namespace-qualified IDs as the general FFI contract rule.
- Add a regression assertion for the actionable suggestion text.

- 修正重复 nominal declaration 的诊断建议，使 callable schema 已使用 namespace
  限定 ID 时仍然可操作。
- 明确每个 nominal 类型只保留一个本地声明源，同时继续要求 FFI contract 使用
  namespace-qualified ID。
- 为建议文案添加回归断言。

## Validation / 验证

- `cargo fmt --check`
- `cargo test ffi_interface_ir::tests:: -- --test-threads=1`
