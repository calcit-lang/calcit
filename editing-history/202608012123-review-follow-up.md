# Review follow-up for CLI consolidation

## Summary

- Updated staged-file rewriting to open the temporary file with write access, truncate it, write through the same handle, and then sync it. This keeps `sync_all` valid on Windows.
- Reject symbolic-link destinations before staging so an atomic rename cannot silently replace the link while preserving permissions from its target.
- Avoid copying every definition tree during whole-project search; only a subtree selected by `--start-path` remains owned.
- Corrected cursor feedback so replace, swap, unwrap, and wrap report an unaffected cursor when their path did not change.

## Tests

- Added staged write/commit coverage and a Unix symbolic-link rejection test.
- Added cursor feedback assertions for unrelated paths and single-child unwraps.

## Review disposition

- Kept the existing parent-context preview for `query search-expr`; it predates this refactor and is intentional shared search presentation rather than a regression.
- Deferred directory `fsync` after rename: it is a larger crash-durability policy decision, especially for multi-file commits, and is not needed to correct the current atomic rename behavior.
