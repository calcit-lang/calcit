# Query type errors use canonical Symbols

## Summary

- Updated `cr query type` guidance and related CLI help to use canonical quoted type symbols such as `'String`, `'Number`, and `'List` instead of legacy tag examples.
- Allowed `cr query type` to parse canonical quoted builtin symbols while preserving compatibility with legacy lowercase/tag spellings.
- Added regression coverage for canonical symbols, compound types, the migrated error message, and excess symbol prefixes.

## Validation

- `cargo fmt`
- `cargo clippy --bin cr -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`
