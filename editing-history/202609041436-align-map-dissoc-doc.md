# Align map dissoc documentation / 对齐 map dissoc 文档

Copilot review on #626 noted that the bundled `&map:dissoc` docstring still described keys as `any` after its schema and Rust metadata were tightened to one `K` type. The docstring now states `Map<K,V>`, `K`, variadic `K`, and `Map<K,V>` return explicitly.

#626 的 Copilot review 指出：`&map:dissoc` schema 与 Rust metadata 已收紧为统一的 `K` 类型，但 bundled docstring 仍把 key 描述为 `any`。文档现在明确写出 `Map<K,V>`、`K`、variadic `K` 与 `Map<K,V>` 返回类型。

The same review included a suppressed readability suggestion for the direct Struct receiver gate. Parentheses now make the intentional `TypeRef && resolves-to-Struct` grouping explicit without changing behavior.

同一次 review 还包含一条 suppressed 可读性建议；直接 Struct receiver gate 现在用括号明确表达 `TypeRef && resolves-to-Struct` 的分组，行为不变。
