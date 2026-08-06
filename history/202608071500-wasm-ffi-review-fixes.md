# WASM FFI review fixes

- Hardened the Node WASM-string reader and corrected host-side string allocation to reserve its header, length and aligned payload.
- Validated `defwasm-import` declarations before preprocessing and rejected optional/rest import parameters, while recording declared arity for direct calls.
- Preserved outer JavaScript await detection after nested function declarations and aligned macro-schema validation with the new WASM declaration forms.
- Moved WASM tag-index diagnostics to stderr and corrected the runtime type-section documentation.
