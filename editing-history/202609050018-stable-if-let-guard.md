# Stable Rust compatibility for guarded matches

Calcit 0.13.77 used `if let` match guards in the FFI Interface IR declaration resolver and typed-method callable selection. Match-arm `if let` guards are still unstable on stable Rust, even though ordinary `if` let-chains are available.

The stable rewrite keeps the branch order and behavior explicit:

- An empty FFI declaration candidate list now performs a nested match to distinguish host-managed capabilities from genuinely missing declarations.
- Function method entries use `Option::is_some_and` in the match guard, then recover the already-proven definition reference inside the arm. Entries that fail this guard still fall through to nominal callable synthesis or runtime dispatch exactly as before.

Existing focused tests cover both FFI diagnostic outcomes and the callable-selection fallthrough behavior. Validation includes formatting, clippy with warnings denied, the focused Rust tests, the full Rust suite, the repository build, and a packaged-crate install smoke on stable Rust.
