# Keep whole-`Dynamic` macro roots strict

- Prevented a nested function hint from exempting a `defmacro` with a whole-`Dynamic` root schema in strict mode.
- Added focused regression coverage for `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA`.
- Aligned migration documentation around function hints and programmatically supplied macros.

# 保持 whole-`Dynamic` 宏根约束

- 防止宏体内嵌套的函数 hint 让 whole-`Dynamic` 根 schema 的 `defmacro` 绕过 strict mode。
- 为 `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA` 增加聚焦回归覆盖。
- 统一函数 hint 与 programmatically supplied macro 的迁移文档术语。
