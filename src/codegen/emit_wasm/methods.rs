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
    "empty" => {
      if !extra_locals.is_empty() {
        return Err("method .empty expects 0 arguments".into());
      }
      emit_method_empty_value(ctx, receiver)
    }
    "empty?" => {
      if !extra_locals.is_empty() {
        return Err("method .empty? expects 0 arguments".into());
      }
      emit_method_empty_question(ctx, receiver)
    }
    "count" => {
      if !extra_locals.is_empty() {
        return Err("method .count expects 0 arguments".into());
      }
      emit_method_count(ctx, receiver)
    }
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
    "contains?" => {
      if extra_locals.len() != 1 {
        return Err("method .contains? expects 1 argument".into());
      }
      emit_method_contains(ctx, receiver, extra_locals[0])
    }
    "includes?" => {
      if extra_locals.len() != 1 {
        return Err("method .includes? expects 1 argument".into());
      }
      emit_method_includes(ctx, receiver, extra_locals[0])
    }
    "min" => {
      if !extra_locals.is_empty() {
        return Err("method .min expects 0 arguments".into());
      }
      emit_method_min(ctx, receiver)
    }
    "max" => {
      if !extra_locals.is_empty() {
        return Err("method .max expects 0 arguments".into());
      }
      emit_method_max(ctx, receiver)
    }
    "assoc" => {
      if extra_locals.len() != 2 {
        return Err("method .assoc expects 2 arguments".into());
      }
      emit_method_assoc(ctx, receiver, extra_locals[0], extra_locals[1])
    }
    // .deref — for atom/ref types; falls through when not a ref (return receiver as-is).
    "deref" => {
      if !extra_locals.is_empty() {
        return Err("method .deref expects 0 arguments".into());
      }
      ctx.emit(Instruction::LocalGet(receiver));
      Ok(())
    }
    // .to-list — nil → empty list; otherwise return receiver as-is (already a list).
    "to-list" => {
      if !extra_locals.is_empty() {
        return Err("method .to-list expects 0 arguments".into());
      }
      let result = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(receiver));
      ctx.emit(Instruction::LocalSet(result));
      // if receiver is nil (0.0): allocate and return empty list
      ctx.emit(Instruction::LocalGet(receiver));
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Eq);
      ctx.begin_block_if();
      emit_list_new(ctx, &[])?;
      ctx.emit(Instruction::LocalSet(result));
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::LocalGet(result));
      Ok(())
    }
    // .to-map — nil → empty map; otherwise return receiver as-is (already a map).
    "to-map" => {
      if !extra_locals.is_empty() {
        return Err("method .to-map expects 0 arguments".into());
      }
      let result = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(receiver));
      ctx.emit(Instruction::LocalSet(result));
      // if receiver is nil (0.0): allocate and return empty map
      ctx.emit(Instruction::LocalGet(receiver));
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Eq);
      ctx.begin_block_if();
      emit_map_new(ctx, &[])?;
      ctx.emit(Instruction::LocalSet(result));
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::LocalGet(result));
      Ok(())
    }
    // .slice start end — fallback for non-list/string types; return nil (0.0).
    // List and string cases are handled before this branch in the definition.
    "slice" => {
      ctx.emit(f64_const(0.0));
      Ok(())
    }
    // .filter f — fallback for non-list types; return nil (0.0).
    // List case is handled before this branch in the definition.
    "filter" => {
      ctx.emit(f64_const(0.0));
      Ok(())
    }
    // .map f — fallback for non-list/non-set types (e.g., maps); return nil (0.0).
    // List case is handled via the `map` HOF interceptor before reaching here.
    "map" => {
      ctx.emit(f64_const(0.0));
      Ok(())
    }
    // .display-by radix — number radix formatting; delegates to &number:display-by runtime.
    "display-by" => {
      if extra_locals.len() != 1 {
        return Err("method .display-by expects 1 argument (radix)".into());
      }
      ctx.emit(Instruction::LocalGet(receiver)); // value
      ctx.emit(Instruction::LocalGet(extra_locals[0])); // radix
      ctx.call_rt("__rt_display_by");
      Ok(())
    }
    // .blank? — true if all chars are whitespace
    "blank?" => {
      if !extra_locals.is_empty() {
        return Err("method .blank? expects 0 arguments".into());
      }
      ctx.emit(Instruction::LocalGet(receiver));
      ctx.call_rt("__rt_blank");
      Ok(())
    }
    // .get-char-code — Unicode code point of first character
    "get-char-code" => {
      if !extra_locals.is_empty() {
        return Err("method .get-char-code expects 0 arguments".into());
      }
      // emit_get_char_code needs args slice; use a quick inline approach:
      // load receiver, convert to ptr, decode UTF-8
      let ptr = ctx.alloc_local_typed(wasm_encoder::ValType::I32);
      ctx.emit(Instruction::LocalGet(receiver));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::LocalSet(ptr));
      let content = ctx.alloc_local_typed(wasm_encoder::ValType::I32);
      ctx.emit(Instruction::LocalGet(ptr));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalSet(content));
      let b0 = ctx.alloc_local_typed(wasm_encoder::ValType::I32);
      ctx.emit(Instruction::LocalGet(content));
      ctx.emit(Instruction::I32Load8U(super::mem_arg_byte(0)));
      ctx.emit(Instruction::LocalSet(b0));
      let result = ctx.alloc_local_typed(wasm_encoder::ValType::I32);
      // ASCII fast path
      ctx.emit(Instruction::LocalGet(b0));
      ctx.emit(Instruction::I32Const(0x80));
      ctx.emit(Instruction::I32LtU);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
      ctx.emit(Instruction::LocalGet(b0));
      ctx.emit(Instruction::LocalSet(result));
      ctx.emit(Instruction::Else);
      // 2-byte
      ctx.emit(Instruction::LocalGet(b0));
      ctx.emit(Instruction::I32Const(0xE0));
      ctx.emit(Instruction::I32LtU);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
      {
        let b1 = ctx.alloc_local_typed(wasm_encoder::ValType::I32);
        ctx.emit(Instruction::LocalGet(content));
        ctx.emit(Instruction::I32Load8U(super::mem_arg_byte(1)));
        ctx.emit(Instruction::LocalSet(b1));
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
      // 3-byte
      ctx.emit(Instruction::LocalGet(b0));
      ctx.emit(Instruction::I32Const(0xF0));
      ctx.emit(Instruction::I32LtU);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
      {
        let b1 = ctx.alloc_local_typed(wasm_encoder::ValType::I32);
        let b2 = ctx.alloc_local_typed(wasm_encoder::ValType::I32);
        ctx.emit(Instruction::LocalGet(content));
        ctx.emit(Instruction::I32Load8U(super::mem_arg_byte(1)));
        ctx.emit(Instruction::LocalSet(b1));
        ctx.emit(Instruction::LocalGet(content));
        ctx.emit(Instruction::I32Load8U(super::mem_arg_byte(2)));
        ctx.emit(Instruction::LocalSet(b2));
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
        // 4-byte
        let b1 = ctx.alloc_local_typed(wasm_encoder::ValType::I32);
        let b2 = ctx.alloc_local_typed(wasm_encoder::ValType::I32);
        let b3 = ctx.alloc_local_typed(wasm_encoder::ValType::I32);
        ctx.emit(Instruction::LocalGet(content));
        ctx.emit(Instruction::I32Load8U(super::mem_arg_byte(1)));
        ctx.emit(Instruction::LocalSet(b1));
        ctx.emit(Instruction::LocalGet(content));
        ctx.emit(Instruction::I32Load8U(super::mem_arg_byte(2)));
        ctx.emit(Instruction::LocalSet(b2));
        ctx.emit(Instruction::LocalGet(content));
        ctx.emit(Instruction::I32Load8U(super::mem_arg_byte(3)));
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
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::LocalGet(result));
      ctx.emit(Instruction::F64ConvertI32U);
      Ok(())
    }
    // .parse-float — parse string as decimal number
    "parse-float" => {
      if !extra_locals.is_empty() {
        return Err("method .parse-float expects 0 arguments".into());
      }
      ctx.emit(Instruction::LocalGet(receiver));
      ctx.call_rt("__rt_parse_float");
      Ok(())
    }
    // .split pat — split string by pattern, return list of non-empty pieces
    "split" => {
      if extra_locals.len() != 1 {
        return Err("method .split expects 1 argument (pattern)".into());
      }
      let s = ctx.alloc_local_typed(ValType::I32);
      let pat = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalGet(receiver));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::LocalSet(s));
      ctx.emit(Instruction::LocalGet(extra_locals[0]));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::LocalSet(pat));
      ctx.emit(Instruction::LocalGet(s));
      ctx.emit(Instruction::LocalGet(pat));
      ctx.call_rt("__rt_str_split");
      ctx.emit(Instruction::F64ConvertI32U);
      Ok(())
    }
    // .split-lines — split string by '\n', return list of strings
    "split-lines" => {
      if !extra_locals.is_empty() {
        return Err("method .split-lines expects 0 arguments".into());
      }
      let s = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalGet(receiver));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::LocalSet(s));
      // build "\n" string inline
      let one = ctx.alloc_i32(1);
      let (ptr_nl, cont_nl) = super::strings::emit_str_alloc(ctx, one);
      ctx.emit(Instruction::LocalGet(cont_nl));
      ctx.emit(Instruction::I32Const(0x0A));
      ctx.emit(Instruction::I32Store8(super::mem_arg_byte(0)));
      ctx.emit(Instruction::LocalGet(s));
      ctx.emit(Instruction::LocalGet(ptr_nl));
      ctx.call_rt("__rt_str_split");
      ctx.emit(Instruction::F64ConvertI32U);
      Ok(())
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

  ctx.ptr_to_f64(dst);
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

fn emit_ptr_local_from_receiver(ctx: &mut WasmGenCtx, receiver_local: u32) -> u32 {
  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr_local));
  ptr_local
}

fn emit_list_contains_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, index_local: u32) {
  let ptr_local = emit_ptr_local_from_receiver(ctx, receiver_local);
  let count_local = emit_load_count_i32(ctx, ptr_local);
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalGet(count_local));
  ctx.emit(Instruction::I32LtU);
  ctx.emit(Instruction::Select);
}

fn emit_list_includes_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, target_local: u32) {
  let ptr_local = emit_ptr_local_from_receiver(ctx, receiver_local);
  let count_local = emit_load_count_i32(ctx, ptr_local);
  let result_local = ctx.alloc_local();
  let index_local = ctx.alloc_local_typed(ValType::I32);

  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result_local));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(index_local));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));

  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::LocalGet(count_local));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(target_local));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::LocalSet(result_local));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(index_local));
  ctx.emit(Instruction::Br(0));

  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(result_local));
}

fn emit_map_contains_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, key_local: u32) {
  let ptr_local = emit_ptr_local_from_receiver(ctx, receiver_local);
  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_map_contains_key", ptr_local, key_local);
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::Select);
}

fn emit_map_includes_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, target_local: u32) {
  let ptr_local = emit_ptr_local_from_receiver(ctx, receiver_local);
  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_map_contains_value", ptr_local, target_local);
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::Select);
}

fn emit_set_includes_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, target_local: u32) {
  let ptr_local = emit_ptr_local_from_receiver(ctx, receiver_local);
  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_set_find_elem", ptr_local, target_local);
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Ne);
  ctx.emit(Instruction::Select);
}

fn emit_sequence_extreme_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, use_min: bool) {
  let ptr_local = emit_ptr_local_from_receiver(ctx, receiver_local);
  let count_local = emit_load_count_i32(ctx, ptr_local);
  let best_local = ctx.alloc_local();
  let index_local = ctx.alloc_local_typed(ValType::I32);

  ctx.emit(Instruction::LocalGet(count_local));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::Else);

  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(best_local));

  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(index_local));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));

  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::LocalGet(count_local));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(best_local));
  if use_min {
    ctx.emit(Instruction::F64Lt);
  } else {
    ctx.emit(Instruction::F64Gt);
  }
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(best_local));
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(index_local));
  ctx.emit(Instruction::Br(0));

  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(best_local));
  ctx.emit(Instruction::End);
}

fn emit_sequence_extreme_option_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, use_min: bool) -> Result<(), String> {
  let ptr_local = emit_ptr_local_from_receiver(ctx, receiver_local);
  let count_local = emit_load_count_i32(ctx, ptr_local);
  ctx.emit(Instruction::LocalGet(count_local));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_option_tuple(ctx, None)?;
  ctx.emit(Instruction::Else);
  emit_sequence_extreme_from_local(ctx, receiver_local, use_min);
  let value_local = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(value_local));
  emit_option_tuple(ctx, Some(value_local))?;
  ctx.emit(Instruction::End);
  Ok(())
}

/// Allocates a 1-byte string containing the byte at (receiver+8+idx) and returns f64 ptr.
/// Emits an inline If/Else that returns nil (0.0) when idx is out-of-range.
fn emit_str_nth_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, index_local: u32) {
  let str_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(str_len));

  let idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32TruncF64S);
  ctx.emit(Instruction::LocalSet(idx));

  // idx < 0 → nil
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32LtS);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::Else);
  // idx >= str_len → nil
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::LocalGet(str_len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::Else);

  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, 16, ptr, "string"); // 8 byte_len + 8 (1 byte padded to 8)
  // write byte_len = 1
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  // write the byte
  let byte = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(8)); // skip byte_len field
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  ctx.emit(Instruction::LocalSet(byte));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8)); // content base offset
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Store8(mem_arg_byte(0)));
  ctx.ptr_to_f64(ptr);

  ctx.emit(Instruction::End); // end inner else
  ctx.emit(Instruction::End); // end outer else
}

/// Returns a 1-byte string for the first byte of a string, or nil if empty.
fn emit_str_first_from_local(ctx: &mut WasmGenCtx, receiver_local: u32) {
  let str_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(str_len));

  // empty → nil
  ctx.emit(Instruction::LocalGet(str_len));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::Else);

  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, 16, ptr, "string");
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  let byte = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Load8U(mem_arg_byte(8))); // first byte after byte_len field
  ctx.emit(Instruction::LocalSet(byte));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Store8(mem_arg_byte(0)));
  ctx.ptr_to_f64(ptr);

  ctx.emit(Instruction::End); // end else
}

fn emit_empty_string(ctx: &mut WasmGenCtx) {
  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, 8, ptr_local, "string");
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.ptr_to_f64(ptr_local);
}

fn emit_method_empty_value(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
  let list_tag = get_type_tag(ctx, "list");
  let map_tag = get_type_tag(ctx, "map");
  let set_tag = get_type_tag(ctx, "set");
  let string_tag = get_type_tag(ctx, "string");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_list_new(ctx, &[])?;
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(map_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_map_new(ctx, &[])?;
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(set_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_set_new(ctx, &[])?;
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(string_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_empty_string(ctx);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_empty_question(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
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

fn emit_method_contains(ctx: &mut WasmGenCtx, receiver_local: u32, key_local: u32) -> Result<(), String> {
  let list_tag = get_type_tag(ctx, "list");
  let map_tag = get_type_tag(ctx, "map");
  let set_tag = get_type_tag(ctx, "set");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_list_contains_from_local(ctx, receiver_local, key_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(map_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_map_contains_from_local(ctx, receiver_local, key_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(set_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_set_includes_from_local(ctx, receiver_local, key_local);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_includes(ctx: &mut WasmGenCtx, receiver_local: u32, target_local: u32) -> Result<(), String> {
  let list_tag = get_type_tag(ctx, "list");
  let map_tag = get_type_tag(ctx, "map");
  let set_tag = get_type_tag(ctx, "set");
  let string_tag = get_type_tag(ctx, "string");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_list_includes_from_local(ctx, receiver_local, target_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(map_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_map_includes_from_local(ctx, receiver_local, target_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(set_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_set_includes_from_local(ctx, receiver_local, target_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(string_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_str_includes_from_local(ctx, receiver_local, target_local);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

/// `.includes? str needle` — string substring check via __rt_str_find_index >= 0.
fn emit_str_includes_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, target_local: u32) {
  use wasm_encoder::Ieee64;
  let recv_ptr = emit_ptr_local_from_receiver(ctx, receiver_local);
  let needle_ptr = emit_ptr_local_from_receiver(ctx, target_local);
  ctx.emit(Instruction::LocalGet(recv_ptr));
  ctx.emit(Instruction::LocalGet(needle_ptr));
  ctx.call_rt("__rt_str_find_index");
  ctx.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::F64ConvertI32U);
}

fn emit_method_min(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
  let list_tag = get_type_tag(ctx, "list");
  let set_tag = get_type_tag(ctx, "set");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_sequence_extreme_option_from_local(ctx, receiver_local, true)?;
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(set_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_sequence_extreme_option_from_local(ctx, receiver_local, true)?;
  ctx.emit(Instruction::Else);
  emit_option_tuple(ctx, None)?;
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_max(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
  let list_tag = get_type_tag(ctx, "list");
  let set_tag = get_type_tag(ctx, "set");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_sequence_extreme_option_from_local(ctx, receiver_local, false)?;
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(set_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_sequence_extreme_option_from_local(ctx, receiver_local, false)?;
  ctx.emit(Instruction::Else);
  emit_option_tuple(ctx, None)?;
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_map_assoc_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, key_local: u32, val_local: u32) {
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_map_assoc")
    .expect("runtime helper __rt_map_assoc must exist");
  let ptr_local = emit_ptr_local_from_receiver(ctx, receiver_local);
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::LocalGet(key_local));
  ctx.emit(Instruction::LocalGet(val_local));
  ctx.emit(Instruction::Call(fn_idx));
  ctx.emit(Instruction::F64ConvertI32U);
}

fn emit_list_assoc_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, index_local: u32, val_local: u32) {
  let ptr_local = emit_ptr_local_from_receiver(ctx, receiver_local);
  let count_local = super::emit_load_count_i32(ctx, ptr_local);
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  let idx_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx_local));

  ctx.emit(Instruction::LocalGet(count_local));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = super::emit_alloc_with_count(ctx, count_local, total_slots, "list");

  let dst_base = super::emit_addr_offset(ctx, dst, 8);
  let src_base = super::emit_addr_offset(ctx, ptr_local, 8);
  super::emit_copy_f64_loop(ctx, dst_base, src_base, count_local);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(idx_local));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(val_local));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.ptr_to_f64(dst);
}

fn emit_tuple_assoc_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, index_local: u32, val_local: u32) {
  let ptr_local = emit_ptr_local_from_receiver(ctx, receiver_local);
  let count_local = super::emit_load_count_i32(ctx, ptr_local);
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  let idx_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx_local));

  ctx.emit(Instruction::LocalGet(count_local));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let size = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(total_slots));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalSet(size));

  let dst = ctx.alloc_local_typed(ValType::I32);
  super::emit_bump_alloc_dynamic(ctx, size, dst, "tuple");

  let dst_base = super::emit_addr_offset(ctx, dst, 0);
  let src_base = super::emit_addr_offset(ctx, ptr_local, 0);
  super::emit_copy_f64_loop(ctx, dst_base, src_base, total_slots);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(idx_local));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(val_local));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.ptr_to_f64(dst);
}

fn emit_method_assoc(ctx: &mut WasmGenCtx, receiver_local: u32, key_local: u32, val_local: u32) -> Result<(), String> {
  let list_tag = super::get_type_tag(ctx, "list");
  let map_tag = super::get_type_tag(ctx, "map");
  let tuple_tag = super::get_type_tag(ctx, "tuple");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_list_assoc_from_local(ctx, receiver_local, key_local, val_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(map_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_map_assoc_from_local(ctx, receiver_local, key_local, val_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(tuple_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_tuple_assoc_from_local(ctx, receiver_local, key_local, val_local);
  ctx.emit(Instruction::Else);
  // Unhandled (record not implemented yet in dynamic dispatch)
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
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
  let string_tag = get_type_tag(ctx, "string");
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
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(string_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_str_nth_from_local(ctx, receiver_local, index_local);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_first(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
  let list_tag = get_type_tag(ctx, "list");
  let tuple_tag = get_type_tag(ctx, "tuple");
  let string_tag = get_type_tag(ctx, "string");
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
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(string_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_str_first_from_local(ctx, receiver_local);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
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
