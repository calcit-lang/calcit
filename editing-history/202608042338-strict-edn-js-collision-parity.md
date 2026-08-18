# Strict EDN JS collision parity and final review

## Knowledge

- Strict typed decoding must observe source collection entries before runtime collection constructors normalize or deduplicate them; otherwise decoded-value collisions cannot be detected reliably.
- Exact duplicate source set items keep ordinary set semantics, and exact duplicate source map keys keep last-value-wins semantics. Distinct source values that normalize to the same typed value must instead fail with an item/key path.
- Data-shape validation promises nominal identity, so structurally equal struct or enum declarations are insufficient; validation must retain and compare the exact declaration `Arc`.

## Changes

- Added an internal typed-EDN set view and a strict JS extraction path that preserves collision evidence without changing dynamic `parse-cirru-edn` behavior.
- Added JS set/map decoded-collision checks and shared Native/JS integration fixtures for collisions and exact source duplicates.
- Tightened struct/enum data-shape validation to pointer identity and fixed the RFC blockquote lint warning.

## Validation

- `cargo fmt --all`
- `cargo test calcit::data_shape::tests::rejects_structurally_equal_but_distinct_nominal_declarations`
- `yarn compile`
- `yarn try-rs`
- `yarn try-js`
