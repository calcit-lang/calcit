# Strict nil migration / strict nil 迁移

Calcit 的默认严格诊断把隐式缺失值视为编译错误；迁移期间可用 `--compat-types`
暂时保留旧运行时语义。`--strict-types` 显式运行严格入口检查，但不会按 Dynamic 数量判断类型正确性。

## Diagnostics / 诊断

| Code | Rejected form | Migration |
| --- | --- | --- |
| `E_LEGACY_OPTIONAL_PARAM` | `defn f (required ? optional) ...` 或等价 `fn` | 移除 `?`，把末尾参数声明成 `Option<T>`；省略实参时自动插入 `Option :none`，显式调用处写 `Option :some value` / `Option :none`。 |
| `E_PARTIAL_STRUCT_NIL_FILL` | `%{}? Struct ...` 与 `&%{}? Struct ...` | 用 `%{}` 显式提供全部字段。真正可缺失的字段声明为 `Option<T>` 并提供 `Option :none`；不要自动猜测业务默认值。 |
| `E_NIL_FOR_UNIT` | A function declared to return `Unit` whose returned expression has static type `Nil`, including legacy `;nil` | Return `&unit`, or end the body with an effect that already returns `Unit`. Intermediate nil values are not treated as the function return. |
| `E_NIL_CALLBACK_SENTINEL` | An inline `map-kv` callback has a return path that uses `nil` to drop an entry, including an `if` without an else branch | Use `filter-map-kv`; return `MapEntryDecision :keep key value` or `MapEntryDecision :drop` on every path. A `nil` nested inside the returned key/value pair remains ordinary data and is not rejected. |
| `W_MAP_KV_UNPROVEN_CONTRACT` / `E_MAP_KV_UNPROVEN_CONTRACT` | Typed project code calls legacy `map-kv`, whose heterogeneous pair and nil/enum drop protocol cannot prove output key/value types. Compatibility mode warns and exposes only `Dynamic`; strict mode rejects it. | Use `filter-map-kv` with `MapEntryDecision` when the result remains a Map or entries may be dropped. Use `map-list-kv` for `Fn(K,V)->U` collection into `List<U>`. |
| `E_IMPLICIT_NIL_BRANCH` / `W_IMPLICIT_NIL_BRANCH` | An `if` / `when` without an else branch, or a branch joining `nil` with a concrete value, used as a call or method argument or as the end of a function whose return is declared with a concrete type. Strict mode rejects it; `--compat-types` reports the warning. A `let` initializer is only noted (`W_IMPLICIT_NIL_BRANCH` in `--check-only` Notes), since the binding can still be narrowed and every non-nullable use of it is rejected at that use. Until 0.32.0 the branch is still checked as legacy `Optional<T>` | Return `Option :some value` / `Option :none` (wrap an existing nil with `nil->option`) or add an explicit else value. Statement-position branches, `nil->option` arguments, declared nullable parameters or returns, `Dynamic` returns and `calcit.core` are not reported. A branch written wholly by a macro (the `if` and its operands all come from the macro body) is skipped, but a library macro that wraps the caller's form in an else-less `if` (e.g. `defmacro maybe (c x) (quasiquote (if ~c ~x))` used as `str \|x (maybe flag \|b)`) is reported at the call site; wrap it as `nil->option (maybe flag \|b)` and handle the Option (e.g. `.unwrap-or`), or give the macro an else value. `case` / `cond` without a default raise on no match and never yield nil. |
| `E_LEGACY_OPTIONAL_SCHEMA` / `W_LEGACY_OPTIONAL_SCHEMA` | A public function schema contains legacy `Optional<T>`, which conflates a nominal API with runtime nil. `Optional<T>` and the `'Optional` public schema will be removed in 0.32.0; strict mode rejects it now, and both messages name the deletion version | Use `Option<T>` for Calcit absence, `JsNullish<T>` only at JS FFI boundaries, `Result<T,E>` for failures, or `Unit` for effects. Convert a possibly-nil value with `nil->option`. Raw core `&...` primitives and the `nil->option` parameter remain internal exceptions until the deletion. See [upgrade.md](upgrade.md#032-删除-optional-公共-schema-与旧-optional-类型) for the locate-and-migrate steps. |
| `W_LEGACY_OPTIONAL_TYPE` | Legacy `Optional<T>` in project source outside a public function schema: a `defstruct` field, a `defenum` variant payload, the `:schema` of a `def` value, a local `hint-fn` argument or return, an `assert-type` target, or a decoder target such as `parse-cirru-edn-as` / `decode-map-as`. It is an advisory listed under `Notes:` by `--check-only` and never fails a check, run or codegen; the positions stop parsing when `Optional<T>` is removed in 0.32.0 | Same replacements as above: `Option<T>` for Calcit absence, `JsNullish<T>` only at JS FFI boundaries. Raw core `&...` primitives and the `nil->option` parameter are not reported. Notes cover definitions reachable from the init / reload entries; the `--keep-going` / `--all-defs` report does not include them. |

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

## Ecosystem audit / 生态审计

删除旧入口前，按 [upgrade.md 的退场条件](upgrade.md#兼容入口的退场节奏) 对下游默认分支重新普查。

Run the audit from a Calcit checkout with clean consumer worktrees:

```bash
cargo build --bin calcit
node scripts/check-strict-nil-ecosystem.mjs \
  --calcit target/debug/calcit \
  --project respo=/path/to/respo.calcit \
  --project gen-code=/path/to/gen-code
```

The script emits revisions, remotes, clean-worktree state, raw source candidate
counts, project-only and dependency-inclusive nil summaries, and the first
whole-strict diagnostic as JSON. A dependency load warning, malformed analyzer
output, or missing repository is a hard failure, preventing an empty program
from being reported as a zero-hit success. Raw candidates still require the
context review above; generated artifacts, documentation, and quoted examples
must not be counted as executable migrations without inspection.
