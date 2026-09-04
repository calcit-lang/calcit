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
| `E_LEGACY_OPTIONAL_SCHEMA` | A public function schema contains legacy `Optional<T>`, which conflates a nominal API with runtime nil | Use `Option<T>` for Calcit absence, `JsNullish<T>` only at JS FFI boundaries, `Result<T,E>` for failures, or `Unit` for effects. Raw core `&...` primitives and the `optionally` bridge remain internal compatibility boundaries. |

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

## Ecosystem evidence for 0.13.78 / 0.13.78 生态证据

The 2026-09-04 UTC follow-up audit used Calcit 0.13.77 main at
`5915ccf38ef3b57ea26897599646eefdbc9e3ba6`. It intentionally reports the nil
slice separately from the first failure of the whole strict preflight: a
project can have no project-local nil debt while still being blocked by a
Dynamic or generic-contract diagnostic.

2026-09-04 UTC 的后续审计基于 Calcit 0.13.77 main
`5915ccf38ef3b57ea26897599646eefdbc9e3ba6`。审计刻意把 nil 专项结果与完整 strict
预检的首个失败分开：项目可能没有自身 nil 债务，但仍被 Dynamic 或泛型契约诊断阻断。

| Consumer | Frozen revision | Project nil audit | Dependency-inclusive nil audit | Legacy syntax/schema candidates | First whole-strict blocker | Migration owner |
| --- | --- | --- | --- | --- | --- | --- |
| Respo | `d106b38a85d7bc0e10c6d621235843f37e69c6e4` | 49 unresolved hits, 28 definitions, 14 namespaces | same 49 hits | no `(? ...)` or `%{}?`; public legacy `Optional<T>` fields and one `map-kv` nil-drop callback remain | `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA` at `respo.main/f%` | [Respo/respo.calcit#131](https://github.com/Respo/respo.calcit/issues/131) |
| gen-code | `a112db30414a865089054edcfb4b73a557433d0e` | 0 hits | 122 unresolved hits from dependencies, 61 definitions, 33 namespaces | one executable `fn (? chunk)` stream callback; no `%{}?`, public `Optional<T>`, or `map-kv` candidate | `E_ERASED_GENERIC_RELATION` in `gen-code.comp.container/comp-container` | [calcit-lang/gen-code#13](https://github.com/calcit-lang/gen-code/issues/13) |

The Respo review identifies three distinct migrations rather than treating all
explicit runtime nil values alike:

- `respo.util.list/pick-event` uses `map-kv` with a visible nil drop sentinel;
  migrate it to `filter-map-kv` and `MapEntryDecision`.
- `Component`, `DomProps`, `Element`, and `RespoEvent` expose legacy
  `Optional<T>` fields. Their DOM/runtime meaning must be reviewed before
  choosing Calcit `Option<T>` or a host-nullish boundary.
- The remaining 49 explicit nil forms need per-definition classification.
  They are visible debt, but their presence alone does not prove that they use
  one of the implicit compatibility constructs rejected by the five strict nil
  diagnostics.

Respo 的审阅把迁移分成三类，不把所有显式 runtime nil 混为一谈：`pick-event` 的
nil-drop sentinel 迁移到 `filter-map-kv`；四个公开数据结构中的 legacy
`Optional<T>` 按 DOM/runtime 语义选择 `Option<T>` 或 host-nullish 边界；其余 49 个
显式 nil 逐定义分类，不能仅凭出现 `nil` 就误判为五种隐式兼容构造之一。

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
