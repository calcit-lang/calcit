# Add the synchronous FFI byte-buffer protocol

- Added a versioned C ABI for synchronous FFI calls using owned UTF-8 Cirru EDN buffers.
- Made method migration incremental by preferring `<method>_calcit_ffi_v1` and retaining the build-ID-guarded Rust ABI as a fallback.
- Kept output ownership in the dylib through `calcit_ffi_buffer_free`, with host-side invariant and size checks before decoding.
- Added focused tests for protocol versioning, symbol derivation, and request encoding; callback migration remains a separate phase.
