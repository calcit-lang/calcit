# 2026-0307-1821 Snapshot and Schema Serialization Refinement

- Verified Rust core changes using `yarn try-rs`, `cargo test`, and `yarn check-all`.
- Ensured `Snapshot` and `CodeEntry` serialization/deserialization remains consistent with the new direct map EDN format for schemas.
- Confirmed unit tests for `CodeEntry` with examples and schema validation are passing.
- Validated that `calcit-core.cirru` example parsing works as expected.
