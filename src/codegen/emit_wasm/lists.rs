use super::*;

// ===========================================================================
// List operations — layout: [count:f64] [elem0:f64] [elem1:f64] ...
// ===========================================================================

/// `[] elem0 elem1 ...` — create a list with static arity.
pub(super) fn emit_list_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  let count = args.len();
  let total_bytes = ((1 + count) * 8) as i32;
  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_bytes, ptr, "list");

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(f64_const(count as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  for (i, arg) in args.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(ptr));
    emit_expr(ctx, arg)?;
    ctx.emit(Instruction::F64Store(mem_arg_f64(((1 + i) * 8) as u64)));
  }

  ctx.ptr_to_f64(ptr);
  Ok(())
}

/// `&list:nth list idx` — element at dynamic index.
pub(super) fn emit_list_nth(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&list:nth expects 2 args")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  // offset = (1 + idx) * 8
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  Ok(())
}

/// `&list:first list` — first element.
pub(super) fn emit_list_first(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&list:first expects 1 arg")?;
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  Ok(())
}

/// `&list:last list` — last element of a list.
pub(super) fn emit_list_last(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&list:last expects 1 arg")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  // last element is at src + 8 + (count-1)*8
  let last_idx = ctx.i32_offset(count, -1);
  // addr = src + 8 + last_idx * 8
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(last_idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  Ok(())
}

/// `&list:rest list` — new list without the first element.
pub(super) fn emit_list_rest(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&list:rest expects 1 arg")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let old_count = emit_load_count_i32(ctx, src);

  // Clamp at zero so `rest []` remains an empty list instead of underflowing.
  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::Select);
  ctx.emit(Instruction::LocalSet(new_count));

  // total_slots = 1 + new_count
  let dst = emit_alloc_list(ctx, new_count);

  // Copy elements: dst[8..] ← src[16..]
  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 16);
  emit_copy_f64_loop(ctx, dst_base, src_base, new_count);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `append list elem` — new list with element added at end.
pub(super) fn emit_list_append(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "append expects 2 args")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let old_count = emit_load_count_i32(ctx, src);
  // Evaluate element into a local BEFORE allocation
  let elem = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(elem));

  let new_count = ctx.i32_offset(old_count, 1);

  let dst = emit_alloc_list(ctx, new_count);

  // Copy old elements: dst[8..] ← src[8..]
  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, old_count);

  // Store new element at dst[8 + old_count * 8]
  emit_list_store_elem(ctx, dst, old_count, elem);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `prepend list elem` — new list with element at front.
pub(super) fn emit_list_prepend(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "prepend expects 2 args")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let old_count = emit_load_count_i32(ctx, src);
  let elem = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(elem));

  let new_count = ctx.i32_offset(old_count, 1);

  let dst = emit_alloc_list(ctx, new_count);

  // Store element at dst[8]
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  // Copy old elements: dst[16..] ← src[8..]
  let dst_base = emit_addr_offset(ctx, dst, 16);
  let src_base = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, old_count);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `butlast list` — new list without the last element.
pub(super) fn emit_list_butlast(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "butlast expects 1 arg")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let old_count = emit_load_count_i32(ctx, src);

  // Clamp at zero so `butlast []` remains an empty list instead of underflowing.
  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::Select);
  ctx.emit(Instruction::LocalSet(new_count));

  let dst = emit_alloc_list(ctx, new_count);

  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, new_count);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&list:slice list start` or `&list:slice list start end`.
pub(super) fn emit_list_slice(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() < 2 || args.len() > 3 {
    return Err("&list:slice expects 2-3 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);

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
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::LocalSet(end));
  }

  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(end));
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_count));

  let dst = emit_alloc_list(ctx, new_count);

  // src_base = src + 8 + start*8
  let src_base = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_base));

  let dst_base = emit_addr_offset(ctx, dst, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, new_count);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&list:reverse list` — new list in reverse order.
pub(super) fn emit_list_reverse(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&list:reverse expects 1 arg")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);

  let dst = emit_alloc_list(ctx, count);

  // Loop: dst[8 + i*8] = src[8 + (count-1-i)*8]
  let i = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();

  ctx.loop_exit_if_ge(i, count);

  // dst addr = dst + 8 + i*8
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);

  // src addr = src + 8 + (count-1-i)*8
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));

  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.i32_inc(i);
  ctx.emit(Instruction::Br(0));

  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&list:concat a b` — concatenate two lists.
/// Flatten one level starting from a pre-evaluated f64 list local.
/// Used by emit_mapcat and &list:flatten intercepts.
pub(super) fn emit_list_flatten_f64_local(ctx: &mut WasmGenCtx, outer_f64: u32) -> Result<(), String> {
  let outer_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(outer_f64));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(outer_ptr));
  let outer_count = emit_load_count_i32(ctx, outer_ptr);

  // === Pass 1: compute total element count ===
  let total = ctx.alloc_i32(0);
  let i1 = ctx.alloc_i32(0);
  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(i1, outer_count);
  let inner_ptr1 = emit_list_load_ptr(ctx, outer_ptr, i1);
  let inner_count1 = emit_load_count_i32(ctx, inner_ptr1);
  ctx.emit(Instruction::LocalGet(total));
  ctx.emit(Instruction::LocalGet(inner_count1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total));
  ctx.i32_inc(i1);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // === Allocate result of size `total` ===
  let dst = emit_alloc_list(ctx, total);

  // === Pass 2: copy elements into result ===
  let write_idx = ctx.alloc_i32(0);
  let i2 = ctx.alloc_i32(0);
  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(i2, outer_count);
  let inner_ptr2 = emit_list_load_ptr(ctx, outer_ptr, i2);
  let inner_count2 = emit_load_count_i32(ctx, inner_ptr2);
  let dst_base = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(dst_base));
  let src_base2 = emit_addr_offset(ctx, inner_ptr2, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base2, inner_count2);
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::LocalGet(inner_count2));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(write_idx));
  ctx.i32_inc(i2);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// Flatten one level: given a list-of-lists (as Calcit expr), emit concat of all inner lists.
fn emit_list_flatten_one_level(ctx: &mut WasmGenCtx, xs_arg: &Calcit) -> Result<(), String> {
  let outer_f64 = ctx.alloc_local();
  emit_expr(ctx, xs_arg)?;
  ctx.emit(Instruction::LocalSet(outer_f64));
  emit_list_flatten_f64_local(ctx, outer_f64)
}

pub(super) fn emit_list_concat(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    // 0 args → empty list (for spread of empty list: &list:concat & [])
    return emit_list_new(ctx, &[]);
  }
  if args.len() == 1 {
    // 1 arg = list-of-lists to flatten one level (for spread: &list:concat & xs)
    return emit_list_flatten_one_level(ctx, &args[0]);
  }
  if args.len() == 2 {
    // Direct 2-arg fast path
    return emit_list_concat_two(ctx, &args[0], &args[1]);
  }
  // Variadic: fold pairs left
  emit_list_concat_two(ctx, &args[0], &args[1])?;
  for extra in &args[2..] {
    // Result of previous concat is on stack as f64; convert to i32 ptr, concat with extra
    let prev_f64 = ctx.alloc_local();
    ctx.emit(Instruction::LocalSet(prev_f64));
    let prev = Calcit::Number(0.0); // dummy — we'll push the f64 local directly
    let _ = prev; // unused
    // Push prev f64 local as first arg, emit extra as second
    let src_a_local = prev_f64; // f64 local holding ptr as f64
    let src_a = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(src_a_local));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(src_a));
    let count_a = emit_load_count_i32(ctx, src_a);
    let src_b = emit_ptr_to_i32(ctx, extra)?;
    let count_b = emit_load_count_i32(ctx, src_b);
    let new_count = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(count_a));
    ctx.emit(Instruction::LocalGet(count_b));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(new_count));
    let dst = emit_alloc_list(ctx, new_count);
    let dst_base_a = emit_addr_offset(ctx, dst, 8);
    let src_base_a = emit_addr_offset(ctx, src_a, 8);
    emit_copy_f64_loop(ctx, dst_base_a, src_base_a, count_a);
    let dst_base_b = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(count_a));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(dst_base_b));
    let src_base_b = emit_addr_offset(ctx, src_b, 8);
    emit_copy_f64_loop(ctx, dst_base_b, src_base_b, count_b);
    ctx.ptr_to_f64(dst);
  }
  Ok(())
}

fn emit_list_concat_two(ctx: &mut WasmGenCtx, a: &Calcit, b: &Calcit) -> Result<(), String> {
  let src_a = emit_ptr_to_i32(ctx, a)?;
  let count_a = emit_load_count_i32(ctx, src_a);
  let src_b = emit_ptr_to_i32(ctx, b)?;
  let count_b = emit_load_count_i32(ctx, src_b);

  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count_a));
  ctx.emit(Instruction::LocalGet(count_b));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(new_count));

  let dst = emit_alloc_list(ctx, new_count);

  // Copy a: dst[8..] ← src_a[8..]
  let dst_base_a = emit_addr_offset(ctx, dst, 8);
  let src_base_a = emit_addr_offset(ctx, src_a, 8);
  emit_copy_f64_loop(ctx, dst_base_a, src_base_a, count_a);

  // Copy b: dst[8 + count_a*8 ..] ← src_b[8..]
  let dst_base_b = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(count_a));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(dst_base_b));
  let src_base_b = emit_addr_offset(ctx, src_b, 8);
  emit_copy_f64_loop(ctx, dst_base_b, src_base_b, count_b);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&list:assoc list idx value` — new list with element replaced at index.
pub(super) fn emit_list_assoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&list:assoc expects 3 args")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let idx = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx));
  let val = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(val));

  let dst = emit_alloc_list(ctx, count);

  // Copy all elements
  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, count);

  // Overwrite at idx: dst[8 + idx*8]
  emit_list_store_elem(ctx, dst, idx, val);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&list:assoc-before list idx val` — new list with `val` inserted before position `idx`.
pub(super) fn emit_list_assoc_before(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&list:assoc-before expects 3 args")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let idx = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx));
  let val = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(val));

  // new_count = count + 1
  let new_count = ctx.i32_offset(count, 1);
  let dst = emit_alloc_list(ctx, new_count);

  // Copy [0..idx): dst_base = dst+8, src_base = src+8, n = idx
  let before_n = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::LocalSet(before_n));
  let dst_b1 = emit_addr_offset(ctx, dst, 8);
  let src_b1 = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_b1, src_b1, before_n);

  // Store val at dst[8 + idx*8]
  emit_list_store_elem(ctx, dst, idx, val);

  // Copy [idx..count): dst offset = idx+1, src offset = idx, n = count - idx
  let after_n = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(after_n));

  let dst_b2 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(dst_b2));

  let src_b2 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_b2));

  emit_copy_f64_loop(ctx, dst_b2, src_b2, after_n);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&list:assoc-after list idx val` — new list with `val` inserted after position `idx`.
pub(super) fn emit_list_assoc_after(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&list:assoc-after expects 3 args")?;
  // assoc-after(xs, idx, val) = assoc-before(xs, idx+1, val)
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let idx = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add); // insert_at = idx + 1
  ctx.emit(Instruction::LocalSet(idx));
  let val = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(val));

  let new_count = ctx.i32_offset(count, 1);
  let dst = emit_alloc_list(ctx, new_count);

  // Copy [0..idx)
  let before_n = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::LocalSet(before_n));
  let dst_b1 = emit_addr_offset(ctx, dst, 8);
  let src_b1 = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_b1, src_b1, before_n);

  // Store val at dst[8 + idx*8]
  emit_list_store_elem(ctx, dst, idx, val);

  // Copy [idx..count)
  let after_n = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(after_n));

  let dst_b2 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(dst_b2));

  let src_b2 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_b2));

  emit_copy_f64_loop(ctx, dst_b2, src_b2, after_n);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&list:to-set list` — convert list to set, deduplicating elements.
/// Uses same O(n²) dedup as emit_list_distinct but allocates a "set"-tagged block.
pub(super) fn emit_list_to_set(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&list:to-set expects 1 arg")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let n = emit_load_count_i32(ctx, src);

  let total_slots = ctx.i32_offset(n, 1);
  let dst = emit_alloc_with_count(ctx, n, total_slots, "set");

  let write_idx = ctx.alloc_i32(0);

  let i = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(i, n);

  let elem = ctx.alloc_local();
  emit_list_load_elem(ctx, src, i);
  ctx.emit(Instruction::LocalSet(elem));

  let j = ctx.alloc_i32(0);
  let found = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(j, write_idx);
  emit_list_load_elem(ctx, dst, j);
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::F64Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.i32_inc(j);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  {
    let write_addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst));
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_addr));
    ctx.emit(Instruction::LocalGet(write_addr));
    ctx.emit(Instruction::LocalGet(elem));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.i32_inc(write_idx);
  }
  ctx.emit(Instruction::End);

  ctx.i32_inc(i);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Update count to actual write_idx
  ctx.emit(Instruction::LocalGet(dst));
  ctx.ptr_to_f64(write_idx);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&list:dissoc list idx` — new list without element at index.
pub(super) fn emit_list_dissoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  use crate::calcit::CalcitSyntax;
  // Handle spread form: (&list:dissoc x & rest_list) — rest_list[0] is the index
  let (list_arg, idx_arg): (&Calcit, Option<&Calcit>) =
    if args.len() == 3 && matches!(args[1], Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
      (&args[0], None) // spread form: extract idx from rest_list at runtime
    } else if args.len() == 2 {
      (&args[0], Some(&args[1]))
    } else {
      return Err("&list:dissoc expects 2 args".into());
    };

  let src = emit_ptr_to_i32(ctx, list_arg)?;
  let count = emit_load_count_i32(ctx, src);
  let idx = ctx.alloc_local_typed(ValType::I32);
  if let Some(idx_expr) = idx_arg {
    emit_expr(ctx, idx_expr)?;
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(idx));
  } else {
    // Spread form: rest_list[0] is the index (load first element of the rest list)
    let rest_list_f64 = ctx.alloc_local();
    emit_expr(ctx, &args[2])?;
    ctx.emit(Instruction::LocalSet(rest_list_f64));
    let rest_list_i32 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(rest_list_f64));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(rest_list_i32));
    // Load element 0: offset 8 from list start
    ctx.emit(Instruction::LocalGet(rest_list_i32));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(idx));
  }

  let new_count = ctx.i32_offset(count, -1);

  let dst = emit_alloc_list(ctx, new_count);

  // Copy [0..idx): src_base = src+8, dst_base = dst+8, n = idx
  let before_n = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::LocalSet(before_n));
  let dst_b1 = emit_addr_offset(ctx, dst, 8);
  let src_b1 = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_b1, src_b1, before_n);

  // Copy [idx+1..count): n = count - idx - 1
  let after_n = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(after_n));

  // dst_base2 = dst + 8 + idx*8
  let dst_b2 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(dst_b2));

  // src_base2 = src + 8 + (idx+1)*8
  let src_b2 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_b2));

  emit_copy_f64_loop(ctx, dst_b2, src_b2, after_n);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `list? x` — true (1.0) when x is a list value.
/// Implemented as: (type-of x) == list-tag
pub(super) fn emit_list_q(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "list?")?;
  let list_tag = get_type_tag(ctx, "list");
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  emit_type_of(ctx, args)?;
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::Select);
  Ok(())
}

/// `&list:contains? list idx` — true if 0 ≤ idx < count.
pub(super) fn emit_list_contains(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&list:contains? expects 2 args")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, ptr);
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32LtU);
  ctx.emit(Instruction::Select);
  Ok(())
}

/// `&list:includes? list value` — linear scan for matching f64 value.
pub(super) fn emit_list_includes(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&list:includes? expects 2 args")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, ptr);
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));

  let result = ctx.alloc_local();
  ctx.emit(f64_const(0.0)); // default: false
  ctx.emit(Instruction::LocalSet(result));

  let i = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();

  ctx.loop_exit_if_ge(i, count);

  // Load elem at ptr + 8 + i*8
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(target));
  ctx.emit(Instruction::F64Eq);

  ctx.begin_block_if();
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // break outer block
  ctx.emit(Instruction::End); // end if

  ctx.i32_inc(i);
  ctx.br_loop();

  ctx.end_block_loop();

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

// ===========================================================================
// BufList operations — layout: [capacity:f64] [count:f64] [elem0:f64] ...
// Mutable append-only list. `push` and `concat` mutate in-place.
// ===========================================================================

pub(super) const BUF_LIST_INITIAL_CAPACITY: i32 = 8;

/// `(&buf-list:new)` — create empty BufList with initial capacity
pub(super) fn emit_buf_list_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if !args.is_empty() {
    return Err("&buf-list:new expects 0 args".into());
  }
  // Allocate: 2 header slots (capacity, count) + initial_capacity data slots
  let total_slots = 2 + BUF_LIST_INITIAL_CAPACITY;
  let byte_size = total_slots * 8;
  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, byte_size, ptr, "buf-list");
  // Store capacity
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(f64_const(BUF_LIST_INITIAL_CAPACITY as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  // Store count = 0
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
  // Return as f64
  ctx.ptr_to_f64(ptr);
  Ok(())
}

/// `(&buf-list:push buf item)` — mutates buf, returns buf.
/// If count == capacity, grow to 2x capacity.
pub(super) fn emit_buf_list_push(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&buf-list:push expects 2 args")?;
  let buf_ptr = emit_ptr_to_i32(ctx, &args[0])?;

  // Load count
  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(count));

  // Load capacity
  let capacity = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(capacity));

  // if count >= capacity, grow
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::LocalGet(capacity));
  ctx.emit(Instruction::I32GeU);
  ctx.begin_block_if();
  {
    // New capacity = old * 2
    let new_cap = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(capacity));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::LocalSet(new_cap));

    // Allocate new buffer: (2 + new_cap) * 8
    let new_size = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(new_cap));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::LocalSet(new_size));

    let new_ptr = ctx.alloc_local_typed(ValType::I32);
    emit_bump_alloc_dynamic(ctx, new_size, new_ptr, "buf-list");

    // Store new capacity
    ctx.emit(Instruction::LocalGet(new_ptr));
    ctx.ptr_to_f64(new_cap);
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

    // Store count (unchanged)
    ctx.emit(Instruction::LocalGet(new_ptr));
    ctx.ptr_to_f64(count);
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

    // Copy old data: new_ptr+16 ← buf_ptr+16, count elements
    let dst_base = emit_addr_offset(ctx, new_ptr, 16);
    let src_base = emit_addr_offset(ctx, buf_ptr, 16);
    emit_copy_f64_loop(ctx, dst_base, src_base, count);

    // Update buf_ptr to new_ptr
    ctx.emit(Instruction::LocalGet(new_ptr));
    ctx.emit(Instruction::LocalSet(buf_ptr));
  }
  ctx.emit(Instruction::End); // end if

  // Store the new element at buf_ptr + 16 + count * 8
  let elem_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(elem_addr));

  ctx.emit(Instruction::LocalGet(elem_addr));
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // Increment count
  let new_count = ctx.i32_offset(count, 1);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.ptr_to_f64(new_count);
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  // Return buf_ptr as f64
  ctx.ptr_to_f64(buf_ptr);
  Ok(())
}

/// `(&buf-list:concat buf list)` — append all list elements to buf
pub(super) fn emit_buf_list_concat(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&buf-list:concat expects 2 args")?;
  let buf_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let list_ptr = emit_ptr_to_i32(ctx, &args[1])?;

  let list_count = emit_load_count_i32(ctx, list_ptr);

  // Loop: for i in 0..list_count, push list[i] to buf
  let i = ctx.alloc_i32(0);

  ctx.begin_block(); // break target
  ctx.begin_loop(); // continue target

  // if i >= list_count, break
  ctx.loop_exit_if_ge(i, list_count); // break

  // Load buf's count and capacity
  let b_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(b_count));

  let b_cap = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(b_cap));

  // if count >= capacity, grow
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::LocalGet(b_cap));
  ctx.emit(Instruction::I32GeU);
  ctx.begin_block_if();
  {
    let new_cap = ctx.alloc_local_typed(ValType::I32);
    // new_cap = max(cap * 2, count + list_count)
    ctx.emit(Instruction::LocalGet(b_cap));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::LocalSet(new_cap));

    // Ensure new_cap >= b_count + list_count
    let needed = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(b_count));
    ctx.emit(Instruction::LocalGet(list_count));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(needed));

    ctx.emit(Instruction::LocalGet(new_cap));
    ctx.emit(Instruction::LocalGet(needed));
    ctx.emit(Instruction::I32LtU);
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(needed));
    ctx.emit(Instruction::LocalSet(new_cap));
    ctx.emit(Instruction::End);

    let new_size = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(new_cap));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::LocalSet(new_size));

    let new_ptr = ctx.alloc_local_typed(ValType::I32);
    emit_bump_alloc_dynamic(ctx, new_size, new_ptr, "buf-list");

    ctx.emit(Instruction::LocalGet(new_ptr));
    ctx.ptr_to_f64(new_cap);
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

    ctx.emit(Instruction::LocalGet(new_ptr));
    ctx.ptr_to_f64(b_count);
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

    let dst_base = emit_addr_offset(ctx, new_ptr, 16);
    let src_base = emit_addr_offset(ctx, buf_ptr, 16);
    emit_copy_f64_loop(ctx, dst_base, src_base, b_count);

    ctx.emit(Instruction::LocalGet(new_ptr));
    ctx.emit(Instruction::LocalSet(buf_ptr));
  }
  ctx.emit(Instruction::End); // end if (grow)

  // Reload count after possible grow
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(b_count));

  // Store list[i] at buf_ptr + 16 + b_count * 8
  let elem_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(elem_addr));

  // Load list[i]: list_ptr + 8 + i * 8
  let list_elem_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(list_ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(list_elem_addr));

  ctx.emit(Instruction::LocalGet(elem_addr));
  ctx.emit(Instruction::LocalGet(list_elem_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // Increment buf count
  let new_b_count = ctx.i32_offset(b_count, 1);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.ptr_to_f64(new_b_count);
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  // i++
  ctx.i32_inc(i);
  ctx.br_loop();

  ctx.end_block_loop();

  // Return buf_ptr
  ctx.ptr_to_f64(buf_ptr);
  Ok(())
}

/// `(&buf-list:to-list buf)` — freeze buf into an immutable list
pub(super) fn emit_buf_list_to_list(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&buf-list:to-list expects 1 arg")?;
  let buf_ptr = emit_ptr_to_i32(ctx, &args[0])?;

  // Load count
  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(count));

  // Allocate a new list: (1 + count) slots
  let dst = emit_alloc_list(ctx, count);

  // Copy data: dst+8 ← buf_ptr+16, count elements
  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, buf_ptr, 16);
  emit_copy_f64_loop(ctx, dst_base, src_base, count);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `(&buf-list:count buf)` — return count as f64
pub(super) fn emit_buf_list_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&buf-list:count expects 1 arg")?;
  let buf_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8))); // count is at offset 8
  Ok(())
}

/// `range n` or `range a b` — create a list of numbers [0..n) or [a..b).
pub(super) fn emit_range(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() || args.len() > 3 {
    return Err("range expects 1, 2, or 3 args".into());
  }

  // 3-arg form: range start end step
  if args.len() == 3 {
    let start = ctx.alloc_local();
    let end = ctx.alloc_local();
    let step = ctx.alloc_local();
    emit_expr(ctx, &args[0])?;
    ctx.emit(Instruction::LocalSet(start));
    emit_expr(ctx, &args[1])?;
    ctx.emit(Instruction::LocalSet(end));
    emit_expr(ctx, &args[2])?;
    ctx.emit(Instruction::LocalSet(step));

    // count = max(0, ceil((end - start) / step))
    let raw_count_f = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(end));
    ctx.emit(Instruction::LocalGet(start));
    ctx.emit(Instruction::F64Sub);
    ctx.emit(Instruction::LocalGet(step));
    ctx.emit(Instruction::F64Div);
    ctx.emit(Instruction::F64Ceil);
    ctx.emit(Instruction::LocalSet(raw_count_f));

    // clamp: if raw_count_f <= 0, count = 0
    let count = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(raw_count_f));
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Gt);
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(raw_count_f));
    ctx.emit(Instruction::I32TruncF64S);
    ctx.emit(Instruction::LocalSet(count));
    ctx.emit(Instruction::End);

    let dst = emit_alloc_list(ctx, count);

    let i = ctx.alloc_i32(0);

    ctx.begin_block();
    ctx.begin_loop();
    ctx.loop_exit_if_ge(i, count);
    // elem = start + i * step
    ctx.emit(Instruction::LocalGet(dst));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(i));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    // value = start + i * step
    ctx.emit(Instruction::LocalGet(start));
    ctx.emit(Instruction::LocalGet(i));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::LocalGet(step));
    ctx.emit(Instruction::F64Mul);
    ctx.emit(Instruction::F64Add);
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.i32_inc(i);
    ctx.emit(Instruction::Br(0));
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::End);

    ctx.ptr_to_f64(dst);
    return Ok(());
  }

  let start = ctx.alloc_local();
  let end = ctx.alloc_local();

  if args.len() == 1 {
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::LocalSet(start));
    emit_expr(ctx, &args[0])?;
    ctx.emit(Instruction::LocalSet(end));
  } else {
    emit_expr(ctx, &args[0])?;
    ctx.emit(Instruction::LocalSet(start));
    emit_expr(ctx, &args[1])?;
    ctx.emit(Instruction::LocalSet(end));
  }

  // count = max(0, trunc(end) - trunc(start))
  let count = ctx.alloc_local_typed(ValType::I32);
  let raw_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(end));
  ctx.emit(Instruction::I32TruncF64S);
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32TruncF64S);
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(raw_count));
  // clamp to 0 if negative: select(val_true, val_false, cond)
  ctx.emit(Instruction::LocalGet(raw_count)); // val if true (raw > 0)
  ctx.emit(Instruction::I32Const(0)); // val if false
  ctx.emit(Instruction::LocalGet(raw_count));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32GtS); // cond: raw_count > 0
  ctx.emit(Instruction::Select);
  ctx.emit(Instruction::LocalSet(count));

  let dst = emit_alloc_list(ctx, count);

  // Fill: dst[8 + i*8] = start + i
  let i = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(i, count);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(start));
  ctx.ptr_to_f64(i);
  ctx.emit(Instruction::F64Add);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.i32_inc(i);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// Two elements are considered equal when their f64 bit patterns are identical.
pub(super) fn emit_list_distinct(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&list:distinct expects 1 arg")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let n = emit_load_count_i32(ctx, src);

  // Allocate output with same max capacity as input (over-alloc; count updated at end)
  let dst = emit_alloc_list(ctx, n);

  let write_idx = ctx.alloc_i32(0);

  let i = ctx.alloc_i32(0);

  // Outer loop: iterate src elements
  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(i, n);

  // elem = src[(1 + i) * 8]
  let elem = ctx.alloc_local();
  emit_list_load_elem(ctx, src, i);
  ctx.emit(Instruction::LocalSet(elem));

  // Inner loop: scan dst[0..write_idx) for elem
  let j = ctx.alloc_i32(0);
  let found = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(j, write_idx);

  // existing = dst[(1 + j) * 8]
  emit_list_load_elem(ctx, dst, j);
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::F64Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2)); // exit inner block (ends inner loop)
  ctx.emit(Instruction::End);

  ctx.i32_inc(j);
  ctx.br_loop();
  ctx.end_block_loop();

  // If not found: dst[1 + write_idx] = elem; write_idx++
  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  {
    let write_addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst));
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_addr));
    ctx.emit(Instruction::LocalGet(write_addr));
    ctx.emit(Instruction::LocalGet(elem));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.i32_inc(write_idx);
  }
  ctx.emit(Instruction::End);

  ctx.i32_inc(i);
  ctx.br_loop();
  ctx.end_block_loop();

  // Update count field to actual write_idx
  ctx.emit(Instruction::LocalGet(dst));
  ctx.ptr_to_f64(write_idx);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// Helper: append one element to a list given a pre-evaluated list f64 local and element f64 local.
/// Returns a new f64 local containing the new list pointer.
fn emit_append_from_local(ctx: &mut WasmGenCtx, list_local: u32, elem_local: u32) -> u32 {
  let src = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(list_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(src));

  let old_count = emit_load_count_i32(ctx, src);

  let new_count = ctx.i32_offset(old_count, 1);

  let dst = emit_alloc_list(ctx, new_count);

  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, old_count);

  // Store new element at dst[8 + old_count * 8]
  emit_list_store_elem(ctx, dst, old_count, elem_local);

  let result = ctx.alloc_local();
  ctx.ptr_to_f64(dst);
  ctx.emit(Instruction::LocalSet(result));
  result
}

/// `conj xs y0 y1 ...` — append one or more elements to a list.
pub(super) fn emit_conj(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() < 2 {
    return Err("conj expects at least 2 args".into());
  }
  let acc = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(acc));

  for arg in &args[1..] {
    let elem = ctx.alloc_local();
    emit_expr(ctx, arg)?;
    ctx.emit(Instruction::LocalSet(elem));
    let new_acc = emit_append_from_local(ctx, acc, elem);
    ctx.emit(Instruction::LocalGet(new_acc));
    ctx.emit(Instruction::LocalSet(acc));
  }

  ctx.emit(Instruction::LocalGet(acc));
  Ok(())
}

/// Core loop body for `repeat x n` — local 0 = x (f64), local 1 = n (f64).
pub(super) fn emit_repeat_from_locals(ctx: &mut WasmGenCtx, x_local: u32, n_local: u32) -> Result<(), String> {
  let n_i32 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(n_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(n_i32));

  let dst = emit_alloc_list(ctx, n_i32);

  let i = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(i, n_i32);

  let addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(addr));
  ctx.emit(Instruction::LocalGet(addr));
  ctx.emit(Instruction::LocalGet(x_local));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.i32_inc(i);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `repeat x n` — call-site intercept: evaluate args and call body emitter.
pub(super) fn emit_repeat(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "repeat")?;
  let x = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(x));
  let n = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(n));
  emit_repeat_from_locals(ctx, x, n)
}

/// Core loop body for `interleave xs ys` — local 0 = xs (f64), local 1 = ys (f64).
pub(super) fn emit_interleave_from_locals(ctx: &mut WasmGenCtx, xs_f64: u32, ys_f64: u32) -> Result<(), String> {
  let xs_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(xs_f64));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(xs_ptr));

  let ys_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ys_f64));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ys_ptr));

  let xs_count = emit_load_count_i32(ctx, xs_ptr);
  let ys_count = emit_load_count_i32(ctx, ys_ptr);

  // min_count = min(xs_count, ys_count)
  let min_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(xs_count));
  ctx.emit(Instruction::LocalGet(ys_count));
  ctx.emit(Instruction::LocalGet(xs_count));
  ctx.emit(Instruction::LocalGet(ys_count));
  ctx.emit(Instruction::I32LtU);
  ctx.emit(Instruction::Select);
  ctx.emit(Instruction::LocalSet(min_count));

  // result_count = 2 * min_count
  let result_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(min_count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalSet(result_count));

  let dst = emit_alloc_list(ctx, result_count);

  let i = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(i, min_count);

  // elem_xs = xs[(i+1)*8]
  let elem_xs = ctx.alloc_local();
  emit_list_load_elem(ctx, xs_ptr, i);
  ctx.emit(Instruction::LocalSet(elem_xs));

  // elem_ys = ys[(i+1)*8]
  let elem_ys = ctx.alloc_local();
  emit_list_load_elem(ctx, ys_ptr, i);
  ctx.emit(Instruction::LocalSet(elem_ys));

  // result[2*i] = elem_xs
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(elem_xs));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // result[2*i+1] = elem_ys
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(elem_ys));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.i32_inc(i);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `interleave xs ys` — call-site intercept.
pub(super) fn emit_interleave(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "interleave")?;
  let xs = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(xs));
  let ys = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(ys));
  emit_interleave_from_locals(ctx, xs, ys)
}

/// Core body for `join xs sep` (list join) — local 0 = xs (f64), local 1 = sep (f64).
pub(super) fn emit_join_from_locals(ctx: &mut WasmGenCtx, xs_f64: u32, sep_f64: u32) -> Result<(), String> {
  let xs_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(xs_f64));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(xs_ptr));

  let count = emit_load_count_i32(ctx, xs_ptr);

  // result_count = max(0, 2*count - 1) = if count == 0 then 0 else 2*count-1
  let result_count = ctx.alloc_local_typed(ValType::I32);
  // result_count = count == 0 ? 0 : 2*count - 1
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(result_count));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(result_count));
  ctx.emit(Instruction::End);

  let dst = emit_alloc_list(ctx, result_count);

  let i = ctx.alloc_i32(0);
  let write_idx = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(i, count);

  // if i > 0: write sep at write_idx, write_idx++
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  let sep_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(sep_addr));
  ctx.emit(Instruction::LocalGet(sep_addr));
  ctx.emit(Instruction::LocalGet(sep_f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.i32_inc(write_idx);
  ctx.emit(Instruction::End);

  // write xs[i] at write_idx, write_idx++
  let elem = ctx.alloc_local();
  emit_list_load_elem(ctx, xs_ptr, i);
  ctx.emit(Instruction::LocalSet(elem));
  let elem_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(elem_addr));
  ctx.emit(Instruction::LocalGet(elem_addr));
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.i32_inc(write_idx);

  ctx.i32_inc(i);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `join xs sep` — call-site intercept (list join).
pub(super) fn emit_join(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "join")?;
  let xs = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(xs));
  let sep = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(sep));
  emit_join_from_locals(ctx, xs, sep)
}
