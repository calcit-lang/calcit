# Test runner selection and reporting guardrails

- Added `cr test --exclude-tag` so CI and agents can leave slow or integration
  guards out of a focused run without weakening the default suite.
- Added `--require-match` to make an empty scoped/tagged/affected selection fail
  instead of producing an accidental green result.
- Added `--summary-only` for compact large-suite output. JSON reports now state
  `detail`, distinguish selected from executed tests, and include per-test
  execution durations in full reports.
- Kept test execution native-only. JS/WASM checks remain dedicated fixtures;
  an in-process timeout was intentionally not added because the interpreter
  cannot safely cancel a running test thread.

Validation:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`
- JSON summary and empty-selection CLI smoke checks
- `git diff --check`
