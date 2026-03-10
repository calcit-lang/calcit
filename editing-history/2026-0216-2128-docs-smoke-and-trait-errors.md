# Docs smoke cases and trait misuse error normalization

## Summary

- Added executable docs smoke tests for key trait semantics.
- Integrated smoke tests into main test entry.
- Normalized high-frequency misuse messages with `Expected/Actual/Fix` guidance.
- Stabilized cross-runtime smoke assertions so `yarn check-all` passes in both eval and js modes.

## Knowledge points

- `assert-traits` first argument is enforced at preprocess as local-only.
- `impl-traits` targets definition-level values (`struct`/`enum`), not instances.
- Error string matching in JS runtime should avoid host-error-dependent formatting; keep strict message-content checks in eval mode.

## Validation

- `cargo fmt`
- `cargo run --bin cr -- calcit/test.cirru -1`
- `yarn check-all` (exit code 0)
