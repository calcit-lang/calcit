# Reject reachable unbound type slots in strict mode

- Added `E_UNBOUND_TYPE_SLOT` during strict preprocessing of reachable function
  and macro definitions.
- Reused one recursive schema walker for bare-container and type-slot checks so
  nested argument, return, rest, macro, and nominal type-argument paths remain
  consistent.
- Kept compatibility mode unchanged: unresolved slots still participate in
  `analyze weak-types` and quality baselines.
- Preserved entry-level `:dynamic` as an explicit, inventoried opt-out rather
  than treating it as a missing binding.
- Added regression coverage for nested schema paths, source location, migration
  guidance, compatibility mode, and explicit Dynamic binding.
