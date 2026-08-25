# Typed method error-stack review

- Kept the typed number-binary fast path aligned with normal proc dispatch by
  attaching the active call stack when a specialized operation returns an
  otherwise stackless error.
- Added a regression test comparing invalid specialized and ordinary remainder
  dispatch with stack tracking enabled.

Validation: `cargo fmt --all -- --check`, targeted runner tests, `cargo clippy
--all-targets -- -D warnings`, and `cargo test`.
