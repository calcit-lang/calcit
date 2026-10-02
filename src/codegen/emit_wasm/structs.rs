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

#[derive(Clone, Default)]
pub(super) struct StructLayouts {
  pub ids: BTreeMap<StructIdentity, u32>,
  pub names: BTreeMap<u32, u32>,
  pub field_tags: HashMap<u32, Vec<u32>>,
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
}

pub(super) fn collect_struct_layouts(
  program_data: &program::CompiledProgram,
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  component_types: &[ComponentAbiType],
  tags: &HashMap<String, u32>,
) -> Result<StructLayouts, String> {
  fn visit(expression: &Calcit, identities: &mut BTreeSet<StructIdentity>) {
    if let Some(graph) = DataShapeGraph::from_calcit_handle(expression) {
      for node in &graph.nodes {
        if let DataShapeNode::Struct { nominal, .. } = node {
          identities.insert(StructIdentity::from(nominal.as_ref()));
        }
      }
    }
    match expression {
      Calcit::StructDef(definition) => {
        identities.insert(StructIdentity::from(definition));
      }
      Calcit::Import(_) => {
        if let Ok(definition) = resolve_struct_ref(expression) {
          identities.insert(StructIdentity::from(&definition));
        }
      }
      Calcit::List(items) => {
        for item in items.iter() {
          visit(item, identities);
        }
      }
      _ => {}
    }
  }
  let mut identities = BTreeSet::new();
  for (_, _, _, body) in fn_defs {
    for expression in body {
      visit(expression, &mut identities);
    }
  }
  for (ns, file) in program_data.iter() {
    for (name, compiled) in &file.defs {
      if let Some(Calcit::StructDef(definition)) = program::lookup_runtime_ready(ns, name) {
        identities.insert(StructIdentity::from(&definition));
      }
      for expression in [&compiled.preprocessed_code, &compiled.codegen_form] {
        visit(expression, &mut identities);
      }
    }
  }
  for ty in component_types {
    if let ComponentAbiType::Struct(record) = ty {
      identities.insert(component_struct_identity(record));
    }
  }
  let mut layouts = StructLayouts::default();
  for (index, identity) in identities.into_iter().enumerate() {
    let id = (index + 1) as u32;
    let tag = *tags
      .get(&identity.name)
      .ok_or_else(|| format!("missing Struct name tag {}", identity.name))?;
    let fields = identity
      .fields
      .iter()
      .map(|field| tags.get(field).copied().ok_or_else(|| format!("missing Struct field tag {field}")))
      .collect::<Result<Vec<_>, _>>()?;
    layouts.names.insert(id, tag);
    layouts.field_tags.insert(id, fields);
    layouts.ids.insert(identity, id);
  }
  Ok(layouts)
}

pub(super) fn emit_struct_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    return Err("&%{} requires at least struct_ref argument".into());
  }
  // First arg is the struct definition — resolve it
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

  // Resolve the full definition identity, independently of the display name.
  let struct_tag_id = ctx.struct_layouts.id(&struct_def)?;

  // Layout: [count:f64][struct_tag:f64][field0:f64][field1:f64]...
  // Total bytes: (2 + field_count) * 8
  let total_size = ((2 + field_count) * 8) as i32;

  // Allocate: save i32 pointer to a temporary local
  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_size, ptr_local, "struct");

  // Store field count at offset 0
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(field_count as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // Store the nominal layout identity at offset 8.
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(struct_tag_id as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  // Store each field value at offset (2 + i) * 8
  // field_args layout: [:tag0, val0, :tag1, val1, ...]
  for i in 0..field_count {
    let value_expr = &field_args[i * 2 + 1]; // skip the tag, take the value
    ctx.emit(Instruction::LocalGet(ptr_local));
    emit_expr(ctx, value_expr)?;
    ctx.emit(Instruction::F64Store(mem_arg_f64(((2 + i) * 8) as u64)));
  }

  // Return pointer as f64
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Resolve a struct reference (either inline Calcit::StructDef or Calcit::Import) to a CalcitStructDef.
pub(super) fn resolve_struct_ref(node: &Calcit) -> Result<CalcitStructDef, String> {
  match node {
    Calcit::StructDef(s) => Ok(s.clone()),
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      // Try runtime first
      if let Some(Calcit::StructDef(s)) = program::lookup_runtime_ready(ns, def) {
        return Ok(s);
      }
      // Try compiled def
      if let Some(compiled) = program::lookup_compiled_def(ns, def) {
        if let Calcit::StructDef(s) = &compiled.codegen_form {
          return Ok(s.clone());
        }
        if let Calcit::StructDef(s) = &compiled.preprocessed_code {
          return Ok(s.clone());
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
      let name_str = sym.as_ref().rsplit('/').next().unwrap_or(sym.as_ref());
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
  let (Calcit::Number(index), Calcit::Tag(field)) = (&args[1], &args[2]) else {
    return Err("&struct:assoc-at requires a static field index and tag in WASM".into());
  };
  if !index.is_finite() || *index < 0.0 || index.fract() != 0.0 || *index > (u32::MAX / 8 - 2) as f64 {
    return Err("&struct:assoc-at field index is out of range".into());
  }
  let index = *index as usize;
  let field_id = ctx.tag_index.get(field.ref_str()).copied().ok_or("unknown Struct field tag")?;
  let mut layouts = ctx
    .struct_layouts
    .field_tags
    .iter()
    .filter_map(|(id, fields)| (fields.get(index) == Some(&field_id)).then_some((*id, fields.len())))
    .collect::<Vec<_>>();
  layouts.sort_unstable();

  let receiver = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(receiver));
  let value = ctx.alloc_local();
  emit_expr(ctx, &args[3])?;
  ctx.emit(Instruction::LocalSet(value));

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
  // The count and identity prefix must exist before either slot is loaded.
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I64ExtendI32U);
  ctx.emit(Instruction::I64Const(16));
  ctx.emit(Instruction::I64Add);
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I64ExtendI32U);
  ctx.emit(Instruction::I64GtU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  let count = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(count));
  let slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(slots));
  for (layout, field_count) in layouts {
    let layout_slots = i32::try_from(field_count)
      .ok()
      .and_then(|count| count.checked_add(2))
      .filter(|slots| slots.checked_mul(8).is_some())
      .ok_or("Struct layout is too large for WASM allocation")?;
    ctx.emit(Instruction::LocalGet(src));
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(f64_const(layout as f64));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(f64_const(field_count as f64));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    ctx.emit(Instruction::I32Const(layout_slots));
    ctx.emit(Instruction::LocalSet(slots));
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(slots));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  // Copy size comes from the validated registered layout, never receiver count.
  let size = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(slots));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalSet(size));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I64ExtendI32U);
  ctx.emit(Instruction::LocalGet(size));
  ctx.emit(Instruction::I64ExtendI32U);
  ctx.emit(Instruction::I64Add);
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I64ExtendI32U);
  ctx.emit(Instruction::I64GtU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  let dst = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc_dynamic(ctx, size, dst, "struct");
  let dst_base = emit_addr_offset(ctx, dst, 0);
  let src_base = emit_addr_offset(ctx, src, 0);
  emit_copy_f64_loop(ctx, dst_base, src_base, slots);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::F64Store(mem_arg_f64(((index + 2) * 8) as u64)));
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

pub(super) fn emit_struct_nth(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  // args: [struct_value_expr, idx_expr, tag_expr]
  if args.len() < 2 {
    return Err("&struct:nth requires at least 2 args (struct_value, index)".into());
  }
  // Layout: [count:f64][struct_tag:f64][field0:f64]...
  // Field at byte offset (2 + idx) * 8 from the struct_value pointer
  match &args[1] {
    Calcit::Number(n) => {
      // Static index — compile-time constant offset
      let idx = *n as usize;
      let offset = ((2 + idx) * 8) as u64;
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::F64Load(mem_arg_f64(offset)));
    }
    _ => {
      // Dynamic index — compute offset at runtime: (2 + idx) * 8
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::I32TruncF64U);
      let ptr_local = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalSet(ptr_local));
      // Compute byte offset: (2 + idx) * 8
      emit_expr(ctx, &args[1])?;
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::I32Const(2));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Mul);
      // Add base pointer
      ctx.emit(Instruction::LocalGet(ptr_local));
      ctx.emit(Instruction::I32Add);
      // Load f64 at computed offset
      ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    }
  }
  Ok(())
}

/// Emit `&struct:get struct_value :field_tag` — dynamic field access by tag name.
///
/// Performs a compile-time dispatch table: for each known struct type, scans
/// field tags and returns the matching field value. A missing tag traps instead
/// of silently returning the numeric representation of nil.
pub(super) fn emit_struct_get(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&struct:get requires 2 args (struct_value, tag)")?;

  let record_ptr = emit_ptr_to_i32(ctx, &args[0])?;

  // Load struct_tag from struct_value at offset 8
  let struct_tag_local = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(record_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(struct_tag_local));

  // Evaluate key_tag argument
  let key_tag_local = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(key_tag_local));

  let mut struct_entries = ctx
    .struct_layouts
    .field_tags
    .iter()
    .map(|(tag, fields)| (*tag, fields.clone()))
    .collect::<Vec<_>>();
  struct_entries.sort_by_key(|(tag, _)| *tag);

  // For each struct type: if struct_tag matches, scan field tags and return matching value
  for (struct_tag_id, field_tag_ids) in &struct_entries {
    ctx.emit(Instruction::LocalGet(struct_tag_local));
    ctx.emit(f64_const(*struct_tag_id as f64));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));

    // Nested if-chain: return field value for matching tag, else trap.
    for (field_idx, field_tag_id) in field_tag_ids.iter().enumerate() {
      ctx.emit(Instruction::LocalGet(key_tag_local));
      ctx.emit(f64_const(*field_tag_id as f64));
      ctx.emit(Instruction::F64Eq);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      // Load field value at offset (2 + field_idx) * 8
      ctx.emit(Instruction::LocalGet(record_ptr));
      ctx.emit(Instruction::F64Load(mem_arg_f64(((2 + field_idx) * 8) as u64)));
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

/// Emit `&struct:count struct_value` — returns the number of fields.
/// Layout: [count:f64][struct_tag:f64][fields...]
/// Count is at offset 0 from the struct_value pointer.
pub(super) fn emit_struct_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    return Err("&struct:count requires 1 arg (struct)".into());
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  Ok(())
}

pub(super) fn emit_struct_field_tag(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&struct:field-tag requires 2 args (struct, index)")?;

  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(ptr_local));

  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  let idx_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(idx_local));

  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  let struct_tag_local = ctx.alloc_local();
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
    ctx.emit(f64_const(*struct_tag_id as f64));
    ctx.emit(Instruction::F64Eq);
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

  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  let identity = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(identity));
  let names = ctx.struct_layouts.names.clone();
  for (id, tag) in &names {
    ctx.emit(Instruction::LocalGet(identity));
    ctx.emit(f64_const(*id as f64));
    ctx.emit(Instruction::F64Eq);
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

pub(super) fn emit_struct_def(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&struct:definition requires 1 arg (struct)")?;

  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  Ok(())
}

pub(super) fn emit_struct_to_map(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&struct:to-map requires 1 arg (struct)")?;

  let record_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let struct_tag_local = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(record_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
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
    ctx.emit(Instruction::LocalGet(struct_tag_local));
    ctx.emit(f64_const(*struct_tag_id as f64));
    ctx.emit(Instruction::F64Eq);
    ctx.begin_block_if();

    for (field_idx, field_tag_id) in field_tag_ids.iter().enumerate() {
      ctx.emit(Instruction::LocalGet(map_ptr));
      ctx.emit(f64_const(*field_tag_id as f64));
      ctx.emit(Instruction::LocalGet(record_ptr));
      ctx.emit(Instruction::F64Load(mem_arg_f64(((2 + field_idx) * 8) as u64)));
      ctx.emit(Instruction::Call(assoc_fn_idx));
      ctx.emit(Instruction::LocalSet(map_ptr));
    }

    ctx.emit(Instruction::End);
  }

  ctx.emit(Instruction::LocalGet(map_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Emit `&struct:matches? a b` — check if two struct values have the same struct type.
///
/// Struct value layout: [count: f64] [struct_tag: f64] [field0: f64] ...
/// Compares the struct_tag (offset 8) of both struct values.
pub(super) fn emit_struct_matches(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&struct:matches? expects 2 args")?;
  // Load struct_tag of first struct_value (at offset 8, after count)
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  // Load struct_tag of second struct_value (at offset 8, after count)
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  // Compare and return 1.0 or 0.0
  ctx.emit(Instruction::F64Eq);
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
///
/// Layout: [count:f64][struct_tag:f64][field0:f64]...
/// Field tags are compile-time known via ctx.struct_layouts.field_tags.
pub(super) fn emit_struct_contains(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&struct:contains? requires 2 args (struct_value, key_tag)")?;
  let record_ptr = emit_ptr_to_i32(ctx, &args[0])?;

  // Load struct_tag from struct_value at offset 8
  let struct_tag_local = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(record_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(struct_tag_local));

  // Evaluate key_tag argument
  let key_tag_local = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(key_tag_local));

  let mut struct_entries = ctx
    .struct_layouts
    .field_tags
    .iter()
    .map(|(tag, fields)| (*tag, fields.clone()))
    .collect::<Vec<_>>();
  struct_entries.sort_by_key(|(tag, _)| *tag);

  // For each known struct type: if struct_tag matches, scan field tags for key_tag
  for (struct_tag_id, field_tag_ids) in &struct_entries {
    ctx.emit(Instruction::LocalGet(struct_tag_local));
    ctx.emit(f64_const(*struct_tag_id as f64));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));

    // Nested if-chain: return 1.0 if key_tag matches any field tag, else 0.0
    for field_tag_id in field_tag_ids {
      ctx.emit(Instruction::LocalGet(key_tag_local));
      ctx.emit(f64_const(*field_tag_id as f64));
      ctx.emit(Instruction::F64Eq);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      ctx.emit(f64_const(1.0)); // field found
      ctx.emit(Instruction::Else);
    }
    ctx.emit(f64_const(0.0)); // field not found
    for _ in field_tag_ids {
      ctx.emit(Instruction::End);
    }

    ctx.emit(Instruction::Else);
  }

  ctx.emit(f64_const(0.0)); // unknown struct type
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
/// Memory layout: [count: f64] [tag_id: f64] [payload_0: f64] [payload_1: f64] ...
/// count = number of payloads (excludes the tag itself).
/// Returns the pointer as f64.
pub(super) fn emit_enum_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    return Err(":: requires at least a tag argument".into());
  }

  // First arg is the tag — accept Calcit::Tag or Calcit::Bool (used by foldl-shortcut pattern)
  let tag_f64 = match &args[0] {
    Calcit::Tag(t) => {
      let tag_str = t.to_string();
      let tag_id = *ctx
        .tag_index
        .get(&tag_str)
        .ok_or_else(|| format!("unknown tag in enum constructor: {tag_str}"))?;
      tag_id as f64
    }
    Calcit::Bool(b) => {
      if *b {
        1.0
      } else {
        0.0
      }
    }
    other => return Err(format!("::: expected tag as first arg, got: {other}")),
  };

  let payload = &args[1..];
  // Layout: count + tag + payloads
  let total_size = ((2 + payload.len()) * 8) as i32;

  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_size, ptr_local, "enum");

  // Store count at offset 0
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(payload.len() as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // Store tag at offset 8
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(tag_f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  // Store payload fields starting at offset 16
  for (i, val) in payload.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(ptr_local));
    emit_expr(ctx, val)?;
    ctx.emit(Instruction::F64Store(mem_arg_f64(((2 + i) * 8) as u64)));
  }

  // Return pointer as f64
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Emit `%:: enum_class tag payload...` — enum variant constructor.
///
/// Unlike `::` (NativeEnum), `%::` carries an enum class as first arg which is
/// ignored in WASM (used for type-checking only). Layout is identical to `::`.
/// args: [enum_class, tag, payload...]
pub(super) fn emit_named_enum_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() < 2 {
    return Err("%:: requires at least (enum_class tag) arguments".into());
  }

  // args[0] is enum_class — ignored in WASM (type info only)
  // args[1] is the variant tag
  let tag_f64 = match &args[1] {
    Calcit::Tag(t) => {
      let tag_str = t.to_string();
      let tag_id = *ctx
        .tag_index
        .get(&tag_str)
        .ok_or_else(|| format!("unknown tag in enum constructor: {tag_str}"))?;
      tag_id as f64
    }
    other => return Err(format!("%:: expected tag as second arg, got: {other}")),
  };

  let payload = &args[2..];
  let total_size = ((2 + payload.len()) * 8) as i32;

  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_size, ptr_local, "enum");

  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(payload.len() as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(tag_f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  for (i, val) in payload.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(ptr_local));
    emit_expr(ctx, val)?;
    ctx.emit(Instruction::F64Store(mem_arg_f64(((2 + i) * 8) as u64)));
  }

  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Emit `&enum:nth enum_value idx` — O(1) payload access by index.
///
/// Enum value layout: [count:f64][tag:f64][payload0:f64]...
/// idx 0 returns tag, idx 1+ returns payloads.
/// Offset = (1 + idx) * 8  (skip count slot).
pub(super) fn emit_enum_nth(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&enum:nth requires 2 args (enum_value, index)")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let idx_local = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx_local));

  ctx.emit(Instruction::LocalGet(idx_local));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  Ok(())
}

/// Emit `&enum:count enum_value` — matches interpreter semantics: payload count + 1 (includes tag).
///
/// Enum value layout: [count:f64][tag:f64][payload0:f64]...
/// Stored count at offset 0 is the raw payload count; the interpreter returns `extra.len() + 1`.
/// The public contract counts the tag as the first enum element.
pub(super) fn emit_enum_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&enum:count expects 1 arg")?;
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  // Add 1 to match interpreter: interpreter returns extra.len() + 1 (tag counts as element)
  ctx.emit(Instruction::F64Const(Ieee64::from(1.0f64)));
  ctx.emit(Instruction::F64Add);
  Ok(())
}

pub(super) fn emit_enum_assoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&enum:assoc expects 3 args")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let idx = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx));
  let val = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(val));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let size = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(total_slots));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalSet(size));

  let dst = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc_dynamic(ctx, size, dst, "enum");

  let dst_base = emit_addr_offset(ctx, dst, 0);
  let src_base = emit_addr_offset(ctx, src, 0);
  emit_copy_f64_loop(ctx, dst_base, src_base, total_slots);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(val));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}
