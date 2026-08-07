# Preserve targeted schema edits

## Scope

- `cr edit schema` now updates the selected `CodeEntry` schema through the original EDN snapshot tree instead of re-rendering the entire typed `Snapshot`.
- Unrelated schema fields, including legacy `:optional` and `:any` forms, remain unchanged by targeted schema updates and clears.

## Validation

- Added a regression fixture covering both schema update and clear operations, preserving an unrelated legacy function schema and the snapshot shebang.
- Verified with the full Rust test suite, strict Clippy, `yarn compile`, and `yarn check-agent-interface`.
