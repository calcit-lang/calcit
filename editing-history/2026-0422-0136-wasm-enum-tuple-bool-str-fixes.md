# WASM Codegen: Enum Tuple, Bool Tag, and Str Fixes

**Date**: 2026-04-22 01:36  
**Skip count**: 68 → 62 (6 fewer)

## Changes Made

### 1. `NativeEnumTupleNew` (`%::`) — New Dedicated Emitter

**File**: `src/codegen/emit_wasm/records.rs`, `src/codegen/emit_wasm.rs`

Added `emit_enum_tuple_new` for the `%::` proc (enum variant constructor).

Key insight: `%::` always has the form `(%:: enum_class tag payload...)` where:
- `enum_class` (args[0]) is for type-checking only — **ignored in WASM**
- `tag` (args[1]) is the actual variant tag (Calcit::Tag)
- `payload` (args[2..]) are the values

This is distinct from `::` (`NativeTuple`) which uses `args[0]` as the tag directly.

**Fixed functions**: `%ok`, `%err`, `%some`, `%none` — now compile correctly.

Memory layout is identical to `::` tuples: `[count:f64][tag_id:f64][payload...]`.

### 2. Bool Support in `emit_tuple_new`

**File**: `src/codegen/emit_wasm/records.rs`

Extended `emit_tuple_new` to accept `Calcit::Bool` as the tag field:
- `true` → `1.0`
- `false` → `0.0`

**Purpose**: The `foldl-shortcut` pattern uses `(:: true value)` and `(:: false value)` as tagged pairs for early-exit signaling. The `foldl-shortcut` implementation reads the tag at offset +8 and compares with `1.0`.

**Fixed functions**: `calcit.core/index-of`, `calcit.core/&list:last-index-of` — these use `foldl-shortcut` with an inline `%index-of` defn that calls `(:: true idx)` / `(:: false ...)`.

### 3. Variadic `str` / `str-spaced` Call-Site Intercepts

**File**: `src/codegen/emit_wasm/strings.rs`, `src/codegen/emit_wasm.rs`

Added two new pub functions:
- `emit_str_variadic(ctx, args)`: `(str a b c ...)` → left-fold of string concat
- `emit_str_spaced(ctx, args)`: `(str-spaced a b c ...)` → join with space separator

Private helper:
- `concat_two_i32_ptrs(ctx, ptr_a, ptr_b)`: concatenates two strings from i32 ptr locals

**Key technique**: `emit_turn_string` converts any f64 value to a string, then `concat_two_i32_ptrs` combines them without re-emitting expressions.

For `str-spaced`, the space character is allocated dynamically (1-byte string with 0x20 written via `I32Store8`) to avoid depending on the string pool.

**Intercept added in `emit_call_expr`** for `calcit.core` namespace:
```rust
"str" if !args_list.is_empty() => return emit_str_variadic(ctx, &args_list),
"str-spaced" if !args_list.is_empty() => return emit_str_spaced(ctx, &args_list),
```

These intercepts help user code calling `str`/`str-spaced`; the core library definitions still fail because they use `(&syntax &)` variadic syntax.

## Remaining 62 Skips (Categorized)

- **14 'f callee** (HOF defs): map, filter, each, find, foldl', update, etc.
- **~9 nested defn**: &fn:apply, &fn:bind, &list:filter-pair, on-click, etc.
- **~7 complex defn callee**: group-by, frequencies, repeat, zipmap, etc.
- **3 `(&syntax &)`**: str, str-spaced, &str-spaced (definitions; call sites intercepted)
- **3 method calls**: .slice, .filter, .deref (definition level)
- **~4 variadic spread**: concat, mapcat, &list:flatten, &list:apply
- **Others**: dissoc, conj, contains-in?, tagging-edn, impl-traits, etc.
