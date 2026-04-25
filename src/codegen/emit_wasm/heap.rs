use super::*;

// ---------------------------------------------------------------------------
// Memory helpers
// ---------------------------------------------------------------------------

/// Emit inline bump-allocator: allocate `byte_size` bytes and store the i32
/// base pointer into `ptr_local`.
///
/// Look up the tag ID for a builtin type tag (e.g. "list", "map").
/// Panics if the tag is missing — builtin type tags are always pre-registered.
pub(super) fn get_type_tag(ctx: &WasmGenCtx, name: &str) -> f64 {
  *ctx
    .tag_index
    .get(name)
    .unwrap_or_else(|| panic!("builtin type tag not registered: {name}")) as f64
}

pub(super) fn emit_hash_mix(ctx: &mut WasmGenCtx) {
  ctx.emit(Instruction::I32Const(0x9e37_79b9u32 as i32));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Rotl);
}

pub(super) fn emit_hash_expr_i32(ctx: &mut WasmGenCtx, expr: &Calcit) -> Result<(), String> {
  match expr {
    Calcit::Nil => {
      ctx.emit(Instruction::I32Const(0x1357_2468u32 as i32));
      Ok(())
    }
    Calcit::Bool(true) => {
      ctx.emit(Instruction::I32Const(0x4210_abceu32 as i32));
      Ok(())
    }
    Calcit::Bool(false) => {
      ctx.emit(Instruction::I32Const(0x24ce_1357u32 as i32));
      Ok(())
    }
    Calcit::Number(_) => {
      emit_expr(ctx, expr)?;
      ctx.emit(Instruction::I64ReinterpretF64);
      ctx.emit(Instruction::I64Const(32));
      ctx.emit(Instruction::I64ShrU);
      ctx.emit(Instruction::I32WrapI64);
      emit_hash_mix(ctx);
      Ok(())
    }
    Calcit::Tag(_) => {
      emit_expr(ctx, expr)?;
      ctx.emit(Instruction::I32TruncF64U);
      emit_hash_mix(ctx);
      Ok(())
    }
    Calcit::Str(_) | Calcit::Local(_) | Calcit::List(_) | Calcit::Import(_) | Calcit::Registered(_) => {
      emit_expr(ctx, expr)?;
      ctx.emit(Instruction::I32TruncF64U);
      emit_hash_mix(ctx);
      Ok(())
    }
    _ => Err(format!("unsupported WASM hash expression: {expr}")),
  }
}

pub(super) fn emit_hash_proc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&hash expects 1 arg".into());
  }
  emit_hash_expr_i32(ctx, &args[0])?;
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Bump-allocator emitter with heap type header.
///
/// Allocates `byte_size + 8` bytes: first 8 bytes store the type tag (f64),
/// the remaining `byte_size` bytes are the logical payload. Returns the LOGICAL
/// pointer (= raw_base + 8) in `ptr_local`, so all existing offset math works
/// unchanged. `type-of` can recover the type by loading at `(ptr - 8)`.
///
/// WAT sketch:
/// ```wasm
/// global.get $heap_ptr
/// f64.const <type_tag>
/// f64.store offset=0
/// global.get $heap_ptr
/// i32.const 8
/// i32.add
/// local.tee $ptr_local
/// i32.const <byte_size>
/// i32.add
/// global.set $heap_ptr
/// ```
pub(super) fn emit_bump_alloc(ctx: &mut WasmGenCtx, byte_size: i32, ptr_local: u32, type_tag: &str) {
  let tag_val = get_type_tag(ctx, type_tag) as i32;
  // Write magic at raw_base+0.
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I32Const(HEAP_MAGIC));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
  // Write tag id at raw_base+4.
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I32Const(tag_val));
  ctx.emit(Instruction::I32Store(mem_arg_i32(4)));
  // Compute logical ptr = base + 8 and save.
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(ptr_local));
  // Bump by byte_size, yielding new heap_ptr = old_base + 8 + byte_size.
  ctx.emit(Instruction::I32Const(byte_size));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
}

/// Bump-allocator with dynamic size (i32 local) and heap type header.
pub(super) fn emit_bump_alloc_dynamic(ctx: &mut WasmGenCtx, size_local: u32, ptr_local: u32, type_tag: &str) {
  let tag_val = get_type_tag(ctx, type_tag) as i32;
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I32Const(HEAP_MAGIC));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I32Const(tag_val));
  ctx.emit(Instruction::I32Store(mem_arg_i32(4)));
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(ptr_local));
  ctx.emit(Instruction::LocalGet(size_local));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
}

// ===========================================================================
// Shared data-structure helpers
// ===========================================================================

/// Evaluate expression → i32 pointer, saved in a new local.
pub(super) fn emit_ptr_to_i32(ctx: &mut WasmGenCtx, expr: &Calcit) -> Result<u32, String> {
  let local = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, expr)?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(local));
  Ok(local)
}

/// Load the count (first f64 slot) from an i32 pointer, store as i32.
pub(super) fn emit_load_count_i32(ctx: &mut WasmGenCtx, ptr_i32: u32) -> u32 {
  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_i32));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(count));
  count
}

/// Compute `base_local + byte_offset` and store in a new i32 local.
pub(super) fn emit_addr_offset(ctx: &mut WasmGenCtx, base: u32, byte_offset: i32) -> u32 {
  let local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(base));
  ctx.emit(Instruction::I32Const(byte_offset));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(local));
  local
}

/// Copy `n` f64 slots from `src_base` to `dst_base` (both i32 locals).
pub(super) fn emit_copy_f64_loop(ctx: &mut WasmGenCtx, dst_base: u32, src_base: u32, n: u32) {
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_copy_f64_slots")
    .expect("runtime helper __rt_copy_f64_slots must exist");
  ctx.emit(Instruction::LocalGet(dst_base));
  ctx.emit(Instruction::LocalGet(src_base));
  ctx.emit(Instruction::LocalGet(n));
  ctx.emit(Instruction::Call(fn_idx));
}

pub(super) fn emit_runtime_lookup_i32_f64_to_i32(ctx: &mut WasmGenCtx, helper: &str, ptr_local: u32, target_local: u32) -> u32 {
  let result = ctx.alloc_local_typed(ValType::I32);
  let fn_idx = *ctx
    .runtime_fn_index
    .get(helper)
    .unwrap_or_else(|| panic!("runtime helper missing: {helper}"));
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::LocalGet(target_local));
  ctx.emit(Instruction::Call(fn_idx));
  ctx.emit(Instruction::LocalSet(result));
  result
}

pub(super) fn emit_runtime_lookup_i32_to_i32(ctx: &mut WasmGenCtx, helper: &str, ptr_local: u32) -> u32 {
  let result = ctx.alloc_local_typed(ValType::I32);
  let fn_idx = *ctx
    .runtime_fn_index
    .get(helper)
    .unwrap_or_else(|| panic!("runtime helper missing: {helper}"));
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::Call(fn_idx));
  ctx.emit(Instruction::LocalSet(result));
  result
}

/// Generic count accessor — works for list, map, set (count is always the first f64).
pub(super) fn emit_ds_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("count expects 1 arg".into());
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  Ok(())
}

/// Generic empty? check — `count == 0`.
pub(super) fn emit_ds_empty(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("empty? expects 1 arg".into());
  }
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::Select);
  Ok(())
}

/// Allocate a new data-structure with a count header and return its i32 pointer.
/// `count_i32` is the count local; `slot_count_expr` computes the total f64 slots
/// (including the count header). Returns the allocated i32 pointer local.
pub(super) fn emit_alloc_with_count(ctx: &mut WasmGenCtx, count_i32: u32, total_slots_i32: u32, type_tag: &str) -> u32 {
  let size = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(total_slots_i32));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalSet(size));

  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc_dynamic(ctx, size, ptr, type_tag);

  // Store count
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(count_i32));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ptr
}

pub(super) fn emit_alloc_map_with_root(ctx: &mut WasmGenCtx, count_i32: u32, root_i32: u32) -> u32 {
  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, 16, ptr, "map");

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(count_i32));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(root_i32));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  ptr
}

// ===========================================================================
// High-level list allocation helpers
// ===========================================================================

/// Allocate a list with `count` (i32 local) elements.
/// total_slots = count + 1 (one extra for the count header).
/// Stores `count` into the first f64 slot and returns the i32 base pointer local.
/// This is the common pattern throughout list-building functions.
pub(super) fn emit_alloc_list(ctx: &mut WasmGenCtx, count: u32) -> u32 {
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  emit_alloc_with_count(ctx, count, total_slots, "list")
}

/// Load the f64 element at index `i` (i32 local) from a list pointer `list_ptr` (i32 local).
/// Pushes the f64 value on the WASM stack.
/// Memory layout: list_ptr[0] = count, list_ptr[8 + i*8] = elem[i].
/// i.e. address = list_ptr + (1 + i) * 8
pub(super) fn emit_list_load_elem(ctx: &mut WasmGenCtx, list_ptr: u32, i: u32) {
  ctx.emit(Instruction::LocalGet(list_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
}

/// Compute the i32 address of list element at index `i` (both i32 locals).
/// Allocates a new i32 local, stores the address in it, and returns it.
/// address = list_ptr + (1 + i) * 8
#[allow(dead_code)]
pub(super) fn emit_list_elem_addr(ctx: &mut WasmGenCtx, list_ptr: u32, i: u32) -> u32 {
  let addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(list_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(addr));
  addr
}
