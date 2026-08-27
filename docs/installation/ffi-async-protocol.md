# Asynchronous FFI task protocol

Calcit's long-term native FFI boundary uses a stable C ABI rather than Rust
container, closure, and trait-object layouts. The asynchronous protocol covers
more than one callback: it must represent timers, file watchers, HTTP work,
WebSocket connections, and server resources.

This document describes protocol version 1 as it is implemented in stages.
The current implementation provides the handle types and lifecycle registry;
existing `&call-dylib-edn-fn` and `&blocking-dylib-edn-fn` calls still use the
build-identity-guarded Rust ABI until the event queue and C host function table
land.

## Shared task model

The protocol has four handle roles:

- `OneShot`: timer, HTTP client request, or other single completion;
- `Stream`: interval, file watcher, or WebSocket connection with repeated
  events;
- `Server`: listener/accept resource that produces child request or connection
  events;
- `Response`: an exactly-once capability for an HTTP response, WebSocket
  upgrade, or similar reply.

Every handle is a non-zero `u64`. Its low 32 bits identify a registry slot and
its high 32 bits contain a generation. Reusing a released slot changes the
generation, so a late event cannot address the new task accidentally. Neither
native modules nor future WASM guests receive a Rust object address.

The lifecycle is:

```text
register -> Active -> Closing -> Finished -> release
                    \----------> Finished -> release
```

Cancellation and host shutdown move an active task to `Closing`. New events
are rejected from that point, while a final completion acknowledgement remains
allowed. `finish` and `release` are exactly-once operations; a finished
tombstone remains until release so duplicate completion has a deterministic
error instead of looking like an unknown handle.

## Events and ordering

Each active handle owns a monotonically increasing event sequence starting at
1. The host reserves the sequence before enqueueing an event. The next stage
will add the bounded host event queue and will execute Calcit callbacks only on
the host scheduling thread. A dylib watcher/server thread may enqueue bytes but
must never enter the Calcit runtime directly.

Streams and servers therefore do not require a Rust closure to cross the ABI.
They keep only the opaque task handle and the C host function table. HTTP
request events will carry a separate `Response` handle, allowing a Calcit
callback to respond later while preserving exactly-once response and timeout
rules.

## Stable transport fields

`FfiAsyncTaskDescriptor` is a C-layout structure containing:

- protocol version and structure size;
- the raw `u64` handle;
- a fixed numeric handle kind;
- capability flags such as serialized events, permitted coalescing, or a
  required response.

Status values are integer constants. Foreign input is validated before it is
converted to a Rust enum, avoiding invalid-discriminant undefined behavior.
Business payloads will continue to use UTF-8 Cirru EDN buffers, with allocator
ownership and matching free functions explicit in both directions.

## WASM compatibility boundary

WASM will not reuse C pointers or native function pointers. It can still reuse
the protocol semantics:

- transport the handle as `i64`, or two `i32` values where required;
- exchange UTF-8 Cirru EDN through guest linear memory;
- submit event/finish/respond through imports and receive work through exports
  or polling;
- use the same kind values, flags, lifecycle, generation checks, event
  sequence, and status codes.

The shared model deliberately does not depend on Rust allocators, trait
objects, thread-local state, an executor/waker ABI, or native threads. Native
threaded producers and a future single-threaded WASM poll adapter can therefore
present the same Calcit-facing behavior.

## Rollout

The remaining implementation order is:

1. bounded event queue, scheduler-thread draining, trace events, and propagated
   callback failures;
2. native C host function table and per-method callback v1 lookup;
3. response handles, server backpressure, cancellation, timeout, and shutdown
   fixtures;
4. blocking calls reusing the same registry and envelopes;
5. migration and release of `calcit-fetch`, `calcit-http`, `calcit-wss`, and
   `calcit.std`;
6. a `calcit_wasmtime` adapter prototype after the event envelope is stable;
7. removal of the guarded Rust ABI fallback under issue #474.

See issue #482 for the full acceptance matrix and module rollout status.
