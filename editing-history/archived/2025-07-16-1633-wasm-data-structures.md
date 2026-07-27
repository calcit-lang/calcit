# WASM Data Structures: List, Map, Set + Optional Args + Raise

## Summary

Comprehensive data structure support for WASM codegen: list (16 ops), map (9 ops), set (6 ops), plus `raise` and optional argument handling.

## Changes

### 1. Optional Args (`CalcitArgLabel::OptionalMark`)
- `compile_fn`: Skip `OptionalMark` markers — they aren't parameters, just annotations. The actual Idx labels become WASM params.
- Added `fn_arity` map alongside `fn_index` to track each function's WASM arity (counting only `Idx` labels).
- At call sites (`Calcit::Import`, `Calcit::Symbol`), pad missing optional args with `f64.const 0` (nil) to match target arity.
- **Key insight**: The Calcit preprocessor doesn't always fill nil for omitted optional args in the code tree; WASM codegen must handle the arity mismatch at the call site.

### 2. Raise
- `CalcitProc::Raise` → evaluate and drop all args, emit `Instruction::Unreachable`.

### 3. Memory Layouts
- **List**: `[count:f64] [elem0:f64] [elem1:f64] ...` — size `(1+count)*8`
- **Map**: `[count:f64] [key0:f64] [val0:f64] [key1:f64] [val1:f64] ...` — size `(1+count*2)*8`
- **Set**: Same as list — `[count:f64] [elem0:f64] ...`

### 4. Helper Functions
- `emit_bump_alloc_dynamic(ctx, size_local, ptr_local)` — bump allocator with dynamic size
- `emit_ptr_to_i32(ctx, expr)` — evaluate expression to i32 pointer in a local
- `emit_load_count_i32(ctx, ptr)` — load count from first f64 slot as i32
- `emit_addr_offset(ctx, base, offset)` — compute base+offset into new local
- `emit_copy_f64_loop(ctx, dst, src, n)` — copy N f64 slots between addresses
- `emit_ds_count(ctx, args)` — shared count accessor (list/map/set)
- `emit_ds_empty(ctx, args)` — shared empty? check
- `emit_alloc_with_count(ctx, count, total_slots)` — allocate and write count header

### 5. List Operations (16)
- Constructor: `emit_list_new` — `[] elem0 elem1 ...`
- Access: `emit_list_nth`, `emit_list_first`, `emit_list_rest`
- Mutation: `emit_list_append`, `emit_list_prepend`, `emit_list_butlast`
- Slicing: `emit_list_slice`, `emit_list_reverse`, `emit_list_concat`
- Update: `emit_list_assoc`, `emit_list_dissoc`
- Query: `emit_list_contains`, `emit_list_includes`

### 6. Map Operations (9)
- Constructor: `emit_map_new` — `&{} key val ...`
- Access: `emit_map_get_op` — linear key scan, return value or nil
- Mutation: `emit_map_assoc` (scan for existing key, branch: update vs append), `emit_map_dissoc` (scan + copy-with-skip)
- Query: `emit_map_contains`, `emit_map_includes`
- Transform: `emit_map_to_pairs` — creates list of 2-elem lists (nested allocation in loop)

### 7. Set Operations (6)
- Constructor: `emit_set_new` — `#{} elem ...`
- Access: `emit_set_includes` (delegates to list_includes — same layout)
- Mutation: `emit_set_include` (scan + conditional append), `emit_set_exclude` (scan + copy-with-skip)

## WASM Patterns Used
- **Copy loop**: `block/loop/br_if/br` with i32 counter for bulk f64 element copying
- **Scan loop**: Same pattern with `f64.eq` comparison for key/value lookup; `found_idx` local with -1 sentinel
- **Conditional allocation**: `if/else` for map_assoc (new key vs update) and set_include (already present vs append)
- **Select instruction**: For boolean returns (`contains?`, `empty?`) — `select(true_val, false_val, condition)`

## Test Results
- 90 WASM checks (55 existing + 35 new) — all pass
- 246 cargo tests — all pass
