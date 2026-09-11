# Scope checked-contract refresh work

- Refresh staged checked-call contracts only for core functions with a known checked contract and matching arity.
- Avoid allocating processed-argument snapshots for unrelated multi-argument calls.
- Keep inline collection callbacks able to use element types inferred from an earlier literal argument.
