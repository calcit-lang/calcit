use super::*;

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
  ctx.emit(Instruction::LocalGet(len_i32));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  let content_base = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(content_base));

  (ptr, content_base)
}

/// `count` on a string — returns byte length as f64.
pub(super) fn emit_str_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&str:count expects 1 arg".into());
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0))); // byte_len at logical_ptr+0
  Ok(())
}

/// `str-empty?` — returns 1.0 if byte_len == 0, else 0.0.
pub(super) fn emit_str_empty(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("str-empty? expects 1 arg".into());
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

/// `&str:concat a b` — new string with a's bytes followed by b's bytes.
pub(super) fn emit_str_concat(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&str:concat expects 2 args".into());
  }
  let ptr_a = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_b = emit_ptr_to_i32(ctx, &args[1])?;

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
  let src_a = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_a));

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

  let src_b = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_b));

  ctx.emit(Instruction::LocalGet(dst_b));
  ctx.emit(Instruction::LocalGet(src_b));
  ctx.emit(Instruction::LocalGet(len_b));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  ctx.emit(Instruction::LocalGet(ptr_c));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:nth str idx` — byte value at index `idx` as f64 (UTF-8 byte, not char).
pub(super) fn emit_str_nth(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&str:nth expects 2 args".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  // addr = ptr + 8 + idx
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:first str` — first byte value as f64.
pub(super) fn emit_str_first(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&str:first expects 1 arg".into());
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Load8U(mem_arg_byte(8))); // offset 8 = first byte after byte_len
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:rest str` — new string without the first byte.
pub(super) fn emit_str_rest(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&str:rest expects 1 arg".into());
  }
  let ptr_a = emit_ptr_to_i32(ctx, &args[0])?;

  let old_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(old_len));

  let new_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(old_len));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_len));

  let (ptr_b, dst_b) = emit_str_alloc(ctx, new_len);

  // src = ptr_a + 8 + 1 (skip byte_len header + first byte)
  let src = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::I32Const(9));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src));

  ctx.emit(Instruction::LocalGet(dst_b));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalGet(new_len));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:slice str start end` — new string from bytes [start, end) (byte indices).
pub(super) fn emit_str_slice(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 && args.len() != 2 {
    return Err("&str:slice expects 2 or 3 args (str, start[, end])".into());
  }
  let ptr_a = emit_ptr_to_i32(ctx, &args[0])?;

  let start = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(start));

  let end = ctx.alloc_local_typed(ValType::I32);
  if args.len() == 3 {
    emit_expr(ctx, &args[2])?;
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(end));
  } else {
    // end defaults to the byte length of the string
    ctx.emit(Instruction::LocalGet(ptr_a));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(end));
  }

  let new_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(end));
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_len));

  let (ptr_b, dst_b) = emit_str_alloc(ctx, new_len);

  // src = ptr_a + 8 + start
  let src = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src));

  ctx.emit(Instruction::LocalGet(dst_b));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalGet(new_len));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:compare a b` — lexicographic byte comparison; returns -1.0 / 0.0 / 1.0.
pub(super) fn emit_str_compare(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&str:compare expects 2 args".into());
  }
  let ptr_a = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_b = emit_ptr_to_i32(ctx, &args[1])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_compare")
    .ok_or_else(|| "runtime helper __rt_str_compare not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::Call(fn_idx));
  Ok(())
}

/// `&str:contains? str idx` — 1.0 if the byte index is within string length, else 0.0.
pub(super) fn emit_str_contains(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&str:contains? expects 2 args (str, idx)".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  // byte_len as i32
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  // idx as i32
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  // byte_len > idx → 1 if in bounds, 0 otherwise
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:find-index haystack needle` — byte offset of first occurrence, or -1.0.
pub(super) fn emit_str_find_index(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&str:find-index expects 2 args (haystack, needle)".into());
  }
  let ptr_h = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_n = emit_ptr_to_i32(ctx, &args[1])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_find_index")
    .ok_or_else(|| "runtime helper __rt_str_find_index not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_h));
  ctx.emit(Instruction::LocalGet(ptr_n));
  ctx.emit(Instruction::Call(fn_idx));
  Ok(())
}

/// `&str:includes? haystack needle` — 1.0 if needle appears in haystack, else 0.0.
pub(super) fn emit_str_includes(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&str:includes? expects 2 args (haystack, needle)".into());
  }
  let ptr_h = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_n = emit_ptr_to_i32(ctx, &args[1])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_find_index")
    .ok_or_else(|| "runtime helper __rt_str_find_index not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_h));
  ctx.emit(Instruction::LocalGet(ptr_n));
  ctx.emit(Instruction::Call(fn_idx));
  // result >= 0.0 → 1 or 0, convert to f64
  ctx.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `starts-with? s prefix` — returns 1.0 if s starts with prefix, else 0.0.
pub(super) fn emit_str_starts_with(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("starts-with? expects 2 args (string, prefix)".into());
  }
  let ptr_s = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_p = emit_ptr_to_i32(ctx, &args[1])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_starts_with")
    .ok_or_else(|| "runtime helper __rt_str_starts_with not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_s));
  ctx.emit(Instruction::LocalGet(ptr_p));
  ctx.emit(Instruction::Call(fn_idx));
  Ok(())
}

/// `ends-with? s suffix` — returns 1.0 if s ends with suffix, else 0.0.
pub(super) fn emit_str_ends_with(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("ends-with? expects 2 args (string, suffix)".into());
  }
  let ptr_s = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_suf = emit_ptr_to_i32(ctx, &args[1])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_ends_with")
    .ok_or_else(|| "runtime helper __rt_str_ends_with not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_s));
  ctx.emit(Instruction::LocalGet(ptr_suf));
  ctx.emit(Instruction::Call(fn_idx));
  Ok(())
}

/// `turn-string v` / `&str v` — convert any value to its string representation.
/// Strings are returned as-is. nil/false → "". Numbers → decimal string.
pub(super) fn emit_turn_string(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("turn-string expects 1 arg".into());
  }

  let f64_to_str_idx = *ctx
    .runtime_fn_index
    .get("__rt_f64_to_str")
    .ok_or_else(|| "runtime helper __rt_f64_to_str not found".to_string())?;

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
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
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
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    ctx.emit(Instruction::LocalGet(v));
    ctx.emit(Instruction::LocalSet(result));
    ctx.emit(Instruction::End); // inner if
  }
  ctx.emit(Instruction::End); // outer if

  // If result is still 0.0, convert value to string
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    // nil/false (0.0) -> allocate empty string ""
    ctx.emit(Instruction::LocalGet(v));
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
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
  // Evaluate args for side effects, then return nil
  for arg in args {
    emit_expr(ctx, arg)?;
    ctx.emit(Instruction::Drop);
  }
  ctx.emit(f64_const(0.0));
  Ok(())
}

/// `&list:distinct xs` — return new list with duplicate elements removed (O(n²)).
/// `&str:pad-left str target-size pattern` — pads str on the left.
pub(super) fn emit_str_pad_left(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 {
    return Err("&str:pad-left expects 3 args (str, size, pattern)".into());
  }
  let ptr_s = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_p = emit_ptr_to_i32(ctx, &args[2])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_pad_left")
    .ok_or_else(|| "runtime helper __rt_str_pad_left not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_s));
  // target_size as i32
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalGet(ptr_p));
  ctx.emit(Instruction::Call(fn_idx));
  Ok(())
}

/// `&str:pad-right str target-size pattern` — pads str on the right.
pub(super) fn emit_str_pad_right(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 {
    return Err("&str:pad-right expects 3 args (str, size, pattern)".into());
  }
  let ptr_s = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_p = emit_ptr_to_i32(ctx, &args[2])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_pad_right")
    .ok_or_else(|| "runtime helper __rt_str_pad_right not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_s));
  // target_size as i32
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalGet(ptr_p));
  ctx.emit(Instruction::Call(fn_idx));
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

  let src_a = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_a));
  ctx.emit(Instruction::LocalGet(dst_c));
  ctx.emit(Instruction::LocalGet(src_a));
  ctx.emit(Instruction::LocalGet(len_a));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  let dst_b_off = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst_c));
  ctx.emit(Instruction::LocalGet(len_a));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(dst_b_off));

  let src_b = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_b));

  ctx.emit(Instruction::LocalGet(dst_b_off));
  ctx.emit(Instruction::LocalGet(src_b));
  ctx.emit(Instruction::LocalGet(len_b));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  ctx.emit(Instruction::LocalGet(ptr_c));
  ctx.emit(Instruction::F64ConvertI32U);
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
    Some(v) => { ctx.locals.insert(sym.as_ref().to_owned(), v); }
    None => { ctx.locals.remove(sym.as_ref()); }
  }
  // emit_turn_string leaves f64 on stack; convert to i32 local
  let str_i32 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(str_i32));
  str_i32
}

/// `str args...` — convert each arg to string and concatenate all.
/// Intercept for the variadic `str` call, bypassing the core library definition.
pub(super) fn emit_str_variadic(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    // Return empty string: allocate string with length 0
    let zero_local = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::LocalSet(zero_local));
    let (ptr, _) = emit_str_alloc(ctx, zero_local);
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(Instruction::F64ConvertI32U);
    return Ok(());
  }

  // Convert first arg to string, store as i32 ptr
  emit_turn_string(ctx, std::slice::from_ref(&args[0]))?;
  let acc_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(acc_ptr));

  for arg in &args[1..] {
    emit_turn_string(ctx, std::slice::from_ref(arg))?;
    let next_ptr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(next_ptr));

    concat_two_i32_ptrs(ctx, acc_ptr, next_ptr);
    // result f64 on stack → convert to i32 and update acc
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(acc_ptr));
  }

  ctx.emit(Instruction::LocalGet(acc_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `str-spaced args...` — convert each arg to string, join with space separators.
pub(super) fn emit_str_spaced(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    let zero_local = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::LocalSet(zero_local));
    let (ptr, _) = emit_str_alloc(ctx, zero_local);
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(Instruction::F64ConvertI32U);
    return Ok(());
  }

  // Allocate a 1-byte space string inline in WASM memory
  let one_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(one_local));
  let (space_ptr, space_content) = emit_str_alloc(ctx, one_local);
  // Write 0x20 (ASCII space) at content_base
  ctx.emit(Instruction::LocalGet(space_content));
  ctx.emit(Instruction::I32Const(0x20));
  ctx.emit(Instruction::I32Store8(super::mem_arg_byte(0)));

  // Convert first arg to string, store as i32 ptr
  emit_turn_string(ctx, std::slice::from_ref(&args[0]))?;
  let acc_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(acc_ptr));

  for arg in &args[1..] {
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

  ctx.emit(Instruction::LocalGet(acc_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
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
pub(super) fn build_str_new_fn(str_tag_id: i32) -> CompiledFn {
  // params: 0 = src_ptr (i32), 1 = byte_len (i32)
  // locals: 2 = padded (i32), 3 = payload (i32), 4 = ptr (i32)
  let instructions = vec![
    // padded = (byte_len + 7) & -8
    Instruction::LocalGet(1),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::I32Const(-8i32),
    Instruction::I32And,
    Instruction::LocalSet(2), // padded
    // payload = 8 + padded
    Instruction::I32Const(8),
    Instruction::LocalGet(2),
    Instruction::I32Add,
    Instruction::LocalSet(3), // payload
    // Write HEAP_MAGIC at heap_ptr (raw_base + 0)
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    // Write str_tag_id at heap_ptr + 4
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(str_tag_id),
    Instruction::I32Store(mem_arg_i32(4)),
    // ptr = heap_ptr + 8; advance heap_ptr += payload
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalTee(4), // ptr = logical_ptr
    Instruction::LocalGet(3), // payload
    Instruction::I32Add,
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    // Store byte_len as f64 at ptr+0
    Instruction::LocalGet(4),
    Instruction::LocalGet(1), // byte_len
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    // memory.copy(dst = ptr+8, src = src_ptr, n = byte_len)
    Instruction::LocalGet(4),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(0), // src_ptr
    Instruction::LocalGet(1), // byte_len
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    // return logical_ptr as f64
    Instruction::LocalGet(4),
    Instruction::F64ConvertI32U,
  ];

  CompiledFn {
    export_name: Some("__str_new".to_string()),
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![ValType::I32, ValType::I32, ValType::I32], // padded, payload, ptr
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
  let zero = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(zero));
  let (empty_ptr, _) = emit_str_alloc(ctx, zero);
  ctx.emit(Instruction::LocalGet(empty_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(result));

  let sep_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(sep_f64));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(sep_ptr));

  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

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
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
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
  if args.len() != 2 {
    return Err(format!("join-str expects 2 args, got {}", args.len()));
  }
  let xs = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(xs));
  let sep = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(sep));
  emit_join_str_from_locals(ctx, xs, sep)
}
