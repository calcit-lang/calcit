# Reject general Dynamic method dispatch in strict mode

- Promoted every remaining unspecialized project prefix/postfix method call to
  `E_DYNAMIC_METHOD_DISPATCH` or `E_DYNAMIC_POSTFIX_METHOD` in strict mode.
- Classified receiver loss as missing schema, Dynamic value/callable, legacy
  Optional, unbound generic/type slot, or explicit `:js-ffi` Dynamic boundary.
- Kept compatibility warnings and `analyze dynamic-methods` inventory intact;
  dependency namespaces and statically dispatchable nominal/trait receivers are
  unchanged.
- Added unit and real Snapshot CLI coverage with migration guidance that favors
  concrete schemas, trait constraints, slot bindings, and narrow typed adapters
  instead of broad coercion.
- Regressed against `respo.calcit`: compatibility analysis reports three
  actionable `.to-list` dispatch sites. Its full strict preflight still stops
  earlier at the existing `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA` macro boundary, so no
  external source was changed to hide the new inventory.
