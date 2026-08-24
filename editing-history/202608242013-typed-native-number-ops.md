# Typed native number operations

## Context

Typed functions carried an injected one-argument `hint-fn` form in their native runtime body. Tail recursion therefore re-evaluated compile-time metadata on every iteration. Native calls also discarded proven `Number` argument types before evaluation and rebuilt a temporary argument vector for the normal proc dispatcher.

## Changes

- Capture surrounding-function `hint-fn` metadata in `CalcitFn`, then remove that one-argument form from the executable body. Targeted two-argument hints retain their runtime behavior.
- Add internal executable-call metadata that does not affect Calcit List equality, ordering, hashing, display, or language-level mutation behavior.
- Mark exact two-argument `&+`, `&-`, `&*`, `&/`, `&<`, and `&>` calls only when both processed arguments resolve to `Number`.
- Evaluate specialized arguments exactly once from left to right into a fixed two-value array, execute the numeric operation directly, and preserve the existing dynamic proc error path if static evidence becomes stale.
- Leave Dynamic, unresolved, wrong-arity, spread, JS, and WASM calls on their existing paths.

## Performance

The release benchmark runs the same two-argument tail-recursive loop for 1,000,000 iterations.

- Typed pre-change internal median: 1,664.953 ms.
- Typed post-change internal median: 457.819 ms, a 72.5% reduction after removing runtime metadata evaluation and adding the specialized calls.
- On the post-change binary, the specialized typed loop median was 457.819 ms versus 483.450 ms for the otherwise identical Dynamic loop, a 5.3% typed fast-path improvement.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -q` (463 + 241 + 24 + 4 tests)
- `yarn compile`
- `yarn check-agent-interface` (13/13)
- `yarn check-all`, including native, JS, IR, and WASM checks
- Latest Recollect `a694d4c`: native tests 9/9; test-entry JS generation and Node runtime passed. Its default entry retains the same four pre-existing dependency type warnings.
- Latest Respo `ead78c3`: tests 25/25, JS generation, Vite production build, and a browser Todo interaction passed with zero console errors.

The downstream Dynamic boundaries remain observable rather than being forced static: Recollect reports 204/405 (50.4%) Dynamic type positions, and Respo reports 284/1034 (27.5%).
