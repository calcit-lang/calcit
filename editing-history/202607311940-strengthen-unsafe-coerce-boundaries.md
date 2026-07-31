# Strengthen unsafe-coerce boundaries

## Summary

- Kept `unsafe-coerce` visible in preprocessed expressions instead of mutating the source local's type globally, preserving a stable boundary node for later static evidence reporting.
- Added type-directed shorthand construction for confirmed `defstruct` and `defenum` definitions while preventing record and enum tuple instances from being treated as constructors.
- Validated shorthand struct fields for pair shape, membership, duplication, required-field coverage, and statically known value type mismatches.
- Reused typed record and enum constructor nodes so native, JavaScript, and WASM backends receive their existing canonical forms.
- Documented the boundary model, distinguishing trusted `unsafe-coerce` declarations from future runtime-validated evidence.
- Added positive and negative preprocessing coverage for constructor rewrites, invalid variants, malformed fields, method syntax, and instance/prototype separation.

## Validation

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface` (12/12 scenarios)
- `yarn check-all` (native, JavaScript, and WASM checks passed)
- Current debug `cr --check-only` against `/Users/jon.chen/repo/respo/respo/calcit.cirru`
