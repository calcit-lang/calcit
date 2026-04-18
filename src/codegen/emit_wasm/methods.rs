use super::*;

pub(super) fn emit_method_invoke(ctx: &mut WasmGenCtx, name: &str, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    return Err(format!("method .{name} expects at least 1 operand"));
  }

  let receiver = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(receiver));

  let extra_locals: Vec<u32> = args[1..]
    .iter()
    .map(|arg| {
      let local = ctx.alloc_local();
      emit_expr(ctx, arg)?;
      ctx.emit(Instruction::LocalSet(local));
      Ok(local)
    })
    .collect::<Result<_, String>>()?;

  match name {
    "empty?" => emit_method_empty(ctx, receiver),
    "count" => emit_method_count(ctx, receiver),
    "first" => {
      if !extra_locals.is_empty() {
        return Err("method .first expects 0 arguments".into());
      }
      emit_method_first(ctx, receiver)
    }
    "rest" => {
      if !extra_locals.is_empty() {
        return Err("method .rest expects 0 arguments".into());
      }
      emit_method_rest(ctx, receiver)
    }
    "nth" => {
      if extra_locals.len() != 1 {
        return Err("method .nth expects 1 argument".into());
      }
      emit_method_nth(ctx, receiver, extra_locals[0])
    }
    "get" => {
      if extra_locals.len() != 1 {
        return Err("method .get expects 1 argument".into());
      }
      emit_method_get(ctx, receiver, extra_locals[0])
    }
    _ => Err(format!("unsupported invoke method in WASM: .{name}")),
  }
}

fn emit_type_of_local(ctx: &mut WasmGenCtx, value_local: u32) {
  let number_tag = get_type_tag(ctx, "number");
  let is_valid_ptr = ctx.alloc_local_typed(ValType::I32);
  let raw_base = ctx.alloc_local_typed(ValType::I32);

  ctx.emit(Instruction::LocalGet(value_local));
  ctx.emit(Instruction::LocalGet(value_local));
  ctx.emit(Instruction::F64Trunc);
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::LocalGet(value_local));
  ctx.emit(f64_const((HEAP_BASE + 8) as f64));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalGet(value_local));
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(is_valid_ptr));

  ctx.emit(Instruction::LocalGet(value_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(raw_base));

  ctx.emit(Instruction::LocalGet(is_valid_ptr));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(Instruction::LocalGet(raw_base));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(HEAP_MAGIC));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(Instruction::LocalGet(raw_base));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(number_tag));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(number_tag));
  ctx.emit(Instruction::End);
}

fn emit_heap_count_from_local(ctx: &mut WasmGenCtx, receiver_local: u32) {
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
}

fn emit_list_nth_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, index_local: u32) {
  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
}

fn emit_list_first_from_local(ctx: &mut WasmGenCtx, receiver_local: u32) {
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
}

fn emit_list_rest_from_local(ctx: &mut WasmGenCtx, receiver_local: u32) {
  let src = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(src));

  let old_count = emit_load_count_i32(ctx, src);
  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(new_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, new_count, total_slots, "list");
  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 16);
  emit_copy_f64_loop(ctx, dst_base, src_base, new_count);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
}

fn emit_tuple_nth_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, index_local: u32) {
  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
}

fn emit_map_get_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, key_local: u32) {
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_map_get_value")
    .expect("runtime helper __rt_map_get_value must exist");
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalGet(key_local));
  ctx.emit(Instruction::Call(fn_idx));
}

fn emit_method_empty(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
  let nil_tag = get_type_tag(ctx, "nil");
  let list_tag = get_type_tag(ctx, "list");
  let map_tag = get_type_tag(ctx, "map");
  let set_tag = get_type_tag(ctx, "set");
  let string_tag = get_type_tag(ctx, "string");
  let tuple_tag = get_type_tag(ctx, "tuple");
  let record_tag = get_type_tag(ctx, "record");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(nil_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::Else);

  for tag in [list_tag, map_tag, set_tag, string_tag, tuple_tag, record_tag] {
    ctx.emit(Instruction::LocalGet(type_local));
    ctx.emit(f64_const(tag));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
    ctx.emit(f64_const(1.0));
    ctx.emit(f64_const(0.0));
    emit_heap_count_from_local(ctx, receiver_local);
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::Select);
    ctx.emit(Instruction::Else);
  }

  ctx.emit(f64_const(0.0));
  for _ in 0..6 {
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_count(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
  let nil_tag = get_type_tag(ctx, "nil");
  let list_tag = get_type_tag(ctx, "list");
  let map_tag = get_type_tag(ctx, "map");
  let set_tag = get_type_tag(ctx, "set");
  let string_tag = get_type_tag(ctx, "string");
  let tuple_tag = get_type_tag(ctx, "tuple");
  let record_tag = get_type_tag(ctx, "record");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(nil_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::Else);

  for tag in [list_tag, map_tag, set_tag, string_tag, tuple_tag, record_tag] {
    ctx.emit(Instruction::LocalGet(type_local));
    ctx.emit(f64_const(tag));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
    emit_heap_count_from_local(ctx, receiver_local);
    ctx.emit(Instruction::Else);
  }

  ctx.emit(f64_const(0.0));
  for _ in 0..6 {
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_nth(ctx: &mut WasmGenCtx, receiver_local: u32, index_local: u32) -> Result<(), String> {
  let list_tag = get_type_tag(ctx, "list");
  let tuple_tag = get_type_tag(ctx, "tuple");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_list_nth_from_local(ctx, receiver_local, index_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(tuple_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_tuple_nth_from_local(ctx, receiver_local, index_local);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_first(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
  let list_tag = get_type_tag(ctx, "list");
  let tuple_tag = get_type_tag(ctx, "tuple");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_list_first_from_local(ctx, receiver_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(tuple_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  let zero = ctx.alloc_local();
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(zero));
  emit_tuple_nth_from_local(ctx, receiver_local, zero);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_rest(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
  let list_tag = get_type_tag(ctx, "list");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_list_rest_from_local(ctx, receiver_local);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_get(ctx: &mut WasmGenCtx, receiver_local: u32, key_local: u32) -> Result<(), String> {
  let map_tag = get_type_tag(ctx, "map");
  let list_tag = get_type_tag(ctx, "list");
  let tuple_tag = get_type_tag(ctx, "tuple");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(map_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_map_get_from_local(ctx, receiver_local, key_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_list_nth_from_local(ctx, receiver_local, key_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(tuple_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_tuple_nth_from_local(ctx, receiver_local, key_local);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

/// Emit argument evaluation for a direct function call.
///
/// Handles three cases:
/// - Fixed-arity (no rest): evaluates each arg, pads nil for missing optional args.
/// - Rest args (callee has `&`): evaluates the first `fixed` args as-is, then
///   packs the remaining args into a list and passes that as the last f64 param.
pub(super) fn emit_call_args(
  ctx: &mut WasmGenCtx,
  args_list: &[Calcit],
  target_arity: u32,
  rest_fixed: Option<u32>,
) -> Result<(), String> {
  match rest_fixed {
    Some(fixed) => {
      let fixed = fixed as usize;
      if args_list.len() < fixed {
        return Err(format!("rest-args call expected at least {} args, got {}", fixed, args_list.len()));
      }
      // Emit fixed args directly
      for arg in args_list.iter().take(fixed) {
        emit_expr(ctx, arg)?;
      }
      // Pack the rest into a list
      emit_list_new(ctx, &args_list[fixed..])?;
    }
    None => {
      for arg in args_list {
        emit_expr(ctx, arg)?;
      }
      // Pad nil for missing optional args
      for _ in args_list.len()..(target_arity as usize) {
        ctx.emit(f64_const(0.0));
      }
    }
  }
  Ok(())
}
