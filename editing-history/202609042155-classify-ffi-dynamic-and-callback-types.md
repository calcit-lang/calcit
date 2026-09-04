# Classify FFI Dynamic and callback types

- Split open `Dynamic` positions into stable `E_FFI_IR_DYNAMIC_TYPE`
  diagnostics with concrete decode-or-adapt migration guidance.
- Split typed and untyped callbacks into stable `E_FFI_IR_CALLBACK_TYPE`
  diagnostics that name the missing ownership, thread-affinity, and lifetime
  contract.
- Preserve the generic unsupported code for the remaining portable-subset gaps
  and cover deterministic codes and nested paths.
