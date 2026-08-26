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

The other one is an asynchorous API, it can be called multiple times, which relies on `Arc` type(not sure if we can find a better solution yet),

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

Asynchronous tasks are based on threads, which is currently decoupled from core features of Calcit. We may need techniques like `tokio` for better performance in the future, but current solution is quite naive yet.

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

The build identity must also match exactly. Debug Calcit hosts reject dylibs that do not export `calcit_ffi_build_id`, which prevents the common debug-host/release-dylib crash before even calling `abi_version()`. Release hosts temporarily retain the legacy path with a warning so maintained modules can migrate incrementally. A matching identity reduces accidental incompatibility; it does not turn Rust's native ABI into a stable public protocol. New FFI designs should use the planned C-safe serialized byte-buffer interface instead.

Inspect the host side before building or debugging a module:

```bash
calcit --ffi-build-id
```

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
