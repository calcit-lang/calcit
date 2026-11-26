//! RFC 8259 streaming JSON parser WASM runtime helpers.

use super::*;
use wasm_encoder::BlockType;

/// `__rt_json_parse_str_at(src_ptr: i32, pos: i32, end: i32) → (str_ptr: i32, next_pos: i32, ok: i32)`
///
/// Parses an RFC 8259 JSON string starting at `pos` (where `src[pos] == '"'`).
/// Supports `\"`, `\\`, `\/`, `\b`, `\f`, `\n`, `\r`, `\t`, and `\uXXXX` (including surrogate pairs).
pub(in crate::wasm::emit_wasm) fn build_rt_json_parse_str_at(string_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3); // 0=src_ptr, 1=pos, 2=end
  let cur = b.alloc_i32();
  let max_len = b.alloc_i32();
  let padded = b.alloc_i32();
  let payload = b.alloc_i32();
  let req_bytes = b.alloc_i32();
  let heap_mark = b.alloc_i32();
  let out = b.alloc_i32();
  let dst = b.alloc_i32();
  let out_idx = b.alloc_i32();
  let byte = b.alloc_i32();
  let cp = b.alloc_i32();
  let cp2 = b.alloc_i32();
  let hi = b.alloc_i32();
  let nib = b.alloc_i32();

  let fail_after_alloc = |b: &mut RuntimeFnBuilder, cur_local: u32| {
    b.emit(Instruction::LocalGet(out));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::I32Store(mem_arg_i32(0)));
    b.emit(Instruction::LocalGet(heap_mark));
    b.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::Return);
  };

  // Must start with '"'
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  // Allocate pessimistic output string of length (end - pos), and shrink/rewind on exit.
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(max_len));
  b.emit(Instruction::LocalGet(max_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(payload));
  b.emit(Instruction::LocalGet(payload));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(req_bytes));
  rt_emit_ensure_heap_capacity(&mut b, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.emit(Instruction::LocalSet(heap_mark));
  rt_emit_alloc_dynamic(&mut b, payload, out, string_tag);
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst));

  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(out_idx));

  let emit_hex_nibble = |b: &mut RuntimeFnBuilder, src_local: u32, cur_local: u32, end_local: u32, nib_local: u32, acc_local: u32| {
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::LocalGet(end_local));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::If(BlockType::Empty));
    fail_after_alloc(b, cur_local);
    b.emit(Instruction::End);
    b.emit(Instruction::LocalGet(src_local));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
    b.emit(Instruction::LocalSet(nib_local));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(cur_local));

    b.emit(Instruction::LocalGet(nib_local));
    b.emit(Instruction::I32Const(b'0' as i32));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::LocalGet(nib_local));
    b.emit(Instruction::I32Const(b'9' as i32));
    b.emit(Instruction::I32LeU);
    b.emit(Instruction::I32And);
    b.emit(Instruction::If(BlockType::Result(ValType::I32)));
    b.emit(Instruction::LocalGet(nib_local));
    b.emit(Instruction::I32Const(b'0' as i32));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::Else);
    b.emit(Instruction::LocalGet(nib_local));
    b.emit(Instruction::I32Const(0x20));
    b.emit(Instruction::I32Or);
    b.emit(Instruction::LocalTee(nib_local));
    b.emit(Instruction::I32Const(b'a' as i32));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::LocalGet(nib_local));
    b.emit(Instruction::I32Const(b'f' as i32));
    b.emit(Instruction::I32LeU);
    b.emit(Instruction::I32And);
    b.emit(Instruction::If(BlockType::Result(ValType::I32)));
    b.emit(Instruction::LocalGet(nib_local));
    b.emit(Instruction::I32Const((b'a' - 10) as i32));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::Else);
    fail_after_alloc(b, cur_local);
    b.emit(Instruction::Unreachable);
    b.emit(Instruction::End);
    b.emit(Instruction::End);
    b.emit(Instruction::LocalGet(acc_local));
    b.emit(Instruction::I32Const(4));
    b.emit(Instruction::I32Shl);
    b.emit(Instruction::I32Or);
    b.emit(Instruction::LocalSet(acc_local));
  };

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));

  // Closing '"'
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  // Reclaim unused trailing padding beyond the decoded string byte length.
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  // Unescaped control characters (< 0x20) are invalid in JSON strings.
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x20));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);

  // Escape sequence
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'u' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(cp));
  for _ in 0..4 {
    emit_hex_nibble(&mut b, 0, cur, 2, nib, cp);
  }
  // High surrogate 0xD800..0xDBFF must be followed by `\uDC00`..`\uDFFF`
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0xd800));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0xdbff));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(6));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(9)));
  b.emit(Instruction::I32Const(b'u' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::LocalSet(hi));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(cp2));
  for _ in 0..4 {
    emit_hex_nibble(&mut b, 0, cur, 2, nib, cp2);
  }
  b.emit(Instruction::LocalGet(cp2));
  b.emit(Instruction::I32Const(0xdc00));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(cp2));
  b.emit(Instruction::I32Const(0xdfff));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(hi));
  b.emit(Instruction::I32Const(0xd800));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Const(10));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::LocalGet(cp2));
  b.emit(Instruction::I32Const(0xdc00));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x10000));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cp));
  b.emit(Instruction::Else);
  // Reject lone low surrogate 0xDC00..0xDFFF
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0xdc00));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0xdfff));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Encode Unicode scalar `cp` as UTF-8 into `dst + out_idx`
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0x800));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0xc0));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(6));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0x3f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(1)));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0x10000));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0xe0));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(12));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(6));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(0x3f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(1)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0x3f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(2)));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0xf0));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(18));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(12));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(0x3f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(1)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(6));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(0x3f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(2)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0x3f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(3)));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  // Single-character JSON escapes: \", \\, \/, \b, \f, \n, \r, \t
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'b' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x08));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'f' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x0c));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'n' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x0a));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'r' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x0d));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b't' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x09));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::Else);
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_idx));
  b.emit(Instruction::End);

  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  fail_after_alloc(&mut b, cur);
  b.finish(vec![ValType::I32; 3], vec![ValType::I32; 3])
}

/// Tag IDs and equality helper needed to construct closed `JsonValue` enum values directly.
#[derive(Clone, Copy)]
pub(in crate::wasm::emit_wasm) struct JsonValueEnumTags {
  pub enum_tag: i32,
  pub null_variant: u32,
  pub bool_variant: u32,
  pub number_variant: u32,
  pub string_variant: u32,
  pub array_variant: u32,
  pub object_variant: u32,
  pub map_key_equal_idx: u32,
}

/// `__rt_json_parse_at(src_ptr: i32, pos: i32, end: i32, depth: i32) → (val: f64, next_pos: i32, ok: i32)`
pub(in crate::wasm::emit_wasm) fn build_rt_json_parse_at(
  tags: (i32, i32, i32, i32, i32, i32),
  fns: (u32, u32, u32, u32, u32, u32),
) -> CompiledFn {
  build_rt_json_parse_at_impl(tags, fns, None)
}

/// `__rt_json_parse_value_at(src_ptr: i32, pos: i32, end: i32, depth: i32) → (val: f64, next_pos: i32, ok: i32)`
///
/// Parses RFC 8259 JSON directly into closed `JsonValue` enum values with `String` object keys
/// and unboxed `Bool`/`Number` payloads (no boxed-scalar ABI needed).
pub(in crate::wasm::emit_wasm) fn build_rt_json_parse_value_at(
  tags: (i32, i32, i32, i32, i32, i32),
  fns: (u32, u32, u32, u32, u32, u32),
  json_value: JsonValueEnumTags,
) -> CompiledFn {
  build_rt_json_parse_at_impl(tags, fns, Some(json_value))
}

fn build_rt_json_parse_at_impl(
  (string_tag, list_tag, map_tag, nil_tag, bool_tag, number_tag): (i32, i32, i32, i32, i32, i32),
  (self_idx, parse_str_idx, tag_from_str_idx, map_from_flat_idx, parse_float_valid_idx, parse_float_idx): (
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
  ),
  json_value: Option<JsonValueEnumTags>,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(4); // 0=src_ptr, 1=pos, 2=end, 3=depth
  let cur = b.alloc_i32();
  let byte = b.alloc_i32();
  let str_ptr = b.alloc_i32();
  let boxed_ptr = b.alloc_i32();
  let ok = b.alloc_i32();
  let val = b.alloc_f64();
  let list_buf = b.alloc_i32();
  let list_cap = b.alloc_i32();
  let list_cnt = b.alloc_i32();
  let list_alloc_size = b.alloc_i32();
  let obj_buf = b.alloc_i32();
  let obj_cap = b.alloc_i32();
  let pair_cnt = b.alloc_i32();
  let member_cnt = b.alloc_i32();
  let obj_alloc_size = b.alloc_i32();
  let key_heap_mark = b.alloc_i32();
  let j = b.alloc_i32();
  let found_key = b.alloc_i32();
  let req_bytes = b.alloc_i32();
  let map_ptr = b.alloc_i32();
  let key_tag = b.alloc_f64();
  let num_start = b.alloc_i32();
  let num_len = b.alloc_i32();
  let num_padded = b.alloc_i32();
  let num_payload = b.alloc_i32();
  let num_heap_mark = b.alloc_i32();
  let num_str = b.alloc_i32();
  let int_digits = b.alloc_i32();
  let first_digit = b.alloc_i32();

  let skip_ws = |b: &mut RuntimeFnBuilder, src_local: u32, cur_local: u32, end_local: u32, byte_local: u32| {
    b.emit(Instruction::Block(BlockType::Empty));
    b.emit(Instruction::Loop(BlockType::Empty));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::LocalGet(end_local));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::BrIf(1));
    b.emit(Instruction::LocalGet(src_local));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
    b.emit(Instruction::LocalSet(byte_local));
    emit_is_ws_check(&mut b.instructions, byte_local);
    b.emit(Instruction::I32Eqz);
    b.emit(Instruction::BrIf(1));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(cur_local));
    b.emit(Instruction::Br(0));
    b.emit(Instruction::End);
    b.emit(Instruction::End);
  };

  let fail_return = |b: &mut RuntimeFnBuilder, cur_local: u32| {
    b.emit(f64_const(0.0));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::Return);
  };

  let emit_wrap_enum_0 = |b: &mut RuntimeFnBuilder, enum_tag: i32, variant_tag: u32, cur_local: u32| {
    rt_emit_ensure_heap_capacity_const(b, 16, req_bytes);
    b.emit(Instruction::I32Eqz);
    b.emit(Instruction::If(BlockType::Empty));
    fail_return(b, cur_local);
    b.emit(Instruction::End);
    rt_emit_alloc_const(b, 8, boxed_ptr, enum_tag);
    b.emit(Instruction::LocalGet(boxed_ptr));
    b.emit(Instruction::I64Const(((variant_tag as u64) << 32) as i64));
    b.emit(Instruction::I64Store(mem_arg_i64(0)));
    b.emit(Instruction::LocalGet(boxed_ptr));
    b.emit(Instruction::F64ConvertI32U);
  };

  let emit_wrap_enum_1 = |b: &mut RuntimeFnBuilder, enum_tag: i32, variant_tag: u32, payload_local: u32, cur_local: u32| {
    rt_emit_ensure_heap_capacity_const(b, 24, req_bytes);
    b.emit(Instruction::I32Eqz);
    b.emit(Instruction::If(BlockType::Empty));
    fail_return(b, cur_local);
    b.emit(Instruction::End);
    rt_emit_alloc_const(b, 16, boxed_ptr, enum_tag);
    b.emit(Instruction::LocalGet(boxed_ptr));
    b.emit(Instruction::I64Const((((variant_tag as u64) << 32) | 1u64) as i64));
    b.emit(Instruction::I64Store(mem_arg_i64(0)));
    b.emit(Instruction::LocalGet(boxed_ptr));
    b.emit(Instruction::LocalGet(payload_local));
    b.emit(Instruction::F64Store(mem_arg_f64(8)));
    b.emit(Instruction::LocalGet(boxed_ptr));
    b.emit(Instruction::F64ConvertI32U);
  };

  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, 1);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalSet(cur));
  skip_ws(&mut b, 0, cur, 2, byte);

  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));

  // Literal words: box `"null"` with `nil_tag` and `"true"`/`"false"` with `bool_tag` so
  // dynamic JSON values remain distinct from numeric `0.0` / `1.0`.
  for (word, kind_tag, result_val) in [("null", nil_tag, 0.0), ("true", bool_tag, 1.0), ("false", bool_tag, 0.0)] {
    b.emit(Instruction::LocalGet(byte));
    b.emit(Instruction::I32Const(word.as_bytes()[0] as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(cur));
    b.emit(Instruction::I32Const(word.len() as i32));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalGet(2));
    b.emit(Instruction::I32GtU);
    b.emit(Instruction::If(BlockType::Empty));
    fail_return(&mut b, cur);
    b.emit(Instruction::End);
    for (idx, ch) in word.bytes().enumerate().skip(1) {
      b.emit(Instruction::LocalGet(0));
      b.emit(Instruction::LocalGet(cur));
      b.emit(Instruction::I32Add);
      b.emit(Instruction::I32Load8U(mem_arg_byte(8 + idx as u64)));
      b.emit(Instruction::I32Const(ch as i32));
      b.emit(Instruction::I32Ne);
      b.emit(Instruction::If(BlockType::Empty));
      fail_return(&mut b, cur);
      b.emit(Instruction::End);
    }
    if let Some(jv) = json_value {
      if word == "null" {
        emit_wrap_enum_0(&mut b, jv.enum_tag, jv.null_variant, cur);
      } else {
        b.emit(f64_const(result_val));
        b.emit(Instruction::LocalSet(val));
        emit_wrap_enum_1(&mut b, jv.enum_tag, jv.bool_variant, val, cur);
      }
    } else {
      rt_emit_ensure_heap_capacity_const(&mut b, 16, req_bytes);
      b.emit(Instruction::I32Eqz);
      b.emit(Instruction::If(BlockType::Empty));
      fail_return(&mut b, cur);
      b.emit(Instruction::End);
      rt_emit_alloc_const(&mut b, 8, boxed_ptr, kind_tag);
      b.emit(Instruction::LocalGet(boxed_ptr));
      b.emit(f64_const(result_val));
      b.emit(Instruction::F64Store(mem_arg_f64(0)));
      b.emit(Instruction::LocalGet(boxed_ptr));
      b.emit(Instruction::F64ConvertI32U);
    }
    b.emit(Instruction::LocalGet(cur));
    b.emit(Instruction::I32Const(word.len() as i32));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::Return);
    b.emit(Instruction::End);
  }

  // String literal: "..."
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::Call(parse_str_idx));
  b.emit(Instruction::LocalSet(ok));
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalSet(str_ptr));
  b.emit(Instruction::LocalGet(ok));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  if let Some(jv) = json_value {
    b.emit(Instruction::LocalGet(str_ptr));
    b.emit(Instruction::F64ConvertI32U);
    b.emit(Instruction::LocalSet(val));
    emit_wrap_enum_1(&mut b, jv.enum_tag, jv.string_variant, val, cur);
  } else {
    b.emit(Instruction::LocalGet(str_ptr));
    b.emit(Instruction::F64ConvertI32U);
  }
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  // Array literal: [...]
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'[' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  skip_ws(&mut b, 0, cur, 2, byte);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Empty array `[]`
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(b']' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  rt_emit_ensure_heap_capacity_const(&mut b, 16, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  rt_emit_alloc_const(&mut b, 8, list_buf, list_tag);
  b.emit(Instruction::LocalGet(list_buf));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(list_buf));
  b.emit(Instruction::I32Const(ContainerSlotKind::EmptyUntyped as i32));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  if let Some(jv) = json_value {
    b.emit(Instruction::LocalGet(list_buf));
    b.emit(Instruction::F64ConvertI32U);
    b.emit(Instruction::LocalSet(val));
    emit_wrap_enum_1(&mut b, jv.enum_tag, jv.array_variant, val, cur);
  } else {
    b.emit(Instruction::LocalGet(list_buf));
    b.emit(Instruction::F64ConvertI32U);
  }
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::LocalSet(list_cap));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(list_cnt));
  b.emit(Instruction::I32Const(8 + 8 * 8));
  b.emit(Instruction::LocalSet(list_alloc_size));
  rt_emit_ensure_heap_capacity_const(&mut b, 8 + 8 + 8 * 8, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  rt_emit_alloc_dynamic(&mut b, list_alloc_size, list_buf, list_tag);

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(list_cnt));
  b.emit(Instruction::I32Const(4096));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::Call(self_idx));
  b.emit(Instruction::LocalSet(ok));
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalSet(val));
  b.emit(Instruction::LocalGet(ok));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Grow list buffer if list_cnt == list_cap
  b.emit(Instruction::LocalGet(list_cnt));
  b.emit(Instruction::LocalGet(list_cap));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(list_cap));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::LocalSet(list_cap));
  b.emit(Instruction::LocalGet(list_cap));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(list_alloc_size));
  b.emit(Instruction::LocalGet(list_alloc_size));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(req_bytes));
  rt_emit_ensure_heap_capacity(&mut b, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  let new_list_buf = b.alloc_i32();
  rt_emit_alloc_dynamic(&mut b, list_alloc_size, new_list_buf, list_tag);
  b.emit(Instruction::LocalGet(new_list_buf));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(list_buf));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(list_cnt));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.emit(Instruction::LocalGet(new_list_buf));
  b.emit(Instruction::LocalSet(list_buf));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(list_buf));
  b.emit(Instruction::LocalGet(list_cnt));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(list_cnt));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(list_cnt));

  skip_ws(&mut b, 0, cur, 2, byte);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b']' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  let arr_elem_kind = if json_value.is_some() {
    ContainerSlotKind::ExactHeapPtr
  } else {
    ContainerSlotKind::DynamicBoxed
  };
  b.emit(Instruction::LocalGet(list_buf));
  b.emit(Instruction::LocalGet(list_cnt));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(list_buf));
  b.emit(Instruction::I32Const(arr_elem_kind as i32));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  if let Some(jv) = json_value {
    b.emit(Instruction::LocalGet(list_buf));
    b.emit(Instruction::F64ConvertI32U);
    b.emit(Instruction::LocalSet(val));
    emit_wrap_enum_1(&mut b, jv.enum_tag, jv.array_variant, val, cur);
  } else {
    b.emit(Instruction::LocalGet(list_buf));
    b.emit(Instruction::F64ConvertI32U);
  }
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b',' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Object literal: {...}
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'{' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  skip_ws(&mut b, 0, cur, 2, byte);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Empty object `{}`
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(b'}' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  rt_emit_ensure_heap_capacity_const(&mut b, 24, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  rt_emit_alloc_const(&mut b, 16, map_ptr, map_tag);
  let empty_kv = ContainerSlotKind::pack_kv(ContainerSlotKind::EmptyUntyped, ContainerSlotKind::EmptyUntyped);
  b.emit(Instruction::LocalGet(map_ptr));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(map_ptr));
  b.emit(Instruction::I32Const(empty_kv as i32));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(map_ptr));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Store(mem_arg_i32(8)));
  b.emit(Instruction::LocalGet(map_ptr));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Store(mem_arg_i32(12)));
  if let Some(jv) = json_value {
    b.emit(Instruction::LocalGet(map_ptr));
    b.emit(Instruction::F64ConvertI32U);
    b.emit(Instruction::LocalSet(val));
    emit_wrap_enum_1(&mut b, jv.enum_tag, jv.object_variant, val, cur);
  } else {
    b.emit(Instruction::LocalGet(map_ptr));
    b.emit(Instruction::F64ConvertI32U);
  }
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::LocalSet(obj_cap));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(pair_cnt));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(member_cnt));
  b.emit(Instruction::I32Const(8 + 8 * 16));
  b.emit(Instruction::LocalSet(obj_alloc_size));
  rt_emit_ensure_heap_capacity_const(&mut b, 8 + 8 + 8 * 16, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  rt_emit_alloc_dynamic(&mut b, obj_alloc_size, obj_buf, map_tag);

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(member_cnt));
  b.emit(Instruction::I32Const(2048));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  skip_ws(&mut b, 0, cur, 2, byte);
  b.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.emit(Instruction::LocalSet(key_heap_mark));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::Call(parse_str_idx));
  b.emit(Instruction::LocalSet(ok));
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalSet(str_ptr));
  b.emit(Instruction::LocalGet(ok));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  if json_value.is_some() {
    // Keep the parsed JSON string directly as the Map<String, JsonValue> key.
    b.emit(Instruction::LocalGet(str_ptr));
    b.emit(Instruction::F64ConvertI32U);
    b.emit(Instruction::LocalSet(key_tag));
  } else {
    b.emit(Instruction::LocalGet(str_ptr));
    b.emit(Instruction::Call(tag_from_str_idx));
    b.emit(Instruction::LocalSet(key_tag));
    // Reclaim temporary key string now that tag_from_str has resolved it to a tag ID.
    b.emit(Instruction::LocalGet(str_ptr));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::I32Store(mem_arg_i32(0)));
    b.emit(Instruction::LocalGet(key_heap_mark));
    b.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
    b.emit(Instruction::LocalGet(key_tag));
    b.emit(f64_const(0.0));
    b.emit(Instruction::F64Eq);
    b.emit(Instruction::If(BlockType::Empty));
    fail_return(&mut b, cur);
    b.emit(Instruction::End);
  }

  skip_ws(&mut b, 0, cur, 2, byte);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(b':' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::Call(self_idx));
  b.emit(Instruction::LocalSet(ok));
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalSet(val));
  b.emit(Instruction::LocalGet(ok));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Update duplicate key in place inside the parser-owned flat buffer; append only new keys.
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(found_key));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(key_tag));
  if let Some(jv) = json_value {
    b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
    b.emit(Instruction::Call(jv.map_key_equal_idx));
  } else {
    b.emit(Instruction::F64Eq);
  }
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(found_key));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(found_key));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::LocalGet(obj_cap));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(obj_cap));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::LocalSet(obj_cap));
  b.emit(Instruction::LocalGet(obj_cap));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(obj_alloc_size));
  b.emit(Instruction::LocalGet(obj_alloc_size));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(req_bytes));
  rt_emit_ensure_heap_capacity(&mut b, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  let new_obj_buf = b.alloc_i32();
  rt_emit_alloc_dynamic(&mut b, obj_alloc_size, new_obj_buf, map_tag);
  b.emit(Instruction::LocalGet(new_obj_buf));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.emit(Instruction::LocalGet(new_obj_buf));
  b.emit(Instruction::LocalSet(obj_buf));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(key_tag));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(pair_cnt));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(member_cnt));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(member_cnt));

  skip_ws(&mut b, 0, cur, 2, byte);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'}' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  let obj_val_kind = if json_value.is_some() {
    ContainerSlotKind::ExactHeapPtr
  } else {
    ContainerSlotKind::DynamicBoxed
  };
  let obj_kv_kind = ContainerSlotKind::pack_kv(ContainerSlotKind::ExactHeapPtr, obj_val_kind);
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::I32Const(obj_kv_kind as i32));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::Call(map_from_flat_idx));
  b.emit(Instruction::LocalSet(map_ptr));
  b.emit(Instruction::LocalGet(map_ptr));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  if let Some(jv) = json_value {
    b.emit(Instruction::LocalGet(map_ptr));
    b.emit(Instruction::F64ConvertI32U);
    b.emit(Instruction::LocalSet(val));
    emit_wrap_enum_1(&mut b, jv.enum_tag, jv.object_variant, val, cur);
  } else {
    b.emit(Instruction::LocalGet(map_ptr));
    b.emit(Instruction::F64ConvertI32U);
  }
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b',' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Number literal: optional '-', integer digits (no leading zero except single '0'),
  // optional '.' + digits, optional ('e'|'E') + ['+'|'-'] + digits.
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalSet(num_start));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'-' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(first_digit));
  b.emit(Instruction::LocalGet(first_digit));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(first_digit));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(int_digits));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(int_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(int_digits));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Reject leading zero followed by more integer digits (e.g. `01`)
  b.emit(Instruction::LocalGet(first_digit));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(int_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Optional fractional part: '.' followed by at least one digit
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(int_digits));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(int_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(int_digits));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(int_digits));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Optional exponent part: ('e'|'E') ['+'|'-'] digits+
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(0x20));
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Const(b'e' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'+' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'-' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(int_digits));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(int_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(int_digits));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(int_digits));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Slice the number token into a heap string and parse via __rt_parse_float_valid / __rt_parse_float.
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(num_start));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(num_len));
  b.emit(Instruction::LocalGet(num_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(num_padded));
  b.emit(Instruction::LocalGet(num_padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(num_payload));
  b.emit(Instruction::LocalGet(num_payload));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(req_bytes));
  rt_emit_ensure_heap_capacity(&mut b, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.emit(Instruction::LocalSet(num_heap_mark));
  rt_emit_alloc_dynamic(&mut b, num_payload, num_str, string_tag);
  b.emit(Instruction::LocalGet(num_str));
  b.emit(Instruction::LocalGet(num_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(num_str));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(num_start));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(num_len));
  b.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  let reclaim_num_str = |b: &mut RuntimeFnBuilder| {
    b.emit(Instruction::LocalGet(num_str));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::I32Store(mem_arg_i32(0)));
    b.emit(Instruction::LocalGet(num_heap_mark));
    b.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  };

  b.emit(Instruction::LocalGet(num_str));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::Call(parse_float_valid_idx));
  b.emit(f64_const(0.0));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::If(BlockType::Empty));
  reclaim_num_str(&mut b);
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Guarantee `__rt_parse_float` can reserve its 1024-byte midpoint scratch
  // buffer without trapping when memory cannot grow.
  rt_emit_ensure_heap_capacity_const(&mut b, 1024, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  reclaim_num_str(&mut b);
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(num_str));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::Call(parse_float_idx));
  b.emit(Instruction::LocalSet(val));
  reclaim_num_str(&mut b);

  // Reject non-finite overflow (Infinity / NaN)
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::F64Sub);
  b.emit(f64_const(0.0));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  if let Some(jv) = json_value {
    emit_wrap_enum_1(&mut b, jv.enum_tag, jv.number_variant, val, cur);
  } else {
    // Box parsed JSON numbers with `number_tag` so integer values in the heap
    // range remain distinguishable from heap pointers under `type-of` and decoding.
    rt_emit_ensure_heap_capacity_const(&mut b, 16, req_bytes);
    b.emit(Instruction::I32Eqz);
    b.emit(Instruction::If(BlockType::Empty));
    fail_return(&mut b, cur);
    b.emit(Instruction::End);
    rt_emit_alloc_const(&mut b, 8, boxed_ptr, number_tag);
    b.emit(Instruction::LocalGet(boxed_ptr));
    b.emit(Instruction::LocalGet(val));
    b.emit(Instruction::F64Store(mem_arg_f64(0)));
    b.emit(Instruction::LocalGet(boxed_ptr));
    b.emit(Instruction::F64ConvertI32U);
  }
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.finish(vec![ValType::I32; 4], vec![ValType::F64, ValType::I32, ValType::I32])
}

/// `__rt_json_parse_root(s: f64) → (val: f64, ok: i32)`
pub(in crate::wasm::emit_wasm) fn build_rt_json_parse_root(parse_at_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let src = b.alloc_i32();
  let len = b.alloc_i32();
  let heap_mark = b.alloc_i32();
  let cur = b.alloc_i32();
  let ok = b.alloc_i32();
  let byte = b.alloc_i32();
  let val = b.alloc_f64();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(src));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(len));

  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32Const(edn::MAX_EDN_OUTPUT_BYTES));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(f64_const(0.0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.emit(Instruction::LocalSet(heap_mark));

  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Call(parse_at_idx));
  b.emit(Instruction::LocalSet(ok));
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalSet(val));

  b.emit(Instruction::LocalGet(ok));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(heap_mark));
  b.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  b.emit(f64_const(0.0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  // Skip trailing whitespace and ensure we reached `len`.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  emit_is_ws_check(&mut b.instructions, byte);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(heap_mark));
  b.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  b.emit(f64_const(0.0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::I32Const(1));
  b.finish(vec![ValType::F64], vec![ValType::F64, ValType::I32])
}
