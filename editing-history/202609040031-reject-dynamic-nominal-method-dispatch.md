# Reject Dynamic nominal method dispatch in strict mode

- Promoted Option/Result nominal method calls on Dynamic receivers to stable
  `E_DYNAMIC_METHOD_DISPATCH` and `E_DYNAMIC_POSTFIX_METHOD` strict errors.
- Kept compatibility warnings and the general dynamic-method inventory intact
  while receiver-loss classification continues for unknown method families.
- Added prefix/postfix unit coverage and a real Snapshot CLI fixture with
  concrete receiver-schema and visible helper migration guidance.
- Documented the strict boundary and its non-coercive migration path.
