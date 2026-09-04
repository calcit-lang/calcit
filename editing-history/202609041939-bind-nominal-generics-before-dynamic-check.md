# Bind nominal generics before Dynamic boundary checks

## Context

Review of PR #638 identified an order-dependent false negative: for
`Fn<T>(T, T)`, a leading Dynamic argument was checked before a later Person
argument could bind `T` to the nominal type.

## Changes

- Resolve argument types once and collect generic bindings from every fully
  non-Dynamic argument before enforcing the open-to-nominal boundary.
- Treat Dynamic nested in containers, nominal type arguments, named refs, and
  function contracts as open evidence that must not supply a generic binding.
- Add a regression where argument 2 binds `T` to Person and argument 1 must then
  fail with `E_DYNAMIC_NOMINAL_ARGUMENT`.

## Validation

- Focused unit and end-to-end strict boundary tests pass.
- Full repository gates are rerun before push.
