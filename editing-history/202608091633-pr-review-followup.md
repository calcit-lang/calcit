# PR review follow-up

- Executed the previously uncalled `test-comma` fixture by wrapping its
  assertion in `fn ()` and invoking it from `test-list.main/main!`.
- Corrected the `&map:diff-new` documentation to describe entries present in
  the first map and absent from the second map.
- Removed 25 duplicate or strict-subset core test entries, reducing the
  definition-attached unit suite from 191 to 166 tests without reducing
  contract coverage.
- Verified earlier review fixes for whitespace-safe test names, JSON empty
  reports, builtin `query tests`, and conservative `--affected` diagnostics.

Validation:

- `yarn try-core-tests` (166 passed)
- `yarn try-rs` (native regression passed)
- `git diff --check`
