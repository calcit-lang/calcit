# Indexed enum match validation fixes

- Fixed the release benchmark example to import internal Calcit value types from the public `calcit::calcit` module.
- Reduced the indexed JS match helper argument count and derived the wildcard slot from the internal table layout, keeping Clippy clean without changing generated semantics.
- Applied `cargo fmt` to the touched Rust code.
- Verified `cargo clippy -- -D warnings`, `cargo test`, `yarn compile`, `yarn check-all`, and `yarn check-agent-interface`.
- Measured the 16-variant last-branch benchmark at 291.55 ms for linear native tag scanning versus 108.35 ms for indexed lookup; the JS comparison measured 37.62 ms versus 31.06 ms.
