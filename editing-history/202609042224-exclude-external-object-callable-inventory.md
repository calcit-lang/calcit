# Exclude external-object contracts from callable FFI inventory

- Treat `:ffi :kind :external-object` traits as static host capability
  contracts, not callable Interface IR lowering entries.
- Preserve inventory and diagnostics for every other callable kind, incomplete
  lowering contract, and malformed metadata shape.
- Cover realistic backend, target, and member-name metadata and document the
  separate `query def` inspection path.
- Current-main ecosystem audit: js-ffi drops from 30 definitions / 21
  diagnostics to 10 / 1, and gen-code-strict drops from 2 / 2 to 0 / 0;
  calcit-wss, calcit-json, and regex callable inventories remain unchanged.
