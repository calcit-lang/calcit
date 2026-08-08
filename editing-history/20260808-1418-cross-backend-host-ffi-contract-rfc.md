# Cross-backend Host/FFI Contract RFC

- Designed a backend-neutral logical FFI contract shared by JavaScript, native registered procedures, WASM, and WASI.
- Separated logical host types and callable checks from backend-specific ABI transport, ownership, symbol binding, and memory layout.
- Proposed nominal stable host shapes, with JavaScript/DOM as the first field and method projection consumer.
- Kept raw host values opaque and required declarations, checked decoders, registered identity, or auditable trusted assertions for stronger host types.
- Preserved existing registered-proc and `defwasm-import`/`defwasm-export` syntax as incremental migration inputs.
