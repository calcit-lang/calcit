# Guard transitional Rust-native FFI with a C-safe build handshake

- Embed the exact rustc release/commit, target, debug-assertion mode, and panic
  strategy in the Calcit host build.
- Read an optional static C string from dylibs before invoking legacy Rust ABI
  version or business symbols.
- Reject mismatched identities deterministically; require identity metadata for
  debug hosts while retaining a warned release-only migration path.
- Apply the same check to runtime calls and `caps` native verification, and
  include the identity in native realization receipts and cache keys.
- Add `calcit --ffi-build-id` and document the extension-side build script and
  export.
- Reproduce the former debug-host/release-dylib abort with calcit_wasmtime: the
  guarded host now rejects before `abi_version`; matching release builds still
  return `27`.
