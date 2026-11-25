use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::*;

/// Whole-program deterministic canonical `TypeId` table for WASM/WASI.
///
/// ID `0` is reserved for targets that are never boxed by `any:box` anywhere in
/// the reachable program; boxed canonical `type_key`s receive dense sorted IDs `1..=N`.
#[derive(Clone, Debug, Default)]
pub(super) struct AnyTypeTable {
  keys_to_id: BTreeMap<String, u32>,
}

impl AnyTypeTable {
  pub(super) fn id_for_key(&self, key: &str) -> u32 {
    self.keys_to_id.get(key).copied().unwrap_or(0)
  }
}

fn collect_boxed_any_keys_in_expr(expr: &Calcit, keys: &mut BTreeSet<String>) {
  match expr {
    Calcit::List(items) => {
      if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::AnyBox, _)))
        && let Some(type_id) = items
          .get(2)
          .and_then(crate::wasm::host::calcit::CompiledAnyTypeId::from_calcit_handle)
      {
        keys.insert(type_id.type_key.to_string());
      }
      for item in items.iter() {
        collect_boxed_any_keys_in_expr(item, keys);
      }
    }
    Calcit::Fn { info, .. } => {
      for item in info.body.iter() {
        collect_boxed_any_keys_in_expr(item, keys);
      }
    }
    Calcit::Thunk(thunk) => collect_boxed_any_keys_in_expr(thunk.get_code(), keys),
    Calcit::Map(map) => {
      for (k, v) in map.iter() {
        collect_boxed_any_keys_in_expr(k, keys);
        collect_boxed_any_keys_in_expr(v, keys);
      }
    }
    Calcit::Set(set) => {
      for item in set.iter() {
        collect_boxed_any_keys_in_expr(item, keys);
      }
    }
    Calcit::Recur(args) => {
      for item in args {
        collect_boxed_any_keys_in_expr(item, keys);
      }
    }
    _ => {}
  }
}

pub(super) fn collect_any_type_table(
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  value_imports: &HashMap<String, Calcit>,
  extra_exprs: &[Calcit],
) -> AnyTypeTable {
  let mut boxed_keys = BTreeSet::new();
  for (_, _, _, body) in fn_defs {
    for expr in body {
      collect_boxed_any_keys_in_expr(expr, &mut boxed_keys);
    }
  }
  for expr in value_imports.values().chain(extra_exprs.iter()) {
    collect_boxed_any_keys_in_expr(expr, &mut boxed_keys);
  }
  let mut keys_to_id = BTreeMap::new();
  for (idx, key) in boxed_keys.into_iter().enumerate() {
    keys_to_id.insert(key, (idx + 1) as u32);
  }
  AnyTypeTable { keys_to_id }
}

/// Whether the boxed canonical `type_key` has a scalar `f64` runtime payload.
/// Known scalar payloads (`flags = 0`) never enter `is_heap_ptr` checks or GC tracing,
/// preventing numeric scalars that numerically alias heap addresses from corruption.
/// Deriving this from `type_id.type_key` keeps `flags` consistent with `preprocess_any_box`
/// and `__rt_value_hash` even when local WASM expression inference returns `None`.
fn type_key_is_scalar_payload(type_key: &str) -> bool {
  matches!(type_key, "nil" | "unit" | "bool" | "number" | "tag") || type_key.starts_with("numeric:")
}

pub(super) fn emit_any_box(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  let Some(value_expr) = args.first() else {
    return Err(String::from("any:box expected 1 value argument"));
  };
  let Some(type_id) = args
    .get(1)
    .and_then(crate::wasm::host::calcit::CompiledAnyTypeId::from_calcit_handle)
  else {
    return Err(String::from("any:box requires preprocessed canonical TypeId handle"));
  };
  let compact_id = ctx.any_type_table.id_for_key(type_id.type_key.as_ref());
  if compact_id == 0 {
    return Err(format!(
      "E_WASM_ANY_TYPE_ID: missing WASM TypeId registration for `{}`",
      type_id.type_key
    ));
  }
  let flags = if type_key_is_scalar_payload(type_id.type_key.as_ref()) {
    0
  } else {
    1
  };

  emit_expr(ctx, value_expr)?;
  let val_local = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(val_local));

  let ptr = ctx.alloc_local_typed(ValType::I32);
  heap::emit_bump_alloc(ctx, 16, ptr, "any-box");
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(compact_id as i32));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(flags));
  ctx.emit(Instruction::I32Store(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(val_local));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
  ctx.ptr_to_f64(ptr);
  Ok(())
}

pub(super) fn emit_any_downcast(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 && args.len() != 3 {
    return Err(format!("any:downcast expected box and target type, got {} arg(s)", args.len()));
  }
  let type_key = if let Some(type_id) = args
    .get(2)
    .and_then(crate::wasm::host::calcit::CompiledAnyTypeId::from_calcit_handle)
  {
    type_id.type_key.to_string()
  } else {
    let target = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(&args[1], &[]);
    crate::wasm::host::calcit::canonical_type_key(target.as_ref(), Some(&ctx.source_ns)).map_err(|e| e.to_string())?
  };
  let target_id = ctx.any_type_table.id_for_key(&type_key);

  let box_ptr = ctx.alloc_local_typed(ValType::I32);
  emit_expr_as_repr(ctx, &args[0], super::NativeValRepr::HeapPtrI32)?;
  ctx.emit(Instruction::LocalSet(box_ptr));

  if target_id == 0 {
    heap::emit_option_enum(ctx, None)?;
    return Ok(());
  }

  ctx.emit(Instruction::LocalGet(box_ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(target_id as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  let payload = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(box_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(payload));
  heap::emit_option_enum(ctx, Some(payload))?;
  ctx.emit(Instruction::Else);
  heap::emit_option_enum(ctx, None)?;
  ctx.emit(Instruction::End);
  Ok(())
}
