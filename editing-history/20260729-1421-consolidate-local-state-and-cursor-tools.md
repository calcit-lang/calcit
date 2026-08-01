# Consolidate local state and cursor tools

## Summary

- Consolidated project-local sidecar files under `.calcit/`: cursor state now uses `.calcit/cursor.cirru`, and persisted runtime errors use `.calcit/error.cirru`.
- Added one-time migration from `.calcit-cursor.cirru` and `.calcit-error.cirru` without dual writes.
- Kept state paths relative to the selected snapshot, including commands invoked from another working directory.
- Extended cursor schema v4 with one region anchor, up to 16 named marks, and a compact last-query descriptor.
- Added `cursor anchor`, `clear-anchor`, `region`, `mark`, `goto`, `marks`, and `rm-mark`, plus `query next` and `query prev`.
- Applied the same path transforms to the active cursor, anchor, and marks during structural edits; stale query continuation is rejected after a snapshot revision change.
- Bounded the complete cursor file to 64 KiB and retained existing history, stack, and mark count limits.

## Validation

- `cargo fmt`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`
- Real-project regression on a temporary Respo workflow copy, including legacy migration, region/marks/query continuation, and successful `cr js` compilation.
- Release binary changed from 8,196,752 bytes to 8,280,352 bytes on the same arm64 build environment (about 1.02%).
