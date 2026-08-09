# Lazy execution and string API test migration

- Changed ordinary `cr test` execution to preprocess a test only immediately
  before running it. `--fail-fast` therefore avoids compiling work it will
  never execute.
- Kept eager candidate compilation for `--affected`, since static dependency
  selection requires it, and build the compiled-definition index once instead
  of once per candidate test.
- Moved 19 direct core guards for bit operations, number base formatting,
  unicode characters, and string APIs to named `:tests` entries in
  `calcit-core.cirru`.
- Retained `calcit/test-string.cirru` as an integration fixture for method
  syntax, eval-specific behavior, and JavaScript backend coverage.

Validation:

- core test transaction dry-run with revision precondition
- `cr src/cirru/calcit-core.cirru test calcit.core --tag unit` (52 passed)
- `cr src/cirru/calcit-core.cirru test --affected calcit.core/str --list --format json`
- `yarn try-core-tests` (62 passed)
- `cargo fmt`
- `cargo test --bin cr` (197 passed)
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test` (372 library tests, 197 CLI tests)
- `yarn compile`
- `yarn check-agent-interface` (12/12)
- `yarn check-all` (native, JavaScript, IR, and WASM)
