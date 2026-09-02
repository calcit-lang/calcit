# Static receiver-method lowering and generic payload preservation

2026-09-02 14:30 CST

- Established the source/ABI boundary for receiver-first APIs: application code writes lowercase `.name`, while preprocessing selects lowercase internal `&scope:name` callables from the receiver type and impl table. Internal scoped names remain a compiler/core/backend ABI rather than a source migration target.
- Specialized typed postfix method calls before code generation. Built-in `Map<K,V> .keys` and `Ref<T> .deref` now reach JS/native/WASM lowering through their direct internal callable heads instead of `invoke-method`.
- Preferred declared function schemas when compiled body inference only reports `Dynamic`, propagated generic bindings through fixed and rest arguments, and preserved initializer payloads as `Ref<T>` across `defatom`.
- Tightened the core contracts for `deref`, `keys`, `merge`, `defatom`, and `&list:nth`. `&map:keys` now consistently returns `Set<K>` in native, JS, and WASM.
- Migrated fixtures away from dynamic compatibility behavior: Struct key access goes through `.to-map .keys`, Struct updates use `struct-with`, homogeneous Map merge keeps a single key/value type, and `nil` no longer acts as an empty Map.
- Added exact lowered-AST, generated-JS, runtime identity, and WASM checks. The bundled-core weak-type baseline improved from `schemaDynamic=291`, `unresolved=197`, `typeNotFull=141` to `284`, `190`, and `138`, with no increases in nil, unsafe coercion, or deprecated calls.
- Custom nominal impls are selected from typed impl tables, but anonymous implementation functions still use the generic invocation representation in generated code. Converting those implementations to stable callable symbols remains a follow-up rather than being hidden behind handwritten internal names.
- Follow-up CI documentation validation migrated the remaining Struct examples away from `keys`/`merge` compatibility behavior, corrected `struct-definition` to `Option<StructDef>`, and kept all 67 checked Markdown files warning-free under the stricter schemas.
- Review follow-up kept `&list:nth -> T` sound by making invalid indexes fail consistently instead of returning/loading nil, registered the Ref impl table for JS dynamic fallback, and strengthened the generated-output negative assertion.

验证覆盖 Rust tests/clippy、235 个 core unit tests、native/JS/IR/WASM 全流程、agent interface、静态方法生成检查以及 literal-path/typed-method 性能 smoke tests。
