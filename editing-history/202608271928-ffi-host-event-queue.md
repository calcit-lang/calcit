# Async FFI host event queue

## Summary

- Added a C-layout event descriptor for repeating emit and terminal
  complete/fail events.
- Added a bounded multi-producer queue that only its creating host thread may
  wait on or drain into the Calcit runtime.
- Kept sequence reservation atomic with queue admission, made terminal events
  exactly once, and limited coalescing to explicitly opted-in stream events.
- Returned callback and lifecycle failures as structured drain diagnostics;
  callback failure finishes the task and purges its remaining queued events.
- Retained producer-thread and queue-delay metadata for low-payload FFI traces.

## Compatibility constraint

Existing Rust callback and blocking dylib calls remain unchanged. This queue
is the scheduling foundation for the next C host-function-table PR; native
threads and a future WASM polling adapter share descriptors and lifecycle
semantics without sharing pointers or callback layouts.

## Validation

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -- --test-threads=1`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`
