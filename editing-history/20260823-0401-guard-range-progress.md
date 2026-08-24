# Guard eager range construction

- Capped eager range cardinality at the JavaScript array length limit in both
  Native and JavaScript backends, returning the same controlled error before
  iteration.
- Detect floating-point steps that no longer change the current value, avoiding
  infinite loops at large magnitudes.
- Added Native regression coverage for oversized and non-advancing ranges; the
  shared Calcit regression runs through Native and JavaScript integration suites.
