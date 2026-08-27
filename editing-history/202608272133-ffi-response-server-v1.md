# Async FFI response and server protocol v1

## Goal

Extend native callback-v1 with cancellable Server tasks and exactly-once
response capabilities without exposing generation handles as lossy Calcit
numbers or allowing foreign threads to enter the runtime.

## Changes

- appended C-safe task configuration and response registration functions to
  the versioned host table;
- allowed modules to declare OneShot, Stream, or Server semantics before the
  first event and install a Server cancellation hook;
- returned opaque task capabilities from callback-v1 methods that install a
  cancel hook, preserved `&unit` for non-cancellable methods, and added
  `&ffi-task-cancel`;
- issued opaque AnyRef response capabilities, appended them to request callback
  arguments, and added `&ffi-response-resolve` / `&ffi-response-reject`;
- enforced response owner, kind, active lifecycle, required-request, timeout,
  and non-coalescing queue rules;
- rejected and released unresolved responses on timeout or owner completion,
  while startup rollback discards handles without entering untransferred module
  context;
- added registry configuration/snapshot, response queue invariants,
  exactly-once/stale reuse, timeout, cross-server, and foreign producer tests;
- documented the expanded C ABI, Calcit capability API, ownership, timeout,
  cancellation, backpressure, and WASM representation boundary.

## Runtime verification

A standalone C dylib configured a cancellable Server, opened a response,
published a request from a pthread, received the Calcit response on the host
thread, enqueued explicit `&unit` completion, and released with pending task
count zero. A second idle Server fixture verified `&ffi-task-cancel` and its
terminal acknowledgement path.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -- --test-threads=1`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`
