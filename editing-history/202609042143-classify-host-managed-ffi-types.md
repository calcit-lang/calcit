# Classify host-managed FFI types

- Give the core `FfiTask` and `FfiResponse` capabilities a stable
  `E_FFI_IR_HOST_MANAGED_TYPE` diagnostic instead of misclassifying them as
  missing local declarations.
- Preserve local declaration precedence for an unqualified matching name, so
  the diagnostic does not hide valid project-owned Struct/Enum shapes.
- Document the handwritten adapter migration and cover deterministic qualified,
  unqualified, parameter, result, and local-shadowing behavior.
