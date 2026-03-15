# Rust bindings

> API status: unstable.

Rust supports extending with dynamic libraries. A demo project can be found at https://github.com/calcit-lang/dylib-workflow

Currently two APIs are supported, based on Cirru EDN data.

First one is a synchronous [Edn](https://github.com/Cirru/cirru-edn.rs) API with type signature:

```rust
#[no_mangle]
pub fn demo(args: Vec<Edn>) -> Result<Edn, String> {
}
```

The other one is an asynchorous API, it can be called multiple times, which relies on `Arc` type(not sure if we can find a better solution yet),

```rust
#[no_mangle]
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

Also to declare runtime compatibility, FFI dylibs need two extra functions with specific names so that Calcit could check before actually calling them:

```rust
#[no_mangle]
pub fn abi_version() -> String {
  String::from("0.0.9")
}

#[no_mangle]
pub fn edn_version() -> String {
  cirru_edn::version().to_owned()
}
```

`abi_version()` must match Calcit's FFI ABI version exactly.

`edn_version()` must match the exact `cirru_edn` crate version used by the running Calcit binary. If either version differs, Calcit aborts the FFI call before invoking the target symbol.

### Call in Calcit

Rust code is compiled into dylibs, and then Calcit could call with:

```cirru.no-check
&call-dylib-edn (get-dylib-path "\"/dylibs/libcalcit_std") "\"read_file" name
```

first argument is the file path to that dylib. And multiple arguments are supported:

```cirru.no-check
&call-dylib-edn (get-dylib-path "\"/dylibs/libcalcit_std") "\"add_duration" (nth date 1) n k
```

calling a function is special, we need another function, with last argument being the callback function:

```cirru.no-check
&call-dylib-edn-fn (get-dylib-path "\"/dylibs/libcalcit_std") "\"set_timeout" t cb
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
