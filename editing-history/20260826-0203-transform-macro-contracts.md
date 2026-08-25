# Transform macro contracts

## Changes

- Migrated `->`, `->%`, and `%<-` with phase-aware base/step contracts.
- Migrated `apply-args` and `flipped` with callable input contracts and explicit dynamic result boundaries.
- Migrated anonymous function shorthands `\` and `\.` with syntax inputs and callable expression outputs.
- Migrated `[,]` with arbitrary syntax inputs and a `List<Dynamic>` expression output.
- Added exact Snapshot assertions for every required, optional, rest, capability, and expansion field.

## Contract notes

- Thread steps in `->` remain unrestricted syntax because the macro deliberately accepts symbol, tag, method, function, and list forms.
- Calcit's canonical callable schema type is `'Fn`; `'DynFn` is not the public schema spelling.
- `%<-`, `\`, and `[,]` retain runtime empty-input diagnostics because their implementation uses rest-only parameters and the phase signature mirrors that actual binder shape.

## Validation

- Reachable strict expansions: `2341/2432` -> `2364/2432`; legacy bypasses: `91` -> `68`.
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `yarn compile`
- `cargo test` (one unrelated parallel feature-policy test flaked once; isolated and full reruns passed)
- `yarn check-all`
- `yarn check-agent-interface`
- Respo `be8141e`: 27 tests and JS check-only passed.
- Recollect `6c235d0`: 9 tests, JS generation, and Node runtime passed.
- js-ffi `25869b6`: default/node/browser checks and both runtime contracts passed.
