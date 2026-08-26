# Preserve nested macro gensym positions on cache hits

- Fixed pure macro expansion caching so its replay delta contains only gensyms
  emitted by the cached macro evaluator. The evaluator end position is captured
  before recursively preprocessing the returned syntax tree.
- Excluded gensyms emitted by nested macros during post-processing from the
  outer cache entry. Nested macros still run or replay their own gensym progress
  when the cached outer syntax is preprocessed, preventing double advancement.
- Added a regression test that compares miss and hit paths for an outer macro
  whose output invokes a gensym-producing inner macro, and verifies that the
  following gensym position is identical.
- Kept the Calcit package at 0.13.45 in this functional PR. Package versions are
  updated together in `Cargo.toml` and `package.json` by the separate release
  commit after the feature PR merges.
- Validated formatting, Clippy with warnings denied, all 799 Rust tests, and the
  full native/JS/IR/WASM integration suite through `yarn check-all`.
