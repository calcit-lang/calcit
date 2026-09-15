use std::collections::{BTreeMap, BTreeSet};

use super::*;
use wasm_encoder::BlockType;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ComponentListCodec {
  pub lift_index: u32,
  pub lower_index: u32,
}

#[derive(Clone, Copy)]
struct ComponentMemoryLayout {
  size: i32,
  alignment: i32,
}

fn component_memory_layout(value_type: &ComponentAbiType) -> ComponentMemoryLayout {
  match value_type {
    ComponentAbiType::Bool => ComponentMemoryLayout { size: 1, alignment: 1 },
    ComponentAbiType::Number => ComponentMemoryLayout { size: 8, alignment: 8 },
    ComponentAbiType::Buffer | ComponentAbiType::String | ComponentAbiType::List(_) => ComponentMemoryLayout { size: 8, alignment: 4 },
  }
}

fn collect_list_type(value_type: &ComponentAbiType, seen: &mut BTreeSet<ComponentAbiType>, ordered: &mut Vec<ComponentAbiType>) {
  let ComponentAbiType::List(item) = value_type else {
    return;
  };
  collect_list_type(item, seen, ordered);
  if seen.insert(value_type.clone()) {
    ordered.push(value_type.clone());
  }
}

pub(super) fn collect_component_list_types(
  program_data: &program::CompiledProgram,
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  import_adapters: &[ComponentImportAdapter],
) -> Result<Vec<ComponentAbiType>, String> {
  let mut seen = BTreeSet::new();
  let mut ordered = Vec::new();
  for adapter in import_adapters {
    for parameter in &adapter.parameters {
      collect_list_type(parameter, &mut seen, &mut ordered);
    }
    collect_list_type(&adapter.result, &mut seen, &mut ordered);
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
      collect_list_type(parameter, &mut seen, &mut ordered);
    }
    collect_list_type(&result, &mut seen, &mut ordered);
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
  str_new_index: u32,
  buffer_new_index: u32,
  list_codecs: &BTreeMap<ComponentAbiType, ComponentListCodec>,
) {
  match item_type {
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
        buffer_new_index
      } else {
        str_new_index
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
      let codec = list_codecs
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
  }
}

pub(super) fn build_component_list_lift_fn(
  list_type: &ComponentAbiType,
  list_tag_id: i32,
  str_new_index: u32,
  buffer_new_index: u32,
  cabi_realloc_index: u32,
  list_codecs: &BTreeMap<ComponentAbiType, ComponentListCodec>,
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
  push_lift_element(
    &mut instructions,
    item_type,
    src_addr,
    bool_value,
    str_new_index,
    buffer_new_index,
    list_codecs,
  );
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
  list_codecs: &BTreeMap<ComponentAbiType, ComponentListCodec>,
) {
  match item_type {
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
      let codec = list_codecs
        .get(item_type)
        .expect("nested Component List lower codec must be registered");
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
  list_codecs: &BTreeMap<ComponentAbiType, ComponentListCodec>,
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
  push_lower_element(&mut instructions, item_type, src_addr, dst_addr, value, canonical_bool, list_codecs);
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
