# Migrate list/map guards and retain target-specific fixtures

- Added definition-attached unit tests for list construction, ranges, option
  accessors, slicing helpers, map merge behavior, and non-nil map keys.
- Expanded the migrated bit, trim, and blank-string tests to cover every
  boundary previously asserted by their standalone native fixture groups.
- Removed `test-trim` and `test-whitespace` from `calcit/test-string.cirru`.
- Kept `test-bitwise` as a compact `inside-js:` fixture so JavaScript codegen
  and Node execution still cover those runtime primitives without duplicating
  native execution.
- Documented the split between native definition-attached tests and JS/WASM
  target-specific fixtures.

Validation:

- core transactions dry-run with revision preconditions
- `cr src/cirru/calcit-core.cirru test calcit.core --tag unit` (67 passed)
- `cr src/cirru/calcit-core.cirru test --tag unit` (77 passed)
- `cr calcit/test-string.cirru` (native fixture)
- `cr calcit/test.cirru js && node js-out/main.mjs` (top-level JS fixture)
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test` (372 library tests, 197 CLI tests)
- `yarn compile`
- `yarn check-agent-interface` (12/12)
- `yarn check-all` (core suite, native, JavaScript, IR, and WASM)
