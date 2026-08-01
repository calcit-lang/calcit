# Cirru quote input diagnostics

- Clarified CLI errors for missing or malformed `quote` transport wrappers.
- Distinguished symbol, string, and expression payload examples in parser hints.
- Added coverage for ambiguous bare forms and multi-payload quote input.

Validation:

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn check-all`

