use super::*;
use wasm_encoder::BlockType;

pub(super) const PERSISTENT_SEMI_SPACE_BYTES: i32 = 1024 * 1024;
pub(super) const PERSISTENT_COMPACT_THRESHOLD: i32 = PERSISTENT_SEMI_SPACE_BYTES / 2;
pub(super) const MAX_PROMOTE_DEPTH: i32 = 256;
const FWD_HASH_MUL: i32 = 0x9E37_79B1u32 as i32;
const FWD_INITIAL_CAP: i32 = 256;
const FWD_MAX_OLD_CAP: i32 = 0x0800_0000;

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
pub(super) struct NativeTurnGlobals {
  pub host_call_depth: u32,
  pub outer_turn_mark: u32,
  pub scratch_roots_head: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PersistentHeapGlobals {
  pub active_space: u32,
  pub ptr: u32,
  pub limit: u32,
  pub needs_compact: u32,
  pub next_compact_ptr: u32,
  pub stage_ptr: u32,
  pub stage_limit: u32,
  pub fwd_base: u32,
  pub fwd_cap: u32,
  pub fwd_len: u32,
  pub scratch_end: u32,
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
  pub native: Option<NativeTurnGlobals>,
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
    let native = if !is_component {
      let g = NativeTurnGlobals {
        host_call_depth: next,
        outer_turn_mark: next + 1,
        scratch_roots_head: next + 2,
      };
      next += 3;
      Some(g)
    } else {
      None
    };
    let persistent = if atom_count > 0 {
      let g = PersistentHeapGlobals {
        active_space: next,
        ptr: next + 1,
        limit: next + 2,
        needs_compact: next + 3,
        next_compact_ptr: next + 4,
        stage_ptr: next + 5,
        stage_limit: next + 6,
        fwd_base: next + 7,
        fwd_cap: next + 8,
        fwd_len: next + 9,
        scratch_end: next + 10,
      };
      next += 11;
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
      native,
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

fn emit_space_bounds_for_flag(
  instructions: &mut Vec<Instruction<'static>>,
  pers: &PersistentHeapGlobals,
  when_zero: (i32, i32),
  when_nonzero: (i32, i32),
  base_local: u32,
  end_local: Option<u32>,
) {
  instructions.extend([
    Instruction::GlobalGet(pers.active_space),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(when_zero.0),
    Instruction::LocalSet(base_local),
  ]);
  if let Some(end_local) = end_local {
    instructions.extend([Instruction::I32Const(when_zero.1), Instruction::LocalSet(end_local)]);
  }
  instructions.extend([
    Instruction::Else,
    Instruction::I32Const(when_nonzero.0),
    Instruction::LocalSet(base_local),
  ]);
  if let Some(end_local) = end_local {
    instructions.extend([Instruction::I32Const(when_nonzero.1), Instruction::LocalSet(end_local)]);
  }
  instructions.push(Instruction::End);
}

fn emit_load_active_space_bounds(
  instructions: &mut Vec<Instruction<'static>>,
  memory: &WasmMemoryLayout,
  pers: &PersistentHeapGlobals,
  base_local: u32,
  end_local: Option<u32>,
) {
  emit_space_bounds_for_flag(
    instructions,
    pers,
    (memory.space_a_base, memory.space_a_end),
    (memory.space_b_base, memory.space_b_end),
    base_local,
    end_local,
  );
}

fn emit_load_inactive_space_bounds(
  instructions: &mut Vec<Instruction<'static>>,
  memory: &WasmMemoryLayout,
  pers: &PersistentHeapGlobals,
  base_local: u32,
  end_local: Option<u32>,
) {
  emit_space_bounds_for_flag(
    instructions,
    pers,
    (memory.space_b_base, memory.space_b_end),
    (memory.space_a_base, memory.space_a_end),
    base_local,
    end_local,
  );
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

#[derive(Clone, Copy)]
pub(super) struct PersistentPromoteFns {
  pub persistent_alloc: u32,
  pub promote_value: u32,
  pub promote_map_node: u32,
  pub fwd_get: u32,
  pub fwd_put: u32,
}

#[derive(Clone, Copy)]
pub(super) struct RelocateScratchFns {
  pub promote_value: u32,
  pub promote_map_node: u32,
  pub fwd_get: u32,
  pub fwd_mark_flag: u32,
  pub relocate_refs: u32,
  pub relocate_map_node: u32,
}

fn emit_fwd_hash_slot(instructions: &mut Vec<Instruction<'static>>, src_local: u32, mask_local: u32, slot_local: u32) {
  instructions.extend([
    Instruction::LocalGet(src_local),
    Instruction::I32Const(3),
    Instruction::I32ShrU,
    Instruction::I32Const(FWD_HASH_MUL),
    Instruction::I32Mul,
    Instruction::LocalGet(mask_local),
    Instruction::I32And,
    Instruction::LocalSet(slot_local),
  ]);
}

/// Allocates or doubles the open-addressed forwarding hash table inside Scratch Arena.
pub(super) fn build_rt_fwd_grow_fn(pers: &PersistentHeapGlobals, arena_alloc_idx: u32) -> CompiledFn {
  let old_base = 0u32;
  let old_cap = 1u32;
  let new_cap = 2u32;
  let new_bytes = 3u32;
  let new_base = 4u32;
  let i = 5u32;
  let old_slot = 6u32;
  let k = 7u32;
  let v = 8u32;
  let slot = 9u32;
  let mask = 10u32;
  let new_slot = 11u32;
  let instructions = vec![
    Instruction::GlobalGet(pers.fwd_base),
    Instruction::LocalSet(old_base),
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::LocalTee(old_cap),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(FWD_INITIAL_CAP),
    Instruction::LocalSet(new_cap),
    Instruction::Else,
    Instruction::LocalGet(old_cap),
    Instruction::I32Const(FWD_MAX_OLD_CAP),
    Instruction::I32GeU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(old_cap),
    Instruction::I32Const(1),
    Instruction::I32Shl,
    Instruction::LocalSet(new_cap),
    Instruction::End,
    Instruction::LocalGet(new_cap),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::LocalTee(new_bytes),
    Instruction::Call(arena_alloc_idx),
    Instruction::LocalTee(new_base),
    Instruction::I32Const(0),
    Instruction::LocalGet(new_bytes),
    Instruction::MemoryFill(0),
    Instruction::LocalGet(new_base),
    Instruction::GlobalSet(pers.fwd_base),
    Instruction::LocalGet(new_cap),
    Instruction::GlobalSet(pers.fwd_cap),
    Instruction::LocalGet(old_cap),
    Instruction::I32Eqz,
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(new_cap),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::LocalSet(mask),
    Instruction::I32Const(0),
    Instruction::LocalSet(i),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(i),
    Instruction::LocalGet(old_cap),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(old_base),
    Instruction::LocalGet(i),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalTee(old_slot),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalTee(k),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(old_slot),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(v),
    Instruction::LocalGet(k),
    Instruction::I32Const(3),
    Instruction::I32ShrU,
    Instruction::I32Const(FWD_HASH_MUL),
    Instruction::I32Mul,
    Instruction::LocalGet(mask),
    Instruction::I32And,
    Instruction::LocalSet(slot),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(new_base),
    Instruction::LocalGet(slot),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalTee(new_slot),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(new_slot),
    Instruction::LocalGet(k),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(new_slot),
    Instruction::LocalGet(v),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::Br(2),
    Instruction::End,
    Instruction::LocalGet(slot),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalGet(mask),
    Instruction::I32And,
    Instruction::LocalSet(slot),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(i),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(i),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::End,
  ];
  CompiledFn {
    export_name: None,
    params: vec![],
    results: vec![],
    locals: vec![ValType::I32; 12],
    instructions,
  }
}

/// Looks up `src_ptr` in the forwarding table, returning the persistent destination (`val & !7`) or `0`.
pub(super) fn build_rt_fwd_get_fn(pers: &PersistentHeapGlobals) -> CompiledFn {
  // param: 0 = src_ptr (i32)
  let cap = 1u32;
  let base = 2u32;
  let mask = 3u32;
  let slot = 4u32;
  let addr = 5u32;
  let k = 6u32;
  let mut instructions = vec![
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::LocalTee(cap),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::GlobalGet(pers.fwd_base),
    Instruction::LocalSet(base),
    Instruction::LocalGet(cap),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::LocalSet(mask),
  ];
  emit_fwd_hash_slot(&mut instructions, 0, mask, slot);
  instructions.extend([
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(base),
    Instruction::LocalGet(slot),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalTee(addr),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalTee(k),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(k),
    Instruction::LocalGet(0),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(addr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Const(-8),
    Instruction::I32And,
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(slot),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalGet(mask),
    Instruction::I32And,
    Instruction::LocalSet(slot),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::I32Const(0),
  ]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32],
    results: vec![ValType::I32],
    locals: vec![ValType::I32; 6],
    instructions,
  }
}

/// Inserts or updates `(src_ptr -> dst_ptr)` while preserving the low 3 flag bits.
pub(super) fn build_rt_fwd_put_fn(pers: &PersistentHeapGlobals, fwd_grow_idx: u32) -> CompiledFn {
  // params: 0 = src_ptr (i32), 1 = dst_ptr (i32)
  let cap = 2u32;
  let base = 3u32;
  let mask = 4u32;
  let slot = 5u32;
  let addr = 6u32;
  let k = 7u32;
  let mut instructions = vec![
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::LocalTee(cap),
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(pers.fwd_base),
    Instruction::LocalSet(base),
    Instruction::LocalGet(cap),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::LocalSet(mask),
  ];
  emit_fwd_hash_slot(&mut instructions, 0, mask, slot);
  instructions.extend([
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(base),
    Instruction::LocalGet(slot),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalTee(addr),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalTee(k),
    Instruction::I32Eqz,
    Instruction::BrIf(1),
    Instruction::LocalGet(k),
    Instruction::LocalGet(0),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(addr),
    Instruction::LocalGet(addr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Const(7),
    Instruction::I32And,
    Instruction::LocalGet(1),
    Instruction::I32Or,
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(slot),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalGet(mask),
    Instruction::I32And,
    Instruction::LocalSet(slot),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::GlobalGet(pers.fwd_len),
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::I32Const(1),
    Instruction::I32ShrU,
    Instruction::I32GeU,
    Instruction::If(BlockType::Empty),
    Instruction::Call(fwd_grow_idx),
    Instruction::End,
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::LocalSet(mask),
    Instruction::GlobalGet(pers.fwd_base),
    Instruction::LocalSet(base),
  ]);
  emit_fwd_hash_slot(&mut instructions, 0, mask, slot);
  instructions.extend([
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(base),
    Instruction::LocalGet(slot),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalTee(addr),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(addr),
    Instruction::LocalGet(0),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(addr),
    Instruction::LocalGet(1),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::GlobalGet(pers.fwd_len),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::GlobalSet(pers.fwd_len),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(slot),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalGet(mask),
    Instruction::I32And,
    Instruction::LocalSet(slot),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
  ]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![],
    locals: vec![ValType::I32; 6],
    instructions,
  }
}

/// Tests and sets `flag_bit` (`1` or `2`) for `src_ptr`. Returns `1` if already set, `0` if newly marked.
/// Updating an existing entry never grows or allocates in Scratch Arena.
pub(super) fn build_rt_fwd_mark_flag_fn(pers: &PersistentHeapGlobals, fwd_grow_idx: u32) -> CompiledFn {
  // params: 0 = src_ptr (i32), 1 = flag_bit (i32)
  let cap = 2u32;
  let base = 3u32;
  let mask = 4u32;
  let slot = 5u32;
  let addr = 6u32;
  let k = 7u32;
  let v = 8u32;
  let mut instructions = vec![
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::LocalTee(cap),
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(pers.fwd_base),
    Instruction::LocalSet(base),
    Instruction::LocalGet(cap),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::LocalSet(mask),
  ];
  emit_fwd_hash_slot(&mut instructions, 0, mask, slot);
  instructions.extend([
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(base),
    Instruction::LocalGet(slot),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalTee(addr),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalTee(k),
    Instruction::I32Eqz,
    Instruction::BrIf(1),
    Instruction::LocalGet(k),
    Instruction::LocalGet(0),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(addr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalTee(v),
    Instruction::LocalGet(1),
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(addr),
    Instruction::LocalGet(v),
    Instruction::LocalGet(1),
    Instruction::I32Or,
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(slot),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalGet(mask),
    Instruction::I32And,
    Instruction::LocalSet(slot),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::GlobalGet(pers.fwd_len),
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::I32Const(1),
    Instruction::I32ShrU,
    Instruction::I32GeU,
    Instruction::If(BlockType::Empty),
    Instruction::Call(fwd_grow_idx),
    Instruction::End,
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::LocalSet(mask),
    Instruction::GlobalGet(pers.fwd_base),
    Instruction::LocalSet(base),
  ]);
  emit_fwd_hash_slot(&mut instructions, 0, mask, slot);
  instructions.extend([
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(base),
    Instruction::LocalGet(slot),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalTee(addr),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(addr),
    Instruction::LocalGet(0),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(addr),
    Instruction::LocalGet(1),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::GlobalGet(pers.fwd_len),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::GlobalSet(pers.fwd_len),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(slot),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalGet(mask),
    Instruction::I32And,
    Instruction::LocalSet(slot),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::I32Const(0),
  ]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::I32],
    locals: vec![ValType::I32; 7],
    instructions,
  }
}

/// Allocates a tagged heap object inside the active (`use_stage == 0`) or staged (`use_stage != 0`)
/// persistent semi-space. Returns the logical pointer (`raw + 8`), or `0` if capacity is exhausted.
pub(super) fn build_rt_persistent_alloc_fn(pers: &PersistentHeapGlobals) -> CompiledFn {
  // params: 0 = payload_bytes (i32), 1 = type_tag (i32), 2 = use_stage (i32)
  let aligned = 3u32;
  let total = 4u32;
  let raw = 5u32;
  let end = 6u32;
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
    Instruction::LocalGet(2),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
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
    Instruction::Else,
    Instruction::GlobalGet(pers.stage_ptr),
    Instruction::LocalTee(raw),
    Instruction::LocalGet(total),
    Instruction::I32Add,
    Instruction::LocalTee(end),
    Instruction::LocalGet(raw),
    Instruction::I32LtU,
    Instruction::LocalGet(end),
    Instruction::GlobalGet(pers.stage_limit),
    Instruction::I32GtU,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(end),
    Instruction::GlobalSet(pers.stage_ptr),
    Instruction::End,
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
    params: vec![ValType::I32, ValType::I32, ValType::I32],
    results: vec![ValType::I32],
    locals: vec![ValType::I32; 4],
    instructions,
  }
}

struct PersistentForwardCtx<'a> {
  dst: u32,
  src_ptr: u32,
  fallback_return: u32,
  mode: u32,
  pers: &'a PersistentHeapGlobals,
  persistent_alloc_idx: u32,
  fwd_put_idx: u32,
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
    Instruction::LocalGet(ctx.mode),
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
    Instruction::LocalGet(ctx.src_ptr),
    Instruction::LocalGet(ctx.dst),
    Instruction::Call(ctx.fwd_put_idx),
  ]);
}

/// Promotes or compacts an internal HAMT node (`table` or `bucket`, header tag `0`).
pub(super) fn build_rt_promote_map_node_fn(
  memory: &WasmMemoryLayout,
  pers: &PersistentHeapGlobals,
  fns: &PersistentPromoteFns,
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
  let existing = 13u32;
  let fval = 14u32;

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
    Instruction::LocalGet(0),
    Instruction::GlobalGet(pers.scratch_end),
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
    Instruction::Else,
    Instruction::LocalGet(0),
    Instruction::I32Const(memory.static_end),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
  ];
  emit_load_inactive_space_bounds(&mut instructions, memory, pers, new_base, Some(new_end));
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
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::Else,
    Instruction::LocalGet(0),
    Instruction::I32Const(memory.arena_base + 8),
    Instruction::I32LtU,
    Instruction::LocalGet(0),
    Instruction::GlobalGet(pers.scratch_end),
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
    Instruction::Call(fns.fwd_get),
    Instruction::LocalTee(existing),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(existing),
    Instruction::Return,
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
    fallback_return: 0,
    mode: 1,
    pers,
    persistent_alloc_idx: fns.persistent_alloc,
    fwd_put_idx: fns.fwd_put,
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
    Instruction::Call(fns.promote_map_node),
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
    Instruction::Call(fns.promote_value),
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
    Instruction::Call(fns.promote_value),
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
      ValType::F64,
    ],
    instructions,
  }
}

/// Promotes or compacts a Calcit value (`f64`) into the active or staged persistent semi-space,
/// deduplicating shared DAG nodes via the Scratch-Arena forwarding table.
pub(super) fn build_rt_promote_value_fn(
  memory: &WasmMemoryLayout,
  pers: &PersistentHeapGlobals,
  fns: &PersistentPromoteFns,
  tags: &PromoteTypeTags,
) -> CompiledFn {
  // params: 0 = val (f64), 1 = mode (i32), 2 = old_base (i32), 3 = old_end (i32), 4 = depth (i32)
  let ptr = 5u32;
  let existing = 6u32;
  let tag = 7u32;
  let dst = 8u32;
  let count = 9u32;
  let cap = 10u32;
  let payload = 11u32;
  let i = 12u32;
  let new_base = 13u32;
  let new_end = 14u32;
  let root_ptr = 15u32;
  let fval = 16u32;

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
    Instruction::GlobalGet(pers.scratch_end),
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
    Instruction::GlobalGet(pers.scratch_end),
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
  emit_load_inactive_space_bounds(&mut instructions, memory, pers, new_base, Some(new_end));
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
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
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
    Instruction::Call(fns.fwd_get),
    Instruction::LocalTee(existing),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(existing),
    Instruction::F64ConvertI32U,
    Instruction::Return,
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
      Instruction::Call(fns.promote_value),
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
    Instruction::LocalGet(1),
    Instruction::Call(fns.persistent_alloc),
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
    Instruction::LocalGet(1),
    Instruction::LocalGet(4),
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::LocalGet(dst),
    Instruction::Call(fns.fwd_put),
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
    Instruction::LocalGet(1),
    Instruction::Call(fns.persistent_alloc),
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
    Instruction::LocalGet(1),
    Instruction::LocalGet(4),
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::LocalGet(dst),
    Instruction::Call(fns.fwd_put),
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
    fallback_return: 0,
    mode: 1,
    pers,
    persistent_alloc_idx: fns.persistent_alloc,
    fwd_put_idx: fns.fwd_put,
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
    Instruction::Call(fns.promote_map_node),
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
      ValType::F64,
    ],
    instructions,
  }
}

/// Reclaims temporary forwarding-table workspace left above `pers.scratch_end` if an earlier
/// promotion or compaction trapped before cleanup.
fn emit_reclaim_abandoned_fwd_workspace(
  instructions: &mut Vec<Instruction<'static>>,
  memory: &WasmMemoryLayout,
  pers: &PersistentHeapGlobals,
) {
  instructions.extend([
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::I32Const(memory.arena_base),
    Instruction::I32GeU,
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::End,
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_ptr),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_limit),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_base),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_cap),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_len),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.scratch_end),
  ]);
}

/// Pops Core Scratch return root nodes at or above `mark_local` when rewinding `__heap_ptr`.
fn emit_pop_scratch_roots_above(
  instructions: &mut Vec<Instruction<'static>>,
  memory: &WasmMemoryLayout,
  native: &NativeTurnGlobals,
  mark_local: u32,
) {
  instructions.extend([
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::GlobalGet(native.scratch_roots_head),
    Instruction::LocalGet(mark_local),
    Instruction::I32LtU,
    Instruction::GlobalGet(native.scratch_roots_head),
    Instruction::I32Const(memory.arena_base),
    Instruction::I32LtU,
    Instruction::I32Or,
    Instruction::BrIf(1),
    Instruction::GlobalGet(native.scratch_roots_head),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::GlobalSet(native.scratch_roots_head),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
  ]);
}

struct ScratchRootsPass<'a> {
  memory: &'a WasmMemoryLayout,
  native: &'a NativeTurnGlobals,
  root_node: u32,
  saved_heap_ptr: u32,
  old_base: u32,
  old_end: u32,
  relocate_scratch_refs_idx: u32,
  do_patch: i32,
}

fn emit_relocate_scratch_roots_pass(instructions: &mut Vec<Instruction<'static>>, pass: &ScratchRootsPass<'_>) {
  instructions.extend([
    Instruction::GlobalGet(pass.native.scratch_roots_head),
    Instruction::LocalSet(pass.root_node),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(pass.root_node),
    Instruction::I32Const(pass.memory.arena_base),
    Instruction::I32LtU,
    Instruction::LocalGet(pass.root_node),
    Instruction::LocalGet(pass.saved_heap_ptr),
    Instruction::I32GeU,
    Instruction::I32Or,
    Instruction::BrIf(1),
    Instruction::LocalGet(pass.root_node),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::LocalGet(pass.old_base),
    Instruction::LocalGet(pass.old_end),
    Instruction::I32Const(0),
    Instruction::I32Const(pass.do_patch),
    Instruction::Call(pass.relocate_scratch_refs_idx),
    Instruction::LocalGet(pass.root_node),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalSet(pass.root_node),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
  ]);
}

/// Copy-on-commit helper invoked by `reset!` before writing a `defref` global.
pub(super) fn build_rt_commit_defref_fn(memory: &WasmMemoryLayout, pers: &PersistentHeapGlobals, promote_value_idx: u32) -> CompiledFn {
  // param: 0 = val (f64)
  let promoted = 1u32;
  let saved_heap_ptr = 2u32;
  let ptr_before = 3u32;
  let mut instructions = Vec::new();
  emit_reclaim_abandoned_fwd_workspace(&mut instructions, memory, pers);
  instructions.extend([
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::LocalTee(saved_heap_ptr),
    Instruction::GlobalSet(pers.scratch_end),
    Instruction::GlobalGet(pers.ptr),
    Instruction::LocalSet(ptr_before),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_base),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_cap),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_len),
    Instruction::LocalGet(0),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::Call(promote_value_idx),
    Instruction::LocalSet(promoted),
    Instruction::LocalGet(saved_heap_ptr),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_base),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_cap),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_len),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.scratch_end),
    Instruction::GlobalGet(pers.needs_compact),
    Instruction::I32Eqz,
    Instruction::GlobalGet(pers.ptr),
    Instruction::LocalGet(ptr_before),
    Instruction::I32GtU,
    Instruction::I32And,
    Instruction::GlobalGet(pers.ptr),
    Instruction::GlobalGet(pers.next_compact_ptr),
    Instruction::I32GeU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::GlobalSet(pers.needs_compact),
    Instruction::End,
    Instruction::LocalGet(promoted),
  ]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![ValType::F64, ValType::I32, ValType::I32],
    instructions,
  }
}

struct RelocateSlotLocals {
  addr: u32,
  old_base: u32,
  old_end: u32,
  depth: u32,
  do_patch: u32,
  fval: u32,
  reloc: u32,
}

fn emit_relocate_f64_slot(
  instructions: &mut Vec<Instruction<'static>>,
  memory: &WasmMemoryLayout,
  pers: &PersistentHeapGlobals,
  fns: &RelocateScratchFns,
  loc: &RelocateSlotLocals,
) {
  instructions.extend([
    Instruction::LocalGet(loc.addr),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalTee(loc.fval),
    Instruction::LocalGet(loc.fval),
    Instruction::F64Trunc,
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(loc.fval),
    Instruction::LocalGet(loc.old_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::F64ConvertI32U,
    Instruction::F64Ge,
    Instruction::LocalGet(loc.fval),
    Instruction::LocalGet(loc.old_end),
    Instruction::F64ConvertI32U,
    Instruction::F64Lt,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(loc.do_patch),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(loc.fval),
    Instruction::I32Const(1),
    Instruction::LocalGet(loc.old_base),
    Instruction::LocalGet(loc.old_end),
    Instruction::LocalGet(loc.depth),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::Call(fns.promote_value),
    Instruction::Drop,
    Instruction::Else,
    Instruction::LocalGet(loc.fval),
    Instruction::I32TruncF64U,
    Instruction::Call(fns.fwd_get),
    Instruction::LocalTee(loc.reloc),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(loc.addr),
    Instruction::LocalGet(loc.reloc),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::End,
    Instruction::End,
    Instruction::Else,
    Instruction::LocalGet(loc.fval),
    f64_const((memory.arena_base + 8) as f64),
    Instruction::F64Ge,
    Instruction::LocalGet(loc.fval),
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::F64ConvertI32U,
    Instruction::F64Lt,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(loc.fval),
    Instruction::LocalGet(loc.old_base),
    Instruction::LocalGet(loc.old_end),
    Instruction::LocalGet(loc.depth),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalGet(loc.do_patch),
    Instruction::Call(fns.relocate_refs),
    Instruction::End,
    Instruction::End,
    Instruction::End,
  ]);
}

/// Traverses a Scratch HAMT node (`table` or `bucket`), either evacuating `old_space` children (`do_patch == 0`)
/// or rewriting those child pointers in place to their staged destinations (`do_patch == 1`).
pub(super) fn build_rt_relocate_scratch_map_node_fn(
  memory: &WasmMemoryLayout,
  pers: &PersistentHeapGlobals,
  fns: &RelocateScratchFns,
) -> CompiledFn {
  // params: 0 = node_ptr (i32), 1 = old_base (i32), 2 = old_end (i32), 3 = depth (i32), 4 = do_patch (i32)
  let kind = 5u32;
  let count = 6u32;
  let i = 7u32;
  let fval = 8u32;
  let child_ptr = 9u32;
  let reloc = 10u32;
  let addr = 11u32;

  let mut instructions = vec![
    Instruction::LocalGet(3),
    Instruction::I32Const(MAX_PROMOTE_DEPTH),
    Instruction::I32GtS,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Const(memory.arena_base + 8),
    Instruction::I32LtU,
    Instruction::LocalGet(0),
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::I32GeU,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::LocalGet(4),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Result(ValType::I32)),
    Instruction::I32Const(1),
    Instruction::Else,
    Instruction::I32Const(2),
    Instruction::End,
    Instruction::Call(fns.fwd_mark_flag),
    Instruction::If(BlockType::Empty),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalTee(kind),
    f64_const(0.0),
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
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
    Instruction::LocalTee(addr),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalTee(fval),
    f64_const(0.0),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(fval),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(child_ptr),
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32GeU,
    Instruction::LocalGet(child_ptr),
    Instruction::LocalGet(2),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(4),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(child_ptr),
    Instruction::I32Const(1),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::Call(fns.promote_map_node),
    Instruction::Drop,
    Instruction::Else,
    Instruction::LocalGet(child_ptr),
    Instruction::Call(fns.fwd_get),
    Instruction::LocalTee(reloc),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(addr),
    Instruction::LocalGet(reloc),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::End,
    Instruction::End,
    Instruction::Else,
    Instruction::LocalGet(child_ptr),
    Instruction::I32Const(memory.arena_base + 8),
    Instruction::I32GeU,
    Instruction::LocalGet(child_ptr),
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(child_ptr),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalGet(4),
    Instruction::Call(fns.relocate_map_node),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(i),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(i),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(kind),
    f64_const(1.0),
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(count),
    Instruction::I32Const(0),
    Instruction::LocalSet(i),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(i),
    Instruction::LocalGet(count),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(0),
    Instruction::I32Const(16),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalSet(addr),
  ];
  let slot_locals = RelocateSlotLocals {
    addr,
    old_base: 1,
    old_end: 2,
    depth: 3,
    do_patch: 4,
    fval,
    reloc,
  };
  emit_relocate_f64_slot(&mut instructions, memory, pers, fns, &slot_locals);
  instructions.extend([
    Instruction::LocalGet(0),
    Instruction::I32Const(24),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalSet(addr),
  ]);
  emit_relocate_f64_slot(&mut instructions, memory, pers, fns, &slot_locals);
  instructions.extend([
    Instruction::LocalGet(i),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(i),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::End,
  ]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32; 5],
    results: vec![],
    locals: vec![
      ValType::F64,
      ValType::I32,
      ValType::I32,
      ValType::F64,
      ValType::I32,
      ValType::I32,
      ValType::I32,
    ],
    instructions,
  }
}

/// Traverses a Scratch container value, either evacuating reachable `old_space` children (`do_patch == 0`)
/// or updating those child slots in place (`do_patch == 1`).
pub(super) fn build_rt_relocate_scratch_refs_fn(
  memory: &WasmMemoryLayout,
  pers: &PersistentHeapGlobals,
  fns: &RelocateScratchFns,
  tags: &PromoteTypeTags,
) -> CompiledFn {
  // params: 0 = val (f64), 1 = old_base (i32), 2 = old_end (i32), 3 = depth (i32), 4 = do_patch (i32)
  let ptr = 5u32;
  let tag = 6u32;
  let count = 7u32;
  let i = 8u32;
  let fval = 9u32;
  let child_ptr = 10u32;
  let reloc = 11u32;
  let addr = 12u32;

  let mut instructions = vec![
    Instruction::LocalGet(3),
    Instruction::I32Const(MAX_PROMOTE_DEPTH),
    Instruction::I32GtS,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::LocalGet(0),
    Instruction::F64Trunc,
    Instruction::F64Ne,
    Instruction::LocalGet(0),
    f64_const((memory.arena_base + 8) as f64),
    Instruction::F64Lt,
    Instruction::I32Or,
    Instruction::LocalGet(0),
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::F64ConvertI32U,
    Instruction::F64Ge,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
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
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(ptr),
    Instruction::LocalGet(4),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Result(ValType::I32)),
    Instruction::I32Const(1),
    Instruction::Else,
    Instruction::I32Const(2),
    Instruction::End,
    Instruction::Call(fns.fwd_mark_flag),
    Instruction::If(BlockType::Empty),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(ptr),
    Instruction::I32Const(4),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalSet(tag),
  ];

  let emit_relocate_elem_loop = |instructions: &mut Vec<Instruction<'static>>, header_offset: i32| {
    instructions.extend([
      Instruction::I32Const(0),
      Instruction::LocalSet(i),
      Instruction::Block(BlockType::Empty),
      Instruction::Loop(BlockType::Empty),
      Instruction::LocalGet(i),
      Instruction::LocalGet(count),
      Instruction::I32GeU,
      Instruction::BrIf(1),
      Instruction::LocalGet(ptr),
      Instruction::I32Const(header_offset),
      Instruction::I32Add,
      Instruction::LocalGet(i),
      Instruction::I32Const(8),
      Instruction::I32Mul,
      Instruction::I32Add,
      Instruction::LocalSet(addr),
    ]);
    emit_relocate_f64_slot(
      instructions,
      memory,
      pers,
      fns,
      &RelocateSlotLocals {
        addr,
        old_base: 1,
        old_end: 2,
        depth: 3,
        do_patch: 4,
        fval,
        reloc,
      },
    );
    instructions.extend([
      Instruction::LocalGet(i),
      Instruction::I32Const(1),
      Instruction::I32Add,
      Instruction::LocalSet(i),
      Instruction::Br(0),
      Instruction::End,
      Instruction::End,
      Instruction::Return,
    ]);
  };

  for kind_tag in [tags.list, tags.set] {
    instructions.extend([
      Instruction::LocalGet(tag),
      Instruction::I32Const(kind_tag),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(ptr),
      Instruction::F64Load(mem_arg_f64(0)),
      Instruction::I32TruncF64U,
      Instruction::LocalSet(count),
    ]);
    emit_relocate_elem_loop(&mut instructions, 8);
    instructions.push(Instruction::End);
  }

  instructions.extend([
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.buf_list),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(count),
  ]);
  emit_relocate_elem_loop(&mut instructions, 16);
  instructions.push(Instruction::End);

  for kind_tag in [tags.struct_tag, tags.enum_tag] {
    instructions.extend([
      Instruction::LocalGet(tag),
      Instruction::I32Const(kind_tag),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(ptr),
      Instruction::F64Load(mem_arg_f64(0)),
      Instruction::I32TruncF64U,
      Instruction::LocalSet(count),
    ]);
    emit_relocate_elem_loop(&mut instructions, 16);
    instructions.push(Instruction::End);
  }

  instructions.extend([
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.map),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::LocalTee(fval),
    f64_const(0.0),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(fval),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(child_ptr),
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32GeU,
    Instruction::LocalGet(child_ptr),
    Instruction::LocalGet(2),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(4),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(child_ptr),
    Instruction::I32Const(1),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::Call(fns.promote_map_node),
    Instruction::Drop,
    Instruction::Else,
    Instruction::LocalGet(child_ptr),
    Instruction::Call(fns.fwd_get),
    Instruction::LocalTee(reloc),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(reloc),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::End,
    Instruction::End,
    Instruction::Else,
    Instruction::LocalGet(child_ptr),
    Instruction::I32Const(memory.arena_base + 8),
    Instruction::I32GeU,
    Instruction::LocalGet(child_ptr),
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(child_ptr),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalGet(4),
    Instruction::Call(fns.relocate_map_node),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
  ]);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::I32, ValType::I32, ValType::I32, ValType::I32],
    results: vec![],
    locals: vec![
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::F64,
      ValType::I32,
      ValType::I32,
      ValType::I32,
    ],
    instructions,
  }
}

/// Evacuates all live non-scalar `defref` graphs (and optional return value from `old_space` or
/// `old_space` children of a Scratch return value) into the alternate semi-space, commits state
/// atomically only after evacuation succeeds, and zeroes the old semi-space.
pub(super) fn build_rt_compact_persistent_fn(
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  pers: &PersistentHeapGlobals,
  promote_value_idx: u32,
  relocate_scratch_refs_idx: u32,
  atom_is_scalar: &[bool],
) -> CompiledFn {
  debug_assert_eq!(atom_is_scalar.len(), globals.atom_count as usize);
  // params: 0 = ret_val (f64), 1 = trace_ret (i32)
  let old_base = 2u32;
  let old_end = 3u32;
  let new_space = 4u32;
  let new_base = 5u32;
  let new_end = 6u32;
  let old_used = 7u32;
  let res = 8u32;
  let saved_heap_ptr = 9u32;
  let ret_is_scratch = 10u32;
  let new_ptr = 11u32;
  let root_node = 12u32;

  let mut locals = vec![
    ValType::I32, // old_base
    ValType::I32, // old_end
    ValType::I32, // new_space
    ValType::I32, // new_base
    ValType::I32, // new_end
    ValType::I32, // old_used
    ValType::F64, // res
    ValType::I32, // saved_heap_ptr
    ValType::I32, // ret_is_scratch
    ValType::I32, // new_ptr
    ValType::I32, // root_node
  ];
  let mut staged_atoms: Vec<(u32, u32)> = Vec::new();
  for (i, &is_scalar) in atom_is_scalar.iter().enumerate().take(globals.atom_count as usize) {
    if !is_scalar {
      let local_idx = 2 + locals.len() as u32;
      locals.push(ValType::F64);
      staged_atoms.push((1 + i as u32, local_idx));
    }
  }

  let mut instructions = Vec::new();
  emit_reclaim_abandoned_fwd_workspace(&mut instructions, memory, pers);
  instructions.extend([
    Instruction::LocalGet(0),
    Instruction::LocalSet(res),
    Instruction::I32Const(0),
    Instruction::LocalSet(ret_is_scratch),
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
    Instruction::LocalGet(new_base),
    Instruction::GlobalSet(pers.stage_ptr),
    Instruction::LocalGet(new_end),
    Instruction::GlobalSet(pers.stage_limit),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::LocalTee(saved_heap_ptr),
    Instruction::GlobalSet(pers.scratch_end),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_base),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_cap),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_len),
  ]);

  // Stage all live non-scalar defref globals before flipping active_space or ptr.
  for &(atom_global, stage_local) in &staged_atoms {
    instructions.extend([
      Instruction::GlobalGet(atom_global),
      Instruction::I32Const(1),
      Instruction::LocalGet(old_base),
      Instruction::LocalGet(old_end),
      Instruction::I32Const(0),
      Instruction::Call(promote_value_idx),
      Instruction::LocalSet(stage_local),
    ]);
  }

  // Optionally stage an old-space return value or Pass-0 evacuate old-space children of a Scratch return.
  instructions.extend([
    Instruction::LocalGet(1),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::LocalGet(0),
    Instruction::F64Trunc,
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::LocalGet(old_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::F64ConvertI32U,
    Instruction::F64Ge,
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
    Instruction::Else,
    Instruction::LocalGet(0),
    f64_const((memory.arena_base + 8) as f64),
    Instruction::F64Ge,
    Instruction::LocalGet(0),
    Instruction::LocalGet(saved_heap_ptr),
    Instruction::F64ConvertI32U,
    Instruction::F64Lt,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::LocalSet(ret_is_scratch),
    Instruction::LocalGet(0),
    Instruction::LocalGet(old_base),
    Instruction::LocalGet(old_end),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::Call(relocate_scratch_refs_idx),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
  ]);
  if let Some(native) = &globals.native {
    emit_relocate_scratch_roots_pass(
      &mut instructions,
      &ScratchRootsPass {
        memory,
        native,
        root_node,
        saved_heap_ptr,
        old_base,
        old_end,
        relocate_scratch_refs_idx,
        do_patch: 0,
      },
    );
  }

  // Atomic commit point: every evacuation succeeded, so publish the new roots and allocator state.
  for &(atom_global, stage_local) in &staged_atoms {
    instructions.extend([Instruction::LocalGet(stage_local), Instruction::GlobalSet(atom_global)]);
  }
  instructions.extend([
    Instruction::LocalGet(new_space),
    Instruction::GlobalSet(pers.active_space),
    Instruction::GlobalGet(pers.stage_ptr),
    Instruction::LocalTee(new_ptr),
    Instruction::GlobalSet(pers.ptr),
    Instruction::LocalGet(new_end),
    Instruction::GlobalSet(pers.limit),
    Instruction::LocalGet(new_ptr),
    Instruction::LocalGet(new_end),
    Instruction::LocalGet(new_ptr),
    Instruction::I32Sub,
    Instruction::I32Const(1),
    Instruction::I32ShrU,
    Instruction::I32Add,
    Instruction::GlobalSet(pers.next_compact_ptr),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.needs_compact),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_ptr),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_limit),
    // Pass 1: patch Scratch return children in place without growing or allocating.
    Instruction::LocalGet(ret_is_scratch),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::LocalGet(old_base),
    Instruction::LocalGet(old_end),
    Instruction::I32Const(0),
    Instruction::I32Const(1),
    Instruction::Call(relocate_scratch_refs_idx),
    Instruction::End,
  ]);
  if let Some(native) = &globals.native {
    emit_relocate_scratch_roots_pass(
      &mut instructions,
      &ScratchRootsPass {
        memory,
        native,
        root_node,
        saved_heap_ptr,
        old_base,
        old_end,
        relocate_scratch_refs_idx,
        do_patch: 1,
      },
    );
  }
  instructions.extend([
    // Restore Scratch Arena watermark over the temporary forwarding table and wipe old semi-space.
    Instruction::LocalGet(saved_heap_ptr),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_base),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_cap),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_len),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.scratch_end),
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
    params: vec![ValType::F64, ValType::I32],
    results: vec![ValType::F64],
    locals,
    instructions,
  }
}

/// Core WASM export prologue: recovers any trapped top-level turn when entering at `host_call_depth == 0`,
/// records `outer_turn_mark`, and returns the call's entry watermark.
pub(super) fn build_rt_turn_enter_fn(
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  native: &NativeTurnGlobals,
  compact_persistent_idx: Option<u32>,
) -> CompiledFn {
  // param: 0 = can_rewind_stale (i32)
  let mark = 1u32;
  let mut instructions = Vec::new();
  if let Some(pers) = &globals.persistent {
    emit_reclaim_abandoned_fwd_workspace(&mut instructions, memory, pers);
  }
  instructions.extend([
    Instruction::GlobalGet(native.host_call_depth),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(globals.active_turns),
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::GlobalSet(globals.active_turns),
    Instruction::GlobalGet(native.outer_turn_mark),
    Instruction::LocalTee(mark),
    Instruction::I32Const(memory.arena_base),
    Instruction::I32LtU,
    Instruction::LocalGet(mark),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32GtU,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(memory.arena_base),
    Instruction::LocalSet(mark),
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::If(BlockType::Empty),
  ]);
  if globals.persistent.is_some() {
    emit_pop_scratch_roots_above(&mut instructions, memory, native, mark);
  }
  if let (Some(pers), Some(compact_idx)) = (&globals.persistent, compact_persistent_idx) {
    instructions.extend([
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::If(BlockType::Empty),
      f64_const(0.0),
      Instruction::I32Const(0),
      Instruction::Call(compact_idx),
      Instruction::Drop,
      Instruction::End,
    ]);
  }
  instructions.extend([
    Instruction::LocalGet(mark),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::End,
    Instruction::LocalGet(mark),
    Instruction::GlobalSet(native.outer_turn_mark),
    Instruction::I32Const(1),
    Instruction::GlobalSet(globals.active_turns),
    Instruction::LocalGet(mark),
    Instruction::Return,
    Instruction::End,
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::LocalTee(mark),
    Instruction::GlobalSet(native.outer_turn_mark),
    Instruction::I32Const(1),
    Instruction::GlobalSet(globals.active_turns),
    Instruction::LocalGet(mark),
    Instruction::Return,
    Instruction::End,
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::LocalSet(mark),
    Instruction::GlobalGet(globals.active_turns),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::GlobalSet(globals.active_turns),
    Instruction::LocalGet(mark),
  ]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32],
    results: vec![ValType::I32],
    locals: vec![ValType::I32],
    instructions,
  }
}

/// Core WASM export epilogue: restores `active_turns` from `saved_turns`, runs compaction at outermost
/// turn end, and rewinds Scratch Arena unless `keep_scratch_result != 0` and `ret_val` is a newly
/// allocated Scratch Arena heap object in `[mark + 8, __heap_ptr)`.
pub(super) fn build_rt_turn_end_fn(
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  compact_persistent_idx: Option<u32>,
  arena_alloc_idx: u32,
) -> CompiledFn {
  // params: 0 = mark (i32), 1 = saved_turns (i32), 2 = keep_scratch_result (i32), 3 = ret_val (f64)
  let ptr = 4u32;
  let root_node = 5u32;
  let mut instructions = vec![
    Instruction::LocalGet(1),
    Instruction::GlobalSet(globals.active_turns),
    Instruction::GlobalGet(globals.active_turns),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
  ];
  if let (Some(pers), Some(compact_idx)) = (&globals.persistent, compact_persistent_idx) {
    instructions.extend([
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(3),
      Instruction::LocalGet(2),
      Instruction::Call(compact_idx),
      Instruction::LocalSet(3),
      Instruction::End,
    ]);
  }
  instructions.push(Instruction::End);

  // Per-call Scratch reclamation runs even when active_turns > 0 (e.g., nested callbacks or calls
  // after a trapped defwasm-import), skipped only when needs_compact == 2 indicates an uncommitted overflow root.
  if let Some(pers) = &globals.persistent {
    instructions.extend([
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::I32Const(2),
      Instruction::I32Ne,
      Instruction::If(BlockType::Empty),
    ]);
  }
  let emit_rewind = |insns: &mut Vec<Instruction<'static>>| {
    insns.extend([Instruction::LocalGet(0), Instruction::GlobalSet(HEAP_PTR_GLOBAL)]);
    if globals.persistent.is_some()
      && let Some(native) = &globals.native
    {
      emit_pop_scratch_roots_above(insns, memory, native, 0);
    }
  };
  instructions.extend([
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::LocalGet(3),
    Instruction::F64Trunc,
    Instruction::F64Eq,
    Instruction::I32And,
    Instruction::LocalGet(3),
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::F64ConvertI32U,
    Instruction::F64Ge,
    Instruction::I32And,
    Instruction::LocalGet(3),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::F64ConvertI32U,
    Instruction::F64Lt,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(ptr),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
  ]);
  if globals.persistent.is_some()
    && let Some(native) = &globals.native
  {
    instructions.extend([
      Instruction::I32Const(16),
      Instruction::Call(arena_alloc_idx),
      Instruction::LocalTee(root_node),
      Instruction::GlobalGet(native.scratch_roots_head),
      Instruction::I32Store(mem_arg_i32(0)),
      Instruction::LocalGet(root_node),
      Instruction::LocalGet(3),
      Instruction::F64Store(mem_arg_f64(8)),
      Instruction::LocalGet(root_node),
      Instruction::GlobalSet(native.scratch_roots_head),
    ]);
  }
  instructions.push(Instruction::Else);
  emit_rewind(&mut instructions);
  instructions.extend([Instruction::End, Instruction::Else]);
  emit_rewind(&mut instructions);
  instructions.push(Instruction::End);
  if globals.persistent.is_some() {
    instructions.push(Instruction::End);
  }
  instructions.push(Instruction::LocalGet(3));
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32, ValType::I32, ValType::F64],
    results: vec![ValType::F64],
    locals: vec![ValType::I32, ValType::I32],
    instructions,
  }
}

/// Host-callable Scratch Arena reset and post-trap recovery entry for Core WASM instances.
pub(super) fn build_arena_reset_fn(
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  native: &NativeTurnGlobals,
  compact_persistent_idx: Option<u32>,
) -> CompiledFn {
  let mut instructions = vec![
    Instruction::I32Const(0),
    Instruction::GlobalSet(globals.active_turns),
    Instruction::I32Const(0),
    Instruction::GlobalSet(native.host_call_depth),
    Instruction::I32Const(0),
    Instruction::GlobalSet(native.scratch_roots_head),
  ];
  if let Some(pers) = &globals.persistent {
    emit_reclaim_abandoned_fwd_workspace(&mut instructions, memory, pers);
  }
  if let (Some(pers), Some(compact_idx)) = (&globals.persistent, compact_persistent_idx) {
    instructions.extend([
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::If(BlockType::Empty),
      f64_const(0.0),
      Instruction::I32Const(0),
      Instruction::Call(compact_idx),
      Instruction::Drop,
      Instruction::End,
    ]);
  }
  instructions.extend([
    Instruction::I32Const(memory.arena_base),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(memory.arena_base),
    Instruction::GlobalSet(native.outer_turn_mark),
  ]);
  CompiledFn {
    export_name: Some("__arena_reset".into()),
    params: vec![],
    results: vec![],
    locals: vec![],
    instructions,
  }
}

pub(super) fn build_rt_component_turn_enter_fn(
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  comp: &ComponentMemoryGlobals,
) -> CompiledFn {
  let mut instructions = Vec::new();
  if let Some(pers) = &globals.persistent {
    emit_reclaim_abandoned_fwd_workspace(&mut instructions, memory, pers);
  }
  instructions.extend([
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
  ]);
  CompiledFn {
    export_name: None,
    params: vec![],
    results: vec![],
    locals: vec![],
    instructions,
  }
}

/// Unlinks all Component free-list nodes at or above `mark` and rewinds `__heap_ptr` to `mark`.
pub(super) fn build_rt_component_rewind_to_mark_fn(comp: &ComponentMemoryGlobals) -> CompiledFn {
  // param: 0 = mark (i32)
  let curr = 1u32;
  let prev = 2u32;
  let next = 3u32;
  let instructions = vec![
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
    Instruction::LocalGet(0),
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
    Instruction::LocalGet(0),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
  ];
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32],
    results: vec![],
    locals: vec![ValType::I32; 3],
    instructions,
  }
}

pub(super) fn build_rt_component_turn_exit_fn(
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  comp: &ComponentMemoryGlobals,
  rewind_to_mark_idx: u32,
  compact_persistent_idx: Option<u32>,
) -> CompiledFn {
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
      Instruction::I32Const(0),
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
    Instruction::GlobalGet(comp.turn_mark),
    Instruction::Call(rewind_to_mark_idx),
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
    locals: vec![],
    instructions,
  }
}
