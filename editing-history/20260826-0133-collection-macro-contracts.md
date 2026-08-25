# Collection macro contracts

- Migrated `{}`, `%{}`, and `{,}` to strict, compile-time-pure macro contracts.
- Declared map literal expansions as `Expr<Map<Dynamic, Dynamic>>` and struct literal expansion as `Expr<Struct>`.
- Kept pair-oriented inputs as `SyntaxList` and the flat comma-filtering `{,}` body as an intentional raw `Syntax` rest boundary.
- Corrected the pre-existing `%{}` example to use field pairs `(:x 1)` and `(:y 2)` rather than list-constructor calls that introduced extra fields.
- Added exact Snapshot assertions for each macro's input and expansion contracts.
- Increased strict macro coverage in `calcit/test.cirru` from 1949 to 2133 of 2432 expansions.
- Verified all repository gates plus Respo `be8141e`, Recollect `6c235d0`, and js-ffi `25869b6` with the current compiler.
