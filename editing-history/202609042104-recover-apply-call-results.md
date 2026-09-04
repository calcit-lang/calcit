# Recover apply call results

## Context

`calcit.core/apply` spreads one homogeneous List into a callable with arbitrary
arity. That arity relationship cannot be represented by the current public
schema, so its return was always Dynamic even when both the callable signature
and list member type were known.

An ecosystem audit also considered replacing the recursive List contract of
`format-cirru` with `CirruQuote`. Editor, code-viewer, error-viewer, and API
projects still construct Cirru trees as nested lists; making that break before a
recursive nominal migration path exists would push callers toward unsafe
coercion, so that candidate was deferred.

## Changes

- Infer the result of `apply f args` from `f` only when the homogeneous list
  member matches every fixed and rest callable input.
- Bind generic callable returns from the list member type; retain Dynamic for
  unknown functions, incompatible heterogeneous-position contracts, or
  unproved trait bounds.
- Add unit coverage for concrete, generic, and incompatible callable contracts,
  plus end-to-end Calcit `assert-type` coverage.
- Document the specialization and classify the remaining schema Dynamic as a
  reviewed compiler-specialized contract rather than unfinished public API
  migration.

## Validation

- Focused Rust inference tests and the type-inference Snapshot check pass.
- Formatting, clippy, the full Rust suite, and `yarn check-all` are rerun before
  push.
