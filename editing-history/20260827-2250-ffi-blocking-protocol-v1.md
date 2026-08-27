# C-safe blocking FFI protocol v1

## Goal

Remove Rust closures, trait objects, `Vec<Edn>`, `Result`, and `FnOnce` from
the supported `&blocking-dylib-edn-fn` boundary while preserving methods that
must own and call back on the Calcit host thread.

## Protocol decisions

- use a distinct `<method>_calcit_ffi_blocking_v1` symbol so blocking and
  asynchronous starts cannot be confused;
- reuse `calcit_ffi_async_version() -> 1`, the generation task registry,
  OneShot lifecycle, serial event sequence, task descriptor, status codes, and
  Cirru EDN request encoding;
- invoke the Calcit callback directly only on the task's owner thread instead
  of enqueueing work that cannot drain while the dylib owns that thread;
- let modules finish explicitly exactly once, with method return providing the
  implicit finish when no explicit finish occurred;
- keep callback result bytes host-owned and indexed by exact pointer/length;
  modules release them through the task-bound host `free_buffer` function;
- report foreign-thread access, callback failures, forged/duplicate frees, and
  leaked host buffers even if the module ignores the returned status;
- probe the migrated method before allocating runtime task state, preserving
  the build-ID-guarded per-method Rust ABI fallback for unmigrated modules;
- make legacy blocking task release exactly once on explicit finish or method
  return, fixing the previous pending-task leak on error paths.

## Tests

- C-layout host table and versioned method name;
- inline callback execution and host-owned EDN result;
- exact buffer metadata and duplicate-free rejection;
- owner-thread enforcement;
- explicit finish exactly once and callback rejection after finish.

## Native verification

An optimized standalone C dylib was loaded by the debug Calcit host and
verified both explicit and implicit finish. Separate methods confirmed that a
foreign pthread callback, an unfreed callback buffer, and a Calcit callback
error all fail deterministically and release the pending task.

## Full verification

- `cargo fmt -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test` (864 Rust tests)
- `yarn compile`
- `yarn check-agent-interface` (17/17)
- `yarn check-all` (Calcit core 221/221, JS, IR, WASM and runtime benchmarks)
- `bash scripts/check-docs-md.sh` (57 files, 325 blocks)
