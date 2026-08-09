# Definition-attached core tests: consolidated history

This development window introduced and completed definition-attached tests in
`src/cirru/calcit-core.cirru`, so pure `calcit.core` API contracts can run from
the definition that implements them while target-specific and integration
coverage remains in the legacy fixtures.

## Durable design decisions

- Added backward-compatible `:tests` metadata and embedded `calcit.test`
  assertions (`is`, `is=`, `is-not=`, `throws?`, `is-throws`, and `fail`).
- `cr test` discovers named tests with namespace, definition, name, and tag
  filters, supports list/fail-fast/JSON modes, and uses conservative static
  dependency analysis for `--affected` selection.
- Unscoped `cr test` stays project-local to the namespaces in the input
  snapshot; core tests are run explicitly (including by CI) so external
  projects do not execute `calcit.core` tests accidentally.
- Ordinary test execution preprocesses lazily, while `--affected` builds its
  static candidate index once. This keeps fail-fast and normal runs from
  compiling tests that will not execute.
- Migrated pure guards for collection, string, parsing, numeric, math, set,
  destructuring, and update APIs into `calcit.core` definitions. Removed their
  duplicate assertions from `calcit/test-*.cirru` while retaining method,
  macro, type/preprocess, multi-definition, JavaScript, and WASM fixtures.
- CI and release workflows explicitly run the embedded core suite. A compact
  JavaScript fixture remains for target-specific bitwise execution.
- Review fixes standardized whitespace/duplicate test-name validation,
  preserved empty JSON report envelopes on selection errors, corrected builtin
  `query tests` behavior and affected-test diagnostics, executed the
  `test-comma` fixture, corrected `&map:diff-new` documentation, and removed
  25 duplicate or strict-subset attached tests. The final core suite contains
  166 tests.

## Verification

- `yarn try-core-tests` (166 passed)
- `yarn try-rs` and `yarn try-js`
- `yarn check-all` (core, native, JavaScript, IR, WASM, and agent interface)
- `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `yarn compile`
- JSON test output remains one parseable report envelope.

The detailed per-commit notes were merged into this record; their exact text
remains recoverable from Git history.
