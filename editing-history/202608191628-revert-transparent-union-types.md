# Revert transparent union types

- Removed the experimental `deftype (or ...)` transparent-union implementation, including its `struct-match` narrowing, type matching, runtime validation, core macro, RFC, Snapshot coverage, and Rust regression tests.
- Kept unrelated release-version and test-only visibility cleanup changes from follow-up commits.
- The experiment showed that collection and recursive boundaries still lose useful member-type evidence, causing consumer code to require additional assertions instead of becoming simpler.
