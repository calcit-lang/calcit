# Classify compiler-specialized contract positions

## Context

The bundled-core Dynamic inventory classified every ordinary public-core
position as migration work except `apply`'s return. That stopped reflecting
merged receiver-driven type checking and inference for indexed access, lookup,
membership, and update contracts. It also made a definition-level conclusion
where some positions are recovered but the receiver capability is still not
expressible in the schema.

## Change

- Record only the exact key, payload, or return positions backed by focused
  compiler specialization as `compiler-specialized-contracts`.
- Keep each corresponding Dynamic receiver position in the public migration
  queue until an honest capability can reject unsupported receivers.
- Fail generation if a configured specialized position disappears from the
  inventory, preventing stale classification entries.
- Document that classification is position-specific and does not claim that a
  whole fallback contract is fully typed.

The complete schema-Dynamic inventory and quality budgets remain unchanged;
this updates ownership and migrate/retain decisions to match implemented type
flow rather than hiding debt.
