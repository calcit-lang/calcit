# Result-based file effects

## Summary

- Added String methods `.read-file`, `.read-dir`, and `.write-file` that convert recoverable file failures into `Result<..., String>` while retaining the raising raw procedures for compatibility.
- Added native and generated-JavaScript regression coverage for missing paths and static Result contracts.
- Completed the JavaScript host boundary for file effects: `read_file` now returns and validates injected content, `read_dir` validates/sorts host paths, and missing injections raise stable errors that the safe methods can catch.
- Updated type guidance and file-operation examples to prefer receiver methods.
- Fixed a Rust 1.97 Clippy finding in the previously landed native-call-shape test so the required `-D warnings` gate remains clean.

## Knowledge retained

- A language-level `try` wrapper is sufficient for Result-returning effects and keeps raw primitives ABI-compatible. Because generated JavaScript `try` normalizes host exceptions to String, the same core wrapper works on native and JS.
- Core String methods are registered in `&core-string-methods`; adding a method there makes receiver typing and method discovery reuse the wrapper function schema.
- Node file effects are host capabilities supplied through `globalThis.__calcit_injections__`. Validate values at this Dynamic boundary before returning them to typed Calcit code. Browser `read-file` keeps its localStorage compatibility path but now raises on a missing key, matching the recoverable failure contract.
- `read-dir` must sort injected JavaScript paths to match the native procedure's deterministic contract.
- WASM currently skips `try`-based file wrappers and does not expose host file procedures; keep this backend limit explicit in API docs and regression output.

## Validation

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `yarn check-all`
- Latest Respo main: 27/27 tests, 111/111 documentation blocks, JS codegen
- Latest Recollect main: 9/9 native tests, generated JS tests, production build with Node 24.4.1, required WASM checks
