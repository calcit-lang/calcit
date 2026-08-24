# Indexed enum match dispatch

## Context

Issue #422 continues the performance roadmap after scoped dynamic-method diagnostics. Native `match` retained typed enum structure but still scanned branch tags, while JS emitted an if/else chain.

## Changes

- Exposed the existing enum tag-to-variant declaration index.
- Rewrote statically known, semantics-safe `match` forms into an internal declaration-ordered branch table.
- Declined the rewrite for duplicate tags, unknown tags, or a wildcard before the final branch so source-order behavior stays unchanged.
- Selected native branches directly through the enum variant index.
- Emitted JS integer-tag `switch` dispatch for indexed matches.
- Kept WASM compatible with the internal table representation.
- Added focused table/codegen tests and native/JS comparison benchmarks.
- Updated the optimization catalog and native-match RFC.

## Compatibility notes

- No user-facing syntax was added.
- `CalcitEnumValue` layout and all enum construction/serialization boundaries remain unchanged.
- Dynamic and anonymous enum matches retain the original linear representation.
- Runtime type, payload arity, wildcard, and no-match behavior remain explicit.

## Validation plan

- `cargo fmt`
- `cargo clippy --all-targets -- -D warnings`
- `yarn compile`
- `cargo test`
- `yarn check-all`
- `yarn check-agent-interface`
- `yarn bench-enum-match`
- current Respo and Recollect main regression
