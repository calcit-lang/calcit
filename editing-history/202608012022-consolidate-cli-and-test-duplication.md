# Consolidate CLI and test duplication

## Summary

- Introduced a typed `TreeOperation` model and shared cursor path transforms so edit, tree, and cursor commands use one implementation for structural mutations.
- Reused one staged-file abstraction for atomic cursor writes and edit transactions, including permission preservation, sync, commit, and cleanup behavior.
- Consolidated query result collection/rendering and type-coverage scope traversal without changing their JSON envelopes or diagnostics.
- Added one test-only temporary project fixture shared by edit, cursor, and query tests.
- Replaced eight identical Calcit test-local `log-title` definitions with `util.core/log-title`, keeping each standalone snapshot loadable through its module configuration.
- Removed the misspelled `test-gynienic.cirru` snapshot because the typed `test-hygienic.cirru` suite is a strict superset, and removed the identical `test-cond` runtime case from the macro suite.
- Made the remaining `case-default` macroexpansion assertion deterministic by resetting the gensym counter before checking the generated symbol.

## Review notes

- Kept repeated `main!` and `reload!` definitions because they are snapshot entry points rather than interchangeable helpers.
- Kept similarly named macro, native, JS, and WASM tests when they exercise different expansion or backend behavior.
- The resulting tracked diff removes more code than it adds while retaining the command boundaries and test coverage.

## Validation

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface` (12/12 scenarios)
- `yarn check-all` (native, JS, and WASM)
- Standalone native runs for the updated Calcit snapshots, including `test-cond.cirru`, `test-macro.cirru`, and `test-hygienic.cirru`
- Real-project Respo regression with JSON definition query and summary-only type-coverage analysis
