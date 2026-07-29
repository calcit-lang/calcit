# Document Calcit local state

## Summary

- Documented `.calcit/` as the bounded project-local state directory and recommended ignoring the directory as a unit.
- Updated cursor RFC and Agent guidance for anchor/region, named marks, query continuation, migration behavior, and state-size limits.
- Moved the recommended multiline snippet location to `.calcit/snippets/`.
- Updated error-stack references to `.calcit/error.cirru` while preserving historical editing records.
- Clarified that `.compact-inc.cirru` remains the existing watcher protocol rather than project-local diagnostic state.
- Corrected repository-local Agent examples and replaced the obsolete binary-size threshold with a same-platform exact-byte baseline comparison.

## Validation

- `target/debug/cr docs check-md docs/CalcitAgent.md --entry calcit/test.cirru` (4/4 blocks passed)
- `git diff --check`
