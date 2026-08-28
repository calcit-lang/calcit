# Asynchronous FFI task protocol

Calcit's long-term native FFI boundary uses a stable C ABI rather than Rust
container, closure, and trait-object layouts. The asynchronous protocol covers
more than one callback: it must represent timers, file watchers, HTTP work,
WebSocket connections, and server resources.

This document describes protocol version 1 as it is implemented in stages.
The current implementation provides the handle types, lifecycle registry,
bounded host event queue, native callback-v1, response/Server capabilities,
and the blocking-v1 entry point used by `&blocking-dylib-edn-fn`. Callback and
blocking calls require their C-safe v1 entry points; Rust ABI fallback has been
removed.

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

## Host shutdown / 宿主关闭

The CLI owns the process Ctrl-C handler. The signal thread only records a
synchronized shutdown request; it never enters the Calcit runtime. A callback
registered by `on-control-c` runs once on the host thread before native task cancellation.
This keeps application cleanup serialized with ordinary callbacks.

Shutdown stops new registrations, moves every live native task and response
capability to `Closing`, rejects open responses with `:host-shutdown`, and
invokes each available module cancel hook. The host continues draining
terminal events for a two-second grace period, so a cooperative module can
still publish its exactly-once `Complete` or `Fail` acknowledgement. At the
deadline the queue closes, then unfinished tasks are purged and released with
diagnostics containing module, method, task handle, kind, age, purged-event
count, and discarded-response count. Closing before forced release prevents a
late producer from racing a reclaimed task handle.

CLI watch loops and the compatibility `async-sleep` task observe the same
shutdown request, so they cannot keep the process alive after native cleanup.
The native evaluator checks the request periodically, including at tail-recur
boundaries, while reload and codegen check between phases. An interrupted
once-mode run still enters bounded async cleanup before returning its error.
The registered `on-control-c` callback temporarily suppresses evaluator
interruption so application cleanup can finish on the host thread.

Calcit CLI 统一拥有进程的 Ctrl-C handler。信号线程只同步记录关闭请求，
不直接进入 Calcit runtime；`on-control-c` 注册的回调会在取消 native
task 之前，由 host thread 串行执行一次。

关闭会停止新注册，把存活的 native task 和 response capability 转为
`Closing`，以 `:host-shutdown` 拒绝未完成 response，并调用模块的
cancel hook。Host 在 2 秒 grace period 内继续 drain terminal event；超时任务
会被强制 purge/release，同时输出 module、method、task handle、kind、age、
purged event 与 discarded response 诊断。在 grace period 内，合作的模块仍可发布
exactly-once `Complete` 或 `Fail`；deadline 到达后先关闭队列再强制 release，
避免迟到 producer 与已回收 task handle 竞态。Watch loop 与兼容性
`async-sleep` 也会观察同一关闭请求，不会在 native cleanup 后继续挂住进程。
Native evaluator 会周期性检查该请求（包括尾递归边界），reload 与 codegen
也会在阶段之间检查。Once-mode 执行被中断后仍会先进入有界 async cleanup，
再返回错误；运行 `on-control-c` 时则临时暂停 evaluator 中断，确保应用清理
可在 host thread 完成。

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
size limit. The CLI host additionally bounds the aggregate queued payload
bytes. Ordinary events cannot consume the event and byte reserves held for
`Complete` and `Fail`, so a saturated stream cannot prevent cancellation or
completion acknowledgement from entering the host loop. Full event or byte
budgets return `QUEUE_FULL` without blocking the producer.
The CLI defaults are 1024 total events and 64 MiB of queued payloads, with 16
events and 64 KiB reserved for terminal traffic. `--trace-ffi` records both
current queue counters on accepted and rejected enqueue attempts.
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

`&call-dylib-edn-fn` requires the following C symbols. Missing symbols are
deterministic migration errors; Calcit does not probe a Rust callback ABI:

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
the start call. The task descriptor and host table must both be copied if the
module needs them after returning. Function pointers remain valid while the
host is running, but the table pointer itself is call-scoped. Modules should
copy only the fields covered by `struct_size` so later hosts can append
functions compatibly.

Callback v1 exposes three host operations. The first publishes events:

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

The module may call `enqueue` from producer threads. Each start receives a
task-bound context; it cannot be reused with another task handle. Calcit
validates the context, handles, kind, pointer, length, and queue capacity,
copies the bytes before returning, and invokes the Calcit callback only while
the CLI host thread drains the queue. The producer retains ownership of its payload and may
reuse or free it after `enqueue` returns. A null pointer is valid only when the
length is zero. No panic, Rust allocator, Rust callback, or executor object
crosses the boundary.

Before its first event, a module may specialize the provisional Stream task
and install its cancellation hook:

```c
int32_t configure_task(
  uint64_t context,
  uint64_t task_handle,
  uint32_t task_kind,
  uint32_t task_flags,
  uint64_t task_context,
  CalcitFfiAsyncCancelFn cancel
);
```

OneShot, Stream, and Server are valid task kinds; Response is host-issued and
cannot be selected here. Configuration after the first event is rejected.
Server tasks must provide a cancel function. `&call-dylib-edn-fn` returns an
opaque native task capability when callback v1 installs a cancel hook (and
keeps returning explicit `&unit` otherwise); `&ffi-task-cancel` invokes this
hook on the host thread. An accepted cancel must eventually enqueue exactly
one `Complete` or `Fail`; repeated cancel while Closing is idempotent.

A Server that declares `REQUIRES_RESPONSE` opens one response capability for
each request before enqueueing it:

```c
int32_t open_response(
  uint64_t context,
  uint64_t task_handle,
  uint64_t response_context,
  uint64_t timeout_ms,
  CalcitFfiAsyncResolveFn resolve,
  uint64_t *out_response_handle
);
```

The timeout must be between 1 millisecond and 24 hours, and one task may keep
at most 1024 responses open concurrently. The resulting
host-issued handle must be attached to exactly one `Emit` from the owning
Server. Missing handles, handles from another Server, response handles on
terminal/ordinary Stream events, and already-resolved or expired handles are
rejected deterministically. Request events are never coalesced; queue-full
leaves the capability active until the module retries or the host rejects it
at timeout.

Calcit appends an opaque AnyRef response capability to the event's decoded EDN
arguments. The callback resolves it with `&ffi-response-resolve` or rejects it
with `&ffi-response-reject`. The host atomically claims the capability, encodes
that value, calls the module's resolve function on the host thread, and
invalidates the capability after the module returns, including when the module
reports an error. Reuse or concurrent resolution therefore cannot invoke the
module twice and fails as a closing/stale generation rather than accidentally
resolving a later request. Timeout, task completion, and callback failure
reject every still-active owned response and release it. A request that
expires while waiting behind other events is rejected and skipped without
terminating its long-lived Server. Deadline and owner indexes make timeout and
task cleanup proportional to the responses being processed rather than all
live FFI handles.
Startup failure discards host handles without calling back into module context
whose ownership never transferred.

AnyRef is deliberately a native non-serializable capability rather than a
Calcit number: the full generation-bearing `u64` cannot safely round-trip
through JavaScript floating-point numbers. A future WASM adapter will expose
the same logical capability through its backend-specific handle value.

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
versioned symbol for this particular method, Calcit reports a deterministic
migration error. The legacy Rust callback ABI fallback has been removed. If a
module advertises an async protocol version other than 1, Calcit fails before
invoking the method.

## Native blocking ABI v1

`&blocking-dylib-edn-fn` probes a separate entry point so a method that owns
the host thread cannot accidentally be started as an asynchronous task:

```c
int32_t <method>_calcit_ffi_blocking_v1(
  const uint8_t *request_ptr,
  size_t request_len,
  const CalcitFfiAsyncTaskV1 *task,
  const CalcitFfiBlockingHostV1 *host,
  CalcitFfiBuffer *output
);
```

It uses the same `calcit_ffi_async_version() -> 1`, generation handle,
OneShot lifecycle, monotonic event sequence, status values, and Cirru EDN
request encoding as callback v1. The final method result follows buffer v1:
the module allocates `output`, and the host copies it before calling that
module's `calcit_ffi_buffer_free`.

The blocking host table is C-layout and contains three operations:

```c
typedef struct {
  uint8_t *ptr;
  size_t len;
  size_t cap;
} CalcitFfiBuffer;

typedef struct {
  uint32_t protocol_version;
  uint32_t struct_size;
  uint64_t context;
  int32_t (*invoke)(
    uint64_t context,
    uint64_t task_handle,
    const uint8_t *payload_ptr,
    size_t payload_len,
    CalcitFfiBuffer *output
  );
  int32_t (*finish)(uint64_t context, uint64_t task_handle);
  int32_t (*free_buffer)(
    uint64_t context,
    uint64_t task_handle,
    CalcitFfiBuffer buffer
  );
} CalcitFfiBlockingHostV1;
```

The function pointer signatures are equivalent to:

```c
int32_t invoke(
  uint64_t context,
  uint64_t task_handle,
  const uint8_t *payload_ptr,
  size_t payload_len,
  CalcitFfiBuffer *output
);

int32_t finish(uint64_t context, uint64_t task_handle);

int32_t free_buffer(
  uint64_t context,
  uint64_t task_handle,
  CalcitFfiBuffer buffer
);
```

`invoke` accepts a Cirru EDN argument list and runs the Calcit callback
synchronously. It is valid only on the thread that registered the blocking
task; a foreign-thread invocation returns `WRONG_THREAD` and never enters the
Calcit runtime. Successful callback output is one Cirru EDN value. A callback
failure returns `CALLBACK_ERROR` with a UTF-8 diagnostic buffer. The host owns
both forms of callback output, records their exact pointer and length, and
releases them only through `free_buffer`; forged metadata, duplicate free, and
cross-task context are rejected. Any unfreed callback buffers are reclaimed
and reported when the blocking method returns.

`finish` is optional and exactly once. If the module does not call it, normal
or failed return from the blocking method performs an implicit finish. After
explicit finish, further `invoke` calls fail deterministically. This preserves
main-thread event loops without introducing a queue-drain deadlock, while
foreign-thread and long-lived work remains on callback v1's bounded queue.

Missing protocol or per-method blocking symbols are deterministic migration
errors. An advertised incompatible version, missing module buffer free
function, malformed output, leaked host callback buffer, or wrong-thread
callback is a hard error.

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

## Rollout status

The maintained native ecosystem has migrated to C-safe v1 protocols. The
primary modules (`calcit-fetch`, `calcit-http`, `calcit-wss`, and `calcit.std`)
and the additional audited modules, including `calcit-paint`, no longer require
Rust-layout entry points. See issues #474 and #482 for the acceptance matrix
and migration history.
