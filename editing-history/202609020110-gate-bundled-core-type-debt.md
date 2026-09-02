# Gate bundled core type debt

## Context

The bundled Cirru core already exposes unresolved Dynamic contracts through
`analyze weak-types`, but CI only executed its unit tests. The inventory grew
from 304 to 305 schema-Dynamic positions when a new core API was added, showing
that diagnostics alone did not prevent gradual regression.

## Changes

- Check in a native v2 `analyze quality` baseline for
  `src/cirru/calcit-core.cirru`, scoped per definition.
- Run the baseline gate from `yarn check-all`, pull-request CI, and the release
  workflow.
- Document the review rule: cleanup batches lower the baseline; unexplained
  regressions must not regenerate it.

The initial inventory is 305 unresolved schema-Dynamic positions, 47 definitions
with no trusted type coverage, and 145 definitions that are not fully typed.
Per-definition budgets ensure a cleanup elsewhere cannot hide new debt.

## Follow-up

Use calcit#579 to classify and migrate caller-propagating collection, lookup,
Ref/state, equality/compare, and public macro contracts. Genuine open boundaries
remain explicit and reviewed rather than being mechanically closed.

## Validation

- `yarn check-core-quality`
- `cargo fmt -- --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn check-all`
