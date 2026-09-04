# Preserve evidence-complete raw Struct constructors

- Refined strict `&%{}` handling so persisted Snapshot IR remains valid when
  the constructor resolves to one concrete `defstruct` and provides every
  declared field exactly once.
- Kept missing, duplicate, unknown, and unresolved raw constructors behind
  `E_RAW_PRIMITIVE_IN_TYPED_CODE`.
- Added focused coverage for embedded definitions, namespace-local Snapshot
  symbols, reordered complete fields, and each rejected shape.
- Regressed the external `js-ffi` project: raw Struct construction now passes;
  its strict preflight advances to the independently tracked
  `E_ERASED_GENERIC_RELATION` boundary in `js-ffi.contract/expect-string`.
