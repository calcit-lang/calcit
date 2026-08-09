# Destructuring and string API test migration

- Added six definition-attached `:unit :core` guards in `src/cirru/calcit-core.cirru`:
  nominal `destruct-str`, `destruct-list`, `destruct-set`, and `destruct-map`
  behavior, string character counting, and cross-type string comparison.
- Removed the matching eight destructuring assertions from
  `calcit/test-anonymous-enum.cirru`, preserving its anonymous enum, dynamic
  tag-match, and local enum reflection coverage.
- Removed duplicate pure string API assertions from `calcit/test-string.cirru`;
  method dispatch, macro expansion, type checks, and target-specific behavior
  remain as integration coverage.

Validation:

- `yarn try-core-tests` (191 passed)
- `yarn try-rs` (native regression passed)
- `yarn try-js` (JavaScript regression passed)
- `git diff --check`
