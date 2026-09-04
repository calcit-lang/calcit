# Reject raw primitives in strict project source

- Added `E_RAW_PRIMITIVE_IN_TYPED_CODE` for hand-written `&get-raw`,
  `&struct:get`, raw `&%{}`, and `&struct:nth` without matching nominal layout
  evidence.
- Preserved public `%{}` macro lowering, reviewed macro/core internals, reusable
  `defimpl` access, and persisted indexed Struct IR whose index/tag pair agrees
  with the concrete receiver layout.
- Added unit and real Snapshot CLI coverage with migrations toward typed
  Option-returning lookups, named Struct fields, and public constructors.
- Regressed the external `reel-strict` Respo application: compatibility
  check-only still passes and the tree stays clean. Its strict preflight still
  stops earlier at the existing `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA` boundary in
  `render-app!`, rather than at the new raw-primitive policy.
