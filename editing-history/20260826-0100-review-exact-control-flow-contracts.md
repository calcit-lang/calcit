# Exact control-flow contract regression checks

- Strengthened the core Snapshot inventory to assert each migrated control-flow macro's exact required, optional, and rest syntax contracts.
- Asserted exact expansion types, including `when-let` as `Expr<Option<Dynamic>>` and the remaining macros as `Expr<Dynamic>`.
- Documented the strict macro body type helpers added by the original change.
