# Require complete evidence for `apply` result recovery

- Keep `apply` on its public Dynamic result when the spread list member type is
  Dynamic.
- Require exact known list cardinality for fixed-arity callables and a proven
  minimum for callables with fixed parameters plus a rest parameter.
- Add regression coverage for heterogeneous list members, unknown list length,
  and incompatible literal length.
- Clarify that all trait-bounded callables remain on the Dynamic fallback.
