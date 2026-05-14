# 202504121453 - Automatic map-to-record rewrite in preprocessing

## Summary

When a function parameter is typed as a struct type in its schema, but the caller passes a hashmap literal `{}`, the preprocessor now automatically rewrites the hashmap to a record construction `%{}` so that runtime gets a proper record with full type checking.

## Key implementation details

### type_annotation.rs
- Added `CalcitTypeAnnotation::resolve_to_struct()` — resolves `Struct`, `Record`, `TypeRef("ns/def")`, and `Optional(inner)` variants to concrete `CalcitStruct` definitions
- Added `resolve_struct_from_program(ns, def)` helper — looks up struct definitions from the program registry via `lookup_runtime_ready_registered` with fallback to `lookup_def_code_registered`

### preprocess.rs
- Added `try_rewrite_map_args_to_records()` — iterates processed args, checks each against `fn_info.arg_types[idx]`, calls `try_rewrite_single_map_to_record()` for potential rewrites
- Added `try_rewrite_single_map_to_record()` — validates arg is `List` with `Proc(NativeMap)` head, resolves expected type to struct, validates all keys are tags, builds `[Proc(NativeRecord), Struct(def), k1, v1, ...]`
- Integration point: `preprocess_list_call()` → `Calcit::Fn` match arm, between arg preprocessing and type checking

### AST transformation
- Input (hashmap literal): `[Proc(NativeMap), Tag(:x), 10, Tag(:y), 20]`
- Output (record literal): `[Proc(NativeRecord), Struct(Point2D), Tag(:x), 10, Tag(:y), 20]`

### Rewrite conditions
- Function has schema with `:args` referencing a struct type (TypeRef, Struct, or Record)
- Argument is a hashmap literal (`{}`) with tag keys
- Struct definition is resolvable at preprocess time
- If any condition fails, argument is left unchanged (safe fallback)

## Test
- Added `Point2D`, `sum-point`, `check-point-type`, `test-map-to-record` to `calcit/test-record.cirru`
- `check-point-type` uses `record?` to verify the rewrite produces actual records, not just maps

## Gotcha
- `cr tree insert-after` can create doubly-nested nodes — always verify with `tree show` after insertion
