# Strict nil migration / strict nil 迁移

`--strict-types` treats implicit absence as a compiler error while ordinary mode
keeps the legacy runtime behavior during migration.

`--strict-types` 把隐式缺失值视为编译错误；迁移期间，普通模式仍保留旧运行时语义。

## Diagnostics / 诊断

| Code | Rejected form | Migration |
| --- | --- | --- |
| `E_LEGACY_OPTIONAL_PARAM` | `defn f (required ? optional) ...` or the equivalent `fn` form | Remove `?`, declare trailing parameters as `Option<T>`, and rely on trailing omission to insert `%none`; use `%some value` / `%none` at explicit call sites. |
| `E_PARTIAL_STRUCT_NIL_FILL` | `%{}? Struct ...` and `&%{}? Struct ...` | Use `%{}` with every field present. Change genuinely absent fields to `Option<T>` and provide `%none`. Do not infer business defaults automatically. |
| `E_NIL_FOR_UNIT` | A function declared to return `Unit` whose returned expression has static type `Nil`, including legacy `;nil` | Return `&unit`, or end the body with an effect that already returns `Unit`. Intermediate nil values are not treated as the function return. |
| `E_NIL_CALLBACK_SENTINEL` | An inline `map-kv` callback has a return path that uses `nil` to drop an entry, including an `if` without an else branch | Use `filter-map-kv`; return `MapEntryDecision :keep key value` or `MapEntryDecision :drop` on every path. A `nil` nested inside the returned key/value pair remains ordinary data and is not rejected. |

The runtime `nil` value, Cirru EDN nil, and explicit untyped/FFI boundaries are
not removed. The strict errors only close constructs that silently manufacture
nil inside typed code.

运行时 `nil`、Cirru EDN nil 以及明确的 untyped/FFI 边界不会删除；上述错误只阻断
typed code 中静默制造 nil 的语法。

`E_NIL_CALLBACK_SENTINEL` is intentionally conservative. It checks only
structurally visible return paths of inline `fn` / `defn` callbacks. It does not
guess through named callbacks or reject `[] key nil`, where nil is the mapped
value rather than a drop sentinel.

`E_NIL_CALLBACK_SENTINEL` 采用保守检查：只分析内联 `fn` / `defn` 回调中结构上
可见的返回路径，不猜测具名回调；`[] key nil` 中的 nil 是映射后的值，不是丢弃
sentinel，因此不会被拒绝。

## Initial executable census / 首轮可执行调用点盘点

The 2026-09-02 workspace census separated executable source from documentation,
quoted examples, vendored copies, and obsolete `compact.cirru` snapshots.

- Calcit keeps four executable `%{}?` compatibility tests: one in
  `calcit/test-edn.cirru` and three in `calcit/test-struct.cirru`. The core macro,
  its quoted example, and API metadata are definitions/documentation rather than
  application call sites.
- Calcit's `fn` placeholder lowering still generates `(? % %2)` when a `%2`
  placeholder is present. The matching `test-macro.cirru` forms exercise that
  generated compatibility behavior.
- Current source snapshots with direct `?` parameters were found in `std`
  (`nanoid!`, `rand`, `rand-int`), `lilac` (seven optional constructor helpers),
  `editor` (two callback parameters), and `gen-code` (one callback parameter).
- `reacher`, `explain-ternary-tree`, `std-old`, and `.compact-inc.cirru` hits are
  legacy snapshot inputs and must first follow the `calcit.cirru` source-model
  migration; they are not migration templates for new code.
- No direct `?` or `%{}?` call was found in the checked Respo or msg-buffer
  workspaces. Caltrop retains its own legacy macro and four test calls; `apis`
  contains documentation metadata only.

首轮结果说明真正需要迁移的重点是 `fn` 的 `%2` placeholder lowering、std/lilac 的
公共可选参数，以及 editor/gen-code 的宿主回调；Calcit 内的 `%{}?` 调用目前都是兼容
测试，不应误当成推荐写法。

Reproduce the text-level census with:

```bash
rg -n -F --glob '*.cirru' --glob '!**/.git/**' -- '%{}?' /path/to/calcit-lang
rg -n -F --glob '*.cirru' --glob '!**/.git/**' -- '(? ' /path/to/calcit-lang
```

Each hit still needs AST/context review: quoted code and macro definitions are
not ordinary executable call sites.
