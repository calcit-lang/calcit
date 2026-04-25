use super::*;

// ===========================================================================
// Map operations — outer layout: [count:f64] [root_ptr:f64]
// Root layout: [count:f64] [k0:f64] [v0:f64] [k1:f64] [v1:f64] ...
// ===========================================================================

/// `&{} key0 val0 key1 val1 ...` — create a map (key-value pairs).
pub(super) fn emit_map_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() % 2 != 0 {
    return Err("&{} expects even number of args (key-value pairs)".into());
  }
  let count = args.len() / 2;
  let count_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(count as i32));
  ctx.emit(Instruction::LocalSet(count_local));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count_local));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let root = emit_alloc_with_count(ctx, count_local, total_slots, "map");

  for i in 0..count {
    // key at offset 8 + i*16
    ctx.emit(Instruction::LocalGet(root));
    emit_expr(ctx, &args[i * 2])?;
    ctx.emit(Instruction::F64Store(mem_arg_f64((8 + i * 16) as u64)));
    // value at offset 16 + i*16
    ctx.emit(Instruction::LocalGet(root));
    emit_expr(ctx, &args[i * 2 + 1])?;
    ctx.emit(Instruction::F64Store(mem_arg_f64((16 + i * 16) as u64)));
  }

  ctx.emit(Instruction::LocalGet(root));
  ctx.call_rt("__rt_map_root_from_flat");
  let hashed_root = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(hashed_root));
  let ptr = emit_alloc_map_with_root(ctx, count_local, hashed_root);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:get map key` — linear scan for key; returns value or nil.
pub(super) fn emit_map_get_op(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&map:get")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(target));
  ctx.call_rt("__rt_map_get_value");
  Ok(())
}

/// `&map:contains? map key` — scan for key, return bool.
pub(super) fn emit_map_contains(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&map:contains?")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));
  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_map_contains_key", ptr, target);
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::Select);
  Ok(())
}

/// `&map:includes? map value` — scan values for match.
pub(super) fn emit_map_includes(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&map:includes?")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));
  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_map_contains_value", ptr, target);
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::Select);
  Ok(())
}

/// `&map:assoc map key value` — new map with key-value added or updated.
pub(super) fn emit_map_assoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&map:assoc")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let key = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(key));
  let val = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(val));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalGet(key));
  ctx.emit(Instruction::LocalGet(val));
  ctx.call_rt("__rt_map_assoc");
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:dissoc map key` — new map without key (or same if key absent).
pub(super) fn emit_map_dissoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  use crate::calcit::CalcitSyntax;
  // Handle spread form: (&map:dissoc x & rest_list) — rest_list[0] is the key
  let (map_arg, key_arg_opt) = if args.len() == 3 && matches!(args[1], Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
    (&args[0], None)
  } else if args.len() == 2 {
    (&args[0], Some(&args[1]))
  } else {
    return Err("&map:dissoc expects 2 args".into());
  };

  let src = emit_ptr_to_i32(ctx, map_arg)?;
  let target = ctx.alloc_local();
  if let Some(key_expr) = key_arg_opt {
    emit_expr(ctx, key_expr)?;
    ctx.emit(Instruction::LocalSet(target));
  } else {
    // Spread form: load key from rest_list[0]
    let rest_list_f64 = ctx.alloc_local();
    emit_expr(ctx, &args[2])?;
    ctx.emit(Instruction::LocalSet(rest_list_f64));
    let rest_list_i32 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(rest_list_f64));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(rest_list_i32));
    ctx.emit(Instruction::LocalGet(rest_list_i32));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalSet(target));
  }
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalGet(target));
  ctx.call_rt("__rt_map_dissoc");
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `to-pairs map` — convert map to list of 2-element lists.
pub(super) fn emit_map_to_pairs(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "to-pairs")?;
  emit_map_to_pair_list(ctx, args, "set")
}

/// `&map:to-list map` — convert map to list of `[key, value]` pairs.
pub(super) fn emit_map_to_list(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&map:to-list")?;
  emit_map_to_pair_list(ctx, args, "list")
}

/// `&map:keys m` — list of all keys in the map (unordered).
pub(super) fn emit_map_keys(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&map:keys")?;
  let map_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, map_ptr);
  let flat_ptr = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", map_ptr);

  let ts = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ts));
  let dst = emit_alloc_with_count(ctx, count, ts, "list");

  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  // dst[8 + i*8] = flat_ptr[8 + i*16]  (keys)
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:vals m` — list of all values in the map (unordered).
pub(super) fn emit_map_vals(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&map:vals expects 1 arg".into());
  }
  let map_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, map_ptr);
  let flat_ptr = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", map_ptr);

  let ts = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ts));
  let dst = emit_alloc_with_count(ctx, count, ts, "list");

  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  // dst[8 + i*8] = flat_ptr[16 + i*16]  (values are at key+8)
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Shared implementation: convert a map to a list/set of `[key, value]` pairs.
pub(super) fn emit_map_to_pair_list(ctx: &mut WasmGenCtx, args: &[Calcit], outer_tag: &str) -> Result<(), String> {
  let map_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, map_ptr);
  let flat_ptr = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", map_ptr);

  // Outer list: [count, pair_ptr_0, pair_ptr_1, ...]
  let outer_ts = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(outer_ts));
  let outer = emit_alloc_with_count(ctx, count, outer_ts, outer_tag);

  // Loop: for each entry, create a 2-element list [key, value]
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // Allocate pair: [2, key, value] = 3 f64 slots = 24 bytes
  let pair = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, 24, pair, "list");
  ctx.emit(Instruction::LocalGet(pair));
  ctx.emit(f64_const(2.0));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // Load key from map: map_ptr + 8 + i*16
  ctx.emit(Instruction::LocalGet(pair));
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  // Load value from map: map_ptr + 16 + i*16
  ctx.emit(Instruction::LocalGet(pair));
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::F64Store(mem_arg_f64(16)));

  // Store pair ptr in outer list: outer[8 + i*8]
  ctx.emit(Instruction::LocalGet(outer));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(pair));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));

  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(outer));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&merge a b` — map merge where keys from `b` override keys from `a`.
pub(super) fn emit_map_merge(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&merge expects 2 args".into());
  }
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let a_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let b_count = emit_load_count_i32(ctx, b);
  let b_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", b);

  // Over-allocate: at most a_count + b_count entries.
  let max_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(max_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(max_count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst_root = emit_alloc_with_count(ctx, max_count, total_slots, "map");

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(write_idx));

  // Copy all entries from a into dst flat buffer.
  let ai = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ai));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  let ak = ctx.alloc_local();
  let av = ctx.alloc_local();
  let a_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(a_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(ak));
  ctx.emit(Instruction::LocalGet(a_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(av));

  let out_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst_root));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(out_addr));
  ctx.emit(Instruction::LocalGet(out_addr));
  ctx.emit(Instruction::LocalGet(ak));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(out_addr));
  ctx.emit(Instruction::LocalGet(av));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(write_idx));

  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Merge entries from b: override existing key, otherwise append.
  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  let bk = ctx.alloc_local();
  let bv = ctx.alloc_local();
  let b_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(b_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(b_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(bk));
  ctx.emit(Instruction::LocalGet(b_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(bv));

  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));
  let di = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(di));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  let d_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst_root));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(d_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(bk));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // Override value for existing key.
  ctx.emit(Instruction::LocalGet(d_addr));
  ctx.emit(Instruction::LocalGet(bv));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(di));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // If key not found, append pair.
  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    let out_addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst_root));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(out_addr));
    ctx.emit(Instruction::LocalGet(out_addr));
    ctx.emit(Instruction::LocalGet(bk));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(out_addr));
    ctx.emit(Instruction::LocalGet(bv));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_idx));
  }
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count and convert flat map back to runtime map.
  ctx.emit(Instruction::LocalGet(dst_root));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  let dst = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_from_flat", dst_root);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:diff-new a b` — map of entries in `a` whose keys are NOT in `b`.
/// Matches interpreter semantics: starts from `a` (args[0]) and removes keys present in `b` (args[1]).
pub(super) fn emit_map_diff_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:diff-new expects 2 args".into());
  }
  // `&map:diff-new a b` returns entries in `a` (args[0]) whose keys are not in `b` (args[1]).
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let a_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let b_count = emit_load_count_i32(ctx, b);
  let b_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", b);

  // Over-allocate: max is a_count entries (we iterate over a)
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst_root = emit_alloc_with_count(ctx, a_count, total_slots, "map");

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(write_idx));

  // Outer loop: iterate over a
  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // Load a[bi] key and val
  let bk = ctx.alloc_local();
  let bv = ctx.alloc_local();
  let bkv_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(bkv_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(bk));
  ctx.emit(Instruction::LocalGet(bkv_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(bv));

  // Scan b for this a key
  let found_in_a = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found_in_a));
  let ai = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ai));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  let akv_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(b_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(akv_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(bk));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));

  // a key found in b — mark and break inner loop
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found_in_a));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // If NOT found in b, copy a[bi] kv to result
  ctx.emit(Instruction::LocalGet(found_in_a));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    let addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst_root));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(addr));
    ctx.emit(Instruction::LocalGet(addr));
    ctx.emit(Instruction::LocalGet(bk));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(addr));
    ctx.emit(Instruction::LocalGet(bv));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_idx));
  }
  ctx.emit(Instruction::End);

  // bi++
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count
  ctx.emit(Instruction::LocalGet(dst_root));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  let dst = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_from_flat", dst_root);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:diff-keys a b` — set of keys in `a` that are NOT in `b`.
pub(super) fn emit_map_diff_keys(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:diff-keys expects 2 args".into());
  }
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let a_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let b_count = emit_load_count_i32(ctx, b);
  let b_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", b);

  // Result is a set: over-allocate with a_count
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst = emit_alloc_with_count(ctx, a_count, total_slots, "set");

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(write_idx));

  let ai = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ai));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // Load a[ai] key
  let ak = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(a_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(ak));

  // Scan b for key
  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));
  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(b_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(ak));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // If NOT found in b, add key to result set
  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ak));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(write_idx));
  ctx.emit(Instruction::End);

  // ai++
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:common-keys a b` — set of keys present in both `a` and `b`.
pub(super) fn emit_map_common_keys(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:common-keys expects 2 args".into());
  }
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let a_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let b_count = emit_load_count_i32(ctx, b);
  let b_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", b);

  // Result is a set: over-allocate with a_count
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst = emit_alloc_with_count(ctx, a_count, total_slots, "set");

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(write_idx));

  let ai = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ai));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // Load a[ai] key
  let ak = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(a_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(ak));

  // Scan b for key
  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));
  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(b_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(ak));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // If found in b, add key to result set
  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ak));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(write_idx));
  ctx.emit(Instruction::End);

  // ai++
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:destruct map` — returns `[key val rest-map]` for the first entry, or nil if empty.
pub(super) fn emit_map_destruct(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&map:destruct expects 1 arg".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);

  let result = ctx.alloc_local();

  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // empty map → nil
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Else);
  {
    // Linearize to get flat [k0,v0,k1,v1,...] starting at flat+8
    let flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", src);

    let key0 = ctx.alloc_local();
    let val0 = ctx.alloc_local();

    // key0 = flat[8 + 0*16] (first key in linearized array)
    ctx.emit(Instruction::LocalGet(flat));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalSet(key0));

    // val0 = flat[8 + 0*16 + 8]
    ctx.emit(Instruction::LocalGet(flat));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalSet(val0));

    // rest-map = __rt_map_dissoc(src, key0)
    let rest_map = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(src));
    ctx.emit(Instruction::LocalGet(key0));
    ctx.call_rt("__rt_map_dissoc");
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::LocalSet(rest_map));

    // Allocate 3-element list [key0, val0, rest-map]
    let list_ptr = ctx.alloc_local_typed(ValType::I32);
    emit_bump_alloc(ctx, 32, list_ptr, "list"); // 4 * 8 bytes
    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(f64_const(3.0));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(Instruction::LocalGet(key0));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(Instruction::LocalGet(val0));
    ctx.emit(Instruction::F64Store(mem_arg_f64(16)));
    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(Instruction::LocalGet(rest_map));
    ctx.emit(Instruction::F64Store(mem_arg_f64(24)));

    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::LocalSet(result));
  }
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `&merge-non-nil a b` — map merge where nil values from `b` are skipped.
pub(super) fn emit_map_merge_non_nil(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&merge-non-nil expects 2 args".into());
  }
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let a_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let b_count = emit_load_count_i32(ctx, b);
  let b_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", b);

  // Over-allocate: at most a_count + b_count entries
  let max_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(max_count));
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(max_count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst_root = emit_alloc_with_count(ctx, max_count, total_slots, "map");

  // Start by copying all non-nil-valued entries from b into a temporary HAMT via __rt_map_assoc
  // Strategy: build result starting from an empty map, then:
  //   1. Insert all a entries
  //   2. Insert all b entries (skip if value is nil=0.0)
  // Use __rt_map_make to build from flat linearised arrays.
  // Simpler approach: re-use the same logic as emit_map_merge but skip nil values from b.

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(write_idx));

  // Copy all a entries
  let ai = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  {
    let ak = ctx.alloc_local();
    let av = ctx.alloc_local();
    let a_addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(a_flat));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(ai));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalTee(a_addr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalSet(ak));
    ctx.emit(Instruction::LocalGet(a_addr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalSet(av));

    let d_addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst_root));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalTee(d_addr));
    ctx.emit(Instruction::LocalGet(ak));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(d_addr));
    ctx.emit(Instruction::LocalGet(av));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_idx));
  }
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Update existing a entries with b, skip nil b values; add new b entries
  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  {
    let bk = ctx.alloc_local();
    let bv = ctx.alloc_local();
    let b_addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(b_flat));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(bi));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalTee(b_addr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalSet(bk));
    ctx.emit(Instruction::LocalGet(b_addr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalSet(bv));

    // Skip if bv is nil (0.0)
    ctx.emit(Instruction::LocalGet(bv));
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Ne);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    {
      // Scan existing dst for bk; update if found, else insert
      let found_di = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::I32Const(-1i32 as u32 as i32));
      ctx.emit(Instruction::LocalSet(found_di));
      let di = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(di));
      ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
      ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
      ctx.emit(Instruction::LocalGet(di));
      ctx.emit(Instruction::LocalGet(write_idx));
      ctx.emit(Instruction::I32GeU);
      ctx.emit(Instruction::BrIf(1));
      {
        let dk_addr = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(dst_root));
        ctx.emit(Instruction::I32Const(8));
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalGet(di));
        ctx.emit(Instruction::I32Const(16));
        ctx.emit(Instruction::I32Mul);
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalTee(dk_addr));
        ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
        ctx.emit(Instruction::LocalGet(bk));
        ctx.emit(Instruction::F64Eq);
        ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
        ctx.emit(Instruction::LocalGet(di));
        ctx.emit(Instruction::LocalSet(found_di));
        ctx.emit(Instruction::Br(2)); // exit scan loop
        ctx.emit(Instruction::End);
      }
      ctx.emit(Instruction::LocalGet(di));
      ctx.emit(Instruction::I32Const(1));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalSet(di));
      ctx.emit(Instruction::Br(0));
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);

      // found_di >= 0: update value; else: append
      ctx.emit(Instruction::LocalGet(found_di));
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::I32GeS);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
      {
        // Update dst[found_di].value = bv
        let upd_addr = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(dst_root));
        ctx.emit(Instruction::I32Const(8));
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalGet(found_di));
        ctx.emit(Instruction::I32Const(16));
        ctx.emit(Instruction::I32Mul);
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalTee(upd_addr));
        ctx.emit(Instruction::LocalGet(bv));
        ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
      }
      ctx.emit(Instruction::Else);
      {
        // Append new key-value pair
        let new_addr = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(dst_root));
        ctx.emit(Instruction::I32Const(8));
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalGet(write_idx));
        ctx.emit(Instruction::I32Const(16));
        ctx.emit(Instruction::I32Mul);
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalTee(new_addr));
        ctx.emit(Instruction::LocalGet(bk));
        ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
        ctx.emit(Instruction::LocalGet(new_addr));
        ctx.emit(Instruction::LocalGet(bv));
        ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
        ctx.emit(Instruction::LocalGet(write_idx));
        ctx.emit(Instruction::I32Const(1));
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalSet(write_idx));
      }
      ctx.emit(Instruction::End);
    }
    ctx.emit(Instruction::End); // end if bv != nil
  }
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Build the result map via __rt_map_from_flat from the flat dst_root buffer
  ctx.emit(Instruction::LocalGet(dst_root));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  let dst = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_from_flat", dst_root);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Core body for `zipmap xs ys` — local 0 = xs (f64), local 1 = ys (f64).
pub(super) fn emit_zipmap_from_locals(ctx: &mut WasmGenCtx, xs_f64: u32, ys_f64: u32) -> Result<(), String> {
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

  // result = empty map
  let result = ctx.alloc_local();
  emit_map_new(ctx, &[])?;
  ctx.emit(Instruction::LocalSet(result));

  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(min_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // k = xs[i]
  let k = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(xs_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(k));

  // v = ys[i]
  let v = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(ys_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(v));

  // result = map_assoc(result, k, v)
  let result_i32 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(result_i32));
  ctx.emit(Instruction::LocalGet(result_i32));
  ctx.emit(Instruction::LocalGet(k));
  ctx.emit(Instruction::LocalGet(v));
  ctx.call_rt("__rt_map_assoc");
  ctx.emit(Instruction::F64ConvertI32U);
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

/// `zipmap xs ys` — call-site intercept.
pub(super) fn emit_zipmap(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err(format!("zipmap expects 2 args, got {}", args.len()));
  }
  let xs = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(xs));
  let ys = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(ys));
  emit_zipmap_from_locals(ctx, xs, ys)
}
