# Schema Generics Quote Normalization

## Summary

Tightened schema loading so legacy generic symbols stored with embedded leading quotes now fail immediately, then normalized existing `calcit-core.cirru` schema generics from `''T`-style output to the correct `'T` source syntax.

## Key Changes

- `src/calcit/type_annotation.rs`
  - Stopped serializing schema generics as `Edn::Symbol("'T")`.
  - Now writes plain EDN symbols like `Edn::Symbol("T")`, which format back to source as `'T`.
  - Keeps compatibility when parsing previously saved legacy symbols, but normalizes them to plain names.

- `src/snapshot.rs`
  - Added load-time rejection for legacy schema generic symbols carrying embedded leading quotes.
  - Normalized schema EDN → Cirru conversion to go through format+parse so quoted symbols render correctly.
  - Added regression tests for:
    - single-quote round-trip,
    - rejecting legacy quoted EDN symbols on load,
    - rejecting `''T` on schema writes.

- `build.rs`
  - Added the same legacy-generic rejection during build-time loading of `src/cirru/calcit-core.cirru`, so bad schema data fails `yarn check-all` immediately.

- `src/cirru/calcit-core.cirru`
  - Replaced all legacy schema generic forms like `''T`, `''A`, `''K` with correct single-quoted forms.

- `docs/CalcitAgent.md`
  - Updated schema/type-annotation guidance to reflect schema-first top-level typing and correct generic syntax examples.

## Validation

- `cargo fmt`
- `cargo test schema_ -- --nocapture`
- `yarn check-all`
