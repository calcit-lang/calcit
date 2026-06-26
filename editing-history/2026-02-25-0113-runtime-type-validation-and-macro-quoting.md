# Runtime Type Validation & Macro Quoting Fix

## Summary

Added runtime type validation for struct record and enum tuple creation. Fixed `defstruct`/`defenum` macros to support complex type annotations (e.g., `(:optional :string)`) by conditionally quoting list-type forms in macro expansion.

## Changes

### 1. Runtime Type Validation (`src/calcit/type_annotation.rs`)

- Added `value_matches_type_annotation(value: &Calcit, expected: &CalcitTypeAnnotation) -> bool`
  - Checks if a runtime value matches a type annotation
  - Handles all annotation variants: Bool, Number, String, Tag, List, Map, Set, Ref, Buffer, Tuple, Record, Struct, Enum, Trait, Optional (allows nil), Dynamic (always true), TypeVar (always true), etc.
- Added `brief_type_of_value(value: &Calcit) -> &'static str` for error messages

### 2. Type Checking at Record/Enum Creation

- `src/builtins/records.rs`: Validates field values against struct `field_types` in:
  - `call_record_with_prototype` (`%{}`)
  - `call_record_partial` (`&%{}?`)
  - `record_with` (`&record:with`)
  - `record_from_map` (`&record:from-map`)
- `src/builtins/meta.rs`: Validates enum payload values in `new_enum_tuple_no_class` (`%::`)

### 3. Macro Quoting Fix (`src/cirru/calcit-core.cirru`)

**Problem**: `defstruct` and `defenum` are macros that expand to `&struct::new`/`&enum::new` (procs). Proc arguments are fully evaluated at runtime. Complex type annotations like `(:optional :string)` were evaluated as function calls (`(:optional :string)` → `(get :string :optional)` → error).

**Fix**: Conditionally quote type annotations in the macro expansion:

- List-type forms (e.g., `(:optional :string)`) are wrapped in `(quote ...)` to prevent evaluation
- Tag/symbol forms (e.g., `:string`, `Status`) are left unquoted for normal evaluation/resolution

### 4. Test Updates (`calcit/test-record.cirru`)

- `Lagopus0`: `:name :string` → `:name (:optional :string)`
- `Person`: field types changed to `(:optional :type)` to support nil initialization

## Key Insight

`defstruct`/`defenum` are macros → expand to proc calls (`&struct::new`/`&enum::new`) → args are evaluated before the handler receives them. Nested list type annotations must be quoted to survive evaluation, but symbol type references (e.g., enum/struct names) must NOT be quoted so they resolve correctly. The fix uses `(list? type-form)` to discriminate.
