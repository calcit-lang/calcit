# Runtime traits: nominal implementations

## What changed

- Runtime `CalcitTrait` values now have nominal identity, so independently evaluated traits with the same displayed name and method shape do not match accidentally.
- Concrete `defimpl` values carry their exact trait origin, require the complete declared method set, and reject non-callable methods. Native preprocessing additionally verifies signatures when metadata is available.
- Tag-based `defimpl` remains compatible as an originless inherent method bag. It continues to support ordinary `.method` dispatch, but cannot satisfy trait bounds, `assert-traits`, or `&trait-call`.
- Core builtin capability implementations now preserve trait origins in native and JS runtimes, including scalar values. WASM remains an internal validation backend and reports an explicit unsupported-runtime-trait error when preprocessing cannot eliminate trait operations.
- `cr edit format` emits a non-blocking migration advisory for legacy tag-based trait arguments.

## Verification

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test` (298 library tests, 176 CLI tests)
- `yarn compile`
- `yarn check-all` (Agent interface, native, JS, IR, and WASM)
- `cr docs check-md` for traits, polymorphism, and upgrade documentation; all 24 Cirru blocks round-trip through the current formatter.
- External Respo check-only, type summary, and targeted example regression.

## Compatibility

This intentionally tightens concrete trait implementation validation. Existing tag-based method bags keep running, with a migration advisory. Code that treated unrelated same-named or partial implementations as satisfying a trait must migrate to a real `deftrait` plus complete nominal `defimpl`.
