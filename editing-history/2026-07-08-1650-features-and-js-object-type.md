# `:features` function schema + `:js-object` type + CLI simplification

## Changes

### Feature: `:features` and `:js-object` type system
- Added `features: Arc<HashSet<EdnTag>>` to `CalcitFnTypeAnnotation` for marking function capabilities
- Added `CalcitTypeAnnotation::JsObject` variant for opaque JS FFI data
- EDN round-trip for both (`features_to_edn` + `parse_fn_features_from_form`)
- Manually implemented `PartialOrd`/`Ord`/`Hash` for `CalcitFnTypeAnnotation`
- `CURRENT_FN_FEATURES` thread-local + `lookup_def_schema` fallback for FFI checking
- `as_fn()` helper on `CalcitTypeAnnotation`
- `#{}` hashset support in `schema_cirru_to_edn`
- `:features` field validation in `validate_schema_for_write`
- All direct FFI callers marked with `:features $ #{} :js-ffi`

### Refactor: CLI simplification
- Removed `--json` option and `--json-input` switch from all edit/tree subcommands
- Auto-detect JSON (`[` prefix) vs Cirru EDN (`quote` prefix) in `parse_input_to_cirru`
- Simplified `read_code_input` to `(file, code)` params; stdin fallback
- Removed `json()`/`json_input()` from `InsertOperation` trait
- Simplified `CodeInputParts` to only carry `file`+`code`
- Added stdin mention to all edit subcommand help texts
- Updated agent docs (CalcitAgent.md, agent-advanced.md, cirru-syntax.md, edit-tree.md)
- Cleaned up stale short flag mentions (`-j`, `-J`, `-e`)
- Changed js-interop doc blocks to `no-check` to avoid FFI warnings
