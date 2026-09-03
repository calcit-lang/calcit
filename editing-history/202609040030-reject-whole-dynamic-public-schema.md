# Reject whole-Dynamic public schemas in strict mode

- Added `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA` for reachable project functions and
  macros whose root schema is missing or explicitly Dynamic and which have no
  embedded structured function contract.
- Kept compatibility mode unchanged and preserved structured `Fn` / `Macro`
  contracts with Dynamic only at exact reviewed positions, including embedded
  `Fn` hints carried by older Snapshots.
- Added unit and real Snapshot CLI coverage for the stable diagnostic, schema
  root path, source location, and migration guidance.
- Documented the strict behavior in CLI, static-analysis, upgrade, and fixture
  guidance.
