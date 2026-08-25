# Macro capability review follow-up

- Keep the canonical strict Macro signature example complete by serializing an
  explicit empty `:capabilities` set.
- Treat mutable BufList construction and mutation as `:mutable-state` during
  macro expansion.
- Validate capability leaves as colon-prefixed tags before general Cirru EDN
  parsing, preserving a stable schema-specific error instead of accepting a
  write that cannot round-trip.
- Verified with the macro capability and macro schema unit-test groups.
