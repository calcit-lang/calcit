# Legacy macro schema inventory

## Summary

- Added the stable `W_LEGACY_MACRO_SCHEMA` diagnostic to `analyze check-types`.
- Preserved whether a compatibility macro came from an old runtime `Fn` schema or a whole `Dynamic` schema.
- Added check-types JSON schema v2 fields for the aggregate count and per-definition source, legacy signature, Snapshot path, and migration guidance.
- Kept compatibility loading and execution unchanged; the analyzer does not infer phase-aware contracts.

## Release constraint

Release audits must use `check-types --deps` against resolved module artifacts. A dependency's main branch may already be strict while its published module still exposes legacy macro schemas. Migrate and release providers before setting consumer graphs to a zero-legacy target.

## Validation

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `yarn check-all`
- latest Respo and Recollect source plus Recollect's resolved published dependency graph
