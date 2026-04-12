# JS Codegen Fix for Map-to-Record Rewrite

## Problem
The map-to-record rewrite (from previous commit) inserted `Calcit::Struct` directly into the AST. The interpreter handles this, but JS codegen (`emit_js.rs` line 319) has no arm for `Calcit::Struct` → panics with `unreachable!`.

## Key Insight
The `%{}` macro normally expands to `&%{}` with a **symbol/import reference** as the first arg (not a literal `Calcit::Struct`). JS codegen relies on this — it emits the symbol as a variable reference (e.g., `Element`), and the JS runtime `_$n__PCT__$M_` function accepts the struct object at runtime.

## Solution
1. Added `resolve_to_struct_with_ref()` to `CalcitTypeAnnotation` — returns `Option<(CalcitStruct, Option<(Arc<str>, Arc<str>)>)>` with ns/def path from TypeRef annotations.
2. Modified `try_rewrite_single_map_to_record()` to emit `Calcit::Import(CalcitImport { ... })` instead of `Calcit::Struct`:
   - `ImportInfo::SameFile` when struct is in same namespace (avoids self-import duplicate declaration)
   - `ImportInfo::NsReferDef` when struct is in a different namespace (generates proper cross-ns import)
   - Falls back to `Calcit::Struct` only when no ns/def path is available

## Pitfall: Self-Import Duplicate Declaration
First attempt used `NsReferDef` for all imports — caused `SyntaxError: Identifier 'Point2D' has already been declared` because a self-import was generated when struct is in same namespace. Fix: detect `ns == file_ns` and use `SameFile`.

## Verification in Respo
- Typed `element->string` as `'respo.schema/Element`, `make-string` as `'respo.schema/Component`
- Test passes `{}` map to `element->string` → auto-rewritten to `%{} Element ...` record construction
- Generated JS: `import { Element } from "./respo.schema.mjs"` + `$clt._$n__PCT__$M_(Element, ...)`

## Files Changed
- `src/calcit/type_annotation.rs`: Added `resolve_to_struct_with_ref()`
- `src/runner/preprocess.rs`: Emit Import reference in `try_rewrite_single_map_to_record()`
