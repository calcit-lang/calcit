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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ComponentStructCodec {
  pub lift_index: u32,
  pub lower_index: u32,
}

pub(super) struct ComponentValueCodecs<'a> {
  pub str_new_index: u32,
  pub buffer_new_index: u32,
  pub list_codecs: &'a BTreeMap<ComponentAbiType, ComponentListCodec>,
  pub struct_codecs: &'a BTreeMap<ComponentAbiType, ComponentStructCodec>,
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
    ComponentAbiType::Numeric(kind) => match kind {
      CalcitNumericRefinement::Int8 | CalcitNumericRefinement::UInt8 => ComponentMemoryLayout { size: 1, alignment: 1 },
      CalcitNumericRefinement::Int16 | CalcitNumericRefinement::UInt16 => ComponentMemoryLayout { size: 2, alignment: 2 },
      CalcitNumericRefinement::Int32 | CalcitNumericRefinement::UInt32 | CalcitNumericRefinement::Float32 => {
        ComponentMemoryLayout { size: 4, alignment: 4 }
      }
      CalcitNumericRefinement::Int64 | CalcitNumericRefinement::UInt64 | CalcitNumericRefinement::Float64 => {
        ComponentMemoryLayout { size: 8, alignment: 8 }
      }
    },
    ComponentAbiType::Buffer | ComponentAbiType::String | ComponentAbiType::List(_) => ComponentMemoryLayout { size: 8, alignment: 4 },
    ComponentAbiType::Option(item) => component_variant_layout(&ComponentAbiType::Unit, item),
    ComponentAbiType::Result(ok, error) => component_variant_layout(ok, error),
    ComponentAbiType::Struct(record) => component_struct_layout(record).0,
    ComponentAbiType::Enum(enum_type) => component_enum_layout(enum_type).0,
  }
}

fn component_struct_layout(record: &ComponentStructType) -> (ComponentMemoryLayout, Vec<i32>) {
  component_fields_layout(record.fields.iter().map(|(_, field_type)| field_type))
}

fn component_fields_layout<'a>(fields: impl Iterator<Item = &'a ComponentAbiType>) -> (ComponentMemoryLayout, Vec<i32>) {
  let mut size = 0;
  let mut alignment = 1;
  let mut offsets = Vec::new();
  for field_type in fields {
    let field = component_memory_layout(field_type);
    size = align_to(size, field.alignment);
    offsets.push(size);
    size += field.size;
    alignment = alignment.max(field.alignment);
  }
  (
    ComponentMemoryLayout {
      size: align_to(size, alignment),
      alignment,
    },
    offsets,
  )
}

fn component_enum_discriminant_size(variant_count: usize) -> i32 {
  if variant_count <= 256 {
    1
  } else if variant_count <= 65_536 {
    2
  } else {
    4
  }
}

fn component_enum_layout(enum_type: &ComponentEnumType) -> (ComponentMemoryLayout, i32, Vec<(ComponentMemoryLayout, Vec<i32>)>) {
  let payloads = enum_type
    .variants
    .iter()
    .map(|variant| component_fields_layout(variant.payload.iter()))
    .collect::<Vec<_>>();
  let discriminant_size = component_enum_discriminant_size(enum_type.variants.len());
  let payload_alignment = payloads.iter().map(|(layout, _)| layout.alignment).max().unwrap_or(1);
  let alignment = discriminant_size.max(payload_alignment);
  let payload_offset = align_to(discriminant_size, payload_alignment);
  let payload_size = payloads.iter().map(|(layout, _)| layout.size).max().unwrap_or(0);
  (
    ComponentMemoryLayout {
      size: align_to(payload_offset + payload_size, alignment),
      alignment,
    },
    payload_offset,
    payloads,
  )
}

fn push_zero(instructions: &mut Vec<Instruction<'static>>, value_type: ValType) {
  instructions.push(match value_type {
    ValType::I32 => Instruction::I32Const(0),
    ValType::I64 => Instruction::I64Const(0),
    ValType::F32 => Instruction::F32Const(wasm_encoder::Ieee32::from(0.0)),
    ValType::F64 => f64_const(0.0),
    other => panic!("unsupported Component flat zero: {other:?}"),
  });
}

fn push_flat_conversion(instructions: &mut Vec<Instruction<'static>>, from: ValType, to: ValType) {
  match (from, to) {
    (from, to) if from == to => {}
    (ValType::I32, ValType::I64) => instructions.push(Instruction::I64ExtendI32U),
    (ValType::I32, ValType::F32) => instructions.push(Instruction::F32ReinterpretI32),
    (ValType::F32, ValType::I32) => instructions.push(Instruction::I32ReinterpretF32),
    (ValType::F32, ValType::I64) => instructions.extend([Instruction::I32ReinterpretF32, Instruction::I64ExtendI32U]),
    (ValType::F64, ValType::I64) => instructions.push(Instruction::I64ReinterpretF64),
    (ValType::I64, ValType::I32) => instructions.push(Instruction::I32WrapI64),
    (ValType::I64, ValType::F32) => instructions.extend([Instruction::I32WrapI64, Instruction::F32ReinterpretI32]),
    (ValType::I64, ValType::F64) => instructions.push(Instruction::F64ReinterpretI64),
    (from, to) => panic!("unsupported Component flat conversion: {from:?} to {to:?}"),
  }
}

fn component_payload_flat_types(value_type: &ComponentAbiType) -> Vec<ValType> {
  component_flat_types(value_type)
}

fn push_validate_numeric_flat(
  instructions: &mut Vec<Instruction<'static>>,
  kind: CalcitNumericRefinement,
  source_local: u32,
  source_type: ValType,
) {
  let actual_type = component_numeric_flat_type(kind);
  let push_value = |instructions: &mut Vec<Instruction<'static>>| {
    instructions.push(Instruction::LocalGet(source_local));
    push_flat_conversion(instructions, source_type, actual_type);
  };
  match kind {
    CalcitNumericRefinement::Int8 | CalcitNumericRefinement::Int16 => {
      let (min, max) = if kind == CalcitNumericRefinement::Int8 {
        (-128, 127)
      } else {
        (-32_768, 32_767)
      };
      push_value(instructions);
      instructions.extend([Instruction::I32Const(min), Instruction::I32LtS]);
      component_trap_if(instructions);
      push_value(instructions);
      instructions.extend([Instruction::I32Const(max), Instruction::I32GtS]);
      component_trap_if(instructions);
    }
    CalcitNumericRefinement::UInt8 | CalcitNumericRefinement::UInt16 => {
      let max = if kind == CalcitNumericRefinement::UInt8 { 255 } else { 65_535 };
      push_value(instructions);
      instructions.extend([Instruction::I32Const(max), Instruction::I32GtU]);
      component_trap_if(instructions);
    }
    CalcitNumericRefinement::Int64 => {
      push_value(instructions);
      instructions.extend([Instruction::I64Const(-9_007_199_254_740_991), Instruction::I64LtS]);
      component_trap_if(instructions);
      push_value(instructions);
      instructions.extend([Instruction::I64Const(9_007_199_254_740_991), Instruction::I64GtS]);
      component_trap_if(instructions);
    }
    CalcitNumericRefinement::UInt64 => {
      push_value(instructions);
      instructions.extend([Instruction::I64Const(9_007_199_254_740_991), Instruction::I64GtU]);
      component_trap_if(instructions);
    }
    CalcitNumericRefinement::Int32
    | CalcitNumericRefinement::UInt32
    | CalcitNumericRefinement::Float32
    | CalcitNumericRefinement::Float64 => {}
  }
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
    ComponentAbiType::Numeric(kind) => {
      push_validate_numeric_flat(instructions, *kind, flat_start, source_types[0]);
      instructions.extend([Instruction::LocalGet(dst_local), Instruction::LocalGet(flat_start)]);
      push_flat_conversion(instructions, source_types[0], actual_types[0]);
      instructions.push(match kind {
        CalcitNumericRefinement::Int8 | CalcitNumericRefinement::UInt8 => Instruction::I32Store8(mem_arg_byte(dst_offset as u64)),
        CalcitNumericRefinement::Int16 | CalcitNumericRefinement::UInt16 => Instruction::I32Store16(mem_arg_i16(dst_offset as u64)),
        CalcitNumericRefinement::Int32 | CalcitNumericRefinement::UInt32 => Instruction::I32Store(mem_arg_i32(dst_offset as u64)),
        CalcitNumericRefinement::Int64 | CalcitNumericRefinement::UInt64 => Instruction::I64Store(mem_arg_i64(dst_offset as u64)),
        CalcitNumericRefinement::Float32 => Instruction::F32Store(mem_arg_f32(dst_offset as u64)),
        CalcitNumericRefinement::Float64 => Instruction::F64Store(mem_arg_f64(dst_offset as u64)),
      });
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
    ComponentAbiType::Enum(enum_type) => {
      push_store_enum_flat_from(instructions, enum_type, source_types, flat_start, dst_local, dst_offset);
    }
    ComponentAbiType::Struct(record) => {
      let (_, offsets) = component_struct_layout(record);
      let mut field_start = flat_start;
      let mut source_offset = 0;
      for ((_, field_type), field_offset) in record.fields.iter().zip(offsets) {
        let field_width = component_flat_types(field_type).len();
        push_store_flat_payload(
          instructions,
          field_type,
          &source_types[source_offset..source_offset + field_width],
          field_start,
          dst_local,
          dst_offset + field_offset,
        );
        field_start += field_width as u32;
        source_offset += field_width;
      }
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

fn push_store_enum_discriminant(
  instructions: &mut Vec<Instruction<'static>>,
  discriminant_size: i32,
  dst_local: u32,
  base_offset: i32,
  discriminant_local: u32,
  discriminant_type: ValType,
) {
  instructions.extend([Instruction::LocalGet(dst_local), Instruction::LocalGet(discriminant_local)]);
  push_flat_conversion(instructions, discriminant_type, ValType::I32);
  instructions.push(match discriminant_size {
    1 => Instruction::I32Store8(mem_arg_byte(base_offset as u64)),
    2 => Instruction::I32Store16(mem_arg_byte(base_offset as u64)),
    4 => Instruction::I32Store(mem_arg_i32(base_offset as u64)),
    _ => unreachable!("Component Enum discriminant size must be 1, 2, or 4"),
  });
}

fn push_load_enum_discriminant(instructions: &mut Vec<Instruction<'static>>, discriminant_size: i32, src_local: u32, base_offset: i32) {
  instructions.push(Instruction::LocalGet(src_local));
  instructions.push(match discriminant_size {
    1 => Instruction::I32Load8U(mem_arg_byte(base_offset as u64)),
    2 => Instruction::I32Load16U(mem_arg_byte(base_offset as u64)),
    4 => Instruction::I32Load(mem_arg_i32(base_offset as u64)),
    _ => unreachable!("Component Enum discriminant size must be 1, 2, or 4"),
  });
}

fn push_store_enum_flat_from(
  instructions: &mut Vec<Instruction<'static>>,
  enum_type: &ComponentEnumType,
  source_types: &[ValType],
  flat_start: u32,
  dst_local: u32,
  base_offset: i32,
) {
  let (_, payload_offset, payload_layouts) = component_enum_layout(enum_type);
  let discriminant_size = component_enum_discriminant_size(enum_type.variants.len());
  instructions.push(Instruction::LocalGet(flat_start));
  push_flat_conversion(instructions, source_types[0], ValType::I32);
  instructions.extend([
    Instruction::I32Const(enum_type.variants.len() as i32),
    Instruction::I32GeU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
  push_store_enum_discriminant(instructions, discriminant_size, dst_local, base_offset, flat_start, source_types[0]);
  for (variant_index, (variant, (_, offsets))) in enum_type.variants.iter().zip(payload_layouts).enumerate() {
    instructions.push(Instruction::LocalGet(flat_start));
    push_flat_conversion(instructions, source_types[0], ValType::I32);
    instructions.extend([
      Instruction::I32Const(variant_index as i32),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
    ]);
    let mut slot = 0;
    for (payload_type, field_offset) in variant.payload.iter().zip(offsets) {
      let width = component_flat_types(payload_type).len();
      push_store_flat_payload(
        instructions,
        payload_type,
        &source_types[1 + slot..1 + slot + width],
        flat_start + 1 + slot as u32,
        dst_local,
        base_offset + payload_offset + field_offset,
      );
      slot += width;
    }
    instructions.push(Instruction::End);
  }
}

pub(super) fn push_store_value_flat(
  instructions: &mut Vec<Instruction<'static>>,
  value_type: &ComponentAbiType,
  flat_start: u32,
  dst_local: u32,
  base_offset: i32,
) {
  push_store_flat_payload(
    instructions,
    value_type,
    &component_flat_types(value_type),
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
    ComponentAbiType::Numeric(kind) => {
      instructions.push(Instruction::LocalGet(src_local));
      instructions.push(match kind {
        CalcitNumericRefinement::Int8 => Instruction::I32Load8S(mem_arg_byte(src_offset as u64)),
        CalcitNumericRefinement::UInt8 => Instruction::I32Load8U(mem_arg_byte(src_offset as u64)),
        CalcitNumericRefinement::Int16 => Instruction::I32Load16S(mem_arg_i16(src_offset as u64)),
        CalcitNumericRefinement::UInt16 => Instruction::I32Load16U(mem_arg_i16(src_offset as u64)),
        CalcitNumericRefinement::Int32 | CalcitNumericRefinement::UInt32 => Instruction::I32Load(mem_arg_i32(src_offset as u64)),
        CalcitNumericRefinement::Int64 | CalcitNumericRefinement::UInt64 => Instruction::I64Load(mem_arg_i64(src_offset as u64)),
        CalcitNumericRefinement::Float32 => Instruction::F32Load(mem_arg_f32(src_offset as u64)),
        CalcitNumericRefinement::Float64 => Instruction::F64Load(mem_arg_f64(src_offset as u64)),
      });
    }
    ComponentAbiType::Buffer | ComponentAbiType::List(_) | ComponentAbiType::String => instructions.extend([
      Instruction::LocalGet(src_local),
      Instruction::I32Load(mem_arg_i32((src_offset + slot as i32 * 4) as u64)),
    ]),
    ComponentAbiType::Unit => unreachable!(),
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) => {
      push_load_variant_flat_slot(instructions, value_type, slot, actual_type, src_local, src_offset)
    }
    ComponentAbiType::Enum(enum_type) => push_load_enum_flat_slot(instructions, enum_type, slot, actual_type, src_local, src_offset),
    ComponentAbiType::Struct(record) => {
      let (_, offsets) = component_struct_layout(record);
      let mut remaining = slot;
      for ((_, field_type), field_offset) in record.fields.iter().zip(offsets) {
        let width = component_flat_types(field_type).len();
        if remaining < width {
          push_load_value_flat_slot(
            instructions,
            field_type,
            remaining,
            actual_type,
            src_local,
            src_offset + field_offset,
          );
          break;
        }
        remaining -= width;
      }
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

fn push_load_enum_case_payload(
  instructions: &mut Vec<Instruction<'static>>,
  variant: &ComponentEnumVariant,
  offsets: &[i32],
  slot: usize,
  desired_type: ValType,
  src_local: u32,
  payload_offset: i32,
) {
  let mut remaining = slot;
  for (payload_type, field_offset) in variant.payload.iter().zip(offsets) {
    let width = component_flat_types(payload_type).len();
    if remaining < width {
      push_load_value_flat_slot(
        instructions,
        payload_type,
        remaining,
        desired_type,
        src_local,
        payload_offset + field_offset,
      );
      return;
    }
    remaining -= width;
  }
  push_zero(instructions, desired_type);
}

struct EnumFlatLoadContext<'a> {
  enum_type: &'a ComponentEnumType,
  payload_layouts: &'a [(ComponentMemoryLayout, Vec<i32>)],
  src_local: u32,
  base_offset: i32,
  payload_offset: i32,
}

fn push_load_enum_case(
  instructions: &mut Vec<Instruction<'static>>,
  context: &EnumFlatLoadContext<'_>,
  case_index: usize,
  slot: usize,
  desired_type: ValType,
) {
  let discriminant_size = component_enum_discriminant_size(context.enum_type.variants.len());
  push_load_enum_discriminant(instructions, discriminant_size, context.src_local, context.base_offset);
  instructions.extend([Instruction::I32Const(case_index as i32), Instruction::I32Eq]);
  if case_index + 1 == context.enum_type.variants.len() {
    instructions.extend([
      Instruction::If(BlockType::Empty),
      Instruction::Else,
      Instruction::Unreachable,
      Instruction::End,
    ]);
    push_load_enum_case_payload(
      instructions,
      &context.enum_type.variants[case_index],
      &context.payload_layouts[case_index].1,
      slot,
      desired_type,
      context.src_local,
      context.base_offset + context.payload_offset,
    );
    return;
  }
  instructions.push(Instruction::If(BlockType::Result(desired_type)));
  push_load_enum_case_payload(
    instructions,
    &context.enum_type.variants[case_index],
    &context.payload_layouts[case_index].1,
    slot,
    desired_type,
    context.src_local,
    context.base_offset + context.payload_offset,
  );
  instructions.push(Instruction::Else);
  push_load_enum_case(instructions, context, case_index + 1, slot, desired_type);
  instructions.push(Instruction::End);
}

fn push_load_enum_flat_slot(
  instructions: &mut Vec<Instruction<'static>>,
  enum_type: &ComponentEnumType,
  slot: usize,
  desired_type: ValType,
  src_local: u32,
  base_offset: i32,
) {
  let (_, payload_offset, payload_layouts) = component_enum_layout(enum_type);
  let discriminant_size = component_enum_discriminant_size(enum_type.variants.len());
  if slot == 0 {
    push_load_enum_discriminant(instructions, discriminant_size, src_local, base_offset);
    push_flat_conversion(instructions, ValType::I32, desired_type);
    return;
  }
  let context = EnumFlatLoadContext {
    enum_type,
    src_local,
    base_offset,
    payload_offset,
    payload_layouts: &payload_layouts,
  };
  push_load_enum_case(instructions, &context, 0, slot - 1, desired_type);
}

pub(super) fn push_load_value_flat(instructions: &mut Vec<Instruction<'static>>, value_type: &ComponentAbiType, src_local: u32) {
  for (slot, flat_type) in component_flat_types(value_type).into_iter().enumerate() {
    push_load_value_flat_slot(instructions, value_type, slot, flat_type, src_local, 0);
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
    ComponentAbiType::Struct(record) => {
      for (_, field_type) in &record.fields {
        collect_compound_type(field_type, seen, ordered);
      }
    }
    ComponentAbiType::Enum(enum_type) => {
      for variant in &enum_type.variants {
        for payload_type in &variant.payload {
          collect_compound_type(payload_type, seen, ordered);
        }
      }
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
    let (parameters, result, _) = component_function_schema(compiled, &definition, fn_param_names(args).len(), program_data, true)?;
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
    ComponentAbiType::Numeric(kind) => match kind {
      CalcitNumericRefinement::Int8 => instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::I32Load8S(mem_arg_byte(0)),
        Instruction::F64ConvertI32S,
      ]),
      CalcitNumericRefinement::UInt8 => instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::I32Load8U(mem_arg_byte(0)),
        Instruction::F64ConvertI32U,
      ]),
      CalcitNumericRefinement::Int16 => instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::I32Load16S(mem_arg_i16(0)),
        Instruction::F64ConvertI32S,
      ]),
      CalcitNumericRefinement::UInt16 => instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::I32Load16U(mem_arg_i16(0)),
        Instruction::F64ConvertI32U,
      ]),
      CalcitNumericRefinement::Int32 => instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::F64ConvertI32S,
      ]),
      CalcitNumericRefinement::UInt32 => instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::F64ConvertI32U,
      ]),
      CalcitNumericRefinement::Int64 => {
        instructions.extend([
          Instruction::LocalGet(src_addr),
          Instruction::I64Load(mem_arg_i64(0)),
          Instruction::I64Const(-9_007_199_254_740_991),
          Instruction::I64LtS,
        ]);
        component_trap_if(instructions);
        instructions.extend([
          Instruction::LocalGet(src_addr),
          Instruction::I64Load(mem_arg_i64(0)),
          Instruction::I64Const(9_007_199_254_740_991),
          Instruction::I64GtS,
        ]);
        component_trap_if(instructions);
        instructions.extend([
          Instruction::LocalGet(src_addr),
          Instruction::I64Load(mem_arg_i64(0)),
          Instruction::F64ConvertI64S,
        ]);
      }
      CalcitNumericRefinement::UInt64 => {
        instructions.extend([
          Instruction::LocalGet(src_addr),
          Instruction::I64Load(mem_arg_i64(0)),
          Instruction::I64Const(9_007_199_254_740_991),
          Instruction::I64GtU,
        ]);
        component_trap_if(instructions);
        instructions.extend([
          Instruction::LocalGet(src_addr),
          Instruction::I64Load(mem_arg_i64(0)),
          Instruction::F64ConvertI64U,
        ]);
      }
      CalcitNumericRefinement::Float32 => {
        instructions.extend([
          Instruction::LocalGet(src_addr),
          Instruction::F32Load(mem_arg_f32(0)),
          Instruction::LocalGet(src_addr),
          Instruction::F32Load(mem_arg_f32(0)),
          Instruction::F32Ne,
        ]);
        component_trap_if(instructions);
        instructions.extend([
          Instruction::LocalGet(src_addr),
          Instruction::F32Load(mem_arg_f32(0)),
          Instruction::F64PromoteF32,
        ]);
      }
      CalcitNumericRefinement::Float64 => instructions.extend([Instruction::LocalGet(src_addr), Instruction::F64Load(mem_arg_f64(0))]),
    },
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
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) | ComponentAbiType::Enum(_) => {
      let codec = codecs
        .variant_codecs
        .get(item_type)
        .expect("nested Component variant lift codec must be registered");
      instructions.extend([Instruction::LocalGet(src_addr), Instruction::Call(codec.lift_index)]);
    }
    ComponentAbiType::Struct(_) => {
      let codec = codecs
        .struct_codecs
        .get(item_type)
        .expect("nested Component Struct lift codec must be registered");
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
    ComponentAbiType::Numeric(kind) => {
      instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::F64Load(mem_arg_f64(0)),
        Instruction::LocalSet(value),
        Instruction::LocalGet(dst_addr),
      ]);
      instructions.extend(component_numeric_from_f64(*kind, value));
      instructions.push(match kind {
        CalcitNumericRefinement::Int8 | CalcitNumericRefinement::UInt8 => Instruction::I32Store8(mem_arg_byte(0)),
        CalcitNumericRefinement::Int16 | CalcitNumericRefinement::UInt16 => Instruction::I32Store16(mem_arg_i16(0)),
        CalcitNumericRefinement::Int32 | CalcitNumericRefinement::UInt32 => Instruction::I32Store(mem_arg_i32(0)),
        CalcitNumericRefinement::Int64 | CalcitNumericRefinement::UInt64 => Instruction::I64Store(mem_arg_i64(0)),
        CalcitNumericRefinement::Float32 => Instruction::F32Store(mem_arg_f32(0)),
        CalcitNumericRefinement::Float64 => Instruction::F64Store(mem_arg_f64(0)),
      });
    }
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
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) | ComponentAbiType::Enum(_) => {
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
    ComponentAbiType::Struct(_) => {
      let codec = codecs
        .struct_codecs
        .get(item_type)
        .expect("nested Component Struct lower codec must be registered");
      instructions.extend([
        Instruction::LocalGet(src_addr),
        Instruction::F64Load(mem_arg_f64(0)),
        Instruction::LocalGet(dst_addr),
        Instruction::Call(codec.lower_index),
      ]);
    }
  }
}

pub(super) fn build_component_struct_lift_fn(
  struct_type: &ComponentAbiType,
  nominal_tag_id: i32,
  struct_tag_id: i32,
  cabi_realloc_index: u32,
  codecs: &ComponentValueCodecs<'_>,
) -> CompiledFn {
  let ComponentAbiType::Struct(record) = struct_type else {
    unreachable!("Component Struct lift helper requires a Struct type")
  };
  let (layout, offsets) = component_struct_layout(record);
  let logical_size = ((record.fields.len() + 2) * 8) as i32;
  let raw_base = 1;
  let struct_ptr = 2;
  let source_addr = 3;
  let bool_value = 4;
  let mut instructions = Vec::new();
  push_checked_alignment(&mut instructions, 0, layout.alignment);
  push_checked_memory_region_const(&mut instructions, 0, i64::from(layout.size));
  instructions.extend([
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(8),
    Instruction::I32Const(logical_size + 8),
    Instruction::Call(cabi_realloc_index),
    Instruction::LocalTee(raw_base),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(raw_base),
    Instruction::I32Const(struct_tag_id),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::LocalGet(raw_base),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalTee(struct_ptr),
    f64_const(record.fields.len() as f64),
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(struct_ptr),
    f64_const(f64::from(nominal_tag_id)),
    Instruction::F64Store(mem_arg_f64(8)),
  ]);
  for (index, ((_, field_type), field_offset)) in record.fields.iter().zip(offsets).enumerate() {
    instructions.extend([
      Instruction::LocalGet(0),
      Instruction::I32Const(field_offset),
      Instruction::I32Add,
      Instruction::LocalSet(source_addr),
      Instruction::LocalGet(struct_ptr),
    ]);
    push_lift_element(&mut instructions, field_type, source_addr, bool_value, codecs);
    instructions.push(Instruction::F64Store(mem_arg_f64(((index + 2) * 8) as u64)));
  }
  instructions.extend([Instruction::LocalGet(struct_ptr), Instruction::F64ConvertI32U]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32],
    results: vec![ValType::F64],
    locals: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
    instructions,
  }
}

pub(super) fn build_component_struct_lower_fn(
  struct_type: &ComponentAbiType,
  nominal_tag_id: i32,
  codecs: &ComponentValueCodecs<'_>,
) -> CompiledFn {
  let ComponentAbiType::Struct(record) = struct_type else {
    unreachable!("Component Struct lower helper requires a Struct type")
  };
  let (layout, offsets) = component_struct_layout(record);
  let logical_size = ((record.fields.len() + 2) * 8) as i64;
  let struct_ptr = 2;
  let source_addr = 3;
  let destination_addr = 4;
  let value = 5;
  let canonical_bool = 6;
  let mut instructions = vec![
    Instruction::LocalGet(0),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(struct_ptr),
    Instruction::F64ConvertI32U,
    Instruction::LocalGet(0),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ];
  push_checked_alignment(&mut instructions, struct_ptr, 8);
  push_checked_memory_region_const(&mut instructions, struct_ptr, logical_size);
  push_checked_alignment(&mut instructions, 1, layout.alignment);
  push_checked_memory_region_const(&mut instructions, 1, i64::from(layout.size));
  instructions.extend([
    Instruction::LocalGet(struct_ptr),
    Instruction::F64Load(mem_arg_f64(0)),
    f64_const(record.fields.len() as f64),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(struct_ptr),
    Instruction::F64Load(mem_arg_f64(8)),
    f64_const(f64::from(nominal_tag_id)),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
  for (index, ((_, field_type), field_offset)) in record.fields.iter().zip(offsets).enumerate() {
    instructions.extend([
      Instruction::LocalGet(struct_ptr),
      Instruction::I32Const(((index + 2) * 8) as i32),
      Instruction::I32Add,
      Instruction::LocalSet(source_addr),
      Instruction::LocalGet(1),
      Instruction::I32Const(field_offset),
      Instruction::I32Add,
      Instruction::LocalSet(destination_addr),
    ]);
    push_lower_element(
      &mut instructions,
      field_type,
      source_addr,
      destination_addr,
      value,
      canonical_bool,
      codecs,
    );
  }
  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::I32],
    results: vec![],
    locals: vec![ValType::I32, ValType::I32, ValType::I32, ValType::F64, ValType::I32],
    instructions,
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

pub(super) fn build_component_enum_lift_fn(
  enum_type: &ComponentAbiType,
  tag_ids: &[i32],
  enum_tag_id: i32,
  cabi_realloc_index: u32,
  codecs: &ComponentValueCodecs<'_>,
) -> CompiledFn {
  let ComponentAbiType::Enum(enum_type) = enum_type else {
    unreachable!("Component Enum lift helper requires an Enum type")
  };
  let (layout, payload_offset, payload_layouts) = component_enum_layout(enum_type);
  let discriminant_size = component_enum_discriminant_size(enum_type.variants.len());
  let max_payload = enum_type.variants.iter().map(|variant| variant.payload.len()).max().unwrap_or(0);
  let logical_allocation_size = ((max_payload + 3) * 8) as i32;
  let discriminant = 1;
  let raw_base = 2;
  let enum_ptr = 3;
  let src_addr = 4;
  let bool_value = 5;
  let mut instructions = Vec::new();
  push_checked_alignment(&mut instructions, 0, layout.alignment);
  push_checked_memory_region_const(&mut instructions, 0, i64::from(layout.size));
  push_load_enum_discriminant(&mut instructions, discriminant_size, 0, 0);
  instructions.extend([
    Instruction::LocalTee(discriminant),
    Instruction::I32Const(enum_type.variants.len() as i32),
    Instruction::I32GeU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(8),
    Instruction::I32Const(logical_allocation_size),
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
  ]);
  for (variant_index, (variant, (_, offsets))) in enum_type.variants.iter().zip(payload_layouts).enumerate() {
    instructions.extend([
      Instruction::LocalGet(discriminant),
      Instruction::I32Const(variant_index as i32),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(enum_ptr),
      f64_const(variant.payload.len() as f64),
      Instruction::F64Store(mem_arg_f64(0)),
      Instruction::LocalGet(enum_ptr),
      f64_const(f64::from(tag_ids[variant_index])),
      Instruction::F64Store(mem_arg_f64(8)),
    ]);
    for (payload_index, (payload_type, field_offset)) in variant.payload.iter().zip(offsets).enumerate() {
      instructions.extend([
        Instruction::LocalGet(0),
        Instruction::I32Const(payload_offset + field_offset),
        Instruction::I32Add,
        Instruction::LocalSet(src_addr),
        Instruction::LocalGet(enum_ptr),
        Instruction::I32Const(((payload_index + 2) * 8) as i32),
        Instruction::I32Add,
      ]);
      push_lift_element(&mut instructions, payload_type, src_addr, bool_value, codecs);
      instructions.push(Instruction::F64Store(mem_arg_f64(0)));
    }
    instructions.push(Instruction::End);
  }
  instructions.extend([Instruction::LocalGet(enum_ptr), Instruction::F64ConvertI32U]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32],
    results: vec![ValType::F64],
    locals: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32],
    instructions,
  }
}

pub(super) fn build_component_enum_lower_fn(
  enum_type: &ComponentAbiType,
  tag_ids: &[i32],
  codecs: &ComponentValueCodecs<'_>,
) -> CompiledFn {
  let ComponentAbiType::Enum(enum_type) = enum_type else {
    unreachable!("Component Enum lower helper requires an Enum type")
  };
  let (layout, payload_offset, payload_layouts) = component_enum_layout(enum_type);
  let discriminant_size = component_enum_discriminant_size(enum_type.variants.len());
  let enum_ptr = 2;
  let dst_addr = 3;
  let src_addr = 4;
  let value = 5;
  let canonical_bool = 6;
  let matched = 7;
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
  push_checked_memory_region_const(&mut instructions, enum_ptr, 16);
  push_checked_alignment(&mut instructions, 1, layout.alignment);
  push_checked_memory_region_const(&mut instructions, 1, i64::from(layout.size));
  for (variant_index, (variant, (_, offsets))) in enum_type.variants.iter().zip(payload_layouts).enumerate() {
    instructions.extend([
      Instruction::LocalGet(enum_ptr),
      Instruction::F64Load(mem_arg_f64(8)),
      f64_const(f64::from(tag_ids[variant_index])),
      Instruction::F64Eq,
      Instruction::If(BlockType::Empty),
    ]);
    push_validate_internal_variant_count(&mut instructions, enum_ptr, variant.payload.len() as f64);
    push_checked_memory_region_const(&mut instructions, enum_ptr, ((variant.payload.len() + 2) * 8) as i64);
    instructions.extend([Instruction::I32Const(1), Instruction::LocalSet(matched), Instruction::LocalGet(1)]);
    instructions.push(Instruction::I32Const(variant_index as i32));
    instructions.push(match discriminant_size {
      1 => Instruction::I32Store8(mem_arg_byte(0)),
      2 => Instruction::I32Store16(mem_arg_byte(0)),
      4 => Instruction::I32Store(mem_arg_i32(0)),
      _ => unreachable!("Component Enum discriminant size must be 1, 2, or 4"),
    });
    for (payload_index, (payload_type, field_offset)) in variant.payload.iter().zip(offsets).enumerate() {
      instructions.extend([
        Instruction::LocalGet(enum_ptr),
        Instruction::I32Const(((payload_index + 2) * 8) as i32),
        Instruction::I32Add,
        Instruction::LocalSet(src_addr),
        Instruction::LocalGet(1),
        Instruction::I32Const(payload_offset + field_offset),
        Instruction::I32Add,
        Instruction::LocalSet(dst_addr),
      ]);
      push_lower_element(&mut instructions, payload_type, src_addr, dst_addr, value, canonical_bool, codecs);
    }
    instructions.push(Instruction::End);
  }
  instructions.extend([
    Instruction::LocalGet(matched),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::I32],
    results: vec![],
    locals: vec![ValType::I32, ValType::I32, ValType::I32, ValType::F64, ValType::I32, ValType::I32],
    instructions,
  }
}
