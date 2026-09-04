# Isolate variadic generic bindings

## Context

Review of the transactional generic-matching change found that the variadic
rest path still mutated shared type-variable bindings while testing a candidate
argument. A failed nested match could therefore constrain later rest arguments
and the final generic trait-bound check.

## Changes

- Match every variadic argument against a cloned binding map and commit that
  candidate only after the whole argument type matches.
- Add a variadic generic fixture whose first nested Map mismatch must not make a
  later valid Map fail.
- Assert that only the original mismatching rest argument is diagnosed and
  document both fixed-argument and rest-argument rollback coverage.

## Validation

- The focused collection member contract fixture passes.
- Formatting, clippy, the full Rust suite, and `yarn check-all` are rerun before
  the review fix is pushed.
