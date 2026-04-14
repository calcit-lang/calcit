# match syntax with exhaustiveness checking

## Summary

Implemented a new native `match` syntax for enum pattern matching with compile-time exhaustiveness detection, as a safer alternative to the existing `tag-match` macro.

## Motivation

- `tag-match` is a macro that expands to nested `if` expressions, losing the match structure — no way to do exhaustiveness checking.
- A native syntax preserves the full branch structure through preprocessing → runtime → JS codegen, enabling variant coverage analysis.

## Design Decisions

### Pair-based input (not flat alternating)

Cirru indentation naturally creates pair-based children when each branch is on its own line:

```cirru
match value
  (:ok) :matched-ok
  (:err msg) msg
```

Parses as: `(match value ((:ok) :matched-ok) ((:err msg) msg))` — each branch is a 2-element list `(pattern body)`.

Initially tried flat alternating input `(match value pattern1 body1 pattern2 body2 ...)`, but Cirru's serialization round-trip groups items into sub-lists based on indentation depth, making flat format unreliable.

### Cirru encoding pitfall

When using `v $ match result-ok` with branches at different indentation levels, the Cirru serializer creates separate nesting for zero-arity patterns (`:ok`, `:matched-ok` deeper) vs multi-element patterns (`(:err msg) msg` shallower). This wraps branches in an extra list.

**Fix**: Write `v` on its own line with `match result-ok` indented underneath, ensuring all branch lines share the same indentation level.

## Implementation

### Files modified

1. **`src/calcit/syntax_name.rs`** — Added `#[strum(serialize = "match")] Match` to `CalcitSyntax` enum + `SyntaxTypeSignature`
2. **`src/builtins/syntax.rs`** — `syntax_match()` runtime handler: evaluates value, extracts tag/extra from `CalcitTuple`, iterates pair branches, creates scope bindings for payloads
3. **`src/builtins.rs`** — Wired `Match => syntax::syntax_match(...)` into `handle_syntax` dispatch
4. **`src/runner/preprocess/mod.rs`** — `preprocess_match()`:
   - Accepts pair-based input: `(match value (pattern body) ...)`
   - Infers enum type via `infer_type_from_expr` + `CalcitTypeAnnotation::Tuple`/`TypeSlot` resolution
   - Validates each variant exists in enum and checks arity
   - Creates `CalcitLocal` bindings with inferred payload types from `EnumVariant::payload_types()`
   - Exhaustiveness: `BTreeSet` difference of `all_variants` vs `covered` tags → warns on missing
5. **`src/codegen/emit_js.rs`** — `gen_match_code()`: IIFE with if-else chain using `_$n_tuple_$o_nth` for tag comparison and binding extraction, wildcard `_` support, fallthrough error

### Test

- Added `test-match` definition in `calcit/test-enum.cirru` with 3 cases: `:ok` match, `:err msg` match, wildcard `_` match
- Wired into `main!` after existing enum tests

## Key learnings

- Cirru serialization of `([:ok] :matched-ok)` breaks it into `:ok` and `:matched-ok` at different indentation depths, causing unexpected nesting on re-read.
- Always verify Cirru AST structure with `cr tree show -p <path> --raw` before assuming a particular encoding.
- For match-like syntax with heterogeneous branch structures (zero-arity vs with-bindings), pair-based format is more robust than flat alternating in Cirru.
- `tag-match` macro in `src/cirru/calcit-core.cirru` lines 4402-4430 uses the same pair pattern: each branch is `(pattern body)`.

## Validation

- `cargo test`: 64 passed
- `cargo clippy --lib -- -D warnings`: clean
- `yarn check-all`: all integration tests pass (compile + try-rs + try-js + try-ir)
