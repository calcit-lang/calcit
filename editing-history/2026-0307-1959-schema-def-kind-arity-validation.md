# Schema vs Definition Kind/Arity Validation

## Summary

Added structural validation that compares a definition's code form against its schema declaration. This catch mismatches between declared schema metadata (`:kind`, arg count, rest param) and the actual `defn`/`defmacro` code at analysis time.

## Key Changes

### `src/calcit/type_annotation.rs`

1. **Added `SchemaKind` enum** (`pub enum SchemaKind { Fn, Macro }`) — distinguishes `:kind :fn` from `:kind :macro` schemas.

2. **Extended `CalcitFnTypeAnnotation`** with two new fields:
   - `pub fn_kind: SchemaKind` — schema's declared kind (fn/macro).
   - `pub rest_type: Option<Arc<CalcitTypeAnnotation>>` — schema's `:rest` type, if present.

3. **`parse_fn_schema_from_edn`** — now reads `:kind :macro` → `SchemaKind::Macro` and `:rest` → `rest_type` from the EDN map.

4. **`to_schema_edn`** — now serializes `fn_kind` (`fn`/`macro`) and `rest_type` (`:rest` key in the output map). Previously always emitted `:kind :fn` and omitted `:rest`.

5. **All `CalcitFnTypeAnnotation` construction sites** updated to add `fn_kind: SchemaKind::Fn, rest_type: None` defaults (7 total).

6. **`substitute_type_vars`** also propagates `fn_kind` and substitutes `rest_type`.

### `src/calcit.rs`

- `SchemaKind` added to the `pub use type_annotation::{ ... }` re-export.

### `src/bin/cr.rs`

1. **`SchemaKind` imported** in the `use calcit::calcit::{...}` block.

2. **`TypeCoverageRow` struct** gained `schema_issues: Vec<String>` field — carries kind/arity mismatch warnings per definition.

3. **`validate_def_vs_schema(ns, def_name, code, schema) -> Vec<String>`** — new function:
   - `&runtime-inplementation` leaf → skip (builtin proc/syntax).
   - Schema not `CalcitTypeAnnotation::Fn` → skip.
   - Code head not `defn`/`defmacro` → skip.
   - Schema `:kind :fn` + code `defmacro` → error.
   - Schema `:kind :macro` + code `defn` → error.
   - Required arg count mismatch → error.
   - `has_rest` mismatch (schema `:rest` vs code `&`) → error.

4. **`analyze_param_arity(args: Option<&Cirru>) -> (usize, bool)`** — helper that counts required params and detects the `&` rest marker in a `defn`/`defmacro` args list.

5. **`analyze_code_entry`** calls `validate_def_vs_schema` to populate `schema_issues` on each `TypeCoverageRow`.

6. **`run_check_types` output loop** prints a `schema-issues:` section for rows with non-empty issues.

7. **10 new unit tests** covering:
   - `&runtime-inplementation` skip
   - Correct defn/macro → no issues
   - Kind mismatch (`:fn` vs `defmacro`, `:macro` vs `defn`)
   - Arity mismatch
   - Rest param mismatch
   - `analyze_param_arity` basics
8. **`CR_DEBUG_SCHEMA` debug flag** — when `CR_DEBUG_SCHEMA=1`, prints per-entry schema kind debug info to stderr during `analyze check-types`.

### `src/snapshot.rs`

- **`test_macro_schema_round_trip`** — verifies `schema_cirru_to_edn` + `parse_fn_schema_from_edn` + `normalize_schema_edn` preserve `SchemaKind::Macro` end-to-end.
- **`test_macro_schema_full_file_round_trip`** — verifies full `CodeEntry` serialize (`From<&CodeEntry> for Edn`) → `cirru_edn::format` → `cirru_edn::parse` → `TryFrom<Edn> for CodeEntry` round-trip preserves `fn_kind: Macro`.

## Validation Rule Summary

| Schema `:kind` | Code form    | Result          |
|----------------|--------------|-----------------|
| `:fn`          | `defn`       | ✓ ok            |
| `:fn`          | `defmacro`   | ✗ kind mismatch |
| `:macro`       | `defmacro`   | ✓ ok            |
| `:macro`       | `defn`       | ✗ kind mismatch |
| any            | `&runtime-inplementation` | ✓ skip |

Arg count and rest param are checked independently of kind.

## Notes

- **`calcit.core` schemas require binary rebuild**: Schema edits to `src/cirru/calcit-core.cirru` do not take effect at runtime until the binary is rebuilt, because the core namespace is always loaded from the embedded `include_bytes!` snapshot produced by `build.rs`. Schema validation is most useful for user-defined namespaces.

## Test Results

All 123 tests pass (97 library + 26 binary), including 2 new snapshot round-trip tests.
