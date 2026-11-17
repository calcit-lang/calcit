# 2026-11-17 WASM/WASI Pattern & Edge Parity (`#2852`)

## Context & Decision Basis

Issue `#2852` tracks the remaining `parity:` exclusions in `scripts/core-tests-exclusions.cirru` across `wasm` and `wasi`:

1. **`emit_match` variant payload arity check (`Option#matches-payloads-and-rejects-arity-mismatch`)**:
   - Previously, `emit_match` in `src/wasm/emit_wasm.rs` only compared the enum tag ID at byte offset `8` (`tag_local == tag_id`), ignoring the payload count stored at byte offset `0`. A pattern `(:a x)` (`1` payload) therefore matched `(:: :a 1 2)` (`2` payloads) instead of falling through to the next branch or wildcard.
   - Decision: load `count_local` at byte offset `0` in `emit_match` and require both `tag_local == tag_id` and `count_local == (pat_xs.len() - 1) as f64`.

2. **`emit_enum_new` runtime boolean / tag first argument (`foldr-shortcut#returns-shortcut-accumulator`)**:
   - `foldl-shortcut` and `foldr-shortcut` expect callbacks returning `(:: should-stop? acc)`. Previously `emit_enum_new` only accepted literal `Calcit::Tag` or `Calcit::Bool` as the first argument of `::`, rejecting runtime boolean expressions such as `(&= x 2)`.
   - Decision: in `emit_enum_new` (`src/wasm/emit_wasm/structs.rs`), accept runtime expressions proven to have static type `Bool` or `Tag` via `infer_wasm_static_type(ctx, other)` and emit their value into offset `8`, while continuing to reject unsupported non-tag expressions (such as runtime quoted symbols).

3. **`apply` literal list lowering outside `calcit.core` (`apply#spreads-list-arguments`)**:
   - In `src/runner/preprocess/mod.rs`, `apply` literal list lowering checked `resolve_type_value(&args[0], scope_types)` before `args[0]` was preprocessed from a `Calcit::Symbol` into a `Calcit::Import`. Inside `calcit.core` (`wasi` replay), `infer_definition_value_type(&info.at_ns, "+")` found `calcit.core/+`, whereas outside `calcit.core` (`wasm` replay in `calcit.core-test-replay`), `resolve_type_value` returned `None`.
   - Decision: when `resolve_type_value(&args[0], scope_types)` returns `None` at the `apply` literal-list check, preprocess `args[0]` with a silent warning buffer to resolve imports before querying `resolve_type_value`.

4. **Static disjoint kind short-circuit in `emit_equals` (`case#literal-patterns-use-match`)**:
   - Literal `match` lowers to an `if` chain of `(&= match_v pattern)` comparisons. In the untagged `f64` WASM scalar ABI, `1` (`Number`) and `true` (`Bool`) both use `1.0`, while `0` (`Number`), `false` (`Bool`), and `nil` (`Nil`) all use `0.0`. When an inline closure such as `pick` is specialized per call site, `match_v` carries the caller's static type in `ctx.local_types`, but `emit_equals` previously ignored disjoint static types and compared raw `f64` values.
   - Decision: in `emit_equals`, when both operands have proven disjoint static value kinds (`proven_disjoint_value_kind`), evaluate both operands for side effects, drop them, and emit `0.0` (`false`).

5. **`__rt_str_escape` implementation & anonymous enum string formatting (`&str:escape#escapes-quoted-strings`, `str#omits-nil-and-renders-enums`)**:
   - `build_rt_str_escape` in `src/wasm/emit_wasm/runtime.rs` was previously a stub returning its input unchanged. Implemented two-pass escaping (`\n`, `\r`, `\t`, `"`, `'`, `\`) wrapped in double quotes.
   - `try_format_enum_literal` in `src/wasm/emit_wasm.rs` formatted `::` literals with `(:: ...)` instead of the canonical `(%:: _ ...)` used by `Display for Calcit`. Updated to `(%:: _ ...)`.

6. **Set literal deduplication (`count#empty-and-collection-counts`)**:
   - `emit_set_new` in `src/wasm/emit_wasm/sets.rs` previously stored all literal arguments without deduplication, causing `count (#{} 1 1 2)` to return `3`. Updated `emit_set_new` to deduplicate elements for `count > 1` using `emit_set_find_structural`.

7. **Nested list failure path in `try-parse-cirru-edn-as` (`try-parse-cirru-edn-as#nested-list-failure-path`)**:
   - `emit_parse_scalar_list` in `src/wasm/emit_wasm/edn_parse.rs` now propagates the failing `[<index>]` path segment into `error_path` and appends ` at $<error_path>` to the error message when non-empty.

8. **Value-alias proc dispatch & exclusion hygiene (`read-dir`, `read-file`, `contains-in?`)**:
   - In `emit_call_expr`, `Calcit::Import` and `Calcit::Symbol` now check `ctx.value_imports` for `Calcit::Proc` aliases (`calcit.core/read-dir` -> `&read-dir`, `calcit.core/read-file` -> `&read-file`) before falling back to `fn_index`, reporting the exact `unsupported proc in WASM` diagnostic.
   - Removed 19 passing exclusions (`10` under `:wasm`, `9` under `:wasi`) and reclassified `contains-in?`, `read-dir`, and `read-file` to `unsupported:`.

## Validation Commands

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build --bin calcit
WASMTIME_CLI=/opt/homebrew/bin/wasmtime node scripts/run-core-tests.mjs --backend wasm,wasi \
  --target 'calcit.core/Option#matches-payloads-and-rejects-arity-mismatch' \
  --target 'calcit.core/case#literal-patterns-use-match' \
  --target 'calcit.core/struct-match#checked-dynamic-input-preserves-nominal-identity' \
  --target 'calcit.core/foldr-shortcut#returns-shortcut-accumulator' \
  --target 'calcit.core/apply#spreads-list-arguments' \
  --target 'calcit.core/&map:destruct#exposes-pairs-and-folds' \
  --target 'calcit.core/&str:escape#escapes-quoted-strings' \
  --target 'calcit.core/str#omits-nil-and-renders-enums' \
  --target 'calcit.core/count#empty-and-collection-counts' \
  --target 'calcit.core/try-parse-cirru-edn-as#nested-list-failure-path' \
  --target 'calcit.core/contains-in?#traverses-maps-lists-tags-and-enums' \
  --target 'calcit.core/read-dir#preserves-one-and-two-argument-directory-reads' \
  --target 'calcit.core/read-file#preserves-file-read-alias' \
  --report-unexpected-pass
```
