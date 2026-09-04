# Reject generic relations erased by Dynamic

- Added strict `E_ERASED_GENERIC_RELATION` call-site validation for declared
  type variables that relate multiple argument, nested, variadic, or return
  positions.
- Match expected and actual schemas structurally: `List<T>` against
  `List<Dynamic>` loses the item relationship, while binding an entire `T` to
  `List<Dynamic>` still preserves `T -> T` and must not be rejected.
- Kept compatibility mode and dependency source unchanged. A genuinely open
  operation belongs behind a structured adapter that does not claim the erased
  generic relation.
- Covered user/imported and local function calls with unit tests and a real
  Snapshot CLI fixture. Verified with `cargo clippy -- -D warnings`,
  `cargo test`, `yarn compile`, `yarn check-agent-interface`, `yarn check-all`,
  and a globally installed `calcit` check against Respo.
