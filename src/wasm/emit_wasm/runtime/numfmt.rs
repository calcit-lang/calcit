//! Number-to-string, radix formatting, and Eisel-Lemire + exact float parsing helpers.

use super::super::*;
use super::*;
use wasm_encoder::BlockType;

/// `__rt_f64_to_str(value: f64) → f64` (logical string pointer)
/// Converts a number to a heap-allocated string.
/// Handles every finite f64 and the language's special values.
pub(in crate::wasm::emit_wasm) fn emit_f64_const_text(b: &mut Vec<Instruction<'static>>, text: &str, string_tag: i32) {
  let padded_len = (text.len() + 7) & !7;
  b.extend([
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::LocalTee(11),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(11),
    Instruction::I32Const(4),
    Instruction::I32Add,
    Instruction::I32Const(string_tag),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(11),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalTee(7),
    Instruction::F64Const(Ieee64::from(text.len() as f64)),
    Instruction::F64Store(mem_arg_f64(0)),
  ]);
  for (offset, byte) in text.bytes().enumerate() {
    b.extend([
      Instruction::LocalGet(7),
      Instruction::I32Const(8 + offset as i32),
      Instruction::I32Add,
      Instruction::I32Const(byte as i32),
      Instruction::I32Store8(mem_arg_byte(0)),
    ]);
  }
  b.extend([
    Instruction::LocalGet(11),
    Instruction::I32Const((16 + padded_len) as i32),
    Instruction::I32Add,
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::LocalGet(7),
    Instruction::F64ConvertI32U,
    Instruction::Return,
  ]);
}

pub(in crate::wasm::emit_wasm) fn build_rt_f64_to_str(string_tag: i32, ryu_format_idx: u32) -> CompiledFn {
  // param 0: value (f64)
  // locals: 1=raw_i64(i64), 2=neg(i32), 3=abs_i64(i64), 4=ndigits(i32),
  //          5=tmp_i64(i64), 6=payload(i32), 7=str_ptr(i32), 8=content(i32),
  //          9=pos(i32), 10=digit(i32), 11=raw_base(i32)
  let mut b = Vec::new();

  // Preserve native spelling for non-finite values and signed zero.
  b.extend([
    Instruction::LocalGet(0),
    Instruction::LocalGet(0),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
  ]);
  emit_f64_const_text(&mut b, "NaN", string_tag);
  b.push(Instruction::End);
  for (value, text) in [(f64::INFINITY, "inf"), (f64::NEG_INFINITY, "-inf")] {
    b.extend([
      Instruction::LocalGet(0),
      Instruction::F64Const(Ieee64::from(value)),
      Instruction::F64Eq,
      Instruction::If(BlockType::Empty),
    ]);
    emit_f64_const_text(&mut b, text, string_tag);
    b.push(Instruction::End);
  }
  b.extend([
    Instruction::LocalGet(0),
    Instruction::I64ReinterpretF64,
    Instruction::I64Const(i64::MIN),
    Instruction::I64Eq,
    Instruction::If(BlockType::Empty),
  ]);
  emit_f64_const_text(&mut b, "-0", string_tag);
  b.extend([Instruction::End, Instruction::LocalGet(0)]);

  // Preserve the cheap integer path; all other finite values use the pinned
  // shortest-decimal formatter and the same canonical expansion as native.
  b.push(Instruction::F64Floor);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Eq);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(-9007199254740992.0f64)));
  b.push(Instruction::F64Ge);
  b.push(Instruction::I32And);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(9007199254740992.0f64)));
  b.push(Instruction::F64Le);
  b.push(Instruction::I32And);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I64ReinterpretF64);
  b.push(Instruction::I64Const(i64::MIN));
  b.push(Instruction::I64Ne);
  b.push(Instruction::I32And);
  b.push(Instruction::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::F64)));
  // --- integer branch ---

  // neg = value < 0.0 ? 1 : 0
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::F64Lt);
  b.push(Instruction::LocalSet(2)); // neg

  // raw_i64 = i64.trunc_f64_s(value)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I64TruncF64S);
  b.push(Instruction::LocalSet(1)); // raw_i64

  // abs_i64 = neg ? -raw_i64 : raw_i64
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::I64Const(0));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I64Sub);
  b.push(Instruction::LocalSet(3));
  b.push(Instruction::Else);
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::LocalSet(3));
  b.push(Instruction::End);

  // Count digits: ndigits = 0; tmp = abs_i64; do { ndigits++; tmp /= 10; } while tmp != 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(4)); // ndigits = 0
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalSet(5)); // tmp = abs_i64

  b.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  // ndigits++
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  // tmp /= 10
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Const(10));
  b.push(Instruction::I64DivU);
  b.push(Instruction::LocalSet(5));
  // if tmp == 0: break
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // total_len = ndigits + (neg ? 1 : 0)
  // payload = 8 (byte_len f64) + total_len rounded up to 8 bytes
  // Compute total_len
  b.push(Instruction::LocalGet(4)); // ndigits
  b.push(Instruction::LocalGet(2)); // neg flag (0 or 1)
  b.push(Instruction::I32Add); // total_len

  // payload = total_len (round up to 8): (total_len + 7) & ~7
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add); // payload = 8 + padded_total_len
  b.push(Instruction::LocalSet(6)); // payload

  // Allocate string: raw_base = HEAP_PTR_GLOBAL
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(11)); // raw_base
  // Write HEAP_MAGIC at raw_base
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // Write string_tag at raw_base+4
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(string_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // str_ptr = raw_base + 8
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(7)); // str_ptr (logical)
  // content = str_ptr + 8 (after byte_len slot)
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8)); // content base
  // Advance heap_ptr by payload
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // Write byte_len = total_len (ndigits + neg) at str_ptr+0
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::LocalGet(4)); // ndigits
  b.push(Instruction::LocalGet(2)); // neg
  b.push(Instruction::I32Add); // total_len
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));

  // Write '-' at content[0] if neg
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(8)); // content addr
  b.push(Instruction::I32Const(b'-' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::End);

  // Write digits right-to-left, pos starts at (neg + ndigits - 1)
  // pos = neg + ndigits - 1
  b.push(Instruction::LocalGet(2)); // neg
  b.push(Instruction::LocalGet(4)); // ndigits
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(9)); // pos

  // tmp = abs_i64 again
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalSet(5));

  // Loop: write digits from least significant to most significant (right to left)
  b.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  // digit = tmp % 10
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Const(10));
  b.push(Instruction::I64RemU);
  b.push(Instruction::I32WrapI64);
  b.push(Instruction::LocalSet(10)); // digit
  // content[pos] = '0' + digit
  b.push(Instruction::LocalGet(8)); // content base
  b.push(Instruction::LocalGet(9)); // pos
  b.push(Instruction::I32Add); // content + pos
  b.push(Instruction::LocalGet(10)); // digit
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  // pos--
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(9));
  // tmp /= 10
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Const(10));
  b.push(Instruction::I64DivU);
  b.push(Instruction::LocalSet(5));
  // if tmp == 0: break
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // return str_ptr as f64 — leave on stack
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::Else);
  // The helper writes into reserved scratch memory, outside the heap. The
  // longest expanded f64 uses 327 bytes; 400 is the helper's ABI bound.
  b.extend([
    Instruction::LocalGet(0),
    Instruction::I32Const(number_format::NUMBER_SCRATCH),
    Instruction::Call(ryu_format_idx),
    Instruction::LocalTee(12),
    Instruction::I32Const(400),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    // payload = 8-byte length slot + padded UTF-8 bytes.
    Instruction::LocalGet(12),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::I32Const(-8),
    Instruction::I32And,
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(13),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::LocalTee(11),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(11),
    Instruction::I32Const(4),
    Instruction::I32Add,
    Instruction::I32Const(string_tag),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(11),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalTee(7),
    Instruction::LocalGet(12),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(7),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32Const(number_format::NUMBER_SCRATCH),
    Instruction::LocalGet(12),
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    Instruction::LocalGet(11),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(13),
    Instruction::I32Add,
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::LocalGet(7),
    Instruction::F64ConvertI32U,
  ]);

  b.push(Instruction::End);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I64, // raw_i64 (1)
      ValType::I32, // neg (2)
      ValType::I64, // abs_i64 (3)
      ValType::I32, // ndigits (4)
      ValType::I64, // tmp_i64 (5)
      ValType::I32, // payload (6)
      ValType::I32, // str_ptr (7)
      ValType::I32, // content (8)
      ValType::I32, // pos (9)
      ValType::I32, // digit (10)
      ValType::I32, // raw_base (11)
      ValType::I32, // canonical_len (12)
      ValType::I32, // ryu_payload (13)
    ],
    instructions: b,
  }
}

/// `__rt_display_by(value: f64, radix: f64) -> f64`
///
/// Converts `value` (a non-negative safe integer) to a string in radix 2, 8 or 16,
/// prefixed with "0b", "0o" or "0x". Other values or radixes trap.
#[allow(clippy::vec_init_then_push)]
pub(in crate::wasm::emit_wasm) fn build_rt_display_by(string_tag: i32) -> CompiledFn {
  // params: 0=value(f64), 1=radix(f64)
  // locals (allocated manually to match flat vec):
  //   2=radix_i64(i64), 3=is_neg(i32), 4=abs_i64(i64), 5=tmp_i64(i64),
  //   6=ndigits(i32), 7=prefix_len(i32), 8=total_len(i32), 9=padded(i32),
  //   10=str_ptr(i32), 11=content(i32), 12=digit_pos(i32), 13=digit(i32),
  //   14=raw_base(i32), 15=ch(i32), 16=payload(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // Domain (docs/data/number.md): value is a non-negative safe integer and radix is 2, 8 or 16;
  // anything else traps where native and JS raise an error.
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Trunc);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Ne); // fractional or NaN
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::F64Lt);
  b.push(Instruction::I32Or);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(9_007_199_254_740_991.0f64)));
  b.push(Instruction::F64Gt);
  b.push(Instruction::I32Or);
  for radix in [2.0f64, 8.0, 16.0] {
    b.push(Instruction::LocalGet(1));
    b.push(Instruction::F64Const(Ieee64::from(radix)));
    b.push(Instruction::F64Ne);
  }
  b.push(Instruction::I32And);
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Unreachable);
  b.push(Instruction::End);

  // radix_i64 = i64.trunc_f64_s(radix)
  b.push(Instruction::LocalGet(1)); // radix f64
  b.push(Instruction::I64TruncF64S);
  b.push(Instruction::LocalSet(2)); // radix_i64

  // is_neg = value < 0.0 ? 1 : 0
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::F64Lt);
  b.push(Instruction::LocalSet(3)); // is_neg

  // abs_i64 = trunc(value) in i64, then abs
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I64TruncF64S);
  b.push(Instruction::LocalSet(5)); // tmp = signed i64(value)
  // abs: if is_neg, negate
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::I64Const(0));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Sub);
  b.push(Instruction::LocalSet(4)); // abs_i64
  b.push(Instruction::Else);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);

  // Count ndigits: loop { ndigits++; tmp /= radix; } while tmp != 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(6)); // ndigits = 0
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalSet(5)); // tmp = abs_i64

  b.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(6)); // ndigits++
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(2)); // radix_i64
  b.push(Instruction::I64DivU);
  b.push(Instruction::LocalSet(5)); // tmp /= radix
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Eqz);
  b.push(Instruction::BrIf(1)); // break if tmp == 0
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // prefix_len: 2 if radix ∈ {2,8,16}, else 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(7)); // prefix_len = 0
  // if radix == 2 || radix == 8 || radix == 16
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(2.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(8.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(16.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::LocalSet(7)); // prefix_len = 2
  b.push(Instruction::End);

  // total_len = is_neg + prefix_len + ndigits
  b.push(Instruction::LocalGet(3)); // is_neg
  b.push(Instruction::LocalGet(7)); // prefix_len
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(6)); // ndigits
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8)); // total_len

  // padded = (total_len + 7) & ~7
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(9)); // padded

  // payload = 8 (byte_len f64) + padded
  b.push(Instruction::I32Const(8));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(16)); // payload

  // Allocate string: raw_base = HEAP_PTR_GLOBAL
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(14)); // raw_base
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(string_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // str_ptr = raw_base + 8
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(10)); // str_ptr (logical)
  // content = str_ptr + 8
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(11)); // content
  // Advance heap_ptr: raw_base + 8 + payload
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // Write byte_len at str_ptr+0
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::LocalGet(8)); // total_len
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));

  // Write '-' if is_neg (content[0] = '-')
  b.push(Instruction::LocalGet(3)); // is_neg
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(b'-' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::End);

  // Write prefix bytes: current byte offset = is_neg (0 or 1)
  // pos = is_neg  (i32 local 12 reused as write-cursor for prefix/digit region)
  b.push(Instruction::LocalGet(3)); // is_neg as starting byte offset
  b.push(Instruction::LocalSet(12)); // digit_pos = is_neg (cursor)

  // if radix == 2: content[cursor]='0', content[cursor+1]='b'; cursor+=2
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(2.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'b' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::End);

  // if radix == 8: prefix "0o"
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(8.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'o' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::End);

  // if radix == 16: prefix "0x"
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(16.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'x' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::End);

  // Now write digits right-to-left starting at content[cursor + ndigits - 1]
  // digit_pos = cursor + ndigits - 1
  b.push(Instruction::LocalGet(12)); // cursor (after prefix)
  b.push(Instruction::LocalGet(6)); // ndigits
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(12)); // digit_pos (rightmost digit byte offset)

  // tmp = abs_i64
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalSet(5));

  // Loop: write digits right to left
  b.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  // digit = i32(tmp % radix_i64)
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(2)); // radix_i64
  b.push(Instruction::I64RemU);
  b.push(Instruction::I32WrapI64);
  b.push(Instruction::LocalSet(13)); // digit
  // ch = digit < 10 ? '0' + digit : 'a' + digit - 10
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::I32Const(10));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(15)); // ch = '0' + digit
  b.push(Instruction::Else);
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::I32Const(b'a' as i32 - 10));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(15)); // ch = 'a' + digit - 10
  b.push(Instruction::End);
  // content[digit_pos] = ch
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  // digit_pos--
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(12));
  // tmp /= radix
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I64DivU);
  b.push(Instruction::LocalSet(5));
  // break if tmp == 0
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // return str_ptr as f64
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I64, // radix_i64 (2)
      ValType::I32, // is_neg (3)
      ValType::I64, // abs_i64 (4)
      ValType::I64, // tmp_i64 (5)
      ValType::I32, // ndigits (6)
      ValType::I32, // prefix_len (7)
      ValType::I32, // total_len (8)
      ValType::I32, // padded (9)
      ValType::I32, // str_ptr (10)
      ValType::I32, // content (11)
      ValType::I32, // digit_pos / cursor (12)
      ValType::I32, // digit (13)
      ValType::I32, // raw_base (14)
      ValType::I32, // ch (15)
      ValType::I32, // payload (16)
    ],
    instructions: b,
  }
}

#[derive(Clone, Copy)]
pub(in crate::wasm::emit_wasm) struct ParseFloatLocals {
  hi: u32,
  lo: u32,
  tz_count: u32,
  sig_digits: u32,
  sticky: u32,
  exp10: u32,
  exp2: u32,
  digit: u32,
  t_lo: u32,
  t_hi: u32,
  t2_lo: u32,
  t2_hi: u32,
  rem: u32,
  shift: u32,
}

pub(in crate::wasm::emit_wasm) fn rt_emit_u128_mul10_add(b: &mut RuntimeFnBuilder, locs: &ParseFloatLocals, add_digit: Option<u32>) {
  // Compute `(hi, lo) * 8` and `(hi, lo) * 2` without overflow (`sig_digits < 38`).
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(3));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalSet(locs.t_lo));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(3));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(61));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.t_hi));

  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalSet(locs.t2_lo));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(63));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.t2_hi));

  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::LocalGet(locs.t2_lo));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(locs.lo));

  // Add optional digit `add_digit` and accumulate both 64-bit carries into `hi`.
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::LocalGet(locs.t2_hi));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::I64LtU);
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Add);
  if let Some(digit) = add_digit {
    b.emit(Instruction::LocalSet(locs.hi));
    b.emit(Instruction::LocalGet(locs.lo));
    b.emit(Instruction::LocalSet(locs.t_lo));
    b.emit(Instruction::LocalGet(locs.t_lo));
    b.emit(Instruction::LocalGet(digit));
    b.emit(Instruction::I64ExtendI32U);
    b.emit(Instruction::I64Add);
    b.emit(Instruction::LocalSet(locs.lo));
    b.emit(Instruction::LocalGet(locs.hi));
    b.emit(Instruction::LocalGet(locs.lo));
    b.emit(Instruction::LocalGet(locs.t_lo));
    b.emit(Instruction::I64LtU);
    b.emit(Instruction::I64ExtendI32U);
    b.emit(Instruction::I64Add);
  }
  b.emit(Instruction::LocalSet(locs.hi));
}

pub(in crate::wasm::emit_wasm) fn rt_emit_u128_decimal_digit(b: &mut RuntimeFnBuilder, locs: &ParseFloatLocals, in_frac: bool) {
  b.emit(Instruction::LocalGet(locs.sig_digits));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(locs.digit));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  if in_frac {
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(-10_000_000));
    b.emit(Instruction::I32GtS);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::LocalSet(locs.exp10));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(locs.digit));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.tz_count));
  b.emit(Instruction::I32Const(10_000_000));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.tz_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.tz_count));
  b.emit(Instruction::End);
  if !in_frac {
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(10_000_000));
    b.emit(Instruction::I32LtS);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(locs.exp10));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::Else);
  // Flush pending zeros into the 128-bit significand before appending this non-zero digit.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.tz_count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LeS);
  b.emit(Instruction::LocalGet(locs.sig_digits));
  b.emit(Instruction::I32Const(38));
  b.emit(Instruction::I32GeS);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  rt_emit_u128_mul10_add(b, locs, None);
  b.emit(Instruction::LocalGet(locs.sig_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.sig_digits));
  b.emit(Instruction::LocalGet(locs.tz_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.tz_count));
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Const(-10_000_000));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.exp10));
  b.emit(Instruction::End);
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.tz_count));

  b.emit(Instruction::LocalGet(locs.sig_digits));
  b.emit(Instruction::I32Const(38));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  rt_emit_u128_mul10_add(b, locs, Some(locs.digit));
  b.emit(Instruction::LocalGet(locs.sig_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.sig_digits));
  if in_frac {
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(-10_000_000));
    b.emit(Instruction::I32GtS);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::LocalSet(locs.exp10));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(locs.sticky));
  if !in_frac {
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(10_000_000));
    b.emit(Instruction::I32LtS);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(locs.exp10));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
}

pub(in crate::wasm::emit_wasm) fn rt_emit_u128_normalize(b: &mut RuntimeFnBuilder, locs: &ParseFloatLocals) {
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::I32Const(64));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.exp2));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Clz);
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::LocalTee(locs.shift));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.shift));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::LocalSet(locs.t_hi)); // shift as u64 (1..=63)
  b.emit(Instruction::I64Const(64));
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::I64Sub);
  b.emit(Instruction::LocalSet(locs.t_lo)); // 64 - shift (1..=63)
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::LocalGet(locs.shift));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.exp2));
  b.emit(Instruction::End);
}

pub(in crate::wasm::emit_wasm) fn rt_emit_u128_mul_by_5_step(b: &mut RuntimeFnBuilder, locs: &ParseFloatLocals) {
  // Ensure `(hi, lo) * 5 < 2^128` (`u128::MAX / 5 = 0x3333...`).
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(0x3333_3333_3333_3333u64 as i64));
  b.emit(Instruction::I64LtU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(locs.sticky));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(63));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.exp2));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Multiply `(hi, lo)` by 5 (`((hi, lo) << 2) + (hi, lo)`).
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(2));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalSet(locs.t_lo));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(2));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(62));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.t_hi));

  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(locs.t2_lo));
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalGet(locs.t2_lo));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64LtU);
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t2_lo));
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.exp2));
}

pub(in crate::wasm::emit_wasm) fn rt_emit_u128_div_by_5_step(b: &mut RuntimeFnBuilder, locs: &ParseFloatLocals) {
  // Normalize `(hi, lo)` first so division retains full 128-bit precision.
  rt_emit_u128_normalize(b, locs);

  // Divide 128-bit `(hi, lo)` by 5 via 64/32-bit chunks: `(q_hi, q_lo)` and `rem` (0..=4).
  // Step 1: `hi / 5`.
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64RemU);
  b.emit(Instruction::LocalSet(locs.t2_hi)); // r0 in 0..=4
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64DivU);
  b.emit(Instruction::LocalSet(locs.hi)); // q_hi

  // Step 2: `((r0 << 32) | (lo >> 32)) / 5`.
  b.emit(Instruction::LocalGet(locs.t2_hi));
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalTee(locs.t_hi)); // num1
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64RemU);
  b.emit(Instruction::LocalSet(locs.t2_lo)); // r1 in 0..=4
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64DivU);
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalSet(locs.t_hi)); // q_mid << 32

  // Step 3: `((r1 << 32) | (lo & 0xFFFF_FFFF)) / 5`.
  b.emit(Instruction::LocalGet(locs.t2_lo));
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(0xFFFF_FFFFi64));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalTee(locs.t_lo)); // num2
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64RemU);
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::LocalSet(locs.rem));
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64DivU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.lo));

  // Factor of 1/2 from dividing by 10 = 5 * 2.
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.exp2));

  // Shift exact fractional bits from `rem / 5` while restoring bit 63 of `hi`.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64LtS);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(locs.rem));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::LocalSet(locs.rem));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.digit));
  b.emit(Instruction::LocalGet(locs.rem));
  b.emit(Instruction::I32Const(5));
  b.emit(Instruction::I32GeS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.rem));
  b.emit(Instruction::I32Const(5));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.rem));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(locs.digit));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(63));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.digit));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.exp2));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(locs.rem));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(locs.sticky));
  b.emit(Instruction::End);
}

pub(in crate::wasm::emit_wasm) fn rt_emit_u128_round_to_f64(b: &mut RuntimeFnBuilder, locs: &ParseFloatLocals, result: u32) {
  let mant = locs.t_hi;
  let extra_nonzero = locs.digit;
  let round_up = locs.rem;
  let exp2_cur = b.alloc_i32();

  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::LocalSet(exp2_cur));

  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(1023));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(f64::INFINITY)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(-1022));
  b.emit(Instruction::I32GeS);
  b.emit(Instruction::If(BlockType::Empty));
  // Normal IEEE 754 binary64: round-to-nearest-even on top 53 bits of normalized `hi`.
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(11));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalSet(mant));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::LocalGet(locs.sticky));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalSet(extra_nonzero));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(0x7FF));
  b.emit(Instruction::I64And);
  b.emit(Instruction::LocalTee(locs.t_lo));
  b.emit(Instruction::I64Const(0x400));
  b.emit(Instruction::I64GtU);
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::I64Const(0x400));
  b.emit(Instruction::I64Eq);
  b.emit(Instruction::LocalGet(extra_nonzero));
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(mant));
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(53));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalSet(mant));
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(exp2_cur));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(1023));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(f64::INFINITY)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(1023));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Const(52));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(0x000F_FFFF_FFFF_FFFFi64));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::F64ReinterpretI64);
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(-1074));
  b.emit(Instruction::I32GeS);
  b.emit(Instruction::If(BlockType::Empty));
  // Subnormal IEEE 754 binary64: `total_shift` is strictly in 12..=63.
  b.emit(Instruction::I32Const(-1011));
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::LocalTee(locs.t2_hi)); // total_shift in 12..=63
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Sub);
  b.emit(Instruction::LocalSet(locs.t2_lo)); // half_pos in 11..=62

  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t2_hi));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalSet(mant));

  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::LocalGet(locs.t2_lo));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Sub);
  b.emit(Instruction::LocalSet(locs.t_lo)); // mask for bits below half

  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(locs.sticky));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalSet(extra_nonzero));

  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t2_lo));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::LocalGet(extra_nonzero));
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(round_up));
  b.emit(Instruction::LocalGet(round_up));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(mant));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::F64ReinterpretI64);
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(-1075));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  // Halfway to smallest subnormal `2^-1074`: round to even (0) only on exact tie.
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(0x7FFF_FFFF_FFFF_FFFFi64));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(locs.sticky));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::F64ReinterpretI64);
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
}

/// `__rt_bigint_mul_add(base_ptr: i32, len: i32, mul: i64, add: i64) -> i32`
///
/// Multiplies a little-endian `u32` limb slice in place by `mul` and adds `add`,
/// returning the updated limb length. Capacity is 128 limbs (4096 bits).
pub(in crate::wasm::emit_wasm) fn build_rt_bigint_mul_add() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(4);
  let base_ptr = 0;
  let len = 1;
  let mul = 2;
  let add = 3;
  let carry = b.alloc_i64();
  let idx = b.alloc_i32();
  let addr = b.alloc_i32();
  let prod = b.alloc_i64();

  b.emit(Instruction::LocalGet(add));
  b.emit(Instruction::LocalSet(carry));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(base_ptr));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(addr));
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::LocalGet(mul));
  b.emit(Instruction::I64Mul);
  b.emit(Instruction::LocalGet(carry));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalTee(prod));
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(prod));
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalSet(carry));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(carry));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32Const(128));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(base_ptr));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(carry));
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(len));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(len));
  b.finish(vec![ValType::I32, ValType::I32, ValType::I64, ValType::I64], vec![ValType::I32])
}

pub(in crate::wasm::emit_wasm) fn rt_emit_scale_bigint_pow5(
  b: &mut RuntimeFnBuilder,
  bigint_mul_add_idx: u32,
  buf_ptr: u32,
  buf_len: u32,
  count: u32,
) {
  let rem = b.alloc_i32();
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::LocalSet(rem));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(13));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(buf_ptr));
  b.emit(Instruction::LocalGet(buf_len));
  b.emit(Instruction::I64Const(1_220_703_125i64));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(buf_len));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(13));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(rem));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LeS);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(buf_ptr));
  b.emit(Instruction::LocalGet(buf_len));
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(buf_len));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(rem));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
}

pub(in crate::wasm::emit_wasm) fn rt_emit_scale_bigint_pow2(
  b: &mut RuntimeFnBuilder,
  bigint_mul_add_idx: u32,
  buf_ptr: u32,
  buf_len: u32,
  count: u32,
) {
  let rem = b.alloc_i32();
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::LocalSet(rem));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(buf_ptr));
  b.emit(Instruction::LocalGet(buf_len));
  b.emit(Instruction::I64Const(1i64 << 24));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(buf_len));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(rem));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(buf_ptr));
  b.emit(Instruction::LocalGet(buf_len));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(buf_len));
  b.emit(Instruction::End);
}

/// `__rt_parse_float_exact_midpoint(content: i32, byte_len: i32, start_i: i32, explicit_exp: i32, bits_low: i64) -> f64`
///
/// Compares the full decimal mantissa (up to 768 significant digits plus sticky)
/// against the exact binary64 midpoint between `bits_low` and `bits_low + 1`.
pub(in crate::wasm::emit_wasm) fn build_rt_parse_float_exact_midpoint(bigint_mul_add_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(5);
  let content = 0;
  let byte_len = 1;
  let start_i = 2;
  let explicit_exp = 3;
  let bits_low = 4;

  let scratch_size = b.alloc_i64();
  let managed_zero = b.alloc_i64();
  let lhs_ptr = b.alloc_i32();
  let rhs_ptr = b.alloc_i32();
  let lhs_len = b.alloc_i32();
  let rhs_len = b.alloc_i32();
  let i = b.alloc_i32();
  let ch = b.alloc_i32();
  let in_frac = b.alloc_i32();
  let sig_digits = b.alloc_i32();
  let tz_count = b.alloc_i32();
  let e_d = b.alloc_i32();
  let extra_nonzero = b.alloc_i32();
  let biased_exp = b.alloc_i32();
  let p2 = b.alloc_i32();
  let frac = b.alloc_i64();
  let m2 = b.alloc_i64();
  let m2_hi = b.alloc_i32();
  let scale_steps = b.alloc_i32();
  let shift2 = b.alloc_i32();
  let cmp = b.alloc_i32();
  let idx = b.alloc_i32();
  let off = b.alloc_i32();
  let lv = b.alloc_i32();
  let rv = b.alloc_i32();

  b.emit(Instruction::I64Const(1024));
  b.emit(Instruction::LocalSet(scratch_size));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::LocalSet(managed_zero));
  rt_emit_reserve_scratch(&mut b, scratch_size, managed_zero, lhs_ptr);
  b.emit(Instruction::LocalGet(lhs_ptr));
  b.emit(Instruction::I32Const(512));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(rhs_ptr));

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(lhs_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(rhs_len));
  b.emit(Instruction::LocalGet(start_i));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(in_frac));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(sig_digits));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(tz_count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(extra_nonzero));

  // Scan up to 768 significant digits of the decimal mantissa into `lhs`.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalTee(ch));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(in_frac));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::BrIf(2));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(in_frac));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(2));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(ch)); // digit 0..=9
  b.emit(Instruction::LocalGet(sig_digits));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(in_frac));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(-10_000_000));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(tz_count));
  b.emit(Instruction::I32Const(10_000_000));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(tz_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(tz_count));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(in_frac));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(10_000_000));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  // Flush pending zeros while within 768 significant digits.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(tz_count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LeS);
  b.emit(Instruction::LocalGet(sig_digits));
  b.emit(Instruction::I32Const(768));
  b.emit(Instruction::I32GeS);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(lhs_ptr));
  b.emit(Instruction::LocalGet(lhs_len));
  b.emit(Instruction::I64Const(10));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(lhs_len));
  b.emit(Instruction::LocalGet(sig_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(sig_digits));
  b.emit(Instruction::LocalGet(tz_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(tz_count));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(-10_000_000));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::End);
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(tz_count));

  b.emit(Instruction::LocalGet(sig_digits));
  b.emit(Instruction::I32Const(768));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(lhs_ptr));
  b.emit(Instruction::LocalGet(lhs_len));
  b.emit(Instruction::I64Const(10));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(lhs_len));
  b.emit(Instruction::LocalGet(sig_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(sig_digits));
  b.emit(Instruction::LocalGet(in_frac));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(-10_000_000));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(extra_nonzero));
  b.emit(Instruction::LocalGet(in_frac));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(10_000_000));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(extra_nonzero));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(lhs_ptr));
  b.emit(Instruction::LocalGet(lhs_len));
  b.emit(Instruction::I64Const(10));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(lhs_len));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::LocalGet(explicit_exp));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(e_d));

  // Build exact binary64 midpoint `m2 * 2^p2` for `(bits_low, bits_low + 1)` into `rhs`.
  b.emit(Instruction::LocalGet(bits_low));
  b.emit(Instruction::I64Const(52));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Const(0x7FF));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::LocalSet(biased_exp));

  b.emit(Instruction::LocalGet(bits_low));
  b.emit(Instruction::I64Const(0x000F_FFFF_FFFF_FFFFi64));
  b.emit(Instruction::I64And);
  b.emit(Instruction::LocalSet(frac));

  b.emit(Instruction::LocalGet(biased_exp));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(frac));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(m2));
  b.emit(Instruction::I32Const(-1075));
  b.emit(Instruction::LocalSet(p2));
  b.emit(Instruction::Else);
  b.emit(Instruction::I64Const(1i64 << 52));
  b.emit(Instruction::LocalGet(frac));
  b.emit(Instruction::I64Or);
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(m2));
  b.emit(Instruction::LocalGet(biased_exp));
  b.emit(Instruction::I32Const(1076));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(p2));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(rhs_ptr));
  b.emit(Instruction::LocalGet(m2));
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(m2));
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::LocalTee(m2_hi));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(rhs_ptr));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(m2_hi));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::LocalSet(rhs_len));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(rhs_len));
  b.emit(Instruction::End);

  // Align powers of 5: multiply `lhs` by 5^e_d when `e_d > 0`, or `rhs` by 5^(-e_d) when `e_d < 0`.
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::LocalSet(scale_steps));
  rt_emit_scale_bigint_pow5(&mut b, bigint_mul_add_idx, lhs_ptr, lhs_len, scale_steps);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(scale_steps));
  rt_emit_scale_bigint_pow5(&mut b, bigint_mul_add_idx, rhs_ptr, rhs_len, scale_steps);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Align powers of 2: `shift2 = e_d - p2`.
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::LocalGet(p2));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalTee(shift2));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(shift2));
  b.emit(Instruction::LocalSet(scale_steps));
  rt_emit_scale_bigint_pow2(&mut b, bigint_mul_add_idx, lhs_ptr, lhs_len, scale_steps);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(shift2));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(shift2));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(scale_steps));
  rt_emit_scale_bigint_pow2(&mut b, bigint_mul_add_idx, rhs_ptr, rhs_len, scale_steps);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Compare unsigned big integers `lhs` and `rhs`.
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(cmp));
  b.emit(Instruction::LocalGet(lhs_len));
  b.emit(Instruction::LocalGet(rhs_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(cmp));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(lhs_len));
  b.emit(Instruction::LocalGet(rhs_len));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(cmp));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(lhs_len));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::LocalSet(off));
  b.emit(Instruction::LocalGet(lhs_ptr));
  b.emit(Instruction::LocalGet(off));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(lv));
  b.emit(Instruction::LocalGet(rhs_ptr));
  b.emit(Instruction::LocalGet(off));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(rv));
  b.emit(Instruction::LocalGet(lv));
  b.emit(Instruction::LocalGet(rv));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalGet(lv));
  b.emit(Instruction::LocalGet(rv));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::Select);
  b.emit(Instruction::LocalSet(cmp));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::End);
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Round up iff `lhs > mid` or exact tie with odd `bits_low`.
  b.emit(Instruction::LocalGet(cmp));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::LocalGet(cmp));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(bits_low));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Result(ValType::F64)));
  b.emit(Instruction::LocalGet(bits_low));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::F64ReinterpretI64);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(bits_low));
  b.emit(Instruction::F64ReinterpretI64);
  b.emit(Instruction::End);
  b.finish(
    vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I64],
    vec![ValType::F64],
  )
}

/// `__rt_parse_float(s: f64) → f64`
///
/// Parses decimal strings to correctly rounded f64 values using 128-bit integer
/// decimal-to-binary scaling with an exact big-integer midpoint fallback.
pub(in crate::wasm::emit_wasm) fn build_rt_parse_float(parse_float_exact_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1); // 0=s(f64)
  let ptr = b.alloc_i32();
  let byte_len = b.alloc_i32();
  let content = b.alloc_i32();
  let i = b.alloc_i32();
  let num_start_i = b.alloc_i32();
  let is_neg = b.alloc_i32();
  let ch = b.alloc_i32();
  let result = b.alloc_f64();
  let result_hi = b.alloc_f64();
  let exp_val = b.alloc_i32();
  let exp_neg = b.alloc_i32();
  let explicit_exp = b.alloc_i32();
  let is_special = b.alloc_i32();
  let scale_steps = b.alloc_i32();
  let locs = ParseFloatLocals {
    hi: b.alloc_i64(),
    lo: b.alloc_i64(),
    tz_count: b.alloc_i32(),
    sig_digits: b.alloc_i32(),
    sticky: b.alloc_i32(),
    exp10: b.alloc_i32(),
    exp2: b.alloc_i32(),
    digit: b.alloc_i32(),
    t_lo: b.alloc_i64(),
    t_hi: b.alloc_i64(),
    t2_lo: b.alloc_i64(),
    t2_hi: b.alloc_i64(),
    rem: b.alloc_i32(),
    shift: b.alloc_i32(),
  };

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(ptr));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(byte_len));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(content));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));

  // Skip leading whitespace.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(ch));
  emit_is_ws_check(&mut b.instructions, ch);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Optional sign.
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(is_neg));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(ch));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'-' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(is_neg));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'+' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalSet(num_start_i));
  b.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.tz_count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.sig_digits));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.sticky));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.exp10));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(explicit_exp));
  b.emit(Instruction::I32Const(127));
  b.emit(Instruction::LocalSet(locs.exp2));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(is_special));

  // `inf` / `infinity` / `nan` spellings (validated by __rt_parse_float_valid).
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(0x20));
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalSet(ch));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'i' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(f64::INFINITY)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(is_special));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'n' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(f64::NAN)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(is_special));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Integer digits accumulated in unsigned 128-bit significand (up to 38 significant digits).
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(ch));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.digit));
  rt_emit_u128_decimal_digit(&mut b, &locs, false);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Optional fractional part.
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(ch));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.digit));
  rt_emit_u128_decimal_digit(&mut b, &locs, true);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Optional decimal exponent: e|E [+-]? digits.
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(0x20));
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Const(b'e' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(exp_neg));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(exp_val));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(ch));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'-' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(exp_neg));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'+' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(ch));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(exp_val));
  b.emit(Instruction::I32Const(10_000_000));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(exp_val));
  b.emit(Instruction::I32Const(10));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(exp_val));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(exp_neg));
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(exp_val));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(exp_val));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalTee(explicit_exp));
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.exp10));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Convert accumulated 128-bit significand and scale by 10^exp10, then round once to f64.
  b.emit(Instruction::LocalGet(is_special));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Eqz);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Eqz);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Const(315));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(f64::INFINITY)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Const(-370));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::LocalSet(scale_steps));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(scale_steps));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));
  rt_emit_u128_mul_by_5_step(&mut b, &locs);
  b.emit(Instruction::LocalGet(scale_steps));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(scale_steps));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(scale_steps));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(scale_steps));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));
  rt_emit_u128_div_by_5_step(&mut b, &locs);
  b.emit(Instruction::LocalGet(scale_steps));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(scale_steps));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  rt_emit_u128_normalize(&mut b, &locs);
  rt_emit_u128_round_to_f64(&mut b, &locs, result);
  b.emit(Instruction::LocalGet(locs.sticky));
  b.emit(Instruction::If(BlockType::Empty));
  // Bound accumulated truncation/scaling error: check whether `V_computed + 65536 ULP_lo`
  // could cross an f64 rounding midpoint, and fall back to exact big-integer comparison if so.
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(65_536));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(locs.t_lo));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64LtU);
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(locs.t_hi));
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I64Const(0x8000_0000_0000_0000u64 as i64));
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.exp2));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.sticky));
  rt_emit_u128_round_to_f64(&mut b, &locs, result_hi);
  b.emit(Instruction::LocalGet(result));
  b.emit(Instruction::I64ReinterpretF64);
  b.emit(Instruction::LocalGet(result_hi));
  b.emit(Instruction::I64ReinterpretF64);
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::LocalGet(num_start_i));
  b.emit(Instruction::LocalGet(explicit_exp));
  b.emit(Instruction::LocalGet(result));
  b.emit(Instruction::I64ReinterpretF64);
  b.emit(Instruction::Call(parse_float_exact_idx));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Apply sign.
  b.emit(Instruction::LocalGet(is_neg));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(result));
  b.emit(Instruction::F64Neg);
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(result));
  b.finish(vec![ValType::F64], vec![ValType::F64])
}

/// `__rt_parse_float_valid(s: f64) → f64`
///
/// Returns 1.0 when the whole text is a float literal accepted by native
/// `str::parse::<f64>`: optional sign, then `inf` / `infinity` / `nan` in any
/// case, or digits with an optional fraction (at least one digit overall) and
/// an optional exponent with at least one digit. No surrounding whitespace.
#[allow(clippy::vec_init_then_push)]
pub(in crate::wasm::emit_wasm) fn build_rt_parse_float_valid() -> CompiledFn {
  // params: 0=s(f64)
  // locals: 1=ptr, 2=len, 3=content, 4=i, 5=b, 6=digits, 7=ok, 8=edigits (all i32)
  let mut b: Vec<Instruction> = Vec::new();
  let byte_at_i = |b: &mut Vec<Instruction<'static>>| {
    b.push(Instruction::LocalGet(3));
    b.push(Instruction::LocalGet(4));
    b.push(Instruction::I32Add);
    b.push(Instruction::I32Load8U(mem_arg_byte(0)));
    b.push(Instruction::LocalSet(5));
  };
  let inc_i = |b: &mut Vec<Instruction<'static>>| {
    b.push(Instruction::LocalGet(4));
    b.push(Instruction::I32Const(1));
    b.push(Instruction::I32Add);
    b.push(Instruction::LocalSet(4));
  };
  // Consume ASCII digits, counting them in `counter`.
  let digit_loop = |b: &mut Vec<Instruction<'static>>, counter: u32| {
    b.push(Instruction::Block(BlockType::Empty));
    b.push(Instruction::Loop(BlockType::Empty));
    b.push(Instruction::LocalGet(4));
    b.push(Instruction::LocalGet(2));
    b.push(Instruction::I32GeU);
    b.push(Instruction::BrIf(1));
    b.push(Instruction::LocalGet(3));
    b.push(Instruction::LocalGet(4));
    b.push(Instruction::I32Add);
    b.push(Instruction::I32Load8U(mem_arg_byte(0)));
    b.push(Instruction::LocalSet(5));
    b.push(Instruction::LocalGet(5));
    b.push(Instruction::I32Const(b'0' as i32));
    b.push(Instruction::I32LtU);
    b.push(Instruction::BrIf(1));
    b.push(Instruction::LocalGet(5));
    b.push(Instruction::I32Const(b'9' as i32));
    b.push(Instruction::I32GtU);
    b.push(Instruction::BrIf(1));
    b.push(Instruction::LocalGet(counter));
    b.push(Instruction::I32Const(1));
    b.push(Instruction::I32Add);
    b.push(Instruction::LocalSet(counter));
    b.push(Instruction::LocalGet(4));
    b.push(Instruction::I32Const(1));
    b.push(Instruction::I32Add);
    b.push(Instruction::LocalSet(4));
    b.push(Instruction::Br(0));
    b.push(Instruction::End);
    b.push(Instruction::End);
  };

  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(1));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(2));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(3));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(6));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(8));

  // optional sign
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  byte_at_i(&mut b);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(b'-' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(b'+' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::If(BlockType::Empty));
  inc_i(&mut b);
  b.push(Instruction::End);
  b.push(Instruction::End);

  // Case-insensitive spellings: the rest must equal one word exactly.
  // Local 6 doubles as the "special matched" flag until the numeric path starts.
  for word in ["inf", "infinity", "nan"] {
    // matched = (len - i == word.len())
    b.push(Instruction::LocalGet(2));
    b.push(Instruction::LocalGet(4));
    b.push(Instruction::I32Sub);
    b.push(Instruction::I32Const(word.len() as i32));
    b.push(Instruction::I32Eq);
    b.push(Instruction::If(BlockType::Empty));
    for (k, ch) in word.bytes().enumerate() {
      // if (content[i + k] | 0x20) != ch then mismatch
      b.push(Instruction::LocalGet(3));
      b.push(Instruction::LocalGet(4));
      b.push(Instruction::I32Add);
      b.push(Instruction::I32Load8U(mem_arg_byte(k as u64)));
      b.push(Instruction::I32Const(0x20));
      b.push(Instruction::I32Or);
      b.push(Instruction::I32Const(ch as i32));
      b.push(Instruction::I32Ne);
      b.push(Instruction::If(BlockType::Empty));
      b.push(Instruction::I32Const(-1));
      b.push(Instruction::LocalSet(8));
      b.push(Instruction::End);
    }
    // local 8 stays 0 when every byte matched
    b.push(Instruction::LocalGet(8));
    b.push(Instruction::I32Eqz);
    b.push(Instruction::If(BlockType::Empty));
    b.push(Instruction::I32Const(1));
    b.push(Instruction::LocalSet(7));
    b.push(Instruction::End);
    b.push(Instruction::I32Const(0));
    b.push(Instruction::LocalSet(8));
    b.push(Instruction::End);
  }

  // Numeric path only when no spelling matched.
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Eqz);
  b.push(Instruction::If(BlockType::Empty));
  digit_loop(&mut b, 6);
  // optional fraction
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  byte_at_i(&mut b);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(b'.' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  inc_i(&mut b);
  digit_loop(&mut b, 6);
  b.push(Instruction::End);
  b.push(Instruction::End);
  // at least one mantissa digit
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::If(BlockType::Empty));
  // ok = 1 unless a malformed exponent follows
  b.push(Instruction::I32Const(1));
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  byte_at_i(&mut b);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(0x20));
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Const(b'e' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  inc_i(&mut b);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  byte_at_i(&mut b);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(b'-' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(b'+' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::If(BlockType::Empty));
  inc_i(&mut b);
  b.push(Instruction::End);
  b.push(Instruction::End);
  digit_loop(&mut b, 8);
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Eqz);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::End);
  b.push(Instruction::End);
  b.push(Instruction::End);
  // the whole text must be consumed
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Ne);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::End);
  b.push(Instruction::End); // end if digits
  b.push(Instruction::End); // end numeric path

  b.push(Instruction::LocalGet(7));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![ValType::I32; 8],
    instructions: b,
  }
}
