use super::*;
use std::collections::BTreeSet;

pub(super) fn component_struct_identity(record: &ComponentStructType) -> StructIdentity {
  StructIdentity {
    name: record.tag.clone(),
    definition: (record.id != record.tag).then(|| record.id.clone()),
    fields: record.fields.iter().map(|(name, _)| name.clone()).collect(),
  }
}

/// The same immutable identity components used by native Struct value equality.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct StructIdentity {
  pub name: String,
  pub definition: Option<String>,
  pub fields: Vec<String>,
}

impl From<&CalcitStructDef> for StructIdentity {
  fn from(definition: &CalcitStructDef) -> Self {
    Self {
      name: definition.name.ref_str().to_owned(),
      definition: definition.definition_ref.as_ref().map(ToString::to_string),
      fields: definition.fields.iter().map(|field| field.ref_str().to_owned()).collect(),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub(super) enum StructFieldKind {
  BoxedF64 = 0,
  BoolU8 = 1,
  TagU32 = 2,
  UnitZero = 3,
  Int8 = 4,
  UInt8 = 5,
  Int16 = 6,
  UInt16 = 7,
  Int32 = 8,
  UInt32 = 9,
  Int64 = 10,
  UInt64 = 11,
  HeapPtrI32 = 12,
}

impl StructFieldKind {
  pub(super) fn byte_size(self) -> u32 {
    match self {
      Self::UnitZero => 0,
      Self::BoolU8 | Self::Int8 | Self::UInt8 => 1,
      Self::Int16 | Self::UInt16 => 2,
      Self::TagU32 | Self::Int32 | Self::UInt32 | Self::HeapPtrI32 => 4,
      Self::BoxedF64 | Self::Int64 | Self::UInt64 => 8,
    }
  }

  pub(super) fn align(self) -> u32 {
    self.byte_size().max(1)
  }

  pub(super) fn from_type_annotation(annotation: &Arc<CalcitTypeAnnotation>) -> Self {
    let expanded = expand_wasm_value_type(annotation);
    match expanded.as_ref() {
      CalcitTypeAnnotation::Bool => Self::BoolU8,
      CalcitTypeAnnotation::Tag => Self::TagU32,
      CalcitTypeAnnotation::Unit => Self::UnitZero,
      CalcitTypeAnnotation::Numeric(refinement) => match refinement {
        CalcitNumericRefinement::Int8 => Self::Int8,
        CalcitNumericRefinement::UInt8 => Self::UInt8,
        CalcitNumericRefinement::Int16 => Self::Int16,
        CalcitNumericRefinement::UInt16 => Self::UInt16,
        CalcitNumericRefinement::Int32 => Self::Int32,
        CalcitNumericRefinement::UInt32 => Self::UInt32,
        CalcitNumericRefinement::Int64 => Self::Int64,
        CalcitNumericRefinement::UInt64 => Self::UInt64,
        CalcitNumericRefinement::Float32 | CalcitNumericRefinement::Float64 => Self::BoxedF64,
      },
      CalcitTypeAnnotation::String
      | CalcitTypeAnnotation::Buffer
      | CalcitTypeAnnotation::List(_)
      | CalcitTypeAnnotation::Tuple(_)
      | CalcitTypeAnnotation::Set(_)
      | CalcitTypeAnnotation::Map(_, _)
      | CalcitTypeAnnotation::Struct(_, _)
      | CalcitTypeAnnotation::StructValue(_)
      | CalcitTypeAnnotation::Enum(_, _)
      | CalcitTypeAnnotation::EnumValue(_)
      | CalcitTypeAnnotation::AnonymousEnum => Self::HeapPtrI32,
      CalcitTypeAnnotation::TypeRef(_, _) if expanded.resolve_to_struct().is_some() || expanded.resolve_to_enum().is_some() => {
        Self::HeapPtrI32
      }
      _ => Self::BoxedF64,
    }
  }

  pub(super) fn from_component_type(value_type: &ComponentAbiType) -> Self {
    match value_type {
      ComponentAbiType::Unit => Self::UnitZero,
      ComponentAbiType::Bool => Self::BoolU8,
      ComponentAbiType::Number => Self::BoxedF64,
      ComponentAbiType::Numeric(refinement) => match refinement {
        CalcitNumericRefinement::Int8 => Self::Int8,
        CalcitNumericRefinement::UInt8 => Self::UInt8,
        CalcitNumericRefinement::Int16 => Self::Int16,
        CalcitNumericRefinement::UInt16 => Self::UInt16,
        CalcitNumericRefinement::Int32 => Self::Int32,
        CalcitNumericRefinement::UInt32 => Self::UInt32,
        CalcitNumericRefinement::Int64 => Self::Int64,
        CalcitNumericRefinement::UInt64 => Self::UInt64,
        CalcitNumericRefinement::Float32 | CalcitNumericRefinement::Float64 => Self::BoxedF64,
      },
      ComponentAbiType::Buffer
      | ComponentAbiType::String
      | ComponentAbiType::List(_)
      | ComponentAbiType::Option(_)
      | ComponentAbiType::Result(_, _)
      | ComponentAbiType::Struct(_)
      | ComponentAbiType::Enum(_) => Self::HeapPtrI32,
    }
  }

  pub(super) fn native_repr(self) -> NativeValRepr {
    match self {
      Self::BoxedF64 => NativeValRepr::F64,
      Self::BoolU8 => NativeValRepr::BoolI32,
      Self::TagU32 => NativeValRepr::TagI32,
      Self::UnitZero => NativeValRepr::UnitI32,
      Self::Int8 | Self::Int16 | Self::Int32 => NativeValRepr::Int32S,
      Self::UInt8 | Self::UInt16 | Self::UInt32 => NativeValRepr::UInt32,
      Self::Int64 => NativeValRepr::Int64S,
      Self::UInt64 => NativeValRepr::UInt64,
      Self::HeapPtrI32 => NativeValRepr::HeapPtrI32,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StructFieldLayout {
  pub offset: u16,
  pub kind: StructFieldKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StructLayout {
  pub payload_bytes: u32,
  pub fields: Vec<StructFieldLayout>,
}

impl StructLayout {
  /// Pack fields after the 8-byte `[count:u32 @ 0][layout_id:u32 @ 4]` header,
  /// ordered by descending natural alignment (tie-breaking on logical index)
  /// and padded up to an 8-byte allocation multiple.
  pub(super) fn compute(kinds: &[StructFieldKind]) -> Result<Self, String> {
    let mut order: Vec<usize> = (0..kinds.len()).collect();
    order.sort_by(|&a, &b| kinds[b].align().cmp(&kinds[a].align()).then_with(|| a.cmp(&b)));
    let mut offsets = vec![0u16; kinds.len()];
    let mut cursor = 8u32;
    for idx in order {
      let kind = kinds[idx];
      let align = kind.align();
      let size = kind.byte_size();
      cursor = (cursor + align - 1) & !(align - 1);
      if size > 0 && cursor > u16::MAX as u32 {
        return Err("Struct layout offset exceeds u16 metadata range".into());
      }
      offsets[idx] = if size == 0 { 0 } else { cursor as u16 };
      cursor = cursor
        .checked_add(size)
        .ok_or_else(|| "Struct layout size overflowed u32".to_owned())?;
    }
    let payload_bytes = cursor.max(8).checked_add(7).ok_or("Struct size overflowed u32")? & !7;
    let fields = kinds
      .iter()
      .zip(offsets)
      .map(|(&kind, offset)| StructFieldLayout { offset, kind })
      .collect();
    Ok(Self { payload_bytes, fields })
  }
}

#[derive(Clone, Default)]
pub(super) struct StructLayouts {
  pub ids: BTreeMap<StructIdentity, u32>,
  pub names: BTreeMap<u32, u32>,
  pub field_tags: HashMap<u32, Vec<u32>>,
  pub layouts: HashMap<u32, StructLayout>,
  pub desc_table_base: i32,
  pub max_layout_id: u32,
}

pub(super) const STRUCT_FIELD_META_STRIDE: i32 = 4;
pub(super) const STRUCT_FIELD_OFFSET_MASK: i32 = 0xFFFF;
pub(super) const STRUCT_FIELD_KIND_SHIFT: i32 = 16;
pub(super) const STRUCT_FIELD_KIND_MASK: i32 = 0xFF;

pub(super) fn push_load_struct_field_meta(out: &mut Vec<Instruction<'static>>, meta_ptr_local: u32, index_local: u32) {
  out.extend([
    Instruction::LocalGet(meta_ptr_local),
    Instruction::LocalGet(index_local),
    Instruction::I32Const(STRUCT_FIELD_META_STRIDE),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::I32Load(mem_arg_i32(0)),
  ]);
}

pub(super) fn push_extract_struct_field_offset(out: &mut Vec<Instruction<'static>>, packed_local: u32) {
  out.extend([
    Instruction::LocalGet(packed_local),
    Instruction::I32Const(STRUCT_FIELD_OFFSET_MASK),
    Instruction::I32And,
  ]);
}

pub(super) fn push_extract_struct_field_kind(out: &mut Vec<Instruction<'static>>, packed_local: u32) {
  out.extend([
    Instruction::LocalGet(packed_local),
    Instruction::I32Const(STRUCT_FIELD_KIND_SHIFT),
    Instruction::I32ShrU,
    Instruction::I32Const(STRUCT_FIELD_KIND_MASK),
    Instruction::I32And,
  ]);
}

impl StructLayouts {
  pub fn id(&self, definition: &CalcitStructDef) -> Result<u32, String> {
    self.ids.get(&StructIdentity::from(definition)).copied().ok_or_else(|| {
      format!(
        "E_WASM_STRUCT_IDENTITY: unregistered Struct definition {:?}",
        definition.definition_ref
      )
    })
  }

  pub fn layout_by_id(&self, id: u32) -> Result<&StructLayout, String> {
    self
      .layouts
      .get(&id)
      .ok_or_else(|| format!("E_WASM_STRUCT_IDENTITY: missing Struct layout for id {id}"))
  }

  /// Serialize static Struct layout descriptors and per-field metadata into the
  /// read-only static data segment, maintaining 8-byte heap alignment at end.
  pub fn serialize_descriptor_table(&mut self, data_segment: &mut Vec<u8>, heap_start: &mut i32) {
    let max_id = self.ids.values().copied().max().unwrap_or(0);
    self.max_layout_id = max_id;
    if max_id == 0 {
      self.desc_table_base = 0;
      while !data_segment.len().is_multiple_of(8) {
        data_segment.push(0);
      }
      *heap_start = HEAP_BASE + data_segment.len() as i32;
      return;
    }
    let mut entries = Vec::with_capacity(max_id as usize + 1);
    entries.push((8u32, 0u32, 0u32));
    for id in 1..=max_id {
      let Some(layout) = self.layouts.get(&id) else {
        entries.push((8, 0, 0));
        continue;
      };
      if layout.fields.is_empty() {
        entries.push((layout.payload_bytes, 0, 0));
      } else {
        while !data_segment.len().is_multiple_of(4) {
          data_segment.push(0);
        }
        let meta_ptr = (HEAP_BASE + data_segment.len() as i32) as u32;
        for field in &layout.fields {
          let packed = u32::from(field.offset) | (u32::from(field.kind as u8) << STRUCT_FIELD_KIND_SHIFT);
          data_segment.extend_from_slice(&packed.to_le_bytes());
        }
        entries.push((layout.payload_bytes, layout.fields.len() as u32, meta_ptr));
      }
    }
    while !data_segment.len().is_multiple_of(4) {
      data_segment.push(0);
    }
    self.desc_table_base = HEAP_BASE + data_segment.len() as i32;
    for (payload_bytes, field_count, meta_ptr) in entries {
      data_segment.extend_from_slice(&payload_bytes.to_le_bytes());
      data_segment.extend_from_slice(&field_count.to_le_bytes());
      data_segment.extend_from_slice(&meta_ptr.to_le_bytes());
    }
    while !data_segment.len().is_multiple_of(8) {
      data_segment.push(0);
    }
    *heap_start = HEAP_BASE + data_segment.len() as i32;
  }
}

pub(super) fn collect_struct_layouts(
  program_data: &program::CompiledProgram,
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  extra_exprs: &[Calcit],
  component_types: &[ComponentAbiType],
  tags: &HashMap<String, u32>,
  reachable_defs: Option<&HashSet<(String, String)>>,
) -> Result<StructLayouts, String> {
  fn record_kinds(
    identity: StructIdentity,
    kinds: Vec<StructFieldKind>,
    field_kinds: &mut BTreeMap<StructIdentity, Vec<StructFieldKind>>,
  ) {
    match field_kinds.get_mut(&identity) {
      Some(existing) if existing.len() == kinds.len() => {
        for (slot, candidate) in existing.iter_mut().zip(kinds) {
          if *slot != candidate {
            *slot = StructFieldKind::BoxedF64;
          }
        }
      }
      Some(_) => {}
      None => {
        field_kinds.insert(identity, kinds);
      }
    }
  }

  fn kinds_from_struct_def(definition: &CalcitStructDef) -> Vec<StructFieldKind> {
    if definition.field_types.len() == definition.fields.len() {
      definition.field_types.iter().map(StructFieldKind::from_type_annotation).collect()
    } else {
      vec![StructFieldKind::BoxedF64; definition.fields.len()]
    }
  }

  fn visit(expression: &Calcit, field_kinds: &mut BTreeMap<StructIdentity, Vec<StructFieldKind>>) {
    if let Some(graph) = DataShapeGraph::from_calcit_handle(expression) {
      for node in &graph.nodes {
        if let DataShapeNode::Struct { nominal, .. } = node {
          record_kinds(StructIdentity::from(nominal.as_ref()), kinds_from_struct_def(nominal), field_kinds);
        }
      }
    }
    match expression {
      Calcit::StructDef(definition) => {
        record_kinds(
          StructIdentity::from(definition.as_ref()),
          kinds_from_struct_def(definition.as_ref()),
          field_kinds,
        );
      }
      Calcit::Import(_) => {
        if let Ok(definition) = resolve_struct_ref(expression) {
          record_kinds(StructIdentity::from(&definition), kinds_from_struct_def(&definition), field_kinds);
        }
      }
      Calcit::List(items) => {
        for item in items.iter() {
          visit(item, field_kinds);
        }
      }
      Calcit::Fn { info, .. } => {
        for item in &info.body {
          visit(item, field_kinds);
        }
      }
      _ => {}
    }
  }
  let mut field_kinds: BTreeMap<StructIdentity, Vec<StructFieldKind>> = BTreeMap::new();
  for (_, _, _, body) in fn_defs {
    for expression in body {
      visit(expression, &mut field_kinds);
    }
  }
  for expression in extra_exprs {
    visit(expression, &mut field_kinds);
  }
  for (ns, file) in program_data.iter() {
    for (name, compiled) in &file.defs {
      if let Some(reachable) = reachable_defs
        && !reachable.contains(&(ns.to_string(), name.to_string()))
      {
        continue;
      }
      if let Some(Calcit::StructDef(definition)) = program::lookup_runtime_ready(ns, name) {
        record_kinds(
          StructIdentity::from(definition.as_ref()),
          kinds_from_struct_def(definition.as_ref()),
          &mut field_kinds,
        );
      }
      for expression in [&compiled.preprocessed_code, &compiled.codegen_form] {
        visit(expression, &mut field_kinds);
      }
    }
  }
  // Register compiled `defstruct` declarations (such as `calcit.core/HttpRequest`) that are only
  // consumed through Component / WASI HTTP boundaries without a source constructor call.
  // Done with `or_insert_with` after `visit` so raw un-preprocessed declarations never widen compact structs.
  for (ns, file) in program_data.iter() {
    for (name, compiled) in &file.defs {
      if let Some(reachable) = reachable_defs
        && !reachable.contains(&(ns.to_string(), name.to_string()))
      {
        continue;
      }
      for expression in [&compiled.preprocessed_code, &compiled.codegen_form] {
        let resolved =
          match expression {
            Calcit::StructDef(def) => Some(def.as_ref().clone()),
            _ => try_parse_defrecord_form(expression).or_else(|| {
              match crate::wasm::host::calcit::type_annotation::resolve_type_def_from_code(expression) {
                Some(Calcit::StructDef(def)) => Some(def.as_ref().clone()),
                _ => None,
              }
            }),
          };
        if let Some(definition) = resolved {
          let definition = if definition.definition_ref.is_some() {
            definition
          } else {
            definition.with_definition_ref(ns, name)
          };
          field_kinds
            .entry(StructIdentity::from(&definition))
            .or_insert_with(|| kinds_from_struct_def(&definition));
        }
      }
    }
  }
  for ty in component_types {
    if let ComponentAbiType::Struct(record) = ty {
      let identity = component_struct_identity(record);
      field_kinds.entry(identity).or_insert_with(|| {
        record
          .fields
          .iter()
          .map(|(_, field_ty)| StructFieldKind::from_component_type(field_ty))
          .collect()
      });
    }
  }
  let mut layouts = StructLayouts::default();
  for (index, (identity, kinds)) in field_kinds.into_iter().enumerate() {
    let id = (index + 1) as u32;
    let tag = *tags
      .get(&identity.name)
      .ok_or_else(|| format!("missing Struct name tag {}", identity.name))?;
    let fields = identity
      .fields
      .iter()
      .map(|field| tags.get(field).copied().ok_or_else(|| format!("missing Struct field tag {field}")))
      .collect::<Result<Vec<_>, _>>()?;
    let layout = StructLayout::compute(&kinds)?;
    layouts.names.insert(id, tag);
    layouts.field_tags.insert(id, fields);
    layouts.layouts.insert(id, layout);
    layouts.ids.insert(identity, id);
  }
  Ok(layouts)
}

/// Store `[count:u32 @ 0][tag_or_layout:u32 @ 4]` into an 8-byte aligned heap object.
pub(super) fn emit_store_compact_header(ctx: &mut WasmGenCtx, ptr_local: u32, count: u32, second: u32) {
  let packed = ((second as u64) << 32) | (count as u64);
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::I64Const(packed as i64));
  ctx.emit(Instruction::I64Store(mem_arg_i64(0)));
}

fn emit_store_struct_field_instr(ctx: &mut WasmGenCtx, field: StructFieldLayout) {
  let offset = u64::from(field.offset);
  match field.kind {
    StructFieldKind::BoxedF64 => ctx.emit(Instruction::F64Store(mem_arg_f64(offset))),
    StructFieldKind::BoolU8 | StructFieldKind::Int8 | StructFieldKind::UInt8 => {
      ctx.emit(Instruction::I32Store8(mem_arg_byte(offset)));
    }
    StructFieldKind::Int16 | StructFieldKind::UInt16 => {
      ctx.emit(Instruction::I32Store16(mem_arg_i16(offset)));
    }
    StructFieldKind::TagU32 | StructFieldKind::Int32 | StructFieldKind::UInt32 | StructFieldKind::HeapPtrI32 => {
      ctx.emit(Instruction::I32Store(mem_arg_i32(offset)));
    }
    StructFieldKind::Int64 | StructFieldKind::UInt64 => {
      ctx.emit(Instruction::I64Store(mem_arg_i64(offset)));
    }
    StructFieldKind::UnitZero => unreachable!("UnitZero fields occupy 0 bytes"),
  }
}

pub(super) fn emit_store_struct_field_expr(
  ctx: &mut WasmGenCtx,
  ptr_local: u32,
  field: StructFieldLayout,
  value_expr: &Calcit,
) -> Result<(), String> {
  if field.kind == StructFieldKind::UnitZero {
    emit_expr(ctx, value_expr)?;
    ctx.emit(Instruction::Drop);
    return Ok(());
  }
  let repr = field.kind.native_repr();
  let val_local = ctx.alloc_local_typed(repr.val_type());
  emit_expr_as_repr(ctx, value_expr, repr)?;
  ctx.emit(Instruction::LocalSet(val_local));
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::LocalGet(val_local));
  emit_store_struct_field_instr(ctx, field);
  Ok(())
}

pub(super) fn emit_store_struct_field_from_f64_local(
  ctx: &mut WasmGenCtx,
  ptr_local: u32,
  field: StructFieldLayout,
  value_f64_local: u32,
) {
  if field.kind == StructFieldKind::UnitZero {
    return;
  }
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::LocalGet(value_f64_local));
  emit_unbox_f64_to_repr(ctx, field.kind.native_repr());
  emit_store_struct_field_instr(ctx, field);
}

pub(super) fn emit_load_struct_field_as_repr(ctx: &mut WasmGenCtx, ptr_local: u32, field: StructFieldLayout, dst_repr: NativeValRepr) {
  let offset = u64::from(field.offset);
  if field.kind == StructFieldKind::UnitZero {
    match dst_repr {
      NativeValRepr::UnitI32 => ctx.emit(Instruction::I32Const(0)),
      _ => {
        ctx.emit(f64_const(0.0));
        emit_unbox_f64_to_repr(ctx, dst_repr);
      }
    }
    return;
  }
  ctx.emit(Instruction::LocalGet(ptr_local));
  match field.kind {
    StructFieldKind::BoxedF64 => ctx.emit(Instruction::F64Load(mem_arg_f64(offset))),
    StructFieldKind::BoolU8 | StructFieldKind::UInt8 => ctx.emit(Instruction::I32Load8U(mem_arg_byte(offset))),
    StructFieldKind::Int8 => ctx.emit(Instruction::I32Load8S(mem_arg_byte(offset))),
    StructFieldKind::UInt16 => ctx.emit(Instruction::I32Load16U(mem_arg_i16(offset))),
    StructFieldKind::Int16 => ctx.emit(Instruction::I32Load16S(mem_arg_i16(offset))),
    StructFieldKind::TagU32 | StructFieldKind::Int32 | StructFieldKind::UInt32 | StructFieldKind::HeapPtrI32 => {
      ctx.emit(Instruction::I32Load(mem_arg_i32(offset)));
    }
    StructFieldKind::Int64 | StructFieldKind::UInt64 => ctx.emit(Instruction::I64Load(mem_arg_i64(offset))),
    StructFieldKind::UnitZero => unreachable!(),
  }
  emit_convert_repr(ctx, field.kind.native_repr(), dst_repr);
}

pub(super) fn emit_struct_new_raw_i32(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<u32, String> {
  if args.is_empty() {
    return Err("&%{} requires at least struct_ref argument".into());
  }
  let struct_def = resolve_struct_ref(&args[0])?;

  let field_count = struct_def.fields.len();

  // Remaining args are interleaved: :tag1, val1, :tag2, val2, ...
  let field_args = &args[1..];
  if field_args.len() != field_count * 2 {
    return Err(format!(
      "&%{{}}: expected {} tag-value pairs ({} args), got {}",
      field_count,
      field_count * 2,
      field_args.len()
    ));
  }
  // Validate the complete mapping before emitting allocation or payload code.
  // Source evaluation order is independent of the nominal storage layout.
  let mut seen = BTreeSet::new();
  let field_slots = field_args
    .as_chunks::<2>()
    .0
    .iter()
    .map(|pair| {
      let Calcit::Tag(tag) = &pair[0] else {
        return Err("&%{}: expected a literal field tag".to_owned());
      };
      let slot = struct_def
        .fields
        .iter()
        .position(|field| field == tag)
        .ok_or_else(|| format!("&%{{}}: unknown field :{tag}"))?;
      if !seen.insert(slot) {
        return Err(format!("&%{{}}: duplicate field :{tag}"));
      }
      Ok(slot)
    })
    .collect::<Result<Vec<_>, String>>()?;

  // Resolve the full definition identity and packed layout.
  let struct_tag_id = ctx.struct_layouts.id(&struct_def)?;
  let layout = ctx.struct_layouts.layout_by_id(struct_tag_id)?.clone();

  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, layout.payload_bytes as i32, ptr_local, "struct");
  emit_store_compact_header(ctx, ptr_local, field_count as u32, struct_tag_id);

  // Evaluate in source order and store in each field's packed slot.
  for (i, slot) in field_slots.into_iter().enumerate() {
    let value_expr = &field_args[i * 2 + 1];
    emit_store_struct_field_expr(ctx, ptr_local, layout.fields[slot], value_expr)?;
  }
  Ok(ptr_local)
}

pub(super) fn emit_struct_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  let ptr_local = emit_struct_new_raw_i32(ctx, args)?;
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Resolve a struct reference (either inline Calcit::StructDef or Calcit::Import) to a CalcitStructDef.
pub(super) fn resolve_struct_ref(node: &Calcit) -> Result<CalcitStructDef, String> {
  match node {
    Calcit::StructDef(s) => Ok(s.as_ref().clone()),
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      // Try runtime first
      if let Some(Calcit::StructDef(s)) = program::lookup_runtime_ready(ns, def) {
        return Ok(s.as_ref().clone());
      }
      // Try compiled def
      if let Some(compiled) = program::lookup_compiled_def(ns, def) {
        if let Calcit::StructDef(s) = &compiled.codegen_form {
          return Ok(s.as_ref().clone());
        }
        if let Calcit::StructDef(s) = &compiled.preprocessed_code {
          return Ok(s.as_ref().clone());
        }
        // Try to extract struct from defrecord form: (defrecord Name :field1 :field2 ...)
        if let Some(struct_def) = try_parse_defrecord_form(&compiled.codegen_form) {
          return Ok(struct_def.with_definition_ref(ns, def));
        }
        if let Some(struct_def) = try_parse_defrecord_form(&compiled.preprocessed_code) {
          return Ok(struct_def.with_definition_ref(ns, def));
        }
        return Err(format!("&%{{}}: compiled def {ns}/{def} is not a struct"));
      }
      // Try source code
      if let Some(source) = program::lookup_def_code(ns, def)
        && let Some(struct_def) = try_parse_defrecord_form(&source)
      {
        return Ok(struct_def.with_definition_ref(ns, def));
      }
      Err(format!("&%{{}}: cannot resolve struct reference {ns}/{def}"))
    }
    other => Err(format!("&%{{}}: expected struct reference, got: {other}")),
  }
}

/// Try to extract a CalcitStructDef from a `(defrecord Name :field1 :field2 ...)` form.
pub(super) fn try_parse_defrecord_form(code: &Calcit) -> Option<CalcitStructDef> {
  let Calcit::List(xs) = code else { return None };
  if xs.len() < 2 {
    return None;
  }
  // Check head is defrecord (Symbol)
  let is_defrecord = match &xs[0] {
    Calcit::Symbol { sym, .. } => sym.as_ref() == "defrecord" || sym.as_ref().ends_with("/defrecord"),
    _ => false,
  };
  if !is_defrecord {
    return None;
  }
  // Extract name
  let name = match &xs[1] {
    Calcit::Tag(t) => t.clone(),
    Calcit::Symbol { sym, .. } => {
      // ns/def format — extract just the def part
      let name_str = sym.as_ref().rsplit('/').next().unwrap_or_else(|| sym.as_ref());
      cirru_edn::EdnTag::from(name_str)
    }
    Calcit::Import(CalcitImport { def, .. }) => cirru_edn::EdnTag::from(def.as_ref()),
    _ => return None,
  };
  // Extract fields (remaining args that are Tags)
  let mut fields: Vec<cirru_edn::EdnTag> = Vec::new();
  for item in xs.iter().skip(2) {
    if let Calcit::Tag(t) = item {
      fields.push(t.clone());
    }
  }
  fields.sort();
  Some(CalcitStructDef {
    field_defaults: Default::default(),
    definition_ref: None,
    name,
    fields: std::sync::Arc::new(fields),
    field_types: std::sync::Arc::new(vec![]),
    generics: std::sync::Arc::new(vec![]),
    where_bounds: std::sync::Arc::new(vec![]),
    impls: vec![],
  })
}

/// Copy a validated Struct allocation before updating a statically selected field.
pub(super) fn emit_struct_assoc_at(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(4, args, "&struct:assoc-at requires 4 args (struct, index, tag, value)")?;
  let (index, field) = static_struct_update_field(&args[1], &args[2])?;
  let receiver = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(receiver));
  let value = ctx.alloc_local();
  emit_expr(ctx, &args[3])?;
  ctx.emit(Instruction::LocalSet(value));
  emit_struct_update_from_locals(ctx, receiver, value, index, &field)
}

fn static_struct_update_field(index: &Calcit, field: &Calcit) -> Result<(usize, String), String> {
  let (Calcit::Number(index), Calcit::Tag(field)) = (index, field) else {
    return Err("&struct:assoc-at requires a static field index and tag in WASM".into());
  };
  if !index.is_finite() || *index < 0.0 || index.fract() != 0.0 || *index > (u32::MAX / 8 - 2) as f64 {
    return Err("&struct:assoc-at field index is out of range".into());
  }
  Ok((*index as usize, field.ref_str().to_owned()))
}

/// Evaluate all update inputs before validating/copying, as native eager calls do.
pub(super) fn emit_struct_with_at(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() || !(args.len() - 1).is_multiple_of(3) {
    return Err("&struct:with-at requires (struct, index, tag, value, ...) triples".into());
  }
  let triples = args[1..].as_chunks::<3>().0;
  let fields = triples
    .iter()
    .map(|triple| static_struct_update_field(&triple[0], &triple[1]))
    .collect::<Result<Vec<_>, _>>()?;
  let receiver = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(receiver));
  let mut updates = Vec::with_capacity(fields.len());
  for ((index, field), triple) in fields.into_iter().zip(triples) {
    let value = ctx.alloc_local();
    emit_expr(ctx, &triple[2])?;
    ctx.emit(Instruction::LocalSet(value));
    updates.push((index, field, value));
  }
  if updates.is_empty() {
    emit_type_of_local(ctx, receiver);
    ctx.emit(f64_const(get_type_tag(ctx, "struct")));
    ctx.emit(Instruction::F64Ne);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    ctx.emit(Instruction::Unreachable);
    ctx.emit(Instruction::End);
  }
  for (index, field, value) in updates {
    emit_struct_update_from_locals(ctx, receiver, value, index, &field)?;
    ctx.emit(Instruction::LocalSet(receiver));
  }
  ctx.emit(Instruction::LocalGet(receiver));
  Ok(())
}

pub(super) fn emit_struct_update_from_locals(
  ctx: &mut WasmGenCtx,
  receiver: u32,
  value: u32,
  index: usize,
  field: &str,
) -> Result<(), String> {
  let field_id = ctx.tag_index.get(field).copied().ok_or("unknown Struct field tag")?;
  let mut layouts = ctx
    .struct_layouts
    .field_tags
    .iter()
    .filter_map(|(id, fields)| {
      if fields.get(index) != Some(&field_id) {
        return None;
      }
      let layout = ctx.struct_layouts.layouts.get(id)?;
      let field_layout = *layout.fields.get(index)?;
      Some((*id, fields.len(), layout.payload_bytes, field_layout))
    })
    .collect::<Vec<_>>();
  layouts.sort_by_key(|(id, ..)| *id);

  // Reuse heap range, magic and allocation-kind validation. A matching payload
  // layout slot in a List must not manufacture nominal Struct authority.
  emit_type_of_local(ctx, receiver);
  ctx.emit(f64_const(get_type_tag(ctx, "struct")));
  ctx.emit(Instruction::F64Ne);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  let src = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(receiver));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(src));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(7));
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  // The 8-byte compact header `[count:u32 @ 0][layout_id:u32 @ 4]` must exist inside the heap.
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I64ExtendI32U);
  ctx.emit(Instruction::I64Const(8));
  ctx.emit(Instruction::I64Add);
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I64ExtendI32U);
  ctx.emit(Instruction::I64GtU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalSet(count));
  let layout_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalSet(layout_local));

  let payload_bytes_local = ctx.alloc_local_typed(ValType::I32);
  let field_offset_local = ctx.alloc_local_typed(ValType::I32);
  let field_kind_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(payload_bytes_local));

  let mut unique_field: Option<StructFieldLayout> = None;
  let mut all_same_field = !layouts.is_empty();
  for &(layout_id, field_count, payload_bytes, field_layout) in &layouts {
    let payload_i32 = i32::try_from(payload_bytes).map_err(|_| "Struct layout is too large for WASM allocation")?;
    let field_count_i32 = i32::try_from(field_count).map_err(|_| "Struct field count is too large for WASM")?;
    match unique_field {
      None => unique_field = Some(field_layout),
      Some(prev) if prev != field_layout => all_same_field = false,
      _ => {}
    }
    ctx.emit(Instruction::LocalGet(layout_local));
    ctx.emit(Instruction::I32Const(layout_id as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(field_count_i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    ctx.emit(Instruction::I32Const(payload_i32));
    ctx.emit(Instruction::LocalSet(payload_bytes_local));
    ctx.emit(Instruction::I32Const(i32::from(field_layout.offset)));
    ctx.emit(Instruction::LocalSet(field_offset_local));
    ctx.emit(Instruction::I32Const(field_layout.kind as i32));
    ctx.emit(Instruction::LocalSet(field_kind_local));
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(payload_bytes_local));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  // Copy size comes from the validated registered layout, never receiver count.
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I64ExtendI32U);
  ctx.emit(Instruction::LocalGet(payload_bytes_local));
  ctx.emit(Instruction::I64ExtendI32U);
  ctx.emit(Instruction::I64Add);
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I64ExtendI32U);
  ctx.emit(Instruction::I64GtU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  let dst = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc_dynamic(ctx, payload_bytes_local, dst, "struct");
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalGet(payload_bytes_local));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  if all_same_field && let Some(field_layout) = unique_field {
    emit_store_struct_field_from_f64_local(ctx, dst, field_layout, value);
  } else {
    ctx.emit(Instruction::LocalGet(dst));
    ctx.emit(Instruction::LocalGet(field_offset_local));
    ctx.emit(Instruction::LocalGet(field_kind_local));
    ctx.emit(Instruction::LocalGet(value));
    ctx.call_rt("__rt_struct_store_field");
  }
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Read field `idx_local` from `ptr_local` via the static Struct descriptor table.
fn emit_dynamic_struct_field_load_as_f64(ctx: &mut WasmGenCtx, ptr_local: u32, idx_local: u32) {
  if ctx.struct_layouts.max_layout_id == 0 {
    ctx.emit(Instruction::Unreachable);
    return;
  }
  let layout_id = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalTee(layout_id));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32LtU);
  ctx.emit(Instruction::LocalGet(layout_id));
  ctx.emit(Instruction::I32Const(ctx.struct_layouts.max_layout_id as i32));
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  let desc_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(ctx.struct_layouts.desc_table_base));
  ctx.emit(Instruction::LocalGet(layout_id));
  ctx.emit(Instruction::I32Const(12));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(desc_ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4))); // field_count
  ctx.emit(Instruction::LocalGet(idx_local));
  ctx.emit(Instruction::I32LeU); // idx >= field_count (field_count <= idx)
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  let packed = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(desc_ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(8))); // fields_meta_ptr
  let meta_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(meta_ptr));
  push_load_struct_field_meta(&mut ctx.instructions, meta_ptr, idx_local);
  ctx.emit(Instruction::LocalSet(packed));

  ctx.emit(Instruction::LocalGet(ptr_local));
  push_extract_struct_field_offset(&mut ctx.instructions, packed);
  push_extract_struct_field_kind(&mut ctx.instructions, packed);
  ctx.call_rt("__rt_struct_load_field");
}

pub(super) fn emit_struct_nth_as_repr(ctx: &mut WasmGenCtx, args: &[Calcit], dst_repr: NativeValRepr) -> Result<(), String> {
  if args.len() < 2 {
    return Err("&struct:nth requires at least 2 args (struct_value, index)".into());
  }
  if let Calcit::Number(n) = &args[1]
    && n.is_finite()
    && *n >= 0.0
    && n.fract() == 0.0
    && let Some(struct_def) = infer_wasm_static_type(ctx, &args[0]).and_then(|ty| ty.resolve_to_struct())
    && let Ok(layout_id) = ctx.struct_layouts.id(&struct_def)
    && let Ok(layout) = ctx.struct_layouts.layout_by_id(layout_id).cloned()
    && let Some(&field_layout) = layout.fields.get(*n as usize)
  {
    let ptr_local = emit_ptr_to_i32(ctx, &args[0])?;
    emit_load_struct_field_as_repr(ctx, ptr_local, field_layout, dst_repr);
    return Ok(());
  }
  let ptr_local = emit_ptr_to_i32(ctx, &args[0])?;
  let idx_local = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx_local));
  emit_dynamic_struct_field_load_as_f64(ctx, ptr_local, idx_local);
  emit_unbox_f64_to_repr(ctx, dst_repr);
  Ok(())
}

pub(super) fn emit_struct_nth(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  emit_struct_nth_as_repr(ctx, args, NativeValRepr::F64)
}

/// Emit `&struct:get struct_value :field_tag` — dynamic field access by tag name.
pub(super) fn emit_struct_get(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&struct:get requires 2 args (struct_value, tag)")?;

  let record_ptr = emit_ptr_to_i32(ctx, &args[0])?;

  let struct_tag_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(record_ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalSet(struct_tag_local));

  let key_tag_local = ctx.alloc_local_typed(ValType::I32);
  emit_expr_as_repr(ctx, &args[1], NativeValRepr::TagI32)?;
  ctx.emit(Instruction::LocalSet(key_tag_local));

  let mut struct_entries = ctx
    .struct_layouts
    .field_tags
    .iter()
    .map(|(tag, fields)| (*tag, fields.clone()))
    .collect::<Vec<_>>();
  struct_entries.sort_by_key(|(tag, _)| *tag);

  for (struct_tag_id, field_tag_ids) in &struct_entries {
    let layout = ctx.struct_layouts.layout_by_id(*struct_tag_id)?.clone();
    ctx.emit(Instruction::LocalGet(struct_tag_local));
    ctx.emit(Instruction::I32Const(*struct_tag_id as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));

    for (field_idx, field_tag_id) in field_tag_ids.iter().enumerate() {
      ctx.emit(Instruction::LocalGet(key_tag_local));
      ctx.emit(Instruction::I32Const(*field_tag_id as i32));
      ctx.emit(Instruction::I32Eq);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      emit_load_struct_field_as_repr(ctx, record_ptr, layout.fields[field_idx], NativeValRepr::F64);
      ctx.emit(Instruction::Else);
    }
    ctx.emit(Instruction::Unreachable);
    for _ in field_tag_ids {
      ctx.emit(Instruction::End);
    }

    ctx.emit(Instruction::Else);
  }

  ctx.emit(Instruction::Unreachable);
  for _ in &struct_entries {
    ctx.emit(Instruction::End);
  }
  Ok(())
}

/// Emit `&struct:count struct_value` — returns the number of fields (`[ptr + 0]: u32`).
pub(super) fn emit_struct_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    return Err("&struct:count requires 1 arg (struct)".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

pub(super) fn emit_struct_field_tag(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&struct:field-tag requires 2 args (struct, index)")?;

  let ptr_local = emit_ptr_to_i32(ctx, &args[0])?;

  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  let idx_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(idx_local));

  let struct_tag_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalSet(struct_tag_local));

  let mut struct_entries = ctx
    .struct_layouts
    .field_tags
    .iter()
    .map(|(tag, fields)| (*tag, fields.clone()))
    .collect::<Vec<_>>();
  struct_entries.sort_by_key(|(tag, _)| *tag);

  if struct_entries.is_empty() {
    ctx.emit(f64_const(0.0));
    return Ok(());
  }

  for (struct_tag_id, field_tag_ids) in &struct_entries {
    ctx.emit(Instruction::LocalGet(struct_tag_local));
    ctx.emit(Instruction::I32Const(*struct_tag_id as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));

    if field_tag_ids.is_empty() {
      ctx.emit(f64_const(0.0));
    } else {
      for (field_idx, field_tag_id) in field_tag_ids.iter().enumerate() {
        ctx.emit(Instruction::LocalGet(idx_local));
        ctx.emit(Instruction::I32Const(field_idx as i32));
        ctx.emit(Instruction::I32Eq);
        ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
        ctx.emit(f64_const(*field_tag_id as f64));
        ctx.emit(Instruction::Else);
      }

      ctx.emit(f64_const(0.0));
      for _ in 0..field_tag_ids.len() {
        ctx.emit(Instruction::End);
      }
    }

    ctx.emit(Instruction::Else);
  }

  ctx.emit(f64_const(0.0));
  for _ in 0..struct_entries.len() {
    ctx.emit(Instruction::End);
  }
  Ok(())
}

pub(super) fn emit_struct_get_name(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&struct:get-name requires 1 arg (struct)")?;

  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let identity = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalSet(identity));
  let names = ctx.struct_layouts.names.clone();
  for (id, tag) in &names {
    ctx.emit(Instruction::LocalGet(identity));
    ctx.emit(Instruction::I32Const(*id as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
    ctx.emit(f64_const(*tag as f64));
    ctx.emit(Instruction::Else);
  }
  ctx.emit(Instruction::Unreachable);
  for _ in names {
    ctx.emit(Instruction::End);
  }
  Ok(())
}

// Native and JS return `Option :none` for anonymous structs. Anonymous structs come from
// `&loose-struct`, which WASM codegen rejects, so every struct value reaching this
// emitter has a definition and the result is always `Option :some`.
pub(super) fn emit_struct_def(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&struct:definition requires 1 arg (struct)")?;

  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let definition = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(definition));
  emit_option_enum(ctx, Some(definition))?;
  Ok(())
}

pub(super) fn emit_struct_to_map(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&struct:to-map requires 1 arg (struct)")?;

  let record_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let struct_tag_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(record_ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalSet(struct_tag_local));

  emit_map_new(ctx, &[])?;
  let map_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(map_ptr));

  let assoc_fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_map_assoc")
    .expect("runtime helper __rt_map_assoc must exist");

  let mut struct_entries = ctx
    .struct_layouts
    .field_tags
    .iter()
    .map(|(tag, fields)| (*tag, fields.clone()))
    .collect::<Vec<_>>();
  struct_entries.sort_by_key(|(tag, _)| *tag);

  for (struct_tag_id, field_tag_ids) in &struct_entries {
    let layout = ctx.struct_layouts.layout_by_id(*struct_tag_id)?.clone();
    ctx.emit(Instruction::LocalGet(struct_tag_local));
    ctx.emit(Instruction::I32Const(*struct_tag_id as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.begin_block_if();

    for (field_idx, field_tag_id) in field_tag_ids.iter().enumerate() {
      ctx.emit(Instruction::LocalGet(map_ptr));
      ctx.emit(f64_const(*field_tag_id as f64));
      emit_load_struct_field_as_repr(ctx, record_ptr, layout.fields[field_idx], NativeValRepr::F64);
      ctx.emit(Instruction::Call(assoc_fn_idx));
      ctx.emit(Instruction::LocalSet(map_ptr));
    }

    ctx.emit(Instruction::End);
  }

  ctx.emit(Instruction::LocalGet(map_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Match an instance against a definition ID or another instance's definition ID.
pub(super) fn emit_struct_matches(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&struct:matches? expects 2 args")?;
  let target_is_definition = if resolve_struct_ref(&args[1]).is_ok() {
    true
  } else if let Some(annotation) = infer_wasm_static_type(ctx, &args[1]) {
    match annotation.as_ref() {
      CalcitTypeAnnotation::StructDef(_) => true,
      CalcitTypeAnnotation::Struct(_, _) | CalcitTypeAnnotation::StructValue(_) => false,
      CalcitTypeAnnotation::Custom(value) if **value == Calcit::tag("struct-def") => true,
      CalcitTypeAnnotation::Custom(value) if **value == Calcit::tag("struct") => false,
      _ if annotation.resolve_to_struct().is_some() => false,
      _ => return Err("E_WASM_STRUCT_IDENTITY: &struct:matches? needs a proven Struct or StructDef target representation".into()),
    }
  } else {
    return Err("E_WASM_STRUCT_IDENTITY: &struct:matches? needs a proven Struct or StructDef target representation".into());
  };

  // Evaluate both operands once in source order. Definitions already are IDs;
  // instances carry `[count:u32 @ 0][layout_id:u32 @ 4]`.
  let instance_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let instance_id = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(instance_ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalSet(instance_id));

  let target_id = ctx.alloc_local_typed(ValType::I32);
  if target_is_definition {
    emit_expr(ctx, &args[1])?;
    ctx.emit(Instruction::I32TruncF64U);
  } else {
    let target_ptr = emit_ptr_to_i32(ctx, &args[1])?;
    ctx.emit(Instruction::LocalGet(target_ptr));
    ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  }
  ctx.emit(Instruction::LocalSet(target_id));

  ctx.emit(Instruction::LocalGet(instance_id));
  ctx.emit(Instruction::LocalGet(target_id));
  ctx.emit(Instruction::I32Eq);

  // Ownerless targets retain structural matching; named targets require their
  // exact module-local identity. Dynamically creating definitions is unsupported.
  let structural_pairs = ctx
    .struct_layouts
    .ids
    .iter()
    .filter(|(target, _)| target.definition.is_none())
    .flat_map(|(target, target_id)| {
      ctx.struct_layouts.ids.iter().filter_map(move |(instance, instance_id)| {
        (instance_id != target_id && instance.name == target.name && instance.fields == target.fields)
          .then_some((*instance_id, *target_id))
      })
    })
    .collect::<Vec<_>>();
  for (instance, target) in structural_pairs {
    ctx.emit(Instruction::LocalGet(instance_id));
    ctx.emit(Instruction::I32Const(instance as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::LocalGet(target_id));
    ctx.emit(Instruction::I32Const(target as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::I32Or);
  }
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.block_depth += 1;
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.block_depth -= 1;
  ctx.emit(Instruction::End);
  Ok(())
}

/// Emit `&struct:contains? struct_value key_tag` — check if a field tag exists in a struct value.
pub(super) fn emit_struct_contains(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&struct:contains? requires 2 args (struct_value, key_tag)")?;
  let record_ptr = emit_ptr_to_i32(ctx, &args[0])?;

  let struct_tag_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(record_ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalSet(struct_tag_local));

  let key_tag_local = ctx.alloc_local_typed(ValType::I32);
  emit_expr_as_repr(ctx, &args[1], NativeValRepr::TagI32)?;
  ctx.emit(Instruction::LocalSet(key_tag_local));

  let mut struct_entries = ctx
    .struct_layouts
    .field_tags
    .iter()
    .map(|(tag, fields)| (*tag, fields.clone()))
    .collect::<Vec<_>>();
  struct_entries.sort_by_key(|(tag, _)| *tag);

  for (struct_tag_id, field_tag_ids) in &struct_entries {
    ctx.emit(Instruction::LocalGet(struct_tag_local));
    ctx.emit(Instruction::I32Const(*struct_tag_id as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));

    for field_tag_id in field_tag_ids {
      ctx.emit(Instruction::LocalGet(key_tag_local));
      ctx.emit(Instruction::I32Const(*field_tag_id as i32));
      ctx.emit(Instruction::I32Eq);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      ctx.emit(f64_const(1.0));
      ctx.emit(Instruction::Else);
    }
    ctx.emit(f64_const(0.0));
    for _ in field_tag_ids {
      ctx.emit(Instruction::End);
    }

    ctx.emit(Instruction::Else);
  }

  ctx.emit(f64_const(0.0));
  for _ in &struct_entries {
    ctx.emit(Instruction::End);
  }
  Ok(())
}

// ---------------------------------------------------------------------------
// Enum operations
// ---------------------------------------------------------------------------

/// Emit `:: tag val0 val1 ...` — allocate an enum value in linear memory.
///
/// Compact layout: `[count:u32 @ 0][tag_id:u32 @ 4][payload_0:f64 @ 8]...`
/// `count` = number of payloads (excludes the tag itself).
pub(super) fn emit_enum_new_raw_i32(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<u32, String> {
  if args.is_empty() {
    return Err(":: requires at least a tag argument".into());
  }

  let static_tag_or_repr = match &args[0] {
    Calcit::Tag(t) => {
      let tag_str = t.to_string();
      let tag_id = *ctx
        .tag_index
        .get(&tag_str)
        .ok_or_else(|| format!("unknown tag in enum constructor: {tag_str}"))?;
      Ok(tag_id)
    }
    Calcit::Bool(b) => Ok(u32::from(*b)),
    other => match infer_wasm_static_type(ctx, other).map(|ty| CalcitTypeAnnotation::expand_schema_alias(&ty)) {
      Some(ty) if matches!(ty.as_ref(), CalcitTypeAnnotation::Bool) => Err(NativeValRepr::BoolI32),
      Some(ty) if matches!(ty.as_ref(), CalcitTypeAnnotation::Tag) => Err(NativeValRepr::TagI32),
      _ => return Err(format!("::: expected tag as first arg, got: {other}")),
    },
  };

  let payload = &args[1..];
  let total_size = ((1 + payload.len()) * 8) as i32;

  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_size, ptr_local, "enum");

  match static_tag_or_repr {
    Ok(tag_id) => {
      emit_store_compact_header(ctx, ptr_local, payload.len() as u32, tag_id);
    }
    Err(tag_repr) => {
      ctx.emit(Instruction::LocalGet(ptr_local));
      ctx.emit(Instruction::I32Const(payload.len() as i32));
      ctx.emit(Instruction::I32Store(mem_arg_i32(0)));

      let tag_local = ctx.alloc_local_typed(ValType::I32);
      emit_expr_as_repr(ctx, &args[0], tag_repr)?;
      ctx.emit(Instruction::LocalSet(tag_local));
      ctx.emit(Instruction::LocalGet(ptr_local));
      ctx.emit(Instruction::LocalGet(tag_local));
      ctx.emit(Instruction::I32Store(mem_arg_i32(4)));
    }
  }

  for (i, val) in payload.iter().enumerate() {
    let val_tmp = ctx.alloc_local();
    emit_expr(ctx, val)?;
    ctx.emit(Instruction::LocalSet(val_tmp));
    ctx.emit(Instruction::LocalGet(ptr_local));
    ctx.emit(Instruction::LocalGet(val_tmp));
    ctx.emit(Instruction::F64Store(mem_arg_f64(((1 + i) * 8) as u64)));
  }

  Ok(ptr_local)
}

pub(super) fn emit_enum_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  let ptr_local = emit_enum_new_raw_i32(ctx, args)?;
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Emit `%:: enum_class tag payload...` — enum variant constructor.
///
/// Args: `[enum_class, tag, payload...]`. Compact layout is identical to `::`.
pub(super) fn emit_named_enum_new_raw_i32(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<u32, String> {
  if args.len() < 2 {
    return Err("%:: requires at least (enum_class tag) arguments".into());
  }

  let tag_id = match &args[1] {
    Calcit::Tag(t) => {
      let tag_str = t.to_string();
      *ctx
        .tag_index
        .get(&tag_str)
        .ok_or_else(|| format!("unknown tag in enum constructor: {tag_str}"))?
    }
    other => return Err(format!("%:: expected tag as second arg, got: {other}")),
  };

  let payload = &args[2..];
  let total_size = ((1 + payload.len()) * 8) as i32;

  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_size, ptr_local, "enum");
  emit_store_compact_header(ctx, ptr_local, payload.len() as u32, tag_id);

  for (i, val) in payload.iter().enumerate() {
    let val_tmp = ctx.alloc_local();
    emit_expr(ctx, val)?;
    ctx.emit(Instruction::LocalSet(val_tmp));
    ctx.emit(Instruction::LocalGet(ptr_local));
    ctx.emit(Instruction::LocalGet(val_tmp));
    ctx.emit(Instruction::F64Store(mem_arg_f64(((1 + i) * 8) as u64)));
  }

  Ok(ptr_local)
}

pub(super) fn emit_named_enum_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  let ptr_local = emit_named_enum_new_raw_i32(ctx, args)?;
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Emit `&enum:nth enum_value idx` in the requested representation.
///
/// Compact layout: `[count:u32 @ 0][tag:u32 @ 4][payload0:f64 @ 8]...`
/// `idx == 0` returns tag (`I32Load(4)`), `1 <= idx <= count` returns
/// `payload[idx - 1]` (`F64Load(idx * 8)`), and out-of-range/invalid indices trap.
pub(super) fn emit_enum_nth_as_repr(ctx: &mut WasmGenCtx, args: &[Calcit], dst_repr: NativeValRepr) -> Result<(), String> {
  expect_arity(2, args, "&enum:nth requires 2 args (enum_value, index)")?;
  if let Calcit::Number(n) = &args[1] {
    let ptr = emit_ptr_to_i32(ctx, &args[0])?;
    if n.is_finite()
      && *n >= 0.0
      && n.fract() == 0.0
      && *n <= (u32::MAX / 8) as f64
      && let Some(byte_offset) = (*n as usize).checked_mul(8)
    {
      let idx = *n as usize;
      if idx == 0 {
        ctx.emit(Instruction::LocalGet(ptr));
        ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
        emit_convert_repr(ctx, NativeValRepr::TagI32, dst_repr);
      } else {
        ctx.emit(Instruction::LocalGet(ptr));
        ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
        ctx.emit(Instruction::I32Const(idx as i32));
        ctx.emit(Instruction::I32LtU);
        ctx.begin_block_if();
        ctx.emit(Instruction::Unreachable);
        ctx.emit(Instruction::End);
        ctx.emit(Instruction::LocalGet(ptr));
        ctx.emit(Instruction::F64Load(mem_arg_f64(byte_offset as u64)));
        emit_unbox_f64_to_repr(ctx, dst_repr);
      }
    } else {
      ctx.emit(Instruction::Unreachable);
    }
    return Ok(());
  }

  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let idx_f64 = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(idx_f64));

  // Reject NaN, negative, fractional, or out-of-range (> payload count) indices.
  ctx.emit(Instruction::LocalGet(idx_f64));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::LocalGet(idx_f64));
  ctx.emit(Instruction::LocalGet(idx_f64));
  ctx.emit(Instruction::F64Trunc);
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalGet(idx_f64));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Le);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  let idx_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(idx_f64));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx_local));

  ctx.emit(Instruction::LocalGet(idx_local));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(idx_local));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::End);
  emit_unbox_f64_to_repr(ctx, dst_repr);
  Ok(())
}

pub(super) fn emit_enum_nth(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  emit_enum_nth_as_repr(ctx, args, NativeValRepr::F64)
}

/// Emit `&enum:count enum_value` — returns payload count + 1 (tag counts as element 0).
pub(super) fn emit_enum_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&enum:count expects 1 arg")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

pub(super) fn emit_enum_assoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&enum:assoc expects 3 args")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_enum_count_i32(ctx, src);
  let idx = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx));
  let val = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(val));

  let size = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalSet(size));

  let dst = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc_dynamic(ctx, size, dst, "enum");

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalGet(size));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(val));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Store(mem_arg_i32(4)));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(val));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

pub(super) fn enum_identity_tag(enum_def: &CalcitEnumDef) -> String {
  let def_ref = match enum_def.definition_ref() {
    Some(r) => format!("1:{r}"),
    None => "0:".to_owned(),
  };
  let generics = enum_def.generics().iter().map(|g| g.as_ref()).collect::<Vec<_>>().join(",");
  let bounds = enum_def
    .where_bounds()
    .iter()
    .map(|b| {
      let traits = b.traits.iter().map(|t| t.name.ref_str()).collect::<Vec<_>>().join("+");
      format!("{}:{traits}", b.name)
    })
    .collect::<Vec<_>>()
    .join(",");
  let variants = enum_def
    .variants()
    .iter()
    .map(|v| {
      let payloads = v
        .payload_types()
        .iter()
        .map(|t| format!("{}", t.to_type_edn()))
        .collect::<Vec<_>>()
        .join(",");
      format!("{}:{:08}:[{}]", v.tag, v.payload_types().len(), payloads)
    })
    .collect::<Vec<_>>()
    .join(";");
  format!(
    "__enum_def__:{}:{}:{:08}:<{}>:{:08}:<{}>:{:08}:{{{}}}",
    def_ref,
    enum_def.name(),
    enum_def.generics().len(),
    generics,
    enum_def.where_bounds().len(),
    bounds,
    enum_def.variants().len(),
    variants
  )
}

pub(super) fn resolve_enum_ref(node: &Calcit) -> Result<CalcitEnumDef, String> {
  match node {
    Calcit::EnumDef(e) => Ok(e.as_ref().clone()),
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      if let Some(Calcit::EnumDef(e)) = program::lookup_runtime_ready(ns, def) {
        return Ok(e.as_ref().clone());
      }
      if let Some(compiled) = program::lookup_compiled_def(ns, def) {
        if let Calcit::EnumDef(e) = &compiled.codegen_form {
          return Ok(e.as_ref().clone());
        }
        if let Calcit::EnumDef(e) = &compiled.preprocessed_code {
          return Ok(e.as_ref().clone());
        }
      }
      Err(format!("cannot resolve enum reference {ns}/{def}"))
    }
    other => Err(format!("expected enum reference, got: {other}")),
  }
}

pub(super) fn emit_enum_definition(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&enum:definition expects 1 arg")?;
  let value_type = infer_wasm_static_type(ctx, &args[0]).map(|ty| expand_wasm_value_type(&ty));
  if let Some(enum_def) = value_type.as_deref().and_then(|ty| ty.resolve_to_enum()) {
    emit_expr(ctx, &args[0])?;
    ctx.emit(Instruction::Drop);
    let enum_key = enum_identity_tag(&enum_def);
    let tag_id = *ctx
      .tag_index
      .get(&enum_key)
      .ok_or_else(|| format!("unknown enum definition tag in WASM: {}", enum_def.name()))?;
    let def_local = ctx.alloc_local();
    ctx.emit(f64_const(tag_id as f64));
    ctx.emit(Instruction::LocalSet(def_local));
    return emit_option_enum(ctx, Some(def_local));
  }
  if matches!(value_type.as_deref(), Some(CalcitTypeAnnotation::AnonymousEnum)) {
    emit_expr(ctx, &args[0])?;
    ctx.emit(Instruction::Drop);
    return emit_option_enum(ctx, None);
  }
  Err("Enum operation &enum:definition not yet supported in WASM codegen".into())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn struct_layout_packs_fields_by_descending_alignment_with_unit_zero() {
    let kinds = [
      StructFieldKind::UnitZero,
      StructFieldKind::BoolU8,
      StructFieldKind::Int16,
      StructFieldKind::BoxedF64,
      StructFieldKind::Int32,
      StructFieldKind::TagU32,
      StructFieldKind::Int8,
      StructFieldKind::HeapPtrI32,
      StructFieldKind::UInt64,
    ];
    let layout = StructLayout::compute(&kinds).expect("layout should compute");
    assert_eq!(layout.payload_bytes, 40);
    assert_eq!(
      layout.fields[0],
      StructFieldLayout {
        offset: 0,
        kind: StructFieldKind::UnitZero
      }
    );
    assert_eq!(
      layout.fields[3],
      StructFieldLayout {
        offset: 8,
        kind: StructFieldKind::BoxedF64
      }
    );
    assert_eq!(
      layout.fields[8],
      StructFieldLayout {
        offset: 16,
        kind: StructFieldKind::UInt64
      }
    );
    assert_eq!(
      layout.fields[4],
      StructFieldLayout {
        offset: 24,
        kind: StructFieldKind::Int32
      }
    );
    assert_eq!(
      layout.fields[5],
      StructFieldLayout {
        offset: 28,
        kind: StructFieldKind::TagU32
      }
    );
    assert_eq!(
      layout.fields[7],
      StructFieldLayout {
        offset: 32,
        kind: StructFieldKind::HeapPtrI32
      }
    );
    assert_eq!(
      layout.fields[2],
      StructFieldLayout {
        offset: 36,
        kind: StructFieldKind::Int16
      }
    );
    assert_eq!(
      layout.fields[1],
      StructFieldLayout {
        offset: 38,
        kind: StructFieldKind::BoolU8
      }
    );
    assert_eq!(
      layout.fields[6],
      StructFieldLayout {
        offset: 39,
        kind: StructFieldKind::Int8
      }
    );
  }

  #[test]
  fn struct_layout_pads_small_and_empty_structs_to_eight_byte_multiples() {
    let empty = StructLayout::compute(&[]).expect("empty struct");
    assert_eq!(empty.payload_bytes, 8);
    assert!(empty.fields.is_empty());

    let unit_only = StructLayout::compute(&[StructFieldKind::UnitZero]).expect("unit-only struct");
    assert_eq!(unit_only.payload_bytes, 8);
    assert_eq!(unit_only.fields[0].offset, 0);

    let single_bool = StructLayout::compute(&[StructFieldKind::BoolU8]).expect("single bool");
    assert_eq!(single_bool.payload_bytes, 16);
    assert_eq!(single_bool.fields[0].offset, 8);

    let packed_16 = StructLayout::compute(&[
      StructFieldKind::BoolU8,
      StructFieldKind::UInt8,
      StructFieldKind::Int16,
      StructFieldKind::TagU32,
    ])
    .expect("packed 16-byte struct");
    assert_eq!(packed_16.payload_bytes, 16);
    assert_eq!(packed_16.fields[3].offset, 8);
    assert_eq!(packed_16.fields[2].offset, 12);
    assert_eq!(packed_16.fields[0].offset, 14);
    assert_eq!(packed_16.fields[1].offset, 15);
  }

  #[test]
  fn struct_layouts_serialize_descriptor_table_aligns_heap_start_and_packs_metadata() {
    let mut layouts = StructLayouts::default();
    let id1 = StructIdentity {
      name: "Empty".into(),
      definition: Some("app/Empty".into()),
      fields: vec![],
    };
    let id2 = StructIdentity {
      name: "Packed".into(),
      definition: Some("app/Packed".into()),
      fields: vec!["flag".into(), "tag".into(), "unit".into()],
    };
    layouts.ids.insert(id1, 1);
    layouts.ids.insert(id2, 2);
    layouts.layouts.insert(1, StructLayout::compute(&[]).unwrap());
    let layout2 = StructLayout::compute(&[StructFieldKind::BoolU8, StructFieldKind::TagU32, StructFieldKind::UnitZero]).unwrap();
    layouts.layouts.insert(2, layout2.clone());

    let mut data_segment = vec![0xAAu8; 3];
    let mut heap_start = HEAP_BASE + 3;
    layouts.serialize_descriptor_table(&mut data_segment, &mut heap_start);

    assert_eq!(layouts.max_layout_id, 2);
    assert_eq!(layouts.desc_table_base % 4, 0);
    assert_eq!(heap_start % 8, 0);
    assert_eq!(heap_start, HEAP_BASE + data_segment.len() as i32);

    let read_u32 = |addr: i32| -> u32 {
      let off = (addr - HEAP_BASE) as usize;
      u32::from_le_bytes(data_segment[off..off + 4].try_into().unwrap())
    };
    assert_eq!(read_u32(layouts.desc_table_base + 12), 8);
    assert_eq!(read_u32(layouts.desc_table_base + 16), 0);
    assert_eq!(read_u32(layouts.desc_table_base + 20), 0);

    let meta_ptr = read_u32(layouts.desc_table_base + 32) as i32;
    assert_eq!(read_u32(layouts.desc_table_base + 24), layout2.payload_bytes);
    assert_eq!(read_u32(layouts.desc_table_base + 28), 3);
    assert_ne!(meta_ptr, 0);

    for (idx, field) in layout2.fields.iter().enumerate() {
      let packed = read_u32(meta_ptr + (idx as i32) * STRUCT_FIELD_META_STRIDE) as i32;
      assert_eq!((packed & STRUCT_FIELD_OFFSET_MASK) as u16, field.offset);
      assert_eq!(
        ((packed >> STRUCT_FIELD_KIND_SHIFT) & STRUCT_FIELD_KIND_MASK) as u8,
        field.kind as u8
      );
    }
  }

  #[test]
  fn collect_struct_layouts_includes_reachable_extra_exprs() {
    use cirru_edn::EdnTag;

    let anonymous_struct = Arc::new(CalcitStructDef::from_fields(EdnTag::new("MethodBox"), vec![EdnTag::new("value")]));
    let extra_exprs = vec![Calcit::StructDef(anonymous_struct.clone())];
    let tags = HashMap::from([("MethodBox".to_owned(), 101u32), ("value".to_owned(), 102u32)]);
    let empty_program = HashMap::new();
    let empty_reachable = HashSet::new();

    let layouts = collect_struct_layouts(&empty_program, &[], &extra_exprs, &[], &tags, Some(&empty_reachable))
      .expect("struct literal inside reached extra_exprs must register a layout");
    let identity = StructIdentity::from(anonymous_struct.as_ref());
    assert!(layouts.ids.contains_key(&identity), "expected struct layout id for {identity:?}");
  }

  #[test]
  fn struct_layouts_serialize_descriptor_table_skips_empty_table_when_no_structs() {
    let mut empty_layouts = StructLayouts::default();
    let mut empty_data = Vec::new();
    let mut empty_heap_start = HEAP_BASE;
    empty_layouts.serialize_descriptor_table(&mut empty_data, &mut empty_heap_start);
    assert_eq!(empty_layouts.max_layout_id, 0);
    assert_eq!(empty_layouts.desc_table_base, 0);
    assert!(empty_data.is_empty());
    assert_eq!(empty_heap_start, HEAP_BASE);

    let mut padded_layouts = StructLayouts::default();
    let mut padded_data = vec![0xAAu8; 3];
    let mut padded_heap_start = HEAP_BASE + 3;
    padded_layouts.serialize_descriptor_table(&mut padded_data, &mut padded_heap_start);
    assert_eq!(padded_layouts.max_layout_id, 0);
    assert_eq!(padded_layouts.desc_table_base, 0);
    assert_eq!(padded_data.len(), 8);
    assert_eq!(padded_heap_start, HEAP_BASE + 8);
  }
}
