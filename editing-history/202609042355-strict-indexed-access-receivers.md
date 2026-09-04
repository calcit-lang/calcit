# Strict indexed-access receivers

- Strict typed preprocessing now rejects concrete unsupported receivers for public `first`, `last`, `nth`, and `get` calls with `E_UNSUPPORTED_INDEXED_RECEIVER`.
- The check runs only after arity and static receiver resolution are available. Supported List/String/Enum receivers and Map `get` continue through the existing Option-returning specialization.
- Compatibility mode and deliberately explicit Dynamic/unresolved boundaries remain on the runtime path, preserving incremental migration.
- Diagnostics point Struct callers to `(:field value)`, optional callers to narrowing/unwrapping, and `JsObject` callers to FFI validation/conversion.
- Rust regression coverage locks shared preprocessing state and verifies helper-level compatibility, strict rejection, explicit Dynamic behavior, and the complete backend-independent check-only preprocessing path shared by native and JS.
