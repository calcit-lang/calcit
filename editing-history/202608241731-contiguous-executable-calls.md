# Contiguous executable calls

## Summary

- Keep immutable ordinary proc, function, and method call nodes in
  `CalcitList::Vector` after preprocessing instead of rebuilding them as
  persistent `TernaryTreeList` values.
- Preserve persistent storage for syntax calls because their runtime handlers
  still rely on cheap structural `skip` operations.
- Leave quoted and runtime List values untouched; the optimization is confined
  to the result of preprocessing executable non-syntax calls.
- Cover nested ordinary calls and the syntax/quote boundary with representation
  regression tests.

## Performance notes

- A broader prototype that also converted syntax calls regressed a 200,000
  iteration tail loop by about 6%, since `CalcitList::skip` rebuilt a persistent
  tail from the Vector on every `if` evaluation. A borrowed syntax-argument view
  is required before that representation can change safely.
- On five release runs of the same 1,000,000-iteration loop, the retained
  ordinary-call-only change reduced median internal runtime from 7,810.963 ms
  to 6,998.677 ms (~10.4%) and median user CPU from 7.23 s to 6.95 s (~3.9%).

## Validation

- `cargo fmt --all -- --check`
- `cargo test -q`
- `cargo clippy --all-targets -- -D warnings`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`
- Current `calcit-lang/recollect` and `Respo/respo.calcit` main heads, including
  native/JS tests, Respo production build, and real-browser interaction.
