# Strengthen polymorphism diagnostics

## Summary

- Clarified the static-semantics RFC around parametric generics, trait bounds, applied data types, callback contracts, enums, optionals, and type slots.
- Restored the intended compatibility rule that legacy `:any` is parsed and serialized as `:dynamic`, rather than acting as a separate static top type.
- Added generic and trait-bound evidence to type-coverage reports.
- Added stable aggregate diagnostics for coverage gaps and unresolved dynamic debt, including per-occurrence impact and relationship-aware suggestions for Agents.
- Reduced false positives from generated metadata namespaces, `defimpl` declarations, and singleton-list wrappers around Cirru data declarations.
- Updated Agent and feature documentation with a concise workflow for replacing accidental dynamic types with concrete types, type variables, trait bounds, parameterized containers, or enums.

## Validation

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface` (12/12 scenarios)
- `yarn check-all`
- `cr docs check-md` for `docs/CalcitAgent.md`, `docs/features/static-analysis.md`, and `docs/features/polymorphism.md`
- Real-project regression on a temporary Respo workflow copy, including type reports and successful `cr js` compilation.
