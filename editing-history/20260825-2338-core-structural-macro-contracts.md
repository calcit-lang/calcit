# Core structural macro contracts

- Migrated `let`, `fn`, `and`, `cond`, and `do` from whole-Dynamic legacy
  schemas to strict, compile-time-pure phase contracts.
- Kept expansion values as `Expr<Dynamic>` where the result depends on user
  code, while enforcing list-shaped binding/argument/condition syntax.
- Canonicalized strict Macro storage with explicit empty capability sets and
  added snapshot assertions so the migrated set cannot silently regress.
- The Calcit test snapshot now exposes 895 strict/pure expansion candidates in
  the measured run (up from 4), while preserving all 2,432 expansions.
