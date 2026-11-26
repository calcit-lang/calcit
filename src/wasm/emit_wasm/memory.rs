use super::*;
use wasm_encoder::BlockType;

pub(super) const INITIAL_PERSISTENT_SEMI_SPACE_BYTES: i32 = 256 * 1024;
pub(super) const INITIAL_PERSISTENT_COMPACT_THRESHOLD: i32 = INITIAL_PERSISTENT_SEMI_SPACE_BYTES / 2;
const FWD_HASH_MUL: i32 = 0x9E37_79B1u32 as i32;
const FWD_INITIAL_CAP: i32 = 256;
const FWD_MAX_OLD_CAP: i32 = 0x0800_0000;
const WORK_INITIAL_CAP: i32 = 256;
const WORK_MAX_OLD_CAP: i32 = 0x0800_0000;

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
      let a_end = a_base + INITIAL_PERSISTENT_SEMI_SPACE_BYTES;
      let b_base = a_end;
      let b_end = b_base + INITIAL_PERSISTENT_SEMI_SPACE_BYTES;
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
  pub stage_base: u32,
  pub stage_ptr: u32,
  pub stage_limit: u32,
  pub stage_overflow: u32,
  pub semi_space_bytes: u32,
  pub fwd_base: u32,
  pub fwd_cap: u32,
  pub fwd_len: u32,
  pub work_base: u32,
  pub work_cap: u32,
  pub work_len: u32,
  pub workspace_end: u32,
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
        stage_base: next + 5,
        stage_ptr: next + 6,
        stage_limit: next + 7,
        stage_overflow: next + 8,
        semi_space_bytes: next + 9,
        fwd_base: next + 10,
        fwd_cap: next + 11,
        fwd_len: next + 12,
        work_base: next + 13,
        work_cap: next + 14,
        work_len: next + 15,
        workspace_end: next + 16,
        scratch_end: next + 17,
      };
      next += 18;
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
  pub dyn_trait: i32,
  pub closure: i32,
  pub nil: i32,
  pub bool_tag: i32,
  pub number: i32,
  pub any_box: i32,
}

fn emit_load_active_space_bounds(
  instructions: &mut Vec<Instruction<'static>>,
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  pers: &PersistentHeapGlobals,
  base_local: u32,
  end_local: Option<u32>,
) {
  instructions.extend([
    Instruction::GlobalGet(pers.active_space),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(memory.static_end),
    Instruction::LocalSet(base_local),
  ]);
  if let Some(end_local) = end_local {
    instructions.extend([
      Instruction::I32Const(memory.static_end),
      Instruction::GlobalGet(pers.semi_space_bytes),
      Instruction::I32Add,
      Instruction::LocalSet(end_local),
    ]);
  }
  instructions.extend([
    Instruction::Else,
    Instruction::I32Const(memory.static_end),
    Instruction::GlobalGet(pers.semi_space_bytes),
    Instruction::I32Add,
    Instruction::LocalSet(base_local),
  ]);
  if let Some(end_local) = end_local {
    instructions.extend([Instruction::GlobalGet(globals.arena_base), Instruction::LocalSet(end_local)]);
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
    emit_load_active_space_bounds(&mut instructions, memory, globals, pers, active_base, None);
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
      Instruction::GlobalGet(globals.arena_base),
      Instruction::I32Const(8),
      Instruction::I32Add,
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
      Instruction::GlobalGet(globals.arena_base),
      Instruction::I32Const(8),
      Instruction::I32Add,
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
  pub copy_shallow_value: u32,
  pub copy_shallow_map_node: u32,
  pub cheney_scan: u32,
  pub fwd_get: u32,
  pub fwd_put: u32,
}

#[derive(Clone, Copy)]
pub(super) struct RelocateScratchFns {
  pub promote_value: u32,
  pub promote_map_node: u32,
  pub fwd_get: u32,
  pub fwd_mark_flag: u32,
  pub work_push: u32,
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
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::GlobalSet(pers.workspace_end),
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

/// Allocates or doubles the explicit Scratch-relocation worklist stack inside Scratch Arena.
pub(super) fn build_rt_work_grow_fn(pers: &PersistentHeapGlobals, arena_alloc_idx: u32) -> CompiledFn {
  let old_base = 0u32;
  let old_cap = 1u32;
  let new_cap = 2u32;
  let new_bytes = 3u32;
  let new_base = 4u32;
  let instructions = vec![
    Instruction::GlobalGet(pers.work_base),
    Instruction::LocalSet(old_base),
    Instruction::GlobalGet(pers.work_cap),
    Instruction::LocalTee(old_cap),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(WORK_INITIAL_CAP),
    Instruction::LocalSet(new_cap),
    Instruction::Else,
    Instruction::LocalGet(old_cap),
    Instruction::I32Const(WORK_MAX_OLD_CAP),
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
    Instruction::LocalSet(new_base),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::GlobalSet(pers.workspace_end),
    Instruction::GlobalGet(pers.work_len),
    Instruction::I32Const(0),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(new_base),
    Instruction::LocalGet(old_base),
    Instruction::GlobalGet(pers.work_len),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    Instruction::End,
    Instruction::LocalGet(new_base),
    Instruction::GlobalSet(pers.work_base),
    Instruction::LocalGet(new_cap),
    Instruction::GlobalSet(pers.work_cap),
  ];
  CompiledFn {
    export_name: None,
    params: vec![],
    results: vec![],
    locals: vec![ValType::I32; 5],
    instructions,
  }
}

/// Pushes `(ptr, kind)` onto the dynamically growable Scratch-relocation worklist stack.
pub(super) fn build_rt_work_push_fn(pers: &PersistentHeapGlobals, work_grow_idx: u32) -> CompiledFn {
  // params: 0 = ptr (i32), 1 = kind (i32)
  let len = 2u32;
  let slot = 3u32;
  let instructions = vec![
    Instruction::GlobalGet(pers.work_len),
    Instruction::LocalTee(len),
    Instruction::GlobalGet(pers.work_cap),
    Instruction::I32GeU,
    Instruction::If(BlockType::Empty),
    Instruction::Call(work_grow_idx),
    Instruction::End,
    Instruction::GlobalGet(pers.work_base),
    Instruction::LocalGet(len),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalTee(slot),
    Instruction::LocalGet(0),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(slot),
    Instruction::LocalGet(1),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::LocalGet(len),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::GlobalSet(pers.work_len),
  ];
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![],
    locals: vec![ValType::I32; 2],
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
    Instruction::LocalGet(2),
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::GlobalSet(pers.stage_overflow),
    Instruction::End,
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
    Instruction::LocalGet(2),
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::GlobalSet(pers.stage_overflow),
    Instruction::End,
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
    Instruction::I32Const(1),
    Instruction::GlobalSet(pers.stage_overflow),
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

struct PromotablePtrCheck<'a> {
  memory: &'a WasmMemoryLayout,
  globals: &'a WasmGlobalLayout,
  pers: &'a PersistentHeapGlobals,
  ptr_local: u32,
  mode_local: u32,
  old_base_local: u32,
  old_end_local: u32,
  known_heap_local: Option<u32>,
  fallback_instr: Instruction<'static>,
}

/// Emits a range/magic check for `mode == 0` (Scratch) or `mode != 0` (stage + old_space + Scratch).
/// Assumes `ptr_local` holds the `u32` candidate pointer, and returns `fallback_instr` if outside
/// the promotable region or lacking `HEAP_MAGIC` (skipping `HEAP_MAGIC` probes when `known_heap_local` is non-zero).
fn emit_check_promotable_ptr(instructions: &mut Vec<Instruction<'static>>, check: PromotablePtrCheck<'_>) {
  let PromotablePtrCheck {
    memory,
    globals,
    pers,
    ptr_local,
    mode_local,
    old_base_local,
    old_end_local,
    known_heap_local,
    fallback_instr,
  } = check;
  let emit_magic_guard = |insns: &mut Vec<Instruction<'static>>| {
    if let Some(kh) = known_heap_local {
      insns.extend([Instruction::LocalGet(kh), Instruction::I32Eqz, Instruction::If(BlockType::Empty)]);
    }
    insns.extend([
      Instruction::LocalGet(ptr_local),
      Instruction::I32Const(8),
      Instruction::I32Sub,
      Instruction::I32Load(mem_arg_i32(0)),
      Instruction::I32Const(HEAP_MAGIC),
      Instruction::I32Ne,
      Instruction::If(BlockType::Empty),
      fallback_instr.clone(),
      Instruction::Return,
      Instruction::End,
    ]);
    if known_heap_local.is_some() {
      insns.push(Instruction::End);
    }
  };
  instructions.extend([
    Instruction::LocalGet(mode_local),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(pers.needs_compact),
    Instruction::I32Const(2),
    Instruction::I32Eq,
    Instruction::LocalGet(ptr_local),
    Instruction::GlobalGet(globals.arena_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32LtU,
    Instruction::I32Or,
    Instruction::LocalGet(ptr_local),
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::I32GeU,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    fallback_instr.clone(),
    Instruction::Return,
    Instruction::End,
  ]);
  emit_magic_guard(instructions);
  instructions.extend([
    Instruction::Else,
    Instruction::LocalGet(ptr_local),
    Instruction::I32Const(memory.static_end),
    Instruction::I32LtU,
    Instruction::If(BlockType::Empty),
    fallback_instr.clone(),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(ptr_local),
    Instruction::GlobalGet(pers.stage_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32GeU,
    Instruction::LocalGet(ptr_local),
    Instruction::GlobalGet(pers.stage_ptr),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    fallback_instr.clone(),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(ptr_local),
    Instruction::LocalGet(old_base_local),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32GeU,
    Instruction::LocalGet(ptr_local),
    Instruction::LocalGet(old_end_local),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
  ]);
  emit_magic_guard(instructions);
  instructions.extend([
    Instruction::Else,
    Instruction::LocalGet(ptr_local),
    Instruction::GlobalGet(globals.arena_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32LtU,
    Instruction::LocalGet(ptr_local),
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::I32GeU,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    fallback_instr.clone(),
    Instruction::Return,
    Instruction::End,
  ]);
  emit_magic_guard(instructions);
  instructions.extend([Instruction::End, Instruction::End]);
}

/// Shallow-copies a HAMT node (`table` or `bucket`, low-16 tag `0`) into the target space, packing
/// `kv_kind << 16` into the persistent header tag slot and recording its forwarding entry.
pub(super) fn build_rt_copy_shallow_map_node_fn(
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  pers: &PersistentHeapGlobals,
  fns: &PersistentPromoteFns,
) -> CompiledFn {
  // params: 0 = node_ptr (i32), 1 = mode (i32), 2 = old_base (i32), 3 = old_end (i32), 4 = kv_kind (i32)
  let existing = 5u32;
  let kind = 6u32;
  let payload = 7u32;
  let dst = 8u32;

  let mut instructions = vec![
    Instruction::LocalGet(0),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
  ];
  emit_check_promotable_ptr(
    &mut instructions,
    PromotablePtrCheck {
      memory,
      globals,
      pers,
      ptr_local: 0,
      mode_local: 1,
      old_base_local: 2,
      old_end_local: 3,
      known_heap_local: None,
      fallback_instr: Instruction::LocalGet(0),
    },
  );
  instructions.extend([
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
    Instruction::Else,
    Instruction::LocalGet(kind),
    f64_const(1.0),
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::I32TruncF64U,
    Instruction::I32Const(2),
    Instruction::I32Mul,
    Instruction::I32Const(2),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::LocalSet(payload),
    Instruction::Else,
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(payload),
    Instruction::LocalGet(4),
    Instruction::I32Const(16),
    Instruction::I32Shl,
    Instruction::LocalGet(1),
    Instruction::Call(fns.persistent_alloc),
    Instruction::LocalTee(dst),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(1),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(2),
    Instruction::GlobalSet(pers.needs_compact),
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::LocalGet(dst),
    Instruction::Call(fns.fwd_put),
    Instruction::LocalGet(dst),
    Instruction::LocalGet(0),
    Instruction::LocalGet(payload),
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    Instruction::LocalGet(dst),
  ]);

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32; 5],
    results: vec![ValType::I32],
    locals: vec![ValType::I32, ValType::F64, ValType::I32, ValType::I32],
    instructions,
  }
}

/// Shallow-copies a Calcit heap value into the target persistent/stage space and records its
/// forwarding entry. Outgoing child pointers remain unmodified until `__rt_cheney_scan` visits `dst`.
pub(super) fn build_rt_copy_shallow_value_fn(
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  pers: &PersistentHeapGlobals,
  fns: &PersistentPromoteFns,
  tags: &PromoteTypeTags,
  struct_layouts: &StructLayouts,
  enum_layouts: &EnumLayouts,
) -> CompiledFn {
  // params: 0 = val (f64), 1 = mode (i32), 2 = old_base (i32), 3 = old_end (i32), 4 = known_heap (i32)
  let ptr = 5u32;
  let existing = 6u32;
  let tag = 7u32;
  let payload = 8u32;
  let copy_bytes = 9u32;
  let dst = 10u32;
  let count = 11u32;
  let cap = 12u32;
  let desc_ptr = 13u32;

  let mut instructions = vec![
    Instruction::LocalGet(4),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::LocalGet(0),
    Instruction::F64Trunc,
    Instruction::F64Ne,
    Instruction::LocalGet(0),
    f64_const((HEAP_BASE + 8) as f64),
    Instruction::F64Lt,
    Instruction::I32Or,
    Instruction::LocalGet(0),
    f64_const(4_294_967_296.0),
    Instruction::F64Ge,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(ptr),
  ];
  emit_check_promotable_ptr(
    &mut instructions,
    PromotablePtrCheck {
      memory,
      globals,
      pers,
      ptr_local: ptr,
      mode_local: 1,
      old_base_local: 2,
      old_end_local: 3,
      known_heap_local: Some(4),
      fallback_instr: Instruction::LocalGet(0),
    },
  );
  instructions.extend([
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
    Instruction::Block(BlockType::Empty),
    // String & Buffer
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
    Instruction::LocalGet(count),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(copy_bytes),
    Instruction::Br(1),
    Instruction::End,
    // Boxed nil / bool / number
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
    Instruction::LocalSet(payload),
    Instruction::I32Const(8),
    Instruction::LocalSet(copy_bytes),
    Instruction::Br(1),
    Instruction::End,
    // AnyBox
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.any_box),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(16),
    Instruction::LocalSet(payload),
    Instruction::I32Const(16),
    Instruction::LocalSet(copy_bytes),
    Instruction::Br(1),
    Instruction::End,
    // List & Set: `[count:u32 @ 0][elem_kind:u32 @ 4]` + elements
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.list),
    Instruction::I32Eq,
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.set),
    Instruction::I32Eq,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::LocalTee(payload),
    Instruction::LocalSet(copy_bytes),
    Instruction::Br(1),
    Instruction::End,
    // BufList: `[cap:u32 @ 0][elem_kind:u32 @ 4][count:u32 @ 8][reserved:u32 @ 12]`
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.buf_list),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Const(2),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::LocalSet(payload),
    Instruction::LocalGet(ptr),
    Instruction::I32Load(mem_arg_i32(8)),
    Instruction::I32Const(2),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::LocalSet(copy_bytes),
    Instruction::Br(1),
    Instruction::End,
    // Enum: `[count:u16 @ 0][layout_id:u16 @ 2][tag_id:u32 @ 4]`
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.enum_tag),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
  ]);
  if enum_layouts.max_layout_id > 0 {
    instructions.extend([
      Instruction::LocalGet(ptr),
      Instruction::I32Load16U(mem_arg_i16(2)),
      Instruction::LocalTee(cap),
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(cap),
      Instruction::I32Const(enum_layouts.max_layout_id as i32),
      Instruction::I32GtU,
      Instruction::If(BlockType::Empty),
      Instruction::Unreachable,
      Instruction::End,
      Instruction::I32Const(enum_layouts.desc_table_base),
      Instruction::LocalGet(cap),
      Instruction::I32Const(12),
      Instruction::I32Mul,
      Instruction::I32Add,
      Instruction::I32Load(mem_arg_i32(0)),
      Instruction::LocalTee(payload),
      Instruction::LocalSet(copy_bytes),
      Instruction::Br(2),
      Instruction::End,
    ]);
  }
  instructions.extend([
    Instruction::LocalGet(ptr),
    Instruction::I32Load16U(mem_arg_i16(0)),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::LocalTee(payload),
    Instruction::LocalSet(copy_bytes),
    Instruction::Br(1),
    Instruction::End,
    // DynTrait
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.dyn_trait),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(16),
    Instruction::LocalSet(payload),
    Instruction::I32Const(16),
    Instruction::LocalSet(copy_bytes),
    Instruction::Br(1),
    Instruction::End,
    // Closure
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.closure),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Const(16),
    Instruction::I32Add,
    Instruction::LocalTee(payload),
    Instruction::LocalSet(copy_bytes),
    Instruction::Br(1),
    Instruction::End,
  ]);
  if struct_layouts.max_layout_id > 0 {
    instructions.extend([
      Instruction::LocalGet(tag),
      Instruction::I32Const(tags.struct_tag),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(ptr),
      Instruction::I32Load(mem_arg_i32(4)),
      Instruction::LocalTee(cap),
      Instruction::I32Const(1),
      Instruction::I32LtU,
      Instruction::LocalGet(cap),
      Instruction::I32Const(struct_layouts.max_layout_id as i32),
      Instruction::I32GtU,
      Instruction::I32Or,
      Instruction::If(BlockType::Empty),
      Instruction::Unreachable,
      Instruction::End,
      Instruction::I32Const(struct_layouts.desc_table_base),
      Instruction::LocalGet(cap),
      Instruction::I32Const(12),
      Instruction::I32Mul,
      Instruction::I32Add,
      Instruction::LocalSet(desc_ptr),
      Instruction::LocalGet(desc_ptr),
      Instruction::I32Load(mem_arg_i32(0)),
      Instruction::LocalTee(payload),
      Instruction::LocalSet(copy_bytes),
      Instruction::Br(1),
      Instruction::End,
    ]);
  }
  instructions.extend([
    // Map: `[count:u32 @ 0][kv_kind:u32 @ 4][root_ptr:i32 @ 8][reserved:i32 @ 12]`
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.map),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(16),
    Instruction::LocalSet(payload),
    Instruction::I32Const(16),
    Instruction::LocalSet(copy_bytes),
    Instruction::Br(1),
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End, // end size dispatch block
    Instruction::LocalGet(payload),
    Instruction::LocalGet(tag),
    Instruction::LocalGet(1),
    Instruction::Call(fns.persistent_alloc),
    Instruction::LocalTee(dst),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(1),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(2),
    Instruction::GlobalSet(pers.needs_compact),
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(ptr),
    Instruction::LocalGet(dst),
    Instruction::Call(fns.fwd_put),
    Instruction::LocalGet(dst),
    Instruction::LocalGet(ptr),
    Instruction::LocalGet(copy_bytes),
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    Instruction::LocalGet(dst),
    Instruction::F64ConvertI32U,
  ]);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::I32, ValType::I32, ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![ValType::I32; 9],
    instructions,
  }
}

/// Iterative Cheney BFS scanner (`scan_raw` chasing `pers.ptr` or `pers.stage_ptr`) with O(1) WASM stack.
pub(super) fn build_rt_cheney_scan_fn(
  pers: &PersistentHeapGlobals,
  fns: &PersistentPromoteFns,
  tags: &PromoteTypeTags,
  struct_layouts: &StructLayouts,
  enum_layouts: &EnumLayouts,
) -> CompiledFn {
  // params: 0 = scan_raw (i32), 1 = mode (i32), 2 = old_base (i32), 3 = old_end (i32)
  let alloc_ptr = 4u32;
  let obj_ptr = 5u32;
  let tag = 6u32;
  let aligned_payload = 7u32;
  let count = 8u32;
  let cap = 9u32;
  let i = 10u32;
  let slot_addr = 11u32;
  let fval = 12u32;
  let child_ptr = 13u32;
  let desc_ptr = 14u32;
  let meta_ptr = 15u32;
  let packed = 16u32;
  let field_offset = 17u32;
  let known_heap_slot = 18u32;
  let kv_kind = 19u32;
  let key_kind = 20u32;
  let val_kind = 21u32;

  let emit_return_if_overflow = |insns: &mut Vec<Instruction<'static>>| {
    insns.extend([
      Instruction::LocalGet(1),
      Instruction::I32Eqz,
      Instruction::If(BlockType::Result(ValType::I32)),
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::I32Const(2),
      Instruction::I32Eq,
      Instruction::Else,
      Instruction::GlobalGet(pers.stage_overflow),
      Instruction::I32Const(0),
      Instruction::I32Ne,
      Instruction::End,
      Instruction::If(BlockType::Empty),
      Instruction::Return,
      Instruction::End,
    ]);
  };

  let emit_scan_f64_slot_at = |insns: &mut Vec<Instruction<'static>>, known_heap_local: Option<u32>| {
    insns.extend([
      Instruction::LocalGet(slot_addr),
      Instruction::F64Load(mem_arg_f64(0)),
      Instruction::LocalGet(1),
      Instruction::LocalGet(2),
      Instruction::LocalGet(3),
    ]);
    if let Some(kh) = known_heap_local {
      insns.push(Instruction::LocalGet(kh));
    } else {
      insns.push(Instruction::I32Const(0));
    }
    insns.extend([Instruction::Call(fns.copy_shallow_value), Instruction::LocalSet(fval)]);
    emit_return_if_overflow(insns);
    insns.extend([
      Instruction::LocalGet(slot_addr),
      Instruction::LocalGet(fval),
      Instruction::F64Store(mem_arg_f64(0)),
    ]);
  };

  let emit_scan_f64_elems = |insns: &mut Vec<Instruction<'static>>, header_offset: i32, known_heap_local: Option<u32>| {
    insns.extend([
      Instruction::I32Const(0),
      Instruction::LocalSet(i),
      Instruction::Block(BlockType::Empty),
      Instruction::Loop(BlockType::Empty),
      Instruction::LocalGet(i),
      Instruction::LocalGet(count),
      Instruction::I32GeU,
      Instruction::BrIf(1),
      Instruction::LocalGet(obj_ptr),
      Instruction::I32Const(header_offset),
      Instruction::I32Add,
      Instruction::LocalGet(i),
      Instruction::I32Const(3),
      Instruction::I32Shl,
      Instruction::I32Add,
      Instruction::LocalSet(slot_addr),
    ]);
    emit_scan_f64_slot_at(insns, known_heap_local);
    insns.extend([
      Instruction::LocalGet(i),
      Instruction::I32Const(1),
      Instruction::I32Add,
      Instruction::LocalSet(i),
      Instruction::Br(0),
      Instruction::End,
      Instruction::End,
    ]);
  };

  let emit_scan_descriptor_fields =
    |insns: &mut Vec<Instruction<'static>>, desc_table_base: i32, max_layout_id: u32, read_layout_id: Instruction<'static>| {
      insns.extend([
        Instruction::LocalGet(obj_ptr),
        read_layout_id,
        Instruction::LocalTee(cap),
        Instruction::I32Const(1),
        Instruction::I32LtU,
        Instruction::LocalGet(cap),
        Instruction::I32Const(max_layout_id as i32),
        Instruction::I32GtU,
        Instruction::I32Or,
        Instruction::If(BlockType::Empty),
        Instruction::Unreachable,
        Instruction::End,
        Instruction::I32Const(desc_table_base),
        Instruction::LocalGet(cap),
        Instruction::I32Const(12),
        Instruction::I32Mul,
        Instruction::I32Add,
        Instruction::LocalTee(desc_ptr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::I32Const(7),
        Instruction::I32Add,
        Instruction::I32Const(-8),
        Instruction::I32And,
        Instruction::LocalSet(aligned_payload),
        Instruction::LocalGet(desc_ptr),
        Instruction::I32Load(mem_arg_i32(4)),
        Instruction::LocalSet(count),
        Instruction::LocalGet(desc_ptr),
        Instruction::I32Load(mem_arg_i32(8)),
        Instruction::LocalSet(meta_ptr),
        Instruction::I32Const(0),
        Instruction::LocalSet(i),
        Instruction::Block(BlockType::Empty),
        Instruction::Loop(BlockType::Empty),
        Instruction::LocalGet(i),
        Instruction::LocalGet(count),
        Instruction::I32GeU,
        Instruction::BrIf(1),
      ]);
      structs::push_load_struct_field_meta(insns, meta_ptr, i);
      insns.push(Instruction::LocalSet(packed));
      structs::push_extract_struct_field_offset(insns, packed);
      insns.push(Instruction::LocalSet(field_offset));
      structs::push_extract_struct_field_kind(insns, packed);
      insns.extend([
        Instruction::LocalTee(cap),
        Instruction::I32Eqz, // BoxedF64 == 0
        Instruction::If(BlockType::Empty),
        Instruction::LocalGet(obj_ptr),
        Instruction::LocalGet(field_offset),
        Instruction::I32Add,
        Instruction::LocalSet(slot_addr),
      ]);
      emit_scan_f64_slot_at(insns, None);
      insns.extend([
        Instruction::Else,
        Instruction::LocalGet(cap),
        Instruction::I32Const(StructFieldKind::HeapPtrI32 as i32),
        Instruction::I32Eq,
        Instruction::LocalGet(cap),
        Instruction::I32Const(StructFieldKind::OptionHeapPtrI32 as i32),
        Instruction::I32Eq,
        Instruction::I32Or,
        Instruction::If(BlockType::Empty),
        Instruction::LocalGet(obj_ptr),
        Instruction::LocalGet(field_offset),
        Instruction::I32Add,
        Instruction::LocalTee(slot_addr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalTee(child_ptr),
        Instruction::If(BlockType::Empty),
        Instruction::LocalGet(child_ptr),
        Instruction::F64ConvertI32U,
        Instruction::LocalGet(1),
        Instruction::LocalGet(2),
        Instruction::LocalGet(3),
        Instruction::I32Const(1),
        Instruction::Call(fns.copy_shallow_value),
        Instruction::LocalSet(fval),
      ]);
      emit_return_if_overflow(insns);
      insns.extend([
        Instruction::LocalGet(slot_addr),
        Instruction::LocalGet(fval),
        Instruction::I32TruncF64U,
        Instruction::I32Store(mem_arg_i32(0)),
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
      ]);
    };

  let emit_non_scalar_container_kind = |insns: &mut Vec<Instruction<'static>>, kind_local: u32| {
    insns.extend([
      Instruction::LocalGet(kind_local),
      Instruction::I32Const(heap::ContainerSlotKind::PureScalar as i32),
      Instruction::I32Ne,
      Instruction::LocalGet(kind_local),
      Instruction::I32Const(heap::ContainerSlotKind::EmptyUntyped as i32),
      Instruction::I32Ne,
      Instruction::I32And,
    ]);
  };

  let mut instructions = vec![Instruction::Block(BlockType::Empty), Instruction::Loop(BlockType::Empty)];
  emit_return_if_overflow(&mut instructions);
  instructions.extend([
    Instruction::LocalGet(1),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Result(ValType::I32)),
    Instruction::GlobalGet(pers.ptr),
    Instruction::Else,
    Instruction::GlobalGet(pers.stage_ptr),
    Instruction::End,
    Instruction::LocalSet(alloc_ptr),
    Instruction::LocalGet(0),
    Instruction::LocalGet(alloc_ptr),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(obj_ptr),
    Instruction::LocalGet(0),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(tag),
    Instruction::Block(BlockType::Empty),
    // MapNode ((tag & 0xFFFF) == 0, kv_kind = tag >>> 16)
    Instruction::LocalGet(tag),
    Instruction::I32Const(0xFFFF),
    Instruction::I32And,
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(tag),
    Instruction::I32Const(16),
    Instruction::I32ShrU,
    Instruction::LocalSet(kv_kind),
    Instruction::LocalGet(obj_ptr),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalTee(fval),
    f64_const(0.0),
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(33 * 8),
    Instruction::LocalSet(aligned_payload),
    Instruction::I32Const(0),
    Instruction::LocalSet(i),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(i),
    Instruction::I32Const(32),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalTee(slot_addr),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalTee(fval),
    f64_const(0.0),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(fval),
    Instruction::I32TruncF64U,
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::LocalGet(kv_kind),
    Instruction::Call(fns.copy_shallow_map_node),
    Instruction::LocalSet(child_ptr),
  ]);
  emit_return_if_overflow(&mut instructions);
  instructions.extend([
    Instruction::LocalGet(slot_addr),
    Instruction::LocalGet(child_ptr),
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
    Instruction::Br(2),
    Instruction::Else,
    Instruction::LocalGet(fval),
    f64_const(1.0),
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(obj_ptr),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(count),
    Instruction::I32Const(2),
    Instruction::I32Mul,
    Instruction::I32Const(2),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::LocalSet(aligned_payload),
    Instruction::LocalGet(kv_kind),
    Instruction::I32Const(3),
    Instruction::I32And,
    Instruction::LocalSet(key_kind),
    Instruction::LocalGet(kv_kind),
    Instruction::I32Const(2),
    Instruction::I32ShrU,
    Instruction::I32Const(3),
    Instruction::I32And,
    Instruction::LocalSet(val_kind),
  ]);
  emit_non_scalar_container_kind(&mut instructions, key_kind);
  emit_non_scalar_container_kind(&mut instructions, val_kind);
  instructions.extend([
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(i),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(i),
    Instruction::LocalGet(count),
    Instruction::I32GeU,
    Instruction::BrIf(1),
  ]);
  emit_non_scalar_container_kind(&mut instructions, key_kind);
  instructions.extend([
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(key_kind),
    Instruction::I32Const(heap::ContainerSlotKind::ExactHeapPtr as i32),
    Instruction::I32Eq,
    Instruction::LocalSet(known_heap_slot),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Const(16),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalSet(slot_addr),
  ]);
  emit_scan_f64_slot_at(&mut instructions, Some(known_heap_slot));
  instructions.push(Instruction::End);
  emit_non_scalar_container_kind(&mut instructions, val_kind);
  instructions.extend([
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(val_kind),
    Instruction::I32Const(heap::ContainerSlotKind::ExactHeapPtr as i32),
    Instruction::I32Eq,
    Instruction::LocalSet(known_heap_slot),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Const(24),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalSet(slot_addr),
  ]);
  emit_scan_f64_slot_at(&mut instructions, Some(known_heap_slot));
  instructions.extend([
    Instruction::End,
    Instruction::LocalGet(i),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(i),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::Br(3),
    Instruction::Else,
    Instruction::Unreachable,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    // String & Buffer
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.string),
    Instruction::I32Eq,
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.buffer),
    Instruction::I32Eq,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(obj_ptr),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::I32Const(-8),
    Instruction::I32And,
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(aligned_payload),
    Instruction::Br(1),
    Instruction::End,
    // Boxed nil / bool / number
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
    Instruction::LocalSet(aligned_payload),
    Instruction::Br(1),
    Instruction::End,
    // AnyBox
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.any_box),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(16),
    Instruction::LocalSet(aligned_payload),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Const(1),
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(slot_addr),
  ]);
  emit_scan_f64_slot_at(&mut instructions, None);
  instructions.extend([
    Instruction::End,
    Instruction::Br(1),
    Instruction::End,
    // List & Set: `[count:u32 @ 0][elem_kind:u32 @ 4]`
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.list),
    Instruction::I32Eq,
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.set),
    Instruction::I32Eq,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalTee(count),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::LocalSet(aligned_payload),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(kv_kind),
  ]);
  emit_non_scalar_container_kind(&mut instructions, kv_kind);
  instructions.extend([
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(kv_kind),
    Instruction::I32Const(heap::ContainerSlotKind::ExactHeapPtr as i32),
    Instruction::I32Eq,
    Instruction::LocalSet(known_heap_slot),
  ]);
  emit_scan_f64_elems(&mut instructions, 8, Some(known_heap_slot));
  instructions.extend([
    Instruction::End,
    Instruction::Br(1),
    Instruction::End,
    // BufList: `[cap:u32 @ 0][elem_kind:u32 @ 4][count:u32 @ 8][reserved:u32 @ 12]`
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.buf_list),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Const(2),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::LocalSet(aligned_payload),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Load(mem_arg_i32(8)),
    Instruction::LocalSet(count),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(kv_kind),
  ]);
  emit_non_scalar_container_kind(&mut instructions, kv_kind);
  instructions.extend([
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(kv_kind),
    Instruction::I32Const(heap::ContainerSlotKind::ExactHeapPtr as i32),
    Instruction::I32Eq,
    Instruction::LocalSet(known_heap_slot),
  ]);
  emit_scan_f64_elems(&mut instructions, 16, Some(known_heap_slot));
  instructions.extend([
    Instruction::End,
    Instruction::Br(1),
    Instruction::End,
    // Enum: `[count:u16 @ 0][layout_id:u16 @ 2][tag_id:u32 @ 4]`
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.enum_tag),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
  ]);
  if enum_layouts.max_layout_id > 0 {
    instructions.extend([
      Instruction::LocalGet(obj_ptr),
      Instruction::I32Load16U(mem_arg_i16(2)),
      Instruction::I32Eqz,
      Instruction::If(BlockType::Empty),
    ]);
  }
  instructions.extend([
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Load16U(mem_arg_i16(0)),
    Instruction::LocalTee(count),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::LocalSet(aligned_payload),
  ]);
  emit_scan_f64_elems(&mut instructions, 8, None);
  if enum_layouts.max_layout_id > 0 {
    instructions.push(Instruction::Else);
    emit_scan_descriptor_fields(
      &mut instructions,
      enum_layouts.desc_table_base,
      enum_layouts.max_layout_id,
      Instruction::I32Load16U(mem_arg_i16(2)),
    );
    instructions.push(Instruction::End);
  }
  instructions.extend([
    Instruction::Br(1),
    Instruction::End,
    // DynTrait
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.dyn_trait),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(16),
    Instruction::LocalSet(aligned_payload),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Const(super::dyn_trait::DYN_TRAIT_HEAP_FLAG as i32),
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(slot_addr),
  ]);
  emit_scan_f64_slot_at(&mut instructions, None);
  instructions.extend([
    Instruction::End,
    Instruction::Br(1),
    Instruction::End,
    // Closure
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.closure),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalTee(count),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Const(16),
    Instruction::I32Add,
    Instruction::LocalSet(aligned_payload),
    Instruction::I32Const(0),
    Instruction::LocalSet(i),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(i),
    Instruction::LocalGet(count),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(obj_ptr),
    Instruction::I64Load(mem_arg_i64(8)),
    Instruction::LocalGet(i),
    Instruction::I64ExtendI32U,
    Instruction::I64ShrU,
    Instruction::I64Const(1),
    Instruction::I64And,
    Instruction::I64Const(0),
    Instruction::I64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Const(16),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalSet(slot_addr),
  ]);
  emit_scan_f64_slot_at(&mut instructions, None);
  instructions.extend([
    Instruction::End,
    Instruction::LocalGet(i),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(i),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::Br(1),
    Instruction::End,
  ]);
  if struct_layouts.max_layout_id > 0 {
    instructions.extend([
      Instruction::LocalGet(tag),
      Instruction::I32Const(tags.struct_tag),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
    ]);
    emit_scan_descriptor_fields(
      &mut instructions,
      struct_layouts.desc_table_base,
      struct_layouts.max_layout_id,
      Instruction::I32Load(mem_arg_i32(4)),
    );
    instructions.extend([Instruction::Br(1), Instruction::End]);
  }
  instructions.extend([
    // Map: `[count:u32 @ 0][kv_kind:u32 @ 4][root_ptr:i32 @ 8][reserved:i32 @ 12]`
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.map),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(16),
    Instruction::LocalSet(aligned_payload),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Load(mem_arg_i32(8)),
    Instruction::LocalTee(child_ptr),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(child_ptr),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::LocalGet(obj_ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::Call(fns.copy_shallow_map_node),
    Instruction::LocalSet(child_ptr),
  ]);
  emit_return_if_overflow(&mut instructions);
  instructions.extend([
    Instruction::LocalGet(obj_ptr),
    Instruction::LocalGet(child_ptr),
    Instruction::I32Store(mem_arg_i32(8)),
    Instruction::End,
    Instruction::Br(1),
    Instruction::End,
    Instruction::Unreachable,
    Instruction::End, // end tag dispatch block
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(aligned_payload),
    Instruction::I32Add,
    Instruction::LocalSet(0),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
  ]);

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32; 4],
    results: vec![],
    locals: vec![
      ValType::I32, // alloc_ptr
      ValType::I32, // obj_ptr
      ValType::I32, // tag
      ValType::I32, // aligned_payload
      ValType::I32, // count
      ValType::I32, // cap
      ValType::I32, // i
      ValType::I32, // slot_addr
      ValType::F64, // fval
      ValType::I32, // child_ptr
      ValType::I32, // desc_ptr
      ValType::I32, // meta_ptr
      ValType::I32, // packed
      ValType::I32, // field_offset
      ValType::I32, // known_heap_slot
      ValType::I32, // kv_kind
      ValType::I32, // key_kind
      ValType::I32, // val_kind
    ],
    instructions,
  }
}

/// Promotes or compacts an internal HAMT node (`table` or `bucket`, low-16 tag `0`) using iterative Cheney BFS.
pub(super) fn build_rt_promote_map_node_fn(pers: &PersistentHeapGlobals, fns: &PersistentPromoteFns) -> CompiledFn {
  // params: 0 = node_ptr (i32), 1 = mode (i32), 2 = old_base (i32), 3 = old_end (i32), 4 = kv_kind (i32)
  let scan_raw = 5u32;
  let res = 6u32;
  let instructions = vec![
    Instruction::LocalGet(1),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Result(ValType::I32)),
    Instruction::GlobalGet(pers.ptr),
    Instruction::Else,
    Instruction::GlobalGet(pers.stage_ptr),
    Instruction::End,
    Instruction::LocalSet(scan_raw),
    Instruction::LocalGet(0),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::LocalGet(4),
    Instruction::Call(fns.copy_shallow_map_node),
    Instruction::LocalSet(res),
    Instruction::LocalGet(scan_raw),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::Call(fns.cheney_scan),
    Instruction::LocalGet(res),
  ];
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32; 5],
    results: vec![ValType::I32],
    locals: vec![ValType::I32; 2],
    instructions,
  }
}

/// Promotes or compacts a Calcit value (`f64`) into the active or staged persistent semi-space
/// using iterative Cheney BFS (`scan_raw` chasing `pers.ptr` / `pers.stage_ptr`) with O(1) WASM stack.
pub(super) fn build_rt_promote_value_fn(pers: &PersistentHeapGlobals, fns: &PersistentPromoteFns) -> CompiledFn {
  // params: 0 = val (f64), 1 = mode (i32), 2 = old_base (i32), 3 = old_end (i32), 4 = known_heap (i32)
  let scan_raw = 5u32;
  let res = 6u32;
  let instructions = vec![
    Instruction::LocalGet(1),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Result(ValType::I32)),
    Instruction::GlobalGet(pers.ptr),
    Instruction::Else,
    Instruction::GlobalGet(pers.stage_ptr),
    Instruction::End,
    Instruction::LocalSet(scan_raw),
    Instruction::LocalGet(0),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::LocalGet(4),
    Instruction::Call(fns.copy_shallow_value),
    Instruction::LocalSet(res),
    Instruction::LocalGet(scan_raw),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::Call(fns.cheney_scan),
    Instruction::LocalGet(res),
  ];
  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::I32, ValType::I32, ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![ValType::I32, ValType::F64],
    instructions,
  }
}

/// Clears persistent forwarding, worklist, and staging metadata without altering `__heap_ptr`.
fn emit_clear_fwd_state(instructions: &mut Vec<Instruction<'static>>, pers: &PersistentHeapGlobals) {
  instructions.extend([
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_base),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_ptr),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_limit),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_overflow),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_base),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_cap),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_len),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.work_base),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.work_cap),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.work_len),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.workspace_end),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.scratch_end),
  ]);
}

/// Reclaims temporary forwarding/worklist/staging workspace above `pers.scratch_end` only when
/// it is still at the top of Scratch Arena with no subsequent host allocations (`__str_new` / `cabi_realloc`).
fn emit_reclaim_abandoned_fwd_workspace(
  instructions: &mut Vec<Instruction<'static>>,
  globals: &WasmGlobalLayout,
  pers: &PersistentHeapGlobals,
) {
  instructions.extend([
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::GlobalGet(globals.arena_base),
    Instruction::I32GeU,
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::I32Const(0),
    Instruction::I32Ne,
    Instruction::GlobalGet(pers.work_cap),
    Instruction::I32Const(0),
    Instruction::I32Ne,
    Instruction::I32Or,
    Instruction::GlobalGet(pers.workspace_end),
    Instruction::I32Const(0),
    Instruction::I32Ne,
    Instruction::I32Or,
    Instruction::I32And,
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::GlobalGet(pers.workspace_end),
    Instruction::I32Eq,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::End,
  ]);
  emit_clear_fwd_state(instructions, pers);
}

/// Pops Core Scratch return root nodes at or above `mark_local` when rewinding `__heap_ptr`.
fn emit_pop_scratch_roots_above(
  instructions: &mut Vec<Instruction<'static>>,
  globals: &WasmGlobalLayout,
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
    Instruction::GlobalGet(globals.arena_base),
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
  globals: &'a WasmGlobalLayout,
  native: &'a NativeTurnGlobals,
  pers: &'a PersistentHeapGlobals,
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
    Instruction::GlobalGet(pass.globals.arena_base),
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
    Instruction::I32Const(pass.do_patch),
    Instruction::Call(pass.relocate_scratch_refs_idx),
  ]);
  if pass.do_patch == 0 {
    instructions.extend([Instruction::GlobalGet(pass.pers.stage_overflow), Instruction::BrIf(1)]);
  }
  instructions.extend([
    Instruction::LocalGet(pass.root_node),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalSet(pass.root_node),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
  ]);
}

/// Copy-on-commit helper invoked by `reset!` before writing a `defref` global.
pub(super) fn build_rt_commit_defref_fn(
  globals: &WasmGlobalLayout,
  pers: &PersistentHeapGlobals,
  promote_value_idx: u32,
) -> CompiledFn {
  // param: 0 = val (f64)
  let promoted = 1u32;
  let saved_heap_ptr = 2u32;
  let ptr_before = 3u32;
  let mut instructions = Vec::new();
  emit_reclaim_abandoned_fwd_workspace(&mut instructions, globals, pers);
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
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.workspace_end),
    Instruction::LocalGet(0),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::Call(promote_value_idx),
    Instruction::LocalSet(promoted),
    Instruction::LocalGet(saved_heap_ptr),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
  ]);
  emit_clear_fwd_state(&mut instructions, pers);
  instructions.extend([
    Instruction::GlobalGet(pers.needs_compact),
    Instruction::I32Const(2),
    Instruction::I32Ne,
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
  do_patch: u32,
  fval: u32,
  child_ptr: u32,
  reloc: u32,
  bit: u32,
  known_heap: Option<u32>,
}

fn emit_relocate_f64_slot_worklist(
  instructions: &mut Vec<Instruction<'static>>,
  globals: &WasmGlobalLayout,
  pers: &PersistentHeapGlobals,
  fns: &RelocateScratchFns,
  loc: &RelocateSlotLocals,
) {
  let push_known_heap = |insns: &mut Vec<Instruction<'static>>| {
    if let Some(kh) = loc.known_heap {
      insns.push(Instruction::LocalGet(kh));
    } else {
      insns.push(Instruction::I32Const(0));
    }
  };
  instructions.extend([
    Instruction::LocalGet(loc.addr),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalSet(loc.fval),
  ]);
  if let Some(kh) = loc.known_heap {
    instructions.extend([
      Instruction::LocalGet(kh),
      Instruction::I32Const(0),
      Instruction::I32Ne,
      Instruction::LocalGet(loc.fval),
      Instruction::LocalGet(loc.fval),
      Instruction::F64Trunc,
      Instruction::F64Eq,
      Instruction::LocalGet(loc.fval),
      f64_const((HEAP_BASE + 8) as f64),
      Instruction::F64Ge,
      Instruction::I32And,
      Instruction::LocalGet(loc.fval),
      f64_const(4_294_967_296.0),
      Instruction::F64Lt,
      Instruction::I32And,
      Instruction::I32Or,
    ]);
  } else {
    instructions.extend([
      Instruction::LocalGet(loc.fval),
      Instruction::LocalGet(loc.fval),
      Instruction::F64Trunc,
      Instruction::F64Eq,
      Instruction::LocalGet(loc.fval),
      f64_const((HEAP_BASE + 8) as f64),
      Instruction::F64Ge,
      Instruction::I32And,
      Instruction::LocalGet(loc.fval),
      f64_const(4_294_967_296.0),
      Instruction::F64Lt,
      Instruction::I32And,
    ]);
  }
  instructions.extend([
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(loc.fval),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(loc.child_ptr),
    Instruction::LocalGet(loc.old_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32GeU,
    Instruction::LocalGet(loc.child_ptr),
    Instruction::LocalGet(loc.old_end),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(loc.do_patch),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(loc.fval),
    Instruction::I32Const(1),
    Instruction::LocalGet(loc.old_base),
    Instruction::LocalGet(loc.old_end),
  ]);
  push_known_heap(instructions);
  instructions.extend([
    Instruction::Call(fns.promote_value),
    Instruction::Drop,
    Instruction::Else,
    Instruction::LocalGet(loc.child_ptr),
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
    Instruction::LocalGet(loc.child_ptr),
    Instruction::GlobalGet(globals.arena_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32GeU,
    Instruction::LocalGet(loc.child_ptr),
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
  ]);
  if let Some(kh) = loc.known_heap {
    instructions.extend([
      Instruction::LocalGet(kh),
      Instruction::I32Const(0),
      Instruction::I32Ne,
      Instruction::If(BlockType::Result(ValType::I32)),
      Instruction::I32Const(1),
      Instruction::Else,
      Instruction::LocalGet(loc.child_ptr),
      Instruction::I32Const(8),
      Instruction::I32Sub,
      Instruction::I32Load(mem_arg_i32(0)),
      Instruction::I32Const(HEAP_MAGIC),
      Instruction::I32Eq,
      Instruction::End,
    ]);
  } else {
    instructions.extend([
      Instruction::LocalGet(loc.child_ptr),
      Instruction::I32Const(8),
      Instruction::I32Sub,
      Instruction::I32Load(mem_arg_i32(0)),
      Instruction::I32Const(HEAP_MAGIC),
      Instruction::I32Eq,
    ]);
  }
  instructions.extend([
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(loc.child_ptr),
    Instruction::LocalGet(loc.bit),
    Instruction::Call(fns.fwd_mark_flag),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(loc.child_ptr),
  ]);
  if let Some(kh) = loc.known_heap {
    instructions.extend([Instruction::LocalGet(kh), Instruction::I32Const(1), Instruction::I32Shl]);
  } else {
    instructions.push(Instruction::I32Const(0));
  }
  instructions.extend([
    Instruction::Call(fns.work_push),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
  ]);
}

/// Traverses a Scratch container graph using an explicit growable worklist stack above `pers.scratch_end`
/// (O(1) WASM stack), either evacuating reachable `old_space` children (`do_patch == 0`) or rewriting
/// those child slots in place (`do_patch == 1`).
pub(super) fn build_rt_relocate_scratch_refs_fn(
  globals: &WasmGlobalLayout,
  pers: &PersistentHeapGlobals,
  fns: &RelocateScratchFns,
  tags: &PromoteTypeTags,
  struct_layouts: &StructLayouts,
  enum_layouts: &EnumLayouts,
) -> CompiledFn {
  // params: 0 = val (f64), 1 = old_base (i32), 2 = old_end (i32), 3 = do_patch (i32)
  let ptr = 4u32;
  let bit = 5u32;
  let kind = 6u32;
  let tag = 7u32;
  let count = 8u32;
  let i = 9u32;
  let fval = 10u32;
  let child_ptr = 11u32;
  let reloc = 12u32;
  let addr = 13u32;
  let desc_ptr = 14u32;
  let meta_ptr = 15u32;
  let field_kind = 16u32;
  let packed = 17u32;
  let len = 18u32;
  let slot = 19u32;
  let elem_kind = 20u32;
  let child_known_heap = 21u32;
  let kv_kind = 22u32;
  let key_kind = 23u32;
  let val_kind = 24u32;

  let slot_locals = RelocateSlotLocals {
    addr,
    old_base: 1,
    old_end: 2,
    do_patch: 3,
    fval,
    child_ptr,
    reloc,
    bit,
    known_heap: None,
  };

  let emit_non_scalar_container_kind = |insns: &mut Vec<Instruction<'static>>, kind_local: u32| {
    insns.extend([
      Instruction::LocalGet(kind_local),
      Instruction::I32Const(heap::ContainerSlotKind::PureScalar as i32),
      Instruction::I32Ne,
      Instruction::LocalGet(kind_local),
      Instruction::I32Const(heap::ContainerSlotKind::EmptyUntyped as i32),
      Instruction::I32Ne,
      Instruction::I32And,
    ]);
  };

  let emit_map_node_child_slot = |insns: &mut Vec<Instruction<'static>>| {
    insns.extend([
      Instruction::LocalGet(addr),
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
      Instruction::LocalGet(3),
      Instruction::I32Eqz,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(child_ptr),
      Instruction::I32Const(1),
      Instruction::LocalGet(1),
      Instruction::LocalGet(2),
      Instruction::LocalGet(kv_kind),
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
      Instruction::GlobalGet(globals.arena_base),
      Instruction::I32Const(8),
      Instruction::I32Add,
      Instruction::I32GeU,
      Instruction::LocalGet(child_ptr),
      Instruction::GlobalGet(pers.scratch_end),
      Instruction::I32LtU,
      Instruction::I32And,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(child_ptr),
      Instruction::I32Const(8),
      Instruction::I32Sub,
      Instruction::I32Load(mem_arg_i32(0)),
      Instruction::I32Const(HEAP_MAGIC),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(child_ptr),
      Instruction::LocalGet(bit),
      Instruction::Call(fns.fwd_mark_flag),
      Instruction::I32Eqz,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(child_ptr),
      Instruction::LocalGet(kind),
      Instruction::Call(fns.work_push),
      Instruction::End,
      Instruction::End,
      Instruction::End,
      Instruction::End,
      Instruction::End,
    ]);
  };

  let emit_relocate_elem_loop = |insns: &mut Vec<Instruction<'static>>, header_offset: i32, known_heap_local: Option<u32>| {
    let elem_slot_locals = RelocateSlotLocals {
      known_heap: known_heap_local,
      ..slot_locals
    };
    insns.extend([
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
    emit_relocate_f64_slot_worklist(insns, globals, pers, fns, &elem_slot_locals);
    insns.extend([
      Instruction::LocalGet(3),
      Instruction::I32Eqz,
      Instruction::GlobalGet(pers.stage_overflow),
      Instruction::I32And,
      Instruction::If(BlockType::Empty),
      Instruction::I32Const(0),
      Instruction::GlobalSet(pers.work_len),
      Instruction::Return,
      Instruction::End,
      Instruction::LocalGet(i),
      Instruction::I32Const(1),
      Instruction::I32Add,
      Instruction::LocalSet(i),
      Instruction::Br(0),
      Instruction::End,
      Instruction::End,
    ]);
  };

  let emit_relocate_descriptor_fields =
    |insns: &mut Vec<Instruction<'static>>, desc_table_base: i32, max_layout_id: u32, read_layout_id: Instruction<'static>| {
      insns.extend([
        Instruction::LocalGet(ptr),
        read_layout_id,
        Instruction::LocalTee(packed),
        Instruction::I32Const(1),
        Instruction::I32LtU,
        Instruction::LocalGet(packed),
        Instruction::I32Const(max_layout_id as i32),
        Instruction::I32GtU,
        Instruction::I32Or,
        Instruction::If(BlockType::Empty),
        Instruction::Unreachable,
        Instruction::End,
        Instruction::I32Const(desc_table_base),
        Instruction::LocalGet(packed),
        Instruction::I32Const(12),
        Instruction::I32Mul,
        Instruction::I32Add,
        Instruction::LocalTee(desc_ptr),
        Instruction::I32Load(mem_arg_i32(4)),
        Instruction::LocalSet(count),
        Instruction::LocalGet(desc_ptr),
        Instruction::I32Load(mem_arg_i32(8)),
        Instruction::LocalSet(meta_ptr),
        Instruction::I32Const(0),
        Instruction::LocalSet(i),
        Instruction::Block(BlockType::Empty),
        Instruction::Loop(BlockType::Empty),
        Instruction::LocalGet(i),
        Instruction::LocalGet(count),
        Instruction::I32GeU,
        Instruction::BrIf(1),
      ]);
      structs::push_load_struct_field_meta(insns, meta_ptr, i);
      insns.push(Instruction::LocalSet(packed));
      structs::push_extract_struct_field_offset(insns, packed);
      insns.extend([Instruction::LocalGet(ptr), Instruction::I32Add, Instruction::LocalSet(addr)]);
      structs::push_extract_struct_field_kind(insns, packed);
      insns.extend([
        Instruction::LocalTee(field_kind),
        Instruction::I32Eqz, // BoxedF64 == 0
        Instruction::If(BlockType::Empty),
      ]);
      emit_relocate_f64_slot_worklist(insns, globals, pers, fns, &slot_locals);
      insns.extend([
        Instruction::Else,
        Instruction::LocalGet(field_kind),
        Instruction::I32Const(StructFieldKind::HeapPtrI32 as i32),
        Instruction::I32Eq,
        Instruction::LocalGet(field_kind),
        Instruction::I32Const(StructFieldKind::OptionHeapPtrI32 as i32),
        Instruction::I32Eq,
        Instruction::I32Or,
        Instruction::If(BlockType::Empty),
        Instruction::LocalGet(addr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalTee(child_ptr),
        Instruction::If(BlockType::Empty),
        Instruction::LocalGet(child_ptr),
        Instruction::LocalGet(1),
        Instruction::I32Const(8),
        Instruction::I32Add,
        Instruction::I32GeU,
        Instruction::LocalGet(child_ptr),
        Instruction::LocalGet(2),
        Instruction::I32LtU,
        Instruction::I32And,
        Instruction::If(BlockType::Empty),
        Instruction::LocalGet(3),
        Instruction::I32Eqz,
        Instruction::If(BlockType::Empty),
        Instruction::LocalGet(child_ptr),
        Instruction::F64ConvertI32U,
        Instruction::I32Const(1),
        Instruction::LocalGet(1),
        Instruction::LocalGet(2),
        Instruction::I32Const(1),
        Instruction::Call(fns.promote_value),
        Instruction::Drop,
        Instruction::Else,
        Instruction::LocalGet(child_ptr),
        Instruction::Call(fns.fwd_get),
        Instruction::LocalTee(reloc),
        Instruction::If(BlockType::Empty),
        Instruction::LocalGet(addr),
        Instruction::LocalGet(reloc),
        Instruction::I32Store(mem_arg_i32(0)),
        Instruction::End,
        Instruction::End,
        Instruction::Else,
        Instruction::LocalGet(child_ptr),
        Instruction::GlobalGet(globals.arena_base),
        Instruction::I32Const(8),
        Instruction::I32Add,
        Instruction::I32GeU,
        Instruction::LocalGet(child_ptr),
        Instruction::GlobalGet(pers.scratch_end),
        Instruction::I32LtU,
        Instruction::I32And,
        Instruction::If(BlockType::Empty),
        Instruction::LocalGet(child_ptr),
        Instruction::LocalGet(bit),
        Instruction::Call(fns.fwd_mark_flag),
        Instruction::I32Eqz,
        Instruction::If(BlockType::Empty),
        Instruction::LocalGet(child_ptr),
        Instruction::I32Const(2), // known_heap = 1 for tagged value
        Instruction::Call(fns.work_push),
        Instruction::End,
        Instruction::End,
        Instruction::End,
        Instruction::End,
        Instruction::End,
        Instruction::End,
        Instruction::LocalGet(3),
        Instruction::I32Eqz,
        Instruction::GlobalGet(pers.stage_overflow),
        Instruction::I32And,
        Instruction::If(BlockType::Empty),
        Instruction::I32Const(0),
        Instruction::GlobalSet(pers.work_len),
        Instruction::Return,
        Instruction::End,
        Instruction::LocalGet(i),
        Instruction::I32Const(1),
        Instruction::I32Add,
        Instruction::LocalSet(i),
        Instruction::Br(0),
        Instruction::End,
        Instruction::End,
      ]);
    };

  let mut instructions = vec![
    Instruction::LocalGet(0),
    Instruction::LocalGet(0),
    Instruction::F64Trunc,
    Instruction::F64Ne,
    Instruction::LocalGet(0),
    Instruction::GlobalGet(globals.arena_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::F64ConvertI32U,
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
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Result(ValType::I32)),
    Instruction::I32Const(1),
    Instruction::Else,
    Instruction::I32Const(2),
    Instruction::End,
    Instruction::LocalSet(bit),
    Instruction::LocalGet(ptr),
    Instruction::LocalGet(bit),
    Instruction::Call(fns.fwd_mark_flag),
    Instruction::If(BlockType::Empty),
    Instruction::Return,
    Instruction::End,
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.work_len),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(0),
    Instruction::Call(fns.work_push),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::GlobalGet(pers.stage_overflow),
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.work_len),
    Instruction::Return,
    Instruction::End,
    Instruction::GlobalGet(pers.work_len),
    Instruction::I32Eqz,
    Instruction::BrIf(1),
    Instruction::GlobalGet(pers.work_len),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::LocalTee(len),
    Instruction::GlobalSet(pers.work_len),
    Instruction::GlobalGet(pers.work_base),
    Instruction::LocalGet(len),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalTee(slot),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalSet(ptr),
    Instruction::LocalGet(slot),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalTee(kind),
    // (kind & 1) != 0: Scratch HAMT MapNode with kv_kind = kind >> 1
    Instruction::I32Const(1),
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(kind),
    Instruction::I32Const(1),
    Instruction::I32ShrU,
    Instruction::LocalSet(kv_kind),
    Instruction::LocalGet(ptr),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalTee(fval),
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
    Instruction::LocalGet(ptr),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalSet(addr),
  ];
  emit_map_node_child_slot(&mut instructions);
  instructions.extend([
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::GlobalGet(pers.stage_overflow),
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.work_len),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(i),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(i),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::Else,
    Instruction::LocalGet(fval),
    f64_const(1.0),
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(kv_kind),
    Instruction::I32Const(3),
    Instruction::I32And,
    Instruction::LocalSet(key_kind),
    Instruction::LocalGet(kv_kind),
    Instruction::I32Const(2),
    Instruction::I32ShrU,
    Instruction::I32Const(3),
    Instruction::I32And,
    Instruction::LocalSet(val_kind),
  ]);
  emit_non_scalar_container_kind(&mut instructions, key_kind);
  emit_non_scalar_container_kind(&mut instructions, val_kind);
  instructions.extend([
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
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
  ]);
  let bucket_slot_locals = RelocateSlotLocals {
    known_heap: Some(child_known_heap),
    ..slot_locals
  };
  emit_non_scalar_container_kind(&mut instructions, key_kind);
  instructions.extend([
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(key_kind),
    Instruction::I32Const(heap::ContainerSlotKind::ExactHeapPtr as i32),
    Instruction::I32Eq,
    Instruction::LocalSet(child_known_heap),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(16),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalSet(addr),
  ]);
  emit_relocate_f64_slot_worklist(&mut instructions, globals, pers, fns, &bucket_slot_locals);
  instructions.push(Instruction::End);
  emit_non_scalar_container_kind(&mut instructions, val_kind);
  instructions.extend([
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(val_kind),
    Instruction::I32Const(heap::ContainerSlotKind::ExactHeapPtr as i32),
    Instruction::I32Eq,
    Instruction::LocalSet(child_known_heap),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(24),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalSet(addr),
  ]);
  emit_relocate_f64_slot_worklist(&mut instructions, globals, pers, fns, &bucket_slot_locals);
  instructions.extend([
    Instruction::End,
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::GlobalGet(pers.stage_overflow),
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.work_len),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(i),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(i),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::Else,
    // (kind & 1) == 0: tagged Scratch object, known_heap = (kind >> 1) & 1
    Instruction::LocalGet(ptr),
    Instruction::I32Const(4),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalSet(tag),
    Instruction::Block(BlockType::Empty),
  ]);

  // List & Set: `[count:u32 @ 0][elem_kind:u32 @ 4]`
  for kind_tag in [tags.list, tags.set] {
    instructions.extend([
      Instruction::LocalGet(tag),
      Instruction::I32Const(kind_tag),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(ptr),
      Instruction::I32Load(mem_arg_i32(4)),
      Instruction::LocalSet(elem_kind),
    ]);
    emit_non_scalar_container_kind(&mut instructions, elem_kind);
    instructions.extend([
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(elem_kind),
      Instruction::I32Const(heap::ContainerSlotKind::ExactHeapPtr as i32),
      Instruction::I32Eq,
      Instruction::LocalSet(child_known_heap),
      Instruction::LocalGet(ptr),
      Instruction::I32Load(mem_arg_i32(0)),
      Instruction::LocalSet(count),
    ]);
    emit_relocate_elem_loop(&mut instructions, 8, Some(child_known_heap));
    instructions.extend([Instruction::End, Instruction::Br(1), Instruction::End]);
  }

  // BufList: `[cap:u32 @ 0][elem_kind:u32 @ 4][count:u32 @ 8][reserved:u32 @ 12]`
  instructions.extend([
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.buf_list),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(elem_kind),
  ]);
  emit_non_scalar_container_kind(&mut instructions, elem_kind);
  instructions.extend([
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(elem_kind),
    Instruction::I32Const(heap::ContainerSlotKind::ExactHeapPtr as i32),
    Instruction::I32Eq,
    Instruction::LocalSet(child_known_heap),
    Instruction::LocalGet(ptr),
    Instruction::I32Load(mem_arg_i32(8)),
    Instruction::LocalSet(count),
  ]);
  emit_relocate_elem_loop(&mut instructions, 16, Some(child_known_heap));
  instructions.extend([Instruction::End, Instruction::Br(1), Instruction::End]);

  // Enum: `[count:u16 @ 0][layout_id:u16 @ 2][tag_id:u32 @ 4]`
  instructions.extend([
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.enum_tag),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
  ]);
  if enum_layouts.max_layout_id > 0 {
    instructions.extend([
      Instruction::LocalGet(ptr),
      Instruction::I32Load16U(mem_arg_i16(2)),
      Instruction::I32Eqz,
      Instruction::If(BlockType::Empty),
    ]);
  }
  instructions.extend([
    Instruction::LocalGet(ptr),
    Instruction::I32Load16U(mem_arg_i16(0)),
    Instruction::LocalSet(count),
  ]);
  emit_relocate_elem_loop(&mut instructions, 8, None);
  if enum_layouts.max_layout_id > 0 {
    instructions.push(Instruction::Else);
    emit_relocate_descriptor_fields(
      &mut instructions,
      enum_layouts.desc_table_base,
      enum_layouts.max_layout_id,
      Instruction::I32Load16U(mem_arg_i16(2)),
    );
    instructions.push(Instruction::End);
  }
  instructions.extend([Instruction::Br(1), Instruction::End]);

  // DynTrait
  instructions.extend([
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.dyn_trait),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Const(super::dyn_trait::DYN_TRAIT_HEAP_FLAG as i32),
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::LocalSet(count),
  ]);
  emit_relocate_elem_loop(&mut instructions, 8, None);
  instructions.extend([Instruction::End, Instruction::Br(1), Instruction::End]);

  // Closure
  instructions.extend([
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.closure),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(count),
    Instruction::I32Const(0),
    Instruction::LocalSet(i),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(i),
    Instruction::LocalGet(count),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(ptr),
    Instruction::I64Load(mem_arg_i64(8)),
    Instruction::LocalGet(i),
    Instruction::I64ExtendI32U,
    Instruction::I64ShrU,
    Instruction::I64Const(1),
    Instruction::I64And,
    Instruction::I64Const(0),
    Instruction::I64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(16),
    Instruction::I32Add,
    Instruction::LocalGet(i),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::I32Add,
    Instruction::LocalSet(addr),
  ]);
  emit_relocate_f64_slot_worklist(&mut instructions, globals, pers, fns, &slot_locals);
  instructions.extend([
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::GlobalGet(pers.stage_overflow),
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.work_len),
    Instruction::Return,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(i),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(i),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::Br(1),
    Instruction::End,
  ]);

  if struct_layouts.max_layout_id > 0 {
    instructions.extend([
      Instruction::LocalGet(tag),
      Instruction::I32Const(tags.struct_tag),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
    ]);
    emit_relocate_descriptor_fields(
      &mut instructions,
      struct_layouts.desc_table_base,
      struct_layouts.max_layout_id,
      Instruction::I32Load(mem_arg_i32(4)),
    );
    instructions.extend([Instruction::Br(1), Instruction::End]);
  }

  // Map: `[count:u32 @ 0][kv_kind:u32 @ 4][root_ptr:i32 @ 8][reserved:i32 @ 12]`
  instructions.extend([
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.map),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(kv_kind),
    Instruction::LocalGet(ptr),
    Instruction::I32Load(mem_arg_i32(8)),
    Instruction::LocalTee(child_ptr),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(child_ptr),
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32GeU,
    Instruction::LocalGet(child_ptr),
    Instruction::LocalGet(2),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(child_ptr),
    Instruction::I32Const(1),
    Instruction::LocalGet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(kv_kind),
    Instruction::Call(fns.promote_map_node),
    Instruction::Drop,
    Instruction::Else,
    Instruction::LocalGet(child_ptr),
    Instruction::Call(fns.fwd_get),
    Instruction::LocalTee(reloc),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::LocalGet(reloc),
    Instruction::I32Store(mem_arg_i32(8)),
    Instruction::End,
    Instruction::End,
    Instruction::Else,
    Instruction::LocalGet(child_ptr),
    Instruction::GlobalGet(globals.arena_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32GeU,
    Instruction::LocalGet(child_ptr),
    Instruction::GlobalGet(pers.scratch_end),
    Instruction::I32LtU,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(child_ptr),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(child_ptr),
    Instruction::LocalGet(bit),
    Instruction::Call(fns.fwd_mark_flag),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(child_ptr),
    Instruction::LocalGet(kv_kind),
    Instruction::I32Const(1),
    Instruction::I32Shl,
    Instruction::I32Const(1),
    Instruction::I32Or,
    Instruction::Call(fns.work_push),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::Br(1),
    Instruction::End,
  ]);

  // AnyBox
  instructions.extend([
    Instruction::LocalGet(tag),
    Instruction::I32Const(tags.any_box),
    Instruction::I32Eq,
    Instruction::LocalGet(ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Const(1),
    Instruction::I32And,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(ptr),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(addr),
  ]);
  emit_relocate_f64_slot_worklist(&mut instructions, globals, pers, fns, &slot_locals);
  instructions.extend([
    Instruction::Br(1),
    Instruction::End,
    Instruction::End, // end tagged dispatch block
    Instruction::End, // end kind if/else
    Instruction::Br(0),
    Instruction::End, // end loop
    Instruction::End, // end outer block
  ]);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::I32, ValType::I32, ValType::I32],
    results: vec![],
    locals: vec![
      ValType::I32, // ptr
      ValType::I32, // bit
      ValType::I32, // kind
      ValType::I32, // tag
      ValType::I32, // count
      ValType::I32, // i
      ValType::F64, // fval
      ValType::I32, // child_ptr
      ValType::I32, // reloc
      ValType::I32, // addr
      ValType::I32, // desc_ptr
      ValType::I32, // meta_ptr
      ValType::I32, // field_kind
      ValType::I32, // packed
      ValType::I32, // len
      ValType::I32, // slot
      ValType::I32, // elem_kind
      ValType::I32, // child_known_heap
      ValType::I32, // kv_kind
      ValType::I32, // key_kind
      ValType::I32, // val_kind
    ],
    instructions,
  }
}

/// Ensures linear memory covers `mem_needed_i64` bytes (via `memory.grow`), trapping on OOM.
fn emit_ensure_memory_covers_i64(
  instructions: &mut Vec<Instruction<'static>>,
  mem_needed_i64: u32,
  desired_pages: u32,
  current_pages: u32,
) {
  instructions.extend([
    Instruction::LocalGet(mem_needed_i64),
    Instruction::I64Const(0xFFFF_FFFFi64),
    Instruction::I64GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(mem_needed_i64),
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
  ]);
}

/// Evacuates all live non-scalar `defref` graphs (and optional return value / Scratch roots) into the
/// alternate semi-space, growing `semi_space_bytes` & `arena_base` via non-overlapping staging when
/// live bytes exceed half a semi-space and `active_turns == 0` with no outstanding Scratch/Component roots.
pub(super) fn build_rt_compact_persistent_fn(
  memory: &WasmMemoryLayout,
  globals: &WasmGlobalLayout,
  pers: &PersistentHeapGlobals,
  promote_value_idx: u32,
  relocate_scratch_refs_idx: u32,
  arena_alloc_idx: u32,
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
  let semi = 13u32;
  let old_arena_base = 14u32;
  let can_grow_arena = 15u32;
  let live_bytes = 16u32;
  let high_base = 17u32;
  let upper_bound = 18u32;
  let new_s = 19u32;
  let new_arena_base = 20u32;
  let zero_end = 21u32;
  let desired_pages = 22u32;
  let current_pages = 23u32;
  let new_s_i64 = 24u32;
  let new_arena_i64 = 25u32;
  let mem_needed_i64 = 26u32;
  let semi_i64 = 27u32;
  let aligned_i64 = 28u32;
  let upper_i64 = 29u32;

  let mut locals = vec![
    ValType::I32, // 2: old_base
    ValType::I32, // 3: old_end
    ValType::I32, // 4: new_space
    ValType::I32, // 5: new_base
    ValType::I32, // 6: new_end
    ValType::I32, // 7: old_used
    ValType::F64, // 8: res
    ValType::I32, // 9: saved_heap_ptr
    ValType::I32, // 10: ret_is_scratch
    ValType::I32, // 11: new_ptr
    ValType::I32, // 12: root_node
    ValType::I32, // 13: semi
    ValType::I32, // 14: old_arena_base
    ValType::I32, // 15: can_grow_arena
    ValType::I32, // 16: live_bytes
    ValType::I32, // 17: high_base
    ValType::I32, // 18: upper_bound
    ValType::I32, // 19: new_s
    ValType::I32, // 20: new_arena_base
    ValType::I32, // 21: zero_end
    ValType::I32, // 22: desired_pages
    ValType::I32, // 23: current_pages
    ValType::I64, // 24: new_s_i64
    ValType::I64, // 25: new_arena_i64
    ValType::I64, // 26: mem_needed_i64
    ValType::I64, // 27: semi_i64
    ValType::I64, // 28: aligned_i64
    ValType::I64, // 29: upper_i64
  ];
  let mut staged_atoms: Vec<(u32, u32)> = Vec::new();
  for (i, &is_scalar) in atom_is_scalar.iter().enumerate().take(globals.atom_count as usize) {
    if !is_scalar {
      let local_idx = 2 + locals.len() as u32;
      locals.push(ValType::F64);
      staged_atoms.push((1 + i as u32, local_idx));
    }
  }

  let emit_compute_new_space_size_and_grow_mem = |insns: &mut Vec<Instruction<'static>>| {
    insns.extend([
      Instruction::LocalGet(semi),
      Instruction::I64ExtendI32U,
      Instruction::I64Const(1),
      Instruction::I64Shl,
      Instruction::LocalSet(semi_i64),
      Instruction::LocalGet(live_bytes),
      Instruction::I64ExtendI32U,
      Instruction::I64Const(7),
      Instruction::I64Add,
      Instruction::I64Const(-8),
      Instruction::I64And,
      Instruction::I64Const(1),
      Instruction::I64Shl,
      Instruction::LocalTee(aligned_i64),
      Instruction::LocalGet(semi_i64),
      Instruction::I64GtU,
      Instruction::If(BlockType::Result(ValType::I64)),
      Instruction::LocalGet(aligned_i64),
      Instruction::Else,
      Instruction::LocalGet(semi_i64),
      Instruction::End,
      Instruction::LocalTee(new_s_i64),
      Instruction::I64Const(0x4000_0000i64),
      Instruction::I64GtU,
      Instruction::If(BlockType::Empty),
      Instruction::Unreachable,
      Instruction::End,
      Instruction::LocalGet(new_s_i64),
      Instruction::I32WrapI64,
      Instruction::LocalSet(new_s),
      Instruction::I64Const(i64::from(memory.static_end)),
      Instruction::LocalGet(new_s_i64),
      Instruction::I64Const(1),
      Instruction::I64Shl,
      Instruction::I64Add,
      Instruction::LocalTee(new_arena_i64),
      Instruction::I64Const(65_536),
      Instruction::I64Add,
      Instruction::LocalSet(mem_needed_i64),
      Instruction::GlobalGet(HEAP_PTR_GLOBAL),
      Instruction::I64ExtendI32U,
      Instruction::LocalGet(mem_needed_i64),
      Instruction::I64GtU,
      Instruction::If(BlockType::Empty),
      Instruction::GlobalGet(HEAP_PTR_GLOBAL),
      Instruction::I64ExtendI32U,
      Instruction::LocalSet(mem_needed_i64),
      Instruction::End,
    ]);
    emit_ensure_memory_covers_i64(insns, mem_needed_i64, desired_pages, current_pages);
    insns.extend([
      Instruction::LocalGet(new_arena_i64),
      Instruction::I32WrapI64,
      Instruction::LocalSet(new_arena_base),
    ]);
  };

  let emit_commit_grown_space_a = |insns: &mut Vec<Instruction<'static>>| {
    for &(atom_global, stage_local) in &staged_atoms {
      insns.extend([Instruction::LocalGet(stage_local), Instruction::GlobalSet(atom_global)]);
    }
    insns.extend([
      Instruction::LocalGet(new_s),
      Instruction::GlobalSet(pers.semi_space_bytes),
      Instruction::LocalGet(new_arena_base),
      Instruction::GlobalSet(globals.arena_base),
      Instruction::I32Const(0),
      Instruction::GlobalSet(pers.active_space),
      Instruction::GlobalGet(pers.stage_ptr),
      Instruction::LocalTee(new_ptr),
      Instruction::GlobalSet(pers.ptr),
      Instruction::I32Const(memory.static_end),
      Instruction::LocalGet(new_s),
      Instruction::I32Add,
      Instruction::LocalTee(new_end),
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
      Instruction::GlobalGet(HEAP_PTR_GLOBAL),
      Instruction::LocalGet(new_arena_base),
      Instruction::I32GtU,
      Instruction::If(BlockType::Result(ValType::I32)),
      Instruction::GlobalGet(HEAP_PTR_GLOBAL),
      Instruction::Else,
      Instruction::LocalGet(new_arena_base),
      Instruction::End,
      Instruction::LocalSet(zero_end),
      Instruction::LocalGet(new_arena_base),
      Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    ]);
    if let Some(native) = &globals.native {
      insns.extend([
        Instruction::LocalGet(new_arena_base),
        Instruction::GlobalSet(native.outer_turn_mark),
      ]);
    }
    if let Some(comp) = &globals.component {
      insns.extend([
        Instruction::I32Const(0),
        Instruction::GlobalSet(comp.free_head),
        Instruction::I32Const(0),
        Instruction::GlobalSet(comp.turn_mark),
      ]);
    }
    emit_clear_fwd_state(insns, pers);
    insns.extend([
      Instruction::LocalGet(zero_end),
      Instruction::LocalGet(new_ptr),
      Instruction::I32GtU,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(new_ptr),
      Instruction::I32Const(0),
      Instruction::LocalGet(zero_end),
      Instruction::LocalGet(new_ptr),
      Instruction::I32Sub,
      Instruction::MemoryFill(0),
      Instruction::End,
      Instruction::LocalGet(res),
      Instruction::Return,
    ]);
  };

  let mut instructions = Vec::new();
  emit_reclaim_abandoned_fwd_workspace(&mut instructions, globals, pers);
  instructions.extend([
    Instruction::LocalGet(0),
    Instruction::LocalSet(res),
    Instruction::I32Const(0),
    Instruction::LocalSet(ret_is_scratch),
    Instruction::GlobalGet(pers.semi_space_bytes),
    Instruction::LocalSet(semi),
    Instruction::GlobalGet(globals.arena_base),
    Instruction::LocalSet(old_arena_base),
    Instruction::GlobalGet(pers.active_space),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(memory.static_end),
    Instruction::LocalTee(old_base),
    Instruction::LocalGet(semi),
    Instruction::I32Add,
    Instruction::LocalTee(old_end),
    Instruction::LocalSet(new_base),
    Instruction::I32Const(1),
    Instruction::LocalSet(new_space),
    Instruction::LocalGet(old_arena_base),
    Instruction::LocalSet(new_end),
    Instruction::Else,
    Instruction::I32Const(memory.static_end),
    Instruction::LocalTee(new_base),
    Instruction::LocalGet(semi),
    Instruction::I32Add,
    Instruction::LocalTee(new_end),
    Instruction::LocalSet(old_base),
    Instruction::I32Const(0),
    Instruction::LocalSet(new_space),
    Instruction::LocalGet(old_arena_base),
    Instruction::LocalSet(old_end),
    Instruction::End,
    Instruction::GlobalGet(pers.ptr),
    Instruction::LocalGet(old_base),
    Instruction::I32Sub,
    Instruction::LocalSet(old_used),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::LocalTee(saved_heap_ptr),
    Instruction::GlobalSet(pers.scratch_end),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_base),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_cap),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_len),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.work_base),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.work_cap),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.work_len),
    Instruction::LocalGet(saved_heap_ptr),
    Instruction::GlobalSet(pers.workspace_end),
    // Determine whether ret_val is a Scratch return value
    Instruction::LocalGet(1),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::LocalGet(0),
    Instruction::F64Trunc,
    Instruction::F64Eq,
    Instruction::LocalGet(0),
    Instruction::LocalGet(old_arena_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::F64ConvertI32U,
    Instruction::F64Ge,
    Instruction::I32And,
    Instruction::LocalGet(0),
    Instruction::LocalGet(saved_heap_ptr),
    Instruction::F64ConvertI32U,
    Instruction::F64Lt,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::LocalSet(ret_is_scratch),
    Instruction::End,
    Instruction::End,
    // Compute can_grow_arena
    Instruction::GlobalGet(globals.active_turns),
    Instruction::I32Eqz,
    Instruction::LocalGet(ret_is_scratch),
    Instruction::I32Eqz,
    Instruction::I32And,
  ]);
  if let Some(native) = &globals.native {
    instructions.extend([
      Instruction::GlobalGet(native.scratch_roots_head),
      Instruction::I32Eqz,
      Instruction::I32And,
    ]);
  }
  if let Some(comp) = &globals.component {
    instructions.extend([Instruction::GlobalGet(comp.total_live), Instruction::I32Eqz, Instruction::I32And]);
  }
  instructions.extend([
    Instruction::LocalSet(can_grow_arena),
    // Pass 0 Attempt 1: stage into inactive semi-space [new_base, new_end)
    Instruction::LocalGet(new_base),
    Instruction::GlobalSet(pers.stage_base),
    Instruction::LocalGet(new_base),
    Instruction::GlobalSet(pers.stage_ptr),
    Instruction::LocalGet(new_end),
    Instruction::GlobalSet(pers.stage_limit),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_overflow),
  ]);

  for &(atom_global, stage_local) in &staged_atoms {
    instructions.extend([
      Instruction::GlobalGet(pers.stage_overflow),
      Instruction::I32Eqz,
      Instruction::If(BlockType::Empty),
      Instruction::GlobalGet(atom_global),
      Instruction::I32Const(1),
      Instruction::LocalGet(old_base),
      Instruction::LocalGet(old_end),
      Instruction::I32Const(0),
      Instruction::Call(promote_value_idx),
      Instruction::LocalSet(stage_local),
      Instruction::End,
    ]);
  }

  instructions.extend([
    Instruction::GlobalGet(pers.stage_overflow),
    Instruction::I32Eqz,
    Instruction::LocalGet(1),
    Instruction::I32And,
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
    Instruction::LocalGet(ret_is_scratch),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::LocalGet(old_base),
    Instruction::LocalGet(old_end),
    Instruction::I32Const(0),
    Instruction::Call(relocate_scratch_refs_idx),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
  ]);
  if let Some(native) = &globals.native {
    instructions.extend([
      Instruction::GlobalGet(pers.stage_overflow),
      Instruction::I32Eqz,
      Instruction::If(BlockType::Empty),
    ]);
    emit_relocate_scratch_roots_pass(
      &mut instructions,
      &ScratchRootsPass {
        globals,
        native,
        pers,
        root_node,
        saved_heap_ptr,
        old_base,
        old_end,
        relocate_scratch_refs_idx,
        do_patch: 0,
      },
    );
    instructions.push(Instruction::End);
  }

  // Outcome A: Attempt 1 overflowed inactive semi-space
  instructions.extend([
    Instruction::GlobalGet(pers.stage_overflow),
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(pers.stage_ptr),
    Instruction::LocalGet(new_base),
    Instruction::I32Sub,
    Instruction::LocalTee(live_bytes),
    Instruction::I32Const(0),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(new_base),
    Instruction::I32Const(0),
    Instruction::LocalGet(live_bytes),
    Instruction::MemoryFill(0),
    Instruction::End,
    Instruction::LocalGet(saved_heap_ptr),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
  ]);
  emit_clear_fwd_state(&mut instructions, pers);
  instructions.extend([
    Instruction::LocalGet(can_grow_arena),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::Return,
    Instruction::End,
    // High-staging Attempt 2 above saved_heap_ptr
    Instruction::LocalGet(old_used),
    Instruction::I64ExtendI32U,
    Instruction::LocalGet(saved_heap_ptr),
    Instruction::LocalGet(old_arena_base),
    Instruction::I32Sub,
    Instruction::I64ExtendI32U,
    Instruction::I64Add,
    Instruction::I64Const(7),
    Instruction::I64Add,
    Instruction::I64Const(-8),
    Instruction::I64And,
    Instruction::LocalTee(upper_i64),
    Instruction::I64Const(0x7FFF_FFF0i64),
    Instruction::I64GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(upper_i64),
    Instruction::I32WrapI64,
    Instruction::LocalTee(upper_bound),
    Instruction::Call(arena_alloc_idx),
    Instruction::LocalSet(high_base),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::GlobalSet(pers.workspace_end),
    Instruction::LocalGet(saved_heap_ptr),
    Instruction::GlobalSet(pers.scratch_end),
    Instruction::LocalGet(high_base),
    Instruction::GlobalSet(pers.stage_base),
    Instruction::LocalGet(high_base),
    Instruction::GlobalSet(pers.stage_ptr),
    Instruction::LocalGet(high_base),
    Instruction::LocalGet(upper_bound),
    Instruction::I32Add,
    Instruction::GlobalSet(pers.stage_limit),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_overflow),
  ]);
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
  instructions.extend([
    Instruction::LocalGet(1),
    Instruction::If(BlockType::Empty),
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
    Instruction::End,
    Instruction::GlobalGet(pers.stage_overflow),
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::GlobalGet(pers.stage_ptr),
    Instruction::LocalGet(high_base),
    Instruction::I32Sub,
    Instruction::LocalSet(live_bytes),
  ]);
  emit_compute_new_space_size_and_grow_mem(&mut instructions);
  // Reset forwarding table and relocate [high_base, high_base + live_bytes) -> [static_end, static_end + live_bytes)
  instructions.extend([
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_len),
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::I32Const(0),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(pers.fwd_base),
    Instruction::I32Const(0),
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::MemoryFill(0),
    Instruction::End,
    Instruction::I32Const(memory.static_end),
    Instruction::GlobalSet(pers.stage_base),
    Instruction::I32Const(memory.static_end),
    Instruction::GlobalSet(pers.stage_ptr),
    Instruction::I32Const(memory.static_end),
    Instruction::LocalGet(new_s),
    Instruction::I32Add,
    Instruction::GlobalSet(pers.stage_limit),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_overflow),
  ]);
  for &(_, stage_local) in &staged_atoms {
    instructions.extend([
      Instruction::LocalGet(stage_local),
      Instruction::I32Const(1),
      Instruction::LocalGet(high_base),
      Instruction::LocalGet(high_base),
      Instruction::LocalGet(live_bytes),
      Instruction::I32Add,
      Instruction::I32Const(0),
      Instruction::Call(promote_value_idx),
      Instruction::LocalSet(stage_local),
    ]);
  }
  instructions.extend([
    Instruction::LocalGet(1),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(res),
    Instruction::I32Const(1),
    Instruction::LocalGet(high_base),
    Instruction::LocalGet(high_base),
    Instruction::LocalGet(live_bytes),
    Instruction::I32Add,
    Instruction::I32Const(0),
    Instruction::Call(promote_value_idx),
    Instruction::LocalSet(res),
    Instruction::End,
  ]);
  emit_commit_grown_space_a(&mut instructions);
  instructions.push(Instruction::End); // end Outcome A

  // Outcome B: Attempt 1 succeeded in inactive semi-space [new_base, new_end)
  instructions.extend([
    Instruction::GlobalGet(pers.stage_ptr),
    Instruction::LocalGet(new_base),
    Instruction::I32Sub,
    Instruction::LocalTee(live_bytes),
    Instruction::LocalGet(semi),
    Instruction::I32Const(1),
    Instruction::I32ShrU,
    Instruction::I32GtU,
    Instruction::LocalGet(can_grow_arena),
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
  ]);
  emit_compute_new_space_size_and_grow_mem(&mut instructions);
  // Second pass only runs when Space A was initially active (Attempt 1 wrote to Space B, new_space == 1);
  // when Space B was initially active (new_space == 0), Attempt 1 already evacuated directly into Space A.
  instructions.extend([
    Instruction::LocalGet(new_space),
    Instruction::I32Const(0),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.fwd_len),
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::I32Const(0),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(pers.fwd_base),
    Instruction::I32Const(0),
    Instruction::GlobalGet(pers.fwd_cap),
    Instruction::I32Const(3),
    Instruction::I32Shl,
    Instruction::MemoryFill(0),
    Instruction::End,
    Instruction::I32Const(memory.static_end),
    Instruction::GlobalSet(pers.stage_base),
    Instruction::I32Const(memory.static_end),
    Instruction::GlobalSet(pers.stage_ptr),
    Instruction::I32Const(memory.static_end),
    Instruction::LocalGet(new_s),
    Instruction::I32Add,
    Instruction::GlobalSet(pers.stage_limit),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_overflow),
  ]);
  for &(_, stage_local) in &staged_atoms {
    instructions.extend([
      Instruction::LocalGet(stage_local),
      Instruction::I32Const(1),
      Instruction::LocalGet(new_base),
      Instruction::LocalGet(new_base),
      Instruction::LocalGet(live_bytes),
      Instruction::I32Add,
      Instruction::I32Const(0),
      Instruction::Call(promote_value_idx),
      Instruction::LocalSet(stage_local),
    ]);
  }
  instructions.extend([
    Instruction::LocalGet(1),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(res),
    Instruction::I32Const(1),
    Instruction::LocalGet(new_base),
    Instruction::LocalGet(new_base),
    Instruction::LocalGet(live_bytes),
    Instruction::I32Add,
    Instruction::I32Const(0),
    Instruction::Call(promote_value_idx),
    Instruction::LocalSet(res),
    Instruction::End,
    Instruction::End,
  ]);
  emit_commit_grown_space_a(&mut instructions);
  instructions.push(Instruction::End);

  // Normal atomic commit into new_space (either <= semi/2, or > semi/2 deferred because !can_grow_arena)
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
    Instruction::LocalGet(live_bytes),
    Instruction::LocalGet(semi),
    Instruction::I32Const(1),
    Instruction::I32ShrU,
    Instruction::I32GtU,
    Instruction::If(BlockType::Result(ValType::I32)),
    Instruction::I32Const(3),
    Instruction::Else,
    Instruction::I32Const(0),
    Instruction::End,
    Instruction::GlobalSet(pers.needs_compact),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_base),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_ptr),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_limit),
    Instruction::I32Const(0),
    Instruction::GlobalSet(pers.stage_overflow),
    // Pass 1: patch Scratch return children in place without growing or allocating.
    Instruction::LocalGet(ret_is_scratch),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::LocalGet(old_base),
    Instruction::LocalGet(old_end),
    Instruction::I32Const(1),
    Instruction::Call(relocate_scratch_refs_idx),
    Instruction::End,
  ]);
  if let Some(native) = &globals.native {
    emit_relocate_scratch_roots_pass(
      &mut instructions,
      &ScratchRootsPass {
        globals,
        native,
        pers,
        root_node,
        saved_heap_ptr,
        old_base,
        old_end,
        relocate_scratch_refs_idx,
        do_patch: 1,
      },
    );
  }
  instructions.extend([Instruction::LocalGet(saved_heap_ptr), Instruction::GlobalSet(HEAP_PTR_GLOBAL)]);
  emit_clear_fwd_state(&mut instructions, pers);
  instructions.extend([
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
  globals: &WasmGlobalLayout,
  native: &NativeTurnGlobals,
  compact_persistent_idx: Option<u32>,
) -> CompiledFn {
  // param: 0 = can_rewind_stale (i32)
  let mark = 1u32;
  let mut instructions = vec![
    Instruction::GlobalGet(native.host_call_depth),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(globals.active_turns),
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::GlobalSet(globals.active_turns),
    Instruction::GlobalGet(native.outer_turn_mark),
    Instruction::LocalTee(mark),
    Instruction::GlobalGet(globals.arena_base),
    Instruction::I32LtU,
    Instruction::LocalGet(mark),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32GtU,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::GlobalGet(globals.arena_base),
    Instruction::LocalSet(mark),
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::If(BlockType::Empty),
  ];
  if let Some(pers) = &globals.persistent {
    emit_reclaim_abandoned_fwd_workspace(&mut instructions, globals, pers);
    emit_pop_scratch_roots_above(&mut instructions, globals, native, mark);
  }
  if let (Some(pers), Some(compact_idx)) = (&globals.persistent, compact_persistent_idx) {
    instructions.extend([
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::If(BlockType::Empty),
      f64_const(0.0),
      Instruction::I32Const(0),
      Instruction::Call(compact_idx),
      Instruction::Drop,
      Instruction::LocalGet(mark),
      Instruction::GlobalGet(globals.arena_base),
      Instruction::I32LtU,
      Instruction::If(BlockType::Empty),
      Instruction::GlobalGet(globals.arena_base),
      Instruction::LocalSet(mark),
      Instruction::End,
      Instruction::End,
    ]);
  }
  instructions.extend([Instruction::LocalGet(mark), Instruction::GlobalSet(HEAP_PTR_GLOBAL)]);
  if let Some(pers) = &globals.persistent {
    instructions.push(Instruction::Else);
    emit_clear_fwd_state(&mut instructions, pers);
  }
  instructions.extend([
    Instruction::End,
    Instruction::LocalGet(mark),
    Instruction::GlobalSet(native.outer_turn_mark),
    Instruction::I32Const(1),
    Instruction::GlobalSet(globals.active_turns),
    Instruction::LocalGet(mark),
    Instruction::Return,
    Instruction::End,
  ]);
  if let Some(pers) = &globals.persistent {
    instructions.extend([Instruction::LocalGet(0), Instruction::If(BlockType::Empty)]);
    emit_reclaim_abandoned_fwd_workspace(&mut instructions, globals, pers);
    instructions.push(Instruction::Else);
    emit_clear_fwd_state(&mut instructions, pers);
    instructions.push(Instruction::End);
  }
  instructions.extend([
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::LocalTee(mark),
    Instruction::GlobalSet(native.outer_turn_mark),
    Instruction::I32Const(1),
    Instruction::GlobalSet(globals.active_turns),
    Instruction::LocalGet(mark),
    Instruction::Return,
    Instruction::End,
  ]);
  if let Some(pers) = &globals.persistent {
    emit_clear_fwd_state(&mut instructions, pers);
  }
  instructions.extend([
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
      Instruction::I32Const(1),
      Instruction::I32Eq,
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::I32Const(2),
      Instruction::I32Eq,
      Instruction::I32Or,
    ]);
    if let Some(native) = &globals.native {
      instructions.extend([
        Instruction::GlobalGet(pers.needs_compact),
        Instruction::I32Const(3),
        Instruction::I32Eq,
        Instruction::GlobalGet(native.scratch_roots_head),
        Instruction::I32Eqz,
        Instruction::I32And,
        Instruction::I32Or,
      ]);
    }
    instructions.extend([
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(3),
      Instruction::LocalGet(2),
      Instruction::Call(compact_idx),
      Instruction::LocalSet(3),
      Instruction::LocalGet(0),
      Instruction::GlobalGet(globals.arena_base),
      Instruction::I32LtU,
      Instruction::If(BlockType::Empty),
      Instruction::GlobalGet(globals.arena_base),
      Instruction::LocalSet(0),
      Instruction::End,
      Instruction::End,
    ]);
  }
  instructions.push(Instruction::End);

  // Always register verified new Scratch return roots so subsequent scalar turns keep deferring growth,
  // while guarding __heap_ptr rewind when needs_compact == 2 indicates an uncommitted overflow target.
  let emit_rewind = |insns: &mut Vec<Instruction<'static>>| {
    if let Some(pers) = &globals.persistent {
      insns.extend([
        Instruction::GlobalGet(pers.needs_compact),
        Instruction::I32Const(2),
        Instruction::I32Ne,
        Instruction::If(BlockType::Empty),
      ]);
    }
    insns.extend([Instruction::LocalGet(0), Instruction::GlobalSet(HEAP_PTR_GLOBAL)]);
    if globals.persistent.is_some()
      && let Some(native) = &globals.native
    {
      emit_pop_scratch_roots_above(insns, globals, native, 0);
    }
    if globals.persistent.is_some() {
      insns.push(Instruction::End);
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
    emit_reclaim_abandoned_fwd_workspace(&mut instructions, globals, pers);
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
    Instruction::GlobalGet(globals.arena_base),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::GlobalGet(globals.arena_base),
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

pub(super) fn build_rt_component_turn_enter_fn(globals: &WasmGlobalLayout, comp: &ComponentMemoryGlobals) -> CompiledFn {
  let mut instructions = Vec::new();
  if let Some(pers) = &globals.persistent {
    emit_reclaim_abandoned_fwd_workspace(&mut instructions, globals, pers);
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
      Instruction::I32Const(1),
      Instruction::I32Eq,
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::I32Const(2),
      Instruction::I32Eq,
      Instruction::I32Or,
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::I32Const(3),
      Instruction::I32Eq,
      Instruction::GlobalGet(comp.total_live),
      Instruction::I32Eqz,
      Instruction::I32And,
      Instruction::I32Or,
      Instruction::If(BlockType::Empty),
      f64_const(0.0),
      Instruction::I32Const(0),
      Instruction::Call(compact_idx),
      Instruction::Drop,
      Instruction::End,
    ]);
  }
  if let Some(pers) = &globals.persistent {
    instructions.extend([
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::I32Const(2),
      Instruction::I32Ne,
      Instruction::If(BlockType::Empty),
    ]);
  }
  instructions.extend([
    Instruction::GlobalGet(comp.total_live),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::GlobalSet(comp.free_head),
    Instruction::GlobalGet(globals.arena_base),
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
  ]);
  if globals.persistent.is_some() {
    instructions.push(Instruction::End);
  }
  instructions.extend([
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
