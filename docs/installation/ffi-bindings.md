---
title: "Rust bindings"
scope: "core"
kind: "reference"
category: "installation"
aliases:
  - "ffi"
  - "rust bindings"
  - "native bindings"
---
# Rust bindings

> API status: unstable.

Rust supports extending Calcit with dynamic libraries. A complete project can
be found at https://github.com/calcit-lang/dylib-workflow.

Only versioned C-safe protocols are supported. Dynamic libraries must not
export business methods that pass Rust `Vec`, `String`, `Result`, `AnyRef`,
`Arc<dyn Fn>`, or `FnOnce` values across the boundary. Those layouts, vtables,
allocators, and drop implementations are compiler-specific and are no longer
probed or invoked by Calcit.

The supported boundaries are:

- synchronous byte-buffer protocol v1;
- asynchronous task/callback protocol v1;
- blocking callback protocol v1;
- opaque-resource protocol v1 for reusable native objects.

Each protocol advertises a fixed `extern "C"` version function. Missing
version or per-method symbols are deterministic migration errors; there is no
Rust-ABI fallback. Native modules should use `cdylib` and export only fixed C
ABI symbols.

Rust module authors should use
[`calcit_native_ffi`](https://crates.io/crates/calcit_native_ffi) instead of
copying protocol structs and transport adapters into each repository. The
crate provides the v1 descriptors, status/task/event constants, buffer
ownership helpers, Cirru EDN transport, backpressure policy, async/blocking
host helpers, and final-dylib export macros. This document remains the
normative host protocol; the shared crate is the maintained Rust
implementation for modules.

Calcit itself consumes the same crate with default features disabled for raw
protocol versions, symbols, function signatures, resource-token constants,
and C-layout descriptors. Dynamic loading, task/resource registries, callback
queues, dylib pinning, and lifecycle enforcement remain runtime-owned.

Rust 模块作者应优先使用
[`calcit_native_ffi`](https://crates.io/crates/calcit_native_ffi)，不要在每个
仓库复制 protocol struct 和 transport adapter。该 crate 统一提供 v1
descriptor、status/task/event 常量、buffer ownership、Cirru EDN transport、
backpressure、async/blocking host helper 以及 final-dylib export macro。本页
仍是 host protocol 的规范定义，共享 crate 是模块侧维护的 Rust 实现。

Calcit runtime 也通过关闭默认 feature 直接消费同一 crate 的 raw protocol
version、symbol、function signature、resource token 常量与 C-layout descriptor。
动态加载、task/resource registry、callback queue、dylib pin 与 lifecycle 检查
仍由 runtime 维护。

## C-safe synchronous buffer ABI

Synchronous methods use buffer protocol version 1. Calcit looks for
`<method>_calcit_ffi_v1`; missing protocol/version/free/method symbols are
migration errors. Existing Calcit source calls do not change.

The dylib exports these C ABI symbols:

```rust
#[repr(C)]
pub struct CalcitFfiBuffer {
  pub ptr: *mut u8,
  pub len: usize,
  pub cap: usize,
}

#[unsafe(no_mangle)]
pub extern "C" fn calcit_ffi_buffer_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub unsafe extern "C" fn calcit_ffi_buffer_free(buffer: CalcitFfiBuffer) {
  // Reconstruct and drop the Vec in the dylib that allocated it.
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn read_file_calcit_ffi_v1(
  request_ptr: *const u8,
  request_len: usize,
  output: *mut CalcitFfiBuffer,
) -> i32 {
  // Decode, call the implementation, and write one owned output buffer.
  0
}
```

Protocol rules:

- Input is one UTF-8 Cirru EDN list containing the method arguments. The host owns it for the duration of the synchronous call.
- Status `0` means the output is one UTF-8 Cirru EDN value. A nonzero status means the output is a UTF-8 error message.
- The dylib allocates every output and Calcit copies it before calling that same dylib's `calcit_ffi_buffer_free` exactly once.
- The adapter must contain panics and return an error status; unwinding across `extern "C"` is invalid.
- Calcit rejects protocol-version mismatches, malformed buffer metadata, oversized responses, invalid UTF-8, and invalid response EDN.

`calcit-lang/calcit_wasmtime` contains a complete synchronous adapter and uses
the shared crate starting with version 0.1.5.

Methods that create reusable native objects use the C-safe
[opaque resource protocol](ffi-resource-protocol.md). The host turns reserved
buffer-v1 tokens into automatically released Calcit `AnyRef` values, validates
module ownership on later calls, and pins the creating dylib until the final
reference is dropped.

## C-safe asynchronous callback ABI

`&call-dylib-edn-fn` requires `<method>_calcit_ffi_async_v1`. A callback-v1 module exports
`calcit_ffi_async_version() -> 1`, accepts a C-layout task descriptor and host
function table, and publishes byte payloads through the host's `enqueue`
function. Foreign producer threads only enqueue; Calcit copies the payload and
runs callbacks on its host thread.

An async-only module does not need to export the buffer protocol or
`calcit_ffi_buffer_free`. Those symbols are required only by synchronous buffer
methods and blocking methods whose final output is allocated by the module.

`Emit` payloads are Cirru EDN argument lists. Successful completion must carry
the explicit `&unit` value, and `Fail` carries a Cirru EDN diagnostic that is
surfaced to the console. Missing version or per-method symbols are migration
errors; an advertised incompatible version is a protocol mismatch. See
[Asynchronous FFI task protocol](ffi-async-protocol.md) for the exact C
signatures, ownership, lifecycle, status, queue, and future WASM rules.

Callback-v1 calls that install a cancel hook return an opaque native task
capability rather than nil or a floating-point handle. Non-cancellable calls
continue to return explicit `&unit`. Long-running tasks can be stopped
explicitly:

```cirru.no-check
let
    task $ &call-dylib-edn-fn lib-path |serve on-request
  &ffi-task-cancel task :shutdown
```

Ctrl-C performs the same cancellation at host scope. A function registered
with `on-control-c` runs on the Calcit host thread before the runtime starts
the two-second native-task shutdown grace period; the signal thread itself
never runs Calcit code.

For a Server request carrying a response handle, Calcit appends an opaque
response capability after the decoded request arguments. It is exactly-once:

```cirru.no-check
defn on-request (method path response!)
  if (= path |/health)
    &ffi-response-resolve response! $ {} (:status 200) (:body |ok)
    &ffi-response-reject response! $ {} (:status 404) (:body |missing)
```

The host validates task-bound context, ownership, and deadline, atomically
claims the capability, invokes the dylib resolver on the Calcit host thread,
and invalidates it after the attempt. Unresolved requests are rejected on
timeout or when their owning task finishes; a queued request that times out is
skipped without terminating the Server.

## C-safe blocking callback ABI

`&blocking-dylib-edn-fn` probes `<method>_calcit_ffi_blocking_v1`. This entry
point reuses the async protocol version, generation task handle, lifecycle,
sequence, and Cirru EDN payload rules, but invokes the Calcit callback directly
on the host thread instead of waiting for the asynchronous queue to drain.
Foreign-thread invocation is rejected.

Callback results are allocated and tracked by the host and must be returned
through the blocking host table's `free_buffer`; the method's final output is
allocated by the module and released through `calcit_ffi_buffer_free`.
`finish` may be called explicitly once, otherwise method return finishes the
task implicitly. Missing protocol or per-method blocking symbols are migration
errors. See
[Asynchronous FFI task protocol](ffi-async-protocol.md#native-blocking-abi-v1)
for the C signatures and ownership rules.

### Call in Calcit

Rust code is compiled into dylibs, and then Calcit could call with:

```cirru.no-check
&call-dylib-edn (get-dylib-path "|/dylibs/libcalcit_std") "|read_file" name
```

first argument is the file path to that dylib. And multiple arguments are supported:

```cirru.no-check
&call-dylib-edn (get-dylib-path "|/dylibs/libcalcit_std") "|add_duration" (nth date 1) n k
```

calling a function is special, we need another function, with last argument being the callback function:

```cirru.no-check
&call-dylib-edn-fn (get-dylib-path "|/dylibs/libcalcit_std") "|set_timeout" t cb
```

Notice that both functions call dylibs and then library instances are cached, for better consistency and performance, with some cost in memory occupation. Linux and MacOS has different strategies loading dylibs while loaded repeatedly, so Calcit just cached them and only load once.

### Extensions

Currently there are some early extensions:

- [Std](https://github.com/calcit-lang/calcit.std) - some collections of util functions
- [WebSocket server binding](https://github.com/calcit-lang/calcit-wss)
- [Regex](https://github.com/calcit-lang/calcit-regex/)
- [HTTP client binding](https://github.com/calcit-lang/calcit-fetch)
- [HTTP server binding](https://github.com/calcit-lang/calcit-http)
- [Wasmtime binding](https://github.com/calcit-lang/calcit_wasmtime)
- [fswatch](https://github.com/calcit-lang/calcit-fswatch)

The first shared-crate rollout is available in `calcit-http 0.3.8`,
`calcit-wss 0.2.17`, `calcit-fetch 0.0.17`, `calcit_wasmtime 0.1.5`, and
`calcit.std 0.2.22`.

首批共享 crate 迁移版本为 `calcit-http 0.3.8`、`calcit-wss 0.2.17`、
`calcit-fetch 0.0.17`、`calcit_wasmtime 0.1.5` 和 `calcit.std 0.2.22`。

The second rollout is available in `calcit-json 0.0.14`,
`calcit-clipboard 0.0.10`, `calcit-command 0.0.6`, and `calcit-regex 0.0.15`.
Regex keeps its module-owned opaque-resource registry while sharing the
buffer-v1 transport adapter.

第二批迁移版本为 `calcit-json 0.0.14`、`calcit-clipboard 0.0.10`、
`calcit-command 0.0.6` 和 `calcit-regex 0.0.15`。Regex 继续由模块维护
opaque-resource registry，只共享 buffer-v1 transport adapter。
