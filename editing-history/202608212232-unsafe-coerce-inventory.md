# Unsafe-coerce inventory

## Summary

- Extended `calcit analyze weak-types` with the closed `unsafe-coerce` kind and
  the `explicit-unsafe` intent.
- Every executable assertion now reports its exact `code@...` Snapshot path and
  declared target schema, without classifying it as unresolved Dynamic debt.
- Bumped the JSON protocol to v4 and added `W_JS_FFI_UNCHECKED_COERCE` so
  tooling can turn the inventory into a runtime-contract test checklist.
- Kept the existing eight-metric `analyze quality` baseline stable; the new
  inventory is deliberately visible but not budgeted until a versioned baseline
  migration is introduced.

## Validation

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface` (13/13 scenarios)
- `yarn check-all`
- New debug CLI against `../respo-calcit-workflow/calcit.cirru` with
  `analyze weak-types --only unsafe-coerce --format json --summary-only`
  (4 inventory hits, JSON protocol v4)
