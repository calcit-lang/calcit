# Scoped byte-stream contract

- Bump the Component Interface IR to v4 and add the closed `readable-byte-stream` kind.
- Restrict the new capability to direct parameters of async Component exports; imports, synchronous calls, nesting, and returned streams fail closed.
- Document that the contract represents scoped `stream<u8>` consumption without exposing a copyable raw handle.
- Keep the core WASM adapter unsupported until the bounded read/cancel/drop state machine is implemented.
