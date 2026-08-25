# Assertion macro contracts

- Migrated the three `calcit.core` assertion macros and five `calcit.test` helpers to strict, pure phase-aware contracts.
- Kept input values as `Expr<Dynamic>` because equality and truthiness intentionally accept heterogeneous runtime values.
- Declared precise expansion results: `Expr<Unit>` for assertion helpers and `Expr<Bool>` for `throws?`.
- Added the missing successful `&unit` branch to `assert=` so its implementation matches its long-standing Unit contract.
- Added snapshot coverage and exercised definition-attached tests plus public examples.
- On `calcit/test.cirru`, strict/pure macro candidates increased from 895 to 1,666 while total expansions stayed at 2,432.
