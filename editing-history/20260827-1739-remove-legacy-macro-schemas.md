# Remove legacy macro schemas

## Context

Issue #480 ends the migration window for runtime-shaped macro schemas. A
`defmacro` now has one persisted and executable schema representation: the
phase-aware `Macro` contract.

## Changes

- Removed the legacy macro compatibility state, provenance metadata, staged
  warning environment switch, metrics/cache exceptions, and coverage-report
  fields.
- Made Snapshot loading reject a `defmacro` with a `Fn`, `Dynamic`, or other
  non-`Macro` schema before compilation. Errors include the Snapshot
  definition path and the final compatible Calcit version, 0.13.51.
- Made schema editing reject legacy `:kind :macro` function maps, preventing
  current tools from emitting a schema the loader will reject.
- Made preprocessing and runtime macro evaluation use only strict parameter,
  expansion, and capability checks.
- Migrated the JS integration fixture and direct temporary-Snapshot test helper
  to explicit strict macro contracts.
- Updated migration, static-analysis, macro-contract, and agent documentation.

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test -- --test-threads=1`
- `yarn compile`
- `yarn check-agent-interface`
- latest Respo and Recollect `--check-only` plus
  `analyze check-types --deps --summary-only --format json` with the local
  release binary

`yarn check-all` was also run after the agent-interface lock cleared; its
component gates are covered by the full Rust, integration-fixture, and release
checks above.
