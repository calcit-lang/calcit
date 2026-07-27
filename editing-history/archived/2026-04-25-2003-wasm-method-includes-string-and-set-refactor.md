# 202604252003 — WASM suite extended: .includes? string branch + uncommitted refactor consolidation

## Background

In the previous session a multi-module WASM suite entry (`calcit/test-wasm-suite.cirru`) was added
to gradually exercise more of the existing `calcit/test-*.cirru` programs against the WASM codegen.
At that point a sizable in-progress refactor in `src/codegen/emit_wasm/{heap,hof,lists,runtime,sets}.rs`
was left uncommitted alongside, and `test-string.cirru` was deferred because folding it into the
suite triggered an `unreachable` trap.

## Findings this round

When folding `test-string.main` into `test-wasm-suite.main/main!`, the runtime trap reproduced. By
calling individual exports (new helper `scripts/test-wasm-call.mjs`) it was clear that:

1. `(.includes? "abc" "abc")` returned `0` (false) instead of `1`. Root cause:
   `methods::emit_method_includes` only handled `list/map/set` receivers; for any other type it
   emitted `f64_const(0.0)` as the fallback. Strings therefore never reached
   `__rt_str_find_index`.
2. After fixing #1, `test-includes` still trapped — the assertions
   `assert= true $ starts-with? :a/b :a/` and `assert= true $ starts-with? :a/b |a/` pass
   tags as arguments. Tags in WASM are encoded as small `f64` values equal to their tag-index id,
   so `__rt_str_starts_with` reads them as bogus heap pointers and silently returns 0. Adding tag→
   string conversion at runtime is a follow-up.
3. `test-bitwise` traps on `&number:display-by` (radix formatting) which is currently stubbed to
   return `0`.

Because #2 and #3 require additional runtime helpers, `test-string.cirru` is left out of the suite
for this commit. The `.includes?` regression is fixed in isolation; it also benefits any other
program that calls `.includes?` on a string.

## Changes in this commit

- `src/codegen/emit_wasm/methods.rs`: extend `emit_method_includes` with a string branch
  (`emit_str_includes_from_local`) that calls `__rt_str_find_index >= 0`.
- `scripts/test-wasm-call.mjs`: minimal helper to invoke a single named export against the latest
  `js-out/program.wasm`, useful for bisecting which assertion in a `main!` traps.

The commit also folds in the previously-uncommitted refactor that landed in
`src/codegen/emit_wasm/{heap,hof,lists,runtime,sets}.rs`:

- `sets.rs`: `emit_set_find_structural` replaces the pointer-only `__rt_set_find_elem` for
  `difference`/`intersection`/set-equality so identical-by-content elements are treated as equal.
- `runtime.rs`: new `__rt_hash_list_or_set(ptr) -> i32` (XOR-based content hash) plus a couple of
  new helper-function indices.
- `hof.rs`: `emit_copy_type_tag` so `map`/`filter` preserve the receiver tag (set→set, list→list).
- `lists.rs`: `range start end step` 3-arg form.
- `heap.rs`: smarter `emit_hash_proc` that detects heap pointers at runtime and dispatches to the
  list/set hash helper. The dead `emit_hash_expr_i32`/`emit_hash_mix` static helpers are removed.
- `emit_wasm.rs`: `#[allow(private_interfaces)]` on `emit_equals_core` and
  `emit_equals_core_shallow` (both already returned a `pub(super)`-only opaque local index but are
  used from sibling modules through the `super::*` re-export).

## Verification

```bash
bash scripts/cargo-with-sdk.sh clippy --bin cr-wasm -- -D warnings   # clean
bash scripts/cargo-with-sdk.sh build  --bin cr-wasm --release        # ok
bash scripts/test-wasm-suite-extended.sh                             # PASS
bash scripts/test-wasm-suite.sh                                      # 6/10 (unchanged)
```

`scripts/test-wasm-suite.sh` skips `test-fn`, `test-string`, `test-tuple`, `test-list` for the
same reasons documented previously; that is independent of this commit.

## Follow-ups

1. Tag → string conversion at runtime so string procs accept tags (unblocks `test-string`).
2. `&number:display-by` radix formatting (unblocks the last 3 assertions in `test-bitwise`).
3. The other test-string skips (`&str:replace`, `parse-float`, `blank?`, `trim`,
   `get-char-code`, `format-to-cirru`, `&cirru-quote:to-list`).
