# Warn on duplicate imports

- Changed duplicate namespace import bindings from validation errors to recoverable warnings on stderr.
- Preserved the existing last-rule-wins behavior so duplicate aliases and repeated `:refer` entries do not stop program execution.
- Kept malformed import structures, invalid rule kinds, and invalid node shapes as errors.
- Updated CLI editing to save valid duplicate imports after displaying the warning.
- Added tests for warnings and last-rule-wins resolution, plus an isolated CLI/runtime regression proving execution continues.
- Updated import documentation to distinguish recoverable duplicate bindings from malformed rules.

## Validation

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface` (12/12 scenarios)
- `cr docs check-md docs/features/imports.md --entry calcit/test.cirru`
