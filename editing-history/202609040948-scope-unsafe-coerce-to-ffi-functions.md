# Scope unsafe-coerce to lexical FFI functions

- Added backend-independent `E_UNSCOPED_UNSAFE_COERCE` enforcement in strict
  project preprocessing. The current structured function schema must declare
  `:js-ffi`; an adapter-like namespace does not grant permission.
- Kept compatibility mode and the existing entry feature policy unchanged.
  Correctly scoped assertions remain visible in the per-definition
  `unsafeCoerce` quality budget.
- Added unit and Snapshot integration coverage for both the rejected unmarked
  adapter and the accepted marked adapter with an ordinary typed caller.
- Regressed the external `js-ffi` browser entry against the marked `random`
  adapter: preprocessing passes, then the existing project-wide strict-zero
  quality gate reports only its reviewed migration debt (`unsafeCoerce: 30`).
  The default Node entry still stops earlier at the independently tracked
  `E_ERASED_GENERIC_RELATION` in `js-ffi.contract/expect-string`.
