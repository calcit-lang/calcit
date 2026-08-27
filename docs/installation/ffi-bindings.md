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

Rust supports extending with dynamic libraries. A demo project can be found at https://github.com/calcit-lang/dylib-workflow

Currently two APIs are supported, based on Cirru EDN data.

First one is a synchronous [Edn](https://github.com/Cirru/cirru-edn.rs) API with type signature:

```rust
#[unsafe(no_mangle)]
pub fn demo(args: Vec<Edn>) -> Result<Edn, String> {
}
```

The legacy asynchronous API can be called multiple times and relies on Rust
`Arc` trait objects:

```rust
#[unsafe(no_mangle)]
pub fn demo(
  args: Vec<Edn>,
  handler: Arc<dyn Fn(Vec<Edn>) -> Result<Edn, String> + Send + Sync + 'static>,
  finish: Box<dyn FnOnce() + Send + Sync + 'static>,
) -> Result<Edn, String> {
}
```

in this snippet, the function `handler` is used as the callback, which could be called multiple times.

The function `finish` is used for indicating that the task has finished. It can be called once, or not being called.
Internally Calcit tracks with a counter to see if all asynchorous tasks are finished.
Process need to keep running when there are tasks running.

New callback methods should instead implement the C-safe asynchronous protocol
described below. The Rust form remains only as a per-method migration fallback.

Rust's native ABI has no stability guarantee. `Vec<Edn>`, `String`, `Result`, and callback trait objects are therefore a transitional interface rather than a portable dylib protocol. Calcit checks a C-safe build identity before invoking those Rust ABI symbols.

Add a `build.rs` that derives the identity from the exact compiler, target, debug-assertion mode, and panic strategy:

```rust
use std::{env, process::Command};

fn field<'a>(output: &'a str, name: &str) -> &'a str {
  output.lines().find_map(|line| line.strip_prefix(name).map(str::trim)).unwrap()
}

fn main() {
  let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
  let output = Command::new(rustc).args(["--version", "--verbose"]).output().unwrap();
  assert!(output.status.success());
  let verbose = String::from_utf8(output.stdout).unwrap();
  let release = field(&verbose, "release:");
  let commit = field(&verbose, "commit-hash:");
  let target = env::var("TARGET").unwrap();
  let debug_assertions = env::var_os("CARGO_CFG_DEBUG_ASSERTIONS").is_some();
  let panic_strategy = env::var("CARGO_CFG_PANIC").unwrap();
  println!(
    "cargo:rustc-env=CALCIT_FFI_BUILD_ID=rustc={release}:{commit};target={target};debug-assertions={debug_assertions};panic={panic_strategy}"
  );
}
```

Export the resulting value through a null-terminated C string. This lookup is safe to perform before any Rust-layout-dependent value crosses the library boundary:

```rust
use std::ffi::c_char;

static FFI_BUILD_ID: &[u8] = concat!(env!("CALCIT_FFI_BUILD_ID"), "\0").as_bytes();

#[unsafe(no_mangle)]
pub extern "C" fn calcit_ffi_build_id() -> *const c_char {
  FFI_BUILD_ID.as_ptr().cast()
}
```

The existing Rust ABI version functions are still required during the migration period:

```rust
#[unsafe(no_mangle)]
pub fn abi_version() -> String {
  String::from("0.0.9")
}

#[unsafe(no_mangle)]
pub fn edn_version() -> String {
  cirru_edn::version().to_owned()
}
```

`abi_version()` must match Calcit's FFI ABI version exactly.

`edn_version()` must match the exact `cirru_edn` crate version used by the running Calcit binary. If either version differs, Calcit aborts the FFI call before invoking the target symbol.

The build identity must also match exactly. Debug Calcit hosts reject dylibs that do not export `calcit_ffi_build_id`, which prevents the common debug-host/release-dylib crash before even calling `abi_version()`. Release hosts temporarily retain the legacy path with a warning so maintained modules can migrate incrementally. A matching identity reduces accidental incompatibility; it does not turn Rust's native ABI into a stable public protocol.

Inspect the host side before building or debugging a module:

```bash
calcit --ffi-build-id
```

## C-safe synchronous buffer ABI

New synchronous methods should use buffer protocol version 1. Calcit first looks for `<method>_calcit_ffi_v1`; only methods without that symbol fall back to the build-ID-guarded Rust ABI. Existing Calcit source calls do not change.

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

`calcit-lang/calcit_wasmtime` contains the first complete synchronous adapter.

## C-safe asynchronous callback ABI

`&call-dylib-edn-fn` now probes `<method>_calcit_ffi_async_v1` before using the
guarded legacy Rust callback. A callback-v1 module exports
`calcit_ffi_async_version() -> 1`, accepts a C-layout task descriptor and host
function table, and publishes byte payloads through the host's `enqueue`
function. Foreign producer threads only enqueue; Calcit copies the payload and
runs callbacks on its host thread.

`Emit` payloads are Cirru EDN argument lists. Successful completion must carry
the explicit `&unit` value, and `Fail` carries a Cirru EDN diagnostic that is
surfaced to the console. Missing version or per-method symbols retain the
legacy fallback; an advertised incompatible version is an error. See
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
task implicitly. Missing per-method blocking symbols retain the guarded legacy
fallback during migration. See
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
