# Complete Unit effect-return review

- Document that `not` accepts Unit as a falsey input in its builtin help and core documentation.
- Ensure `each` and `&doseq` discard callback or body values and return Unit for non-empty traversals.
- Add definition-attached Unit regressions for both traversal helpers.
