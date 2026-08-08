# Migrate remaining record/tuple naming to struct/enum

Follow-up to the earlier terminology migration: located and updated the
remaining stale `record`/`tuple` naming that refers to the current
struct/enum data model, keeping genuine legacy-compat and EDN semantics.

## Updated

- `calcit/test-wasm.cirru`: renamed `test-record-*` → `test-struct-*` and
  `test-tuple-*` → `test-enum-*` (incl. `test-type-of-tuple` →
  `test-type-of-enum`) via `cr edit rename`; updated 12 doc strings. Kept
  `defrecord Point :x :y` as the WASM legacy-compat coverage.
- `scripts/test-wasm.mjs`: synced the renamed def references and section
  labels.
- `src/runner/preprocess/mod.rs`: test-namespace labels `tests.record` →
  `tests.struct` (kept the legacy `record` tag coverage in the loose-struct
  test).
- `src/builtins/meta.rs`: `Calcit::Enum(tuple)` local bindings →
  `enum_value`.
- `src/codegen/emit_wasm.rs`: comments `tuple fields`/`tuple pointer` →
  `enum fields`/`enum pointer`.
- `src/bin/cli_handlers/query.rs`: test message `tuple operations` →
  `enum operations`.

## Deliberately kept

- Legacy migration tables in `docs/`, `deprecated_api.rs`, and
  `removed_data_api_replacement`.
- Type-name aliases (`record`/`tuple` → Struct/Enum parsing), `SIMPLE_TYPES`
  `tuple` query name, and IR/format-stable kind tags.
- `cirru_edn` `Edn::Record`/`Edn::Enum` contexts and hash prefixes.

Validation:

- `cargo test -q`（368 lib + 192 integration）
- `cargo clippy --lib --bin cr -- -D warnings`
- `cargo fmt --check`
- `yarn try-wasm`（含重命名后的 `test-struct-*` / `test-enum-*`）
