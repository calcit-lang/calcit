# Definition-value macro contracts

## Changes

- Migrated `def` to a pure phase-aware macro contract with a symbol declaration name and an explicit dynamic expression boundary.
- Migrated `deftrait`, `defstruct`, `defimpl`, and `defenum` to strict syntax-input contracts with precise `Trait`, `StructDef`, `Impl`, and `EnumDef` expression outputs.
- Kept symbol/tag compatibility for data-definition names by using unrestricted syntax for those positions; method and field forms use list syntax where the macro requires pair/list forms.
- Added Snapshot assertions for required, optional, rest, capability, and expansion fields so serialization regressions cannot silently weaken these contracts.

## Semantic finding

Although these macros generate top-level definition values, the core `def` macro is consumed before the outer expansion-result check. Their observable expansion contract is therefore `Expr<T>`, not `Definition<T>`. Using `Definition<T>` fails against the real core corpus because the checked form is already `&trait::new`, `&struct-def:new`, `&impl::new`, or `&enum-def:new`.

## Validation

- Core strict macro expansions: `2133/2432` -> `2341/2432`; legacy bypasses: `299` -> `91`.
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `yarn compile`
- `cargo test`
- `yarn check-all`
- `yarn check-agent-interface`
- Respo `be8141e`: 27 definition-attached tests and JS check-only passed.
- Recollect `6c235d0`: 9 unit tests, JS generation, and Node runtime passed.
- js-ffi `25869b6`: default/node/browser checks plus Node and browser-node contracts passed.
