# Tailor FFI ambiguity guidance / 按限定状态细化 FFI 歧义指引

- Tailor duplicate declaration guidance based on whether the referenced type ID
  is already namespace-qualified.
- Qualified references now point directly to removing duplicate local sources;
  unqualified references first recommend qualification, then deduplication.
- Cover both diagnostic branches with regression assertions.

- 根据被引用类型 ID 是否已包含 namespace，分别生成重复声明修复建议。
- 已限定引用直接要求移除重复本地声明源；未限定引用先建议补全 namespace，再
  在仍有歧义时去重。
- 为两条诊断分支分别添加回归断言。

## Validation / 验证

- `cargo fmt --check`
- `cargo test ffi_interface_ir::tests:: -- --test-threads=1`
