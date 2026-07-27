# Fix exponential `or` expansion and bound code generation

## Summary

- Fixed the `or` macro so its recursive tail is emitted once instead of being duplicated in both falsy branches.
- Preserved Calcit truthiness semantics for `nil`, `false`, and arbitrary truthy values.
- Added a long-operand regression case to the macro test suite.
- Added verbose preprocessing phase/definition diagnostics for JS and IR generation.
- Added a configurable code generation timeout, defaulting to 60 seconds, including watch-mode rebuilds.
- Increased the code generation worker stack for deeply nested macro and type preprocessing.

## Root cause

The previous `or` macro emitted the remaining operands in both the `nil` and `false` branches. Recursive macro expansion therefore grew exponentially for long `or` expressions. A real-world 40-plus-operand predicate appeared to hang and consumed CPU until externally terminated.

## Validation

- `cargo fmt --check`
- `cargo clippy --bin cr -- -D warnings`
- `cargo test --lib runner::preprocess -- --nocapture`
- `target/debug/cr calcit/test.cirru`
- `target/debug/cr calcit/test.cirru --timeout 60 js`
- The original `respo/markdown/calcit.cirru` reproduction now reaches its project-level warnings in under one second instead of timing out after 60 seconds.
