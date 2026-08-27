# Asynchronous FFI task protocol

Calcit's long-term native FFI boundary uses a stable C ABI rather than Rust
container, closure, and trait-object layouts. The asynchronous protocol covers
more than one callback: it must represent timers, file watchers, HTTP work,
WebSocket connections, and server resources.

This document describes protocol version 1 as it is implemented in stages.
The current implementation provides the handle types, lifecycle registry,
bounded host event queue, and the native callback-v1 entry point used by
`&call-dylib-edn-fn`. `&blocking-dylib-edn-fn` still uses the
build-identity-guarded Rust ABI until the blocking protocol lands.

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
1. The host reserves the sequence only after queue capacity has been secured,
so `QUEUE_FULL` does not consume a sequence or claim a terminal event. The
bounded queue accepts producers from foreign threads, but only the thread that
created it may wait or drain and execute Calcit callbacks. A dylib
watcher/server thread may enqueue bytes but must never enter the Calcit runtime
directly.

An event uses one of three stable tags: repeating `Emit`, successful terminal
`Complete`, or failed terminal `Fail`. At most one terminal event may be queued
for a task. Cancellation changes the task to `Closing`; already queued ordinary
events are then discarded, while the terminal acknowledgement is still
delivered. Terminal dispatch marks the task `Finished` before it can be
released.

Queue capacity is measured in events and every payload also has a host-side
size limit. Full queues return `QUEUE_FULL` without blocking the producer.
Only `Emit` events on a task registered with `COALESCE_ALLOWED` may replace an
older queued emit for that same task; the replacement carries a newer sequence
and the `COALESCED` event flag. Complete/fail and server request events are
never silently dropped or coalesced.

Drain returns a structured report rather than printing and forgetting
failures. A callback failure records task/sequence metadata and its message,
finishes the failed task, and purges its remaining queued events. Lifecycle
rejections are reported separately. Queue entries also retain producer thread
and queue-delay metadata for `--trace-ffi` without logging complete payloads.

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

`FfiAsyncEventDescriptor` adds the event kind, event flags, task and optional
response handles, sequence, and payload length. It contains no Rust pointer;
the native C host function table and a WASM linear-memory adapter can wrap the
same descriptor with different byte-copying transports.

Status values are integer constants. Foreign input is validated before it is
converted to a Rust enum, avoiding invalid-discriminant undefined behavior.
Business payloads will continue to use UTF-8 Cirru EDN buffers, with allocator
ownership and matching free functions explicit in both directions.

## Native callback ABI v1

Before using the legacy Rust callback ABI, `&call-dylib-edn-fn` probes the
following C symbols:

```c
uint32_t calcit_ffi_async_version(void); /* returns 1 */

int32_t <method>_calcit_ffi_async_v1(
  const uint8_t *request_ptr,
  size_t request_len,
  const CalcitFfiAsyncTaskV1 *task,
  const CalcitFfiAsyncHostV1 *host
);
```

The request is a UTF-8 Cirru EDN list and is readable only for the duration of
the start call. The task descriptor contains the host-issued handle and must
be copied if the module needs it after returning. The host table has process
lifetime, although modules should copy only the fields covered by
`struct_size` so later hosts can append functions compatibly.

Callback v1 currently exposes one host operation:

```c
int32_t enqueue(
  uint64_t context,
  uint64_t task_handle,
  uint32_t event_kind,
  uint64_t response_handle,
  const uint8_t *payload_ptr,
  size_t payload_len
);
```

The module may call `enqueue` from producer threads. Calcit validates the
context, handles, kind, pointer, length, and queue capacity, copies the bytes
before returning, and invokes the Calcit callback only while the CLI host
thread drains the queue. The producer retains ownership of its payload and may
reuse or free it after `enqueue` returns. A null pointer is valid only when the
length is zero. No panic, Rust allocator, Rust callback, or executor object
crosses the boundary.

Payload rules are explicit:

- `Emit` (`1`) carries a Cirru EDN list whose elements become callback
  arguments;
- `Complete` (`2`) carries exactly the explicit unit value `&unit` (surrounding
  whitespace is allowed); it never uses null/nil as success;
- `Fail` (`3`) carries one Cirru EDN diagnostic value and is reported to the
  console with task and sequence metadata.

The start function returns `0` after it has accepted responsibility for the
task. A nonzero result makes the host purge startup events and reclaim the
handle. Host calls return the stable `async_status` integer codes, including
invalid/stale handle, closing/finished task, host closing, queue full, invalid
payload, and internal error.

If a dylib does not export `calcit_ffi_async_version`, or does not export the
versioned symbol for this particular method, Calcit falls back to the guarded
Rust callback ABI. If it advertises an async protocol version other than 1,
Calcit fails before invoking either ABI. This per-method rule lets maintained
modules migrate incrementally without hiding an actual version mismatch.

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

The remaining implementation order after native callback v1 is:

1. response handles, server backpressure, cancellation, timeout, and shutdown
   fixtures;
2. blocking calls reusing the same registry and envelopes;
3. migration and release of `calcit-fetch`, `calcit-http`, `calcit-wss`, and
   `calcit.std`;
4. a `calcit_wasmtime` adapter prototype after the event envelope is stable;
5. removal of the guarded Rust ABI fallback under issue #474.

See issue #482 for the full acceptance matrix and module rollout status.
