use super::*;
use wasm_encoder::BlockType;

pub(super) const PERSISTENT_SEMI_SPACE_BYTES: i32 = 1024 * 1024;
pub(super) const PERSISTENT_COMPACT_THRESHOLD: i32 = PERSISTENT_SEMI_SPACE_BYTES / 2;
pub(super) const HEAP_FORWARD_MAGIC: i32 = 0xCA1C_F0D1u32 as i32;
pub(super) const MAX_PROMOTE_DEPTH: i32 = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WasmMemoryLayout {
  pub static_end: i32,
  pub space_a_base: i32,
  pub space_a_end: i32,
  pub space_b_base: i32,
  pub space_b_end: i32,
  pub arena_base: i32,
  pub initial_pages: u64,
}

impl WasmMemoryLayout {
  pub fn compute(static_end: i32, atom_count: u32) -> Self {
    let static_end = (static_end + 7) & !7;
    let (space_a_base, space_a_end, space_b_base, space_b_end, arena_base) = if atom_count > 0 {
      let a_base = static_end;
      let a_end = a_base + PERSISTENT_SEMI_SPACE_BYTES;
      let b_base = a_end;
      let b_end = b_base + PERSISTENT_SEMI_SPACE_BYTES;
      (a_base, a_end, b_base, b_end, b_end)
    } else {
      (static_end, static_end, static_end, static_end, static_end)
    };
    let min_bytes = (arena_base as u64) + 65_536;
    let initial_pages = 100u64.max(min_bytes.div_ceil(65_536));
    Self {
      static_end,
      space_a_base,
      space_a_end,
      space_b_base,
      space_b_end,
      arena_base,
      initial_pages,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PersistentHeapGlobals {
  pub active_space: u32,
  pub ptr: u32,
  pub limit: u32,
  pub needs_compact: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ComponentMemoryGlobals {
  pub free_head: u32,
  pub total_live: u32,
  pub turn_mark: u32,
  pub live_above_mark: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WasmGlobalLayout {
  pub atom_count: u32,
  pub string_tag: u32,
  pub arena_base: u32,
  pub active_turns: u32,
  pub persistent: Option<PersistentHeapGlobals>,
  pub component: Option<ComponentMemoryGlobals>,
  pub number_stack: u32,
}

impl WasmGlobalLayout {
  pub fn compute(atom_count: u32, is_component: bool) -> Self {
    let mut next = 1 + atom_count;
    let string_tag = next;
    next += 1;
    let arena_base = next;
    next += 1;
    let active_turns = next;
    next += 1;
    let persistent = if atom_count > 0 {
      let g = PersistentHeapGlobals {
        active_space: next,
        ptr: next + 1,
        limit: next + 2,
        needs_compact: next + 3,
      };
      next += 4;
      Some(g)
    } else {
      None
    };
    let component = if is_component {
      let g = ComponentMemoryGlobals {
        free_head: next,
        total_live: next + 1,
        turn_mark: next + 2,
        live_above_mark: next + 3,
      };
      next += 4;
      Some(g)
    } else {
      None
    };
    let number_stack = next;
    Self {
      atom_count,
      string_tag,
      arena_base,
      active_turns,
      persistent,
      component,
      number_stack,
    }
  }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PromoteTypeTags {
  pub string: i32,
  pub buffer: i32,
  pub list: i32,
  pub buf_list: i32,
  pub map: i32,
  pub set: i32,
  pub struct_tag: i32,
  pub enum_tag: i32,
  pub nil: i32,
  pub bool_tag: i32,
  pub number: i32,
}

fn emit_load_active_space_bounds(
  instructions: &mut Vec<Instruction<'static>>,
  memory: &WasmMemoryLayout,
  pers: &PersistentHeapGlobals,
  base_local: u32,
  end_local: Option<u32>,
) {
  instructions.extend([
    Instruction::GlobalGet(pers.active_space),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(memory.space_a_base),
    Instruction::LocalSet(base_local),
  ]);
  if let Some(end_local) = end_local {
    instructions.extend([Instruction::I32Const(memory.space_a_end), Instruction::LocalSet(end_local)]);
  }
  instructions.extend([
    Instruction::Else,
    Instruction::I32Const(memory.space_b_base),
    Instruction::LocalSet(base_local),
  ]);
  if let Some(end_local) = end_local {
    instructions.extend([Instruction::I32Const(memory.space_b_end), Instruction::LocalSet(end_local)]);
  }
  instructions.push(Instruction::End);
}

/// Returns `1` iff `val` (`f64`) is an integer pointing to a live heap header with `HEAP_MAGIC`.
pub(super) fn build_rt_is_heap_ptr_fn(memory: &WasmMemoryLayout, globals: &WasmGlobalLayout) -> CompiledFn {
  let ptr = 1u32;
  let heap_ptr = 2u32;
  let in_region = 3u32;
  let active_base = 4u32;
  let mut locals = vec![ValType::I32, ValType::I32, ValType::I32];
  if globals.persistent.is_some() {
    locals.push(ValType::I32);
  }
  let heap_min = (HEAP_BASE + 8) as f64;
  let mut instructions = vec![
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::LocalSet(heap_ptr),
    Instruction::LocalGet(0),
    Instruction::LocalGet(0),
    Instruction::F64Trunc,
    Instruction::F64Eq,
    Instruction::LocalGet(0),
    f64_const(heap_min),
    Instruction::F64Ge,
    Instruction::I32And,
    Instruction::LocalGet(0),
    Instruction::LocalGet(heap_ptr),
    Instruction::F64ConvertI32U,
    Instruction::F64Lt,
    Instruction::I32And,
    Instruction::If(BlockType::Result(ValType::I32)),
    Instruction::LocalGet(0),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(ptr),
  ];

  if let Some(pers) = &globals.persistent {
    emit_load_active_space_bounds(&mut instructions, memory, pers, active_base, None);
    instructions.extend([
      Instruction::LocalGet(ptr),
      Instruction::I32Const(memory.static_end),
      Instruction::I32LtU,
      Instruction::LocalGet(ptr),
      Instruction::LocalGet(active_base),
      Instruction::I32Const(8),
      Instruction::I32Add,
      Instruction::I32GeU,
      Instruction::LocalGet(ptr),
      Instruction::GlobalGet(pers.ptr),
      Instruction::I32LtU,
      Instruction::I32And,
      Instruction::I32Or,
      Instruction::LocalGet(ptr),
      Instruction::I32Const(memory.arena_base + 8),
      Instruction::I32GeU,
      Instruction::I32Or,
      Instruction::LocalSet(in_region),
      Instruction::LocalGet(in_region),
      Instruction::If(BlockType::Result(ValType::I32)),
      Instruction::LocalGet(ptr),
      Instruction::I32Const(8),
      Instruction::I32Sub,
      Instruction::I32Load(mem_arg_i32(0)),
      Instruction::I32Const(HEAP_MAGIC),
      Instruction::I32Eq,
      Instruction::Else,
      Instruction::I32Const(0),
      Instruction::End,
    ]);
  } else {
    instructions.extend([
      Instruction::LocalGet(ptr),
      Instruction::I32Const(memory.static_end),
      Instruction::I32LtU,
      Instruction::LocalGet(ptr),
      Instruction::I32Const(memory.arena_base + 8),
      Instruction::I32GeU,
      Instruction::I32Or,
      Instruction::If(BlockType::Result(ValType::I32)),
      Instruction::LocalGet(ptr),
      Instruction::I32Const(8),
      Instruction::I32Sub,
      Instruction::I32Load(mem_arg_i32(0)),
      Instruction::I32Const(HEAP_MAGIC),
      Instruction::I32Eq,
      Instruction::Else,
      Instruction::I32Const(0),
      Instruction::End,
    ]);
  }

  instructions.extend([Instruction::Else, Instruction::I32Const(0), Instruction::End]);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::I32],
    locals,
    instructions,
  }
}

/// Allocates an 8-byte aligned raw buffer of `byte_len` bytes in the Scratch Arena.
pub(super) fn build_rt_arena_alloc_fn() -> CompiledFn {
  // params: 0 = byte_len (i32)
  let raw = 1u32;
  let aligned_len = 2u32;
  let end = 3u32;
  let desired_pages = 4u32;
  let current_pages = 5u32;
  let instructions = vec![
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::LocalTee(raw),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(raw),
    Instruction::I32Const(-8),
    Instruction::I32And,
    Instruction::LocalSet(raw),
    Instruction::LocalGet(0),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::LocalTee(aligned_len),
    Instruction::LocalGet(0),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(aligned_len),
    Instruction::I32Const(-8),
    Instruction::I32And,
    Instruction::LocalSet(aligned_len),
    Instruction::LocalGet(raw),
    Instruction::LocalGet(aligned_len),
    Instruction::I32Add,
    Instruction::LocalTee(end),
    Instruction::LocalGet(raw),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(end),
    Instruction::I64ExtendI32U,
    Instruction::I64Const(65_535),
    Instruction::I64Add,
    Instruction::I64Const(16),
    Instruction::I64ShrU,
    Instruction::I32WrapI64,
    Instruction::LocalSet(desired_pages),
    Instruction::MemorySize(0),
    Instruction::LocalTee(current_pages),
    Instruction::LocalGet(desired_pages),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(desired_pages),
    Instruction::LocalGet(current_pages),
    Instruction::I32Sub,
    Instruction::MemoryGrow(0),
    Instruction::I32Const(-1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(end),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::LocalGet(raw),
  ];
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32],
    results: vec![ValType::I32],
    locals: vec![ValType::I32; 5],
    instructions,
  }
}

/// Allocates a tagged heap object inside the active persistent semi-space.
/// Returns the logical pointer (`raw + 8`), or `0` if capacity is exhausted.
pub(super) fn build_rt_persistent_alloc_fn(pers: &PersistentHeapGlobals) -> CompiledFn {
  // params: 0 = payload_bytes (i32), 1 = type_tag (i32)
  let aligned = 2u32;
  let total = 3u32;
  let raw = 4u32;
  let end = 5u32;
  let instructions = vec![
    Instruction::LocalGet(0),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::LocalTee(aligned),
    Instruction::LocalGet(0),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(aligned),
    Instruction::I32Const(-8),
    Instruction::I32And,
    Instruction::LocalTee(aligned),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalTee(total),
    Instruction::LocalGet(aligned),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::GlobalGet(pers.ptr),
    Instruction::LocalTee(raw),
    Instruction::LocalGet(total),
    Instruction::I32Add,
    Instruction::LocalTee(end),
    Instruction::LocalGet(raw),
    Instruction::I32LtU,
    Instruction::LocalGet(end),
    Instruction::GlobalGet(pers.limit),
    Instruction::I32GtU,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(end),
    Instruction::GlobalSet(pers.ptr),
    Instruction::LocalGet(raw),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(raw),
    Instruction::I32Const(4),
    Instruction::I32Add,
    Instruction::LocalGet(1),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(raw),
    Instruction::I32Const(8),
    Instruction::I32Add,
  ];
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::I32],
    locals: vec![ValType::I32; 4],
    instructions,
  }
}

struct PersistentForwardCtx<'a> {
  dst: u32,
  src_ptr: u32,
  can_forward: u32,
  fallback_return: u32,
  mode: u32,
  pers: &'a PersistentHeapGlobals,
  persistent_alloc_idx: u32,
}

fn emit_persistent_alloc_and_forward(
  instructions: &mut Vec<Instruction<'static>>,
  payload_bytes: u32,
  type_tag: i32,
  ctx: &PersistentForwardCtx<'_>,
) {
  instructions.extend([
    Instruction::LocalGet(payload_bytes),
    Instruction::I32Const(type_tag),
    Instruction::Call(ctx.persistent_alloc_idx),
    Instruction::LocalTee(ctx.dst),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ctx.mode),
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::Else,
    Instruction::I32Const(2),
    Instruction::GlobalSet(ctx.pers.needs_compact),
    Instruction::LocalGet(ctx.fallback_return),
    Instruction::Return,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(ctx.can_forward),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ctx.src_ptr),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::I32Const(HEAP_FORWARD_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(ctx.src_ptr),
    Instruction::I32Const(4),
    Instruction::I32Sub,
    Instruction::LocalGet(ctx.dst),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::End,
  ]);
}

/// Promotes or compacts an internal HAMT node (`table` or `bucket`, header tag `0`).
pub(super) fn build_rt_promote_map_node_fn(
  memory: &WasmMemoryLayout,
  pers: &PersistentHeapGlobals,
  persistent_alloc_idx: u32,
  promote_value_idx: u32,
  self_idx: u32,
) -> CompiledFn {
  // params: 0 = node_ptr (i32), 1 = mode (i32), 2 = old_base (i32), 3 = old_end (i32), 4 = depth (i32)
  let new_base = 5u32;
  let new_end = 6u32;
  let dst = 7u32;
  let kind = 8u32;
  let count = 9u32;
  let payload = 10u32;
  let i = 11u32;
  let child_ptr = 12u32;
  let can_forward = 13u32;
  let magic = 14u32;
  let fval = 15u32;

  let mut instructions = vec![
    Instruction::LocalGet(4),
    Instruction::I32Const(MAX_PROMOTE_DEPTH),
    Instruction::I32GtS,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::I32Const(0),
    Instruction::LocalSet(can_forward),
    Instruction::LocalGet(1),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(pers.needs_compact),
    Instruction::I32Const(2),
    Instruction::I32Eq,
    Instruction::LocalGet(0),
    Instruction::I32Const(memory.arena_base + 8),
    Instruction::I32LtU,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32GeU,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::Else,
    Instruction::LocalGet(0),
    Instruction::I32Const(memory.static_end),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
  ];
  emit_load_active_space_bounds(&mut instructions, memory, pers, new_base, Some(new_end));
  instructions.extend([
    Instruction::LocalGet(0),
    Instruction::LocalGet(new_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32GeU,
    Instruction::LocalGet(0),
    Instruction::LocalGet(new_end),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::LocalGet(2),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32GeU,
    Instruction::LocalGet(0),
    Instruction::LocalGet(3),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalTee(magic),
    Instruction::I32Const(HEAP_FORWARD_MAGIC),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::I32Const(4),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(magic),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::I32Const(1),
    Instruction::LocalSet(can_forward),
    Instruction::Else,
    Instruction::LocalGet(0),
    Instruction::I32Const(memory.arena_base + 8),
    Instruction::I32LtU,
    Instruction::LocalGet(0),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32GeU,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalTee(kind),
    f64_const(0.0),
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(33 * 8),
    Instruction::LocalSet(payload),
  ]);
  let fwd_ctx = PersistentForwardCtx {
    dst,
    src_ptr: 0,
    can_forward,
    fallback_return: 0,
    mode: 1,
    pers,
    persistent_alloc_idx,
  };
  emit_persistent_alloc_and_forward(&mut instructions, payload, 0, &fwd_ctx);
  instructions.extend([
    Instruction::LocalGet(dst),
    f64_const(0.0),
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::I32Const(0),
    Instruction::LocalSet(i),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(i),
    Instruction::I32Const(32),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalTee(fval),
    f64_const(0.0),
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(dst),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    f64_const(0.0),
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::Else,
    Instruction::LocalGet(fval),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(child_ptr),
    Instruction::LocalGet(dst),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalGet(child_ptr),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::LocalGet(4),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::Call(self_idx),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::End,
    Instruction::LocalGet(i),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(i),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(dst),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(kind),
    f64_const(1.0),
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(count),
    Instruction::I32Const(2),
    Instruction::I32Mul,
    Instruction::I32Const(2),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::LocalSet(payload),
  ]);
  let fwd_ctx = PersistentForwardCtx {
    dst,
    src_ptr: 0,
    can_forward,
    fallback_return: 0,
    mode: 1,
    pers,
    persistent_alloc_idx,
  };
  emit_persistent_alloc_and_forward(&mut instructions, payload, 0, &fwd_ctx);
  instructions.extend([
    Instruction::LocalGet(dst),
    f64_const(1.0),
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(dst),
    Instruction::LocalGet(count),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(8)),
    Instruction::I32Const(0),
    Instruction::LocalSet(i),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(i),
    Instruction::LocalGet(count),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(dst),
    Instruction::I32Const(16),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalGet(0),
    Instruction::I32Const(16),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::LocalGet(4),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::Call(promote_value_idx),
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(dst),
    Instruction::I32Const(24),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalGet(0),
    Instruction::I32Const(24),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::LocalGet(4),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::Call(promote_value_idx),
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(i),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(i),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(dst),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
  ]);

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32; 5],
    results: vec![ValType::I32],
    locals: vec![
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::F64,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::F64,
    ],
    instructions,
  }
}

/// Promotes or compacts a Calcit value (`f64`) into the active persistent semi-space.
pub(super) fn build_rt_promote_value_fn(
  memory: &WasmMemoryLayout,
  pers: &PersistentHeapGlobals,
  persistent_alloc_idx: u32,
  self_idx: u32,
  promote_map_node_idx: u32,
  tags: &PromoteTypeTags,
) -> CompiledFn {
  // params: 0 = val (f64), 1 = mode (i32), 2 = old_base (i32), 3 = old_end (i32), 4 = depth (i32)
  let ptr = 5u32;
  let magic = 6u32;
  let tag = 7u32;
  let dst = 8u32;
  let count = 9u32;
  let cap = 10u32;
  let payload = 11u32;
  let i = 12u32;
  let new_base = 13u32;
  let new_end = 14u32;
  let can_forward = 15u32;
  let root_ptr = 16u32;
  let fval = 17u32;

  let mut instructions = vec![
    Instruction::LocalGet(4),
    Instruction::I32Const(MAX_PROMOTE_DEPTH),
    Instruction::I32GtS,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::LocalGet(0),
    Instruction::F64Trunc,
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::I32Const(0),
    Instruction::LocalSet(can_forward),
    Instruction::LocalGet(1),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(pers.needs_compact),
    Instruction::I32Const(2),
    Instruction::I32Eq,
    Instruction::LocalGet(0),
    f64_const((memory.arena_base + 8) as f64),
    Instruction::F64Lt,
    Instruction::I32Or,
    Instruction::LocalGet(0),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::F64ConvertI32U,
    Instruction::F64Ge,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(ptr),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::Else,
    Instruction::LocalGet(0),
    f64_const(memory.static_end as f64),
    Instruction::F64Lt,
    Instruction::LocalGet(0),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::F64ConvertI32U,
    Instruction::F64Ge,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(ptr),
  ];
  emit_load_active_space_bounds(&mut instructions, memory, pers, new_base, Some(new_end));
  instructions.extend([
    Instruction::LocalGet(ptr),
    Instruction::LocalGet(new_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32GeU,
    Instruction::LocalGet(ptr),
    Instruction::LocalGet(new_end),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(ptr),
    Instruction::LocalGet(2),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32GeU,
    Instruction::LocalGet(ptr),
    Instruction::LocalGet(3),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalTee(magic),
    Instruction::I32Const(HEAP_FORWARD_MAGIC),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(4),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::F64ConvertI32U,
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(magic),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::I32Const(1),
    Instruction::LocalSet(can_forward),
    Instruction::Else,
    Instruction::LocalGet(ptr),
    Instruction::I32Const(memory.arena_base + 8),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(ptr),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(ptr),
    Instruction::I32Const(4),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalSet(tag),
  ]);

  let emit_copy_f64_elems = |instructions: &mut Vec<Instruction<'static>>, header_offset: i32| {
    instructions.extend([
      Instruction::I32Const(0),
      Instruction::LocalSet(i),
      Instruction::Block(BlockType::Empty),
      Instruction::Loop(BlockType::Empty),
      Instruction::LocalGet(i),
      Instruction::LocalGet(count),
      Instruction::I32GeU,
      Instruction::BrIf(1),
      Instruction::LocalGet(dst),
      Instruction::I32Const(header_offset),
      Instruction::I32Add,
      Instruction::LocalGet(i),
      Instruction::I32Const(8),
      Instruction::I32Mul,
      Instruction::I32Add,
      Instruction::LocalGet(ptr),
      Instruction::I32Const(header_offset),
      Instruction::I32Add,
      Instruction::LocalGet(i),
      Instruction::I32Const(8),
      Instruction::I32Mul,
      Instruction::I32Add,
      Instruction::F64Load(mem_arg_f64(0)),
      Instruction::LocalGet(1),
      Instruction::LocalGet(2),
      Instruction::LocalGet(3),
      Instruction::LocalGet(4),
      Instruction::I32Const(1),
      Instruction::I32Add,
      Instruction::Call(self_idx),
      Instruction::F64Store(mem_arg_f64(0)),
      Instruction::LocalGet(i),
      Instruction::I32Const(1),
      Instruction::I32Add,
      Instruction::LocalSet(i),
      Instruction::Br(0),
      Instruction::End,
      Instruction::End,
      Instruction::LocalGet(dst),
      Instruction::F64ConvertI32U,
      Instruction::Return,
    ]);
  };

  // String & Buffer
  instructions.extend([
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.string),
    Instruction::I32Eq,
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.buffer),
    Instruction::I32Eq,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(count),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::I32Const(-8),
    Instruction::I32And,
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(payload),
    Instruction::LocalGet(payload),
    Instruction::LocalGet(tag),
    Instruction::Call(persistent_alloc_idx),
    Instruction::LocalTee(dst),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(1),
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::Else,
    Instruction::I32Const(2),
    Instruction::GlobalSet(pers.needs_compact),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(can_forward),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::I32Const(HEAP_FORWARD_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(4),
    Instruction::I32Sub,
    Instruction::LocalGet(dst),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::End,
    Instruction::LocalGet(dst),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(8),
    Instruction::LocalGet(count),
    Instruction::I32Add,
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    Instruction::LocalGet(dst),
    Instruction::F64ConvertI32U,
    Instruction::Return,
    Instruction::End,
  ]);

  // Boxed nil / bool / number
  instructions.extend([
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.nil),
    Instruction::I32Eq,
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.bool_tag),
    Instruction::I32Eq,
    Instruction::I32Or,
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.number),
    Instruction::I32Eq,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(8),
    Instruction::LocalGet(tag),
    Instruction::Call(persistent_alloc_idx),
    Instruction::LocalTee(dst),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(1),
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::Else,
    Instruction::I32Const(2),
    Instruction::GlobalSet(pers.needs_compact),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(can_forward),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::I32Const(HEAP_FORWARD_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(4),
    Instruction::I32Sub,
    Instruction::LocalGet(dst),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::End,
    Instruction::LocalGet(dst),
    Instruction::LocalGet(ptr),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(dst),
    Instruction::F64ConvertI32U,
    Instruction::Return,
    Instruction::End,
  ]);

  let value_fwd_ctx = PersistentForwardCtx {
    dst,
    src_ptr: ptr,
    can_forward,
    fallback_return: 0,
    mode: 1,
    pers,
    persistent_alloc_idx,
  };

  // List & Set
  for kind_tag in [tags.list, tags.set] {
    instructions.extend([
      Instruction::LocalGet(tag),
      Instruction::I32Const(kind_tag),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(ptr),
      Instruction::F64Load(mem_arg_f64(0)),
      Instruction::I32TruncF64U,
      Instruction::LocalTee(count),
      Instruction::I32Const(1),
      Instruction::I32Add,
      Instruction::I32Const(8),
      Instruction::I32Mul,
      Instruction::LocalSet(payload),
    ]);
    emit_persistent_alloc_and_forward(&mut instructions, payload, kind_tag, &value_fwd_ctx);
    instructions.extend([
      Instruction::LocalGet(dst),
      Instruction::LocalGet(count),
      Instruction::F64ConvertI32U,
      Instruction::F64Store(mem_arg_f64(0)),
    ]);
    emit_copy_f64_elems(&mut instructions, 8);
    instructions.push(Instruction::End);
  }

  // BufList
  instructions.extend([
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.buf_list),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(cap),
    Instruction::I32Const(2),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::LocalSet(payload),
    Instruction::LocalGet(ptr),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(count),
  ]);
  emit_persistent_alloc_and_forward(&mut instructions, payload, tags.buf_list, &value_fwd_ctx);
  instructions.extend([
    Instruction::LocalGet(dst),
    Instruction::LocalGet(cap),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(dst),
    Instruction::LocalGet(count),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(8)),
  ]);
  emit_copy_f64_elems(&mut instructions, 16);
  instructions.push(Instruction::End);

  // Struct & Enum
  for kind_tag in [tags.struct_tag, tags.enum_tag] {
    instructions.extend([
      Instruction::LocalGet(tag),
      Instruction::I32Const(kind_tag),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(ptr),
      Instruction::F64Load(mem_arg_f64(0)),
      Instruction::I32TruncF64U,
      Instruction::LocalTee(count),
      Instruction::I32Const(2),
      Instruction::I32Add,
      Instruction::I32Const(8),
      Instruction::I32Mul,
      Instruction::LocalSet(payload),
    ]);
    emit_persistent_alloc_and_forward(&mut instructions, payload, kind_tag, &value_fwd_ctx);
    instructions.extend([
      Instruction::LocalGet(dst),
      Instruction::LocalGet(count),
      Instruction::F64ConvertI32U,
      Instruction::F64Store(mem_arg_f64(0)),
      Instruction::LocalGet(dst),
      Instruction::LocalGet(ptr),
      Instruction::F64Load(mem_arg_f64(8)),
      Instruction::F64Store(mem_arg_f64(8)),
    ]);
    emit_copy_f64_elems(&mut instructions, 16);
    instructions.push(Instruction::End);
  }

  // Map
  instructions.extend([
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.map),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(16),
    Instruction::LocalSet(payload),
  ]);
  emit_persistent_alloc_and_forward(&mut instructions, payload, tags.map, &value_fwd_ctx);
  instructions.extend([
    Instruction::LocalGet(dst),
    Instruction::LocalGet(ptr),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(ptr),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::LocalTee(fval),
    f64_const(0.0),
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(dst),
    f64_const(0.0),
    Instruction::F64Store(mem_arg_f64(8)),
    Instruction::Else,
    Instruction::LocalGet(fval),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(root_ptr),
    Instruction::LocalGet(dst),
    Instruction::LocalGet(root_ptr),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::LocalGet(4),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::Call(promote_map_node_idx),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(8)),
    Instruction::End,
    Instruction::LocalGet(dst),
    Instruction::F64ConvertI32U,
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
  ]);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::I32, ValType::I32, ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::F64,
    ],
    instructions,
  }
}

/// Copy-on-commit helper invoked by `reset!` before writing a `defref` global.
pub(super) fn build_rt_commit_defref_fn(memory: &WasmMemoryLayout, pers: &PersistentHeapGlobals, promote_value_idx: u32) -> CompiledFn {
  let promoted = 1u32;
  let active_base = 2u32;
  let mut instructions = vec![
    Instruction::LocalGet(0),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::Call(promote_value_idx),
    Instruction::LocalSet(promoted),
    Instruction::GlobalGet(pers.needs_compact),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
  ];
  emit_load_active_space_bounds(&mut instructions, memory, pers, active_base, None);
  instructions.extend([
    Instruction::GlobalGet(pers.ptr),
    Instruction::LocalGet(active_base),
    Instruction::I32Sub,
    Instruction::I32Const(PERSISTENT_COMPACT_THRESHOLD),
    Instruction::I32GeU,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::GlobalSet(pers.needs_compact),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(promoted),
  ]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![ValType::F64, ValType::I32],
    instructions,
  }
}

/// Evacuates all live `defref` graphs (and optional return value from `old_space`)
/// into the alternate semi-space at a turn-end safepoint, then zeroes the old space.
pub(super) fn build_rt_compact_persistent_fn(
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  pers: &PersistentHeapGlobals,
  promote_value_idx: u32,
) -> CompiledFn {
  // param: 0 = ret_val (f64)
  let old_base = 1u32;
  let old_end = 2u32;
  let new_space = 3u32;
  let new_base = 4u32;
  let new_end = 5u32;
  let old_used = 6u32;
  let res = 7u32;

  let mut instructions = vec![
    Instruction::LocalGet(0),
    Instruction::LocalSet(res),
    Instruction::GlobalGet(pers.active_space),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(memory.space_a_base),
    Instruction::LocalSet(old_base),
    Instruction::I32Const(memory.space_a_end),
    Instruction::LocalSet(old_end),
    Instruction::I32Const(1),
    Instruction::LocalSet(new_space),
    Instruction::I32Const(memory.space_b_base),
    Instruction::LocalSet(new_base),
    Instruction::I32Const(memory.space_b_end),
    Instruction::LocalSet(new_end),
    Instruction::Else,
    Instruction::I32Const(memory.space_b_base),
    Instruction::LocalSet(old_base),
    Instruction::I32Const(memory.space_b_end),
    Instruction::LocalSet(old_end),
    Instruction::I32Const(0),
    Instruction::LocalSet(new_space),
    Instruction::I32Const(memory.space_a_base),
    Instruction::LocalSet(new_base),
    Instruction::I32Const(memory.space_a_end),
    Instruction::LocalSet(new_end),
    Instruction::End,
    Instruction::GlobalGet(pers.ptr),
    Instruction::LocalGet(old_base),
    Instruction::I32Sub,
    Instruction::LocalSet(old_used),
    Instruction::LocalGet(new_space),
    Instruction::GlobalSet(pers.active_space),
    Instruction::LocalGet(new_base),
    Instruction::GlobalSet(pers.ptr),
    Instruction::LocalGet(new_end),
    Instruction::GlobalSet(pers.limit),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.needs_compact),
  ];

  for atom_global in 1..=globals.atom_count {
    instructions.extend([
      Instruction::GlobalGet(atom_global),
      Instruction::I32Const(1),
      Instruction::LocalGet(old_base),
      Instruction::LocalGet(old_end),
      Instruction::I32Const(0),
      Instruction::Call(promote_value_idx),
      Instruction::GlobalSet(atom_global),
    ]);
  }

  instructions.extend([
    Instruction::LocalGet(0),
    Instruction::LocalGet(0),
    Instruction::F64Trunc,
    Instruction::F64Eq,
    Instruction::LocalGet(0),
    Instruction::LocalGet(old_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::F64ConvertI32U,
    Instruction::F64Ge,
    Instruction::I32And,
    Instruction::LocalGet(0),
    Instruction::LocalGet(old_end),
    Instruction::F64ConvertI32U,
    Instruction::F64Lt,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::I32Const(1),
    Instruction::LocalGet(old_base),
    Instruction::LocalGet(old_end),
    Instruction::I32Const(0),
    Instruction::Call(promote_value_idx),
    Instruction::LocalSet(res),
    Instruction::End,
    Instruction::LocalGet(old_used),
    Instruction::I32Const(0),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(old_base),
    Instruction::I32Const(0),
    Instruction::LocalGet(old_used),
    Instruction::MemoryFill(0),
    Instruction::End,
    Instruction::LocalGet(res),
  ]);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::F64,
    ],
    instructions,
  }
}

/// Core WASM export epilogue: compacts persistent heap at outermost turn end and restores Scratch Arena watermark.
pub(super) fn build_rt_turn_end_fn(globals: &WasmGlobalLayout, compact_persistent_idx: Option<u32>) -> CompiledFn {
  // params: 0 = mark (i32), 1 = ret_val (f64)
  let mut instructions = vec![
    Instruction::GlobalGet(globals.active_turns),
    Instruction::I32Const(0),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(globals.active_turns),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::GlobalSet(globals.active_turns),
    Instruction::End,
    Instruction::GlobalGet(globals.active_turns),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
  ];
  if let (Some(pers), Some(compact_idx)) = (&globals.persistent, compact_persistent_idx) {
    instructions.extend([
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(1),
      Instruction::Call(compact_idx),
      Instruction::LocalSet(1),
      Instruction::End,
    ]);
  }
  instructions.extend([
    Instruction::LocalGet(0),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::End,
    Instruction::LocalGet(1),
  ]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::F64],
    results: vec![ValType::F64],
    locals: vec![],
    instructions,
  }
}

/// Host-callable explicit Scratch Arena reset for Core WASM loops that allocate via `__str_new`.
pub(super) fn build_arena_reset_fn(
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  compact_persistent_idx: Option<u32>,
) -> CompiledFn {
  let mut instructions = vec![
    Instruction::GlobalGet(globals.active_turns),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
  ];
  if let (Some(pers), Some(compact_idx)) = (&globals.persistent, compact_persistent_idx) {
    instructions.extend([
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::If(BlockType::Empty),
      f64_const(0.0),
      Instruction::Call(compact_idx),
      Instruction::Drop,
      Instruction::End,
    ]);
  }
  instructions.extend([
    Instruction::I32Const(memory.arena_base),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::End,
  ]);
  CompiledFn {
    export_name: Some("__arena_reset".into()),
    params: vec![],
    results: vec![],
    locals: vec![],
    instructions,
  }
}

pub(super) fn build_rt_component_turn_enter_fn(globals: &WasmGlobalLayout, comp: &ComponentMemoryGlobals) -> CompiledFn {
  let instructions = vec![
    Instruction::GlobalGet(globals.active_turns),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::GlobalSet(comp.turn_mark),
    Instruction::I32Const(0),
    Instruction::GlobalSet(comp.live_above_mark),
    Instruction::End,
    Instruction::GlobalGet(globals.active_turns),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::GlobalSet(globals.active_turns),
  ];
  CompiledFn {
    export_name: None,
    params: vec![],
    results: vec![],
    locals: vec![],
    instructions,
  }
}

pub(super) fn build_rt_component_turn_exit_fn(
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  comp: &ComponentMemoryGlobals,
  compact_persistent_idx: Option<u32>,
) -> CompiledFn {
  let curr = 0u32;
  let prev = 1u32;
  let next = 2u32;
  let mut instructions = vec![
    Instruction::GlobalGet(globals.active_turns),
    Instruction::I32Const(0),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(globals.active_turns),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::GlobalSet(globals.active_turns),
    Instruction::End,
    Instruction::GlobalGet(globals.active_turns),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
  ];
  if let (Some(pers), Some(compact_idx)) = (&globals.persistent, compact_persistent_idx) {
    instructions.extend([
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::If(BlockType::Empty),
      f64_const(0.0),
      Instruction::Call(compact_idx),
      Instruction::Drop,
      Instruction::End,
    ]);
  }
  instructions.extend([
    Instruction::GlobalGet(comp.total_live),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::GlobalSet(comp.free_head),
    Instruction::I32Const(memory.arena_base),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::Else,
    Instruction::GlobalGet(comp.turn_mark),
    Instruction::I32Const(0),
    Instruction::I32Ne,
    Instruction::GlobalGet(comp.live_above_mark),
    Instruction::I32Eqz,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(prev),
    Instruction::GlobalGet(comp.free_head),
    Instruction::LocalSet(curr),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(curr),
    Instruction::I32Eqz,
    Instruction::BrIf(1),
    Instruction::LocalGet(curr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(next),
    Instruction::LocalGet(curr),
    Instruction::GlobalGet(comp.turn_mark),
    Instruction::I32GeU,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(prev),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(next),
    Instruction::GlobalSet(comp.free_head),
    Instruction::Else,
    Instruction::LocalGet(prev),
    Instruction::LocalGet(next),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::End,
    Instruction::Else,
    Instruction::LocalGet(curr),
    Instruction::LocalSet(prev),
    Instruction::End,
    Instruction::LocalGet(next),
    Instruction::LocalSet(curr),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::GlobalGet(comp.turn_mark),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::End,
    Instruction::End,
    Instruction::I32Const(0),
    Instruction::GlobalSet(comp.turn_mark),
    Instruction::I32Const(0),
    Instruction::GlobalSet(comp.live_above_mark),
    Instruction::End,
  ]);
  CompiledFn {
    export_name: None,
    params: vec![],
    results: vec![],
    locals: vec![ValType::I32; 3],
    instructions,
  }
}
