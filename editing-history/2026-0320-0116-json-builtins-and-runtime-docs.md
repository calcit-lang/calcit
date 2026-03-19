## Summary

This commit moves JSON parsing and serialization into Calcit builtins and aligns the surrounding runtime/docs/tooling updates needed to ship it cleanly.

1. Added builtin JSON runtime functions
- Introduced `json-parse`, `json-stringify`, and `json-pretty` in Rust builtin dispatch and JS runtime exports.
- Added fast arity/type checks on exposed runtime entry points.
- Normalized integer-valued numbers to encode as JSON integers in Rust, matching JS output.
- Added native tests and Calcit-level coverage for parse/stringify/pretty behavior and error paths.

2. Updated core docs and runtime placeholder naming
- Added `calcit.core` docs/examples for the JSON runtime functions.
- Renamed the runtime placeholder spelling from `runtime-inplementation` to `runtime-implementation` across core snapshot metadata and Rust handling.
- Preserved an explicit `cr <snapshot-file> edit format` example in `docs/CalcitAgent.md`.

3. Snapshot formatting workflow
- Re-ran `cr edit format` against the touched Cirru snapshot files:
  - `src/cirru/calcit-core.cirru`
  - `calcit/test.cirru`

## Files touched (high level)

- Rust builtin registration and implementation for JSON runtime support.
- JS runtime proc exports for JSON behavior and validation.
- Calcit core snapshot docs/examples and test snapshot updates.
- CLI/codegen placeholder spelling cleanup.
- Agent guide example for `edit format`.

## Validation notes

- `cargo test json_`
- `cargo test validate_runtime_impl_is_skipped -- --nocapture`
- `cargo test runtime_placeholder -- --nocapture`
- `cargo run --bin cr -- calcit/test.cirru -1`
- `yarn check-all`

## Release size check

Measured `target/release/cr` against `HEAD` using a detached worktree build:

- Current: `5192960` bytes (`5.0M`)
- Base: `5174400` bytes (`4.9M`)
- Delta: `+18560` bytes (about `+18.1 KiB`)

Conclusion: the builtin JSON work increases the release binary size slightly, but the delta is small relative to the ~5 MB target.
