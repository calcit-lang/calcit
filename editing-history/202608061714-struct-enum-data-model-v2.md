# Struct / Enum data model v2

## Scope

- Split runtime definitions from values: `StructDef` / `EnumDef` versus `Struct` / `Enum`.
- Replaced outward `record` / `tuple` names with `struct` / `enum`; historical snapshot input remains readable, but new writes and old API calls receive migration diagnostics.
- Added canonical anonymous forms `%{} _ ...` and `%:: _ ...`.
- Render named data definitions with symbols, for example `(%{} 'TodoState ...)`, in Rust display and the TypeScript custom formatter.

## Field access contract

- A struct has a fixed declared field set. Known-field `get` and postfix access return the field's declared type directly rather than `Option<T>`.
- Missing struct fields produce static diagnostics when type information is available and ordinary runtime errors otherwise; they never turn into silent `nil`/`Option` absence.
- Map, list, string, and other genuinely fallible collection lookups retain `Option` behavior.

## Migration surface

- Native APIs moved from `&record:*` / `&tuple:*` to `&struct:*` / `&enum:*`; definition-only operations use `&struct-def:*` / `&enum-def:*`.
- Public reflection is `struct-definition` / `enum-definition`; predicates distinguish values (`struct?`, `enum?`) from definitions (`struct-def?`, `enum-def?`).
- Removed calls emit `W_REMOVED_DATA_API` with concrete replacements. Compatibility wrapper bodies also fail explicitly if checking is bypassed.
- Documentation now presents structs, enums, anonymous structs, and anonymous enums as the public model and keeps old spellings only in the migration table.

## Recursive types and regressions

- Recursive nominal references stay finite while being resolved.
- `Optional<Node>` accepts a `nil` leaf and nested non-`nil` nodes.
- Required invalid recursion and unsupported recursive data-shape forms return normal diagnostics instead of overflowing the stack.

## Cross-backend work

- Rust, JS, IR, and WASM emitters and runtime tags use the new semantic categories.
- TypeScript runtime files are separated into struct/enum definition and value modules.
- Custom formatter headers embed named and anonymous definition markers as `CalcitSymbol`, including `_`.

## Real-project validation

- Respo was migrated from legacy data APIs and predicates using AST leaf replacements.
- `Element` stays a struct through `purify-element`, allowing direct typed field access and removing obsolete `Option` unwraps.
- Respo check-only and its full JS test suite pass when loaded against this repository's newly compiled `@calcit/procs` runtime.
