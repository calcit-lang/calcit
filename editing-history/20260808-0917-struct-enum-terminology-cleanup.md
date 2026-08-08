# Struct and Enum terminology cleanup

## Summary

- Renamed current Struct and Enum locals in trait fixtures to `struct_value` and `enum_value`.
- Updated active test comments, method inspection tags, type-failure documentation, and WASM validation labels to use Struct/Enum terminology.
- Renamed the `struct-with` macro parameter and Struct implementation validation locals in the current preprocessing path.
- Updated the agent interface diagnostic to describe source-backed struct methods.

## Compatibility retained

- Legacy `record` and `tuple` API spellings, migration diagnostics, historical test namespaces, and explicit WASM/type-failure fixture names remain unchanged.

## Validation

- `cargo check --all-targets`
- `cargo test runner::preprocess::tests --lib` (72 passed)
