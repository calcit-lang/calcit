//! Static WASM lowering for object-safe `(Dyn Trait)` fat pointers and `BrTable` vtable dispatch (#2941).
//!
//! Fat pointer payload layout (`"dyn-trait"`, 16 bytes after 8-byte `[magic][tag]` header):
//! - offset `0` (`i32`): `vtable_id` — dense index `0..K-1` within `trait_id`'s closed impl set
//! - offset `4` (`i32`): `trait_id` — dense index `0..T-1` of the boxed `CalcitTrait`
//! - offset `8` (`f64`): `data` — boxed Calcit value in canonical f64 slot representation
//!
//! Every reachable `(boxed_trait, target_trait, method, arity)` dispatch compiles to ONE shared
//! trampoline `__dyn_dispatch_<boxed>_<target>_<method>_<arity>` that switches on `vtable_id`
//! via `Instruction::BrTable` and lowers directly to `Instruction::Call` / inlined impls.
//! Zero `CallIndirect`, zero extra funcref table slots.

use super::*;
use crate::wasm::host::calcit::{CalcitNumericRefinement, CalcitSymbolInfo, CalcitTrait};
use crate::wasm::host::program;
use crate::wasm::host::runner::preprocess::check_trait_object_safe;

/// Heap tag name registered in `BUILTIN_TYPE_TAGS` for boxed `(Dyn Trait)` fat pointers.
pub(super) const DYN_TRAIT_HEAP_TAG: &str = "dyn-trait";

/// Concrete vtable entry captured at `(dyn:box expr Trait)` sites for a boxed trait.
#[derive(Clone, Debug)]
struct DynVtableEntry {
  concrete_type: Arc<CalcitTypeAnnotation>,
  file_ns: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DynDispatchKey {
  boxed_trait_id: u32,
  target_trait_id: u32,
  method_name: String,
  arity: u32,
}

#[derive(Clone, Debug)]
enum DynTrampolineSpec {
  Dispatch {
    boxed_trait_id: u32,
    boxed_trait: Arc<CalcitTrait>,
    target_trait: Arc<CalcitTrait>,
    method_name: String,
    arity: u32,
  },
  ToStr,
}

/// Compile-time registry of reachable `(Dyn Trait)` traits, concrete vtables, and dispatch trampolines.
#[derive(Clone, Default)]
pub(super) struct DynTraitRegistry {
  dyn_base_fn_idx: u32,
  traits: Vec<Arc<CalcitTrait>>,
  trait_id_by_key: HashMap<String, u32>,
  vtables_by_trait: Vec<Vec<DynVtableEntry>>,
  vtable_id_by_trait_and_type: Vec<HashMap<String, u32>>,
  trampolines: Vec<DynTrampolineSpec>,
  dispatch_slot_by_key: HashMap<DynDispatchKey, u32>,
  to_str_slot: Option<u32>,
}

impl DynTraitRegistry {
  pub(super) fn new(dyn_base_fn_idx: u32) -> Self {
    Self {
      dyn_base_fn_idx,
      ..Self::default()
    }
  }

  fn intern_trait(&mut self, trait_def: &Arc<CalcitTrait>) -> u32 {
    let key = trait_registry_key(trait_def.as_ref());
    if let Some(&id) = self.trait_id_by_key.get(&key) {
      return id;
    }
    let id = self.traits.len() as u32;
    self.traits.push(trait_def.clone());
    self.trait_id_by_key.insert(key, id);
    self.vtables_by_trait.push(Vec::new());
    self.vtable_id_by_trait_and_type.push(HashMap::new());
    id
  }

  fn intern_vtable(&mut self, trait_id: u32, concrete_key: String, concrete_type: Arc<CalcitTypeAnnotation>, file_ns: String) -> u32 {
    let trait_idx = trait_id as usize;
    if let Some(&vtable_id) = self.vtable_id_by_trait_and_type[trait_idx].get(&concrete_key) {
      return vtable_id;
    }
    let vtable_id = self.vtables_by_trait[trait_idx].len() as u32;
    self.vtables_by_trait[trait_idx].push(DynVtableEntry { concrete_type, file_ns });
    self.vtable_id_by_trait_and_type[trait_idx].insert(concrete_key, vtable_id);
    vtable_id
  }

  fn ensure_dispatch_trampoline(
    &mut self,
    boxed_trait: &Arc<CalcitTrait>,
    target_trait: &Arc<CalcitTrait>,
    method_name: &str,
    arity: u32,
  ) -> u32 {
    let boxed_trait_id = self.intern_trait(boxed_trait);
    let target_trait_id = self.intern_trait(target_trait);
    let key = DynDispatchKey {
      boxed_trait_id,
      target_trait_id,
      method_name: method_name.to_owned(),
      arity,
    };
    if let Some(&slot) = self.dispatch_slot_by_key.get(&key) {
      return self.dyn_base_fn_idx + slot;
    }
    let slot = self.trampolines.len() as u32;
    self.trampolines.push(DynTrampolineSpec::Dispatch {
      boxed_trait_id,
      boxed_trait: boxed_trait.clone(),
      target_trait: target_trait.clone(),
      method_name: method_name.to_owned(),
      arity,
    });
    self.dispatch_slot_by_key.insert(key, slot);
    self.dyn_base_fn_idx + slot
  }

  pub(super) fn ensure_to_str_trampoline(&mut self) -> u32 {
    if let Some(slot) = self.to_str_slot {
      return self.dyn_base_fn_idx + slot;
    }
    let slot = self.trampolines.len() as u32;
    self.trampolines.push(DynTrampolineSpec::ToStr);
    self.to_str_slot = Some(slot);
    self.dyn_base_fn_idx + slot
  }
}

fn trait_registry_key(trait_def: &CalcitTrait) -> String {
  if let Some(def_ref) = &trait_def.definition_ref {
    format!("def:{def_ref}")
  } else if let Some(runtime_id) = trait_def.runtime_id {
    format!("rt:{runtime_id}")
  } else {
    let methods = trait_def.methods.iter().map(|tag| tag.ref_str()).collect::<Vec<_>>().join(",");
    format!("anon:{}:{methods}", trait_def.name)
  }
}

/// Derives a canonical concrete type key for `(Dyn Trait)` vtable deduplication.
///
/// Isolated in one helper so `#2940` (`src/calcit/type_id.rs`) can replace the
/// key derivation cleanly upon rebase.
pub(super) fn dyn_concrete_type_key(ty: &Arc<CalcitTypeAnnotation>) -> Result<String, String> {
  let expanded = expand_wasm_value_type(ty);
  match expanded.as_ref() {
    CalcitTypeAnnotation::Number => Ok(String::from("number")),
    CalcitTypeAnnotation::Numeric(refinement) => Ok(format!(
      "numeric:{}",
      match refinement {
        CalcitNumericRefinement::Int8 => "i8",
        CalcitNumericRefinement::Int16 => "i16",
        CalcitNumericRefinement::Int32 => "i32",
        CalcitNumericRefinement::UInt8 => "u8",
        CalcitNumericRefinement::UInt16 => "u16",
        CalcitNumericRefinement::UInt32 => "u32",
        CalcitNumericRefinement::Int64 => "i64",
        CalcitNumericRefinement::UInt64 => "u64",
        CalcitNumericRefinement::Float32 => "f32",
        CalcitNumericRefinement::Float64 => "f64",
      }
    )),
    CalcitTypeAnnotation::Bool => Ok(String::from("bool")),
    CalcitTypeAnnotation::Tag => Ok(String::from("tag")),
    CalcitTypeAnnotation::Nil => Ok(String::from("nil")),
    CalcitTypeAnnotation::Unit => Ok(String::from("unit")),
    CalcitTypeAnnotation::String => Ok(String::from("string")),
    CalcitTypeAnnotation::Symbol => Ok(String::from("symbol")),
    CalcitTypeAnnotation::Buffer => Ok(String::from("buffer")),
    CalcitTypeAnnotation::BufList => Ok(String::from("buf-list")),
    CalcitTypeAnnotation::Never => Ok(String::from("never")),
    CalcitTypeAnnotation::AnonymousEnum => Ok(String::from("enum-anon")),
    CalcitTypeAnnotation::StructValue(def) => {
      let base = def.definition_ref.as_deref().unwrap_or_else(|| def.name.ref_str());
      Ok(format!("struct:{base}"))
    }
    CalcitTypeAnnotation::Struct(def, args) => {
      let base = def.definition_ref.as_deref().unwrap_or_else(|| def.name.ref_str());
      if args.is_empty() {
        Ok(format!("struct:{base}"))
      } else {
        let arg_keys = args.iter().map(dyn_concrete_type_key).collect::<Result<Vec<_>, _>>()?;
        Ok(format!("struct:{base}<{}>", arg_keys.join(",")))
      }
    }
    CalcitTypeAnnotation::EnumValue(def) => {
      let base = def.definition_ref().map_or_else(|| def.name().ref_str(), |v| v.as_ref());
      Ok(format!("enum:{base}"))
    }
    CalcitTypeAnnotation::Enum(def, args) => {
      let base = def.definition_ref().map_or_else(|| def.name().ref_str(), |v| v.as_ref());
      if args.is_empty() {
        Ok(format!("enum:{base}"))
      } else {
        let arg_keys = args.iter().map(dyn_concrete_type_key).collect::<Result<Vec<_>, _>>()?;
        Ok(format!("enum:{base}<{}>", arg_keys.join(",")))
      }
    }
    CalcitTypeAnnotation::List(inner) => Ok(format!("list<{}>", dyn_concrete_type_key(inner)?)),
    CalcitTypeAnnotation::Set(inner) => Ok(format!("set<{}>", dyn_concrete_type_key(inner)?)),
    CalcitTypeAnnotation::Ref(inner) => Ok(format!("ref<{}>", dyn_concrete_type_key(inner)?)),
    CalcitTypeAnnotation::Map(k, v) => Ok(format!("map<{},{}>", dyn_concrete_type_key(k)?, dyn_concrete_type_key(v)?)),
    CalcitTypeAnnotation::Tuple(slots) => {
      let slot_keys = slots.iter().map(dyn_concrete_type_key).collect::<Result<Vec<_>, _>>()?;
      Ok(format!("tuple<{}>", slot_keys.join(",")))
    }
    CalcitTypeAnnotation::DynTrait(tr) => {
      let base = tr.definition_ref.as_deref().unwrap_or_else(|| tr.name.ref_str());
      Ok(format!("dyn:{base}"))
    }
    CalcitTypeAnnotation::Nullable(inner, _) => Ok(format!("nullable<{}>", dyn_concrete_type_key(inner)?)),
    CalcitTypeAnnotation::Fn(_) => Ok(format!("fn:{}", expanded.to_brief_string())),
    CalcitTypeAnnotation::Custom(val) => Ok(format!("custom:{val}")),
    CalcitTypeAnnotation::TypeRef(name, args) => {
      if let Some(def) = expanded.resolve_to_struct() {
        let base = def.definition_ref.as_deref().unwrap_or_else(|| def.name.ref_str());
        if args.is_empty() {
          return Ok(format!("struct:{base}"));
        }
        let arg_keys = args.iter().map(dyn_concrete_type_key).collect::<Result<Vec<_>, _>>()?;
        return Ok(format!("struct:{base}<{}>", arg_keys.join(",")));
      }
      if let Some(def) = expanded.resolve_to_enum() {
        let base = def.definition_ref().map_or_else(|| def.name().ref_str(), |v| v.as_ref());
        if args.is_empty() {
          return Ok(format!("enum:{base}"));
        }
        let arg_keys = args.iter().map(dyn_concrete_type_key).collect::<Result<Vec<_>, _>>()?;
        return Ok(format!("enum:{base}<{}>", arg_keys.join(",")));
      }
      Err(format!("E_DYN_BOX_TYPE: unresolved type `{name}` cannot be boxed into `(Dyn ...)`"))
    }
    _ => Err(format!(
      "E_DYN_BOX_TYPE: open or non-concrete type `{}` cannot be boxed into `(Dyn ...)`",
      expanded.to_brief_string()
    )),
  }
}

pub(super) fn resolve_wasm_trait_def(expr: &Calcit, file_ns: &str) -> Result<Arc<CalcitTrait>, String> {
  match expr {
    Calcit::Trait(trait_def) => Ok(trait_def.clone()),
    Calcit::Import(import) => match program::lookup_runtime_ready(&import.ns, &import.def) {
      Some(Calcit::Trait(trait_def)) => {
        if trait_def.definition_ref.is_some() {
          Ok(trait_def)
        } else {
          Ok(Arc::new((*trait_def).clone().with_definition_ref(&import.ns, &import.def)))
        }
      }
      _ => Err(format!(
        "E_DYN_BOX_TYPE: expected a trait definition at `{}/{}`, got import",
        import.ns, import.def
      )),
    },
    Calcit::Symbol { sym, info, .. } => {
      let target_ns = if info.at_ns.is_empty() { file_ns } else { info.at_ns.as_ref() };
      match program::lookup_runtime_ready(target_ns, sym.as_ref()) {
        Some(Calcit::Trait(trait_def)) => {
          if trait_def.definition_ref.is_some() {
            Ok(trait_def)
          } else {
            Ok(Arc::new((*trait_def).clone().with_definition_ref(target_ns, sym.as_ref())))
          }
        }
        _ => Err(format!("E_DYN_BOX_TYPE: expected a trait definition, got `{sym}`")),
      }
    }
    Calcit::List(items) if items.len() == 2 && matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Quote, _))) => {
      resolve_wasm_trait_def(&items[1], file_ns)
    }
    other => Err(format!("E_DYN_BOX_TYPE: expected a trait definition, got `{other}`")),
  }
}

/// Emits `(dyn:box expr Trait)` as a heap-allocated 16-byte `"dyn-trait"` fat pointer.
pub(super) fn emit_dyn_box(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err(format!("E_DYN_BOX_TYPE: dyn:box expects 2 args, got {}", args.len()));
  }
  let file_ns = ctx
    .specialization_stack
    .last()
    .and_then(|qualified| qualified.rsplit_once('/').map(|(ns, _)| ns))
    .unwrap_or(&ctx.source_ns)
    .to_owned();
  let trait_def = resolve_wasm_trait_def(&args[1], &file_ns)?;
  check_trait_object_safe(trait_def.as_ref())?;

  let ev = infer_wasm_value_evidence(ctx, &args[0])
    .filter(|evidence| {
      let concrete = expand_wasm_value_type(evidence.scalar_origin.as_ref().unwrap_or(&evidence.annotation));
      !nil_type_evidence_is_open(&concrete) && !concrete.contains_type_var()
    })
    .ok_or_else(|| "E_WASM_TRAIT_TYPE_EVIDENCE: dyn:box requires a concrete non-open value type".to_owned())?;
  let concrete_ty = expand_wasm_value_type(ev.scalar_origin.as_ref().unwrap_or(&ev.annotation));
  if matches!(concrete_ty.as_ref(), CalcitTypeAnnotation::AnonymousEnum)
    || matches!(concrete_ty.as_ref(), CalcitTypeAnnotation::Custom(v) if matches!(v.as_ref(), Calcit::Tag(tag) if tag.ref_str() == "struct"))
  {
    return Err(
      "E_WASM_TRAIT_TYPE_EVIDENCE: dyn:box requires a concrete nominal type rather than an open record/enum wrapper".to_owned(),
    );
  }
  let type_key = dyn_concrete_type_key(&concrete_ty)?;

  // Evaluate payload into canonical f64 representation before registering/allocating.
  let src_repr = NativeValRepr::from_type_annotation(&concrete_ty);
  let data_local = ctx.alloc_local();
  emit_expr_as_repr(ctx, &args[0], src_repr)?;
  emit_box_repr_to_f64(ctx, src_repr);
  ctx.emit(Instruction::LocalSet(data_local));

  let (trait_id, vtable_id) = {
    let mut reg = ctx.dyn_traits.borrow_mut();
    let trait_id = reg.intern_trait(&trait_def);
    let vtable_id = reg.intern_vtable(trait_id, type_key, concrete_ty, file_ns);
    (trait_id, vtable_id)
  };

  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, 16, ptr_local, DYN_TRAIT_HEAP_TAG);

  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::I32Const(vtable_id as i32));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));

  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::I32Const(trait_id as i32));
  ctx.emit(Instruction::I32Store(mem_arg_i32(4)));

  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::LocalGet(data_local));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Emits a static `BrTable` trampoline call for `(&trait-call target_trait :method dyn_receiver args...)`.
pub(super) fn emit_dyn_trait_dispatch(
  ctx: &mut WasmGenCtx,
  boxed_trait: &Arc<CalcitTrait>,
  target_trait_expr: &Calcit,
  method_name: &str,
  call_args: &[Calcit],
) -> Result<(), String> {
  if call_args.is_empty() {
    return Err("E_WASM_TRAIT_CALL: dynamic trait dispatch requires at least the receiver argument".into());
  }
  let file_ns = ctx
    .specialization_stack
    .last()
    .and_then(|qualified| qualified.rsplit_once('/').map(|(ns, _)| ns))
    .unwrap_or(&ctx.source_ns)
    .to_owned();
  let target_trait = resolve_wasm_trait_def(target_trait_expr, &file_ns)?;
  let arity = call_args.len() as u32;
  let trampoline_fn_idx = ctx
    .dyn_traits
    .borrow_mut()
    .ensure_dispatch_trampoline(boxed_trait, &target_trait, method_name, arity);

  // Push receiver (`f64` fat pointer) and remaining args in canonical `f64` ABI.
  emit_expr(ctx, &call_args[0])?;
  for arg in &call_args[1..] {
    emit_expr_as_repr(ctx, arg, NativeValRepr::F64)?;
  }
  ctx.emit(Instruction::Call(trampoline_fn_idx));
  Ok(())
}

fn find_trait_method_signature(
  boxed_trait: &CalcitTrait,
  target_trait: &CalcitTrait,
  method_name: &str,
) -> Option<Arc<CalcitFnTypeAnnotation>> {
  let get_sig = |tr: &CalcitTrait| -> Option<Arc<CalcitFnTypeAnnotation>> {
    tr.method_index(method_name)
      .and_then(|idx| tr.method_types.get(idx))
      .and_then(|ty| ty.resolve_to_fn())
  };
  if let Some(sig) = get_sig(target_trait) {
    return Some(sig);
  }
  for tr in boxed_trait.normalized_reachable_traits().unwrap_or_default() {
    if let Some(sig) = get_sig(tr.as_ref()) {
      return Some(sig);
    }
  }
  None
}

fn is_scalar_origin_type(ty: &CalcitTypeAnnotation) -> bool {
  matches!(
    ty,
    CalcitTypeAnnotation::Number
      | CalcitTypeAnnotation::Bool
      | CalcitTypeAnnotation::Tag
      | CalcitTypeAnnotation::Nil
      | CalcitTypeAnnotation::Unit
      | CalcitTypeAnnotation::Numeric(_)
  )
}

fn compile_dispatch_trampoline(
  env: &WasmCompileEnv,
  trampoline_fn_idx: u32,
  boxed_trait_id: u32,
  boxed_trait: &Arc<CalcitTrait>,
  target_trait: &Arc<CalcitTrait>,
  method_name: &str,
  arity: u32,
) -> Result<(CompiledFn, usize), String> {
  let vtables = env.dyn_traits.borrow().vtables_by_trait[boxed_trait_id as usize].clone();
  let vtable_count = vtables.len();

  let mut ctx = WasmGenCtx::new(arity, env.clone());
  ctx.source_fn_index = Some(trampoline_fn_idx);

  let ptr_i32 = ctx.alloc_local_typed(ValType::I32);
  let vtable_id_local = ctx.alloc_local_typed(ValType::I32);
  let data_f64_local = ctx.alloc_local();

  ctx.emit(Instruction::LocalGet(0));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalTee(ptr_i32));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalSet(vtable_id_local));
  ctx.emit(Instruction::LocalGet(ptr_i32));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(data_f64_local));

  if vtable_count == 0 {
    ctx.emit(Instruction::Unreachable);
  } else {
    let method_sig = find_trait_method_signature(boxed_trait.as_ref(), target_trait.as_ref(), method_name);
    let target_trait_val = Calcit::Trait(target_trait.clone());

    ctx.emit(Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64))); // $done
    ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty)); // $trap
    for _ in 0..vtable_count {
      ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty)); // $arm_i
    }
    let targets: Vec<u32> = (0..vtable_count as u32).collect();
    ctx.emit(Instruction::LocalGet(vtable_id_local));
    ctx.emit(Instruction::BrTable(targets.into(), vtable_count as u32));

    for (arm_idx, vtable) in vtables.iter().enumerate() {
      ctx.emit(Instruction::End); // close $arm_{arm_idx}
      ctx.source_ns.clone_from(&vtable.file_ns);
      ctx.locals.clear();
      ctx.local_types.clear();
      ctx.local_reprs.clear();
      ctx.lambda_locals.clear();
      ctx.impl_selectors.clear();

      let self_sym = "__dyn_self";
      ctx.locals.insert(self_sym.to_owned(), data_f64_local);
      ctx.local_reprs.insert(data_f64_local, NativeValRepr::F64);
      let scalar_origin = if is_scalar_origin_type(vtable.concrete_type.as_ref()) {
        Some(vtable.concrete_type.clone())
      } else {
        None
      };
      ctx.local_types.insert(
        self_sym.to_owned(),
        WasmTypeEvidence {
          annotation: vtable.concrete_type.clone(),
          codec_hint: None,
          set_member_types: None,
          list_element_types: None,
          scalar_origin,
          exact_impls: true,
        },
      );

      let mut synthetic_args = Vec::with_capacity(arity as usize);
      synthetic_args.push(Calcit::Local(CalcitLocal {
        idx: data_f64_local as u16,
        sym: Arc::from(self_sym),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from(vtable.file_ns.as_str()),
          at_def: Arc::from("__dyn_trampoline"),
        }),
        location: None,
        type_info: vtable.concrete_type.clone(),
      }));

      for arg_idx in 1..arity {
        let arg_sym = format!("__dyn_arg_{arg_idx}");
        let arg_type = method_sig
          .as_ref()
          .and_then(|sig| sig.arg_types.get(arg_idx as usize).cloned())
          .unwrap_or_else(|| DYNAMIC_TYPE.clone());
        ctx.locals.insert(arg_sym.clone(), arg_idx);
        ctx.local_reprs.insert(arg_idx, NativeValRepr::F64);
        ctx
          .local_types
          .insert(arg_sym.clone(), WasmTypeEvidence::contract(arg_type.clone()));
        synthetic_args.push(Calcit::Local(CalcitLocal {
          idx: arg_idx as u16,
          sym: Arc::from(arg_sym),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from(vtable.file_ns.as_str()),
            at_def: Arc::from("__dyn_trampoline"),
          }),
          location: None,
          type_info: arg_type,
        }));
      }

      let callable = resolve_static_trait_callable(&target_trait_val, method_name, vtable.concrete_type.as_ref(), &vtable.file_ns)
        .map_err(|error| format!("E_WASM_TRAIT_RESOLUTION: {error}"))?;
      emit_resolved_method_call(&mut ctx, callable, &synthetic_args)?;
      ctx.emit(Instruction::Br((vtable_count - arm_idx) as u32));
    }

    ctx.emit(Instruction::End); // close $trap
    ctx.emit(Instruction::Unreachable);
    ctx.emit(Instruction::End); // close $done
  }

  Ok((
    CompiledFn {
      export_name: None,
      params: vec![ValType::F64; arity as usize],
      results: vec![ValType::F64],
      locals: std::mem::take(&mut ctx.extra_locals),
      instructions: std::mem::take(&mut ctx.instructions),
    },
    vtable_count,
  ))
}

fn compile_to_str_trampoline(env: &WasmCompileEnv) -> Result<(CompiledFn, usize), String> {
  let traits = env.dyn_traits.borrow().traits.clone();
  let trait_count = traits.len();
  let mut instructions = Vec::new();
  if trait_count == 0 {
    instructions.push(Instruction::Unreachable);
  } else {
    instructions.push(Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64))); // $done
    instructions.push(Instruction::Block(wasm_encoder::BlockType::Empty)); // $trap
    for _ in 0..trait_count {
      instructions.push(Instruction::Block(wasm_encoder::BlockType::Empty)); // $arm_i
    }
    let targets: Vec<u32> = (0..trait_count as u32).collect();
    instructions.push(Instruction::LocalGet(0));
    instructions.push(Instruction::BrTable(targets.into(), trait_count as u32));
    for (arm_idx, trait_def) in traits.iter().enumerate() {
      instructions.push(Instruction::End);
      let text = format!("(&dyn {})", trait_def.name);
      let ptr = *env
        .string_pool
        .get(&text)
        .ok_or_else(|| format!("missing WASM dyn-trait string literal: {text}"))?;
      instructions.push(f64_const(ptr as f64));
      instructions.push(Instruction::Br((trait_count - arm_idx) as u32));
    }
    instructions.push(Instruction::End); // $trap
    instructions.push(Instruction::Unreachable);
    instructions.push(Instruction::End); // $done
  }
  Ok((
    CompiledFn {
      export_name: None,
      params: vec![ValType::I32],
      results: vec![ValType::F64],
      locals: Vec::new(),
      instructions,
    },
    trait_count,
  ))
}

type CompiledTrampolineSlot = Option<Result<CompiledFn, (String, String, u32)>>;

/// Compiles all reachable `(Dyn Trait)` `BrTable` trampolines to a fixed point and
/// appends them in deterministic slot order to `compiled_fns`.
pub(super) fn compile_dyn_trampolines(
  env: &WasmCompileEnv,
  compiled_fns: &mut Vec<CompiledFn>,
  unsupported_dependencies: &mut HashMap<u32, (String, String)>,
) -> Result<(), String> {
  let base_fn_idx = env.dyn_traits.borrow().dyn_base_fn_idx;
  let mut compiled_slots: Vec<CompiledTrampolineSlot> = Vec::new();
  let mut compiled_counts: Vec<usize> = Vec::new();

  loop {
    let specs = env.dyn_traits.borrow().trampolines.clone();
    if compiled_slots.len() < specs.len() {
      compiled_slots.resize_with(specs.len(), || None);
      compiled_counts.resize(specs.len(), 0);
    }

    let mut changed = false;
    for (slot, spec) in specs.iter().enumerate() {
      let current_count = match spec {
        DynTrampolineSpec::Dispatch { boxed_trait_id, .. } => env.dyn_traits.borrow().vtables_by_trait[*boxed_trait_id as usize].len(),
        DynTrampolineSpec::ToStr => env.dyn_traits.borrow().traits.len(),
      };
      if compiled_slots[slot].is_some() && compiled_counts[slot] == current_count {
        continue;
      }
      changed = true;
      let trampoline_fn_idx = base_fn_idx + slot as u32;
      let compiled = match spec {
        DynTrampolineSpec::Dispatch {
          boxed_trait_id,
          boxed_trait,
          target_trait,
          method_name,
          arity,
        } => {
          let label = format!("__dyn_dispatch/{}/{}:{}", boxed_trait.name, target_trait.name, method_name);
          match compile_dispatch_trampoline(
            env,
            trampoline_fn_idx,
            *boxed_trait_id,
            boxed_trait,
            target_trait,
            method_name,
            *arity,
          ) {
            Ok((func, count)) => {
              compiled_counts[slot] = count;
              Ok(func)
            }
            Err(err) => Err((label, err, *arity)),
          }
        }
        DynTrampolineSpec::ToStr => match compile_to_str_trampoline(env) {
          Ok((func, count)) => {
            compiled_counts[slot] = count;
            Ok(func)
          }
          Err(err) => Err((String::from("__dyn_trait_to_str"), err, 1)),
        },
      };
      compiled_slots[slot] = Some(compiled);
    }

    if !changed && compiled_slots.len() == env.dyn_traits.borrow().trampolines.len() {
      break;
    }
  }

  for (slot, entry) in compiled_slots.into_iter().enumerate() {
    let fn_idx = base_fn_idx + slot as u32;
    match entry.expect("trampoline slot must be compiled") {
      Ok(func) => compiled_fns.push(func),
      Err((label, err, arity)) => {
        let deferred_wasi_capability = env.target == WasmTarget::Wasi
          && env.boundary == WasmBoundary::Component
          && (err.starts_with("E_WASI_COMMAND_CAPABILITY:") || err.starts_with("E_WASI_HTTP_CAPABILITY:"));
        if !deferred_wasi_capability {
          return Err(err);
        }
        unsupported_dependencies.insert(fn_idx, (label, err));
        let params = if arity == 1 && matches!(env.dyn_traits.borrow().trampolines[slot], DynTrampolineSpec::ToStr) {
          vec![ValType::I32]
        } else {
          vec![ValType::F64; arity as usize]
        };
        compiled_fns.push(CompiledFn {
          export_name: None,
          params,
          results: vec![ValType::F64],
          locals: Vec::new(),
          instructions: vec![Instruction::Unreachable],
        });
      }
    }
  }

  Ok(())
}
