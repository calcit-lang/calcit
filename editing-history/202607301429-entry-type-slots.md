# Entry-level type slots

Date: 2026-07-30 14:29 +0800

## Context

`with-type-slot` represented a build-wide type choice as a body wrapper. Its single-body and multi-body preprocessing paths diverged, so adding an otherwise meaningless `do` could decide whether a runtime `with-type-slot` call leaked into generated code. Respo also needs the same dispatch `Op` type across a large component call graph without passing generics through every API.

## Changes

- Added optional `:type-slots` maps to default and named entry configurations.
- Added `cr config type-slots`, `set-type-slot`, and `rm-type-slot` commands.
- Installed the selected entry's bindings before preprocessing and validated configured full type paths after module loading.
- Kept named entry configurations independent instead of inheriting default bindings.
- Made `with-type-slot` compile-time-only for both single and multiple bodies; codegen now rejects leaked forms.
- Preserved unrelated raw Snapshot EDN when a type-slot config command writes its map.
- Migrated type-slot fixtures and added round-trip, entry isolation, diagnostic, and multi-body erasure coverage.
- Updated the static-analysis, entry, project-structure, core API, and RFC documentation.

## Validation

- `cargo fmt --all`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`
- Calcit Markdown checks for the updated static-analysis and entry documentation
- Respo main/test entry regression, JS codegen, Node tests, and all Markdown code blocks with the new CLI
