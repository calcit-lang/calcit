# Library quality gates and format advisories

## Summary

- Add a dedicated Calcit library/module acceptance guide covering Snapshot canonicalization, entries, type slots, weak-type baselines, examples, Markdown checks, entry/backend validation, call-graph limits, consumer regression, and CI evidence.
- Refresh the project upgrade guide for once-by-default execution, entry modes, unified `:entries`, per-entry type slots, canonical `:dynamic`, and current static-analysis commands.
- Extend `cr edit format` with recoverable advisories for legacy `:configs`, the `compact.cirru` filename, legacy `:any`, and unresolved dynamic type debt.
- Keep formatting non-blocking for quality debt and direct users to `check-types` / `weak-types` for semantic paths and recommendations.

## Validation

- `cargo fmt`
- `cargo clippy -- -D warnings`
- focused `cr` format advisory unit tests
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`
- `cr docs graph check`
- `cr docs check-md docs/run/library-quality.md`
- `cr docs check-md docs/run/upgrade.md`
- legacy config and filename formatting on temporary Snapshots
- Respo consumer regression on a temporary Snapshot: format, config show, weak-types summary, and `--check-only`
