# Check Calx lifecycle roots as one program

- Added fail-closed eligibility for a `calcit-calx-program/1` compilation unit: exactly one init root and one reload root must pass together.
- Merged reachable function closures deterministically while preserving lifecycle roles and deduplicating shared definitions.
- Kept platform behavior outside the VM through explicit, signature-checked typed imports shared by all roots.
- Added stable program-contract and root-scoped diagnostic summaries; no partial program, Dynamic/Nil fallback, lowering, dispatcher, or runtime mode was introduced.
- Verified the change with `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `yarn compile`, `yarn check-agent-interface`, and `yarn check-all`.
