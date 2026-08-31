# 守门 FFI 成员标识符冲突 / Guard FFI member identifier collisions

- 将生成标识符守门扩展到 struct fields 与 enum variants。
- 分别按 Rust/TypeScript 与 WIT 的名称规范化规则检查字段，并按 Rust 与 TypeScript/WIT 规则检查 enum variants。
- 新增 `foo-bar` / `foo_bar` 字段与 `retry-now` / `retry_now` variant 回归，确保生成前失败而非输出无法编译的代码。

- Extended generated-identifier guards to struct fields and enum variants.
- Fields are checked under Rust/TypeScript and WIT normalization, while enum variants are checked under Rust and TypeScript/WIT normalization.
- Added `foo-bar` / `foo_bar` field and `retry-now` / `retry_now` variant regressions so generation fails before emitting uncompilable code.
