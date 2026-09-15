use std::collections::{BTreeMap, BTreeSet};

use super::*;
use wasm_encoder::BlockType;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ComponentListCodec {
  pub lift_index: u32,
  pub lower_index: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ComponentVariantCodec {
  pub lift_index: u32,
  pub lower_index: u32,
}

pub(super) struct ComponentValueCodecs<'a> {
  pub str_new_index: u32,
  pub buffer_new_index: u32,
  pub list_codecs: &'a BTreeMap<ComponentAbiType, ComponentListCodec>,
  pub variant_codecs: &'a BTreeMap<ComponentAbiType, ComponentVariantCodec>,
}

#[derive(Clone, Copy)]
pub(super) struct ComponentMemoryLayout {
  pub size: i32,
  pub alignment: i32,
}

pub(super) fn component_memory_layout(value_type: &ComponentAbiType) -> ComponentMemoryLayout {
  match value_type {
    ComponentAbiType::Unit => ComponentMemoryLayout { size: 0, alignment: 1 },
    ComponentAbiType::Bool => ComponentMemoryLayout { size: 1, alignment: 1 },
    ComponentAbiType::Number => ComponentMemoryLayout { size: 8, alignment: 8 },
    ComponentAbiType::Buffer | ComponentAbiType::String | ComponentAbiType::List(_) => ComponentMemoryLayout { size: 8, alignment: 4 },
    ComponentAbiType::Option(item) => component_variant_layout(&ComponentAbiType::Unit, item),
    ComponentAbiType::Result(ok, error) => component_variant_layout(ok, error),
  }
}

fn push_zero(instructions: &mut Vec<Instruction<'static>>, value_type: ValType) {
  instructions.push(match value_type {
    ValType::I32 => Instruction::I32Const(0),
    ValType::I64 => Instruction::I64Const(0),
    ValType::F64 => f64_const(0.0),
    other => panic!("unsupported Component flat zero: {other:?}"),
  });
}

fn push_flat_conversion(instructions: &mut Vec<Instruction<'static>>, from: ValType, to: ValType) {
  match (from, to) {
    (from, to) if from == to => {}
    (ValType::I32, ValType::I64) => instructions.push(Instruction::I64ExtendI32U),
    (ValType::F64, ValType::I64) => instructions.push(Instruction::I64ReinterpretF64),
    (ValType::I64, ValType::I32) => instructions.push(Instruction::I32WrapI64),
    (ValType::I64, ValType::F64) => instructions.push(Instruction::F64ReinterpretI64),
    (from, to) => panic!("unsupported Component flat conversion: {from:?} to {to:?}"),
  }
}

fn component_payload_flat_types(value_type: &ComponentAbiType) -> Vec<ValType> {
  component_flat_types(value_type)
}

fn push_store_flat_payload(
  instructions: &mut Vec<Instruction<'static>>,
  payload_type: &ComponentAbiType,
  source_types: &[ValType],
  flat_start: u32,
  dst_local: u32,
  dst_offset: i32,
) {
  let actual_types = component_payload_flat_types(payload_type);
  match payload_type {
    ComponentAbiType::Unit => {}
    ComponentAbiType::Bool => {
      instructions.extend([Instruction::LocalGet(dst_local), Instruction::LocalGet(flat_start)]);
      push_flat_conversion(instructions, source_types[0], actual_types[0]);
      instructions.push(Instruction::I32Store8(mem_arg_byte(dst_offset as u64)));
    }
    ComponentAbiType::Number => {
      instructions.extend([Instruction::LocalGet(dst_local), Instruction::LocalGet(flat_start)]);
      push_flat_conversion(instructions, source_types[0], actual_types[0]);
      instructions.push(Instruction::F64Store(mem_arg_f64(dst_offset as u64)));
    }
    ComponentAbiType::Buffer | ComponentAbiType::List(_) | ComponentAbiType::String => {
      for index in 0..2 {
        instructions.extend([Instruction::LocalGet(dst_local), Instruction::LocalGet(flat_start + index)]);
        push_flat_conversion(instructions, source_types[index as usize], actual_types[index as usize]);
        instructions.push(Instruction::I32Store(mem_arg_i32((dst_offset + index as i32 * 4) as u64)));
      }
    }
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) => {
      push_store_variant_flat_from(instructions, payload_type, source_types, flat_start, dst_local, dst_offset);
    }
  }
}

fn push_store_variant_flat_from(
  instructions: &mut Vec<Instruction<'static>>,
  variant_type: &ComponentAbiType,
  source_types: &[ValType],
  flat_start: u32,
  dst_local: u32,
  base_offset: i32,
) {
  let (left, right) = match variant_type {
    ComponentAbiType::Option(item) => (&ComponentAbiType::Unit, item.as_ref()),
    ComponentAbiType::Result(ok, error) => (ok.as_ref(), error.as_ref()),
    _ => unreachable!("flat variant store requires Option or Result"),
  };
  let payload_offset = component_variant_payload_offset(variant_type);
  instructions.push(Instruction::LocalGet(flat_start));
  push_flat_conversion(instructions, source_types[0], ValType::I32);
  instructions.extend([
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
  instructions.extend([Instruction::LocalGet(dst_local), Instruction::LocalGet(flat_start)]);
  push_flat_conversion(instructions, source_types[0], ValType::I32);
  instructions.extend([
    Instruction::I32Store8(mem_arg_byte(base_offset as u64)),
    Instruction::LocalGet(flat_start),
  ]);
  push_flat_conversion(instructions, source_types[0], ValType::I32);
  instructions.push(Instruction::If(BlockType::Empty));
  push_store_flat_payload(
    instructions,
    right,
    &source_types[1..],
    flat_start + 1,
    dst_local,
    base_offset + payload_offset,
  );
  instructions.push(Instruction::Else);
  push_store_flat_payload(
    instructions,
    left,
    &source_types[1..],
    flat_start + 1,
    dst_local,
    base_offset + payload_offset,
  );
  instructions.push(Instruction::End);
}

pub(super) fn push_store_variant_flat(
  instructions: &mut Vec<Instruction<'static>>,
  variant_type: &ComponentAbiType,
  flat_start: u32,
  dst_local: u32,
  base_offset: i32,
) {
  push_store_variant_flat_from(
    instructions,
    variant_type,
    &component_flat_types(variant_type),
    flat_start,
    dst_local,
    base_offset,
  );
}

fn push_load_value_flat_slot(
  instructions: &mut Vec<Instruction<'static>>,
  value_type: &ComponentAbiType,
  slot: usize,
  desired_type: ValType,
  src_local: u32,
  src_offset: i32,
) {
  let actual_types = component_payload_flat_types(value_type);
  let Some(actual_type) = actual_types.get(slot).copied() else {
    push_zero(instructions, desired_type);
    return;
  };
  match value_type {
    ComponentAbiType::Bool => instructions.extend([
      Instruction::LocalGet(src_local),
      Instruction::I32Load8U(mem_arg_byte(src_offset as u64)),
    ]),
    ComponentAbiType::Number => instructions.extend([
      Instruction::LocalGet(src_local),
      Instruction::F64Load(mem_arg_f64(src_offset as u64)),
    ]),
    ComponentAbiType::Buffer | ComponentAbiType::List(_) | ComponentAbiType::String => instructions.extend([
      Instruction::LocalGet(src_local),
      Instruction::I32Load(mem_arg_i32((src_offset + slot as i32 * 4) as u64)),
    ]),
    ComponentAbiType::Unit => unreachable!(),
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) => {
      push_load_variant_flat_slot(instructions, value_type, slot, actual_type, src_local, src_offset)
    }
  }
  push_flat_conversion(instructions, actual_type, desired_type);
}

fn push_load_variant_flat_slot(
  instructions: &mut Vec<Instruction<'static>>,
  variant_type: &ComponentAbiType,
  slot: usize,
  desired_type: ValType,
  src_local: u32,
  base_offset: i32,
) {
  let (left, right) = match variant_type {
    ComponentAbiType::Option(item) => (&ComponentAbiType::Unit, item.as_ref()),
    ComponentAbiType::Result(ok, error) => (ok.as_ref(), error.as_ref()),
    _ => unreachable!("flat variant load requires Option or Result"),
  };
  let joined = component_join_flat_types(&component_flat_types(left), &component_flat_types(right));
  if slot == 0 {
    instructions.extend([
      Instruction::LocalGet(src_local),
      Instruction::I32Load8U(mem_arg_byte(base_offset as u64)),
    ]);
    push_flat_conversion(instructions, ValType::I32, desired_type);
    return;
  }
  let Some(joined_type) = joined.get(slot - 1).copied() else {
    push_zero(instructions, desired_type);
    return;
  };
  let payload_offset = component_variant_payload_offset(variant_type);
  instructions.extend([
    Instruction::LocalGet(src_local),
    Instruction::I32Load8U(mem_arg_byte(base_offset as u64)),
    Instruction::If(BlockType::Result(joined_type)),
  ]);
  push_load_value_flat_slot(instructions, right, slot - 1, joined_type, src_local, base_offset + payload_offset);
  instructions.push(Instruction::Else);
  push_load_value_flat_slot(instructions, left, slot - 1, joined_type, src_local, base_offset + payload_offset);
  instructions.push(Instruction::End);
  push_flat_conversion(instructions, joined_type, desired_type);
}

pub(super) fn push_load_variant_flat(instructions: &mut Vec<Instruction<'static>>, variant_type: &ComponentAbiType, src_local: u32) {
  for (slot, flat_type) in component_flat_types(variant_type).into_iter().enumerate() {
    push_load_variant_flat_slot(instructions, variant_type, slot, flat_type, src_local, 0);
  }
}

fn align_to(value: i32, alignment: i32) -> i32 {
  (value + alignment - 1) / alignment * alignment
}

fn component_variant_layout(left: &ComponentAbiType, right: &ComponentAbiType) -> ComponentMemoryLayout {
  let left = component_memory_layout(left);
  let right = component_memory_layout(right);
  let alignment = left.alignment.max(right.alignment);
  let payload_offset = align_to(1, alignment);
  ComponentMemoryLayout {
    size: align_to(payload_offset + left.size.max(right.size), alignment),
    alignment,
  }
}

fn component_variant_payload_offset(value_type: &ComponentAbiType) -> i32 {
  match value_type {
    ComponentAbiType::Option(item) => component_memory_layout(item).alignment,
    ComponentAbiType::Result(ok, error) => component_memory_layout(ok).alignment.max(component_memory_layout(error).alignment),
    _ => unreachable!("payload offset requires a Component variant type"),
  }
}

fn collect_compound_type(value_type: &ComponentAbiType, seen: &mut BTreeSet<ComponentAbiType>, ordered: &mut Vec<ComponentAbiType>) {
  match value_type {
    ComponentAbiType::List(item) | ComponentAbiType::Option(item) => collect_compound_type(item, seen, ordered),
    ComponentAbiType::Result(ok, error) => {
      collect_compound_type(ok, seen, ordered);
      collect_compound_type(error, seen, ordered);
    }
    _ => return,
  }
  if seen.insert(value_type.clone()) {
    ordered.push(value_type.clone());
  }
}

pub(super) fn collect_component_compound_types(
  program_data: &program::CompiledProgram,
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  import_adapters: &[ComponentImportAdapter],
) -> Result<Vec<ComponentAbiType>, String> {
  let mut seen = BTreeSet::new();
  let mut ordered = Vec::new();
  for adapter in import_adapters {
    for parameter in &adapter.parameters {
      collect_compound_type(parameter, &mut seen, &mut ordered);
    }
    collect_compound_type(&adapter.result, &mut seen, &mut ordered);
  }
  for (namespace, name, args, _) in fn_defs {
    let Some(compiled) = program_data.get(namespace.as_str()).and_then(|file| file.defs.get(name.as_str())) else {
      continue;
    };
    if !is_wasm_export_def(&compiled.preprocessed_code) {
      continue;
    }
    let definition = format!("{namespace}/{name}");
    let (parameters, result) = component_function_schema(compiled, &definition, fn_param_names(args).len())?;
    for parameter in &parameters {
      collect_compound_type(parameter, &mut seen, &mut ordered);
    }
    collect_compound_type(&result, &mut seen, &mut ordered);
  }
  Ok(ordered)
}

fn push_checked_u32_product(instructions: &mut Vec<Instruction<'static>>, value_local: u32, factor: i32, result_local: u32) {
  instructions.extend([
    Instruction::LocalGet(value_local),
    Instruction::I64ExtendI32U,
    Instruction::I64Const(i64::from(factor)),
    Instruction::I64Mul,
    Instruction::LocalTee(result_local),
    Instruction::I64Const(i64::from(u32::MAX)),
    Instruction::I64GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
}

fn push_checked_alignment(instructions: &mut Vec<Instruction<'static>>, ptr_local: u32, alignment: i32) {
  if alignment <= 1 {
    return;
  }
  instructions.extend([
    Instruction::LocalGet(ptr_local),
    Instruction::I32Const(alignment - 1),
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
}

fn push_checked_memory_region(instructions: &mut Vec<Instruction<'static>>, ptr_local: u32, byte_len_local: u32) {
  instructions.extend([
    Instruction::LocalGet(ptr_local),
    Instruction::I64ExtendI32U,
    Instruction::LocalGet(byte_len_local),
    Instruction::I64Add,
    Instruction::MemorySize(0),
    Instruction::I64ExtendI32U,
    Instruction::I64Const(16),
    Instruction::I64Shl,
    Instruction::I64GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
}

fn push_checked_memory_region_const(instructions: &mut Vec<Instruction<'static>>, ptr_local: u32, byte_len: i64) {
  instructions.extend([
    Instruction::LocalGet(ptr_local),
    Instruction::I64ExtendI32U,
    Instruction::I64Const(byte_len),
    Instruction::I64Add,
    Instruction::MemorySize(0),
    Instruction::I64ExtendI32U,
    Instruction::I64Const(16),
    Instruction::I64Shl,
    Instruction::I64GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
}

fn push_lift_element(
  instructions: &mut Vec<Instruction<'static>>,
  item_type: &ComponentAbiType,
  src_addr: u32,
  bool_value: u32,
  codecs: &ComponentValueCodecs<'_>,
) {
  match item_type {
    ComponentAbiType::Unit => instructions.push(f64_const(0.0)),
    ComponentAbiType::Bool => instructions.extend([
      Instruction::LocalGet(src_addr),
      Instruction::I32Load8U(mem_arg_byte(0)),
      Instruction::LocalTee(bool_value),
      Instruction::I32Const(1),
      Instruction::I32GtU,
      Instruction::If(BlockType::Empty),
      Instruction::Unreachable,
      Instruction::End,
      Instruction::LocalGet(bool_value),
      Instruction::F64ConvertI32U,
    ]),
    ComponentAbiType::Number => instructions.extend([Instruction::LocalGet(src_addr), Instruction::F64Load(mem_arg_f64(0))]),
    ComponentAbiType::Buffer | ComponentAbiType::String => {
      let constructor = if matches!(item_type, ComponentAbiType::Buffer) {
        codecs.buffer_new_index
      } else {
        codecs.str_new_index
      };
      instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalGet(src_addr),
        Instruction::I32Load(mem_arg_i32(4)),
        Instruction::Call(constructor),
      ]);
    }
    ComponentAbiType::List(_) => {
      let codec = codecs
        .list_codecs
        .get(item_type)
        .expect("nested Component List lift codec must be registered");
      instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalGet(src_addr),
        Instruction::I32Load(mem_arg_i32(4)),
        Instruction::Call(codec.lift_index),
      ]);
    }
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) => {
      let codec = codecs
        .variant_codecs
        .get(item_type)
        .expect("nested Component variant lift codec must be registered");
      instructions.extend([Instruction::LocalGet(src_addr), Instruction::Call(codec.lift_index)]);
    }
  }
}

pub(super) fn build_component_list_lift_fn(
  list_type: &ComponentAbiType,
  list_tag_id: i32,
  cabi_realloc_index: u32,
  codecs: &ComponentValueCodecs<'_>,
) -> CompiledFn {
  let ComponentAbiType::List(item_type) = list_type else {
    unreachable!("Component List lift helper requires a List type")
  };
  let layout = component_memory_layout(item_type);
  // params: 0 = canonical ptr, 1 = element count
  // locals: 2 = canonical bytes (i64), 3 = allocation bytes (i64),
  // 4 = raw base, 5 = logical list ptr, 6 = index, 7 = source address,
  // 8 = destination address, 9 = canonical Bool value.
  let canonical_bytes = 2;
  let allocation_bytes = 3;
  let raw_base = 4;
  let list_ptr = 5;
  let index = 6;
  let src_addr = 7;
  let dst_addr = 8;
  let bool_value = 9;
  let mut instructions = Vec::new();
  push_checked_u32_product(&mut instructions, 1, layout.size, canonical_bytes);
  push_checked_alignment(&mut instructions, 0, layout.alignment);
  push_checked_memory_region(&mut instructions, 0, canonical_bytes);
  instructions.extend([
    // Complete internal allocation: type header + count slot + f64 elements.
    Instruction::LocalGet(1),
    Instruction::I64ExtendI32U,
    Instruction::I64Const(8),
    Instruction::I64Mul,
    Instruction::I64Const(16),
    Instruction::I64Add,
    Instruction::LocalTee(allocation_bytes),
    Instruction::I64Const(i64::from(u32::MAX)),
    Instruction::I64GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(8),
    Instruction::LocalGet(allocation_bytes),
    Instruction::I32WrapI64,
    Instruction::Call(cabi_realloc_index),
    Instruction::LocalTee(raw_base),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(raw_base),
    Instruction::I32Const(list_tag_id),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::LocalGet(raw_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalTee(list_ptr),
    Instruction::LocalGet(1),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::I32Const(0),
    Instruction::LocalSet(index),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(index),
    Instruction::LocalGet(1),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(0),
    Instruction::LocalGet(index),
    Instruction::I32Const(layout.size),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalSet(src_addr),
    Instruction::LocalGet(list_ptr),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(index),
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalTee(dst_addr),
  ]);
  push_lift_element(&mut instructions, item_type, src_addr, bool_value, codecs);
  instructions.extend([
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(index),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(index),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(list_ptr),
    Instruction::F64ConvertI32U,
  ]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I64,
      ValType::I64,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
    ],
    instructions,
  }
}

fn push_lower_element(
  instructions: &mut Vec<Instruction<'static>>,
  item_type: &ComponentAbiType,
  src_addr: u32,
  dst_addr: u32,
  value: u32,
  canonical_bool: u32,
  codecs: &ComponentValueCodecs<'_>,
) {
  match item_type {
    ComponentAbiType::Unit => {}
    ComponentAbiType::Bool => {
      instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::F64Load(mem_arg_f64(0)),
        Instruction::LocalSet(value),
        Instruction::LocalGet(dst_addr),
      ]);
      instructions.extend(component_bool_f64_to_i32(value, canonical_bool));
      instructions.push(Instruction::I32Store8(mem_arg_byte(0)));
    }
    ComponentAbiType::Number => instructions.extend([
      Instruction::LocalGet(dst_addr),
      Instruction::LocalGet(src_addr),
      Instruction::F64Load(mem_arg_f64(0)),
      Instruction::F64Store(mem_arg_f64(0)),
    ]),
    ComponentAbiType::Buffer | ComponentAbiType::String => instructions.extend([
      Instruction::LocalGet(src_addr),
      Instruction::F64Load(mem_arg_f64(0)),
      Instruction::LocalSet(value),
      Instruction::LocalGet(dst_addr),
      Instruction::LocalGet(value),
      Instruction::I32TruncF64U,
      Instruction::I32Const(8),
      Instruction::I32Add,
      Instruction::I32Store(mem_arg_i32(0)),
      Instruction::LocalGet(dst_addr),
      Instruction::LocalGet(value),
      Instruction::I32TruncF64U,
      Instruction::F64Load(mem_arg_f64(0)),
      Instruction::I32TruncF64U,
      Instruction::I32Store(mem_arg_i32(4)),
    ]),
    ComponentAbiType::List(_) => {
      let codec = codecs
        .list_codecs
        .get(item_type)
        .expect("nested Component List lower codec must be registered");
      instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::F64Load(mem_arg_f64(0)),
        Instruction::LocalGet(dst_addr),
        Instruction::Call(codec.lower_index),
      ]);
    }
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) => {
      let codec = codecs
        .variant_codecs
        .get(item_type)
        .expect("nested Component variant lower codec must be registered");
      instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::F64Load(mem_arg_f64(0)),
        Instruction::LocalGet(dst_addr),
        Instruction::Call(codec.lower_index),
      ]);
    }
  }
}

pub(super) fn build_component_list_lower_fn(
  list_type: &ComponentAbiType,
  cabi_realloc_index: u32,
  codecs: &ComponentValueCodecs<'_>,
) -> CompiledFn {
  let ComponentAbiType::List(item_type) = list_type else {
    unreachable!("Component List lower helper requires a List type")
  };
  let layout = component_memory_layout(item_type);
  // params: 0 = internal List value (f64), 1 = canonical (ptr,len) return area.
  // locals: 2 = list ptr, 3 = count, 4 = canonical bytes (i64),
  // 5 = internal bytes (i64), 6 = canonical ptr, 7 = index,
  // 8 = source address, 9 = destination address, 10 = value (f64),
  // 11 = canonical Bool value.
  let list_ptr = 2;
  let count = 3;
  let canonical_bytes = 4;
  let internal_bytes = 5;
  let canonical_ptr = 6;
  let index = 7;
  let src_addr = 8;
  let dst_addr = 9;
  let value = 10;
  let canonical_bool = 11;
  let mut instructions = vec![
    Instruction::LocalGet(0),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(list_ptr),
    Instruction::F64ConvertI32U,
    Instruction::LocalGet(0),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ];
  push_checked_alignment(&mut instructions, list_ptr, 8);
  push_checked_memory_region_const(&mut instructions, list_ptr, 8);
  instructions.extend([
    Instruction::LocalGet(list_ptr),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalTee(value),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(count),
    Instruction::F64ConvertI32U,
    Instruction::LocalGet(value),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(count),
    Instruction::I64ExtendI32U,
    Instruction::I64Const(8),
    Instruction::I64Mul,
    Instruction::I64Const(8),
    Instruction::I64Add,
    Instruction::LocalTee(internal_bytes),
    Instruction::I64Const(i64::from(u32::MAX)),
    Instruction::I64GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
  push_checked_memory_region(&mut instructions, list_ptr, internal_bytes);
  push_checked_u32_product(&mut instructions, count, layout.size, canonical_bytes);
  push_checked_alignment(&mut instructions, 1, 4);
  push_checked_memory_region_const(&mut instructions, 1, 8);
  instructions.extend([
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(layout.alignment),
    Instruction::LocalGet(canonical_bytes),
    Instruction::I32WrapI64,
    Instruction::Call(cabi_realloc_index),
    Instruction::LocalSet(canonical_ptr),
    Instruction::LocalGet(1),
    Instruction::LocalGet(canonical_ptr),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(1),
    Instruction::LocalGet(count),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::I32Const(0),
    Instruction::LocalSet(index),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(index),
    Instruction::LocalGet(count),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(list_ptr),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(index),
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalSet(src_addr),
    Instruction::LocalGet(canonical_ptr),
    Instruction::LocalGet(index),
    Instruction::I32Const(layout.size),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalSet(dst_addr),
  ]);
  push_lower_element(&mut instructions, item_type, src_addr, dst_addr, value, canonical_bool, codecs);
  instructions.extend([
    Instruction::LocalGet(index),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(index),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
  ]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::I32],
    results: vec![],
    locals: vec![
      ValType::I32,
      ValType::I32,
      ValType::I64,
      ValType::I64,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::I32,
      ValType::F64,
      ValType::I32,
    ],
    instructions,
  }
}

fn push_variant_tag_and_payload(
  instructions: &mut Vec<Instruction<'static>>,
  enum_ptr: u32,
  src_addr: u32,
  payload_type: &ComponentAbiType,
  branch: (i32, f64),
  bool_value: u32,
  codecs: &ComponentValueCodecs<'_>,
) {
  let (tag_id, count) = branch;
  instructions.extend([
    Instruction::LocalGet(enum_ptr),
    f64_const(count),
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(enum_ptr),
    f64_const(f64::from(tag_id)),
    Instruction::F64Store(mem_arg_f64(8)),
  ]);
  if count != 0.0 {
    instructions.extend([Instruction::LocalGet(enum_ptr), Instruction::I32Const(16), Instruction::I32Add]);
    push_lift_element(instructions, payload_type, src_addr, bool_value, codecs);
    instructions.push(Instruction::F64Store(mem_arg_f64(0)));
  }
}

pub(super) fn build_component_variant_lift_fn(
  variant_type: &ComponentAbiType,
  tag_ids: [i32; 2],
  enum_tag_id: i32,
  cabi_realloc_index: u32,
  codecs: &ComponentValueCodecs<'_>,
) -> CompiledFn {
  let layout = component_memory_layout(variant_type);
  let payload_offset = component_variant_payload_offset(variant_type);
  let (left, right, left_count, right_count) = match variant_type {
    ComponentAbiType::Option(item) => (&ComponentAbiType::Unit, item.as_ref(), 0.0, 1.0),
    ComponentAbiType::Result(ok, error) => (ok.as_ref(), error.as_ref(), 1.0, 1.0),
    _ => unreachable!("Component variant lift helper requires Option or Result"),
  };
  // params: 0 = canonical variant address.
  // locals: 1 = discriminant, 2 = raw base, 3 = logical enum pointer,
  // 4 = payload source address, 5 = canonical Bool value.
  let discriminant = 1;
  let raw_base = 2;
  let enum_ptr = 3;
  let src_addr = 4;
  let bool_value = 5;
  let mut instructions = Vec::new();
  push_checked_alignment(&mut instructions, 0, layout.alignment);
  push_checked_memory_region_const(&mut instructions, 0, i64::from(layout.size));
  instructions.extend([
    Instruction::LocalGet(0),
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalTee(discriminant),
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Const(payload_offset),
    Instruction::I32Add,
    Instruction::LocalSet(src_addr),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(8),
    Instruction::I32Const(32),
    Instruction::Call(cabi_realloc_index),
    Instruction::LocalTee(raw_base),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(raw_base),
    Instruction::I32Const(enum_tag_id),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::LocalGet(raw_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(enum_ptr),
    Instruction::LocalGet(discriminant),
    Instruction::If(BlockType::Empty),
  ]);
  push_variant_tag_and_payload(
    &mut instructions,
    enum_ptr,
    src_addr,
    right,
    (tag_ids[1], right_count),
    bool_value,
    codecs,
  );
  instructions.push(Instruction::Else);
  push_variant_tag_and_payload(
    &mut instructions,
    enum_ptr,
    src_addr,
    left,
    (tag_ids[0], left_count),
    bool_value,
    codecs,
  );
  instructions.extend([Instruction::End, Instruction::LocalGet(enum_ptr), Instruction::F64ConvertI32U]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32],
    results: vec![ValType::F64],
    locals: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32],
    instructions,
  }
}

fn push_validate_internal_variant_count(instructions: &mut Vec<Instruction<'static>>, enum_ptr: u32, expected: f64) {
  instructions.extend([
    Instruction::LocalGet(enum_ptr),
    Instruction::F64Load(mem_arg_f64(0)),
    f64_const(expected),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
}

fn push_lower_variant_payload(
  instructions: &mut Vec<Instruction<'static>>,
  enum_ptr: u32,
  dst_addr: u32,
  payload_type: &ComponentAbiType,
  value: u32,
  canonical_bool: u32,
  codecs: &ComponentValueCodecs<'_>,
) {
  instructions.extend([
    Instruction::LocalGet(enum_ptr),
    Instruction::I32Const(16),
    Instruction::I32Add,
    Instruction::LocalSet(value),
  ]);
  push_lower_element(instructions, payload_type, value, dst_addr, value + 1, canonical_bool, codecs);
}

pub(super) fn build_component_variant_lower_fn(
  variant_type: &ComponentAbiType,
  tag_ids: [i32; 2],
  codecs: &ComponentValueCodecs<'_>,
) -> CompiledFn {
  let layout = component_memory_layout(variant_type);
  let payload_offset = component_variant_payload_offset(variant_type);
  let (left, right, left_count, right_count) = match variant_type {
    ComponentAbiType::Option(item) => (&ComponentAbiType::Unit, item.as_ref(), 0.0, 1.0),
    ComponentAbiType::Result(ok, error) => (ok.as_ref(), error.as_ref(), 1.0, 1.0),
    _ => unreachable!("Component variant lower helper requires Option or Result"),
  };
  // params: 0 = internal enum value, 1 = canonical variant address.
  // locals: 2 = logical enum pointer, 3 = payload destination, 4 = internal payload address,
  // 5 = internal payload value, 6 = canonical Bool value.
  let enum_ptr = 2;
  let dst_addr = 3;
  let src_addr = 4;
  let canonical_bool = 6;
  let mut instructions = vec![
    Instruction::LocalGet(0),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(enum_ptr),
    Instruction::F64ConvertI32U,
    Instruction::LocalGet(0),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ];
  push_checked_alignment(&mut instructions, enum_ptr, 8);
  push_checked_memory_region_const(&mut instructions, enum_ptr, 24);
  push_checked_alignment(&mut instructions, 1, layout.alignment);
  push_checked_memory_region_const(&mut instructions, 1, i64::from(layout.size));
  instructions.extend([
    Instruction::LocalGet(1),
    Instruction::I32Const(payload_offset),
    Instruction::I32Add,
    Instruction::LocalSet(dst_addr),
    Instruction::LocalGet(enum_ptr),
    Instruction::F64Load(mem_arg_f64(8)),
    f64_const(f64::from(tag_ids[0])),
    Instruction::F64Eq,
    Instruction::If(BlockType::Empty),
  ]);
  push_validate_internal_variant_count(&mut instructions, enum_ptr, left_count);
  instructions.extend([
    Instruction::LocalGet(1),
    Instruction::I32Const(0),
    Instruction::I32Store8(mem_arg_byte(0)),
  ]);
  if left_count != 0.0 {
    push_lower_variant_payload(&mut instructions, enum_ptr, dst_addr, left, src_addr, canonical_bool, codecs);
  }
  instructions.extend([
    Instruction::Else,
    Instruction::LocalGet(enum_ptr),
    Instruction::F64Load(mem_arg_f64(8)),
    f64_const(f64::from(tag_ids[1])),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
  push_validate_internal_variant_count(&mut instructions, enum_ptr, right_count);
  instructions.extend([
    Instruction::LocalGet(1),
    Instruction::I32Const(1),
    Instruction::I32Store8(mem_arg_byte(0)),
  ]);
  if right_count != 0.0 {
    push_lower_variant_payload(&mut instructions, enum_ptr, dst_addr, right, src_addr, canonical_bool, codecs);
  }
  instructions.extend([Instruction::End]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::I32],
    results: vec![],
    locals: vec![ValType::I32, ValType::I32, ValType::I32, ValType::F64, ValType::I32],
    instructions,
  }
}
