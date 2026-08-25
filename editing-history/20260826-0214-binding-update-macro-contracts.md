# Binding and update macro contracts

## Changes

- Migrated `let-destruct`, `let-sugar`, `let[]`, `let{}`, and `loop` to phase-aware binding and body contracts.
- Migrated `struct-with`, `swap!`, and `&doseq` with concrete struct/ref/callable inputs and struct/Unit outputs where proven.
- Added `assert-type pair 'List` inside `let-sugar` so strict macro-body analysis retains the list evidence already guaranteed by its runtime validation.
- Added exact Snapshot assertions for all contract fields.

## Validation

- Reachable strict expansions: `2364/2432` -> `2379/2432`; legacy bypasses: `68` -> `53`.
- Full `cargo fmt`, Clippy, compile, Rust tests, `yarn check-all`, and Agent interface gates passed.
- Latest Respo `be8141e`, Recollect `6c235d0`, and js-ffi `25869b6` regressions passed.
