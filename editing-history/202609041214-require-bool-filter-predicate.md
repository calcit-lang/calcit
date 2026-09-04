# Require Bool from filter predicates

- Tighten the new structured `filter` callback contract from arbitrary return
  `R` to `Bool`, matching `every?` and the underlying `&list:filter` contract.
- Add a negative preprocessing regression that proves a `Number`-returning
  predicate produces `W_FN_ARG_TYPE_MISMATCH` for `calcit.core/filter`.
- This addresses the focused review finding on PR #622.
