# Public check review fixes

- Preserved `analyze check-public --format json` as one stdout JSON document under `--strict-types` by running its entry and quality preflight silently inside the public-check report.
- Changed public checking to validate target metadata across the complete selected scope before preprocessing any selected definition; rejected or incomplete scopes now report zero checked definitions.
- Tightened Snapshot loading so definition `:ffi` metadata must be a struct or a tag-keyed map, including valid target metadata when present.
- Added loader unit coverage and an agent-interface regression for structured strict-preflight failures.

Validation:

- `cargo fmt --all -- --check`
- `cargo test code_entry_rejects_malformed_ffi_metadata`
- `cargo test --bin calcit public_check`
- `cargo test` (780 library, 315 CLI, and 23 WASM tests)
- `cargo clippy --all-targets -- -D warnings`
- `yarn compile`
- `yarn check-agent-interface` (22/22 scenarios plus definition protocol round trips)
- `yarn check-all`
