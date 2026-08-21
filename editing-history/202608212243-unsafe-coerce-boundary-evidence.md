# Unsafe-coerce boundary evidence

## Summary

- Extended the `unsafe-coerce` inventory with an explicit static `evidence`
  object: input source form, target schema, declared `:js-ffi` capability, and
  `js-ffi.raw.*` adapter-namespace convention.
- Kept the evidence honest: it describes Snapshot source only and never claims
  to have observed or validated a JavaScript runtime value.
- Bumped `analyze.weak-types` JSON protocol to v5 and added tests for raw JS,
  host-member, ordinary-expression, and adapter namespace classification.
- Applied #378 review feedback before merging this follow-up: quoted and
  quasiquoted descendants no longer create unsafe inventory debt, while `~` /
  `~@` expressions remain executable; unsafe diagnostics now describe generic
  runtime values rather than assuming every assertion is JavaScript-backed.

## Validation

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface` (13/13 scenarios)
- `yarn check-all`
- New debug CLI against `../respo-calcit-workflow/calcit.cirru`; it surfaced
  four assertions and accurately identifies two definitions without a declared
  `:js-ffi` capability.
