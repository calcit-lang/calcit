# Replace the legacy cr-sync binary with a Calcit script

- Removed the `cr-sync` Rust binary and its installation documentation.
- Moved the bundle script to `calcit/scripts/` and added `sync-calcit.cirru` beside it.
- Added `unix-time-ms` for script-generated snapshot metadata.
- Reused `bisection-key` lexical-key APIs; `bisection-key` 0.0.18 fixes string-key append support.
- Verified synchronization against the editor repository's real `compact.cirru` and `calcit.cirru` fixtures, then checked the output with the legacy dry-run binary.
