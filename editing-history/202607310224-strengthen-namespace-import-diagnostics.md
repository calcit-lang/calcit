# Strengthen namespace import diagnostics

## Summary

- Replaced namespace/import parser panics and silent fallbacks with contextual errors for malformed namespace forms, short rules, invalid rule kinds, and invalid node shapes.
- Added one shared import-rule validator for program loading and `cr edit add-import` / `cr edit imports`, so invalid edits are rejected before the snapshot is saved.
- Detect duplicate local bindings across rules and repeated `:refer` definitions within one rule instead of silently overwriting the earlier import.
- Preserved legacy `(:ns namespace)` compatibility while validating supported `:require` structures.
- Clarified that `call-graph --show-unused` reports definitions unreachable from the selected entry, not proven dead code or unused import declarations.
- Kept unused-definition analysis independent from `--max-depth`, preventing display truncation from producing false dead-code reports.
- Documented validation behavior, atomic edits, dead-code analysis limits, and the relevant CLI workflow.

## Validation

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface` (12/12 scenarios)
- `cr docs check-md docs/features/imports.md --entry calcit/test.cirru` (4/4 blocks)
- Isolated CLI regressions confirming malformed and duplicate imports are rejected without changing the snapshot.
- Compared `call-graph --show-unused` with unlimited depth and `--max-depth 1`; unused-definition results are identical.
