## Summary

- restored high-level `defstruct` / `defenum` where-map syntax end to end after the Rust/runtime work landed
- aligned JS runtime parsing in `ts-src/calcit.procs.mts` so `defstruct` / `defenum` skip optional generics and where-map entries before parsing fields or variants
- kept the smoke test in `calcit/test-enum.cirru` on the high-level syntax and reran broad validation

## Knowledge Points

- the JS-emitted high-level data-definition forms do not preserve where-map heads as a stable plain symbol; the first item may be a runtime function alias or even an unresolved value shape, so detecting where-map entries by exact head identity is brittle
- a safer JS-side strategy is structural detection: after an optional generics list, treat a list as a where-map form when its head is not a tag/symbol/string and the remaining items are all 2-item pairs
- for current JS codegen, generic parameter lists arrive as a simple list of `CalcitSymbol`s, so the runtime parser can ignore that leading entry without needing full type-variable reconstruction

## Validation

- `cargo fmt`
- `cargo test where_bounds --lib`
- `cargo run --bin cr -- calcit/test-enum.cirru`
- `yarn compile && yarn try-js`
- `yarn try-rs`
- `yarn try-ir`
- `yarn try-wasm`