# Canonical schema types use quoted symbols

## Summary

- Keep legacy lowercase type tags load-compatible, while making quoted symbols such as `'String`, `'List`, `'Fn`, and `'Dynamic` the canonical schema serialization.
- Make `cr edit format` rewrite only recognized type positions in schemas, hints, assertions, coercions, structs, and enums; ordinary tag data remains unchanged.
- Migrate repository Snapshots, type-query/IR/agent-interface output, build-time core Snapshot loading, and primary type documentation.

## Validation

- `cargo fmt`
- `cargo test`
- `cargo clippy -- -D warnings`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`
- `cr docs check-md docs/features/static-analysis.md --entry calcit/test.cirru --failures-only`
- `cr docs check-md docs/CalcitAgent.md --entry calcit/test.cirru --failures-only`
- `cr docs graph check`
- formatted every valid repository Snapshot with `cr edit format`, then `cr calcit/test.cirru --check-only`
