# Migrate core math and set guards to definition metadata

- Moved 33 pure behavior checks from the existing math/set integration fixtures
  into named `:tests` on their corresponding `calcit.core` definitions.
- Covered arithmetic, ordering, rounding, bit operations, numeric helpers,
  set operations, collection conversion, and generic set mapping/predicates.
- Kept `calcit/test-math.cirru` and `calcit/test-set.cirru` as JS/WASM and
  multi-definition integration fixtures; they remain necessary backend inputs.
- Added `yarn try-core-tests` and included it in `yarn check-all`, loading the
  core snapshot explicitly so external projects still do not run core tests by
  default.

Validation:

- transactional `edit add-test` dry-run with revision precondition
- `cargo build --bin cr`
- `cr src/cirru/calcit-core.cirru test calcit.core --tag unit` (33 passed)
- `yarn try-core-tests` (43 passed, including `calcit.test` self-tests)
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test` (372 passed)
- `yarn compile`
- `yarn check-agent-interface` (12/12)
- `yarn check-all` (native, JavaScript, IR, and WASM)
