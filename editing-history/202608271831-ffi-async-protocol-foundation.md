# Async FFI protocol foundation

## Summary

- Defined a versioned, C-safe async task descriptor for one-shot, stream,
  server, and response handles without changing the existing synchronous FFI.
- Added a thread-safe generational handle registry with ordered event
  sequences, explicit close/finish/release transitions, and host shutdown.
- Retired exhausted generation slots so stale handles cannot become valid
  again after counter wraparound.
- Documented the transport-neutral lifecycle intended for timers, watchers,
  HTTP/WebSocket servers, and a future WASM adapter.

## Compatibility constraint

Native libraries must enqueue events for the Calcit host instead of invoking
Calcit callbacks from foreign threads. The protocol models stable integer
handles and byte payloads; a WASM adapter may map them to imports/exports
without reproducing native pointers or function pointers.

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test -- --test-threads=1`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`
