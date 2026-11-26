//! Persistent HAMT map and set WASM runtime helpers.

use super::*;

pub(in crate::wasm::emit_wasm) const RT_MAP_TABLE_KIND: f64 = 0.0;
pub(in crate::wasm::emit_wasm) const RT_MAP_BUCKET_KIND: f64 = 1.0;
pub(in crate::wasm::emit_wasm) const RT_MAP_TABLE_CHILDREN: i32 = 32;
pub(in crate::wasm::emit_wasm) const RT_MAP_TABLE_SLOTS: i32 = 33; // kind + 32 child pointers
pub(in crate::wasm::emit_wasm) const RT_MAP_TABLE_BYTES: i32 = RT_MAP_TABLE_SLOTS * 8;

pub(in crate::wasm::emit_wasm) fn rt_emit_table_child_addr(
  builder: &mut RuntimeFnBuilder,
  table_local: u32,
  child_idx_local: u32,
  dst_local: u32,
) {
  builder.emit(Instruction::LocalGet(table_local));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalGet(child_idx_local));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Mul);
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(dst_local));
}

pub(in crate::wasm::emit_wasm) fn rt_emit_load_table_child(
  builder: &mut RuntimeFnBuilder,
  table_local: u32,
  child_idx_local: u32,
  dst_local: u32,
) {
  builder.emit(Instruction::LocalGet(table_local));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalGet(child_idx_local));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Mul);
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::F64Load(mem_arg_f64(0)));
  builder.emit(Instruction::I32TruncF64U);
  builder.emit(Instruction::LocalSet(dst_local));
}

pub(in crate::wasm::emit_wasm) fn rt_emit_alloc_empty_table(builder: &mut RuntimeFnBuilder, dst_local: u32) {
  rt_emit_alloc_const(builder, RT_MAP_TABLE_BYTES, dst_local, 0);
  builder.emit(Instruction::LocalGet(dst_local));
  builder.emit(f64_const(RT_MAP_TABLE_KIND));
  builder.emit(Instruction::F64Store(mem_arg_f64(0)));

  let i = builder.alloc_i32();
  let addr = builder.alloc_i32();
  builder.emit(Instruction::I32Const(0));
  builder.emit(Instruction::LocalSet(i));
  builder.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  builder.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  builder.emit(Instruction::LocalGet(i));
  builder.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  builder.emit(Instruction::I32GeU);
  builder.emit(Instruction::BrIf(1));
  rt_emit_table_child_addr(builder, dst_local, i, addr);
  builder.emit(Instruction::LocalGet(addr));
  builder.emit(f64_const(0.0));
  builder.emit(Instruction::F64Store(mem_arg_f64(0)));
  builder.emit(Instruction::LocalGet(i));
  builder.emit(Instruction::I32Const(1));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(i));
  builder.emit(Instruction::Br(0));
  builder.emit(Instruction::End);
  builder.emit(Instruction::End);
}

pub(in crate::wasm::emit_wasm) fn rt_emit_alloc_bucket(builder: &mut RuntimeFnBuilder, count_local: u32, dst_local: u32) {
  let slots = builder.alloc_i32();
  let size = builder.alloc_i32();
  builder.emit(Instruction::LocalGet(count_local));
  builder.emit(Instruction::I32Const(2));
  builder.emit(Instruction::I32Mul);
  builder.emit(Instruction::I32Const(2));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(slots));
  builder.emit(Instruction::LocalGet(slots));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Mul);
  builder.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(builder, size, dst_local, 0);
  builder.emit(Instruction::LocalGet(dst_local));
  builder.emit(f64_const(RT_MAP_BUCKET_KIND));
  builder.emit(Instruction::F64Store(mem_arg_f64(0)));
  builder.emit(Instruction::LocalGet(dst_local));
  builder.emit(Instruction::LocalGet(count_local));
  builder.emit(Instruction::F64ConvertI32U);
  builder.emit(Instruction::F64Store(mem_arg_f64(8)));
}

pub(in crate::wasm::emit_wasm) fn rt_emit_copy_slots(
  builder: &mut RuntimeFnBuilder,
  copy_fn_idx: u32,
  dst_local: u32,
  src_local: u32,
  count_local: u32,
) {
  builder.emit(Instruction::LocalGet(dst_local));
  builder.emit(Instruction::LocalGet(src_local));
  builder.emit(Instruction::LocalGet(count_local));
  builder.emit(Instruction::Call(copy_fn_idx));
}

fn rt_emit_join_slot_kind(b: &mut RuntimeFnBuilder, a_kind: u32, b_kind: u32, out: u32) {
  b.emit(Instruction::LocalGet(a_kind));
  b.emit(Instruction::LocalGet(b_kind));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(a_kind));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(a_kind));
  b.emit(Instruction::I32Const(ContainerSlotKind::EmptyUntyped as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(b_kind));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(b_kind));
  b.emit(Instruction::I32Const(ContainerSlotKind::EmptyUntyped as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(a_kind));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(ContainerSlotKind::DynamicBoxed as i32));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalSet(out));
}

fn rt_emit_hash_with_kind(
  b: &mut RuntimeFnBuilder,
  val: u32,
  kind: u32,
  hash_out: u32,
  scalar_hash_idx: u32,
  value_hash_idx: u32,
  heap_value_hash_idx: u32,
) {
  b.emit(Instruction::LocalGet(kind));
  b.emit(Instruction::I32Const(ContainerSlotKind::PureScalar as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::Call(scalar_hash_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(kind));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::Call(heap_value_hash_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::Call(value_hash_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalSet(hash_out));
}

pub(in crate::wasm::emit_wasm) fn rt_emit_eq_with_single_kind(
  b: &mut RuntimeFnBuilder,
  a: u32,
  b_val: u32,
  kind: u32,
  (f64_value_eq_idx, value_equal_idx, heap_value_equal_idx, is_heap_ptr_idx): (u32, u32, u32, u32),
) {
  b.emit(Instruction::LocalGet(kind));
  b.emit(Instruction::I32Const(ContainerSlotKind::PureScalar as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(a));
  b.emit(Instruction::LocalGet(b_val));
  b.emit(Instruction::Call(f64_value_eq_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(kind));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(a));
  b.emit(Instruction::Call(is_heap_ptr_idx));
  b.emit(Instruction::LocalGet(b_val));
  b.emit(Instruction::Call(is_heap_ptr_idx));
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(a));
  b.emit(Instruction::LocalGet(b_val));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(a));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalGet(b_val));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::Call(heap_value_equal_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(a));
  b.emit(Instruction::LocalGet(b_val));
  b.emit(Instruction::Call(value_equal_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
}

pub(in crate::wasm::emit_wasm) fn rt_emit_eq_with_pair_kinds(
  b: &mut RuntimeFnBuilder,
  (a, b_val): (u32, u32),
  (kind_a, kind_b): (u32, u32),
  (f64_value_eq_idx, value_equal_idx, heap_value_equal_idx, is_heap_ptr_idx): (u32, u32, u32, u32),
) {
  b.emit(Instruction::LocalGet(kind_a));
  b.emit(Instruction::I32Const(ContainerSlotKind::PureScalar as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(kind_b));
  b.emit(Instruction::I32Const(ContainerSlotKind::PureScalar as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(kind_a));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(kind_b));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(a));
  b.emit(Instruction::LocalGet(b_val));
  b.emit(Instruction::Call(f64_value_eq_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(kind_a));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(kind_b));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(a));
  b.emit(Instruction::LocalGet(b_val));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(a));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalGet(b_val));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::Call(heap_value_equal_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(kind_a));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(kind_b));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(a));
  b.emit(Instruction::Call(is_heap_ptr_idx));
  b.emit(Instruction::LocalGet(b_val));
  b.emit(Instruction::Call(is_heap_ptr_idx));
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(a));
  b.emit(Instruction::LocalGet(b_val));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(a));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalGet(b_val));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::Call(heap_value_equal_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(a));
  b.emit(Instruction::LocalGet(b_val));
  b.emit(Instruction::Call(value_equal_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_key_equal(
  value_equal_idx: u32,
  heap_value_equal_idx: u32,
  f64_value_eq_idx: u32,
  is_heap_ptr_idx: u32,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3); // a: f64, b: f64, key_kind: i32
  rt_emit_eq_with_single_kind(
    &mut b,
    0,
    1,
    2,
    (f64_value_eq_idx, value_equal_idx, heap_value_equal_idx, is_heap_ptr_idx),
  );
  b.finish(vec![ValType::F64, ValType::F64, ValType::I32], vec![ValType::I32])
}

/// Scans `count` key-value pairs at `base + entry_offset + i*16` and sets `found_idx` to the
/// matching entry index or `-1`. For multi-entry `DynamicBoxed` buckets/maps, prefers an exact
/// `PureScalar` f64 match before falling back to `DynamicBoxed` equality so aliased numeric keys
/// widened into `DynamicBoxed` still resolve to their own entries.
#[allow(clippy::too_many_arguments)]
fn rt_emit_find_key_in_pairs(
  b: &mut RuntimeFnBuilder,
  base: u32,
  entry_offset: i32,
  count: u32,
  target_key: u32,
  key_kind: u32,
  key_equal_idx: u32,
  i: u32,
  found_idx: u32,
) {
  let emit_scan = |b: &mut RuntimeFnBuilder, exact_scalar: bool| {
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::LocalSet(i));
    b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::LocalGet(count));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::BrIf(1));
    b.emit(Instruction::LocalGet(base));
    b.emit(Instruction::I32Const(entry_offset));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::I32Const(16));
    b.emit(Instruction::I32Mul);
    b.emit(Instruction::I32Add);
    b.emit(Instruction::F64Load(mem_arg_f64(0)));
    b.emit(Instruction::LocalGet(target_key));
    if exact_scalar {
      b.emit(Instruction::I32Const(ContainerSlotKind::PureScalar as i32));
    } else {
      b.emit(Instruction::LocalGet(key_kind));
    }
    b.emit(Instruction::Call(key_equal_idx));
    b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::LocalSet(found_idx));
    b.emit(Instruction::Br(2));
    b.emit(Instruction::End);
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(i));
    b.emit(Instruction::Br(0));
    b.emit(Instruction::End);
    b.emit(Instruction::End);
  };

  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(found_idx));

  b.emit(Instruction::LocalGet(key_kind));
  b.emit(Instruction::I32Const(ContainerSlotKind::DynamicBoxed as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  emit_scan(b, true);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  emit_scan(b, false);
  b.emit(Instruction::End);
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_make(map_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3); // count, root_ptr, kv_kind
  let dst = b.alloc_i32();
  rt_emit_alloc_const(&mut b, 16, dst, map_tag);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Store(mem_arg_i32(8)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Store(mem_arg_i32(12)));
  b.emit(Instruction::LocalGet(dst));
  b.finish(vec![ValType::I32, ValType::I32, ValType::I32], vec![ValType::I32])
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_root_assoc(copy_fn_idx: u32, key_equal_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(5); // root, key, value, hash, key_kind
  let idx0 = b.alloc_i32();
  let idx1 = b.alloc_i32();
  let table1 = b.alloc_i32();
  let bucket = b.alloc_i32();
  let new_root = b.alloc_i32();
  let new_table1 = b.alloc_i32();
  let new_bucket = b.alloc_i32();
  let bucket_count = b.alloc_i32();
  let found_idx = b.alloc_i32();
  let i = b.alloc_i32();
  let slots = b.alloc_i32();
  let addr = b.alloc_i32();
  let added = b.alloc_i32();

  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(31));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(idx0));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(5));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(31));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(idx1));

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  rt_emit_alloc_empty_table(&mut b, new_root);
  rt_emit_alloc_empty_table(&mut b, new_table1);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(bucket_count));
  rt_emit_alloc_bucket(&mut b, bucket_count, new_bucket);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Store(mem_arg_f64(16)));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(24)));
  rt_emit_table_child_addr(&mut b, new_table1, idx1, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  rt_emit_table_child_addr(&mut b, new_root, idx0, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_table1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::Else);

  rt_emit_load_table_child(&mut b, 0, idx0, table1);
  b.emit(Instruction::LocalGet(table1));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_root, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_root, 0, slots);
  rt_emit_alloc_empty_table(&mut b, new_table1);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(bucket_count));
  rt_emit_alloc_bucket(&mut b, bucket_count, new_bucket);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Store(mem_arg_f64(16)));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(24)));
  rt_emit_table_child_addr(&mut b, new_table1, idx1, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  rt_emit_table_child_addr(&mut b, new_root, idx0, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_table1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::Else);

  rt_emit_load_table_child(&mut b, table1, idx1, bucket);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_root, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_root, 0, slots);
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_table1, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_table1, table1, slots);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(bucket_count));
  rt_emit_alloc_bucket(&mut b, bucket_count, new_bucket);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Store(mem_arg_f64(16)));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(24)));
  rt_emit_table_child_addr(&mut b, new_table1, idx1, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  rt_emit_table_child_addr(&mut b, new_root, idx0, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_table1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::Else);

  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(bucket_count));
  rt_emit_find_key_in_pairs(&mut b, bucket, 16, bucket_count, 1, 4, key_equal_idx, i, found_idx);

  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  rt_emit_alloc_bucket(&mut b, slots, new_bucket);
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_bucket, bucket, slots);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(8)));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(addr));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(8)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::Else);
  rt_emit_alloc_bucket(&mut b, bucket_count, new_bucket);
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_bucket, bucket, slots);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::End);

  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_root, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_root, 0, slots);
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_table1, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_table1, table1, slots);
  rt_emit_table_child_addr(&mut b, new_table1, idx1, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  rt_emit_table_child_addr(&mut b, new_root, idx0, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_table1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(new_root));
  b.emit(Instruction::LocalGet(added));

  b.finish(
    vec![ValType::I32, ValType::F64, ValType::F64, ValType::I32, ValType::I32],
    vec![ValType::I32, ValType::I32],
  )
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_root_lookup(key_equal_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(4); // root, key, hash, key_kind
  let idx0 = b.alloc_i32();
  let idx1 = b.alloc_i32();
  let table1 = b.alloc_i32();
  let bucket = b.alloc_i32();
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let found_idx = b.alloc_i32();
  let found = b.alloc_i32();
  let value = b.alloc_f64();

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(found));
  b.emit(f64_const(0.0));
  b.emit(Instruction::LocalSet(value));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(31));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(idx0));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(5));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(31));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(idx1));
  rt_emit_load_table_child(&mut b, 0, idx0, table1);
  b.emit(Instruction::LocalGet(table1));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  rt_emit_load_table_child(&mut b, table1, idx1, bucket);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  rt_emit_find_key_in_pairs(&mut b, bucket, 16, count, 1, 3, key_equal_idx, i, found_idx);
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(value));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(found));
  b.emit(Instruction::LocalGet(value));
  b.finish(
    vec![ValType::I32, ValType::F64, ValType::I32, ValType::I32],
    vec![ValType::I32, ValType::F64],
  )
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_root_contains_value() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2); // root, target
  let i0 = b.alloc_i32();
  let i1 = b.alloc_i32();
  let table1 = b.alloc_i32();
  let bucket = b.alloc_i32();
  let count = b.alloc_i32();
  let bi = b.alloc_i32();
  let found = b.alloc_i32();
  let entry_value = b.alloc_f64();

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i0));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i0));
  b.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  rt_emit_load_table_child(&mut b, 0, i0, table1);
  b.emit(Instruction::LocalGet(table1));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i1));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i1));
  b.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  rt_emit_load_table_child(&mut b, table1, i1, bucket);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(bi));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalTee(entry_value));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Eq);
  // NaN equals NaN as a value
  b.emit(Instruction::LocalGet(entry_value));
  b.emit(Instruction::LocalGet(entry_value));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::Br(6));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(bi));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i1));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i1));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i0));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i0));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(found));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_root_write_pairs() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3); // root, dst_base, write_idx
  let i0 = b.alloc_i32();
  let i1 = b.alloc_i32();
  let table1 = b.alloc_i32();
  let bucket = b.alloc_i32();
  let count = b.alloc_i32();
  let bi = b.alloc_i32();
  let out = b.alloc_i32();
  let addr = b.alloc_i32();
  let tmp = b.alloc_f64();

  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalSet(out));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i0));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i0));
  b.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  rt_emit_load_table_child(&mut b, 0, i0, table1);
  b.emit(Instruction::LocalGet(table1));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i1));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i1));
  b.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  rt_emit_load_table_child(&mut b, table1, i1, bucket);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(bi));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));

  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(addr));

  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(tmp));
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(tmp));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));

  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(tmp));
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(tmp));
  b.emit(Instruction::F64Store(mem_arg_f64(8)));

  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out));
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(bi));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i1));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i1));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i0));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i0));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(out));
  b.finish(vec![ValType::I32, ValType::I32, ValType::I32], vec![ValType::I32])
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_from_flat(
  scalar_hash_idx: u32,
  value_hash_idx: u32,
  heap_value_hash_idx: u32,
  root_assoc_idx: u32,
  map_make_idx: u32,
) -> CompiledFn {
  build_rt_map_from_flat_impl(
    scalar_hash_idx,
    value_hash_idx,
    heap_value_hash_idx,
    root_assoc_idx,
    map_make_idx,
    false,
  )
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_from_flat_checked(
  scalar_hash_idx: u32,
  value_hash_idx: u32,
  heap_value_hash_idx: u32,
  root_assoc_idx: u32,
  map_make_idx: u32,
) -> CompiledFn {
  build_rt_map_from_flat_impl(
    scalar_hash_idx,
    value_hash_idx,
    heap_value_hash_idx,
    root_assoc_idx,
    map_make_idx,
    true,
  )
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_from_flat_impl(
  scalar_hash_idx: u32,
  value_hash_idx: u32,
  heap_value_hash_idx: u32,
  root_assoc_idx: u32,
  map_make_idx: u32,
  checked: bool,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let count = b.alloc_i32();
  let kv_kind = b.alloc_i32();
  let key_kind = b.alloc_i32();
  let i = b.alloc_i32();
  let root = b.alloc_i32();
  let actual = b.alloc_i32();
  let key = b.alloc_f64();
  let val = b.alloc_f64();
  let hash = b.alloc_i32();
  let added = b.alloc_i32();
  let req_bytes = if checked { Some(b.alloc_i32()) } else { None };

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalTee(kv_kind));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(key_kind));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(actual));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(key));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(val));
  if let Some(req) = req_bytes {
    // Each assoc allocates at most two 272-byte HAMT tables plus one bucket of up to `actual + 1` entries (`16 * (actual + 1) + 24`).
    b.emit(Instruction::LocalGet(actual));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::I32Const(16));
    b.emit(Instruction::I32Mul);
    b.emit(Instruction::I32Const(568));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(req));
    rt_emit_ensure_heap_capacity(&mut b, req);
    b.emit(Instruction::I32Eqz);
    b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::Return);
    b.emit(Instruction::End);
  }
  rt_emit_hash_with_kind(&mut b, key, key_kind, hash, scalar_hash_idx, value_hash_idx, heap_value_hash_idx);
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::LocalGet(key_kind));
  b.emit(Instruction::Call(root_assoc_idx));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(actual));
  b.emit(Instruction::LocalGet(added));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(actual));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  if let Some(req) = req_bytes {
    rt_emit_ensure_heap_capacity_const(&mut b, 24, req);
    b.emit(Instruction::I32Eqz);
    b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::Return);
    b.emit(Instruction::End);
  }
  b.emit(Instruction::LocalGet(actual));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(kv_kind));
  b.emit(Instruction::Call(map_make_idx));
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_root_from_flat(
  scalar_hash_idx: u32,
  value_hash_idx: u32,
  heap_value_hash_idx: u32,
  root_assoc_idx: u32,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let count = b.alloc_i32();
  let key_kind = b.alloc_i32();
  let i = b.alloc_i32();
  let root = b.alloc_i32();
  let actual = b.alloc_i32();
  let key = b.alloc_f64();
  let val = b.alloc_f64();
  let hash = b.alloc_i32();
  let added = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(key_kind));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(actual));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(key));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(val));
  rt_emit_hash_with_kind(&mut b, key, key_kind, hash, scalar_hash_idx, value_hash_idx, heap_value_hash_idx);
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::LocalGet(key_kind));
  b.emit(Instruction::Call(root_assoc_idx));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(actual));
  b.emit(Instruction::LocalGet(added));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(actual));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(actual));
  b.emit(Instruction::LocalGet(root));
  b.finish(vec![ValType::I32], vec![ValType::I32, ValType::I32])
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_linearize(root_write_pairs_idx: u32, list_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let count = b.alloc_i32();
  let kv_kind = b.alloc_i32();
  let root = b.alloc_i32();
  let slots = b.alloc_i32();
  let size = b.alloc_i32();
  let dst = b.alloc_i32();
  let base = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(kv_kind));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  b.emit(Instruction::LocalGet(slots));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(&mut b, size, dst, list_tag);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(kv_kind));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(base));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(base));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Call(root_write_pairs_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::LocalGet(dst));
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_assoc(
  scalar_hash_idx: u32,
  value_hash_idx: u32,
  heap_value_hash_idx: u32,
  root_assoc_idx: u32,
  map_make_idx: u32,
  map_linearize_idx: u32,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(4);
  let count = b.alloc_i32();
  let old_kv_kind = b.alloc_i32();
  let old_k = b.alloc_i32();
  let old_v = b.alloc_i32();
  let add_k = b.alloc_i32();
  let add_v = b.alloc_i32();
  let new_k = b.alloc_i32();
  let new_v = b.alloc_i32();
  let new_kv_kind = b.alloc_i32();
  let root = b.alloc_i32();
  let hash = b.alloc_i32();
  let added = b.alloc_i32();
  let new_count = b.alloc_i32();
  let flat = b.alloc_i32();
  let i = b.alloc_i32();
  let rebuilt_count = b.alloc_i32();
  let rk = b.alloc_f64();
  let rv = b.alloc_f64();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalTee(old_kv_kind));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(old_k));
  b.emit(Instruction::LocalGet(old_kv_kind));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(old_v));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(add_k));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(add_v));
  rt_emit_join_slot_kind(&mut b, old_k, add_k, new_k);
  rt_emit_join_slot_kind(&mut b, old_v, add_v, new_v);
  b.emit(Instruction::LocalGet(new_k));
  b.emit(Instruction::LocalGet(new_v));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalSet(new_kv_kind));

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalSet(root));
  // When widening PureScalar -> DynamicBoxed, re-index under DynamicBoxed value_hash
  // so lookups remain in the matching buckets, while comparing existing PureScalar keys
  // with PureScalar equality so two distinct heap-aliased numbers never merge, and
  // keep the exact rebuilt entry count.
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::LocalGet(old_k));
  b.emit(Instruction::I32Const(ContainerSlotKind::PureScalar as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalGet(new_k));
  b.emit(Instruction::I32Const(ContainerSlotKind::DynamicBoxed as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(map_linearize_idx));
  b.emit(Instruction::LocalSet(flat));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(rebuilt_count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(rk));
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(rv));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(rk));
  b.emit(Instruction::LocalGet(rv));
  b.emit(Instruction::LocalGet(rk));
  b.emit(Instruction::Call(value_hash_idx));
  b.emit(Instruction::I32Const(ContainerSlotKind::PureScalar as i32));
  b.emit(Instruction::Call(root_assoc_idx));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(rebuilt_count));
  b.emit(Instruction::LocalGet(added));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(rebuilt_count));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(rebuilt_count));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::End);
  rt_emit_hash_with_kind(&mut b, 1, new_k, hash, scalar_hash_idx, value_hash_idx, heap_value_hash_idx);
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::LocalGet(new_k));
  b.emit(Instruction::Call(root_assoc_idx));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::LocalGet(added));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(new_count));
  b.emit(Instruction::LocalGet(new_count));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(new_kv_kind));
  b.emit(Instruction::Call(map_make_idx));
  b.finish(vec![ValType::I32, ValType::F64, ValType::F64, ValType::I32], vec![ValType::I32])
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_get_value(
  scalar_hash_idx: u32,
  value_hash_idx: u32,
  heap_value_hash_idx: u32,
  root_lookup_idx: u32,
  is_heap_ptr_idx: u32,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let key_kind = b.alloc_i32();
  let root = b.alloc_i32();
  let hash = b.alloc_i32();
  let found = b.alloc_i32();
  let value = b.alloc_f64();
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalTee(key_kind));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(is_heap_ptr_idx));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(f64_const(0.0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalSet(root));
  rt_emit_hash_with_kind(&mut b, 1, key_kind, hash, scalar_hash_idx, value_hash_idx, heap_value_hash_idx);
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::LocalGet(key_kind));
  b.emit(Instruction::Call(root_lookup_idx));
  b.emit(Instruction::LocalSet(value));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(value));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::F64])
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_contains_key(
  scalar_hash_idx: u32,
  value_hash_idx: u32,
  heap_value_hash_idx: u32,
  root_lookup_idx: u32,
  is_heap_ptr_idx: u32,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let key_kind = b.alloc_i32();
  let root = b.alloc_i32();
  let hash = b.alloc_i32();
  let found = b.alloc_i32();
  let value = b.alloc_f64();
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalTee(key_kind));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(is_heap_ptr_idx));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalSet(root));
  rt_emit_hash_with_kind(&mut b, 1, key_kind, hash, scalar_hash_idx, value_hash_idx, heap_value_hash_idx);
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::LocalGet(key_kind));
  b.emit(Instruction::Call(root_lookup_idx));
  b.emit(Instruction::LocalSet(value));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(found));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_contains_value(root_contains_value_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let root = b.alloc_i32();
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(root_contains_value_idx));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

pub(in crate::wasm::emit_wasm) fn build_rt_map_dissoc(
  copy_fn_idx: u32,
  map_linearize_idx: u32,
  map_from_flat_idx: u32,
  key_equal_idx: u32,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let count = b.alloc_i32();
  let kv_kind = b.alloc_i32();
  let key_kind = b.alloc_i32();
  let flat = b.alloc_i32();
  let found = b.alloc_i32();
  let found_idx = b.alloc_i32();
  let i = b.alloc_i32();
  let new_count = b.alloc_i32();
  let slots = b.alloc_i32();
  let size = b.alloc_i32();
  let dst = b.alloc_i32();
  let before_slots = b.alloc_i32();
  let after_slots = b.alloc_i32();
  let dst_base = b.alloc_i32();
  let src_base = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalTee(kv_kind));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(key_kind));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(map_linearize_idx));
  b.emit(Instruction::LocalSet(flat));
  rt_emit_find_key_in_pairs(&mut b, flat, 8, count, 1, key_kind, key_equal_idx, i, found_idx);
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::LocalSet(found));

  b.emit(Instruction::LocalGet(found));
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(new_count));
  b.emit(Instruction::LocalGet(new_count));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  b.emit(Instruction::LocalGet(slots));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(&mut b, size, dst, 0);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(new_count));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(kv_kind));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(before_slots));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst_base));
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(src_base));
  rt_emit_copy_slots(&mut b, copy_fn_idx, dst_base, src_base, before_slots);
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(after_slots));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst_base));
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(src_base));
  rt_emit_copy_slots(&mut b, copy_fn_idx, dst_base, src_base, after_slots);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::Call(map_from_flat_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

/// `__rt_map_equal(a: i32, b: i32) → i32`
/// Returns 1 if maps a and b have the same key-value pairs, else 0.
pub(in crate::wasm::emit_wasm) fn build_rt_map_equal(
  map_linearize_idx: u32,
  map_root_lookup_idx: u32,
  (scalar_hash_idx, value_hash_idx, heap_value_hash_idx): (u32, u32, u32),
  (f64_value_eq_idx, value_equal_idx, heap_value_equal_idx, is_heap_ptr_idx): (u32, u32, u32, u32),
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2); // a=0, b=1
  let count_a = b.alloc_i32();
  let count_b = b.alloc_i32();
  let kv_a = b.alloc_i32();
  let kv_b = b.alloc_i32();
  let key_kind_a = b.alloc_i32();
  let key_kind_b = b.alloc_i32();
  let val_kind_a = b.alloc_i32();
  let val_kind_b = b.alloc_i32();
  let flat = b.alloc_i32();
  let root_b = b.alloc_i32();
  let i = b.alloc_i32();
  let hash = b.alloc_i32();
  let found = b.alloc_i32();
  let key = b.alloc_f64();
  let val_a = b.alloc_f64();
  let val_b = b.alloc_f64();
  let all_eq = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count_a));

  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count_b));

  // if count_a != count_b → return 0
  b.emit(Instruction::LocalGet(count_a));
  b.emit(Instruction::LocalGet(count_b));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Else);

  // if count_a == 0 → return 1
  b.emit(Instruction::LocalGet(count_a));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Else);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalTee(kv_a));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(key_kind_a));
  b.emit(Instruction::LocalGet(kv_a));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(val_kind_a));

  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalTee(kv_b));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(key_kind_b));
  b.emit(Instruction::LocalGet(kv_b));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(val_kind_b));

  // linearize a → flat; root_b = b.I32Load(8)
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(map_linearize_idx));
  b.emit(Instruction::LocalSet(flat));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalSet(root_b));

  // all_eq = 1, i = 0
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(all_eq));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));

  // loop over pairs
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  // if i >= count_a, exit
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count_a));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));

  // key = flat + 8 + i*16
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(key));

  // val_a = flat + 8 + i*16 + 8
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::LocalSet(val_a));

  // Disjoint pure-scalar vs exact-heap-ptr non-empty maps can never match.
  b.emit(Instruction::LocalGet(key_kind_a));
  b.emit(Instruction::I32Const(ContainerSlotKind::PureScalar as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(key_kind_b));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalGet(key_kind_a));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(key_kind_b));
  b.emit(Instruction::I32Const(ContainerSlotKind::PureScalar as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(all_eq));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);

  // If map_b expects ExactHeapPtr keys, a non-heap key from map_a (e.g. DynamicBoxed scalar)
  // cannot match and must not be truncated via I32TruncF64U.
  b.emit(Instruction::LocalGet(key_kind_b));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(key_kind_a));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::Call(is_heap_ptr_idx));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(all_eq));
  b.emit(Instruction::Br(3));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // (found, val_b) = __rt_map_root_lookup(root_b, key, hash(key, key_kind_b), key_kind_b)
  rt_emit_hash_with_kind(&mut b, key, key_kind_b, hash, scalar_hash_idx, value_hash_idx, heap_value_hash_idx);
  b.emit(Instruction::LocalGet(root_b));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::LocalGet(key_kind_b));
  b.emit(Instruction::Call(map_root_lookup_idx));
  b.emit(Instruction::LocalSet(val_b));
  b.emit(Instruction::LocalSet(found));

  // if found == 0 → all_eq = 0, break
  b.emit(Instruction::LocalGet(found));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(all_eq));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);

  // if val_a != val_b → all_eq = 0, break (fast-path PureScalar / ExactHeapPtr)
  rt_emit_eq_with_pair_kinds(
    &mut b,
    (val_a, val_b),
    (val_kind_a, val_kind_b),
    (f64_value_eq_idx, value_equal_idx, heap_value_equal_idx, is_heap_ptr_idx),
  );
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(all_eq));
  b.emit(Instruction::Br(2)); // break out of block
  b.emit(Instruction::End);

  // i++
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End); // loop
  b.emit(Instruction::End); // block

  b.emit(Instruction::LocalGet(all_eq));
  b.emit(Instruction::End); // end count_a == 0 else
  b.emit(Instruction::End); // end count_a != count_b if

  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}

/// `__rt_set_equal(a: i32, b: i32) → i32`
/// Compare unordered set elements using kind-aware structural equality.
pub(in crate::wasm::emit_wasm) fn build_rt_set_equal(
  value_equal_idx: u32,
  heap_value_equal_idx: u32,
  f64_value_eq_idx: u32,
  is_heap_ptr_idx: u32,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let count = b.alloc_i32();
  let kind_a = b.alloc_i32();
  let kind_b = b.alloc_i32();
  let i = b.alloc_i32();
  let j = b.alloc_i32();
  let found = b.alloc_i32();
  let result = b.alloc_i32();
  let elem = b.alloc_f64();
  let elem_b = b.alloc_f64();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(kind_a));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(kind_b));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(elem));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(elem_b));
  rt_emit_eq_with_pair_kinds(
    &mut b,
    (elem, elem_b),
    (kind_a, kind_b),
    (f64_value_eq_idx, value_equal_idx, heap_value_equal_idx, is_heap_ptr_idx),
  );
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End); // inner loop
  b.emit(Instruction::End); // inner block
  b.emit(Instruction::LocalGet(found));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End); // outer loop
  b.emit(Instruction::End); // outer block
  b.emit(Instruction::LocalGet(result));
  b.emit(Instruction::End); // count comparison
  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}

/// `__rt_set_find_elem(set_ptr: i32, needle: f64) → i32`
/// Find an element in a set using kind-aware structural equality; returns index or -1.
pub(in crate::wasm::emit_wasm) fn build_rt_set_find_elem(
  value_equal_idx: u32,
  heap_value_equal_idx: u32,
  f64_value_eq_idx: u32,
  is_heap_ptr_idx: u32,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let count = b.alloc_i32();
  let elem_kind = b.alloc_i32();
  let i = b.alloc_i32();
  let found_idx = b.alloc_i32();
  let elem = b.alloc_f64();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(elem_kind));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(found_idx));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(elem));
  rt_emit_eq_with_single_kind(
    &mut b,
    elem,
    1,
    elem_kind,
    (f64_value_eq_idx, value_equal_idx, heap_value_equal_idx, is_heap_ptr_idx),
  );
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalSet(found_idx));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(found_idx));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
  use super::*;
  use wasm_encoder::BlockType;
  use wasmtime::{Engine, Instance, Module, Store};

  #[test]
  fn map_dissoc_prefix_copy_does_not_overwrite_past_flat_buffer() {
    let copy_fn = CompiledFn {
      export_name: None,
      params: vec![ValType::I32, ValType::I32, ValType::I32],
      results: vec![],
      locals: vec![ValType::I32],
      instructions: vec![
        Instruction::I32Const(0),
        Instruction::LocalSet(3),
        Instruction::Block(BlockType::Empty),
        Instruction::Loop(BlockType::Empty),
        Instruction::LocalGet(3),
        Instruction::LocalGet(2),
        Instruction::I32GeU,
        Instruction::BrIf(1),
        Instruction::LocalGet(0),
        Instruction::LocalGet(3),
        Instruction::I32Const(8),
        Instruction::I32Mul,
        Instruction::I32Add,
        Instruction::LocalGet(1),
        Instruction::LocalGet(3),
        Instruction::I32Const(8),
        Instruction::I32Mul,
        Instruction::I32Add,
        Instruction::F64Load(mem_arg_f64(0)),
        Instruction::F64Store(mem_arg_f64(0)),
        Instruction::LocalGet(3),
        Instruction::I32Const(1),
        Instruction::I32Add,
        Instruction::LocalSet(3),
        Instruction::Br(0),
        Instruction::End,
        Instruction::End,
      ],
    };
    let linearize_stub = CompiledFn {
      export_name: None,
      params: vec![ValType::I32],
      results: vec![ValType::I32],
      locals: vec![],
      instructions: vec![Instruction::LocalGet(0), Instruction::I32Load(mem_arg_i32(8))],
    };
    let from_flat_stub = CompiledFn {
      export_name: None,
      params: vec![ValType::I32],
      results: vec![ValType::I32],
      locals: vec![],
      instructions: vec![Instruction::LocalGet(0)],
    };
    let key_equal_stub = CompiledFn {
      export_name: None,
      params: vec![ValType::F64, ValType::F64, ValType::I32],
      results: vec![ValType::I32],
      locals: vec![],
      instructions: vec![Instruction::LocalGet(0), Instruction::LocalGet(1), Instruction::F64Eq],
    };
    let mut dissoc_fn = build_rt_map_dissoc(0, 1, 2, 3);
    dissoc_fn.export_name = Some("dissoc".into());

    let bytes = build_wasm_module(
      &[copy_fn, linearize_stub, from_flat_stub, key_equal_stub, dissoc_fn],
      &[],
      HEAP_BASE,
      &[],
      &[],
      1,
      ModuleFunctionLayout {
        number_format_fn_count: 0,
        memory_layout: WasmMemoryLayout::compute(HEAP_BASE, 0),
        global_layout: WasmGlobalLayout::compute(0, false),
        prune_unused_imports: false,
      },
    )
    .expect("valid map_dissoc module");

    let engine = Engine::default();
    let module = Module::new(&engine, &bytes).expect("compile map_dissoc wasm");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate map_dissoc");
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let heap_ptr = instance.get_global(&mut store, "__heap_ptr").expect("heap ptr global");
    let dissoc = instance
      .get_typed_func::<(i32, f64), i32>(&mut store, "dissoc")
      .expect("dissoc export");

    const CANARY: u64 = 0xDEADBEEF_CAFEBABEu64;
    let fill_canary = |store: &mut Store<()>| {
      heap_ptr.set(&mut *store, wasmtime::Val::I32(HEAP_BASE)).expect("reset heap_ptr");
      for offset in (0..512usize).step_by(8) {
        memory
          .write(&mut *store, HEAP_BASE as usize + offset, &CANARY.to_le_bytes())
          .expect("write canary");
      }
    };
    let read_u32 = |store: &Store<()>, addr: usize| -> u32 {
      let mut buf = [0u8; 4];
      memory.read(store, addr, &mut buf).expect("read u32");
      u32::from_le_bytes(buf)
    };
    let write_map_header = |store: &mut Store<()>, addr: usize, count: u32, kv_kind: u32, root_ptr: i32| {
      memory.write(&mut *store, addr, &count.to_le_bytes()).expect("count");
      memory.write(&mut *store, addr + 4, &kv_kind.to_le_bytes()).expect("kv_kind");
      memory.write(&mut *store, addr + 8, &root_ptr.to_le_bytes()).expect("root_ptr");
      memory.write(&mut *store, addr + 12, &0i32.to_le_bytes()).expect("reserved");
    };
    let write_flat_header = |store: &mut Store<()>, addr: usize, count: u32, kv_kind: u32| {
      memory.write(&mut *store, addr, &count.to_le_bytes()).expect("count");
      memory.write(&mut *store, addr + 4, &kv_kind.to_le_bytes()).expect("kv_kind");
    };
    let read_f64 = |store: &Store<()>, addr: usize| -> f64 {
      let mut buf = [0u8; 8];
      memory.read(store, addr, &mut buf).expect("read f64");
      f64::from_le_bytes(buf)
    };
    let assert_canary_intact = |store: &Store<()>, start_addr: usize| {
      for addr in (start_addr..HEAP_BASE as usize + 512).step_by(8) {
        let mut buf = [0u8; 8];
        memory.read(store, addr, &mut buf).expect("read canary");
        assert_eq!(
          u64::from_le_bytes(buf),
          CANARY,
          "dissoc wrote past allocated flat buffer at {addr:#x}"
        );
      }
    };

    // Case 1: removing the sole entry of a 1-entry map (`found_idx == 0`).
    fill_canary(&mut store);
    let pure_kv = ContainerSlotKind::pack_kv(ContainerSlotKind::PureScalar, ContainerSlotKind::PureScalar);
    write_flat_header(&mut store, 1024, 1, pure_kv);
    for (slot, val) in [10.0f64, 100.0].into_iter().enumerate() {
      memory.write(&mut store, 1024 + (slot + 1) * 8, &val.to_le_bytes()).expect("flat");
    }
    write_map_header(&mut store, 2048, 1, pure_kv, 1024);
    let dst = dissoc.call(&mut store, (2048, 10.0)).expect("dissoc sole entry") as usize;
    assert_eq!(dst, HEAP_BASE as usize + 8);
    assert_eq!(read_u32(&store, dst), 0);
    assert_eq!(read_u32(&store, dst + 4), pure_kv);
    assert_canary_intact(&store, dst + 8);

    // Case 2: removing the last entry of a 2-entry map (`found_idx == 1`).
    fill_canary(&mut store);
    write_flat_header(&mut store, 1024, 2, pure_kv);
    for (slot, val) in [10.0f64, 100.0, 20.0, 200.0].into_iter().enumerate() {
      memory.write(&mut store, 1024 + (slot + 1) * 8, &val.to_le_bytes()).expect("flat");
    }
    write_map_header(&mut store, 2048, 2, pure_kv, 1024);
    let dst_last = dissoc.call(&mut store, (2048, 20.0)).expect("dissoc last entry") as usize;
    assert_eq!(dst_last, HEAP_BASE as usize + 8);
    assert_eq!(read_u32(&store, dst_last), 1);
    assert_eq!(read_u32(&store, dst_last + 4), pure_kv);
    assert_eq!(read_f64(&store, dst_last + 8), 10.0);
    assert_eq!(read_f64(&store, dst_last + 16), 100.0);
    assert_canary_intact(&store, dst_last + 24);

    // Case 3: removing the first entry of a 2-entry map (`found_idx == 0`).
    fill_canary(&mut store);
    let dst_first = dissoc.call(&mut store, (2048, 10.0)).expect("dissoc first entry") as usize;
    assert_eq!(dst_first, HEAP_BASE as usize + 8);
    assert_eq!(read_u32(&store, dst_first), 1);
    assert_eq!(read_u32(&store, dst_first + 4), pure_kv);
    assert_eq!(read_f64(&store, dst_first + 8), 20.0);
    assert_eq!(read_f64(&store, dst_first + 16), 200.0);
    assert_canary_intact(&store, dst_first + 24);
  }
}
