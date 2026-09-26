use super::*;
use wasm_encoder::BlockType;

// ===========================================================================
// String operations — layout: [magic:i32][type_tag:i32][byte_len:f64][utf8_bytes... padded]
// logical_ptr points to byte_len field; bytes start at logical_ptr+8.
// All byte counts are UTF-8 byte lengths (matching Rust str::len() semantics).
// ===========================================================================

/// Allocate a new heap string of `len_i32` (i32 local) bytes.
/// Returns `(ptr_local, content_base_local)`:
/// - `ptr_local`: i32 local = logical pointer (where byte_len f64 lives)
/// - `content_base_local`: i32 local = ptr + 8 (start of UTF-8 content)
///
/// Also stores byte_len as f64 into ptr+0.
pub(super) fn emit_str_alloc(ctx: &mut WasmGenCtx, len_i32: u32) -> (u32, u32) {
  // padded_len = (len + 7) & -8  (round up to 8-byte boundary)
  let padded = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(len_i32));
  ctx.emit(Instruction::I32Const(7));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(-8i32)); // ~7 in two's complement
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(padded));

  // payload = 8 (byte_len f64) + padded_len
  let payload = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::LocalGet(padded));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(payload));

  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc_dynamic(ctx, payload, ptr, "string");

  // Store byte_len as f64 at ptr+0
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.ptr_to_f64(len_i32);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  let content_base = ctx.i32_offset(ptr, 8);

  (ptr, content_base)
}

/// `count` on a string — returns Unicode scalar count as f64.
pub(super) fn emit_str_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&str:count")?;
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(ptr));

  let byte_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(byte_len));

  let index = ctx.alloc_i32(0);
  let count = ctx.alloc_i32(0);
  ctx.emit(Instruction::Block(BlockType::Empty));
  ctx.emit(Instruction::Loop(BlockType::Empty));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::LocalGet(byte_len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  ctx.emit(Instruction::I32Const(0xc0));
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::I32Const(0x80));
  ctx.emit(Instruction::I32Ne);
  ctx.emit(Instruction::If(BlockType::Empty));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(count));
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(index));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// UTF-8 byte count is stored directly in the string header.
pub(super) fn emit_str_utf8_byte_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&str:utf8-byte-count")?;
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  Ok(())
}

/// `str-empty?` — returns 1.0 if byte_len == 0, else 0.0.
pub(super) fn emit_str_empty(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "str-empty?")?;
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

/// `&str:concat a b` — new string with a's bytes followed by b's bytes.
pub(super) fn emit_str_concat(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&str:concat")?;
  // Evaluate each arg as f64, then convert to string (matching interpreter behavior:
  // non-string args are turned to strings via turn_string).
  emit_expr(ctx, &args[0])?;
  let f_a = ctx.alloc_local_typed(ValType::F64);
  ctx.emit(Instruction::LocalSet(f_a));
  let ptr_a = emit_turn_string_from_local(ctx, f_a);

  emit_expr(ctx, &args[1])?;
  let f_b = ctx.alloc_local_typed(ValType::F64);
  ctx.emit(Instruction::LocalSet(f_b));
  let ptr_b = emit_turn_string_from_local(ctx, f_b);

  emit_str_concat_from_ptrs(ctx, ptr_a, ptr_b);
  Ok(())
}

/// Concatenate two known heap-string pointers and leave the resulting f64
/// pointer on the stack.
pub(super) fn emit_str_concat_from_ptrs(ctx: &mut WasmGenCtx, ptr_a: u32, ptr_b: u32) {
  let len_a = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(len_a));

  let len_b = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(len_b));

  let len_c = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(len_a));
  ctx.emit(Instruction::LocalGet(len_b));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(len_c));

  let (ptr_c, dst_c) = emit_str_alloc(ctx, len_c);

  // Copy a's bytes: memory.copy(dst_c, ptr_a+8, len_a)
  let src_a = ctx.i32_offset(ptr_a, 8);

  ctx.emit(Instruction::LocalGet(dst_c));
  ctx.emit(Instruction::LocalGet(src_a));
  ctx.emit(Instruction::LocalGet(len_a));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  // Copy b's bytes: memory.copy(dst_c + len_a, ptr_b+8, len_b)
  let dst_b = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst_c));
  ctx.emit(Instruction::LocalGet(len_a));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(dst_b));

  let src_b = ctx.i32_offset(ptr_b, 8);

  ctx.emit(Instruction::LocalGet(dst_b));
  ctx.emit(Instruction::LocalGet(src_b));
  ctx.emit(Instruction::LocalGet(len_b));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  ctx.ptr_to_f64(ptr_c);
}

/// Reject invalid numbers and saturate large valid indices after all call
/// arguments have been evaluated. This preserves eager argument effects.
/// A string cannot contain more than u32::MAX bytes in wasm32 memory.
fn emit_str_index(ctx: &mut WasmGenCtx, value: u32) -> u32 {
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::F64Trunc);
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(f64_const(f64::INFINITY));
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  let index = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncSatF64U);
  ctx.emit(Instruction::LocalSet(index));
  index
}

fn emit_str_index_value(ctx: &mut WasmGenCtx, expr: &Calcit) -> Result<u32, String> {
  let value = ctx.alloc_local();
  emit_expr(ctx, expr)?;
  ctx.emit(Instruction::LocalSet(value));
  Ok(value)
}

/// Advance a byte offset by one UTF-8 scalar, clamping at byte_len. Every byte
/// load is guarded independently; empty strings never read or subtract a byte.
fn emit_str_advance_scalar(ctx: &mut WasmGenCtx, ptr: u32, len: u32, offset: u32) {
  ctx.emit(Instruction::LocalGet(offset));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32LtU);
  ctx.emit(Instruction::If(BlockType::Empty));
  ctx.emit(Instruction::LocalGet(offset));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(offset));
  ctx.emit(Instruction::Block(BlockType::Empty));
  ctx.emit(Instruction::Loop(BlockType::Empty));
  ctx.emit(Instruction::LocalGet(offset));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(offset));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  ctx.emit(Instruction::I32Const(0xc0));
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::I32Const(0x80));
  ctx.emit(Instruction::I32Ne);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(offset));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(offset));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
}

/// Convert a scalar index to a bounded byte offset without allocating text.
fn emit_str_scalar_offset(ctx: &mut WasmGenCtx, ptr: u32, len: u32, index: u32) -> u32 {
  let offset = ctx.alloc_i32(0);
  let scalar = ctx.alloc_i32(0);
  ctx.emit(Instruction::Block(BlockType::Empty));
  ctx.emit(Instruction::Loop(BlockType::Empty));
  ctx.emit(Instruction::LocalGet(scalar));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(offset));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  emit_str_advance_scalar(ctx, ptr, len, offset);
  ctx.emit(Instruction::LocalGet(scalar));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(scalar));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  offset
}

/// Copy a range of already bounded scalar-aligned byte offsets. Reversed
/// ranges produce an empty string without signed subtraction or underflow.
fn emit_str_copy_range(ctx: &mut WasmGenCtx, ptr: u32, start: u32, end: u32) {
  let size = ctx.alloc_i32(0);
  ctx.emit(Instruction::LocalGet(end));
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::If(BlockType::Empty));
  ctx.emit(Instruction::LocalGet(end));
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(size));
  ctx.emit(Instruction::End);
  let (result, dst) = emit_str_alloc(ctx, size);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(size));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  ctx.ptr_to_f64(result);
}

/// `&str:nth str idx` — one scalar as a string, or nil when out of range.
pub(super) fn emit_str_nth(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&str:nth")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;

  let value = emit_str_index_value(ctx, &args[1])?;
  let index = emit_str_index(ctx, value);
  let len = emit_load_count_i32(ctx, ptr);
  let start = emit_str_scalar_offset(ctx, ptr, len, index);
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::If(BlockType::Result(ValType::F64)));
  ctx.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  ctx.emit(Instruction::Else);

  let end = ctx.i32_offset(start, 0);
  emit_str_advance_scalar(ctx, ptr, len, end);
  emit_str_copy_range(ctx, ptr, start, end);
  ctx.emit(Instruction::End);
  Ok(())
}

/// `&str:first str` — first scalar as a string, or nil.
pub(super) fn emit_str_first(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&str:first")?;
  emit_str_nth(ctx, &[args[0].clone(), Calcit::Number(0.0)])
}

/// `&str:rest str` — new string without the first scalar; empty stays empty.
pub(super) fn emit_str_rest(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&str:rest")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  emit_str_rest_from_ptr(ctx, ptr);
  Ok(())
}

pub(super) fn emit_str_rest_from_ptr(ctx: &mut WasmGenCtx, ptr: u32) {
  let len = emit_load_count_i32(ctx, ptr);
  let start = ctx.alloc_i32(0);
  emit_str_advance_scalar(ctx, ptr, len, start);
  emit_str_copy_range(ctx, ptr, start, len);
}

/// `&str:slice str start end` — scalar range [start, end), clamped to the text.
pub(super) fn emit_str_slice(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 && args.len() != 2 {
    return Err("&str:slice expects 2 or 3 args (str, start[, end])".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let from_value = emit_str_index_value(ctx, &args[1])?;
  let to_value = args.get(2).map(|expr| emit_str_index_value(ctx, expr)).transpose()?;
  let from = emit_str_index(ctx, from_value);
  let to = to_value.map(|value| emit_str_index(ctx, value));
  let len = emit_load_count_i32(ctx, ptr);
  let start = emit_str_scalar_offset(ctx, ptr, len, from);
  let end = to.map(|index| emit_str_scalar_offset(ctx, ptr, len, index)).unwrap_or(len);
  emit_str_copy_range(ctx, ptr, start, end);
  Ok(())
}

/// `&str:compare a b` — lexicographic byte comparison; returns -1.0 / 0.0 / 1.0.
pub(super) fn emit_str_compare(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&str:compare")?;
  let ptr_a = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_b = emit_ptr_to_i32(ctx, &args[1])?;
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.call_rt("__rt_str_compare");
  Ok(())
}

/// `&str:contains? str idx` — whether the scalar index is within the string.
pub(super) fn emit_str_contains(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&str:contains?")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let value = emit_str_index_value(ctx, &args[1])?;
  let index = emit_str_index(ctx, value);
  let len = emit_load_count_i32(ctx, ptr);
  let offset = emit_str_scalar_offset(ctx, ptr, len, index);
  ctx.emit(Instruction::LocalGet(offset));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32LtU);
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:find-index haystack needle` — byte offset of first occurrence, or -1.0.
pub(super) fn emit_str_find_index(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&str:find-index")?;
  let ptr_h = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_n = emit_ptr_to_i32(ctx, &args[1])?;
  ctx.emit(Instruction::LocalGet(ptr_h));
  ctx.emit(Instruction::LocalGet(ptr_n));
  ctx.call_rt("__rt_str_find_index");
  Ok(())
}

/// `&str:includes? haystack needle` — 1.0 if needle appears in haystack, else 0.0.
pub(super) fn emit_str_includes(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&str:includes?")?;
  let ptr_h = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_n = emit_ptr_to_i32(ctx, &args[1])?;
  ctx.emit(Instruction::LocalGet(ptr_h));
  ctx.emit(Instruction::LocalGet(ptr_n));
  ctx.call_rt("__rt_str_find_index");
  // result >= 0.0 → 1 or 0, convert to f64
  ctx.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `starts-with? s prefix` — returns 1.0 if s starts with prefix, else 0.0.
pub(super) fn emit_str_starts_with(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "starts-with?")?;
  let ptr_s = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_p = emit_ptr_to_i32(ctx, &args[1])?;
  ctx.emit(Instruction::LocalGet(ptr_s));
  ctx.emit(Instruction::LocalGet(ptr_p));
  ctx.call_rt("__rt_str_starts_with");
  Ok(())
}

/// `ends-with? s suffix` — returns 1.0 if s ends with suffix, else 0.0.
pub(super) fn emit_str_ends_with(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "ends-with?")?;
  let ptr_s = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_suf = emit_ptr_to_i32(ctx, &args[1])?;
  ctx.emit(Instruction::LocalGet(ptr_s));
  ctx.emit(Instruction::LocalGet(ptr_suf));
  ctx.call_rt("__rt_str_ends_with");
  Ok(())
}

/// `turn-string v` — convert any value to its string representation.
pub(super) fn emit_turn_string(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "turn-string")?;

  // Compile-time fast path: literal enum constructor like `(:: :tag val0 val1 ...)`.
  // Pre-compute the lispy string and emit it as a string-pool constant.
  if let Some(enum_str) = super::try_format_enum_literal(&args[0])
    && let Some(&ptr) = ctx.string_pool.get(&enum_str)
  {
    ctx.emit(super::f64_const(ptr as f64));
    return Ok(());
  }

  let f64_to_str_idx = *ctx
    .runtime_fn_index
    .get("__rt_f64_to_str")
    .unwrap_or_else(|| panic!("runtime helper __rt_f64_to_str not found"));

  let string_type_tag = *ctx.tag_index.get("string").ok_or("string tag missing")? as i32;

  let v = ctx.alloc_local(); // f64 -- the argument value
  let result = ctx.alloc_local(); // f64 -- the output string ptr (0.0 = not yet set)
  let v_i32 = ctx.alloc_local_typed(ValType::I32); // v truncated to i32
  let raw_base = ctx.alloc_local_typed(ValType::I32); // raw_base = v_i32 - 8

  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(v));

  // --- Check if v is already a heap string ---
  // Use f64 comparison first (avoids unsafe truncation for small/negative values)
  let heap_min = (HEAP_BASE as f64) + 8.0;
  ctx.emit(Instruction::LocalGet(v));
  ctx.emit(f64_const(heap_min));
  ctx.emit(Instruction::F64Ge);
  // AND floor(v) == v (integer-valued pointer)
  ctx.emit(Instruction::LocalGet(v));
  ctx.emit(Instruction::F64Floor);
  ctx.emit(Instruction::LocalGet(v));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  {
    ctx.emit(Instruction::LocalGet(v));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(v_i32));
    ctx.emit(Instruction::LocalGet(v_i32));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Sub);
    ctx.emit(Instruction::LocalSet(raw_base));
    // Check HEAP_MAGIC AND type_tag == string_tag
    ctx.emit(Instruction::LocalGet(raw_base));
    ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
    ctx.emit(Instruction::I32Const(HEAP_MAGIC));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::LocalGet(raw_base));
    ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
    ctx.emit(Instruction::I32Const(string_type_tag));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::I32And);
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(v));
    ctx.emit(Instruction::LocalSet(result));
    ctx.emit(Instruction::End); // inner if
  }
  ctx.emit(Instruction::End); // outer if

  // If result is still 0.0, convert value to string
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.begin_block_if();
  {
    // nil/false (0.0) -> allocate empty string ""
    ctx.emit(Instruction::LocalGet(v));
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Eq);
    ctx.begin_block_if();
    {
      let raw = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
      ctx.emit(Instruction::LocalTee(raw));
      ctx.emit(Instruction::I32Const(HEAP_MAGIC));
      ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
      ctx.emit(Instruction::LocalGet(raw));
      ctx.emit(Instruction::I32Const(string_type_tag));
      ctx.emit(Instruction::I32Store(mem_arg_i32(4)));
      // write byte_len = 0.0 at raw+8
      ctx.emit(Instruction::LocalGet(raw));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::F64Const(wasm_encoder::Ieee64::from(0.0f64)));
      ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
      // advance heap_ptr by 16 (header=8 + byte_len_slot=8)
      ctx.emit(Instruction::LocalGet(raw));
      ctx.emit(Instruction::I32Const(16));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
      // result = (raw + 8) as f64
      ctx.emit(Instruction::LocalGet(raw));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalSet(result));
    }
    ctx.emit(Instruction::Else);
    // Non-nil non-string -> call __rt_f64_to_str
    ctx.emit(Instruction::LocalGet(v));
    ctx.emit(Instruction::Call(f64_to_str_idx));
    ctx.emit(Instruction::LocalSet(result));
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

pub(super) fn emit_format_to_lisp(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  // Fast path: `(format-to-lisp (quote X))` — the result is a compile-time constant string.
  // The assert= macro always calls this with a quoted expression for error display.
  if args.len() == 1
    && let Calcit::List(inner) = &args[0]
    && inner.len() >= 2
    && let Calcit::Syntax(CalcitSyntax::Quote, _) = &inner[0]
  {
    let s = crate::calcit::format_to_lisp(&inner[1]);
    // Look up in string pool (pre-interned by collect_strings_from_expr)
    if let Some(&ptr) = ctx.string_pool.get(&s) {
      ctx.emit(super::f64_const(ptr as f64));
      return Ok(());
    }
  }
  // Fallback: evaluate args for side-effects and return nil
  ctx.stub_proc(args)
}

/// `&list:distinct xs` — return new list with duplicate elements removed (O(n²)).
/// `&str:pad-left str target-size pattern` — pads str on the left.
pub(super) fn emit_str_pad_left(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&str:pad-left")?;
  let ptr_s = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_p = emit_ptr_to_i32(ctx, &args[2])?;
  ctx.emit(Instruction::LocalGet(ptr_s));
  // target_size as i32
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalGet(ptr_p));
  ctx.call_rt("__rt_str_pad_left");
  Ok(())
}

/// `&str:pad-right str target-size pattern` — pads str on the right.
pub(super) fn emit_str_pad_right(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&str:pad-right")?;
  let ptr_s = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_p = emit_ptr_to_i32(ctx, &args[2])?;
  ctx.emit(Instruction::LocalGet(ptr_s));
  // target_size as i32
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalGet(ptr_p));
  ctx.call_rt("__rt_str_pad_right");
  Ok(())
}

/// Internal helper: concat two string i32 pointer locals into a new string.
/// Leaves the result f64 pointer on the stack.
fn concat_two_i32_ptrs(ctx: &mut WasmGenCtx, ptr_a: u32, ptr_b: u32) {
  let len_a = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(len_a));

  let len_b = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(len_b));

  let len_c = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(len_a));
  ctx.emit(Instruction::LocalGet(len_b));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(len_c));

  let (ptr_c, dst_c) = emit_str_alloc(ctx, len_c);

  let src_a = ctx.i32_offset(ptr_a, 8);
  ctx.emit(Instruction::LocalGet(dst_c));
  ctx.emit(Instruction::LocalGet(src_a));
  ctx.emit(Instruction::LocalGet(len_a));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  let dst_b_off = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst_c));
  ctx.emit(Instruction::LocalGet(len_a));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(dst_b_off));

  let src_b = ctx.i32_offset(ptr_b, 8);

  ctx.emit(Instruction::LocalGet(dst_b_off));
  ctx.emit(Instruction::LocalGet(src_b));
  ctx.emit(Instruction::LocalGet(len_b));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  ctx.ptr_to_f64(ptr_c);
}

/// Convert a pre-evaluated f64 value local to a string i32 pointer local.
/// Equivalent to `emit_turn_string` but takes a local instead of a Calcit expression.
pub(super) fn emit_turn_string_from_local(ctx: &mut WasmGenCtx, v_local: u32) -> u32 {
  use crate::calcit::{CalcitLocal, CalcitSymbolInfo, DYNAMIC_TYPE};
  use std::sync::Arc;
  // Map a temporary name to the pre-evaluated local
  let sym: Arc<str> = Arc::from("__ts_arg__");
  let dummy_info = Arc::new(CalcitSymbolInfo {
    at_ns: Arc::from("wasm"),
    at_def: Arc::from("__ts"),
  });
  let prev = ctx.locals.insert(sym.as_ref().to_owned(), v_local);
  let expr = crate::calcit::Calcit::Local(CalcitLocal {
    idx: 0,
    sym: sym.clone(),
    info: dummy_info,
    location: None,
    type_info: DYNAMIC_TYPE.clone(),
  });
  let _ = emit_turn_string(ctx, std::slice::from_ref(&expr));
  match prev {
    Some(v) => {
      ctx.locals.insert(sym.as_ref().to_owned(), v);
    }
    None => {
      ctx.locals.remove(sym.as_ref());
    }
  }
  // emit_turn_string leaves f64 on stack; convert to i32 local
  let str_i32 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(str_i32));
  str_i32
}

/// `str args...` — convert each non-nil arg to string and concatenate all.
/// Intercept for the variadic `str` call, bypassing the core library definition.
pub(super) fn emit_str_variadic(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  // Filter nil args at codegen time (matching interpreter behavior)
  let non_nil: Vec<&Calcit> = args.iter().filter(|a| !matches!(a, Calcit::Nil)).collect();

  if non_nil.is_empty() {
    // Return empty string: allocate string with length 0
    let zero_local = ctx.alloc_i32(0);
    let (ptr, _) = emit_str_alloc(ctx, zero_local);
    ctx.ptr_to_f64(ptr);
    return Ok(());
  }

  // Convert first arg to string, store as i32 ptr
  emit_turn_string(ctx, std::slice::from_ref(non_nil[0]))?;
  let acc_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(acc_ptr));

  for arg in &non_nil[1..] {
    emit_turn_string(ctx, std::slice::from_ref(arg))?;
    let next_ptr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(next_ptr));

    concat_two_i32_ptrs(ctx, acc_ptr, next_ptr);
    // result f64 on stack → convert to i32 and update acc
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(acc_ptr));
  }

  ctx.ptr_to_f64(acc_ptr);
  Ok(())
}

/// `str-spaced args...` — convert each non-nil arg to string, join with space separators.
pub(super) fn emit_str_spaced(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  // Filter out nil args at codegen time (matching interpreter behavior)
  let non_nil: Vec<&Calcit> = args.iter().filter(|a| !matches!(a, Calcit::Nil)).collect();

  if non_nil.is_empty() {
    let zero_local = ctx.alloc_i32(0);
    let (ptr, _) = emit_str_alloc(ctx, zero_local);
    ctx.ptr_to_f64(ptr);
    return Ok(());
  }

  // Allocate a 1-byte space string inline in WASM memory
  let one_local = ctx.alloc_i32(1);
  let (space_ptr, space_content) = emit_str_alloc(ctx, one_local);
  // Write 0x20 (ASCII space) at content_base
  ctx.emit(Instruction::LocalGet(space_content));
  ctx.emit(Instruction::I32Const(0x20));
  ctx.emit(Instruction::I32Store8(super::mem_arg_byte(0)));

  // Convert first arg to string, store as i32 ptr
  emit_turn_string(ctx, std::slice::from_ref(non_nil[0]))?;
  let acc_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(acc_ptr));

  for arg in &non_nil[1..] {
    // Append space: acc = acc + " "
    concat_two_i32_ptrs(ctx, acc_ptr, space_ptr);
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(acc_ptr));

    // Append next arg
    emit_turn_string(ctx, std::slice::from_ref(arg))?;
    let next_ptr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(next_ptr));
    concat_two_i32_ptrs(ctx, acc_ptr, next_ptr);
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(acc_ptr));
  }

  ctx.ptr_to_f64(acc_ptr);
  Ok(())
}

// ===========================================================================
// __str_new — exported FFI helper for JS → WASM string passing.
//
// JS protocol:
//   1. Read heap top: `const top = inst.exports.__heap_ptr.value`
//   2. Write UTF-8 bytes at `top + 16` (after the 8-byte header and 8-byte byte_len)
//   3. Call `inst.exports.__str_new(top + 16, byteLen)` → returns f64 logical pointer
//
// The zero-copy path: when src_ptr == logical_ptr + 8, memory.copy is a no-op.
// JS may also write to any other scratch address and pass that as src_ptr.
// ===========================================================================

/// Build the `__str_new(src_ptr: i32, byte_len: i32) → f64` runtime function.
/// Copies `byte_len` bytes from `src_ptr` into a new heap-allocated tagged string.
pub(super) fn build_str_new_fn(str_tag_id: i32, component_cabi_realloc_index: Option<u32>) -> CompiledFn {
  build_tagged_bytes_new_fn(str_tag_id, Some("__str_new"), component_cabi_realloc_index)
}

/// Build the internal constructor used to lift Canonical ABI `list<u8>` values.
pub(super) fn build_component_buffer_new_fn(buffer_tag_id: i32, cabi_realloc_index: u32) -> CompiledFn {
  build_tagged_bytes_new_fn(buffer_tag_id, None, Some(cabi_realloc_index))
}

fn build_tagged_bytes_new_fn(type_tag_id: i32, export_name: Option<&str>, cabi_realloc_index: Option<u32>) -> CompiledFn {
  // params: 0 = src_ptr (i32), 1 = byte_len (i32)
  // locals: 2 = padded, 3 = payload, 4 = allocation_size, 5 = raw_base, 6 = ptr
  let mut instructions = vec![
    // padded = (byte_len + 7) & -8
    Instruction::LocalGet(1),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::LocalTee(2),
    Instruction::LocalGet(1),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(2),
    Instruction::I32Const(-8i32),
    Instruction::I32And,
    Instruction::LocalSet(2), // padded
    // payload = 8 + padded
    Instruction::I32Const(8),
    Instruction::LocalGet(2),
    Instruction::I32Add,
    Instruction::LocalTee(3), // payload
    Instruction::LocalGet(2),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    // allocation_size = 8-byte header + payload
    Instruction::LocalGet(3),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalTee(4),
    Instruction::LocalGet(3),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ];
  if let Some(cabi_realloc_index) = cabi_realloc_index {
    instructions.extend([
      Instruction::I32Const(0),
      Instruction::I32Const(0),
      Instruction::I32Const(8),
      Instruction::LocalGet(4),
      Instruction::Call(cabi_realloc_index),
      Instruction::LocalSet(5),
    ]);
  } else {
    instructions.extend([
      Instruction::GlobalGet(HEAP_PTR_GLOBAL),
      Instruction::LocalTee(5),
      Instruction::LocalGet(4),
      Instruction::I32Add,
      Instruction::LocalTee(6),
      Instruction::LocalGet(5),
      Instruction::I32LtU,
      Instruction::If(BlockType::Empty),
      Instruction::Unreachable,
      Instruction::End,
      Instruction::LocalGet(6),
      Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    ]);
  }
  instructions.extend([
    // Write HEAP_MAGIC and the type tag at raw_base.
    Instruction::LocalGet(5),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(5),
    Instruction::I32Const(type_tag_id),
    Instruction::I32Store(mem_arg_i32(4)),
    // ptr = raw_base + 8
    Instruction::LocalGet(5),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(6),
    // Store byte_len as f64 at ptr+0
    Instruction::LocalGet(6),
    Instruction::LocalGet(1), // byte_len
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    // memory.copy(dst = ptr+8, src = src_ptr, n = byte_len)
    Instruction::LocalGet(6),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(0), // src_ptr
    Instruction::LocalGet(1), // byte_len
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    // return logical_ptr as f64
    Instruction::LocalGet(6),
    Instruction::F64ConvertI32U,
  ]);

  CompiledFn {
    export_name: export_name.map(str::to_owned),
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![ValType::I32; 5],
    instructions,
  }
}

/// Build `__rt_str_pad_left(str_ptr: i32, target_size: i32, pattern_ptr: i32) → f64`.
/// Pads `str` on the left with repeating `pattern` bytes until `target_size` total bytes.
/// If `str` is already >= `target_size`, returns the original pointer unchanged.
pub(super) fn build_str_pad_left_fn(str_tag_id: i32) -> CompiledFn {
  // params: 0=str_ptr(i32), 1=target_size(i32), 2=pattern_ptr(i32)
  // locals: 3=str_len, 4=pad_size, 5=pat_len, 6=padded, 7=payload,
  //         8=new_ptr, 9=dst_base, 10=i, 11=j, 12=byte_val  (all i32)
  let instructions = vec![
    // str_len = i32(f64.load str_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // Block $outer (result f64) — for early return
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if str_len >= target_size: return f64(str_ptr)
    Instruction::LocalGet(3),
    Instruction::LocalGet(1),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::F64ConvertI32U,
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // pad_size = target_size - str_len
    Instruction::LocalGet(1),
    Instruction::LocalGet(3),
    Instruction::I32Sub,
    Instruction::LocalSet(4),
    // pat_len = i32(f64.load pattern_ptr+0)
    Instruction::LocalGet(2),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(5),
    // if pat_len == 0: return original (guard)
    Instruction::LocalGet(5),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::F64ConvertI32U,
    Instruction::Br(1),
    Instruction::End,
    // padded = (target_size + 7) & -8
    Instruction::LocalGet(1),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::I32Const(-8i32),
    Instruction::I32And,
    Instruction::LocalSet(6),
    // payload = 8 + padded
    Instruction::I32Const(8),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::LocalSet(7),
    // Write HEAP_MAGIC at heap_ptr
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    // Write str_tag_id at heap_ptr+4
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(str_tag_id),
    Instruction::I32Store(mem_arg_i32(4)),
    // new_ptr = heap_ptr + 8; advance heap by 8 + payload
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(8), // new_ptr
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(8),
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Add,
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    // Write byte_len (target_size as f64) at new_ptr+0
    Instruction::LocalGet(8),
    Instruction::LocalGet(1),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    // dst_base = new_ptr + 8
    Instruction::LocalGet(8),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(9),
    // i = 0, j = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(10),
    Instruction::I32Const(0),
    Instruction::LocalSet(11),
    // Fill pad_size bytes with pattern (cycling)
    Instruction::Block(wasm_encoder::BlockType::Empty), // $pad_break
    Instruction::Loop(wasm_encoder::BlockType::Empty),  // $pad_loop
    // if i >= pad_size: break
    Instruction::LocalGet(10),
    Instruction::LocalGet(4),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    // if j >= pat_len: j = 0
    Instruction::LocalGet(11),
    Instruction::LocalGet(5),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(11),
    Instruction::End,
    // byte_val = (pattern_ptr+8)[j]
    Instruction::LocalGet(2),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(11),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(12),
    // dst_base[i] = byte_val
    Instruction::LocalGet(9),
    Instruction::LocalGet(10),
    Instruction::I32Add,
    Instruction::LocalGet(12),
    Instruction::I32Store8(mem_arg_byte(0)),
    // i++, j++
    Instruction::LocalGet(10),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(10),
    Instruction::LocalGet(11),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(11),
    Instruction::Br(0), // continue $pad_loop
    Instruction::End,   // $pad_loop
    Instruction::End,   // $pad_break
    // Copy original string bytes after the pad region
    // memory.copy(dst = dst_base + pad_size, src = str_ptr+8, n = str_len)
    Instruction::LocalGet(9),
    Instruction::LocalGet(4),
    Instruction::I32Add, // dst = dst_base + pad_size
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,      // src = str_ptr + 8
    Instruction::LocalGet(3), // str_len
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    // return f64(new_ptr)
    Instruction::LocalGet(8),
    Instruction::F64ConvertI32U,
    Instruction::End, // end $outer
  ];
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // str_len (3)
      ValType::I32, // pad_size (4)
      ValType::I32, // pat_len (5)
      ValType::I32, // padded (6)
      ValType::I32, // payload (7)
      ValType::I32, // new_ptr (8)
      ValType::I32, // dst_base (9)
      ValType::I32, // i (10)
      ValType::I32, // j (11)
      ValType::I32, // byte_val (12)
    ],
    instructions,
  }
}

/// Build `__rt_str_pad_right(str_ptr: i32, target_size: i32, pattern_ptr: i32) → f64`.
/// Pads `str` on the right with repeating `pattern` bytes until `target_size` total bytes.
pub(super) fn build_str_pad_right_fn(str_tag_id: i32) -> CompiledFn {
  // params: 0=str_ptr(i32), 1=target_size(i32), 2=pattern_ptr(i32)
  // Same locals layout as pad_left.
  let instructions = vec![
    // str_len = i32(f64.load str_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // Block $outer (result f64)
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if str_len >= target_size: return f64(str_ptr)
    Instruction::LocalGet(3),
    Instruction::LocalGet(1),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::F64ConvertI32U,
    Instruction::Br(1),
    Instruction::End,
    // pad_size = target_size - str_len
    Instruction::LocalGet(1),
    Instruction::LocalGet(3),
    Instruction::I32Sub,
    Instruction::LocalSet(4),
    // pat_len = i32(f64.load pattern_ptr+0)
    Instruction::LocalGet(2),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(5),
    // if pat_len == 0: return original (guard)
    Instruction::LocalGet(5),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::F64ConvertI32U,
    Instruction::Br(1),
    Instruction::End,
    // padded = (target_size + 7) & -8
    Instruction::LocalGet(1),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::I32Const(-8i32),
    Instruction::I32And,
    Instruction::LocalSet(6),
    // payload = 8 + padded
    Instruction::I32Const(8),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::LocalSet(7),
    // Write HEAP_MAGIC at heap_ptr
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    // Write str_tag_id at heap_ptr+4
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(str_tag_id),
    Instruction::I32Store(mem_arg_i32(4)),
    // new_ptr = heap_ptr + 8; advance heap
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(8), // new_ptr
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(8),
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Add,
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    // Write byte_len (target_size as f64) at new_ptr+0
    Instruction::LocalGet(8),
    Instruction::LocalGet(1),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    // dst_base = new_ptr + 8
    Instruction::LocalGet(8),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(9),
    // Copy original string to the start of dst_base
    Instruction::LocalGet(9), // dst = dst_base
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,      // src = str_ptr + 8
    Instruction::LocalGet(3), // n = str_len
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    // i = 0, j = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(10),
    Instruction::I32Const(0),
    Instruction::LocalSet(11),
    // Fill pad_size bytes with pattern after the original
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(10),
    Instruction::LocalGet(4),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    // if j >= pat_len: j = 0
    Instruction::LocalGet(11),
    Instruction::LocalGet(5),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(11),
    Instruction::End,
    // byte_val = (pattern_ptr+8)[j]
    Instruction::LocalGet(2),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(11),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(12),
    // dst_base[str_len + i] = byte_val
    Instruction::LocalGet(9),
    Instruction::LocalGet(3), // str_len
    Instruction::I32Add,
    Instruction::LocalGet(10), // i
    Instruction::I32Add,
    Instruction::LocalGet(12),
    Instruction::I32Store8(mem_arg_byte(0)),
    // i++, j++
    Instruction::LocalGet(10),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(10),
    Instruction::LocalGet(11),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(11),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    // return f64(new_ptr)
    Instruction::LocalGet(8),
    Instruction::F64ConvertI32U,
    Instruction::End, // end $outer
  ];
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // str_len (3)
      ValType::I32, // pad_size (4)
      ValType::I32, // pat_len (5)
      ValType::I32, // padded (6)
      ValType::I32, // payload (7)
      ValType::I32, // new_ptr (8)
      ValType::I32, // dst_base (9)
      ValType::I32, // i (10)
      ValType::I32, // j (11)
      ValType::I32, // byte_val (12)
    ],
    instructions,
  }
}

/// Core body for `join-str xs sep` — local 0 = xs (f64), local 1 = sep (f64 str).
pub(super) fn emit_join_str_from_locals(ctx: &mut WasmGenCtx, xs_f64: u32, sep_f64: u32) -> Result<(), String> {
  let xs_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(xs_f64));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(xs_ptr));
  let count = emit_load_count_i32(ctx, xs_ptr);

  // result starts as "" (empty string in pool or alloc 0-length string)
  let result = ctx.alloc_local();
  // Alloc an empty string
  let zero = ctx.alloc_i32(0);
  let (empty_ptr, _) = emit_str_alloc(ctx, zero);
  ctx.ptr_to_f64(empty_ptr);
  ctx.emit(Instruction::LocalSet(result));

  let sep_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(sep_f64));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(sep_ptr));

  let i = ctx.alloc_i32(0);

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // if i > 0: result = concat(result, sep)
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  let result_ptr_for_sep = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(result_ptr_for_sep));
  concat_two_i32_ptrs(ctx, result_ptr_for_sep, sep_ptr);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::End);

  // elem = xs[i], turn to string
  let elem = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(xs_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(elem));

  // elem_str = turn_string(elem)
  let elem_arr = [crate::calcit::Calcit::Nil]; // dummy
  let _ = elem_arr;
  // Use emit_turn_string_from_f64_local instead (inline the conversion)
  let elem_str_ptr = emit_turn_string_from_local(ctx, elem);

  // result = concat(result, elem_str)
  let result_ptr2 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(result_ptr2));
  concat_two_i32_ptrs(ctx, result_ptr2, elem_str_ptr);
  ctx.emit(Instruction::LocalSet(result));

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `join-str xs sep` — call-site intercept.
pub(super) fn emit_join_str(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "join-str")?;
  let xs = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(xs));
  let sep = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(sep));
  emit_join_str_from_locals(ctx, xs, sep)
}

/// `trim` — strips whitespace (1 arg) or a specific character (2 args) from both ends.
pub(super) fn emit_trim(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() == 1 {
    emit_expr(ctx, &args[0])?;
    ctx.call_rt("__rt_trim_ws");
    Ok(())
  } else if args.len() == 2 {
    emit_expr(ctx, &args[0])?;
    emit_expr(ctx, &args[1])?;
    ctx.call_rt("__rt_trim_char");
    Ok(())
  } else {
    Err(format!("trim expects 1 or 2 args, got {}", args.len()))
  }
}

/// `blank?` — returns 1.0 if all bytes are ASCII whitespace, else 0.0.
pub(super) fn emit_blank(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "blank?")?;
  emit_expr(ctx, &args[0])?;
  ctx.call_rt("__rt_blank");
  Ok(())
}

/// `get-char-code` — returns the Unicode code point of the first character as f64.
/// Handles full UTF-8 decoding (1-, 2-, 3-, 4-byte sequences).
pub(super) fn emit_get_char_code(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "get-char-code")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  // content base = ptr + 8
  let content = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(content));

  let b0 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(content));
  ctx.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  ctx.emit(Instruction::LocalSet(b0));

  let result = ctx.alloc_local_typed(ValType::I32);

  // if b0 < 0x80: ASCII, result = b0
  ctx.emit(Instruction::LocalGet(b0));
  ctx.emit(Instruction::I32Const(0x80));
  ctx.emit(Instruction::I32LtU);
  ctx.emit(Instruction::If(BlockType::Empty));
  ctx.emit(Instruction::LocalGet(b0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Else);
  // elif b0 < 0xE0: 2-byte sequence
  ctx.emit(Instruction::LocalGet(b0));
  ctx.emit(Instruction::I32Const(0xE0));
  ctx.emit(Instruction::I32LtU);
  ctx.emit(Instruction::If(BlockType::Empty));
  {
    let b1 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(content));
    ctx.emit(Instruction::I32Load8U(mem_arg_byte(1)));
    ctx.emit(Instruction::LocalSet(b1));
    // result = (b0 & 0x1F) << 6 | (b1 & 0x3F)
    ctx.emit(Instruction::LocalGet(b0));
    ctx.emit(Instruction::I32Const(0x1F));
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::I32Const(6));
    ctx.emit(Instruction::I32Shl);
    ctx.emit(Instruction::LocalGet(b1));
    ctx.emit(Instruction::I32Const(0x3F));
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::I32Or);
    ctx.emit(Instruction::LocalSet(result));
  }
  ctx.emit(Instruction::Else);
  // elif b0 < 0xF0: 3-byte sequence
  ctx.emit(Instruction::LocalGet(b0));
  ctx.emit(Instruction::I32Const(0xF0));
  ctx.emit(Instruction::I32LtU);
  ctx.emit(Instruction::If(BlockType::Empty));
  {
    let b1 = ctx.alloc_local_typed(ValType::I32);
    let b2 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(content));
    ctx.emit(Instruction::I32Load8U(mem_arg_byte(1)));
    ctx.emit(Instruction::LocalSet(b1));
    ctx.emit(Instruction::LocalGet(content));
    ctx.emit(Instruction::I32Load8U(mem_arg_byte(2)));
    ctx.emit(Instruction::LocalSet(b2));
    // (b0 & 0x0F) << 12 | (b1 & 0x3F) << 6 | (b2 & 0x3F)
    ctx.emit(Instruction::LocalGet(b0));
    ctx.emit(Instruction::I32Const(0x0F));
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::I32Const(12));
    ctx.emit(Instruction::I32Shl);
    ctx.emit(Instruction::LocalGet(b1));
    ctx.emit(Instruction::I32Const(0x3F));
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::I32Const(6));
    ctx.emit(Instruction::I32Shl);
    ctx.emit(Instruction::I32Or);
    ctx.emit(Instruction::LocalGet(b2));
    ctx.emit(Instruction::I32Const(0x3F));
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::I32Or);
    ctx.emit(Instruction::LocalSet(result));
  }
  ctx.emit(Instruction::Else);
  {
    // 4-byte sequence: (b0 & 0x07) << 18 | (b1 & 0x3F) << 12 | (b2 & 0x3F) << 6 | (b3 & 0x3F)
    let b1 = ctx.alloc_local_typed(ValType::I32);
    let b2 = ctx.alloc_local_typed(ValType::I32);
    let b3 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(content));
    ctx.emit(Instruction::I32Load8U(mem_arg_byte(1)));
    ctx.emit(Instruction::LocalSet(b1));
    ctx.emit(Instruction::LocalGet(content));
    ctx.emit(Instruction::I32Load8U(mem_arg_byte(2)));
    ctx.emit(Instruction::LocalSet(b2));
    ctx.emit(Instruction::LocalGet(content));
    ctx.emit(Instruction::I32Load8U(mem_arg_byte(3)));
    ctx.emit(Instruction::LocalSet(b3));
    ctx.emit(Instruction::LocalGet(b0));
    ctx.emit(Instruction::I32Const(0x07));
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::I32Const(18));
    ctx.emit(Instruction::I32Shl);
    ctx.emit(Instruction::LocalGet(b1));
    ctx.emit(Instruction::I32Const(0x3F));
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::I32Const(12));
    ctx.emit(Instruction::I32Shl);
    ctx.emit(Instruction::I32Or);
    ctx.emit(Instruction::LocalGet(b2));
    ctx.emit(Instruction::I32Const(0x3F));
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::I32Const(6));
    ctx.emit(Instruction::I32Shl);
    ctx.emit(Instruction::I32Or);
    ctx.emit(Instruction::LocalGet(b3));
    ctx.emit(Instruction::I32Const(0x3F));
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::I32Or);
    ctx.emit(Instruction::LocalSet(result));
  }
  ctx.emit(Instruction::End); // end 3-byte/4-byte elif
  ctx.emit(Instruction::End); // end 2-byte elif
  ctx.emit(Instruction::End); // end ASCII if

  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `parse-float` — parse a decimal string to f64.
pub(super) fn emit_parse_float(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "parse-float")?;
  emit_expr(ctx, &args[0])?;
  ctx.call_rt("__rt_parse_float");
  Ok(())
}

/// `char-from-code` — encode a Unicode codepoint as a single-character heap string.
pub(super) fn emit_char_from_code(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "char-from-code")?;
  emit_expr(ctx, &args[0])?;
  ctx.call_rt("__rt_char_from_code");
  Ok(())
}

/// `&str:replace s pattern replacement` — replace all occurrences of pattern in s.
pub(super) fn emit_str_replace(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&str:replace")?;
  emit_expr(ctx, &args[0])?;
  emit_expr(ctx, &args[1])?;
  emit_expr(ctx, &args[2])?;
  ctx.call_rt("__rt_str_replace");
  Ok(())
}

/// `&str:escape s` — escape special characters in string, wrap in double quotes.
pub(super) fn emit_str_escape(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&str:escape")?;
  emit_expr(ctx, &args[0])?;
  ctx.call_rt("__rt_str_escape");
  Ok(())
}

/// `split s pat` — split string by pattern, return list of non-empty pieces.
pub(super) fn emit_split(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "split")?;
  let s = emit_ptr_to_i32(ctx, &args[0])?;
  let pat = emit_ptr_to_i32(ctx, &args[1])?;
  ctx.emit(Instruction::LocalGet(s));
  ctx.emit(Instruction::LocalGet(pat));
  ctx.call_rt("__rt_str_split");
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `split-lines s` — split string by newline ('\n'), return list of strings.
pub(super) fn emit_split_lines(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "split-lines")?;
  // Emit the string arg and a hardcoded "\n" literal, then call __rt_str_split.
  let s = emit_ptr_to_i32(ctx, &args[0])?;
  // Build a "\n" string (1 byte, value 0x0A) inline in the heap.
  // Allocate: size = 8 (byte_len:f64) + 8 (padded 1 byte) = 16 bytes payload.
  let nl_ptr = ctx.alloc_local_typed(wasm_encoder::ValType::I32);
  let one = ctx.alloc_i32(1);
  let (ptr_nl, cont_nl) = emit_str_alloc(ctx, one);
  // Store the '\n' byte at cont_nl + 0
  ctx.emit(Instruction::LocalGet(cont_nl));
  ctx.emit(Instruction::I32Const(0x0A)); // '\n'
  ctx.emit(Instruction::I32Store8(mem_arg_byte(0)));
  ctx.emit(Instruction::LocalGet(ptr_nl));
  ctx.emit(Instruction::LocalSet(nl_ptr));
  // Call __rt_str_split(s_ptr, nl_ptr) → i32 list
  ctx.emit(Instruction::LocalGet(s));
  ctx.emit(Instruction::LocalGet(nl_ptr));
  ctx.call_rt("__rt_str_split");
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}
