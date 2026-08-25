# Phase-aware macro signatures

Implemented roadmap issue #434 after the parameter-shape diagnostics from
#433/#438 landed.

- Added a dedicated `MacroSignature` model instead of representing macros as
  runtime `CalcitFnTypeAnnotation` values.
- Split raw inputs into `Syntax`, `SyntaxSymbol`, `SyntaxList`, and `Expr<T>`;
  split expansion results into `Expr<T>`, `Definition<T>`, and `Declarations`.
- Added required, optional, rest, generic, where-bound, and feature metadata.
- Preserved old `Macro {:args ... :return ...}` snapshots as explicit legacy,
  non-strict signatures; the ordinary Fn parser rejects strict macro fields.
- Attached signatures to materialized `CalcitMacro` values, typed macro-body
  locals at syntax phase, and added separate call-input and expansion-result
  diagnostics with source locations and macro stacks.
- Updated snapshot, detailed snapshot, weak-type, and coverage paths so the new
  model survives all formats and contributes accurate coverage.
- Migrated core `%{}?` to `SyntaxSymbol + & SyntaxList -> Expr<Struct>` and
  documented Respo `defstyle` as the declaration-output case study.

Validation performed:

- `cargo fmt`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`
- `calcit docs check-md --entry src/cirru/calcit-core.cirru docs/MacroSignature.md`
- latest Respo main: 27/27 tests and 126/126 documentation blocks
- latest Recollect main: 9/9 Calcit tests plus JS tests

External regressions used globally installed Calcit 0.13.44 and refreshed
`@calcit/procs` 0.13.44; stale local 0.13.27 packages were replaced before the
final run.
