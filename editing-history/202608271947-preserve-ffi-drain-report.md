# Preserve async FFI drain accounting

## Summary

- Kept malformed event-kind failures inside the structured drain report rather
  than returning after a batch had already been removed from the queue.
- Recorded queue-purge failures alongside callback and lifecycle failures.
- Separated discarded batch events from events purged before drain, preserving
  the invariant that every dequeued event is delivered or discarded.

## Validation

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test ffi_async::tests -- --nocapture`
