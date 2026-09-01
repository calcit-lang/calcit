# Type map-indexed and watch callbacks

## Context

The first bundled-core cleanup batch targets contracts whose generic
relationships are determined by their runtime behavior. APIs such as generic
`get`, `filter`, and host-extensible `deref` remain deferred because their
result relationship cannot yet be expressed honestly by one ordinary function
schema.

## Changes

- Strengthen `map-indexed` to
  `List<T> × Fn(Number,T)->U -> List<U>`.
- Strengthen `add-watch` callbacks to `Fn(T,T)->Unit` for `Ref<T>`.
- Lower the reviewed core quality baseline after the cleanup.

## Result

- Unresolved schema-Dynamic positions: 305 → 303.
- Definitions without full type coverage: 145 → 144.

## Validation

- `analyze check-examples --ns calcit.core --def map-indexed`: 2/2.
- `test calcit.core/map-indexed`: 1/1.
- Core `check-types`, `weak-types`, and baseline quality reports.
- Full Rust, native, JS, IR, WASM, and Agent-interface gates before push.
