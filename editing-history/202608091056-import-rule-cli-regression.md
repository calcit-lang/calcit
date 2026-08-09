# Import rule CLI regression coverage

- Added direct handler regressions for `cr edit imports` and
  `cr edit add-import` when an import rule has the wrong arity.
- Both tests assert the indexed validation diagnostic and verify that the
  snapshot remains byte-for-byte unchanged after rejection.
- This protects the persistence boundary in addition to the existing
  `validate_import_rules` unit coverage.

Validation:

- `cargo fmt`
- `cargo clippy --bin cr -- -D warnings`
- `cargo test --bin cr` (194 passed)
- `git diff --check`
