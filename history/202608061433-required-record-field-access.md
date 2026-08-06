# Required record field access

## Background

Struct-backed values and loose records both have a fixed runtime field set.
Direct field syntax therefore must not inherit the optional semantics of the
generic collection `get` API: a declared field returns its value directly and
an absent field is an ordinary diagnostic.

## Changes

- Specialize postfix and tag-headed record access to indexed or required
  record lookup during preprocessing.
- Make missing native record fields fail consistently in the Rust, JavaScript,
  and WASM runtimes instead of producing nil or undefined.
- Keep the public collection `get` API optional by guarding required record
  lookup with `record:contains?`.
- Add Rust and JavaScript regression coverage for direct field specialization
  and missing-field diagnostics.
- Update the internal `&record:get` schema and documentation to describe the
  required lookup contract.

## Verification

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`
- Respo `cr calcit.cirru --check-only`
- Respo `cr calcit.cirru js`
