//! UTF-8 string comparison, trimming, escaping, and splitting WASM runtime helpers.

use super::super::*;
use super::*;
use wasm_encoder::BlockType;

/// `__rt_str_compare(ptr_a: i32, ptr_b: i32) → f64`
///
/// Lexicographic byte comparison. Returns -1.0, 0.0, or 1.0.
/// Strings are UTF-8 in linear memory at logical_ptr+8, length at logical_ptr+0 (f64).
pub(super) fn build_rt_str_compare() -> CompiledFn {
  // params: 0 = ptr_a (i32), 1 = ptr_b (i32)
  // locals: 2 = len_a, 3 = len_b, 4 = min_len, 5 = i, 6 = byte_a, 7 = byte_b
  let instructions = vec![
    // len_a = i32.trunc(f64.load ptr_a+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    // len_b = i32.trunc(f64.load ptr_b+0)
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // min_len = min(len_a, len_b)
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)),
    Instruction::LocalGet(2),
    Instruction::Else,
    Instruction::LocalGet(3),
    Instruction::End,
    Instruction::LocalSet(4),
    // i = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(5),
    // block $outer (result f64) — used for early return via Br(2)
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // block $break_loop — exit loop via BrIf(1) or Br(1)
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // loop $loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if i >= min_len: exit loop
    Instruction::LocalGet(5),
    Instruction::LocalGet(4),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    // byte_a = i32.load8u(ptr_a + 8 + i)
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(5),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(6),
    // byte_b = i32.load8u(ptr_b + 8 + i)
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(5),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(7),
    // if byte_a < byte_b: push -1.0, br $outer (depth 3 from inside if)
    Instruction::LocalGet(6),
    Instruction::LocalGet(7),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(-1.0f64)),
    Instruction::Br(3),
    Instruction::End,
    // if byte_a > byte_b: push 1.0, br $outer
    Instruction::LocalGet(6),
    Instruction::LocalGet(7),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    Instruction::Br(3),
    Instruction::End,
    // i += 1; continue loop
    Instruction::LocalGet(5),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(5),
    Instruction::Br(0),
    Instruction::End, // end loop
    Instruction::End, // end $break_loop
    // After loop: compare lengths to determine result
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)),
    Instruction::F64Const(Ieee64::from(-1.0f64)),
    Instruction::Else,
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    Instruction::Else,
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::End,
    Instruction::End,
    Instruction::End, // end $outer
  ];

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // len_a
      ValType::I32, // len_b
      ValType::I32, // min_len
      ValType::I32, // i
      ValType::I32, // byte_a
      ValType::I32, // byte_b
    ],
    instructions,
  }
}

/// `__rt_str_find_index(h_ptr: i32, n_ptr: i32) → f64`
///
/// Naive byte-level substring search. Returns the Unicode scalar index of the first
/// occurrence of the needle string in the haystack, or -1.0 if not found.
/// An empty needle always returns 0.0.
pub(super) fn build_rt_str_find_index() -> CompiledFn {
  // params: 0=h_ptr (i32), 1=n_ptr (i32)
  // locals: 2=h_len(i32), 3=n_len(i32), 4=h_base(i32), 5=n_base(i32),
  //         6=i(i32), 7=j(i32), 8=limit(i32), 9=byte_h(i32), 10=byte_n(i32), 11=scalar(i32)
  let instructions = vec![
    // h_len = i32(f64.load h_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    // n_len = i32(f64.load n_ptr+0)
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // h_base = h_ptr + 8
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(4),
    // n_base = n_ptr + 8
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(5),
    // Block $outer (result f64) — used for early-exit via Br
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if n_len == 0: push 0.0, br $outer
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // if n_len > h_len: push -1.0, br $outer
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(-1.0f64)),
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // limit = h_len - n_len  (last valid start position)
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32Sub,
    Instruction::LocalSet(8),
    // i = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(6),
    // Block $exit_outer (empty) — outer loop exit
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // Loop $outer_loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if i > limit: br $exit_outer
    Instruction::LocalGet(6),
    Instruction::LocalGet(8),
    Instruction::I32GtU,
    Instruction::BrIf(1), // 0=$outer_loop(continue), 1=$exit_outer(break)
    // j = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(7),
    // Block $mismatch (empty) — inner loop exit on mismatch
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // Loop $inner_loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if j >= n_len: found at byte i. Count scalars in [0, i) only on success.
    Instruction::LocalGet(7),
    Instruction::LocalGet(3),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(7), // reuse j as prefix cursor
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(7),
    Instruction::LocalGet(6),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    // A valid UTF-8 scalar starts at every non-continuation byte.
    // j < i <= h_len - n_len keeps every load inside the haystack.
    Instruction::LocalGet(11),
    Instruction::LocalGet(4),
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::I32Const(0xc0),
    Instruction::I32And,
    Instruction::I32Const(0x80),
    Instruction::I32Ne,
    Instruction::I32Add,
    Instruction::LocalSet(11),
    Instruction::LocalGet(7),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(7),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(11),
    Instruction::F64ConvertI32U,
    // depths inside If: 0=If,1=$inner_loop,2=$mismatch,3=$outer_loop,4=$exit_outer,5=$outer
    Instruction::Br(5),
    Instruction::End,
    // byte_h = h_base[i + j]
    Instruction::LocalGet(4),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(9),
    // byte_n = n_base[j]
    Instruction::LocalGet(5),
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(10),
    // if byte_h != byte_n: br $mismatch
    Instruction::LocalGet(9),
    Instruction::LocalGet(10),
    Instruction::I32Ne,
    Instruction::BrIf(1), // 0=$inner_loop, 1=$mismatch
    // j++
    Instruction::LocalGet(7),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(7),
    Instruction::Br(0), // continue $inner_loop
    Instruction::End,   // end $inner_loop
    Instruction::End,   // end $mismatch
    // i++
    Instruction::LocalGet(6),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(6),
    Instruction::Br(0), // continue $outer_loop
    Instruction::End,   // end $outer_loop
    Instruction::End,   // end $exit_outer
    // Not found: push -1.0 (result of $outer)
    Instruction::F64Const(Ieee64::from(-1.0f64)),
    Instruction::End, // end $outer (result f64)
  ];

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // h_len (2)
      ValType::I32, // n_len (3)
      ValType::I32, // h_base (4)
      ValType::I32, // n_base (5)
      ValType::I32, // i (6)
      ValType::I32, // j (7)
      ValType::I32, // limit (8)
      ValType::I32, // byte_h (9)
      ValType::I32, // byte_n (10)
      ValType::I32, // scalar (11), zero-initialized on entry
    ],
    instructions,
  }
}

/// `__rt_str_starts_with(s_ptr: i32, p_ptr: i32) → f64`
///
/// Returns 1.0 if the string at s_ptr starts with the prefix at p_ptr, else 0.0.
/// An empty prefix always returns 1.0.
pub(super) fn build_rt_str_starts_with() -> CompiledFn {
  // params: 0=s_ptr(i32), 1=p_ptr(i32)
  // locals: 2=s_len(i32), 3=p_len(i32), 4=s_base(i32), 5=p_base(i32),
  //         6=i(i32), 7=byte_s(i32), 8=byte_p(i32)
  let instructions = vec![
    // s_len = i32(f64.load s_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    // p_len = i32(f64.load p_ptr+0)
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // s_base = s_ptr + 8
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(4),
    // p_base = p_ptr + 8
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(5),
    // Block $outer (result f64) — early-exit via Br
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if p_len == 0: push 1.0, br $outer (empty prefix always matches)
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // if p_len > s_len: push 0.0, br $outer
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // i = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(6),
    // Block $fail (empty) — break here on first mismatch
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // Loop $loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if i >= p_len: all bytes matched → push 1.0, br $outer
    Instruction::LocalGet(6),
    Instruction::LocalGet(3),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    // depths: 0=If, 1=$loop, 2=$fail, 3=$outer
    Instruction::Br(3),
    Instruction::End,
    // byte_s = s_base[i]
    Instruction::LocalGet(4),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(7),
    // byte_p = p_base[i]
    Instruction::LocalGet(5),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(8),
    // if byte_s != byte_p: br $fail
    Instruction::LocalGet(7),
    Instruction::LocalGet(8),
    Instruction::I32Ne,
    Instruction::BrIf(1), // 0=$loop, 1=$fail
    // i++
    Instruction::LocalGet(6),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(6),
    Instruction::Br(0), // continue $loop
    Instruction::End,   // end $loop
    Instruction::End,   // end $fail
    // Mismatch: push 0.0 (result of $outer)
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::End, // end $outer
  ];

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // s_len (2)
      ValType::I32, // p_len (3)
      ValType::I32, // s_base (4)
      ValType::I32, // p_base (5)
      ValType::I32, // i (6)
      ValType::I32, // byte_s (7)
      ValType::I32, // byte_p (8)
    ],
    instructions,
  }
}

/// `__rt_str_ends_with(s_ptr: i32, suf_ptr: i32) → f64`
///
/// Returns 1.0 if the string at s_ptr ends with the suffix at suf_ptr, else 0.0.
/// An empty suffix always returns 1.0.
pub(super) fn build_rt_str_ends_with() -> CompiledFn {
  // params: 0=s_ptr(i32), 1=suf_ptr(i32)
  // locals: 2=s_len(i32), 3=suf_len(i32), 4=s_base(i32), 5=suf_base(i32),
  //         6=offset(i32), 7=i(i32), 8=byte_s(i32), 9=byte_suf(i32)
  let instructions = vec![
    // s_len = i32(f64.load s_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    // suf_len = i32(f64.load suf_ptr+0)
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // s_base = s_ptr + 8
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(4),
    // suf_base = suf_ptr + 8
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(5),
    // Block $outer (result f64)
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if suf_len == 0: push 1.0, br $outer
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    Instruction::Br(1),
    Instruction::End,
    // if suf_len > s_len: push 0.0, br $outer
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::Br(1),
    Instruction::End,
    // offset = s_len - suf_len  (start byte position in s for comparison)
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32Sub,
    Instruction::LocalSet(6),
    // i = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(7),
    // Block $fail (empty)
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // Loop $loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if i >= suf_len: push 1.0, br $outer
    Instruction::LocalGet(7),
    Instruction::LocalGet(3),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    // depths: 0=If, 1=$loop, 2=$fail, 3=$outer
    Instruction::Br(3),
    Instruction::End,
    // byte_s = s_base[offset + i]
    Instruction::LocalGet(4),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(8),
    // byte_suf = suf_base[i]
    Instruction::LocalGet(5),
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(9),
    // if byte_s != byte_suf: br $fail
    Instruction::LocalGet(8),
    Instruction::LocalGet(9),
    Instruction::I32Ne,
    Instruction::BrIf(1),
    // i++
    Instruction::LocalGet(7),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(7),
    Instruction::Br(0), // continue $loop
    Instruction::End,   // end $loop
    Instruction::End,   // end $fail
    // Mismatch: push 0.0
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::End, // end $outer
  ];

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // s_len (2)
      ValType::I32, // suf_len (3)
      ValType::I32, // s_base (4)
      ValType::I32, // suf_base (5)
      ValType::I32, // offset (6)
      ValType::I32, // i (7)
      ValType::I32, // byte_s (8)
      ValType::I32, // byte_suf (9)
    ],
    instructions,
  }
}

// ---------------------------------------------------------------------------
// Helper: emit "is ASCII whitespace" check for local `loc` → leaves i32 on stack.
// Whitespace: 0x09 (tab), 0x0A (LF), 0x0D (CR), 0x20 (space).
pub(super) fn emit_is_ws_check(b: &mut Vec<Instruction<'static>>, loc: u32) {
  b.push(Instruction::LocalGet(loc));
  b.push(Instruction::I32Const(0x09));
  b.push(Instruction::I32Eq);
  b.push(Instruction::LocalGet(loc));
  b.push(Instruction::I32Const(0x0A));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::LocalGet(loc));
  b.push(Instruction::I32Const(0x0D));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::LocalGet(loc));
  b.push(Instruction::I32Const(0x20));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
}

/// `__rt_trim_ws(s: f64) → f64`
///
/// Strips leading and trailing ASCII whitespace from a heap string.
/// Returns a new heap-allocated string.
#[allow(clippy::vec_init_then_push)]
pub(super) fn build_rt_trim_ws(str_tag: i32) -> CompiledFn {
  // params: 0=s(f64)
  // locals: 1=ptr(i32), 2=byte_len(i32), 3=content(i32), 4=start(i32),
  //         5=end(i32), 6=new_len(i32), 7=raw_base(i32), 8=new_ptr(i32),
  //         9=padded(i32), 10=b(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // ptr = i32(s)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(1));

  // byte_len = i32(f64load(ptr+0))
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(2));

  // content = ptr + 8
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(3));

  // start = 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(4));

  // Forward loop: while start < byte_len && is_ws(content[start]): start++
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
  b.push(Instruction::LocalSet(10));
  emit_is_ws_check(&mut b, 10);
  b.push(Instruction::I32Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // end = byte_len
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::LocalSet(5));

  // Backward loop: while end > start && is_ws(content[end-1]): end--
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32LeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-1i32));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(10));
  emit_is_ws_check(&mut b, 10);
  b.push(Instruction::I32Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(5));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // new_len = end - start
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(6));

  // padded = (new_len + 7) & -8
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(9));

  // Allocate heap string
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(7)); // raw_base
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(str_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // new_ptr = raw_base + 8
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8));
  // byte_len at new_ptr+0
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));
  // advance heap: raw_base + 16 + padded
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  // memory.copy(new_ptr+8, content+start, new_len)
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  // return f64(new_ptr)
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // ptr (1)
      ValType::I32, // byte_len (2)
      ValType::I32, // content (3)
      ValType::I32, // start (4)
      ValType::I32, // end (5)
      ValType::I32, // new_len (6)
      ValType::I32, // raw_base (7)
      ValType::I32, // new_ptr (8)
      ValType::I32, // padded (9)
      ValType::I32, // b (10)
    ],
    instructions: b,
  }
}

/// `__rt_trim_char(s: f64, c: f64) → f64`
///
/// Strips single Unicode scalar pattern `c` (valid 1..=4 byte UTF-8 scalar) from
/// both ends of `s`, trapping on empty or multi-scalar patterns to match
/// `builtins::strings::trim` and JS `trim`.
#[allow(clippy::vec_init_then_push)]
pub(super) fn build_rt_trim_char(str_tag: i32) -> CompiledFn {
  // params: 0=s(f64), 1=c(f64) (char string ptr)
  // locals: 2=ptr(i32), 3=byte_len(i32), 4=content(i32), 5=start(i32),
  //         6=end(i32), 7=new_len(i32), 8=raw_base(i32), 9=new_ptr(i32),
  //         10=padded(i32), 11=c_len(i32), 12=c_ptr(i32), 13=c_content(i32),
  //         14=expected_width/k(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // Validate that pattern `c` is a single UTF-8 Unicode scalar value.
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalTee(12));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalTee(11));
  b.push(Instruction::I32Eqz);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::Unreachable);
  b.push(Instruction::End);

  // c_content = c_ptr + 8
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalTee(13));
  // Compute expected UTF-8 byte width from leading byte b0 (stored in local 12 temporarily).
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalTee(12));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::Else);
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(0xC0));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::Else);
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(0xE0));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::Else);
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(0xF0));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(3));
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::Else);
  b.push(Instruction::I32Const(4));
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::End);
  b.push(Instruction::End);
  b.push(Instruction::End);
  b.push(Instruction::End);

  // Trap if c_len != expected_width (empty, multi-scalar, or invalid leading byte).
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Ne);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::Unreachable);
  b.push(Instruction::End);

  // ptr = i32(s)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(2));

  // byte_len = i32(f64load(ptr))
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(3));

  // content = ptr + 8
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));

  // start = 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(5));

  // Forward loop: while byte_len - start >= c_len && content[start..start+c_len] == c_content[0..c_len]
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32LtU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::I32Ne);
  b.push(Instruction::BrIf(3));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(5));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // end = byte_len
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalSet(6));

  // Backward loop: while end - start >= c_len && content[end-c_len..end] == c_content[0..c_len]
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32LtU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::I32Ne);
  b.push(Instruction::BrIf(3));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(6));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // new_len = end - start
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(7));

  // padded = (new_len + 7) & -8
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(10));

  // Allocate heap string
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(8));
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(str_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(9));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // ptr (2)
      ValType::I32, // byte_len (3)
      ValType::I32, // content (4)
      ValType::I32, // start (5)
      ValType::I32, // end (6)
      ValType::I32, // new_len (7)
      ValType::I32, // raw_base (8)
      ValType::I32, // new_ptr (9)
      ValType::I32, // padded (10)
      ValType::I32, // c_len (11)
      ValType::I32, // c_ptr / b0 (12)
      ValType::I32, // c_content (13)
      ValType::I32, // expected_width / k (14)
    ],
    instructions: b,
  }
}

/// `__rt_blank(s: f64) → f64`
///
/// Returns 1.0 if the string contains only ASCII whitespace (or is empty), 0.0 otherwise.
#[allow(clippy::vec_init_then_push)]
pub(super) fn build_rt_blank() -> CompiledFn {
  // params: 0=s(f64)
  // locals: 1=ptr(i32), 2=byte_len(i32), 3=content(i32), 4=i(i32), 5=b(i32)
  let mut b: Vec<Instruction> = Vec::new();

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

  // Loop: if any byte is non-whitespace, return 0.0
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  // if i >= byte_len: break (all whitespace)
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  // b = load8(content + i)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(5));
  // if NOT is_ws: return 0.0
  emit_is_ws_check(&mut b, 5);
  b.push(Instruction::I32Eqz);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::Return);
  b.push(Instruction::End);
  // i++
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  b.push(Instruction::F64Const(Ieee64::from(1.0f64)));

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // ptr (1)
      ValType::I32, // byte_len (2)
      ValType::I32, // content (3)
      ValType::I32, // i (4)
      ValType::I32, // b (5)
    ],
    instructions: b,
  }
}

/// `__rt_char_from_code(cp: f64) → f64`
///
/// Encodes a Unicode codepoint (u32) as a UTF-8 string and allocates it on the heap.
/// Returns the logical string pointer as f64.
#[allow(clippy::vec_init_then_push)]
pub(super) fn build_rt_char_from_code(str_tag: i32) -> CompiledFn {
  // params: 0=cp(f64)
  // locals: 1=code(i32), 2=raw_base(i32), 3=new_ptr(i32), 4=byte_len(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // Reject non-integers, out-of-range values, and surrogate code points.
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Trunc);
  b.push(Instruction::F64Ne);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::Unreachable);
  b.push(Instruction::End);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(0.0.into()));
  b.push(Instruction::F64Lt);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::Unreachable);
  b.push(Instruction::End);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const((0x10ffff as f64).into()));
  b.push(Instruction::F64Gt);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::Unreachable);
  b.push(Instruction::End);

  // code = i32(cp)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(1));

  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0xd800));
  b.push(Instruction::I32GeU);
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0xdfff));
  b.push(Instruction::I32LeU);
  b.push(Instruction::I32And);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::Unreachable);
  b.push(Instruction::End);

  // Determine byte_len based on codepoint range
  // Default: 1 byte (ASCII)
  b.push(Instruction::I32Const(1));
  b.push(Instruction::LocalSet(4));

  // if code >= 0x80: byte_len = 2
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::I32GeU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);

  // if code >= 0x800: byte_len = 3
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x800));
  b.push(Instruction::I32GeU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(3));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);

  // if code >= 0x10000: byte_len = 4
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x10000));
  b.push(Instruction::I32GeU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);

  // Allocate: raw_base = HEAP_PTR; write magic+tag; new_ptr = raw_base+8
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(2)); // raw_base
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(str_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // new_ptr = raw_base + 8
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(3));
  // store byte_len as f64 at new_ptr+0
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));
  // advance heap: raw_base + 16 + ((byte_len+7)&-8)
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // Write UTF-8 bytes at new_ptr+8 based on byte_len
  // content_base = new_ptr + 8
  // byte_len == 1: store byte code (0..7F)
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Store8(mem_arg_byte(8)));
  b.push(Instruction::End);

  // byte_len == 2: 110xxxxx 10xxxxxx
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  // b0 = 0xC0 | (code >> 6)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0xC0));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(6));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(8)));
  // b1 = 0x80 | (code & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(9)));
  b.push(Instruction::End);

  // byte_len == 3: 1110xxxx 10xxxxxx 10xxxxxx
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(3));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  // b0 = 0xE0 | (code >> 12)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0xE0));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(12));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(8)));
  // b1 = 0x80 | ((code >> 6) & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(6));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(9)));
  // b2 = 0x80 | (code & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(10)));
  b.push(Instruction::End);

  // byte_len == 4: 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  // b0 = 0xF0 | (code >> 18)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0xF0));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(18));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(8)));
  // b1 = 0x80 | ((code >> 12) & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(12));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(9)));
  // b2 = 0x80 | ((code >> 6) & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(6));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(10)));
  // b3 = 0x80 | (code & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(11)));
  b.push(Instruction::End);

  // return f64(new_ptr)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // code (1)
      ValType::I32, // raw_base (2)
      ValType::I32, // new_ptr (3)
      ValType::I32, // byte_len (4)
    ],
    instructions: b,
  }
}

/// `__rt_str_replace(s: f64, pat: f64, rep: f64) → f64`
///
/// Replaces ALL non-overlapping occurrences of `pat` in `s` with `rep`.
/// Writes output directly to heap — no separate scratch buffer needed.
///
/// Layout of a string logical ptr P: [byte_len:f64][utf8_bytes...]
#[allow(clippy::vec_init_then_push)]
pub(super) fn build_rt_str_replace(str_tag: i32) -> CompiledFn {
  // params: 0=s(f64), 1=pat(f64), 2=rep(f64)
  // locals: 3=s_ptr(i32), 4=s_len(i32), 5=s_cont(i32)
  //         6=pat_ptr(i32), 7=pat_len(i32), 8=pat_cont(i32)
  //         9=rep_ptr(i32), 10=rep_len(i32), 11=rep_cont(i32)
  //         12=raw_base(i32), 13=content_base(i32)
  //         14=out_pos(i32), 15=si(i32)
  //         16=pi(i32), 17=matched(i32)
  //         18=new_ptr(i32), 19=padded(i32)
  //         20=max_out(i32), 21=tmp(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // unpack s
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(3));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(5));

  // unpack pat
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(6));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8));

  // if pat_len == 0: return s unchanged
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Eqz);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::Return);
  b.push(Instruction::End);

  // unpack rep
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(9));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(10));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(11));

  // max_out = s_len + (s_len + 1) * rep_len (worst case: all chars replaced)
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::I32Mul);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(20));

  // raw_base = HEAP_PTR (save before allocating)
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalSet(12));

  // content_base = raw_base + 16 (after 8-byte header + 8-byte byte_len)
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(13));

  // padded = (max_out + 7) & -8
  b.push(Instruction::LocalGet(20));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(19));

  // Pessimistic bump: HEAP_PTR = raw_base + 16 + padded
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(19));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // out_pos = 0, si = 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(15));

  // Main scan loop: while si + pat_len <= s_len
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  // break if si + pat_len > s_len
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32GtU);
  b.push(Instruction::BrIf(1));

  // Try to match: pi = 0, matched = 1
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(16));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::LocalSet(17));

  // Inner compare loop
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  // if pi >= pat_len: done (matched)
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  // if s_cont[si+pi] != pat_cont[pi]: mismatch
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::I32Ne);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(17)); // matched = 0
  b.push(Instruction::Br(2)); // break out of inner block
  b.push(Instruction::End);
  // pi++; continue
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(16));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // if matched: copy rep → content_base+out_pos, si += pat_len, out_pos += rep_len
  b.push(Instruction::LocalGet(17));
  b.push(Instruction::If(BlockType::Empty));
  // memory.copy(content_base + out_pos, rep_cont, rep_len)
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(14)); // out_pos += rep_len
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(15)); // si += pat_len
  b.push(Instruction::Else);
  // not matched: copy 1 byte; si++; out_pos++
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(15));
  b.push(Instruction::End);
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // Copy tail: s_cont[si..s_len] → content_base + out_pos
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(21)); // tmp = s_len - si
  b.push(Instruction::LocalGet(21));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::I32GtU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(21));
  b.push(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::LocalGet(21));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::End);

  // Write header: magic, str_tag, byte_len at raw_base
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(str_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // new_ptr = raw_base + 8
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(18));
  // byte_len = out_pos
  b.push(Instruction::LocalGet(18));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));

  // Correct HEAP_PTR: raw_base + 16 + ((out_pos + 7) & -8)
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(19));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(19));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // return f64(new_ptr)
  b.push(Instruction::LocalGet(18));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::F64, ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // s_ptr (3)
      ValType::I32, // s_len (4)
      ValType::I32, // s_cont (5)
      ValType::I32, // pat_ptr (6)
      ValType::I32, // pat_len (7)
      ValType::I32, // pat_cont (8)
      ValType::I32, // rep_ptr (9)
      ValType::I32, // rep_len (10)
      ValType::I32, // rep_cont (11)
      ValType::I32, // raw_base (12)
      ValType::I32, // content_base (13)
      ValType::I32, // out_pos (14)
      ValType::I32, // si (15)
      ValType::I32, // pi (16)
      ValType::I32, // matched (17)
      ValType::I32, // new_ptr (18)
      ValType::I32, // padded (19)
      ValType::I32, // max_out (20)
      ValType::I32, // tmp (21)
    ],
    instructions: b,
  }
}

/// `__rt_str_escape(s: f64) → f64`
///
/// Escapes special characters in a string and wraps in double quotes.
pub(super) fn build_rt_str_escape(string_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let src = b.alloc_i32();
  let len = b.alloc_i32();
  let index = b.alloc_i32();
  let byte = b.alloc_i32();
  let extra = b.alloc_i32();
  let out_len = b.alloc_i32();
  let padded = b.alloc_i32();
  let payload = b.alloc_i32();
  let out = b.alloc_i32();
  let dst = b.alloc_i32();
  let out_index = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(src));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(extra));

  // Pass 1: count escape bytes for \n, \r, \t, ", ', \
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(byte));

  for escaped in b"\n\r\t\"'\\" {
    b.emit(Instruction::LocalGet(byte));
    b.emit(Instruction::I32Const(*escaped as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(extra));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(extra));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Allocate: 2 quotes + len + extra
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::LocalGet(extra));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_len));
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(payload));
  rt_emit_alloc_dynamic(&mut b, payload, out, string_tag);
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst));

  // Leading '"'
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(out_index));

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(byte));

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\n' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\r' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\t' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\'' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\n' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(b'n' as i32));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\r' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(b'r' as i32));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\t' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(b't' as i32));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Trailing '"'
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));

  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::F64ConvertI32U);
  b.finish(vec![ValType::F64], vec![ValType::F64])
}

/// `__rt_edn_string(s: f64) → f64`
///
/// Formats a heap string as a Cirru EDN string leaf. Simple ASCII tokens use
/// the `|text` form; other values use a quoted `"|text"` leaf and escape the
/// four characters handled specially by the Cirru writer.
pub(super) fn build_rt_edn_string(string_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let src = b.alloc_i32();
  let len = b.alloc_i32();
  let index = b.alloc_i32();
  let byte = b.alloc_i32();
  let simple = b.alloc_i32();
  let extra = b.alloc_i32();
  let out_len = b.alloc_i32();
  let padded = b.alloc_i32();
  let payload = b.alloc_i32();
  let out = b.alloc_i32();
  let dst = b.alloc_i32();
  let out_index = b.alloc_i32();

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
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(simple));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(extra));

  // First pass: decide whether the leaf can stay bare and count escapes.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(byte));

  // Cirru bare-leaf characters: ASCII alphanumeric plus writer punctuation.
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'A' as i32));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'Z' as i32));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'a' as i32));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'z' as i32));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  for allowed in b"$-:<>[]{}*=+.,\\/!?~_@#&%^|;'" {
    b.emit(Instruction::LocalGet(byte));
    b.emit(Instruction::I32Const(*allowed as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::I32Or);
  }
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(simple));
  b.emit(Instruction::End);

  for escaped in b"\n\t\"\\" {
    b.emit(Instruction::LocalGet(byte));
    b.emit(Instruction::I32Const(*escaped as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(extra));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(extra));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // bare: `|` + bytes; quoted: `"|` + escaped bytes + `"`.
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::LocalGet(simple));
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::LocalGet(extra));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_len));
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::I32Const(edn::MAX_EDN_OUTPUT_BYTES));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(payload));
  rt_emit_alloc_dynamic(&mut b, payload, out, string_tag);
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst));

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::LocalGet(simple));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'|' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(byte));

  b.emit(Instruction::LocalGet(simple));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\n' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\t' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\n' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(b'n' as i32));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\t' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(b't' as i32));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(simple));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::F64ConvertI32U);
  b.finish(vec![ValType::F64], vec![ValType::F64])
}

/// `__rt_json_string(s: f64) → f64`
///
/// Formats a heap string as an RFC 8259 JSON string literal (`"..."`),
/// escaping `"`, `\`, `\b`, `\f`, `\n`, `\r`, `\t`, and any control byte `< 0x20`
/// as `\u00xx`.
pub(super) fn build_rt_json_string(string_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let src = b.alloc_i32();
  let len = b.alloc_i32();
  let index = b.alloc_i32();
  let byte = b.alloc_i32();
  let extra = b.alloc_i32();
  let out_len = b.alloc_i32();
  let padded = b.alloc_i32();
  let payload = b.alloc_i32();
  let out = b.alloc_i32();
  let dst = b.alloc_i32();
  let out_index = b.alloc_i32();
  let hex_digit = b.alloc_i32();

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
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(extra));

  // Pass 1: count extra escape bytes.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(byte));

  // '"', '\\', '\b' (8), '\t' (9), '\n' (10), '\f' (12), '\r' (13) add 1 byte.
  // Any other control byte (< 0x20) adds 5 bytes ("\u00xx").
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x08));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x09));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0a));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0c));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0d));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(extra));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(extra));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x20));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(extra));
  b.emit(Instruction::I32Const(5));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(extra));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::LocalGet(extra));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_len));
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::I32Const(edn::MAX_EDN_OUTPUT_BYTES));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(payload));
  rt_emit_alloc_dynamic(&mut b, payload, out, string_tag);
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst));

  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(byte));

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x08));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x09));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0a));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0c));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0d));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  for (raw_byte, esc_char) in [(0x08, b'b'), (0x09, b't'), (0x0a, b'n'), (0x0c, b'f'), (0x0d, b'r')] {
    b.emit(Instruction::LocalGet(byte));
    b.emit(Instruction::I32Const(raw_byte));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::I32Const(esc_char as i32));
    b.emit(Instruction::LocalSet(byte));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x20));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  for (offset, ch) in [(0, b'\\'), (1, b'u'), (2, b'0'), (3, b'0')] {
    b.emit(Instruction::LocalGet(dst));
    b.emit(Instruction::LocalGet(out_index));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::I32Const(ch as i32));
    b.emit(Instruction::I32Store8(mem_arg_byte(offset)));
  }
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::LocalSet(hex_digit));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(hex_digit));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Store8(mem_arg_byte(4)));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(hex_digit));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(hex_digit));
  b.emit(Instruction::I32Const(10));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(hex_digit));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(hex_digit));
  b.emit(Instruction::I32Const((b'a' - 10) as i32));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Store8(mem_arg_byte(5)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(6));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::F64ConvertI32U);
  b.finish(vec![ValType::F64], vec![ValType::F64])
}

/// `__rt_tag_from_str(str_ptr: i32) → f64`
///
/// Looks up a heap string's bytes in the static tag table and returns the tag id as f64,
/// or 0.0 if the tag is not registered in the compiled module.
pub(super) fn build_rt_tag_from_str(tag_index: &HashMap<String, u32>) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let len = b.alloc_i32();
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(len));

  let mut by_len: BTreeMap<usize, Vec<(&str, u32)>> = BTreeMap::new();
  for (name, &id) in tag_index {
    by_len.entry(name.len()).or_default().push((name.as_str(), id));
  }
  for entries in by_len.values_mut() {
    entries.sort_by_key(|(_, id)| *id);
  }

  for (tag_len, entries) in by_len {
    b.emit(Instruction::LocalGet(len));
    b.emit(Instruction::I32Const(tag_len as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::If(BlockType::Empty));
    for (name, id) in entries {
      if tag_len == 0 {
        b.emit(f64_const(id as f64));
        b.emit(Instruction::Return);
      } else {
        for (idx, ch) in name.bytes().enumerate() {
          b.emit(Instruction::LocalGet(0));
          b.emit(Instruction::I32Load8U(mem_arg_byte(8 + idx as u64)));
          b.emit(Instruction::I32Const(ch as i32));
          b.emit(Instruction::I32Eq);
          if idx > 0 {
            b.emit(Instruction::I32And);
          }
        }
        b.emit(Instruction::If(BlockType::Empty));
        b.emit(f64_const(id as f64));
        b.emit(Instruction::Return);
        b.emit(Instruction::End);
      }
    }
    b.emit(Instruction::End);
  }

  b.emit(f64_const(0.0));
  b.finish(vec![ValType::I32], vec![ValType::F64])
}

/// `__rt_str_find_from(h_ptr: i32, h_start: i32, pat_ptr: i32) -> i32`
/// Searches for pat in h starting at byte offset h_start.
/// Returns the byte offset of the first match, or -1 if not found.
pub(super) fn build_rt_str_find_from() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3); // h_ptr=0, h_start=1, pat_ptr=2
  let h_len = b.alloc_i32();
  let p_len = b.alloc_i32();
  let h_base = b.alloc_i32();
  let p_base = b.alloc_i32();
  let i = b.alloc_i32();
  let j = b.alloc_i32();
  let limit = b.alloc_i32();
  let bh = b.alloc_i32();
  let bp = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(h_len));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(p_len));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(h_base));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(p_base));

  // Block (result i32) for early-exit
  b.emit(Instruction::Block(wasm_encoder::BlockType::Result(ValType::I32)));

  // if p_len == 0: return h_start
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);

  // if h_start + p_len > h_len: return -1
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(h_len));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);

  // limit = h_len - p_len
  b.emit(Instruction::LocalGet(h_len));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(limit));

  // i = h_start
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalSet(i));

  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty)); // $exit
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty)); // $outer
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(limit));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::BrIf(1)); // $exit
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty)); // $mismatch
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty)); // $inner
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  // nesting: 0=If, 1=$inner, 2=$mismatch, 3=$outer, 4=$exit, 5=Block(i32)
  b.emit(Instruction::Br(5));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(h_base));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(bh));
  b.emit(Instruction::LocalGet(p_base));
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(bp));
  b.emit(Instruction::LocalGet(bh));
  b.emit(Instruction::LocalGet(bp));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::BrIf(1)); // $mismatch
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Br(0)); // $inner
  b.emit(Instruction::End); // end $inner
  b.emit(Instruction::End); // end $mismatch
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0)); // $outer
  b.emit(Instruction::End); // end $outer
  b.emit(Instruction::End); // end $exit
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::End); // end Block(i32)

  b.finish(vec![ValType::I32, ValType::I32, ValType::I32], vec![ValType::I32])
}

/// `__rt_utf8_char_len(b: i32) -> i32`
/// Given the first byte of a UTF-8 sequence, returns its byte width (1-4).
pub(super) fn build_rt_utf8_char_len() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  b.emit(Instruction::Block(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(0xE0));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(0xF0));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::End);
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

/// `__rt_str_split(s_ptr: i32, pat_ptr: i32) -> i32` (list logical ptr)
/// Splits string s by pat. Empty pieces are dropped. Empty pat = char-split.
pub(super) fn build_rt_str_split(string_tag: i32, list_tag: i32, find_from_idx: u32, utf8_char_len_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2); // s_ptr=0, pat_ptr=1
  let s_len = b.alloc_i32();
  let p_len = b.alloc_i32();
  let s_cont = b.alloc_i32();
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let prev = b.alloc_i32();
  let list_ptr = b.alloc_i32();
  let li = b.alloc_i32();
  let idx = b.alloc_i32();
  let piece_len = b.alloc_i32();
  let str_ptr = b.alloc_i32();
  let str_cont = b.alloc_i32();
  let char_len = b.alloc_i32();
  let padded = b.alloc_i32();
  let size = b.alloc_i32();
  let ki = b.alloc_i32();
  let list_size = b.alloc_i32();

  // load s_len, p_len, s_cont
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(s_len));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(p_len));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(s_cont));

  // --- PASS 1: COUNT ---
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // empty-pat: count UTF-8 chars
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(s_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(s_cont));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::Call(utf8_char_len_idx));
  b.emit(Instruction::LocalSet(char_len));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  // non-empty pat: count delimited pieces
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(prev));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(find_from_idx));
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // last segment
  b.emit(Instruction::LocalGet(s_len));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(piece_len));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::End);
  b.emit(Instruction::Br(2)); // break loop
  b.emit(Instruction::End);
  // mid segment
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(piece_len));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(prev));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End); // if p_len == 0

  // --- ALLOCATE LIST ---
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(list_size));
  rt_emit_alloc_dynamic(&mut b, list_size, list_ptr, list_tag);
  b.emit(Instruction::LocalGet(list_ptr));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));

  // --- PASS 2: FILL ---
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(li));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(prev));

  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // empty-pat: emit each UTF-8 char as a string
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(s_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(s_cont));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::Call(utf8_char_len_idx));
  b.emit(Instruction::LocalSet(char_len));
  // padded = (char_len + 7) & -8
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(&mut b, size, str_ptr, string_tag);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(str_cont));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(ki));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(str_cont));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(s_cont));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(ki));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  // store str_ptr in list
  b.emit(Instruction::LocalGet(list_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(li));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(li));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(li));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  // non-empty pat: emit each delimited piece
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(find_from_idx));
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(s_len));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(piece_len));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(piece_len));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(&mut b, size, str_ptr, string_tag);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(str_cont));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(ki));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(str_cont));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(s_cont));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(ki));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(list_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(li));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(li));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(li));
  b.emit(Instruction::End); // if piece_len > 0
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::BrIf(1)); // exit
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(prev));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);

  b.emit(Instruction::End);
  b.emit(Instruction::End); // if p_len == 0

  b.emit(Instruction::LocalGet(list_ptr));
  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}
