# Remaining core match macro contracts

- Migrated `case-default`, `field-match`, and `calcit.internal/&field-match-internal` to strict phase-aware macro signatures.
- `case-default` now validates both value/default expression inputs and requires every pattern arm to be list-shaped syntax while keeping its branch-dependent output honest as `Expr<Dynamic>`.
- `field-match` and its recursive helper now require a map expression before expansion, so invalid values fail at the public macro boundary with `E_MACRO_INPUT_EXPR_TYPE` instead of reaching generated runtime assertions.
- Added snapshot assertions for required inputs, rest-arm syntax, pure capability sets, and expansion categories.
- The core suite now observes 2392 pure/cache-eligible expansions, 5 explicitly platform-dependent expansions, and only 35 legacy expansions, all of which belong to test helper macros.
