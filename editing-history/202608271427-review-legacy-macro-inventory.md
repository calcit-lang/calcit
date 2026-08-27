# Legacy macro inventory review follow-up

## Summary

- Persisted the `Dynamic` legacy macro origin through canonical Snapshot serialization instead of reclassifying it as `Fn` after reload.
- Limited legacy macro parameter-shape validation to old `Fn` contracts; a whole-`Dynamic` schema carries no arity contract.
- Corrected the Agent guide to use the complete `data.summary.legacy_macro_schemas` JSON path.

## Compatibility

Old wrapped macro schemas omit `:legacy-origin` and continue to load as `Fn`. Only whole-`Dynamic` compatibility schemas write `:legacy-origin :dynamic`, so existing `Fn` serialization remains unchanged.

## Validation

- focused origin round-trip and Dynamic-parameter regression tests
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -- --test-threads=1`
- `yarn check-all`
