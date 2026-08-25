# Control-flow macro contracts

- Migrated `or`, `either`, `when`, `when-not`, `if-not`, `if-let`, and `when-let` to strict, pure phase-aware contracts.
- Preserved expression-level Dynamic boundaries where branch value types intentionally vary, while declaring `when-let` as `Expr<Option<Dynamic>>`.
- Corrected the `or false nil` example to expect the final falsey value (`nil`) and clarified that behavior in its documentation.
- Fixed strict macro body parameter typing: `SyntaxList` is represented as a list, `SyntaxSymbol` as a symbol, and rest bindings as lists of their declared syntax element type.
- Added regression coverage for strict macro body parameter representations and the real core Snapshot inventory.
- Increased strict macro coverage in `calcit/test.cirru` from 1666 to 1805 of 2432 expansions.
- Verified the release compiler against Respo `be8141e` (27 tests and check-only JS) and Recollect `6c235d0` (9 native tests plus JS/Node tests).
