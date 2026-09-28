# Seeded List fold

## Decision

Expose `fold` and List `.fold` for seeded, left-to-right accumulation. The result type `U` is independent of the element type `T`; an empty List returns its seed. Keep `reduce`, `.reduce`, and `foldl` callable for compatibility. List `.reduce` and `.fold` deliberately resolve to the same `calcit.core/fold` definition, so a narrowly proven method migration can preserve behavior without changing callback order or count.

## Type boundary

The core function schema is `(List<T>, U, Fn(U,T)->U) -> U`. Preprocessing must specialize an inline reducer after the receiver and seed have been processed, before assigning callback parameter types. Final argument checking alone is too late: it detects a mismatch after the anonymous function has already captured unbound generic hints. The same preprocessing refinement applies to existing `reduce` and `foldl` calls.

## Migration boundary

`core-list-fold-v1` is opt-in, outside the version preset. It rewrites only complete `.reduce` calls on a concrete List when old and new static method contracts are proven and point to the same definition. Quoted data, known non-List receivers, first-class method values, macros, and unstable source contexts are not automatically changed. The rule operates on definition code, not attached tests or examples. Existing `calcit fix` preview, revision guard, staged validation, and idempotence apply.

## Validation

The `calcit.core/fold` definition carries Calcit tests for order, empty seed, heterogeneous accumulator, and both prefix and method forms. The CLI integration test checks migration, preserved execution, revision rejection, and idempotence. Run `cargo test`, `yarn compile`, `yarn check-all`, and the definition test with `--require-match` before merging.
