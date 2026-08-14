# 2026-08-14 限定名类型应用

- 未限定泛型语境中，带 `/` 的 TypeRef 仍是名义类型，不应在 `::` 类型应用时退化为普通值。
- `(:: 'app.schema/Box 'String)` 现在保留 `app.schema/Box` 及其类型参数；保留现有 TypeVar 行为。
- 添加针对无 scope 限定名类型应用的回归测试，覆盖 `assert-type` 使用的同一类型解析路径。
