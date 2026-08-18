# Recursive nominal types and symbol presentation

## Background

`defstruct` field annotations eagerly resolved a reference to their own
definition. A recursive field such as `(:: 'Optional Node)` therefore expanded
`Node` while it was being declared and could exhaust the Rust stack. In
addition, nominal record, struct, and enum names were displayed as tags even
though they are type symbols.

## Changes

- Preserve a strict self-reference as `TypeRef` during type annotation parsing,
  while keeping normal trait reference resolution unchanged.
- Add native and snapshot regression coverage for nil and nested recursive
  nodes, plus a required recursive field that produces a regular type error.
- Render record, struct, and enum names as quoted symbols in Rust and the JS
  runtime.
- Extend the browser custom formatter to expose nominal struct field types and
  enum payload types, with structured runtime checks for the formatter output.

## Verification

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
