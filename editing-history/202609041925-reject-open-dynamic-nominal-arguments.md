# Reject open Dynamic nominal arguments in strict mode

## Context

An explicitly open `Dynamic` value could still flow directly into a function
argument whose contract required a Struct or Enum. General inference may look
through a Dynamic return contract to support gradual migration, so strict
checking needs to retain the explicit boundary evidence without changing
compatibility-mode inference.

## Changes

- Add strict project-source diagnostic `E_DYNAMIC_NOMINAL_ARGUMENT` for root
  `Dynamic` and matching containers whose Dynamic member erases a closed
  Struct/Enum argument contract.
- Preserve an explicitly declared Dynamic return as boundary evidence for this
  check while leaving missing-schema and compatibility behavior unchanged.
- Cover direct and local calls, generic substitution, rest arguments, nested
  containers, compatibility mode, and the decoder migration message.
- Document typed Cirru EDN, runtime-map, and typed FFI adapter migration paths.

## Validation

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -- --test-threads=1`
- `yarn check-all`
