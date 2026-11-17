# 2026-11-17 WASM/WASI `&section-by-loop` Specialization & Collection Helpers (`#2853`)

## Context & Decision Basis

Issue `#2853` unlocks `&section-by-loop`, `chunks`, `keys-non-nil`, `merge`, `merge-non-nil`, single-argument `sort` (`&list:sort`), `&enum:definition`, and statically proven nominal `Struct` `update` on `wasm` and `wasi`:

1. **Generic return type substitution & empty collection parameter widening (`&section-by-loop`, `chunks`, `merge`, `merge-non-nil`)**:
   - In `infer_wasm_callable_result` (`src/wasm/emit_wasm.rs`), calling a generic function such as `(drop xs n)` or `(take xs n)` inside a specialized loop body previously returned the uninstantiated `List('T)` from `infer_static_type_from_expr(expr)` instead of substituting `'T` from the specialized `locals` map.
   - In `resolve_static_specialization_types`, passing an empty list literal `([])` (`List(Never)` / `Tuple([])`) or a fixed-length tuple to a parameter declared as `List('T)` or `List(List('T))` kept the parameter's loop-carried type at `List(Never)` / `Tuple(...)`, causing `validate_recur_evidence` to reject the `recur` back-edge `(append acc (take xs n))`.
   - Decision: substitute `signature.return_type` with `bindings` inferred from `locals` in `infer_wasm_callable_result` (replacing any remaining unbound type variables from empty collections with `Never`), and widen collection/tuple arguments in `resolve_static_specialization_types` to `expected.substitute_type_vars(&bindings)` when proven and non-dynamic.

2. **`keys-non-nil` WASM lowering (`keys-non-nil#omits-nil-map-values`)**:
   - Added `emit_map_keys_non_nil` in `src/wasm/emit_wasm/maps.rs` (mirroring `emit_map_merge_non_nil`), linearizing the map via `__rt_map_linearize` and collecting keys whose values are non-nil (`!= 0.0`) into a result set.

3. **Default-comparator `sort` (`sort#orders-list-with-default-comparator`, `sort#sorts-nan-after-numbers`)**:
   - Updated `emit_list_sort` in `src/wasm/emit_wasm/lists.rs` to accept 1 argument when the list element type is statically proven to be a comparable scalar or string (`Number`, numeric refinement, `String`, `Tag`, `Bool`, `Nil`, `Unit`, `Never`), dispatching to `FoldlCallKind::Proc(CalcitProc::NativeCompare)`.

4. **`&enum:definition` and proven `Struct` `update` (`&enum:definition#nominal-definition-evidence`, `update#updates-nominal-struct-fields`)**:
   - Added `resolve_enum_ref` and `emit_enum_definition` in `src/wasm/emit_wasm/structs.rs` for statically proven nominal and anonymous enums.
   - Updated `emit_update` in `src/wasm/emit_wasm/hof.rs` to lower `update` on statically proven nominal `Struct` receivers with literal tag keys via `emit_struct_update_from_locals`.

## Validation Commands

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build --bin calcit
WASMTIME_CLI=/opt/homebrew/bin/wasmtime node scripts/run-core-tests.mjs --backend wasm,wasi \
  --target 'calcit.core/chunks#splits-list-into-fixed-sections' \
  --target 'calcit.core/chunks#keeps-short-lists-whole' \
  --target 'calcit.core/chunks#empty-list-has-no-sections' \
  --target 'calcit.core/keys-non-nil#omits-nil-map-values' \
  --target 'calcit.core/merge#combines-maps-left-to-right' \
  --target 'calcit.core/merge-non-nil#keeps-existing-values-for-nil' \
  --target 'calcit.core/sort#orders-list-with-default-comparator' \
  --target 'calcit.core/sort#sorts-nan-after-numbers' \
  --target 'calcit.core/&enum:definition#nominal-definition-evidence' \
  --target 'calcit.core/update#updates-nominal-struct-fields' \
  --report-unexpected-pass
```
