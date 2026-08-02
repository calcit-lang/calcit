# Trait review follow-ups

## What changed

- Corrected the public `Mappable` capability table: Result joins List, Map, Set, and Option.
- Documented why `calcit.internal` is excluded from the legacy inherent-impl advisory: bootstrap method bags precede public nominal trait availability.
- Made builtin literal method introspection tolerant of unavailable core impl lists, matching record and tuple behavior in embedding/unit-test startup states.
- Corrected `&str:contains?` documentation and schema to describe numeric character-index bounds checking; substring membership remains `&str:includes?`.
- Restrict primitive trait-name bootstrap fallback to the period before its real core impl list is evaluated, and added a test that reads the embedded core Snapshot to detect table drift.

## Verification

- `cr src/cirru/calcit-core.cirru edit format`
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test` (300 library tests, 179 CLI tests)
- `yarn compile`
- `yarn check-all` (Agent interface, native, JS, IR, WASM)
- `cr docs check-md --entry calcit/test.cirru docs/features/polymorphism.md`
