//! Minimal WASM codegen for Calcit — generates binary `.wasm` via `wasm-encoder`.
//!
//! Supports a small subset of Calcit for demonstration purposes:
//! - `defn` with fixed-arity arguments (all f64)
//! - `let` bindings
//! - `if` conditionals
//! - Arithmetic: `&+`, `&-`, `&*`, `&/`, `&number:rem`
//! - Comparisons: `&<`, `&>`, `&=`
//! - `recur` (tail recursion via WASM loop)
//! - Number literals, Bool literals, Nil/Unit (→ 0.0)
//! - Tag values (compiled to f64 integer constants)
//! - Struct creation (`&%{}`) and field access (`&struct:nth`, `&struct:get`)
//! - Tuple creation (`::`) and field access (`&enum:nth`)
//!
//! All values are represented as f64 (matching Calcit's single numeric type).
//! Booleans: true → 1.0, false/nil → 0.0.
//! Tags: mapped to positive f64 integers at compile time.
//! Struct/Enum pointers: i32 offsets into linear memory, converted to/from f64.
//! Output is a `.wasm` binary that can be loaded by Node.js, Deno, or any WASM runtime.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use wasm_encoder::{
  CodeSection, ConstExpr, ElementSection, Elements, ExportKind, ExportSection, Function, FunctionSection, GlobalSection, GlobalType,
  Ieee64, Instruction, MemorySection, MemoryType, Module, RefType, TableSection, TableType, TypeSection, ValType,
};

use crate::builtins::syntax::get_raw_args_fn;
use crate::calcit::data_shape::{DataShapeGraph, DataShapeNode};
use crate::calcit::{
  Calcit, CalcitArgLabel, CalcitEnumDef, CalcitFnArgs, CalcitFnTypeAnnotation, CalcitImport, CalcitLocal, CalcitNumericRefinement,
  CalcitProc, CalcitStructDef, CalcitSyntax, CalcitTypeAnnotation, MethodKind,
};
use crate::program;
use crate::runner::preprocess::infer_static_type_from_expr;

#[path = "emit_wasm/component.rs"]
mod component;
#[path = "emit_wasm/edn_parse.rs"]
mod edn_parse;
#[path = "emit_wasm/methods.rs"]
mod methods;
#[path = "emit_wasm/runtime.rs"]
mod runtime;
#[path = "emit_wasm/structs.rs"]
mod structs;

use component::{
  ComponentListCodec, ComponentStructCodec, ComponentValueCodecs, ComponentVariantCodec, build_component_drop_fn,
  build_component_enum_lift_fn, build_component_enum_lower_fn, build_component_list_lift_fn, build_component_list_lower_fn,
  build_component_post_return_fn, build_component_struct_lift_fn, build_component_struct_lower_fn, build_component_variant_lift_fn,
  build_component_variant_lower_fn, collect_component_compound_types, collect_component_owned_types, component_fields_layout,
  component_memory_layout, push_component_result_reclaim, push_load_value_flat, push_store_value_flat,
};
use methods::{emit_call_args, emit_method_invoke};
use runtime::{
  HostImport, ModuleFunctionLayout, build_runtime_fns, build_utf8_valid_fn, build_wasi_component_get_env_fn, build_wasi_get_args_fn,
  build_wasi_get_env_fn, build_wasi_open_path_fn, build_wasi_read_dir_fn, build_wasi_read_text_fn, build_wasi_wait_fn,
  build_wasi_write_all_fn, build_wasi_write_text_fn, build_wasm_module, core_host_import, host_imports_for_target,
};
use structs::{
  emit_enum_assoc, emit_enum_count, emit_enum_new, emit_enum_nth, emit_named_enum_new, emit_struct_contains, emit_struct_count,
  emit_struct_def, emit_struct_field_tag, emit_struct_get, emit_struct_get_name, emit_struct_matches, emit_struct_new, emit_struct_nth,
  emit_struct_to_map, resolve_struct_ref, try_parse_defrecord_form,
};

/// Base offset — reserve first 16 bytes for bookkeeping.
/// The actual heap start will be shifted when string literals occupy the
/// initial segment (see `build_string_pool`).
const HEAP_BASE: i32 = 16;
/// Global index for the heap pointer (bump allocator).
const HEAP_PTR_GLOBAL: u32 = 0;
/// Magic marker written at `raw_base` of every heap allocation. Used by
/// `type-of` to distinguish real pointers from raw f64 numbers that happen to
/// fall inside the heap address range. Value chosen to be unlikely to appear
/// as the low 32 bits of a typical integer f64 value.
const HEAP_MAGIC: i32 = 0xCA1C_17A9u32 as i32;

/// Convert f64 to wasm-encoder's Ieee64 representation.
fn f64_const(v: f64) -> Instruction<'static> {
  Instruction::F64Const(Ieee64::from(v))
}

/// MemArg for f64 load/store (8-byte aligned, memory 0).
fn mem_arg_f64(offset: u64) -> wasm_encoder::MemArg {
  wasm_encoder::MemArg {
    offset,
    align: 3, // log2(8) = 3
    memory_index: 0,
  }
}

/// MemArg for i64 load/store (8-byte aligned, memory 0).
fn mem_arg_i64(offset: u64) -> wasm_encoder::MemArg {
  wasm_encoder::MemArg {
    offset,
    align: 3,
    memory_index: 0,
  }
}

fn mem_arg_f32(offset: u64) -> wasm_encoder::MemArg {
  wasm_encoder::MemArg {
    offset,
    align: 2,
    memory_index: 0,
  }
}

fn mem_arg_i16(offset: u64) -> wasm_encoder::MemArg {
  wasm_encoder::MemArg {
    offset,
    align: 1,
    memory_index: 0,
  }
}

/// MemArg for i32 load/store (4-byte aligned, memory 0).
fn mem_arg_i32(offset: u64) -> wasm_encoder::MemArg {
  wasm_encoder::MemArg {
    offset,
    align: 2, // log2(4) = 2
    memory_index: 0,
  }
}

/// MemArg for single-byte load/store (1-byte aligned, memory 0).
fn mem_arg_byte(offset: u64) -> wasm_encoder::MemArg {
  wasm_encoder::MemArg {
    offset,
    align: 0, // 2^0 = 1 byte
    memory_index: 0,
  }
}

#[path = "emit_wasm/edn.rs"]
mod edn;
/// Emit a WASM binary module from the compiled program.
/// Processes functions from all namespaces in the program.
#[path = "emit_wasm/heap.rs"]
mod heap;
#[path = "emit_wasm/hof.rs"]
mod hof;
#[path = "emit_wasm/lists.rs"]
mod lists;
#[path = "emit_wasm/maps.rs"]
mod maps;
#[path = "emit_wasm/sets.rs"]
mod sets;
#[path = "emit_wasm/strings.rs"]
mod strings;

use edn::*;
#[allow(unused_imports)]
pub(super) use heap::*; // makes heap fns available to sibling submodules via `use super::*`
use hof::*;
use lists::*;
use maps::*;
use sets::*;
use strings::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WasmTarget {
  #[default]
  Core,
  Wasi,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WasmBoundary {
  #[default]
  Native,
  Component,
}

impl FromStr for WasmBoundary {
  type Err = String;

  fn from_str(value: &str) -> Result<Self, Self::Err> {
    match value {
      "native" => Ok(Self::Native),
      "component" => Ok(Self::Component),
      _ => Err(format!(
        "E_WASM_BOUNDARY: unknown WASM boundary `{value}`; expected `native` or `component`"
      )),
    }
  }
}

impl FromStr for WasmTarget {
  type Err = String;

  fn from_str(value: &str) -> Result<Self, Self::Err> {
    match value {
      "core" => Ok(Self::Core),
      "wasi" => Ok(Self::Wasi),
      _ => Err(format!("E_WASM_TARGET: unknown WASM target `{value}`; expected `core` or `wasi`")),
    }
  }
}

fn deterministic_namespace_order(program_data: &program::CompiledProgram, first: Option<&str>) -> Vec<String> {
  let mut namespaces = program_data.keys().map(|namespace| namespace.to_string()).collect::<Vec<_>>();
  namespaces.sort_unstable();
  if let Some(first) = first
    && let Ok(index) = namespaces.binary_search_by(|namespace| namespace.as_str().cmp(first))
  {
    let first = namespaces.remove(index);
    namespaces.insert(0, first);
  }
  namespaces
}

fn deterministic_definition_order(file: &program::CompiledFileData) -> Vec<(&Arc<str>, &program::CompiledDef)> {
  let mut definitions = file.defs.iter().collect::<Vec<_>>();
  definitions.sort_unstable_by_key(|(name, _)| *name);
  definitions
}

pub fn emit_wasm(init_ns: &str, init_def: &str, emit_path: &str, target: WasmTarget, boundary: WasmBoundary) -> Result<(), String> {
  emit_wasm_impl(init_ns, init_def, emit_path, target, boundary, true)
}

pub fn check_wasm_command_component(init_ns: &str, init_def: &str) -> Result<(), String> {
  emit_wasm_impl(init_ns, init_def, "", WasmTarget::Wasi, WasmBoundary::Component, false)
}

fn emit_wasm_impl(
  init_ns: &str,
  init_def: &str,
  emit_path: &str,
  target: WasmTarget,
  boundary: WasmBoundary,
  write_output: bool,
) -> Result<(), String> {
  let program_data = program::clone_compiled_program_snapshot()?;
  validate_wasm_target_in_program(&program_data, init_ns, init_def, target)?;
  if target == WasmTarget::Wasi && boundary == WasmBoundary::Component {
    validate_wasm_command_component_in_program(&program_data, init_ns, init_def)?;
  }

  // First pass: extract all function signatures from all namespaces
  let mut fn_defs: Vec<(String, String, CalcitFnArgs, Vec<Calcit>)> = Vec::new(); // (ns, def_name, args, body)

  // Collect init_ns first, then other namespaces (ordering for export clarity)
  let ns_order = deterministic_namespace_order(&program_data, Some(init_ns));

  for ns in &ns_order {
    let Some(file_info) = program_data.get(ns.as_str()) else {
      continue;
    };
    for (def_name, compiled) in deterministic_definition_order(file_info) {
      if compiled.kind != program::CompiledDefKind::Fn {
        continue;
      }
      if is_wasm_import_def(&compiled.preprocessed_code) {
        parse_wasm_import_def(&compiled.preprocessed_code).ok_or_else(|| {
          format!("[wasm] invalid import declaration {ns}/{def_name}: expected `defwasm-import name (args) |module |field`")
        })?;
        continue;
      }
      match extract_fn_parts(&compiled.preprocessed_code) {
        Ok((args, body)) => {
          fn_defs.push((ns.to_string(), def_name.to_string(), args, body));
        }
        Err(e) => {
          if must_reject_extraction_failure(init_ns, ns, &compiled.preprocessed_code) {
            return Err(format!("[wasm] target function {ns}/{def_name} is not compilable: {e}"));
          }
          if write_output {
            eprintln!("[wasm] omitting unsupported dependency {ns}/{def_name}: {e}");
          }
        }
      }
    }
  }

  if fn_defs.is_empty() {
    return Err(format!("namespace not found or no functions: {init_ns}"));
  }
  let wasi_command_stdio = target == WasmTarget::Wasi
    && boundary == WasmBoundary::Component
    && fn_defs
      .iter()
      .any(|(namespace, _, _, body)| namespace != "calcit.core" && body.iter().any(expr_uses_wasi_stdio));
  // Build the import table before assigning user function indices. Built-in imports
  // stay first so internal lowering keeps its stable indices; user declarations
  // append after them.
  // A Component contract must account for every host dependency. The legacy
  // core target keeps its broad host table, while the Component boundary only
  // imports explicitly declared functions through typed Canonical ABI shapes.
  let mut host_imports = if boundary == WasmBoundary::Component {
    Vec::new()
  } else {
    host_imports_for_target(target)
  };
  let mut component_import_adapters = if boundary == WasmBoundary::Component && target == WasmTarget::Core {
    collect_component_import_adapters(&program_data)?
  } else {
    Vec::new()
  };
  if boundary == WasmBoundary::Component && target == WasmTarget::Wasi {
    component_import_adapters.push(ComponentImportAdapter {
      definition: "calcit.core/get-args".into(),
      module: "wasi:cli/environment@0.3.1".into(),
      symbol: "get-arguments".into(),
      raw_index: 0,
      source_arity: 0,
      invocation: ComponentAbiInvocation::Sync,
      parameters: vec![],
      result: ComponentAbiType::List(Box::new(ComponentAbiType::String)),
    });
  }
  let mut component_adapters = if boundary == WasmBoundary::Component && target == WasmTarget::Core {
    let provisional_fn_index = fn_defs
      .iter()
      .map(|(namespace, name, _, _)| (format!("{namespace}/{name}"), 0))
      .collect::<HashMap<_, _>>();
    collect_component_export_adapters(&program_data, &fn_defs, &provisional_fn_index)?
  } else {
    Vec::new()
  };
  let mut wasm_import_names: HashMap<String, u32> = HashMap::new();
  let mut wasm_import_arities: HashMap<String, u32> = HashMap::new();
  let mut component_async_canonical_imports = None;
  let mut component_stackless_canonical_imports = None;
  if boundary == WasmBoundary::Component {
    if target == WasmTarget::Wasi {
      host_imports.push(HostImport {
        module: "wasi:cli/exit@0.3.1".into(),
        name: "exit-with-code".into(),
        params: vec![ValType::I32],
        results: vec![],
      });
    }
    for adapter in &mut component_import_adapters {
      let index = host_imports.len() as u32;
      adapter.raw_index = index;
      let (params, results) = component_import_signature(adapter);
      host_imports.push(HostImport {
        module: adapter.module.clone(),
        name: component_import_emitted_symbol(adapter),
        params,
        results,
      });
    }
    if target == WasmTarget::Wasi {
      host_imports.push(HostImport {
        module: "wasi:cli/environment@0.3.1".into(),
        name: "get-environment".into(),
        params: vec![ValType::I32],
        results: vec![],
      });
      if wasi_command_stdio {
        for interface in ["stdout", "stderr"] {
          let module = format!("wasi:cli/{interface}@0.3.1");
          for (name, params, results) in [
            ("[stream-new-0]write-via-stream", vec![], vec![ValType::I64]),
            ("[stream-write-0]write-via-stream", vec![ValType::I32; 3], vec![ValType::I32]),
            ("[stream-drop-writable-0]write-via-stream", vec![ValType::I32], vec![]),
            ("[future-drop-readable-1]write-via-stream", vec![ValType::I32], vec![]),
            ("write-via-stream", vec![ValType::I32], vec![ValType::I32]),
          ] {
            host_imports.push(HostImport {
              module: module.clone(),
              name: name.into(),
              params,
              results,
            });
          }
        }
        host_imports.push(HostImport {
          module: "[export]wasi:cli/run@0.3.1".into(),
          name: "[task-return]run".into(),
          params: vec![ValType::I32],
          results: vec![],
        });
      }
    }
    if component_import_adapters
      .iter()
      .any(|adapter| adapter.invocation == ComponentAbiInvocation::Async)
      || component_adapters.iter().any(|adapter| adapter.stream_consumer.is_some())
    {
      component_async_canonical_imports = Some(register_component_async_canonical_imports(&mut host_imports));
    }
    if component_adapters
      .iter()
      .any(|adapter| adapter.stackless_tail_import.is_some() || adapter.stream_consumer.is_some())
    {
      component_stackless_canonical_imports = Some(register_component_stackless_canonical_imports(&mut host_imports));
    }
    for adapter in &mut component_adapters {
      if let Some(consumer) = &mut adapter.stream_consumer {
        register_component_stream_canonical_imports(&mut host_imports, &adapter.symbol, consumer);
      }
    }
    for adapter in &mut component_adapters {
      if adapter.invocation != ComponentAbiInvocation::Async {
        continue;
      }
      let index = host_imports.len() as u32;
      adapter.task_return_index = Some(index);
      host_imports.push(HostImport {
        module: COMPONENT_ASYNC_EXPORT_IMPORT_MODULE.into(),
        name: component_task_return_symbol(&adapter.symbol),
        params: component_task_return_signature(&adapter.result),
        results: vec![],
      });
    }
  } else {
    for ns in &ns_order {
      let Some(file_info) = program_data.get(ns.as_str()) else {
        continue;
      };
      for (def_name, compiled) in deterministic_definition_order(file_info) {
        if !is_wasm_import_def(&compiled.preprocessed_code) {
          continue;
        }
        let (module, name, args) = parse_wasm_import_def(&compiled.preprocessed_code).ok_or_else(|| {
          format!("[wasm] invalid import declaration {ns}/{def_name}: expected `defwasm-import name (args) |module |field`")
        })?;
        let arity =
          wasm_import_arity(&args).map_err(|reason| format!("[wasm] invalid import declaration {ns}/{def_name}: {reason}"))?;
        let index = host_imports.len() as u32;
        host_imports.push(HostImport {
          module,
          name,
          params: vec![ValType::F64; arity as usize],
          results: vec![ValType::F64],
        });
        let qualified = format!("{ns}/{def_name}");
        wasm_import_names.insert(qualified.clone(), index);
        wasm_import_names.insert(def_name.to_string(), index);
        wasm_import_arities.insert(qualified, arity);
        wasm_import_arities.insert(def_name.to_string(), arity);
      }
    }
  }
  let num_imports = host_imports.len() as u32;

  // Collect tags early — needed to embed the string type tag in the __str_new helper.
  let tag_index = collect_all_tags_from(
    &fn_defs,
    if boundary == WasmBoundary::Component {
      Some(&program_data)
    } else {
      None
    },
  );
  if write_output {
    eprintln!("[wasm] tag index: {tag_index:?}");
  }

  let (mut compiled_fns, mut runtime_fn_index) = build_runtime_fns(
    num_imports,
    *tag_index.get("map").expect("map tag must exist") as i32,
    *tag_index.get("list").expect("list tag must exist") as i32,
    *tag_index.get("string").expect("string tag must exist") as i32,
    *tag_index.get("enum").expect("enum tag must exist") as i32,
    *tag_index.get("set").expect("set tag must exist") as i32,
  );
  let component_free_head_global = if boundary == WasmBoundary::Component {
    let atom_count = ns_order
      .iter()
      .filter_map(|ns| program_data.get(ns.as_str()))
      .flat_map(|file| file.defs.values())
      .filter(|compiled| {
        matches!(
          &compiled.preprocessed_code,
          Calcit::List(xs)
            if matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::Defatom, _)))
        )
      })
      .count() as u32;
    Some(2 + atom_count)
  } else {
    None
  };
  if target == WasmTarget::Wasi && boundary == WasmBoundary::Native {
    let fd_write_idx = *index_host_imports(&host_imports)
      .get(&("wasi_snapshot_preview1".into(), "fd_write".into()))
      .expect("WASI fd_write import must be registered");
    let wasi_write_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_write_all".into(), wasi_write_idx);
    compiled_fns.push(build_wasi_write_all_fn(fd_write_idx));
  }

  let (component_cabi_free_index, component_cabi_realloc_index) = if boundary == WasmBoundary::Component {
    let free_index = num_imports + compiled_fns.len() as u32;
    compiled_fns.push(build_cabi_free_fn(
      component_free_head_global.expect("Component boundary must reserve a free-list global"),
    ));
    let realloc_index = num_imports + compiled_fns.len() as u32;
    compiled_fns.push(build_cabi_realloc_fn(
      free_index,
      component_free_head_global.expect("Component boundary must reserve a free-list global"),
    ));
    (Some(free_index), Some(realloc_index))
  } else {
    (None, None)
  };

  // Emit __str_new(src_ptr: i32, byte_len: i32) → f64 now that we know the string tag id.
  // This is a runtime helper exported for JS FFI: copies bytes into a tagged heap string.
  let str_tag_id = *tag_index.get("string").expect("string tag must exist") as i32;
  let str_new_idx = num_imports + compiled_fns.len() as u32;
  runtime_fn_index.insert("__str_new".to_string(), str_new_idx);
  compiled_fns.push(build_str_new_fn(str_tag_id, component_cabi_realloc_index));

  if target == WasmTarget::Wasi && boundary == WasmBoundary::Component {
    let environment_idx = *index_host_imports(&host_imports)
      .get(&("wasi:cli/environment@0.3.1".into(), "get-environment".into()))
      .expect("WASI 0.3 environment import must be registered");
    let get_env_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_get_env".into(), get_env_idx);
    compiled_fns.push(build_wasi_component_get_env_fn(
      environment_idx,
      str_new_idx,
      component_cabi_realloc_index.expect("Component boundary must install cabi_realloc"),
      component_cabi_free_index.expect("Component boundary must install cabi_free"),
    ));
  }

  let component_buffer_new_index = if boundary == WasmBoundary::Component {
    let buffer_tag_id = *tag_index.get("buffer").expect("buffer tag must exist") as i32;
    let index = num_imports + compiled_fns.len() as u32;
    compiled_fns.push(build_component_buffer_new_fn(
      buffer_tag_id,
      component_cabi_realloc_index.expect("Component boundary must install cabi_realloc"),
    ));
    Some(index)
  } else {
    None
  };
  let mut component_list_codecs = BTreeMap::new();
  let mut component_struct_codecs = BTreeMap::new();
  let mut component_variant_codecs = BTreeMap::new();
  if let Some(cabi_realloc_index) = component_cabi_realloc_index {
    let buffer_new_index = component_buffer_new_index.expect("Component boundary must install the Buffer constructor");
    let list_tag_id = *tag_index.get("list").expect("list tag must exist") as i32;
    let enum_tag_id = *tag_index.get("enum").expect("enum tag must exist") as i32;
    for compound_type in collect_component_compound_types(&program_data, &fn_defs, &component_import_adapters)? {
      let lift_index = num_imports + compiled_fns.len() as u32;
      let lower_index = lift_index + 1;
      match &compound_type {
        ComponentAbiType::List(_) => {
          component_list_codecs.insert(compound_type.clone(), ComponentListCodec { lift_index, lower_index });
          let codecs = ComponentValueCodecs {
            str_new_index: str_new_idx,
            buffer_new_index,
            cabi_realloc_index,
            list_codecs: &component_list_codecs,
            struct_codecs: &component_struct_codecs,
            variant_codecs: &component_variant_codecs,
          };
          compiled_fns.push(build_component_list_lift_fn(
            &compound_type,
            list_tag_id,
            cabi_realloc_index,
            &codecs,
          ));
          compiled_fns.push(build_component_list_lower_fn(&compound_type, cabi_realloc_index, &codecs));
        }
        ComponentAbiType::Struct(record) => {
          let nominal_tag_id = *tag_index
            .get(record.tag.as_str())
            .ok_or_else(|| format!("E_COMPONENT_ABI_STRUCT_TAG: `{}` is missing from the WASM tag index", record.id))?
            as i32;
          component_struct_codecs.insert(compound_type.clone(), ComponentStructCodec { lift_index, lower_index });
          let codecs = ComponentValueCodecs {
            str_new_index: str_new_idx,
            buffer_new_index,
            cabi_realloc_index,
            list_codecs: &component_list_codecs,
            struct_codecs: &component_struct_codecs,
            variant_codecs: &component_variant_codecs,
          };
          compiled_fns.push(build_component_struct_lift_fn(
            &compound_type,
            nominal_tag_id,
            *tag_index.get("struct").expect("struct tag must exist") as i32,
            cabi_realloc_index,
            &codecs,
          ));
          compiled_fns.push(build_component_struct_lower_fn(&compound_type, nominal_tag_id, &codecs));
        }
        ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) => {
          let tag_ids = match &compound_type {
            ComponentAbiType::Option(_) => ["none", "some"],
            ComponentAbiType::Result(_, _) => ["ok", "err"],
            _ => unreachable!(),
          }
          .map(|tag| *tag_index.get(tag).unwrap_or_else(|| panic!("{tag} tag must exist")) as i32);
          component_variant_codecs.insert(compound_type.clone(), ComponentVariantCodec { lift_index, lower_index });
          let codecs = ComponentValueCodecs {
            str_new_index: str_new_idx,
            buffer_new_index,
            cabi_realloc_index,
            list_codecs: &component_list_codecs,
            struct_codecs: &component_struct_codecs,
            variant_codecs: &component_variant_codecs,
          };
          compiled_fns.push(build_component_variant_lift_fn(
            &compound_type,
            tag_ids,
            enum_tag_id,
            cabi_realloc_index,
            &codecs,
          ));
          compiled_fns.push(build_component_variant_lower_fn(&compound_type, tag_ids, &codecs));
        }
        ComponentAbiType::Enum(enum_type) => {
          let tag_ids = enum_type
            .variants
            .iter()
            .map(|variant| {
              tag_index.get(&variant.tag).copied().map(|value| value as i32).ok_or_else(|| {
                format!(
                  "E_COMPONENT_ABI_ENUM_TAG: `{}` variant `{}` is missing from the WASM tag index",
                  enum_type.id, variant.tag
                )
              })
            })
            .collect::<Result<Vec<_>, _>>()?;
          component_variant_codecs.insert(compound_type.clone(), ComponentVariantCodec { lift_index, lower_index });
          let codecs = ComponentValueCodecs {
            str_new_index: str_new_idx,
            buffer_new_index,
            cabi_realloc_index,
            list_codecs: &component_list_codecs,
            struct_codecs: &component_struct_codecs,
            variant_codecs: &component_variant_codecs,
          };
          compiled_fns.push(build_component_enum_lift_fn(
            &compound_type,
            &tag_ids,
            enum_tag_id,
            cabi_realloc_index,
            &codecs,
          ));
          compiled_fns.push(build_component_enum_lower_fn(&compound_type, &tag_ids, &codecs));
        }
        _ => unreachable!("compound type collector returned a scalar type"),
      }
    }
  }
  let mut component_drop_indices = BTreeMap::new();
  if let Some(cabi_free_index) = component_cabi_free_index {
    for owned_type in collect_component_owned_types(&component_adapters) {
      let index = num_imports + compiled_fns.len() as u32;
      component_drop_indices.insert(owned_type.clone(), index);
      compiled_fns.push(build_component_drop_fn(&owned_type, cabi_free_index, &component_drop_indices));
    }
  }
  if let Some(cabi_realloc_index) = component_cabi_realloc_index {
    let buffer_new_index = component_buffer_new_index.expect("Component boundary must install the Buffer constructor");
    let codecs = ComponentValueCodecs {
      str_new_index: str_new_idx,
      buffer_new_index,
      cabi_realloc_index,
      list_codecs: &component_list_codecs,
      struct_codecs: &component_struct_codecs,
      variant_codecs: &component_variant_codecs,
    };
    for adapter in &component_import_adapters {
      let index = num_imports + compiled_fns.len() as u32;
      let local_name = adapter
        .definition
        .rsplit_once('/')
        .map_or(adapter.definition.as_str(), |(_, name)| name);
      wasm_import_names.insert(adapter.definition.clone(), index);
      wasm_import_names.insert(local_name.to_string(), index);
      wasm_import_arities.insert(adapter.definition.clone(), adapter.source_arity);
      wasm_import_arities.insert(local_name.to_string(), adapter.source_arity);
      if target == WasmTarget::Wasi && adapter.definition == "calcit.core/get-args" {
        runtime_fn_index.insert("__rt_wasi_get_args".into(), index);
      }
      compiled_fns.push(build_component_import_adapter(
        adapter,
        component_async_canonical_imports.as_ref(),
        cabi_realloc_index,
        &codecs,
      ));
    }
  }

  if target == WasmTarget::Wasi && boundary == WasmBoundary::Native {
    let import_indices = index_host_imports(&host_imports);
    let wasi_import = |name: &str| {
      *import_indices
        .get(&("wasi_snapshot_preview1".into(), name.into()))
        .unwrap_or_else(|| panic!("WASI {name} import must be registered"))
    };
    let utf8_valid_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_utf8_valid".into(), utf8_valid_idx);
    compiled_fns.push(build_utf8_valid_fn());
    let wait_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_wait".into(), wait_idx);
    compiled_fns.push(build_wasi_wait_fn(wasi_import("poll_oneoff")));
    let open_path_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_open_path".into(), open_path_idx);
    compiled_fns.push(build_wasi_open_path_fn(
      wasi_import("fd_prestat_get"),
      wasi_import("fd_prestat_dir_name"),
      wasi_import("path_open"),
    ));
    let read_text_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_read_text".into(), read_text_idx);
    compiled_fns.push(build_wasi_read_text_fn(
      open_path_idx,
      wasi_import("fd_filestat_get"),
      wasi_import("fd_read"),
      wasi_import("fd_close"),
      utf8_valid_idx,
      str_tag_id,
    ));
    let read_dir_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_read_dir".into(), read_dir_idx);
    compiled_fns.push(build_wasi_read_dir_fn(
      open_path_idx,
      wasi_import("fd_readdir"),
      wasi_import("fd_close"),
      utf8_valid_idx,
      *runtime_fn_index
        .get("__rt_str_compare")
        .expect("string comparison helper must exist"),
      *tag_index.get("list").expect("list tag must exist") as i32,
      str_tag_id,
    ));
    let write_text_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_write_text".into(), write_text_idx);
    compiled_fns.push(build_wasi_write_text_fn(
      open_path_idx,
      wasi_import("fd_write"),
      wasi_import("fd_close"),
    ));
    let get_args_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_get_args".into(), get_args_idx);
    compiled_fns.push(build_wasi_get_args_fn(
      wasi_import("args_sizes_get"),
      wasi_import("args_get"),
      str_new_idx,
      *tag_index.get("list").expect("list tag must exist") as i32,
    ));
    let get_env_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_get_env".into(), get_env_idx);
    compiled_fns.push(build_wasi_get_env_fn(
      wasi_import("environ_sizes_get"),
      wasi_import("environ_get"),
      str_new_idx,
    ));
  }

  // Pad helpers (need str_tag_id for heap allocation).
  let str_pad_left_idx = num_imports + compiled_fns.len() as u32;
  runtime_fn_index.insert("__rt_str_pad_left".to_string(), str_pad_left_idx);
  compiled_fns.push(build_str_pad_left_fn(str_tag_id));

  let str_pad_right_idx = num_imports + compiled_fns.len() as u32;
  runtime_fn_index.insert("__rt_str_pad_right".to_string(), str_pad_right_idx);
  compiled_fns.push(build_str_pad_right_fn(str_tag_id));

  let runtime_fn_count = compiled_fns.len() as u32;
  let mut export_name_counts: HashMap<String, usize> = HashMap::new();
  for (_, name, _, _) in &fn_defs {
    *export_name_counts.entry(name.clone()).or_insert(0) += 1;
  }
  let mut fn_index: HashMap<String, u32> = HashMap::new();
  let mut fn_arity: HashMap<String, u32> = HashMap::new();
  // Track functions with rest args: value is the fixed-arity count (params before `&`).
  // WASM arity for such functions is `fixed_arity + 1` (the rest list pointer).
  let mut fn_has_rest: HashMap<String, u32> = HashMap::new();
  // fn_table_index: user calcit fn (index i in fn_defs) → table slot i.
  // Table slots are 0-based within user calcit functions only (not runtime helpers).
  let mut fn_table_index: HashMap<String, u32> = HashMap::new();
  for (name, index) in &wasm_import_names {
    fn_index.insert(name.clone(), *index);
    fn_arity.insert(
      name.clone(),
      *wasm_import_arities.get(name).expect("WASM import arity must be registered"),
    );
  }
  for (i, (ns, name, args, _)) in fn_defs.iter().enumerate() {
    let idx = num_imports + runtime_fn_count + i as u32;
    let qualified = format!("{ns}/{name}");
    fn_index.insert(qualified.clone(), idx);
    fn_index.insert(name.clone(), idx);
    let (arity, rest_fixed) = compute_fn_arity(args);
    fn_arity.insert(qualified.clone(), arity);
    fn_arity.insert(name.clone(), arity);
    if let Some(fixed) = rest_fixed {
      fn_has_rest.insert(qualified.clone(), fixed);
      fn_has_rest.insert(name.clone(), fixed);
    }
    // Table slot = position in fn_defs (0-based)
    fn_table_index.insert(qualified, i as u32);
    fn_table_index.insert(name.clone(), i as u32);
  }

  let struct_field_tags = collect_struct_field_tags_from_program(&program_data, &tag_index);

  let mut static_fn_defs: HashMap<String, Arc<StaticFnDef>> = HashMap::new();
  for (ns, name, args, body) in &fn_defs {
    let Some(signature) = program_data
      .get(ns.as_str())
      .and_then(|file| file.defs.get(name.as_str()))
      .and_then(|compiled| compiled.schema.resolve_to_fn())
    else {
      continue;
    };
    let callback_arities = signature
      .arg_types
      .iter()
      .enumerate()
      .filter_map(|(index, annotation)| {
        annotation
          .resolve_to_fn()
          .filter(|callback| callback.rest_type.is_none())
          .map(|callback| (index, callback.arg_types.len()))
      })
      .collect::<HashMap<_, _>>();
    let definition = Arc::new(StaticFnDef {
      params: fn_param_names(args),
      body: body.clone(),
      callback_arities,
      fixed_arity: matches!(args, CalcitFnArgs::Args(_)),
    });
    static_fn_defs.insert(format!("{ns}/{name}"), definition.clone());
    if export_name_counts.get(name).copied() == Some(1) {
      static_fn_defs.insert(name.clone(), definition);
    }
  }

  // Collect top-level value defs before building the string pool because
  // imported constants may be inlined later by emit_expr.
  let mut value_imports: HashMap<String, Calcit> = HashMap::new();
  for ns in &ns_order {
    let Some(file_info) = program_data.get(ns.as_str()) else {
      continue;
    };
    for (def_name, compiled) in deterministic_definition_order(file_info) {
      if matches!(compiled.kind, program::CompiledDefKind::Value | program::CompiledDefKind::LazyValue) {
        value_imports.insert(format!("{ns}/{def_name}"), compiled.preprocessed_code.to_owned());
      }
    }
  }

  // Build string literal pool: assigns each unique string a memory offset.
  let (string_pool, string_data_segment, heap_start) = build_string_pool(&fn_defs, &value_imports, &tag_index, target);

  // Scan for defatom definitions — each gets a mutable WASM global (f64).
  let mut atom_initial_values: Vec<f64> = Vec::new();
  let mut atom_globals: HashMap<String, u32> = HashMap::new();
  for ns in &ns_order {
    let Some(file_info) = program_data.get(ns.as_str()) else {
      continue;
    };
    for (def_name, compiled) in deterministic_definition_order(file_info) {
      let qualified = format!("{ns}/{def_name}");
      if let crate::calcit::Calcit::List(xs) = &compiled.preprocessed_code
        && matches!(
          xs.first(),
          Some(crate::calcit::Calcit::Syntax(crate::calcit::CalcitSyntax::Defatom, _))
        )
      {
        let global_idx = atom_initial_values.len() as u32;
        atom_globals.insert(qualified, global_idx);
        // Determine initial value from 3rd node (index 2)
        let init_val = match xs.get(2) {
          Some(crate::calcit::Calcit::Bool(true)) => 1.0,
          Some(crate::calcit::Calcit::Number(n)) => *n,
          _ => 0.0, // false / nil / complex init → default 0.0
        };
        atom_initial_values.push(init_val);
      }
    }
  }

  let env = WasmCompileEnv {
    fn_index,
    fn_arity,
    fn_has_rest,
    runtime_fn_index,
    tag_index,
    struct_field_tags,
    string_pool,
    atom_globals,
    value_imports,
    static_fn_defs,
    fn_table_index,
    host_imports: index_host_imports(&host_imports),
    target,
    boundary,
  };
  for adapter in &mut component_adapters {
    adapter.target_index = *env
      .fn_index
      .get(&adapter.definition)
      .ok_or_else(|| format!("E_COMPONENT_ABI_TARGET: compiled target `{}` is missing", adapter.definition))?;
    if let Some(consumer) = &mut adapter.stream_consumer {
      consumer.handler_index = *env.fn_index.get(&consumer.handler_definition).ok_or_else(|| {
        format!(
          "E_COMPONENT_STREAM_HANDLER: compiled handler `{}` for `{}` is missing",
          consumer.handler_definition, adapter.definition
        )
      })?;
    }
  }

  let mut unsupported_dependencies = HashMap::new();
  // Second pass: target failures reject the artifact. Dependency failures keep
  // a trapping slot so preassigned call and table indices remain stable.
  for (ns, def_name, args, body) in &fn_defs {
    let explicit_export = program_data
      .get(ns.as_str())
      .and_then(|file| file.defs.get(def_name.as_str()))
      .is_some_and(|compiled| is_wasm_export_def(&compiled.preprocessed_code));
    let export_name = if export_name_counts.get(def_name).copied().unwrap_or(0) > 1 {
      format!("{ns}/{def_name}")
    } else {
      def_name.clone()
    };
    let stream_export = boundary == WasmBoundary::Component
      && component_adapters
        .iter()
        .any(|adapter| adapter.definition == format!("{ns}/{def_name}") && adapter.stream_consumer.is_some());
    // Scoped stream exports are compiler forms. Their source body is validated above,
    // then replaced by the dedicated adapter so a raw stream handle can never enter
    // the ordinary Calcit numeric ABI.
    let result = if stream_export {
      let (arity, _) = compute_fn_arity(args);
      Ok(CompiledFn {
        export_name: None,
        params: vec![ValType::F64; arity as usize],
        results: vec![ValType::F64],
        locals: vec![],
        instructions: vec![Instruction::Unreachable],
      })
    } else {
      try_custom_def_impl(ns, def_name, &export_name, args, &env)
        .unwrap_or_else(|| compile_fn(def_name, &export_name, args, body, &env))
    };
    match result {
      Ok(mut func) => {
        // Ordinary `defn` values stay callable inside the module but are not part
        // of the public host ABI. Only explicit `defwasm-export` declarations
        // (and reserved runtime helpers emitted separately) reach the export
        // surface. Component adapters replace the explicit declaration itself.
        if !explicit_export {
          func.export_name = None;
        }
        if boundary == WasmBoundary::Component && explicit_export {
          func.export_name = None;
        }
        compiled_fns.push(func);
      }
      Err(e) => {
        if ns == init_ns || explicit_export {
          return Err(format!("[wasm] target function {ns}/{def_name} is not compilable: {e}"));
        }
        if target == WasmTarget::Wasi && boundary == WasmBoundary::Component {
          unsupported_dependencies.insert(num_imports + compiled_fns.len() as u32, (format!("{ns}/{def_name}"), e.clone()));
        }
        if write_output {
          eprintln!("[wasm] trapping unsupported dependency {ns}/{def_name}: {e}");
        }
        let (arity, _) = compute_fn_arity(args);
        compiled_fns.push(CompiledFn {
          export_name: None,
          params: vec![ValType::F64; arity as usize],
          results: vec![ValType::F64],
          locals: vec![],
          instructions: vec![Instruction::Unreachable],
        });
      }
    }
  }

  if boundary == WasmBoundary::Component {
    let cabi_realloc_index = component_cabi_realloc_index.expect("Component boundary must install cabi_realloc");
    let buffer_new_index = component_buffer_new_index.expect("Component boundary must install the Buffer constructor");
    let export_runtime = ComponentExportRuntime {
      codecs: ComponentValueCodecs {
        str_new_index: str_new_idx,
        buffer_new_index,
        cabi_realloc_index,
        list_codecs: &component_list_codecs,
        struct_codecs: &component_struct_codecs,
        variant_codecs: &component_variant_codecs,
      },
      cabi_free_index: component_cabi_free_index.expect("Component export adapter must install cabi_free"),
      drop_indices: &component_drop_indices,
    };
    for adapter in &component_adapters {
      if adapter.stream_consumer.is_some() {
        let step_index = num_imports + compiled_fns.len() as u32;
        let [step, entry, callback] = build_component_stream_export(
          adapter,
          component_async_canonical_imports
            .as_ref()
            .expect("stream export must register async lifecycle imports"),
          component_stackless_canonical_imports
            .as_ref()
            .expect("stream export must register callback lifecycle imports"),
          buffer_new_index,
          component_cabi_free_index.expect("stream export must install cabi_free"),
          cabi_realloc_index,
          step_index,
        );
        compiled_fns.push(step);
        compiled_fns.push(entry);
        compiled_fns.push(callback);
      } else if let Some(import_definition) = &adapter.stackless_tail_import {
        let imported = component_import_adapters
          .iter()
          .find(|imported| &imported.definition == import_definition)
          .expect("stackless tail import must reference a collected Component import");
        let [entry, callback] = build_component_stackless_tail_export(
          adapter,
          imported,
          component_async_canonical_imports
            .as_ref()
            .expect("stackless tail export must register async lifecycle imports"),
          component_stackless_canonical_imports
            .as_ref()
            .expect("stackless tail export must register callback lifecycle imports"),
          component_cabi_free_index.expect("stackless tail export must install cabi_free"),
          cabi_realloc_index,
          &component_drop_indices,
        );
        compiled_fns.push(entry);
        compiled_fns.push(callback);
      } else {
        compiled_fns.push(build_component_export_adapter(adapter, &export_runtime));
      }
      if component_export_needs_post_return(adapter) {
        compiled_fns.push(build_component_post_return_fn(
          adapter,
          component_cabi_free_index.expect("memory-owning Component exports must install cabi_free"),
          &component_drop_indices,
        ));
      }
    }
  }

  if target == WasmTarget::Wasi {
    let qualified_init = format!("{init_ns}/{init_def}");
    let init_index = env
      .fn_index
      .get(&qualified_init)
      .copied()
      .ok_or_else(|| format!("E_WASM_TARGET: command entry `{qualified_init}` was not compiled"))?;
    let init_arity = env.fn_arity.get(&qualified_init).copied().unwrap_or(0);
    if init_arity != 0 {
      return Err(format!(
        "E_WASM_TARGET: WASI command entry `{qualified_init}` must take no arguments, got {init_arity}"
      ));
    }
    if boundary == WasmBoundary::Component {
      reject_reachable_wasi_command_dependencies(&compiled_fns, num_imports, init_index, &unsupported_dependencies)?;
    }
    let component_command = boundary == WasmBoundary::Component;
    compiled_fns.push(CompiledFn {
      export_name: Some(if component_command && wasi_command_stdio {
        "[async-lift-stackful]wasi:cli/run@0.3.1#run".into()
      } else if component_command {
        "wasi:cli/run@0.3.1#run".into()
      } else {
        "_start".into()
      }),
      params: vec![],
      results: if component_command && !wasi_command_stdio {
        vec![ValType::I32]
      } else {
        vec![]
      },
      locals: vec![],
      instructions: if component_command && wasi_command_stdio {
        vec![
          Instruction::Call(init_index),
          Instruction::Drop,
          Instruction::I32Const(0),
          Instruction::Call(
            *index_host_imports(&host_imports)
              .get(&("[export]wasi:cli/run@0.3.1".into(), "[task-return]run".into()))
              .expect("stdio command must register task return"),
          ),
        ]
      } else if component_command {
        vec![Instruction::Call(init_index), Instruction::Drop, Instruction::I32Const(0)]
      } else {
        vec![Instruction::Call(init_index), Instruction::Drop]
      },
    });
  }

  if compiled_fns.is_empty() {
    return Err("no functions could be compiled to WASM".into());
  }

  // Build module using wasm-encoder
  let wasm_bytes = build_wasm_module(
    &compiled_fns,
    &host_imports,
    heap_start,
    &string_data_segment,
    &atom_initial_values,
    str_tag_id,
    ModuleFunctionLayout {
      runtime_fn_count,
      table_fn_count: fn_defs.len() as u32,
      component_free_head: component_free_head_global.is_some(),
    },
  )?;

  let wasm_bytes = if target == WasmTarget::Wasi && boundary == WasmBoundary::Component {
    calcit_bindgen::package_wasi_command(&wasm_bytes)?
  } else {
    wasm_bytes
  };
  if !write_output {
    return Ok(());
  }
  let out_path = Path::new(emit_path);
  if !out_path.exists() {
    fs::create_dir_all(out_path).map_err(|e| format!("failed to create dir: {e}"))?;
  }
  let wasm_file = out_path.join("program.wasm");
  fs::write(&wasm_file, &wasm_bytes).map_err(|e| format!("failed to write WASM: {e}"))?;
  println!("wrote WASM to: {}", wasm_file.display());

  Ok(())
}

pub fn validate_wasm_target(init_ns: &str, init_def: &str, target: WasmTarget) -> Result<(), String> {
  let program_data = program::clone_compiled_program_snapshot()?;
  validate_wasm_target_in_program(&program_data, init_ns, init_def, target)
}

pub fn validate_wasm_boundary(target: WasmTarget, boundary: WasmBoundary, init_ns: &str, init_def: &str) -> Result<(), String> {
  if target == WasmTarget::Wasi {
    if boundary == WasmBoundary::Component {
      let program_data = program::clone_compiled_program_snapshot()?;
      validate_wasm_command_component_in_program(&program_data, init_ns, init_def)?;
    }
    return Ok(());
  }
  if boundary == WasmBoundary::Native {
    return Ok(());
  }
  let program_data = program::clone_compiled_program_snapshot()?;
  let mut fn_defs = Vec::new();
  for namespace in deterministic_namespace_order(&program_data, None) {
    let file = program_data
      .get(namespace.as_str())
      .expect("ordered namespace must remain in the compiled program");
    for (name, compiled) in deterministic_definition_order(file) {
      if compiled.kind != program::CompiledDefKind::Fn || is_wasm_import_def(&compiled.preprocessed_code) {
        continue;
      }
      match extract_fn_parts(&compiled.preprocessed_code) {
        Ok((args, body)) => fn_defs.push((namespace.to_string(), name.to_string(), args, body)),
        Err(reason) if is_wasm_export_def(&compiled.preprocessed_code) => {
          return Err(format!("E_COMPONENT_ABI_TARGET: `{namespace}/{name}` is not compilable: {reason}"));
        }
        Err(_) => {}
      }
    }
  }
  let fn_index = fn_defs
    .iter()
    .enumerate()
    .map(|(index, (namespace, name, _, _))| (format!("{namespace}/{name}"), index as u32))
    .collect::<HashMap<_, _>>();
  collect_component_import_adapters(&program_data)?;
  collect_component_export_adapters(&program_data, &fn_defs, &fn_index)?;
  Ok(())
}

fn validate_wasm_target_in_program(
  program_data: &program::CompiledProgram,
  init_ns: &str,
  init_def: &str,
  target: WasmTarget,
) -> Result<(), String> {
  if target == WasmTarget::Core {
    return Ok(());
  }

  for ns in deterministic_namespace_order(program_data, None) {
    let file_info = program_data
      .get(ns.as_str())
      .expect("ordered namespace must remain in the compiled program");
    for (def_name, compiled) in deterministic_definition_order(file_info) {
      if compiled.kind != program::CompiledDefKind::Fn {
        continue;
      }
      if is_wasm_import_def(&compiled.preprocessed_code) {
        return Err(format!(
          "E_WASM_CAPABILITY: `{ns}/{def_name}` declares a custom WASM import; the `wasi` target only permits registered capabilities"
        ));
      }
      if def_name.as_ref() == "_start" {
        return Err("E_WASM_TARGET: `_start` is reserved for the generated WASI command entry".into());
      }
    }
  }

  let qualified_init = format!("{init_ns}/{init_def}");
  let compiled_init = program_data
    .get(init_ns)
    .and_then(|file| file.defs.get(init_def))
    .ok_or_else(|| format!("E_WASM_TARGET: command entry `{qualified_init}` was not compiled"))?;
  let (args, _) = extract_fn_parts(&compiled_init.preprocessed_code)
    .map_err(|reason| format!("E_WASM_TARGET: command entry `{qualified_init}` is not a function: {reason}"))?;
  let (init_arity, _) = compute_fn_arity(&args);
  if init_arity != 0 {
    return Err(format!(
      "E_WASM_TARGET: WASI command entry `{qualified_init}` must take no arguments, got {init_arity}"
    ));
  }
  Ok(())
}

fn validate_wasm_command_component_in_program(
  program_data: &program::CompiledProgram,
  init_ns: &str,
  init_def: &str,
) -> Result<(), String> {
  for namespace in deterministic_namespace_order(program_data, None) {
    let file = program_data.get(namespace.as_str()).expect("ordered namespace must exist");
    for (name, compiled) in deterministic_definition_order(file) {
      if is_wasm_export_def(&compiled.preprocessed_code) {
        return Err(format!(
          "E_WASI_COMMAND_EXPORT: `{namespace}/{name}` declares a generic Component export; `calcit wasi --boundary component` only exports `wasi:cli/run@0.3.1`"
        ));
      }
    }
  }
  let qualified_init = format!("{init_ns}/{init_def}");
  let signature = program_data
    .get(init_ns)
    .and_then(|file| file.defs.get(init_def))
    .and_then(|compiled| compiled.schema.resolve_to_fn())
    .ok_or_else(|| format!("E_WASI_COMMAND_ENTRY: `{qualified_init}` needs an explicit function schema returning Unit"))?;
  if !matches!(signature.return_type.as_ref(), CalcitTypeAnnotation::Unit) {
    return Err(format!(
      "E_WASI_COMMAND_ENTRY: `{qualified_init}` must return Unit; other results cannot be silently discarded"
    ));
  }
  Ok(())
}

/// Intermediate representation of a compiled function before encoding.
struct CompiledFn {
  export_name: Option<String>,
  params: Vec<ValType>,
  results: Vec<ValType>,
  /// All local variables (including temporaries), indexed by declaration order
  locals: Vec<ValType>,
  /// Instruction sequence for the function body
  instructions: Vec<Instruction<'static>>,
}

fn reject_reachable_wasi_command_dependencies(
  functions: &[CompiledFn],
  num_imports: u32,
  entry_index: u32,
  unsupported: &HashMap<u32, (String, String)>,
) -> Result<(), String> {
  let mut pending = vec![entry_index];
  let mut visited = HashSet::new();
  while let Some(index) = pending.pop() {
    if !visited.insert(index) {
      continue;
    }
    if let Some((definition, reason)) = unsupported.get(&index) {
      let code = if reason.starts_with("E_WASI_COMMAND_CAPABILITY:") {
        "E_WASI_COMMAND_CAPABILITY"
      } else {
        "E_WASI_COMMAND_DEPENDENCY"
      };
      return Err(format!("{code}: reachable `{definition}` cannot compile: {reason}"));
    }
    if index < num_imports {
      continue;
    }
    let function = functions
      .get((index - num_imports) as usize)
      .ok_or_else(|| format!("E_WASI_COMMAND_DEPENDENCY: missing function index {index}"))?;
    for instruction in &function.instructions {
      match instruction {
        Instruction::Call(callee) => pending.push(*callee),
        Instruction::CallIndirect { .. } => {
          return Err("E_WASI_COMMAND_INDIRECT: an indirect call may reach an unsupported host capability".into());
        }
        _ => {}
      }
    }
  }
  Ok(())
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ComponentAbiType {
  Unit,
  Bool,
  Buffer,
  List(Box<ComponentAbiType>),
  Number,
  Numeric(CalcitNumericRefinement),
  Option(Box<ComponentAbiType>),
  Result(Box<ComponentAbiType>, Box<ComponentAbiType>),
  String,
  Struct(ComponentStructType),
  Enum(ComponentEnumType),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ComponentStructType {
  id: String,
  tag: String,
  fields: Vec<(String, ComponentAbiType)>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ComponentEnumType {
  id: String,
  variants: Vec<ComponentEnumVariant>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ComponentEnumVariant {
  tag: String,
  payload: Vec<ComponentAbiType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ComponentExportAdapter {
  definition: String,
  symbol: String,
  target_index: u32,
  invocation: ComponentAbiInvocation,
  stackless_tail_import: Option<String>,
  stream_consumer: Option<ComponentStreamConsumer>,
  task_return_index: Option<u32>,
  parameters: Vec<ComponentAbiType>,
  result: ComponentAbiType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ComponentStreamConsumer {
  parameter_index: usize,
  max_total_bytes: u32,
  max_chunk_bytes: u32,
  handler_definition: String,
  handler_index: u32,
  total_limit_variant: u32,
  stream_read_index: u32,
  stream_cancel_read_index: u32,
  stream_drop_readable_index: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ComponentImportAdapter {
  definition: String,
  module: String,
  symbol: String,
  raw_index: u32,
  source_arity: u32,
  invocation: ComponentAbiInvocation,
  parameters: Vec<ComponentAbiType>,
  result: ComponentAbiType,
}

struct ComponentExportRuntime<'a> {
  codecs: ComponentValueCodecs<'a>,
  cabi_free_index: u32,
  drop_indices: &'a BTreeMap<ComponentAbiType, u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ComponentAsyncCanonicalImports {
  waitable_set_new: u32,
  waitable_set_wait: u32,
  waitable_set_drop: u32,
  waitable_join: u32,
  subtask_drop: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ComponentStacklessCanonicalImports {
  context_get: u32,
  context_set: u32,
  subtask_cancel: u32,
  task_cancel: u32,
}

fn register_component_stream_canonical_imports(
  host_imports: &mut Vec<HostImport>,
  symbol: &str,
  consumer: &mut ComponentStreamConsumer,
) {
  let mut register = |name: String, params: Vec<ValType>, results: Vec<ValType>| {
    let index = host_imports.len() as u32;
    host_imports.push(HostImport {
      module: COMPONENT_ASYNC_EXPORT_IMPORT_MODULE.into(),
      name,
      params,
      results,
    });
    index
  };
  let suffix = format!("{}]{symbol}", consumer.parameter_index);
  consumer.stream_read_index = register(
    format!("[async-lower][stream-read-{suffix}"),
    vec![ValType::I32, ValType::I32, ValType::I32],
    vec![ValType::I32],
  );
  consumer.stream_cancel_read_index = register(
    format!("[async-lower][stream-cancel-read-{suffix}"),
    vec![ValType::I32],
    vec![ValType::I32],
  );
  consumer.stream_drop_readable_index = register(format!("[stream-drop-readable-{suffix}"), vec![ValType::I32], vec![]);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComponentAbiInvocation {
  Sync,
  Async,
}

const COMPONENT_ASYNC_ROOT_IMPORT_MODULE: &str = "$root";
const COMPONENT_ASYNC_EXPORT_IMPORT_MODULE: &str = "[export]$root";
const COMPONENT_ASYNC_MAX_FLAT_PARAMETERS: usize = 4;

fn register_component_async_canonical_imports(host_imports: &mut Vec<HostImport>) -> ComponentAsyncCanonicalImports {
  let mut register = |name: &str, params: Vec<ValType>, results: Vec<ValType>| {
    let index = host_imports.len() as u32;
    host_imports.push(HostImport {
      module: COMPONENT_ASYNC_ROOT_IMPORT_MODULE.into(),
      name: name.into(),
      params,
      results,
    });
    index
  };
  ComponentAsyncCanonicalImports {
    waitable_set_new: register("[waitable-set-new]", vec![], vec![ValType::I32]),
    waitable_set_wait: register("[waitable-set-wait]", vec![ValType::I32, ValType::I32], vec![ValType::I32]),
    waitable_set_drop: register("[waitable-set-drop]", vec![ValType::I32], vec![]),
    waitable_join: register("[waitable-join]", vec![ValType::I32, ValType::I32], vec![]),
    subtask_drop: register("[subtask-drop]", vec![ValType::I32], vec![]),
  }
}

fn register_component_stackless_canonical_imports(host_imports: &mut Vec<HostImport>) -> ComponentStacklessCanonicalImports {
  let mut register = |module: &str, name: &str, params: Vec<ValType>, results: Vec<ValType>| {
    let index = host_imports.len() as u32;
    host_imports.push(HostImport {
      module: module.into(),
      name: name.into(),
      params,
      results,
    });
    index
  };
  ComponentStacklessCanonicalImports {
    context_get: register(COMPONENT_ASYNC_ROOT_IMPORT_MODULE, "[context-get-0]", vec![], vec![ValType::I32]),
    context_set: register(COMPONENT_ASYNC_ROOT_IMPORT_MODULE, "[context-set-0]", vec![ValType::I32], vec![]),
    subtask_cancel: register(
      COMPONENT_ASYNC_ROOT_IMPORT_MODULE,
      "[async-lower][subtask-cancel]",
      vec![ValType::I32],
      vec![ValType::I32],
    ),
    task_cancel: register(COMPONENT_ASYNC_EXPORT_IMPORT_MODULE, "[task-cancel]", vec![], vec![]),
  }
}

fn component_async_import_symbol(symbol: &str) -> String {
  format!("[async-lower]{symbol}")
}

fn component_async_export_symbol(adapter: &ComponentExportAdapter) -> String {
  if adapter.stackless_tail_import.is_some() || adapter.stream_consumer.is_some() {
    format!("[async-lift]{}", adapter.symbol)
  } else {
    format!("[async-lift-stackful]{}", adapter.symbol)
  }
}

fn component_async_callback_symbol(adapter: &ComponentExportAdapter) -> String {
  format!("[callback][async-lift]{}", adapter.symbol)
}

fn component_post_return_symbol(adapter: &ComponentExportAdapter) -> String {
  format!("cabi_post_{}", component_export_emitted_symbol(adapter))
}

fn component_export_needs_post_return(adapter: &ComponentExportAdapter) -> bool {
  if adapter.invocation == ComponentAbiInvocation::Async || matches!(adapter.result, ComponentAbiType::Unit) {
    return false;
  }
  match &adapter.result {
    ComponentAbiType::Buffer | ComponentAbiType::String | ComponentAbiType::List(_) => true,
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) | ComponentAbiType::Struct(_) | ComponentAbiType::Enum(_) => {
      component_flat_types(&adapter.result).len() > 1
    }
    ComponentAbiType::Unit | ComponentAbiType::Bool | ComponentAbiType::Number | ComponentAbiType::Numeric(_) => false,
  }
}

fn component_import_emitted_symbol(adapter: &ComponentImportAdapter) -> String {
  if adapter.invocation == ComponentAbiInvocation::Async {
    component_async_import_symbol(&adapter.symbol)
  } else {
    adapter.symbol.clone()
  }
}

fn component_export_emitted_symbol(adapter: &ComponentExportAdapter) -> String {
  if adapter.invocation == ComponentAbiInvocation::Async {
    component_async_export_symbol(adapter)
  } else {
    adapter.symbol.clone()
  }
}

fn component_task_return_symbol(symbol: &str) -> String {
  format!("[task-return]{symbol}")
}

#[cfg(test)]
fn component_abi_type(annotation: &CalcitTypeAnnotation, definition: &str, path: &str) -> Result<ComponentAbiType, String> {
  component_abi_type_inner(annotation, definition, path, &mut Vec::new(), None)
}

fn component_abi_type_inner(
  annotation: &CalcitTypeAnnotation,
  definition: &str,
  path: &str,
  nominal_stack: &mut Vec<String>,
  program_data: Option<&program::CompiledProgram>,
) -> Result<ComponentAbiType, String> {
  match annotation {
    CalcitTypeAnnotation::Unit => Ok(ComponentAbiType::Unit),
    CalcitTypeAnnotation::Bool => Ok(ComponentAbiType::Bool),
    CalcitTypeAnnotation::Buffer => Ok(ComponentAbiType::Buffer),
    CalcitTypeAnnotation::List(item) => Ok(ComponentAbiType::List(Box::new(component_abi_type_inner(
      item,
      definition,
      &format!("{path}.item"),
      nominal_stack,
      program_data,
    )?))),
    CalcitTypeAnnotation::Number => Ok(ComponentAbiType::Number),
    CalcitTypeAnnotation::Numeric(kind) => Ok(ComponentAbiType::Numeric(*kind)),
    CalcitTypeAnnotation::String => Ok(ComponentAbiType::String),
    CalcitTypeAnnotation::TypeRef(name, arguments) => {
      let name = name.trim_start_matches('\'').trim_start_matches(':');
      match (name, arguments.as_slice()) {
        ("Option" | "calcit.core/Option", [item]) => Ok(ComponentAbiType::Option(Box::new(component_abi_type_inner(
          item,
          definition,
          &format!("{path}.item"),
          nominal_stack,
          program_data,
        )?))),
        ("Result" | "calcit.core/Result", [ok, error]) => Ok(ComponentAbiType::Result(
          Box::new(component_abi_type_inner(
            ok,
            definition,
            &format!("{path}.ok"),
            nominal_stack,
            program_data,
          )?),
          Box::new(component_abi_type_inner(
            error,
            definition,
            &format!("{path}.error"),
            nominal_stack,
            program_data,
          )?),
        )),
        ("StreamConsumeError" | "calcit.core/StreamConsumeError", []) => Ok(ComponentAbiType::Enum(ComponentEnumType {
          id: "calcit.core/StreamConsumeError".into(),
          variants: vec![ComponentEnumVariant {
            tag: "total-limit".into(),
            payload: vec![],
          }],
        })),
        ("Option" | "calcit.core/Option", _) => Err(format!(
          "E_COMPONENT_ABI_TYPE_ARGUMENT_ARITY: `{definition}` at `{path}` requires Option<T> with exactly one type argument"
        )),
        ("Result" | "calcit.core/Result", _) => Err(format!(
          "E_COMPONENT_ABI_TYPE_ARGUMENT_ARITY: `{definition}` at `{path}` requires Result<T, E> with exactly two type arguments"
        )),
        _ => {
          if let Some(struct_def) = resolve_component_struct_ref(name, definition, program_data) {
            component_abi_type_inner(
              &CalcitTypeAnnotation::Struct(struct_def, arguments.clone()),
              definition,
              path,
              nominal_stack,
              program_data,
            )
          } else if let Some(enum_def) = resolve_component_enum_ref(name, definition, program_data) {
            component_abi_type_inner(
              &CalcitTypeAnnotation::Enum(enum_def, arguments.clone()),
              definition,
              path,
              nominal_stack,
              program_data,
            )
          } else {
            Err(format!(
              "E_COMPONENT_ABI_UNSUPPORTED_TYPE: `{definition}` at `{path}` uses `{annotation}`, but this adapter slice only supports Unit results, Bool, Buffer, List<T>, Number, Option<T>, Result<T, E>, String, monomorphic Struct records, and monomorphic Enum variants"
            ))
          }
        }
      }
    }
    CalcitTypeAnnotation::Struct(struct_def, arguments) => {
      if struct_def.generics.len() != arguments.len() {
        return Err(format!(
          "E_COMPONENT_ABI_TYPE_ARGUMENT_ARITY: `{definition}` at `{path}` requires `{}` with {} type argument(s), got {}",
          struct_def.name,
          struct_def.generics.len(),
          arguments.len()
        ));
      }
      if struct_def.fields.len() != struct_def.field_types.len() {
        return Err(format!(
          "E_COMPONENT_ABI_STRUCT_FIELDS: `{definition}` at `{path}` uses `{}` with {} field name(s) but {} field type(s)",
          struct_def.name,
          struct_def.fields.len(),
          struct_def.field_types.len()
        ));
      }
      let id = struct_def
        .definition_ref
        .as_deref()
        .unwrap_or_else(|| struct_def.name.ref_str())
        .to_owned();
      let application = if arguments.is_empty() {
        id.clone()
      } else {
        format!(
          "{id}<{}>",
          arguments.iter().map(|item| item.to_brief_string()).collect::<Vec<_>>().join(",")
        )
      };
      if nominal_stack.contains(&application) {
        return Err(format!(
          "E_COMPONENT_ABI_RECURSIVE_STRUCT: `{definition}` at `{path}` recursively reaches `{application}`, which is not supported by the synchronous record adapter"
        ));
      }
      nominal_stack.push(application);
      let bindings = struct_def
        .generics
        .iter()
        .cloned()
        .zip(arguments.iter().cloned())
        .collect::<HashMap<_, _>>();
      let fields = struct_def
        .fields
        .iter()
        .zip(struct_def.field_types.iter())
        .map(|(field, field_type)| {
          let resolved = field_type.substitute_type_vars(&bindings);
          component_abi_type_inner(
            resolved.as_ref(),
            definition,
            &format!("{path}.fields.{}", field.ref_str()),
            nominal_stack,
            program_data,
          )
          .map(|value_type| (field.ref_str().to_owned(), value_type))
        })
        .collect::<Result<Vec<_>, _>>();
      nominal_stack.pop();
      Ok(ComponentAbiType::Struct(ComponentStructType {
        id,
        tag: struct_def.name.ref_str().to_owned(),
        fields: fields?,
      }))
    }
    CalcitTypeAnnotation::StructValue(struct_def) => component_abi_type_inner(
      &CalcitTypeAnnotation::Struct(struct_def.clone(), Arc::new(vec![])),
      definition,
      path,
      nominal_stack,
      program_data,
    ),
    CalcitTypeAnnotation::Enum(enum_def, arguments) => {
      if !enum_def.generics().is_empty() || !arguments.is_empty() {
        return Err(format!(
          "E_COMPONENT_ABI_UNSUPPORTED_GENERIC: `{definition}` at `{path}` requires a non-generic Enum declaration"
        ));
      }
      let id = enum_def
        .definition_ref()
        .map_or_else(|| enum_def.name().ref_str(), AsRef::as_ref)
        .to_owned();
      if nominal_stack.contains(&id) {
        return Err(format!(
          "E_COMPONENT_ABI_RECURSIVE_ENUM: `{definition}` at `{path}` recursively reaches `{id}`, which is not supported by the synchronous variant adapter"
        ));
      }
      nominal_stack.push(id.clone());
      let variants = enum_def
        .variants()
        .iter()
        .enumerate()
        .map(|(variant_index, variant)| {
          let payload = variant
            .payload_types()
            .iter()
            .enumerate()
            .map(|(payload_index, payload_type)| {
              let converted = component_abi_type_inner(
                payload_type,
                definition,
                &format!("{path}.variants[{variant_index}].payload[{payload_index}]"),
                nominal_stack,
                program_data,
              )?;
              if matches!(converted, ComponentAbiType::Unit) {
                return Err(format!(
                  "E_COMPONENT_ABI_UNIT_ENUM_PAYLOAD: `{definition}` at `{path}.variants[{variant_index}].payload[{payload_index}]` cannot use Unit as an explicit payload; omit the payload instead"
                ));
              }
              Ok(converted)
            })
            .collect::<Result<Vec<_>, _>>()?;
          Ok(ComponentEnumVariant {
            tag: variant.tag.ref_str().to_owned(),
            payload,
          })
        })
        .collect::<Result<Vec<_>, String>>();
      nominal_stack.pop();
      let variants = variants?;
      if variants.is_empty() {
        return Err(format!(
          "E_COMPONENT_ABI_EMPTY_ENUM: `{definition}` at `{path}` uses `{id}` without variants"
        ));
      }
      Ok(ComponentAbiType::Enum(ComponentEnumType { id, variants }))
    }
    CalcitTypeAnnotation::EnumValue(enum_def) => component_abi_type_inner(
      &CalcitTypeAnnotation::Enum(enum_def.clone(), Arc::new(vec![])),
      definition,
      path,
      nominal_stack,
      program_data,
    ),
    other => Err(format!(
      "E_COMPONENT_ABI_UNSUPPORTED_TYPE: `{definition}` at `{path}` uses `{other}`, but this adapter slice only supports Unit results, Bool, Buffer, List<T>, Number, Option<T>, Result<T, E>, String, monomorphic Struct records, and monomorphic Enum variants"
    )),
  }
}

fn resolve_component_enum_ref(
  name: &str,
  owner_definition: &str,
  program_data: Option<&program::CompiledProgram>,
) -> Option<Arc<CalcitEnumDef>> {
  let name = name.trim_start_matches('\'').trim_start_matches(':');
  let (namespace, definition) = name
    .rsplit_once('/')
    .or_else(|| owner_definition.rsplit_once('/').map(|(namespace, _)| (namespace, name)))?;
  let compiled = program_data
    .and_then(|program_data| program_data.get(namespace).and_then(|file| file.defs.get(definition)).cloned())
    .or_else(|| program::lookup_compiled_def(namespace, definition))?;
  match compiled.schema.as_ref() {
    CalcitTypeAnnotation::EnumDef(enum_def) => Some(enum_def.clone()),
    CalcitTypeAnnotation::Enum(enum_def, _) | CalcitTypeAnnotation::EnumValue(enum_def) => Some(enum_def.clone()),
    _ => compiled
      .source_code
      .iter()
      .chain([&compiled.preprocessed_code, &compiled.codegen_form])
      .find_map(|code| match code {
        Calcit::EnumDef(enum_def) => Some(Arc::new(enum_def.clone())),
        code => match crate::calcit::type_annotation::resolve_type_def_from_code(code) {
          Some(Calcit::EnumDef(enum_def)) => Some(Arc::new(enum_def)),
          _ => None,
        },
      }),
  }
}

fn resolve_component_struct_ref(
  name: &str,
  owner_definition: &str,
  program_data: Option<&program::CompiledProgram>,
) -> Option<Arc<CalcitStructDef>> {
  let name = name.trim_start_matches('\'').trim_start_matches(':');
  let (namespace, definition) = name
    .rsplit_once('/')
    .or_else(|| owner_definition.rsplit_once('/').map(|(namespace, _)| (namespace, name)))?;
  let compiled = program_data
    .and_then(|program_data| program_data.get(namespace).and_then(|file| file.defs.get(definition)).cloned())
    .or_else(|| program::lookup_compiled_def(namespace, definition))?;
  match compiled.schema.as_ref() {
    CalcitTypeAnnotation::StructDef(struct_def) => Some(struct_def.clone()),
    CalcitTypeAnnotation::Struct(struct_def, _) | CalcitTypeAnnotation::StructValue(struct_def) => Some(struct_def.clone()),
    _ => compiled
      .source_code
      .iter()
      .chain([&compiled.preprocessed_code, &compiled.codegen_form])
      .find_map(|code| match code {
        Calcit::StructDef(struct_def) => Some(Arc::new(struct_def.clone())),
        code => match crate::calcit::type_annotation::resolve_type_def_from_code(code) {
          Some(Calcit::StructDef(struct_def)) => Some(Arc::new(struct_def)),
          _ => None,
        },
      }),
  }
}

fn component_function_schema(
  compiled: &program::CompiledDef,
  definition: &str,
  source_arity: usize,
  program_data: &program::CompiledProgram,
  validate_direct_parameter_limit: bool,
  allow_readable_byte_stream: bool,
) -> Result<(Vec<ComponentAbiType>, ComponentAbiType, ComponentAbiInvocation), String> {
  let signature = compiled
    .schema
    .resolve_to_fn()
    .ok_or_else(|| format!("E_COMPONENT_ABI_MISSING_SCHEMA: `{definition}` at `logical_schema` requires a resolved function schema"))?;
  let invocation = if signature.is_async_invocation() {
    ComponentAbiInvocation::Async
  } else {
    ComponentAbiInvocation::Sync
  };
  if !signature.generics.is_empty() {
    return Err(format!(
      "E_COMPONENT_ABI_UNSUPPORTED_GENERIC: `{definition}` at `logical_schema.generics` must be monomorphized"
    ));
  }
  if signature.rest_type.is_some() {
    return Err(format!(
      "E_COMPONENT_ABI_UNSUPPORTED_ARITY: `{definition}` at `logical_schema.rest` must use fixed arity"
    ));
  }
  let parameters = signature
    .arg_types
    .iter()
    .enumerate()
    .map(|(index, annotation)| {
      if is_readable_byte_stream_annotation(annotation) {
        if allow_readable_byte_stream {
          Ok(ComponentAbiType::Numeric(CalcitNumericRefinement::UInt32))
        } else {
          Err(format!(
            "E_COMPONENT_ABI_STREAM_DIRECTION: `{definition}` at `logical_schema.parameters[{index}]` can consume ReadableByteStream only from an async Component export"
          ))
        }
      } else {
        component_abi_type_inner(
          annotation,
          definition,
          &format!("logical_schema.parameters[{index}]"),
          &mut Vec::new(),
          Some(program_data),
        )
      }
    })
    .collect::<Result<Vec<_>, _>>()?;
  if let Some(index) = parameters.iter().position(|parameter| matches!(parameter, ComponentAbiType::Unit)) {
    return Err(format!(
      "E_COMPONENT_ABI_UNIT_PARAMETER: `{definition}` at `logical_schema.parameters[{index}]` cannot use Unit as a parameter; omit that parameter instead"
    ));
  }
  if parameters.len() != source_arity {
    return Err(format!(
      "E_COMPONENT_ABI_SCHEMA_ARITY: `{definition}` declares {source_arity} source parameters but its schema declares {}",
      parameters.len()
    ));
  }
  if validate_direct_parameter_limit {
    validate_component_flat_parameters(&parameters, definition)?;
  }
  let result = component_abi_type_inner(
    &signature.return_type,
    definition,
    "logical_schema.result",
    &mut Vec::new(),
    Some(program_data),
  )?;
  Ok((parameters, result, invocation))
}

fn is_readable_byte_stream_annotation(annotation: &CalcitTypeAnnotation) -> bool {
  matches!(
    annotation,
    CalcitTypeAnnotation::TypeRef(name, arguments)
      if arguments.is_empty()
        && matches!(name.trim_start_matches('\'').trim_start_matches(':'), "ReadableByteStream" | "calcit.core/ReadableByteStream")
  )
}

const COMPONENT_MAX_FLAT_PARAMETERS: usize = 16;

fn validate_component_flat_parameters(parameters: &[ComponentAbiType], definition: &str) -> Result<(), String> {
  let flat_count = parameters
    .iter()
    .map(|parameter| component_flat_types(parameter).len())
    .sum::<usize>();
  if flat_count > COMPONENT_MAX_FLAT_PARAMETERS {
    return Err(format!(
      "E_COMPONENT_ABI_FLAT_PARAMETER_LIMIT: `{definition}` at `logical_schema.parameters` flattens to {flat_count} values, but the synchronous direct adapter supports at most {COMPONENT_MAX_FLAT_PARAMETERS}; split the record or parameters until indirect parameter lowering is available"
    ));
  }
  Ok(())
}

fn component_async_import_uses_indirect_parameters(adapter: &ComponentImportAdapter) -> bool {
  adapter.invocation == ComponentAbiInvocation::Async
    && adapter
      .parameters
      .iter()
      .map(|parameter| component_flat_types(parameter).len())
      .sum::<usize>()
      > COMPONENT_ASYNC_MAX_FLAT_PARAMETERS
}

fn component_flat_types(value_type: &ComponentAbiType) -> Vec<ValType> {
  match value_type {
    ComponentAbiType::Unit => vec![],
    ComponentAbiType::Bool => vec![ValType::I32],
    ComponentAbiType::Number => vec![ValType::F64],
    ComponentAbiType::Numeric(kind) => vec![component_numeric_flat_type(*kind)],
    ComponentAbiType::Buffer | ComponentAbiType::List(_) | ComponentAbiType::String => vec![ValType::I32, ValType::I32],
    ComponentAbiType::Option(item) => {
      let mut flattened = vec![ValType::I32];
      flattened.extend(component_join_flat_types(&[], &component_flat_types(item)));
      flattened
    }
    ComponentAbiType::Result(ok, error) => {
      let mut flattened = vec![ValType::I32];
      flattened.extend(component_join_flat_types(&component_flat_types(ok), &component_flat_types(error)));
      flattened
    }
    ComponentAbiType::Struct(record) => record
      .fields
      .iter()
      .flat_map(|(_, field_type)| component_flat_types(field_type))
      .collect(),
    ComponentAbiType::Enum(enum_type) => {
      let joined = enum_type.variants.iter().fold(Vec::new(), |joined, variant| {
        let flattened = variant.payload.iter().flat_map(component_flat_types).collect::<Vec<_>>();
        component_join_flat_types(&joined, &flattened)
      });
      let mut flattened = vec![ValType::I32];
      flattened.extend(joined);
      flattened
    }
  }
}

fn component_join_flat_types(left: &[ValType], right: &[ValType]) -> Vec<ValType> {
  let width = left.len().max(right.len());
  (0..width)
    .map(|index| match (left.get(index), right.get(index)) {
      (Some(left), Some(right)) if left == right => *left,
      (Some(ValType::I32), Some(ValType::F32)) | (Some(ValType::F32), Some(ValType::I32)) => ValType::I32,
      (Some(_), Some(_)) => ValType::I64,
      (Some(value), None) | (None, Some(value)) => *value,
      (None, None) => unreachable!("join width is derived from the longer shape"),
    })
    .collect()
}

fn component_numeric_flat_type(kind: CalcitNumericRefinement) -> ValType {
  match kind {
    CalcitNumericRefinement::Int8
    | CalcitNumericRefinement::UInt8
    | CalcitNumericRefinement::Int16
    | CalcitNumericRefinement::UInt16
    | CalcitNumericRefinement::Int32
    | CalcitNumericRefinement::UInt32 => ValType::I32,
    CalcitNumericRefinement::Int64 | CalcitNumericRefinement::UInt64 => ValType::I64,
    CalcitNumericRefinement::Float32 => ValType::F32,
    CalcitNumericRefinement::Float64 => ValType::F64,
  }
}

fn collect_component_import_adapters(program_data: &program::CompiledProgram) -> Result<Vec<ComponentImportAdapter>, String> {
  let mut adapters = Vec::new();
  for (namespace, file) in program_data {
    for (name, compiled) in &file.defs {
      if !is_wasm_import_def(&compiled.preprocessed_code) {
        continue;
      }
      let definition = format!("{namespace}/{name}");
      let (module, symbol, args) = parse_wasm_import_def(&compiled.preprocessed_code)
        .ok_or_else(|| format!("E_COMPONENT_ABI_IMPORT: `{definition}` must use `defwasm-import name (args) |module |field`"))?;
      if matches!(
        module.as_str(),
        COMPONENT_ASYNC_ROOT_IMPORT_MODULE | COMPONENT_ASYNC_EXPORT_IMPORT_MODULE
      ) {
        return Err(format!(
          "E_COMPONENT_ABI_RESERVED_IMPORT: `{definition}` cannot declare the compiler-reserved module `{module}`"
        ));
      }
      let source_arity = wasm_import_arity(&args)
        .map_err(|reason| format!("E_COMPONENT_ABI_UNSUPPORTED_ARITY: `{definition}` at `logical_schema.parameters` {reason}"))?;
      let (parameters, result, invocation) =
        component_function_schema(compiled, &definition, source_arity as usize, program_data, false, false)?;
      if invocation == ComponentAbiInvocation::Sync {
        validate_component_flat_parameters(&parameters, &definition)?;
      }
      adapters.push(ComponentImportAdapter {
        definition,
        module,
        symbol,
        raw_index: 0,
        source_arity,
        invocation,
        parameters,
        result,
      });
    }
  }
  adapters.sort_unstable_by(|left, right| left.definition.cmp(&right.definition));
  validate_component_import_symbols(&adapters)?;
  Ok(adapters)
}

fn direct_tail_import_definition(body: &[Calcit], args: &CalcitFnArgs) -> Option<String> {
  let expressions = body
    .iter()
    .filter(|item| CalcitTypeAnnotation::extract_fn_annotation_from_hint_form(item).is_none())
    .collect::<Vec<_>>();
  let [Calcit::List(call)] = expressions.as_slice() else {
    return None;
  };
  let Calcit::Import(import) = call.first()? else {
    return None;
  };
  let parameters = fn_param_names(args);
  if call.len() != parameters.len() + 1 {
    return None;
  }
  for (argument, parameter) in call.iter().skip(1).zip(parameters) {
    let Calcit::Local(local) = argument else {
      return None;
    };
    if local.sym.as_ref() != parameter {
      return None;
    }
  }
  Some(format!("{}/{}", import.ns, import.def))
}

fn direct_definition_reference(value: &Calcit) -> Option<String> {
  match value {
    Calcit::Import(import) => Some(format!("{}/{}", import.ns, import.def)),
    Calcit::Symbol { sym, info, .. } => Some(format!("{}/{}", info.at_ns, sym)),
    _ => None,
  }
}

fn is_consume_readable_byte_stream(value: &Calcit) -> bool {
  match value {
    Calcit::Import(import) => import.def.as_ref() == "consume-readable-byte-stream",
    Calcit::Symbol { sym, .. } => sym.as_ref() == "consume-readable-byte-stream",
    _ => false,
  }
}

fn positive_u32_literal(value: &Calcit, definition: &str, path: &str) -> Result<u32, String> {
  let Calcit::Number(value) = value else {
    return Err(format!(
      "E_COMPONENT_STREAM_LITERAL_BOUND: `{definition}` at `{path}` must be a positive integer literal"
    ));
  };
  if !value.is_finite() || value.fract() != 0.0 || *value <= 0.0 || *value > i32::MAX as f64 {
    return Err(format!(
      "E_COMPONENT_STREAM_LITERAL_BOUND: `{definition}` at `{path}` must be a positive integer literal no greater than {}",
      i32::MAX
    ));
  }
  Ok(*value as u32)
}

fn component_stream_total_limit_variant(result: &ComponentAbiType, definition: &str) -> Result<u32, String> {
  let ComponentAbiType::Result(ok, error) = result else {
    return Err(format!(
      "E_COMPONENT_STREAM_RESULT: `{definition}` must return Result<Unit, StreamConsumeError>"
    ));
  };
  if !matches!(ok.as_ref(), ComponentAbiType::Unit) {
    return Err(format!(
      "E_COMPONENT_STREAM_RESULT: `{definition}` must return Result<Unit, StreamConsumeError>"
    ));
  }
  let ComponentAbiType::Enum(error) = error.as_ref() else {
    return Err(format!(
      "E_COMPONENT_STREAM_RESULT: `{definition}` must use a named StreamConsumeError enum"
    ));
  };
  if error.id != "calcit.core/StreamConsumeError" {
    return Err(format!(
      "E_COMPONENT_STREAM_RESULT: `{definition}` must return Result<Unit, StreamConsumeError>, not `{}`",
      error.id
    ));
  }
  error
    .variants
    .iter()
    .position(|variant| variant.tag == "total-limit" && variant.payload.is_empty())
    .map(|index| index as u32)
    .ok_or_else(|| {
      format!(
        "E_COMPONENT_STREAM_RESULT: `{definition}` error enum `{}` must define a payload-free `:total-limit` variant",
        error.id
      )
    })
}

fn component_stream_consumer(
  definition: &str,
  body: &[Calcit],
  args: &CalcitFnArgs,
  signature: &CalcitFnTypeAnnotation,
  result: &ComponentAbiType,
  program_data: &program::CompiledProgram,
) -> Result<Option<ComponentStreamConsumer>, String> {
  let stream_parameters = signature
    .arg_types
    .iter()
    .enumerate()
    .filter(|(_, annotation)| is_readable_byte_stream_annotation(annotation))
    .map(|(index, _)| index)
    .collect::<Vec<_>>();
  if stream_parameters.is_empty() {
    return Ok(None);
  }
  let [parameter_index] = stream_parameters.as_slice() else {
    return Err(format!(
      "E_COMPONENT_STREAM_PARAMETER_COUNT: `{definition}` must consume exactly one ReadableByteStream"
    ));
  };
  if signature.arg_types.len() != 1 || *parameter_index != 0 {
    return Err(format!(
      "E_COMPONENT_STREAM_PARAMETER_COUNT: `{definition}` first slice requires ReadableByteStream to be the only parameter"
    ));
  }
  if !signature.is_async_invocation() {
    return Err(format!(
      "E_COMPONENT_STREAM_REQUIRES_ASYNC: `{definition}` must use an async function schema"
    ));
  }
  let expressions = body
    .iter()
    .filter(|item| CalcitTypeAnnotation::extract_fn_annotation_from_hint_form(item).is_none())
    .collect::<Vec<_>>();
  let [Calcit::List(call)] = expressions.as_slice() else {
    return Err(format!(
      "E_COMPONENT_STREAM_CONSUMER_SHAPE: `{definition}` must contain exactly one `consume-readable-byte-stream stream max-total-bytes max-chunk-bytes on-chunk` expression"
    ));
  };
  let (Some(head), Some(stream), Some(max_total), Some(max_chunk), Some(handler)) =
    (call.get(0), call.get(1), call.get(2), call.get(3), call.get(4))
  else {
    return Err(format!(
      "E_COMPONENT_STREAM_CONSUMER_SHAPE: `{definition}` must call `consume-readable-byte-stream` with four arguments"
    ));
  };
  if call.len() != 5 {
    return Err(format!(
      "E_COMPONENT_STREAM_CONSUMER_SHAPE: `{definition}` must call `consume-readable-byte-stream` with four arguments"
    ));
  }
  if !is_consume_readable_byte_stream(head) {
    return Err(format!(
      "E_COMPONENT_STREAM_CONSUMER_SHAPE: `{definition}` must delegate its stream parameter to `consume-readable-byte-stream`"
    ));
  }
  let parameters = fn_param_names(args);
  let Some(parameter_name) = parameters.get(*parameter_index) else {
    return Err(format!(
      "E_COMPONENT_STREAM_PARAMETER: `{definition}` stream parameter is missing from the source arguments"
    ));
  };
  if !matches!(stream, Calcit::Local(local) if local.sym.as_ref() == parameter_name) {
    return Err(format!(
      "E_COMPONENT_STREAM_PARAMETER: `{definition}` must pass its ReadableByteStream parameter directly without storing, copying, or wrapping it"
    ));
  }
  let max_total_bytes = positive_u32_literal(max_total, definition, "consume-readable-byte-stream.max-total-bytes")?;
  let max_chunk_bytes = positive_u32_literal(max_chunk, definition, "consume-readable-byte-stream.max-chunk-bytes")?;
  if max_chunk_bytes > max_total_bytes {
    return Err(format!(
      "E_COMPONENT_STREAM_BOUND_ORDER: `{definition}` max-chunk-bytes ({max_chunk_bytes}) cannot exceed max-total-bytes ({max_total_bytes})"
    ));
  }
  let handler_definition = direct_definition_reference(handler).ok_or_else(|| {
    format!(
      "E_COMPONENT_STREAM_HANDLER: `{definition}` requires a top-level `(Buffer) -> Bool` handler; inline or dynamic callbacks are not supported"
    )
  })?;
  let (handler_namespace, handler_name) = handler_definition
    .rsplit_once('/')
    .ok_or_else(|| format!("E_COMPONENT_STREAM_HANDLER: invalid handler `{handler_definition}`"))?;
  let handler = program_data
    .get(handler_namespace)
    .and_then(|file| file.defs.get(handler_name))
    .ok_or_else(|| format!("E_COMPONENT_STREAM_HANDLER: `{definition}` cannot resolve handler `{handler_definition}`"))?;
  let handler_signature = handler
    .schema
    .resolve_to_fn()
    .ok_or_else(|| format!("E_COMPONENT_STREAM_HANDLER: `{handler_definition}` must declare a `(Buffer) -> Bool` function schema"))?;
  if handler_signature.is_async_invocation()
    || handler_signature.rest_type.is_some()
    || !handler_signature.generics.is_empty()
    || !matches!(handler_signature.arg_types.as_slice(), [item] if matches!(item.as_ref(), CalcitTypeAnnotation::Buffer))
    || !matches!(handler_signature.return_type.as_ref(), CalcitTypeAnnotation::Bool)
  {
    return Err(format!(
      "E_COMPONENT_STREAM_HANDLER: `{handler_definition}` must be a synchronous, monomorphic `(Buffer) -> Bool` function"
    ));
  }
  Ok(Some(ComponentStreamConsumer {
    parameter_index: *parameter_index,
    max_total_bytes,
    max_chunk_bytes,
    handler_definition,
    handler_index: 0,
    total_limit_variant: component_stream_total_limit_variant(result, definition)?,
    stream_read_index: 0,
    stream_cancel_read_index: 0,
    stream_drop_readable_index: 0,
  }))
}

fn collect_component_export_adapters(
  program_data: &program::CompiledProgram,
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  fn_index: &HashMap<String, u32>,
) -> Result<Vec<ComponentExportAdapter>, String> {
  let mut adapters = Vec::new();
  for (namespace, name, args, body) in fn_defs {
    let definition = format!("{namespace}/{name}");
    let Some(compiled) = program_data.get(namespace.as_str()).and_then(|file| file.defs.get(name.as_str())) else {
      continue;
    };
    if !is_wasm_export_def(&compiled.preprocessed_code) {
      continue;
    }
    if !matches!(args, CalcitFnArgs::Args(_)) {
      return Err(format!(
        "E_COMPONENT_ABI_UNSUPPORTED_ARITY: `{definition}` at `logical_schema.parameters` must use fixed arity"
      ));
    }
    let source_arity = fn_param_names(args).len();
    let (parameters, result, invocation) = component_function_schema(compiled, &definition, source_arity, program_data, true, true)?;
    let signature = compiled
      .schema
      .resolve_to_fn()
      .expect("component_function_schema already required a resolved function schema");
    let stream_consumer = component_stream_consumer(&definition, body, args, &signature, &result, program_data)?;
    let target_index = *fn_index
      .get(&definition)
      .ok_or_else(|| format!("E_COMPONENT_ABI_TARGET: compiled target `{definition}` is missing"))?;
    let stackless_tail_import = if invocation == ComponentAbiInvocation::Async && stream_consumer.is_none() {
      direct_tail_import_definition(body, args).filter(|import_definition| {
        let Some((import_namespace, import_name)) = import_definition.rsplit_once('/') else {
          return false;
        };
        let Some(imported) = program_data
          .get(import_namespace)
          .and_then(|file| file.defs.get(import_name))
          .filter(|imported| is_wasm_import_def(&imported.preprocessed_code))
        else {
          return false;
        };
        let Ok(import_signature) = component_function_schema(imported, import_definition, source_arity, program_data, false, false)
        else {
          return false;
        };
        import_signature.2 == ComponentAbiInvocation::Async && import_signature.0 == parameters && import_signature.1 == result
      })
    } else {
      None
    };
    adapters.push(ComponentExportAdapter {
      definition,
      symbol: name.clone(),
      target_index,
      invocation,
      stackless_tail_import,
      stream_consumer,
      task_return_index: None,
      parameters,
      result,
    });
  }
  if adapters.is_empty() {
    return Err("E_COMPONENT_ABI_EXPORTS: no `defwasm-export` definitions were found for the Component boundary".into());
  }
  adapters.sort_unstable_by(|left, right| left.definition.cmp(&right.definition));
  validate_component_export_symbols(&adapters)?;
  Ok(adapters)
}

fn validate_component_export_symbols(adapters: &[ComponentExportAdapter]) -> Result<(), String> {
  let mut symbol_owners = BTreeMap::<String, Vec<&str>>::new();
  for adapter in adapters {
    symbol_owners
      .entry(component_export_emitted_symbol(adapter))
      .or_default()
      .push(adapter.definition.as_str());
    if adapter.stackless_tail_import.is_some() || adapter.stream_consumer.is_some() {
      symbol_owners
        .entry(component_async_callback_symbol(adapter))
        .or_default()
        .push(adapter.definition.as_str());
    }
    if component_export_needs_post_return(adapter) {
      symbol_owners
        .entry(component_post_return_symbol(adapter))
        .or_default()
        .push(adapter.definition.as_str());
    }
  }
  if let Some((symbol, owners)) = symbol_owners.into_iter().find(|(_, owners)| owners.len() > 1) {
    return Err(format!(
      "E_COMPONENT_ABI_SYMBOL_CONFLICT: export symbol `{symbol}` is declared by {}",
      owners.join(", ")
    ));
  }
  Ok(())
}

fn validate_component_import_symbols(adapters: &[ComponentImportAdapter]) -> Result<(), String> {
  let mut symbol_owners = BTreeMap::<(&str, String), Vec<&str>>::new();
  for adapter in adapters {
    symbol_owners
      .entry((adapter.module.as_str(), component_import_emitted_symbol(adapter)))
      .or_default()
      .push(adapter.definition.as_str());
  }
  if let Some(((module, symbol), owners)) = symbol_owners.into_iter().find(|(_, owners)| owners.len() > 1) {
    return Err(format!(
      "E_COMPONENT_ABI_SYMBOL_CONFLICT: import symbol `{module}/{symbol}` is declared by {}",
      owners.join(", ")
    ));
  }
  Ok(())
}

fn component_import_signature(adapter: &ComponentImportAdapter) -> (Vec<ValType>, Vec<ValType>) {
  let mut parameters = Vec::new();
  if component_async_import_uses_indirect_parameters(adapter) {
    parameters.push(ValType::I32);
  } else {
    for parameter in &adapter.parameters {
      parameters.extend(component_flat_types(parameter));
    }
  }
  if adapter.invocation == ComponentAbiInvocation::Async {
    if !matches!(adapter.result, ComponentAbiType::Unit) {
      parameters.push(ValType::I32);
    }
    return (parameters, vec![ValType::I32]);
  }
  let results = component_flat_types(&adapter.result);
  if results.len() > 1 {
    parameters.push(ValType::I32);
    (parameters, vec![])
  } else {
    (parameters, results)
  }
}

fn component_task_return_signature(result: &ComponentAbiType) -> Vec<ValType> {
  let flattened = component_flat_types(result);
  if flattened.len() > COMPONENT_MAX_FLAT_PARAMETERS {
    vec![ValType::I32]
  } else {
    flattened
  }
}

fn component_bool_i32_to_f64(value: u32) -> Vec<Instruction<'static>> {
  vec![
    Instruction::LocalGet(value),
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(value),
    Instruction::F64ConvertI32U,
  ]
}

fn component_bool_f64_to_i32(value: u32, canonical: u32) -> Vec<Instruction<'static>> {
  vec![
    Instruction::LocalGet(value),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(canonical),
    Instruction::F64ConvertI32U,
    Instruction::LocalGet(value),
    Instruction::F64Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(canonical),
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(canonical),
  ]
}

fn component_trap_if(instructions: &mut Vec<Instruction<'static>>) {
  instructions.extend([
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
}

fn component_numeric_to_f64(kind: CalcitNumericRefinement, value: u32) -> Vec<Instruction<'static>> {
  let mut instructions = Vec::new();
  match kind {
    CalcitNumericRefinement::Int8 | CalcitNumericRefinement::Int16 => {
      let (min, max) = match kind {
        CalcitNumericRefinement::Int8 => (-128, 127),
        CalcitNumericRefinement::Int16 => (-32_768, 32_767),
        _ => unreachable!(),
      };
      instructions.extend([Instruction::LocalGet(value), Instruction::I32Const(min), Instruction::I32LtS]);
      component_trap_if(&mut instructions);
      instructions.extend([Instruction::LocalGet(value), Instruction::I32Const(max), Instruction::I32GtS]);
      component_trap_if(&mut instructions);
      instructions.extend([Instruction::LocalGet(value), Instruction::F64ConvertI32S]);
    }
    CalcitNumericRefinement::UInt8 | CalcitNumericRefinement::UInt16 => {
      let max = if kind == CalcitNumericRefinement::UInt8 { 255 } else { 65_535 };
      instructions.extend([Instruction::LocalGet(value), Instruction::I32Const(max), Instruction::I32GtU]);
      component_trap_if(&mut instructions);
      instructions.extend([Instruction::LocalGet(value), Instruction::F64ConvertI32U]);
    }
    CalcitNumericRefinement::Int32 => instructions.extend([Instruction::LocalGet(value), Instruction::F64ConvertI32S]),
    CalcitNumericRefinement::UInt32 => instructions.extend([Instruction::LocalGet(value), Instruction::F64ConvertI32U]),
    CalcitNumericRefinement::Int64 => {
      instructions.extend([
        Instruction::LocalGet(value),
        Instruction::I64Const(-9_007_199_254_740_991),
        Instruction::I64LtS,
      ]);
      component_trap_if(&mut instructions);
      instructions.extend([
        Instruction::LocalGet(value),
        Instruction::I64Const(9_007_199_254_740_991),
        Instruction::I64GtS,
      ]);
      component_trap_if(&mut instructions);
      instructions.extend([Instruction::LocalGet(value), Instruction::F64ConvertI64S]);
    }
    CalcitNumericRefinement::UInt64 => {
      instructions.extend([
        Instruction::LocalGet(value),
        Instruction::I64Const(9_007_199_254_740_991),
        Instruction::I64GtU,
      ]);
      component_trap_if(&mut instructions);
      instructions.extend([Instruction::LocalGet(value), Instruction::F64ConvertI64U]);
    }
    CalcitNumericRefinement::Float32 => {
      instructions.extend([Instruction::LocalGet(value), Instruction::LocalGet(value), Instruction::F32Ne]);
      component_trap_if(&mut instructions);
      instructions.extend([Instruction::LocalGet(value), Instruction::F64PromoteF32]);
    }
    CalcitNumericRefinement::Float64 => instructions.push(Instruction::LocalGet(value)),
  }
  instructions
}

fn component_numeric_from_f64(kind: CalcitNumericRefinement, value: u32) -> Vec<Instruction<'static>> {
  let mut instructions = Vec::new();
  match kind {
    CalcitNumericRefinement::Float64 => instructions.push(Instruction::LocalGet(value)),
    CalcitNumericRefinement::Float32 => {
      instructions.extend([
        Instruction::LocalGet(value),
        Instruction::F32DemoteF64,
        Instruction::F64PromoteF32,
        Instruction::LocalGet(value),
        Instruction::F64Ne,
      ]);
      component_trap_if(&mut instructions);
      instructions.extend([Instruction::LocalGet(value), Instruction::F32DemoteF64]);
    }
    kind => {
      let (min, max) = match kind {
        CalcitNumericRefinement::Int8 => (-128.0, 127.0),
        CalcitNumericRefinement::UInt8 => (0.0, 255.0),
        CalcitNumericRefinement::Int16 => (-32_768.0, 32_767.0),
        CalcitNumericRefinement::UInt16 => (0.0, 65_535.0),
        CalcitNumericRefinement::Int32 => (i32::MIN as f64, i32::MAX as f64),
        CalcitNumericRefinement::UInt32 => (0.0, u32::MAX as f64),
        CalcitNumericRefinement::Int64 => (-9_007_199_254_740_991.0, 9_007_199_254_740_991.0),
        CalcitNumericRefinement::UInt64 => (0.0, 9_007_199_254_740_991.0),
        _ => unreachable!(),
      };
      instructions.extend([Instruction::LocalGet(value), f64_const(min), Instruction::F64Lt]);
      component_trap_if(&mut instructions);
      instructions.extend([Instruction::LocalGet(value), f64_const(max), Instruction::F64Gt]);
      component_trap_if(&mut instructions);
      instructions.extend([
        Instruction::LocalGet(value),
        Instruction::F64Trunc,
        Instruction::LocalGet(value),
        Instruction::F64Ne,
      ]);
      component_trap_if(&mut instructions);
      instructions.push(Instruction::LocalGet(value));
      instructions.push(match kind {
        CalcitNumericRefinement::Int8 | CalcitNumericRefinement::Int16 | CalcitNumericRefinement::Int32 => Instruction::I32TruncF64S,
        CalcitNumericRefinement::UInt8 | CalcitNumericRefinement::UInt16 | CalcitNumericRefinement::UInt32 => Instruction::I32TruncF64U,
        CalcitNumericRefinement::Int64 => Instruction::I64TruncF64S,
        CalcitNumericRefinement::UInt64 => Instruction::I64TruncF64U,
        _ => unreachable!(),
      });
    }
  }
  instructions
}

fn build_cabi_free_fn(free_head_global: u32) -> CompiledFn {
  let header = 2;
  let capacity = 3;
  let instructions = vec![
    Instruction::LocalGet(0),
    Instruction::I32Eqz,
    Instruction::LocalGet(1),
    Instruction::I32Eqz,
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::LocalTee(header),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalTee(capacity),
    Instruction::LocalGet(1),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(header),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Const(-1),
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(header),
    Instruction::GlobalGet(free_head_global),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::LocalGet(header),
    Instruction::GlobalSet(free_head_global),
  ];
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32; 2],
    results: vec![],
    locals: vec![ValType::I32; 2],
    instructions,
  }
}

fn build_cabi_realloc_fn(cabi_free_index: u32, free_head_global: u32) -> CompiledFn {
  let new_ptr = 4;
  let new_end = 5;
  let copy_len = 6;
  let current_pages = 7;
  let current_free = 8;
  let previous_free = 9;
  let next_free = 10;
  let free_capacity = 11;
  let header = 12;
  let instructions = vec![
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)),
    Instruction::LocalGet(0),
    Instruction::LocalGet(1),
    Instruction::Call(cabi_free_index),
    Instruction::I32Const(0),
    Instruction::Else,
    Instruction::LocalGet(2),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(2),
    Instruction::LocalGet(2),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::I32And,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::I32Const(0),
    Instruction::LocalSet(new_ptr),
    Instruction::GlobalGet(free_head_global),
    Instruction::LocalSet(current_free),
    Instruction::I32Const(0),
    Instruction::LocalSet(previous_free),
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(current_free),
    Instruction::I32Eqz,
    Instruction::BrIf(1),
    Instruction::LocalGet(current_free),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalSet(free_capacity),
    Instruction::LocalGet(current_free),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(next_free),
    Instruction::LocalGet(free_capacity),
    Instruction::LocalGet(3),
    Instruction::I32GeU,
    Instruction::LocalGet(current_free),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(2),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::I32And,
    Instruction::I32Eqz,
    Instruction::I32And,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(previous_free),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(next_free),
    Instruction::GlobalSet(free_head_global),
    Instruction::Else,
    Instruction::LocalGet(previous_free),
    Instruction::LocalGet(next_free),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::End,
    Instruction::LocalGet(current_free),
    Instruction::I32Const(-1),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::LocalGet(current_free),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(new_ptr),
    Instruction::Br(2),
    Instruction::End,
    Instruction::LocalGet(current_free),
    Instruction::LocalSet(previous_free),
    Instruction::LocalGet(next_free),
    Instruction::LocalSet(current_free),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(new_ptr),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(2),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::I32Add,
    Instruction::I32Const(0),
    Instruction::LocalGet(2),
    Instruction::I32Sub,
    Instruction::I32And,
    Instruction::LocalTee(new_ptr),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(new_ptr),
    Instruction::LocalGet(3),
    Instruction::I32Add,
    Instruction::LocalTee(new_end),
    Instruction::LocalGet(new_ptr),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::MemorySize(0),
    Instruction::LocalTee(current_pages),
    Instruction::I32Const(65_536),
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(new_end),
    Instruction::LocalGet(current_pages),
    Instruction::I32Const(16),
    Instruction::I32Shl,
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(new_end),
    Instruction::I64ExtendI32U,
    Instruction::LocalGet(current_pages),
    Instruction::I64ExtendI32U,
    Instruction::I64Const(16),
    Instruction::I64Shl,
    Instruction::I64Sub,
    Instruction::I64Const(65_535),
    Instruction::I64Add,
    Instruction::I64Const(16),
    Instruction::I64ShrU,
    Instruction::I32WrapI64,
    Instruction::MemoryGrow(0),
    Instruction::I32Const(-1),
    Instruction::I32Eq,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(new_ptr),
    Instruction::I32Const(8),
    Instruction::I32Sub,
    Instruction::LocalTee(header),
    Instruction::LocalGet(3),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(header),
    Instruction::I32Const(-1),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::LocalGet(new_end),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Eqz,
    Instruction::I32Eqz,
    Instruction::LocalGet(1),
    Instruction::I32Eqz,
    Instruction::I32Eqz,
    Instruction::I32And,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(1),
    Instruction::LocalGet(3),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)),
    Instruction::LocalGet(1),
    Instruction::Else,
    Instruction::LocalGet(3),
    Instruction::End,
    Instruction::LocalSet(copy_len),
    Instruction::LocalGet(new_ptr),
    Instruction::LocalGet(0),
    Instruction::LocalGet(copy_len),
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    Instruction::LocalGet(0),
    Instruction::LocalGet(1),
    Instruction::Call(cabi_free_index),
    Instruction::End,
    Instruction::LocalGet(new_ptr),
    Instruction::End,
  ];
  CompiledFn {
    export_name: Some("cabi_realloc".into()),
    params: vec![ValType::I32; 4],
    results: vec![ValType::I32],
    locals: vec![ValType::I32; 9],
    instructions,
  }
}

fn allocate_component_flat_locals(parameter_count: usize, locals: &mut Vec<ValType>, flat_types: &[ValType]) -> u32 {
  let start = parameter_count as u32 + locals.len() as u32;
  locals.extend(flat_types.iter().copied());
  start
}

fn lower_component_import_parameter_to_flat_locals(
  parameter: &ComponentAbiType,
  parameter_index: u32,
  parameter_count: usize,
  locals: &mut Vec<ValType>,
  instructions: &mut Vec<Instruction<'static>>,
  cabi_realloc_index: u32,
  codecs: &ComponentValueCodecs<'_>,
) -> u32 {
  match parameter {
    ComponentAbiType::Unit => unreachable!("Unit Component parameters are rejected before adapter construction"),
    ComponentAbiType::Bool => {
      let flat_start = allocate_component_flat_locals(parameter_count, locals, &[ValType::I32]);
      instructions.extend(component_bool_f64_to_i32(parameter_index, flat_start));
      instructions.push(Instruction::Drop);
      flat_start
    }
    ComponentAbiType::Number => {
      let flat_start = allocate_component_flat_locals(parameter_count, locals, &[ValType::F64]);
      instructions.extend([Instruction::LocalGet(parameter_index), Instruction::LocalSet(flat_start)]);
      flat_start
    }
    ComponentAbiType::Numeric(kind) => {
      let flat_start = allocate_component_flat_locals(parameter_count, locals, &[component_numeric_flat_type(*kind)]);
      instructions.extend(component_numeric_from_f64(*kind, parameter_index));
      instructions.push(Instruction::LocalSet(flat_start));
      flat_start
    }
    ComponentAbiType::Buffer | ComponentAbiType::String => {
      let flat_start = allocate_component_flat_locals(parameter_count, locals, &[ValType::I32, ValType::I32]);
      instructions.extend([
        Instruction::LocalGet(parameter_index),
        Instruction::I32TruncF64U,
        Instruction::I32Const(8),
        Instruction::I32Add,
        Instruction::LocalSet(flat_start),
        Instruction::LocalGet(parameter_index),
        Instruction::I32TruncF64U,
        Instruction::F64Load(mem_arg_f64(0)),
        Instruction::I32TruncF64U,
        Instruction::LocalSet(flat_start + 1),
      ]);
      flat_start
    }
    ComponentAbiType::List(_) => {
      let pair_ptr = parameter_count as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let flat_start = allocate_component_flat_locals(parameter_count, locals, &[ValType::I32, ValType::I32]);
      let codec = codecs
        .list_codecs
        .get(parameter)
        .expect("Component List import parameter codec must be registered");
      instructions.extend([
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(4),
        Instruction::I32Const(8),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalSet(pair_ptr),
        Instruction::LocalGet(parameter_index),
        Instruction::LocalGet(pair_ptr),
        Instruction::Call(codec.lower_index),
        Instruction::LocalGet(pair_ptr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalSet(flat_start),
        Instruction::LocalGet(pair_ptr),
        Instruction::I32Load(mem_arg_i32(4)),
        Instruction::LocalSet(flat_start + 1),
      ]);
      flat_start
    }
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) | ComponentAbiType::Enum(_) | ComponentAbiType::Struct(_) => {
      let canonical_ptr = parameter_count as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let flat_types = component_flat_types(parameter);
      let flat_start = allocate_component_flat_locals(parameter_count, locals, &flat_types);
      let layout = component_memory_layout(parameter);
      let lower_index = match parameter {
        ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) | ComponentAbiType::Enum(_) => {
          codecs
            .variant_codecs
            .get(parameter)
            .expect("Component variant import parameter codec must be registered")
            .lower_index
        }
        ComponentAbiType::Struct(_) => {
          codecs
            .struct_codecs
            .get(parameter)
            .expect("Component Struct import parameter codec must be registered")
            .lower_index
        }
        _ => unreachable!("compound parameter branch must use a registered codec"),
      };
      instructions.extend([
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(layout.alignment),
        Instruction::I32Const(layout.size),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalSet(canonical_ptr),
        Instruction::LocalGet(parameter_index),
        Instruction::LocalGet(canonical_ptr),
        Instruction::Call(lower_index),
      ]);
      push_load_value_flat(instructions, parameter, canonical_ptr);
      for offset in (0..flat_types.len()).rev() {
        instructions.push(Instruction::LocalSet(flat_start + offset as u32));
      }
      flat_start
    }
  }
}

fn push_component_async_indirect_parameters(
  adapter: &ComponentImportAdapter,
  parameter_count: usize,
  locals: &mut Vec<ValType>,
  instructions: &mut Vec<Instruction<'static>>,
  cabi_realloc_index: u32,
  codecs: &ComponentValueCodecs<'_>,
) {
  let (layout, offsets) = component_fields_layout(adapter.parameters.iter());
  let record_ptr = parameter_count as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  instructions.extend([
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(layout.alignment),
    Instruction::I32Const(layout.size),
    Instruction::Call(cabi_realloc_index),
    Instruction::LocalSet(record_ptr),
  ]);
  for (index, (parameter, offset)) in adapter.parameters.iter().zip(offsets).enumerate() {
    let flat_start = lower_component_import_parameter_to_flat_locals(
      parameter,
      index as u32,
      parameter_count,
      locals,
      instructions,
      cabi_realloc_index,
      codecs,
    );
    push_store_value_flat(instructions, parameter, flat_start, record_ptr, offset);
  }
  instructions.push(Instruction::LocalGet(record_ptr));
}

fn build_component_import_adapter(
  adapter: &ComponentImportAdapter,
  async_imports: Option<&ComponentAsyncCanonicalImports>,
  cabi_realloc_index: u32,
  codecs: &ComponentValueCodecs<'_>,
) -> CompiledFn {
  let params = vec![ValType::F64; adapter.parameters.len()];
  let mut locals = Vec::new();
  let mut instructions = Vec::new();

  if component_async_import_uses_indirect_parameters(adapter) {
    push_component_async_indirect_parameters(adapter, params.len(), &mut locals, &mut instructions, cabi_realloc_index, codecs);
  } else {
    for (index, parameter) in adapter.parameters.iter().enumerate() {
      match parameter {
        ComponentAbiType::Unit => unreachable!("Unit Component parameters are rejected before adapter construction"),
        ComponentAbiType::Bool => {
          let canonical = params.len() as u32 + locals.len() as u32;
          locals.push(ValType::I32);
          instructions.extend(component_bool_f64_to_i32(index as u32, canonical));
        }
        ComponentAbiType::Number => instructions.push(Instruction::LocalGet(index as u32)),
        ComponentAbiType::Numeric(kind) => instructions.extend(component_numeric_from_f64(*kind, index as u32)),
        ComponentAbiType::Buffer | ComponentAbiType::String => instructions.extend([
          Instruction::LocalGet(index as u32),
          Instruction::I32TruncF64U,
          Instruction::I32Const(8),
          Instruction::I32Add,
          Instruction::LocalGet(index as u32),
          Instruction::I32TruncF64U,
          Instruction::F64Load(mem_arg_f64(0)),
          Instruction::I32TruncF64U,
        ]),
        ComponentAbiType::List(_) => {
          let pair_ptr = params.len() as u32 + locals.len() as u32;
          locals.push(ValType::I32);
          let codec = codecs
            .list_codecs
            .get(parameter)
            .expect("Component List import parameter codec must be registered");
          instructions.extend([
            Instruction::I32Const(0),
            Instruction::I32Const(0),
            Instruction::I32Const(4),
            Instruction::I32Const(8),
            Instruction::Call(cabi_realloc_index),
            Instruction::LocalSet(pair_ptr),
            Instruction::LocalGet(index as u32),
            Instruction::LocalGet(pair_ptr),
            Instruction::Call(codec.lower_index),
            Instruction::LocalGet(pair_ptr),
            Instruction::I32Load(mem_arg_i32(0)),
            Instruction::LocalGet(pair_ptr),
            Instruction::I32Load(mem_arg_i32(4)),
          ]);
        }
        ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) | ComponentAbiType::Enum(_) => {
          let canonical_ptr = params.len() as u32 + locals.len() as u32;
          locals.push(ValType::I32);
          let layout = component_memory_layout(parameter);
          let codec = codecs
            .variant_codecs
            .get(parameter)
            .expect("Component variant import parameter codec must be registered");
          instructions.extend([
            Instruction::I32Const(0),
            Instruction::I32Const(0),
            Instruction::I32Const(layout.alignment),
            Instruction::I32Const(layout.size),
            Instruction::Call(cabi_realloc_index),
            Instruction::LocalSet(canonical_ptr),
            Instruction::LocalGet(index as u32),
            Instruction::LocalGet(canonical_ptr),
            Instruction::Call(codec.lower_index),
          ]);
          push_load_value_flat(&mut instructions, parameter, canonical_ptr);
        }
        ComponentAbiType::Struct(_) => {
          let canonical_ptr = params.len() as u32 + locals.len() as u32;
          locals.push(ValType::I32);
          let layout = component_memory_layout(parameter);
          let codec = codecs
            .struct_codecs
            .get(parameter)
            .expect("Component Struct import parameter codec must be registered");
          instructions.extend([
            Instruction::I32Const(0),
            Instruction::I32Const(0),
            Instruction::I32Const(layout.alignment),
            Instruction::I32Const(layout.size),
            Instruction::Call(cabi_realloc_index),
            Instruction::LocalSet(canonical_ptr),
            Instruction::LocalGet(index as u32),
            Instruction::LocalGet(canonical_ptr),
            Instruction::Call(codec.lower_index),
          ]);
          push_load_value_flat(&mut instructions, parameter, canonical_ptr);
        }
      }
    }
  }

  if adapter.invocation == ComponentAbiInvocation::Async {
    return finish_component_async_import_adapter(
      adapter,
      async_imports.expect("async Component imports must register canonical lifecycle builtins"),
      params,
      locals,
      instructions,
      cabi_realloc_index,
      codecs,
    );
  }

  match &adapter.result {
    ComponentAbiType::Unit => {
      instructions.push(Instruction::Call(adapter.raw_index));
      instructions.push(f64_const(0.0));
    }
    ComponentAbiType::Bool => {
      instructions.push(Instruction::Call(adapter.raw_index));
      let canonical = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      instructions.push(Instruction::LocalSet(canonical));
      instructions.extend(component_bool_i32_to_f64(canonical));
    }
    ComponentAbiType::Number => instructions.push(Instruction::Call(adapter.raw_index)),
    ComponentAbiType::Numeric(kind) => {
      instructions.push(Instruction::Call(adapter.raw_index));
      let canonical = params.len() as u32 + locals.len() as u32;
      locals.push(component_numeric_flat_type(*kind));
      instructions.push(Instruction::LocalSet(canonical));
      instructions.extend(component_numeric_to_f64(*kind, canonical));
    }
    ComponentAbiType::Buffer | ComponentAbiType::String => {
      let ret_ptr = params.len() as u32;
      locals.push(ValType::I32);
      let bytes_new_index = match &adapter.result {
        ComponentAbiType::Buffer => codecs.buffer_new_index,
        ComponentAbiType::String => codecs.str_new_index,
        _ => unreachable!("byte result branch must use a byte-backed Component type"),
      };
      instructions.extend([
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(4),
        Instruction::I32Const(8),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalTee(ret_ptr),
        Instruction::Call(adapter.raw_index),
        Instruction::LocalGet(ret_ptr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalGet(ret_ptr),
        Instruction::I32Load(mem_arg_i32(4)),
        Instruction::Call(bytes_new_index),
      ]);
    }
    ComponentAbiType::List(_) => {
      let ret_ptr = params.len() as u32;
      locals.push(ValType::I32);
      let codec = codecs
        .list_codecs
        .get(&adapter.result)
        .expect("Component List import result codec must be registered");
      instructions.extend([
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(4),
        Instruction::I32Const(8),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalTee(ret_ptr),
        Instruction::Call(adapter.raw_index),
        Instruction::LocalGet(ret_ptr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalGet(ret_ptr),
        Instruction::I32Load(mem_arg_i32(4)),
        Instruction::Call(codec.lift_index),
      ]);
    }
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) | ComponentAbiType::Enum(_) => {
      let ret_ptr = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let layout = component_memory_layout(&adapter.result);
      let codec = codecs
        .variant_codecs
        .get(&adapter.result)
        .expect("Component variant import result codec must be registered");
      instructions.extend([
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(layout.alignment),
        Instruction::I32Const(layout.size),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalSet(ret_ptr),
      ]);
      if component_flat_types(&adapter.result).len() > 1 {
        instructions.push(Instruction::LocalGet(ret_ptr));
        instructions.push(Instruction::Call(adapter.raw_index));
      } else {
        let discriminant = params.len() as u32 + locals.len() as u32;
        locals.push(ValType::I32);
        instructions.extend([Instruction::Call(adapter.raw_index), Instruction::LocalSet(discriminant)]);
        push_store_value_flat(&mut instructions, &adapter.result, discriminant, ret_ptr, 0);
      }
      instructions.extend([Instruction::LocalGet(ret_ptr), Instruction::Call(codec.lift_index)]);
    }
    ComponentAbiType::Struct(_) => {
      let ret_ptr = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let layout = component_memory_layout(&adapter.result);
      let codec = codecs
        .struct_codecs
        .get(&adapter.result)
        .expect("Component Struct import result codec must be registered");
      instructions.extend([
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(layout.alignment),
        Instruction::I32Const(layout.size),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalSet(ret_ptr),
      ]);
      let flat_types = component_flat_types(&adapter.result);
      if flat_types.len() > 1 {
        instructions.extend([Instruction::LocalGet(ret_ptr), Instruction::Call(adapter.raw_index)]);
      } else if let Some(flat_type) = flat_types.first().copied() {
        let result_local = params.len() as u32 + locals.len() as u32;
        locals.push(flat_type);
        instructions.extend([Instruction::Call(adapter.raw_index), Instruction::LocalSet(result_local)]);
        push_store_value_flat(&mut instructions, &adapter.result, result_local, ret_ptr, 0);
      } else {
        instructions.push(Instruction::Call(adapter.raw_index));
      }
      instructions.extend([Instruction::LocalGet(ret_ptr), Instruction::Call(codec.lift_index)]);
    }
  }

  CompiledFn {
    export_name: None,
    params,
    results: vec![ValType::F64],
    locals,
    instructions,
  }
}

fn finish_component_async_import_adapter(
  adapter: &ComponentImportAdapter,
  canonical: &ComponentAsyncCanonicalImports,
  params: Vec<ValType>,
  mut locals: Vec<ValType>,
  mut instructions: Vec<Instruction<'static>>,
  cabi_realloc_index: u32,
  codecs: &ComponentValueCodecs<'_>,
) -> CompiledFn {
  let ret_ptr = if matches!(adapter.result, ComponentAbiType::Unit) {
    None
  } else {
    let local = params.len() as u32 + locals.len() as u32;
    locals.push(ValType::I32);
    let layout = component_memory_layout(&adapter.result);
    instructions.extend([
      Instruction::I32Const(0),
      Instruction::I32Const(0),
      Instruction::I32Const(layout.alignment),
      Instruction::I32Const(layout.size),
      Instruction::Call(cabi_realloc_index),
      Instruction::LocalTee(local),
    ]);
    Some(local)
  };

  let status = params.len() as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  let status_kind = params.len() as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  instructions.extend([
    Instruction::Call(adapter.raw_index),
    Instruction::LocalTee(status),
    Instruction::I32Const(0x0f),
    Instruction::I32And,
    Instruction::LocalSet(status_kind),
    Instruction::LocalGet(status_kind),
    Instruction::I32Const(2),
    Instruction::I32Eq,
    Instruction::If(wasm_encoder::BlockType::Empty),
    // Immediate completion has no subtask in the high status bits.
    Instruction::LocalGet(status),
    Instruction::I32Const(2),
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::Else,
    // Only the starting and started states may carry a subtask.
    Instruction::LocalGet(status_kind),
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);

  let subtask = params.len() as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  let waitable_set = params.len() as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  let event_ptr = params.len() as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  let subtask_state = params.len() as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  instructions.extend([
    Instruction::LocalGet(status),
    Instruction::I32Const(4),
    Instruction::I32ShrU,
    Instruction::LocalTee(subtask),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::Call(canonical.waitable_set_new),
    Instruction::LocalSet(waitable_set),
    Instruction::LocalGet(subtask),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_join),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(8),
    Instruction::I32Const(8),
    Instruction::Call(cabi_realloc_index),
    Instruction::LocalSet(event_ptr),
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(waitable_set),
    Instruction::LocalGet(event_ptr),
    Instruction::Call(canonical.waitable_set_wait),
    Instruction::I32Const(1),
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event_ptr),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalGet(subtask),
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event_ptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalTee(subtask_state),
    Instruction::I32Const(2),
    Instruction::I32Eq,
    Instruction::BrIf(1),
    Instruction::LocalGet(subtask_state),
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    // Cancellation is terminal. Drop the resolved handles before trapping,
    // since Calcit does not expose a cancellation value at this boundary.
    Instruction::LocalGet(subtask),
    Instruction::I32Const(0),
    Instruction::Call(canonical.waitable_join),
    Instruction::LocalGet(subtask),
    Instruction::Call(canonical.subtask_drop),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_set_drop),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(subtask),
    Instruction::I32Const(0),
    Instruction::Call(canonical.waitable_join),
    Instruction::LocalGet(subtask),
    Instruction::Call(canonical.subtask_drop),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_set_drop),
    Instruction::End,
  ]);

  match &adapter.result {
    ComponentAbiType::Unit => instructions.push(f64_const(0.0)),
    ComponentAbiType::Bool => {
      let value = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      instructions.extend([
        Instruction::LocalGet(ret_ptr.expect("non-Unit async result must have a return area")),
        Instruction::I32Load8U(mem_arg_byte(0)),
        Instruction::LocalSet(value),
      ]);
      instructions.extend(component_bool_i32_to_f64(value));
    }
    ComponentAbiType::Number => instructions.extend([
      Instruction::LocalGet(ret_ptr.expect("Number async result must have a return area")),
      Instruction::F64Load(mem_arg_f64(0)),
    ]),
    ComponentAbiType::Numeric(kind) => {
      let value = params.len() as u32 + locals.len() as u32;
      locals.push(component_numeric_flat_type(*kind));
      push_load_value_flat(
        &mut instructions,
        &adapter.result,
        ret_ptr.expect("numeric async result must have a return area"),
      );
      instructions.push(Instruction::LocalSet(value));
      instructions.extend(component_numeric_to_f64(*kind, value));
    }
    ComponentAbiType::Buffer | ComponentAbiType::String => {
      let bytes_new_index = if matches!(adapter.result, ComponentAbiType::Buffer) {
        codecs.buffer_new_index
      } else {
        codecs.str_new_index
      };
      let ptr = ret_ptr.expect("byte-backed async result must have a return area");
      instructions.extend([
        Instruction::LocalGet(ptr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalGet(ptr),
        Instruction::I32Load(mem_arg_i32(4)),
        Instruction::Call(bytes_new_index),
      ]);
    }
    ComponentAbiType::List(_) => {
      let ptr = ret_ptr.expect("List async result must have a return area");
      let codec = codecs
        .list_codecs
        .get(&adapter.result)
        .expect("Component List async import result codec must be registered");
      instructions.extend([
        Instruction::LocalGet(ptr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalGet(ptr),
        Instruction::I32Load(mem_arg_i32(4)),
        Instruction::Call(codec.lift_index),
      ]);
    }
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) | ComponentAbiType::Enum(_) => {
      let codec = codecs
        .variant_codecs
        .get(&adapter.result)
        .expect("Component variant async import result codec must be registered");
      instructions.extend([
        Instruction::LocalGet(ret_ptr.expect("variant async result must have a return area")),
        Instruction::Call(codec.lift_index),
      ]);
    }
    ComponentAbiType::Struct(_) => {
      let codec = codecs
        .struct_codecs
        .get(&adapter.result)
        .expect("Component Struct async import result codec must be registered");
      instructions.extend([
        Instruction::LocalGet(ret_ptr.expect("Struct async result must have a return area")),
        Instruction::Call(codec.lift_index),
      ]);
    }
  }

  CompiledFn {
    export_name: None,
    params,
    results: vec![ValType::F64],
    locals,
    instructions,
  }
}

fn finish_component_async_export(
  adapter: &ComponentExportAdapter,
  params_len: usize,
  locals: &mut Vec<ValType>,
  instructions: &mut Vec<Instruction<'static>>,
  runtime: &ComponentExportRuntime<'_>,
) {
  let codecs = &runtime.codecs;
  let cabi_realloc_index = codecs.cabi_realloc_index;
  let task_return_index = adapter
    .task_return_index
    .expect("async Component export must have a task.return import");
  let mut result_area = None;
  match &adapter.result {
    ComponentAbiType::Unit => instructions.push(Instruction::Drop),
    ComponentAbiType::Bool => {
      let value = params_len as u32 + locals.len() as u32;
      locals.push(ValType::F64);
      let canonical = params_len as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      instructions.push(Instruction::LocalSet(value));
      instructions.extend(component_bool_f64_to_i32(value, canonical));
    }
    ComponentAbiType::Number => {}
    ComponentAbiType::Numeric(kind) => {
      let value = params_len as u32 + locals.len() as u32;
      locals.push(ValType::F64);
      instructions.push(Instruction::LocalSet(value));
      instructions.extend(component_numeric_from_f64(*kind, value));
    }
    ComponentAbiType::Buffer | ComponentAbiType::String => {
      let value = params_len as u32 + locals.len() as u32;
      locals.push(ValType::F64);
      let value_ptr = params_len as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let len = params_len as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let data_ptr = params_len as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let pair_ptr = params_len as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      instructions.extend([
        Instruction::LocalTee(value),
        Instruction::I32TruncF64U,
        Instruction::LocalSet(value_ptr),
        Instruction::LocalGet(value_ptr),
        Instruction::F64Load(mem_arg_f64(0)),
        Instruction::I32TruncF64U,
        Instruction::LocalSet(len),
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(1),
        Instruction::LocalGet(len),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalSet(data_ptr),
        Instruction::LocalGet(data_ptr),
        Instruction::LocalGet(value_ptr),
        Instruction::I32Const(8),
        Instruction::I32Add,
        Instruction::LocalGet(len),
        Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(4),
        Instruction::I32Const(8),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalTee(pair_ptr),
        Instruction::LocalGet(data_ptr),
        Instruction::I32Store(mem_arg_i32(0)),
        Instruction::LocalGet(pair_ptr),
        Instruction::LocalGet(len),
        Instruction::I32Store(mem_arg_i32(4)),
        Instruction::LocalGet(pair_ptr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalGet(pair_ptr),
        Instruction::I32Load(mem_arg_i32(4)),
      ]);
      result_area = Some(pair_ptr);
    }
    ComponentAbiType::List(_) => {
      let value = params_len as u32 + locals.len() as u32;
      locals.push(ValType::F64);
      let pair_ptr = params_len as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let codec = codecs
        .list_codecs
        .get(&adapter.result)
        .expect("Component List async export result codec must be registered");
      instructions.extend([
        Instruction::LocalSet(value),
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(4),
        Instruction::I32Const(8),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalSet(pair_ptr),
        Instruction::LocalGet(value),
        Instruction::LocalGet(pair_ptr),
        Instruction::Call(codec.lower_index),
        Instruction::LocalGet(pair_ptr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalGet(pair_ptr),
        Instruction::I32Load(mem_arg_i32(4)),
      ]);
      result_area = Some(pair_ptr);
    }
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) | ComponentAbiType::Enum(_) | ComponentAbiType::Struct(_) => {
      let value = params_len as u32 + locals.len() as u32;
      locals.push(ValType::F64);
      let result_ptr = params_len as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let layout = component_memory_layout(&adapter.result);
      instructions.extend([
        Instruction::LocalSet(value),
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(layout.alignment),
        Instruction::I32Const(layout.size),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalSet(result_ptr),
        Instruction::LocalGet(value),
        Instruction::LocalGet(result_ptr),
      ]);
      let lower_index = match &adapter.result {
        ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) | ComponentAbiType::Enum(_) => {
          codecs
            .variant_codecs
            .get(&adapter.result)
            .expect("Component variant async export result codec must be registered")
            .lower_index
        }
        ComponentAbiType::Struct(_) => {
          codecs
            .struct_codecs
            .get(&adapter.result)
            .expect("Component Struct async export result codec must be registered")
            .lower_index
        }
        _ => unreachable!(),
      };
      instructions.push(Instruction::Call(lower_index));
      if component_flat_types(&adapter.result).len() > COMPONENT_MAX_FLAT_PARAMETERS {
        instructions.push(Instruction::LocalGet(result_ptr));
      } else {
        push_load_value_flat(instructions, &adapter.result, result_ptr);
      }
      result_area = Some(result_ptr);
    }
  }
  instructions.push(Instruction::Call(task_return_index));
  if let Some(result_area) = result_area {
    push_component_result_reclaim(
      &adapter.result,
      result_area,
      runtime.cabi_free_index,
      runtime.drop_indices,
      instructions,
    );
  }
}

fn push_component_task_return_from_area(result: &ComponentAbiType, ret_ptr: Option<u32>, instructions: &mut Vec<Instruction<'static>>) {
  if matches!(result, ComponentAbiType::Unit) {
    return;
  }
  let ret_ptr = ret_ptr.expect("non-Unit stackless result must have a return area");
  if component_flat_types(result).len() > COMPONENT_MAX_FLAT_PARAMETERS {
    instructions.push(Instruction::LocalGet(ret_ptr));
  } else {
    push_load_value_flat(instructions, result, ret_ptr);
  }
}

fn push_component_stackless_cleanup(
  canonical: &ComponentAsyncCanonicalImports,
  subtask: u32,
  waitable_set: u32,
  instructions: &mut Vec<Instruction<'static>>,
) {
  instructions.extend([
    Instruction::LocalGet(subtask),
    Instruction::I32Const(0),
    Instruction::Call(canonical.waitable_join),
    Instruction::LocalGet(subtask),
    Instruction::Call(canonical.subtask_drop),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_set_drop),
  ]);
}

const STREAM_STATE_HANDLE: u64 = 0;
const STREAM_STATE_WAITABLE_SET: u64 = 4;
const STREAM_STATE_CHUNK_PTR: u64 = 8;
const STREAM_STATE_TOTAL: u64 = 12;
const STREAM_STATE_CANCEL_REQUESTED: u64 = 16;
const STREAM_STATE_SIZE: i32 = 20;

fn push_component_stream_cleanup(
  consumer: &ComponentStreamConsumer,
  canonical: &ComponentAsyncCanonicalImports,
  stackless: &ComponentStacklessCanonicalImports,
  cabi_free_index: u32,
  state: u32,
  instructions: &mut Vec<Instruction<'static>>,
) {
  instructions.extend([
    Instruction::LocalGet(state),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_HANDLE)),
    Instruction::I32Const(0),
    Instruction::Call(canonical.waitable_join),
    Instruction::LocalGet(state),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_HANDLE)),
    Instruction::Call(consumer.stream_drop_readable_index),
    Instruction::LocalGet(state),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_WAITABLE_SET)),
    Instruction::Call(canonical.waitable_set_drop),
    Instruction::I32Const(0),
    Instruction::Call(stackless.context_set),
    Instruction::LocalGet(state),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_CHUNK_PTR)),
    Instruction::I32Const(consumer.max_chunk_bytes as i32),
    Instruction::Call(cabi_free_index),
    Instruction::LocalGet(state),
    Instruction::I32Const(STREAM_STATE_SIZE),
    Instruction::Call(cabi_free_index),
  ]);
}

fn push_component_stream_task_return(
  adapter: &ComponentExportAdapter,
  result_discriminant: i32,
  error_variant: i32,
  instructions: &mut Vec<Instruction<'static>>,
) {
  instructions.extend([
    Instruction::I32Const(result_discriminant),
    Instruction::I32Const(error_variant),
    Instruction::Call(adapter.task_return_index.expect("stream export must have task.return")),
    Instruction::I32Const(0),
    Instruction::Return,
  ]);
}

fn build_component_stream_export(
  adapter: &ComponentExportAdapter,
  canonical: &ComponentAsyncCanonicalImports,
  stackless: &ComponentStacklessCanonicalImports,
  buffer_new_index: u32,
  cabi_free_index: u32,
  cabi_realloc_index: u32,
  step_index: u32,
) -> [CompiledFn; 3] {
  let consumer = adapter
    .stream_consumer
    .as_ref()
    .expect("stream export builder requires a stream consumer");

  // step(state, return-code) drives immediate reads in a loop and yields only
  // while exactly one canonical stream read is outstanding.
  let step_params = vec![ValType::I32, ValType::I32];
  let mut step_locals = Vec::new();
  let local = |locals: &mut Vec<ValType>, ty| {
    let index = step_params.len() as u32 + locals.len() as u32;
    locals.push(ty);
    index
  };
  let kind = local(&mut step_locals, ValType::I32);
  let count = local(&mut step_locals, ValType::I32);
  let total = local(&mut step_locals, ValType::I32);
  let continue_reading = local(&mut step_locals, ValType::I32);
  let handler_result = local(&mut step_locals, ValType::F64);
  let read_len = local(&mut step_locals, ValType::I32);
  let mut step = vec![Instruction::Loop(wasm_encoder::BlockType::Empty)];
  step.extend([
    Instruction::LocalGet(1),
    Instruction::I32Const(-1),
    Instruction::I32Eq,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_WAITABLE_SET)),
    Instruction::I32Const(4),
    Instruction::I32Shl,
    Instruction::I32Const(2),
    Instruction::I32Or,
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(1),
    Instruction::I32Const(0x0f),
    Instruction::I32And,
    Instruction::LocalTee(kind),
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(1),
    Instruction::I32Const(4),
    Instruction::I32ShrU,
    Instruction::LocalSet(count),
    Instruction::I32Const(1),
    Instruction::LocalSet(continue_reading),
    Instruction::LocalGet(0),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_TOTAL)),
    Instruction::LocalSet(total),
    Instruction::LocalGet(count),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
  ]);
  // A zero-item completed read cannot make progress and would otherwise spin
  // inside this core call. Treat it as a terminal empty read, just like EOF.
  push_component_stream_cleanup(consumer, canonical, stackless, cabi_free_index, 0, &mut step);
  push_component_stream_task_return(adapter, 0, 0, &mut step);
  step.extend([
    Instruction::Else,
    Instruction::LocalGet(total),
    Instruction::LocalGet(count),
    Instruction::I32Add,
    Instruction::LocalTee(total),
    Instruction::I32Const(consumer.max_total_bytes as i32),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
  ]);
  push_component_stream_cleanup(consumer, canonical, stackless, cabi_free_index, 0, &mut step);
  push_component_stream_task_return(adapter, 1, consumer.total_limit_variant as i32, &mut step);
  step.extend([
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::LocalGet(total),
    Instruction::I32Store(mem_arg_i32(STREAM_STATE_TOTAL)),
    Instruction::LocalGet(0),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_CHUNK_PTR)),
    Instruction::LocalGet(count),
    Instruction::Call(buffer_new_index),
    Instruction::Call(consumer.handler_index),
    Instruction::LocalTee(handler_result),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(continue_reading),
    Instruction::F64ConvertI32U,
    Instruction::LocalGet(handler_result),
    Instruction::F64Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(continue_reading),
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(kind),
    Instruction::I32Const(1),
    Instruction::I32Eq,
    Instruction::LocalGet(continue_reading),
    Instruction::I32Eqz,
    Instruction::I32Or,
    Instruction::If(wasm_encoder::BlockType::Empty),
  ]);
  push_component_stream_cleanup(consumer, canonical, stackless, cabi_free_index, 0, &mut step);
  push_component_stream_task_return(adapter, 0, 0, &mut step);
  step.extend([
    Instruction::End,
    Instruction::I32Const(consumer.max_total_bytes as i32),
    Instruction::LocalGet(total),
    Instruction::I32Sub,
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::I32Const(consumer.max_chunk_bytes as i32),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)),
    Instruction::I32Const(consumer.max_total_bytes as i32),
    Instruction::LocalGet(total),
    Instruction::I32Sub,
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::Else,
    Instruction::I32Const(consumer.max_chunk_bytes as i32),
    Instruction::End,
    Instruction::LocalSet(read_len),
    Instruction::LocalGet(0),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_HANDLE)),
    Instruction::LocalGet(0),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_CHUNK_PTR)),
    Instruction::LocalGet(read_len),
    Instruction::Call(consumer.stream_read_index),
    Instruction::LocalSet(1),
    Instruction::Br(0),
    Instruction::End,
    Instruction::Unreachable,
  ]);
  let step_fn = CompiledFn {
    export_name: None,
    params: step_params,
    results: vec![ValType::I32],
    locals: step_locals,
    instructions: step,
  };

  let entry_params = vec![ValType::I32];
  let entry_locals = vec![ValType::I32, ValType::I32, ValType::I32];
  let state = 1;
  let waitable_set = 2;
  let chunk_ptr = 3;
  let entry = vec![
    Instruction::Call(canonical.waitable_set_new),
    Instruction::LocalSet(waitable_set),
    Instruction::LocalGet(0),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_join),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(1),
    Instruction::I32Const(consumer.max_chunk_bytes as i32),
    Instruction::Call(cabi_realloc_index),
    Instruction::LocalSet(chunk_ptr),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(4),
    Instruction::I32Const(STREAM_STATE_SIZE),
    Instruction::Call(cabi_realloc_index),
    Instruction::LocalSet(state),
    Instruction::LocalGet(state),
    Instruction::LocalGet(0),
    Instruction::I32Store(mem_arg_i32(STREAM_STATE_HANDLE)),
    Instruction::LocalGet(state),
    Instruction::LocalGet(waitable_set),
    Instruction::I32Store(mem_arg_i32(STREAM_STATE_WAITABLE_SET)),
    Instruction::LocalGet(state),
    Instruction::LocalGet(chunk_ptr),
    Instruction::I32Store(mem_arg_i32(STREAM_STATE_CHUNK_PTR)),
    Instruction::LocalGet(state),
    Instruction::I32Const(0),
    Instruction::I32Store(mem_arg_i32(STREAM_STATE_TOTAL)),
    Instruction::LocalGet(state),
    Instruction::I32Const(0),
    Instruction::I32Store(mem_arg_i32(STREAM_STATE_CANCEL_REQUESTED)),
    Instruction::LocalGet(state),
    Instruction::Call(stackless.context_set),
    Instruction::LocalGet(state),
    Instruction::LocalGet(0),
    Instruction::LocalGet(chunk_ptr),
    Instruction::I32Const(consumer.max_chunk_bytes as i32),
    Instruction::Call(consumer.stream_read_index),
    Instruction::Call(step_index),
  ];
  let entry_fn = CompiledFn {
    export_name: Some(component_async_export_symbol(adapter)),
    params: entry_params,
    results: vec![ValType::I32],
    locals: entry_locals,
    instructions: entry,
  };

  let callback_params = vec![ValType::I32; 3];
  let callback_locals = vec![ValType::I32, ValType::I32];
  let callback_state = 3;
  let cancel_status = 4;
  let mut callback = vec![
    Instruction::Call(stackless.context_get),
    Instruction::LocalTee(callback_state),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(callback_state),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_CANCEL_REQUESTED)),
    Instruction::If(wasm_encoder::BlockType::Empty),
  ];
  push_component_stream_cleanup(consumer, canonical, stackless, cabi_free_index, callback_state, &mut callback);
  callback.extend([
    Instruction::Call(stackless.task_cancel),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Const(6),
    Instruction::I32Eq,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(callback_state),
    Instruction::I32Const(1),
    Instruction::I32Store(mem_arg_i32(STREAM_STATE_CANCEL_REQUESTED)),
    Instruction::LocalGet(callback_state),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_HANDLE)),
    Instruction::Call(consumer.stream_cancel_read_index),
    Instruction::LocalTee(cancel_status),
    Instruction::I32Const(-1),
    Instruction::I32Eq,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(callback_state),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_WAITABLE_SET)),
    Instruction::I32Const(4),
    Instruction::I32Shl,
    Instruction::I32Const(2),
    Instruction::I32Or,
    Instruction::Return,
    Instruction::End,
  ]);
  push_component_stream_cleanup(consumer, canonical, stackless, cabi_free_index, callback_state, &mut callback);
  callback.extend([
    Instruction::Call(stackless.task_cancel),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Const(2),
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(1),
    Instruction::LocalGet(callback_state),
    Instruction::I32Load(mem_arg_i32(STREAM_STATE_HANDLE)),
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(callback_state),
    Instruction::LocalGet(2),
    Instruction::Call(step_index),
  ]);
  let callback_fn = CompiledFn {
    export_name: Some(component_async_callback_symbol(adapter)),
    params: callback_params,
    results: vec![ValType::I32],
    locals: callback_locals,
    instructions: callback,
  };
  [step_fn, entry_fn, callback_fn]
}

fn build_component_stackless_tail_export(
  adapter: &ComponentExportAdapter,
  imported: &ComponentImportAdapter,
  canonical: &ComponentAsyncCanonicalImports,
  stackless: &ComponentStacklessCanonicalImports,
  cabi_free_index: u32,
  cabi_realloc_index: u32,
  drop_indices: &BTreeMap<ComponentAbiType, u32>,
) -> [CompiledFn; 2] {
  const STATE_RET_PTR: u64 = 0;
  const STATE_SUBTASK: u64 = 4;
  const STATE_WAITABLE_SET: u64 = 8;
  const STATE_CANCEL_REQUESTED: u64 = 12;
  const STATE_SIZE: i32 = 16;

  let params = adapter.parameters.iter().flat_map(component_flat_types).collect::<Vec<_>>();
  let mut locals = Vec::new();
  let mut instructions = Vec::new();
  let ret_ptr = if matches!(adapter.result, ComponentAbiType::Unit) {
    None
  } else {
    let local = params.len() as u32 + locals.len() as u32;
    locals.push(ValType::I32);
    let layout = component_memory_layout(&adapter.result);
    instructions.extend([
      Instruction::I32Const(0),
      Instruction::I32Const(0),
      Instruction::I32Const(layout.alignment),
      Instruction::I32Const(layout.size),
      Instruction::Call(cabi_realloc_index),
      Instruction::LocalSet(local),
    ]);
    Some(local)
  };

  if component_async_import_uses_indirect_parameters(imported) {
    let (layout, offsets) = component_fields_layout(imported.parameters.iter());
    let record_ptr = params.len() as u32 + locals.len() as u32;
    locals.push(ValType::I32);
    instructions.extend([
      Instruction::I32Const(0),
      Instruction::I32Const(0),
      Instruction::I32Const(layout.alignment),
      Instruction::I32Const(layout.size),
      Instruction::Call(cabi_realloc_index),
      Instruction::LocalSet(record_ptr),
    ]);
    let mut flat_start = 0;
    for ((parameter, offset), width) in imported
      .parameters
      .iter()
      .zip(offsets)
      .zip(imported.parameters.iter().map(|parameter| component_flat_types(parameter).len()))
    {
      push_store_value_flat(&mut instructions, parameter, flat_start, record_ptr, offset);
      flat_start += width as u32;
    }
    instructions.push(Instruction::LocalGet(record_ptr));
  } else {
    for index in 0..params.len() {
      instructions.push(Instruction::LocalGet(index as u32));
    }
  }
  if let Some(ret_ptr) = ret_ptr {
    instructions.push(Instruction::LocalGet(ret_ptr));
  }

  let status = params.len() as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  let status_kind = params.len() as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  let subtask = params.len() as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  let waitable_set = params.len() as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  let state = params.len() as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  instructions.extend([
    Instruction::Call(imported.raw_index),
    Instruction::LocalTee(status),
    Instruction::I32Const(0x0f),
    Instruction::I32And,
    Instruction::LocalTee(status_kind),
    Instruction::I32Const(2),
    Instruction::I32Eq,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)),
    Instruction::LocalGet(status),
    Instruction::I32Const(2),
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
  push_component_task_return_from_area(&adapter.result, ret_ptr, &mut instructions);
  instructions.push(Instruction::Call(
    adapter.task_return_index.expect("stackless export must have task.return"),
  ));
  if let Some(ret_ptr) = ret_ptr {
    push_component_result_reclaim(&adapter.result, ret_ptr, cabi_free_index, drop_indices, &mut instructions);
  }
  instructions.extend([
    Instruction::I32Const(0),
    Instruction::Else,
    Instruction::LocalGet(status_kind),
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(status),
    Instruction::I32Const(4),
    Instruction::I32ShrU,
    Instruction::LocalTee(subtask),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::Call(canonical.waitable_set_new),
    Instruction::LocalSet(waitable_set),
    Instruction::LocalGet(subtask),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_join),
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(4),
    Instruction::I32Const(STATE_SIZE),
    Instruction::Call(cabi_realloc_index),
    Instruction::LocalTee(state),
    Instruction::I32Const(0),
    Instruction::I32Store(mem_arg_i32(STATE_RET_PTR)),
  ]);
  if let Some(ret_ptr) = ret_ptr {
    instructions.extend([
      Instruction::LocalGet(state),
      Instruction::LocalGet(ret_ptr),
      Instruction::I32Store(mem_arg_i32(STATE_RET_PTR)),
    ]);
  }
  instructions.extend([
    Instruction::LocalGet(state),
    Instruction::LocalGet(subtask),
    Instruction::I32Store(mem_arg_i32(STATE_SUBTASK)),
    Instruction::LocalGet(state),
    Instruction::LocalGet(waitable_set),
    Instruction::I32Store(mem_arg_i32(STATE_WAITABLE_SET)),
    Instruction::LocalGet(state),
    Instruction::I32Const(0),
    Instruction::I32Store(mem_arg_i32(STATE_CANCEL_REQUESTED)),
    Instruction::LocalGet(state),
    Instruction::Call(stackless.context_set),
    Instruction::LocalGet(waitable_set),
    Instruction::I32Const(4),
    Instruction::I32Shl,
    Instruction::I32Const(2),
    Instruction::I32Or,
    Instruction::End,
  ]);

  let entry = CompiledFn {
    export_name: Some(component_async_export_symbol(adapter)),
    params,
    results: vec![ValType::I32],
    locals,
    instructions,
  };

  let callback_params = vec![ValType::I32; 3];
  let mut callback_locals = Vec::new();
  let callback_state = callback_params.len() as u32 + callback_locals.len() as u32;
  callback_locals.push(ValType::I32);
  let callback_ret_ptr = callback_params.len() as u32 + callback_locals.len() as u32;
  callback_locals.push(ValType::I32);
  let callback_subtask = callback_params.len() as u32 + callback_locals.len() as u32;
  callback_locals.push(ValType::I32);
  let callback_set = callback_params.len() as u32 + callback_locals.len() as u32;
  callback_locals.push(ValType::I32);
  let cancel_requested = callback_params.len() as u32 + callback_locals.len() as u32;
  callback_locals.push(ValType::I32);
  let cancel_status = callback_params.len() as u32 + callback_locals.len() as u32;
  callback_locals.push(ValType::I32);
  let mut callback_instructions = vec![
    Instruction::Call(stackless.context_get),
    Instruction::LocalTee(callback_state),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(callback_state),
    Instruction::I32Load(mem_arg_i32(STATE_RET_PTR)),
    Instruction::LocalSet(callback_ret_ptr),
    Instruction::LocalGet(callback_state),
    Instruction::I32Load(mem_arg_i32(STATE_SUBTASK)),
    Instruction::LocalSet(callback_subtask),
    Instruction::LocalGet(callback_state),
    Instruction::I32Load(mem_arg_i32(STATE_WAITABLE_SET)),
    Instruction::LocalSet(callback_set),
    Instruction::LocalGet(callback_state),
    Instruction::I32Load(mem_arg_i32(STATE_CANCEL_REQUESTED)),
    Instruction::LocalSet(cancel_requested),
    Instruction::LocalGet(0),
    Instruction::I32Const(6),
    Instruction::I32Eq,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(callback_state),
    Instruction::I32Const(1),
    Instruction::I32Store(mem_arg_i32(STATE_CANCEL_REQUESTED)),
    Instruction::LocalGet(callback_subtask),
    Instruction::I32Const(0),
    Instruction::Call(canonical.waitable_join),
    Instruction::LocalGet(callback_subtask),
    Instruction::Call(stackless.subtask_cancel),
    Instruction::LocalTee(cancel_status),
    Instruction::I32Const(-1),
    Instruction::I32Eq,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(callback_subtask),
    Instruction::LocalGet(callback_set),
    Instruction::Call(canonical.waitable_join),
    Instruction::LocalGet(callback_set),
    Instruction::I32Const(4),
    Instruction::I32Shl,
    Instruction::I32Const(2),
    Instruction::I32Or,
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(cancel_status),
    Instruction::I32Const(2),
    Instruction::I32LtS,
    Instruction::LocalGet(cancel_status),
    Instruction::I32Const(4),
    Instruction::I32GtU,
    Instruction::I32Or,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ];
  push_component_stackless_cleanup(canonical, callback_subtask, callback_set, &mut callback_instructions);
  callback_instructions.extend([
    Instruction::I32Const(0),
    Instruction::Call(stackless.context_set),
    Instruction::LocalGet(callback_state),
    Instruction::I32Const(STATE_SIZE),
    Instruction::Call(cabi_free_index),
  ]);
  if ret_ptr.is_some() {
    callback_instructions.extend([
      Instruction::LocalGet(callback_ret_ptr),
      Instruction::I32Const(component_memory_layout(&adapter.result).size),
      Instruction::Call(cabi_free_index),
    ]);
  }
  callback_instructions.extend([
    Instruction::Call(stackless.task_cancel),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::I32Const(1),
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(1),
    Instruction::LocalGet(callback_subtask),
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(2),
    Instruction::I32Const(1),
    Instruction::I32LeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(callback_set),
    Instruction::I32Const(4),
    Instruction::I32Shl,
    Instruction::I32Const(2),
    Instruction::I32Or,
    Instruction::Return,
    Instruction::End,
  ]);
  push_component_stackless_cleanup(canonical, callback_subtask, callback_set, &mut callback_instructions);
  callback_instructions.extend([
    Instruction::I32Const(0),
    Instruction::Call(stackless.context_set),
    Instruction::LocalGet(callback_state),
    Instruction::I32Const(STATE_SIZE),
    Instruction::Call(cabi_free_index),
    Instruction::LocalGet(cancel_requested),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(2),
    Instruction::I32Const(2),
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
  ]);
  push_component_task_return_from_area(&adapter.result, ret_ptr.map(|_| callback_ret_ptr), &mut callback_instructions);
  callback_instructions.extend([Instruction::Call(
    adapter.task_return_index.expect("stackless callback must have task.return"),
  )]);
  push_component_result_reclaim(
    &adapter.result,
    callback_ret_ptr,
    cabi_free_index,
    drop_indices,
    &mut callback_instructions,
  );
  callback_instructions.push(Instruction::Else);
  if ret_ptr.is_some() {
    callback_instructions.extend([
      Instruction::LocalGet(callback_ret_ptr),
      Instruction::I32Const(component_memory_layout(&adapter.result).size),
      Instruction::Call(cabi_free_index),
    ]);
  }
  callback_instructions.extend([Instruction::Call(stackless.task_cancel), Instruction::End, Instruction::I32Const(0)]);

  let callback = CompiledFn {
    export_name: Some(component_async_callback_symbol(adapter)),
    params: callback_params,
    results: vec![ValType::I32],
    locals: callback_locals,
    instructions: callback_instructions,
  };
  [entry, callback]
}

fn build_component_export_adapter(adapter: &ComponentExportAdapter, runtime: &ComponentExportRuntime<'_>) -> CompiledFn {
  let str_new_index = runtime.codecs.str_new_index;
  let buffer_new_index = runtime.codecs.buffer_new_index;
  let cabi_realloc_index = runtime.codecs.cabi_realloc_index;
  let list_codecs = runtime.codecs.list_codecs;
  let struct_codecs = runtime.codecs.struct_codecs;
  let variant_codecs = runtime.codecs.variant_codecs;
  let mut params = Vec::new();
  for parameter in &adapter.parameters {
    params.extend(component_flat_types(parameter));
  }

  let mut locals = Vec::new();
  let mut instructions = Vec::new();
  let mut flat_index = 0u32;
  let mut lowered = Vec::with_capacity(adapter.parameters.len());
  for parameter in &adapter.parameters {
    match parameter {
      ComponentAbiType::Unit => unreachable!("Unit Component parameters are rejected before adapter construction"),
      ComponentAbiType::Bool => {
        let local = params.len() as u32 + locals.len() as u32;
        locals.push(ValType::F64);
        instructions.extend(component_bool_i32_to_f64(flat_index));
        instructions.push(Instruction::LocalSet(local));
        lowered.push(local);
        flat_index += 1;
      }
      ComponentAbiType::Number => {
        lowered.push(flat_index);
        flat_index += 1;
      }
      ComponentAbiType::Numeric(kind) => {
        let local = params.len() as u32 + locals.len() as u32;
        locals.push(ValType::F64);
        instructions.extend(component_numeric_to_f64(*kind, flat_index));
        instructions.push(Instruction::LocalSet(local));
        lowered.push(local);
        flat_index += 1;
      }
      ComponentAbiType::Buffer | ComponentAbiType::String => {
        let local = params.len() as u32 + locals.len() as u32;
        locals.push(ValType::F64);
        let bytes_new_index = match parameter {
          ComponentAbiType::Buffer => buffer_new_index,
          ComponentAbiType::String => str_new_index,
          _ => unreachable!("byte parameter branch must use a byte-backed Component type"),
        };
        instructions.extend([
          Instruction::LocalGet(flat_index),
          Instruction::LocalGet(flat_index + 1),
          Instruction::Call(bytes_new_index),
          Instruction::LocalSet(local),
        ]);
        lowered.push(local);
        flat_index += 2;
      }
      ComponentAbiType::List(_) => {
        let local = params.len() as u32 + locals.len() as u32;
        locals.push(ValType::F64);
        let codec = list_codecs
          .get(parameter)
          .expect("Component List export parameter codec must be registered");
        instructions.extend([
          Instruction::LocalGet(flat_index),
          Instruction::LocalGet(flat_index + 1),
          Instruction::Call(codec.lift_index),
          Instruction::LocalSet(local),
        ]);
        lowered.push(local);
        flat_index += 2;
      }
      ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) | ComponentAbiType::Enum(_) => {
        let canonical_ptr = params.len() as u32 + locals.len() as u32;
        locals.push(ValType::I32);
        let local = params.len() as u32 + locals.len() as u32;
        locals.push(ValType::F64);
        let layout = component_memory_layout(parameter);
        let codec = variant_codecs
          .get(parameter)
          .expect("Component variant export parameter codec must be registered");
        instructions.extend([
          Instruction::I32Const(0),
          Instruction::I32Const(0),
          Instruction::I32Const(layout.alignment),
          Instruction::I32Const(layout.size),
          Instruction::Call(cabi_realloc_index),
          Instruction::LocalSet(canonical_ptr),
        ]);
        push_store_value_flat(&mut instructions, parameter, flat_index, canonical_ptr, 0);
        instructions.extend([
          Instruction::LocalGet(canonical_ptr),
          Instruction::Call(codec.lift_index),
          Instruction::LocalSet(local),
        ]);
        lowered.push(local);
        flat_index += component_flat_types(parameter).len() as u32;
      }
      ComponentAbiType::Struct(_) => {
        let canonical_ptr = params.len() as u32 + locals.len() as u32;
        locals.push(ValType::I32);
        let local = params.len() as u32 + locals.len() as u32;
        locals.push(ValType::F64);
        let layout = component_memory_layout(parameter);
        let codec = struct_codecs
          .get(parameter)
          .expect("Component Struct export parameter codec must be registered");
        instructions.extend([
          Instruction::I32Const(0),
          Instruction::I32Const(0),
          Instruction::I32Const(layout.alignment),
          Instruction::I32Const(layout.size),
          Instruction::Call(cabi_realloc_index),
          Instruction::LocalSet(canonical_ptr),
        ]);
        push_store_value_flat(&mut instructions, parameter, flat_index, canonical_ptr, 0);
        instructions.extend([
          Instruction::LocalGet(canonical_ptr),
          Instruction::Call(codec.lift_index),
          Instruction::LocalSet(local),
        ]);
        lowered.push(local);
        flat_index += component_flat_types(parameter).len() as u32;
      }
    }
  }
  for local in lowered {
    instructions.push(Instruction::LocalGet(local));
  }
  instructions.push(Instruction::Call(adapter.target_index));

  if adapter.invocation == ComponentAbiInvocation::Async {
    finish_component_async_export(adapter, params.len(), &mut locals, &mut instructions, runtime);
    return CompiledFn {
      export_name: Some(component_export_emitted_symbol(adapter)),
      params,
      results: vec![],
      locals,
      instructions,
    };
  }

  let results = match &adapter.result {
    ComponentAbiType::Unit => {
      instructions.push(Instruction::Drop);
      vec![]
    }
    ComponentAbiType::Bool => {
      let value = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::F64);
      let canonical = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      instructions.push(Instruction::LocalSet(value));
      instructions.extend(component_bool_f64_to_i32(value, canonical));
      vec![ValType::I32]
    }
    ComponentAbiType::Number => vec![ValType::F64],
    ComponentAbiType::Numeric(kind) => {
      let value = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::F64);
      instructions.push(Instruction::LocalSet(value));
      instructions.extend(component_numeric_from_f64(*kind, value));
      vec![component_numeric_flat_type(*kind)]
    }
    ComponentAbiType::Buffer | ComponentAbiType::String => {
      let value = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::F64);
      let value_ptr = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let len = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let data_ptr = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let ret_ptr = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      instructions.extend([
        Instruction::LocalTee(value),
        Instruction::I32TruncF64U,
        Instruction::LocalSet(value_ptr),
        Instruction::LocalGet(value_ptr),
        Instruction::F64Load(mem_arg_f64(0)),
        Instruction::I32TruncF64U,
        Instruction::LocalSet(len),
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(1),
        Instruction::LocalGet(len),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalTee(data_ptr),
        Instruction::LocalGet(value_ptr),
        Instruction::I32Const(8),
        Instruction::I32Add,
        Instruction::LocalGet(len),
        Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(4),
        Instruction::I32Const(8),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalTee(ret_ptr),
        Instruction::LocalGet(data_ptr),
        Instruction::I32Store(mem_arg_i32(0)),
        Instruction::LocalGet(ret_ptr),
        Instruction::LocalGet(len),
        Instruction::I32Store(mem_arg_i32(4)),
        Instruction::LocalGet(ret_ptr),
      ]);
      vec![ValType::I32]
    }
    ComponentAbiType::List(_) => {
      let value = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::F64);
      let ret_ptr = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let codec = list_codecs
        .get(&adapter.result)
        .expect("Component List export result codec must be registered");
      instructions.extend([
        Instruction::LocalSet(value),
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(4),
        Instruction::I32Const(8),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalSet(ret_ptr),
        Instruction::LocalGet(value),
        Instruction::LocalGet(ret_ptr),
        Instruction::Call(codec.lower_index),
        Instruction::LocalGet(ret_ptr),
      ]);
      vec![ValType::I32]
    }
    ComponentAbiType::Option(_) | ComponentAbiType::Result(_, _) | ComponentAbiType::Enum(_) => {
      let value = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::F64);
      let ret_ptr = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let layout = component_memory_layout(&adapter.result);
      let codec = variant_codecs
        .get(&adapter.result)
        .expect("Component variant export result codec must be registered");
      instructions.extend([
        Instruction::LocalSet(value),
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(layout.alignment),
        Instruction::I32Const(layout.size),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalSet(ret_ptr),
        Instruction::LocalGet(value),
        Instruction::LocalGet(ret_ptr),
        Instruction::Call(codec.lower_index),
      ]);
      let flat_types = component_flat_types(&adapter.result);
      if flat_types.len() > 1 {
        instructions.push(Instruction::LocalGet(ret_ptr));
        vec![ValType::I32]
      } else {
        push_load_value_flat(&mut instructions, &adapter.result, ret_ptr);
        flat_types
      }
    }
    ComponentAbiType::Struct(_) => {
      let value = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::F64);
      let ret_ptr = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let layout = component_memory_layout(&adapter.result);
      let codec = struct_codecs
        .get(&adapter.result)
        .expect("Component Struct export result codec must be registered");
      instructions.extend([
        Instruction::LocalSet(value),
        Instruction::I32Const(0),
        Instruction::I32Const(0),
        Instruction::I32Const(layout.alignment),
        Instruction::I32Const(layout.size),
        Instruction::Call(cabi_realloc_index),
        Instruction::LocalSet(ret_ptr),
        Instruction::LocalGet(value),
        Instruction::LocalGet(ret_ptr),
        Instruction::Call(codec.lower_index),
      ]);
      let flat_types = component_flat_types(&adapter.result);
      if flat_types.len() > 1 {
        instructions.push(Instruction::LocalGet(ret_ptr));
        vec![ValType::I32]
      } else {
        push_load_value_flat(&mut instructions, &adapter.result, ret_ptr);
        flat_types
      }
    }
  };

  CompiledFn {
    export_name: Some(component_export_emitted_symbol(adapter)),
    params,
    results,
    locals,
    instructions,
  }
}

#[derive(Clone)]
struct WasmCompileEnv {
  fn_index: HashMap<String, u32>,
  fn_arity: HashMap<String, u32>,
  fn_has_rest: HashMap<String, u32>,
  runtime_fn_index: HashMap<String, u32>,
  tag_index: HashMap<String, u32>,
  struct_field_tags: HashMap<u32, Vec<u32>>,
  string_pool: HashMap<String, u32>,
  /// qualified atom name ("ns/def") → WASM global index
  atom_globals: HashMap<String, u32>,
  /// qualified value name ("ns/def") → preprocessed expression for inlining.
  value_imports: HashMap<String, Calcit>,
  /// Statically resolved function definitions available for closure-argument specialization.
  static_fn_defs: HashMap<String, Arc<StaticFnDef>>,
  /// qualified function name → funcref table slot index (0-based, for call_indirect)
  fn_table_index: HashMap<String, u32>,
  /// Target-approved host imports keyed by `(module, field)`.
  host_imports: HashMap<(String, String), u32>,
  /// Selected output ABI. Surface Calcit calls stay target-independent.
  target: WasmTarget,
  boundary: WasmBoundary,
}

fn extract_fn_parts(code: &Calcit) -> Result<(CalcitFnArgs, Vec<Calcit>), String> {
  let Calcit::List(items) = code else {
    return Err(format!("expected preprocessed defn list, got: {code}"));
  };
  match (items.first(), items.get(1), items.get(2)) {
    (
      Some(Calcit::Syntax(CalcitSyntax::Defn | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport, _)),
      Some(Calcit::Symbol { .. }),
      Some(Calcit::List(args)),
    ) => {
      let raw_args = get_raw_args_fn(args)?;
      Ok((raw_args, items.drop_left().drop_left().drop_left().to_vec()))
    }
    _ => Err(format!("expected preprocessed defn form, got: {code}")),
  }
}

fn is_wasm_export_def(code: &Calcit) -> bool {
  matches!(code, Calcit::List(xs) if matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::DefWasmExport, _))))
}

fn must_reject_extraction_failure(init_ns: &str, ns: &str, code: &Calcit) -> bool {
  ns == init_ns || is_wasm_export_def(code)
}

fn is_wasm_import_def(code: &Calcit) -> bool {
  matches!(code, Calcit::List(xs) if matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::DefWasmImport, _))))
}

/// A WASM import is a function-shaped definition whose first two body forms are
/// literal module and field names, for example:
/// `defwasm-import host-upper (text) |host |upper`.
fn parse_wasm_import_def(code: &Calcit) -> Option<(String, String, CalcitFnArgs)> {
  let Calcit::List(xs) = code else {
    return None;
  };
  if !matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::DefWasmImport, _))) {
    return None;
  }
  let Calcit::List(args) = xs.get(2)? else {
    return None;
  };
  let args = get_raw_args_fn(args).ok()?;
  // Preprocessing prepends a `hint-fn` form for a declared schema. Apart from
  // that internal annotation, the declaration body is exactly two strings.
  let body: Vec<&Calcit> = xs
    .iter()
    .skip(3)
    .filter(|item| CalcitTypeAnnotation::extract_fn_annotation_from_hint_form(item).is_none())
    .collect();
  let [Calcit::Str(module), Calcit::Str(name)] = body.as_slice() else {
    return None;
  };
  Some((module.to_string(), name.to_string(), args))
}

fn wasm_import_arity(args: &CalcitFnArgs) -> Result<u32, String> {
  match args {
    CalcitFnArgs::Args(names) => Ok(names.len() as u32),
    CalcitFnArgs::MarkedArgs(labels) => {
      if labels
        .iter()
        .any(|label| matches!(label, CalcitArgLabel::OptionalMark | CalcitArgLabel::RestMark))
      {
        return Err("optional and rest parameters are not supported for defwasm-import".into());
      }
      Ok(labels.iter().filter(|label| matches!(label, CalcitArgLabel::Idx(_))).count() as u32)
    }
  }
}

#[derive(Clone)]
struct InlineClosure {
  params: Vec<String>,
  body: Vec<Calcit>,
  captured_locals: HashMap<String, u32>,
  captured_closures: HashMap<String, Arc<InlineClosure>>,
}

#[derive(Clone)]
struct StaticFnDef {
  params: Vec<String>,
  body: Vec<Calcit>,
  callback_arities: HashMap<usize, usize>,
  fixed_arity: bool,
}

enum InlineArgument {
  Value(u32),
  Closure(Arc<InlineClosure>),
}

/// Context for WASM code generation within a single function.
struct WasmGenCtx {
  /// Map from local variable name to WASM local index
  locals: HashMap<String, u32>,
  /// Local declarations to add (beyond parameters)
  extra_locals: Vec<ValType>,
  /// Next local index (starts after parameters)
  next_local: u32,
  /// Whether this function uses recur (needs loop wrapping)
  uses_recur: bool,
  /// Argument local indices in order (for recur)
  arg_indices: Vec<u32>,
  /// Collected instructions
  instructions: Vec<Instruction<'static>>,
  /// Function name → index map for cross-function calls
  fn_index: HashMap<String, u32>,
  /// Function name → WASM arity (number of f64 params, excluding markers)
  fn_arity: HashMap<String, u32>,
  /// Function name → fixed-arity (params before `&`) for functions with rest args.
  /// WASM arity for these is `fixed_arity + 1` (the rest list pointer).
  fn_has_rest: HashMap<String, u32>,
  /// Internal runtime helper name → function index.
  runtime_fn_index: HashMap<String, u32>,
  /// Tag name → integer ID map (compile-time constant, shared across all functions)
  tag_index: HashMap<String, u32>,
  /// Struct tag id → field tag ids in index order.
  struct_field_tags: HashMap<u32, Vec<u32>>,
  /// Current block nesting depth relative to the recur loop
  /// (0 = directly inside the loop, 1 = inside one if/block, etc.)
  block_depth: u32,
  /// String literal pool: string content → logical pointer (f64).
  /// Strings are pre-allocated in a data segment before the heap.
  string_pool: HashMap<String, u32>,
  /// qualified atom name ("ns/def") → WASM global index
  atom_globals: HashMap<String, u32>,
  /// qualified value name ("ns/def") → preprocessed expression for inlining.
  value_imports: HashMap<String, Calcit>,
  /// Statically resolved function definitions available for closure-argument specialization.
  static_fn_defs: HashMap<String, Arc<StaticFnDef>>,
  /// qualified function name → funcref table slot index (for call_indirect)
  fn_table_index: HashMap<String, u32>,
  /// Target-approved host imports keyed by `(module, field)`.
  host_imports: HashMap<(String, String), u32>,
  /// Selected output ABI. Surface Calcit calls stay target-independent.
  target: WasmTarget,
  boundary: WasmBoundary,
  /// Statically known closures retain the lexical local bindings visible at creation.
  /// They are specialized at known call sites instead of receiving a dynamic heap ABI.
  lambda_locals: HashMap<String, Arc<InlineClosure>>,
  /// Active static specializations, used to reject recursive inlining deterministically.
  specialization_stack: Vec<String>,
}

impl WasmGenCtx {
  fn new(num_params: u32, env: WasmCompileEnv) -> Self {
    WasmGenCtx {
      locals: HashMap::new(),
      extra_locals: Vec::new(),
      next_local: num_params,
      uses_recur: false,
      arg_indices: Vec::new(),
      instructions: Vec::new(),
      fn_index: env.fn_index,
      fn_arity: env.fn_arity,
      fn_has_rest: env.fn_has_rest,
      runtime_fn_index: env.runtime_fn_index,
      tag_index: env.tag_index,
      struct_field_tags: env.struct_field_tags,
      block_depth: 0,
      string_pool: env.string_pool,
      atom_globals: env.atom_globals,
      value_imports: env.value_imports,
      static_fn_defs: env.static_fn_defs,
      fn_table_index: env.fn_table_index,
      host_imports: env.host_imports,
      target: env.target,
      boundary: env.boundary,
      lambda_locals: HashMap::new(),
      specialization_stack: vec![],
    }
  }

  /// Allocate an anonymous f64 local variable.
  fn alloc_local(&mut self) -> u32 {
    self.alloc_local_typed(ValType::F64)
  }

  /// Allocate an anonymous local variable of the given type.
  fn alloc_local_typed(&mut self, vt: ValType) -> u32 {
    let idx = self.next_local;
    self.next_local += 1;
    self.extra_locals.push(vt);
    idx
  }

  fn declare_local(&mut self, name: &str) -> u32 {
    let idx = self.alloc_local();
    self.locals.insert(name.to_owned(), idx);
    idx
  }

  fn emit(&mut self, instr: Instruction<'static>) {
    self.instructions.push(instr);
  }

  /// Emit a `Call` to a runtime helper by name.
  /// Caller must push the arguments onto the stack first.
  /// Panics if the name is not registered.
  pub(super) fn call_rt(&mut self, name: &str) {
    let fn_idx = *self
      .runtime_fn_index
      .get(name)
      .unwrap_or_else(|| panic!("runtime helper missing: {name}"));
    self.emit(Instruction::Call(fn_idx));
  }

  /// Evaluate all `args`, dropping each result, then push nil (0.0).
  /// Used for operations that are intentionally no-ops in WASM.
  pub(super) fn stub_proc(&mut self, args: &[Calcit]) -> Result<(), String> {
    for arg in args {
      emit_expr(self, arg)?;
      self.emit(Instruction::Drop);
    }
    self.emit(f64_const(0.0));
    Ok(())
  }

  /// Silently ignore all args and return nil. Use for type-system procs whose
  /// arguments may contain tags/values not representable in WASM (e.g. `:&core-number-methods`).
  /// NOTE: This should only be used for initialization/setup code that runs before user code.
  pub(super) fn silent_nil(&mut self) -> Result<(), String> {
    self.emit(f64_const(0.0));
    Ok(())
  }

  // -----------------------------------------------------------------------
  // Integer arithmetic helpers
  // -----------------------------------------------------------------------

  /// `local += 1` — 4-instruction pattern compressed to one call.
  pub(super) fn i32_inc(&mut self, local: u32) {
    self.emit(Instruction::LocalGet(local));
    self.emit(Instruction::I32Const(1));
    self.emit(Instruction::I32Add);
    self.emit(Instruction::LocalSet(local));
  }

  /// `local -= 1`.
  pub(super) fn i32_dec(&mut self, local: u32) {
    self.emit(Instruction::LocalGet(local));
    self.emit(Instruction::I32Const(1));
    self.emit(Instruction::I32Sub);
    self.emit(Instruction::LocalSet(local));
  }

  /// `local = a + b` — store sum of two i32 locals into `dst`.
  #[allow(dead_code)]
  pub(super) fn i32_add_into(&mut self, a: u32, b: u32, dst: u32) {
    self.emit(Instruction::LocalGet(a));
    self.emit(Instruction::LocalGet(b));
    self.emit(Instruction::I32Add);
    self.emit(Instruction::LocalSet(dst));
  }

  /// `local = a - b`.
  #[allow(dead_code)]
  pub(super) fn i32_sub_into(&mut self, a: u32, b: u32, dst: u32) {
    self.emit(Instruction::LocalGet(a));
    self.emit(Instruction::LocalGet(b));
    self.emit(Instruction::I32Sub);
    self.emit(Instruction::LocalSet(dst));
  }

  // -----------------------------------------------------------------------
  // Control-flow structure helpers (Block / Loop / If)
  // -----------------------------------------------------------------------

  /// Begin a `block` (used as the outer breakable container around a loop).
  pub(super) fn begin_block(&mut self) {
    self.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  }

  /// Begin a `loop` (the inner repeated body).
  pub(super) fn begin_loop(&mut self) {
    self.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  }

  /// Begin an `if` with no result value (void block).
  pub(super) fn begin_block_if(&mut self) {
    self.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  }

  /// Emit `end` for a single block/loop/if.
  #[allow(dead_code)]
  pub(super) fn end_one(&mut self) {
    self.emit(Instruction::End);
  }

  /// Emit two `end`s — closes a loop then its surrounding block.
  pub(super) fn end_block_loop(&mut self) {
    self.emit(Instruction::End); // end loop
    self.emit(Instruction::End); // end block
  }

  /// `br 0` — jump back to the top of the enclosing loop.
  pub(super) fn br_loop(&mut self) {
    self.emit(Instruction::Br(0));
  }

  /// `br_if 1` — exit the outer block (break out of the block+loop pair) if i32 on stack is non-zero.
  pub(super) fn br_if_exit(&mut self) {
    self.emit(Instruction::BrIf(1));
  }

  /// Emit the "i >= count → break" guard common to all forward-indexed loops.
  /// Expects both locals to be i32. Emits: `LocalGet(i); LocalGet(count); I32GeU; BrIf(1)`.
  pub(super) fn loop_exit_if_ge(&mut self, i: u32, count: u32) {
    self.emit(Instruction::LocalGet(i));
    self.emit(Instruction::LocalGet(count));
    self.emit(Instruction::I32GeU);
    self.br_if_exit();
  }

  /// Emit the "i < 0 → break" guard used by reverse loops.
  /// Expects `i` to be an i32. Emits: `LocalGet(i); I32Const(0); I32LtS; BrIf(1)`.
  pub(super) fn loop_exit_if_neg(&mut self, i: u32) {
    self.emit(Instruction::LocalGet(i));
    self.emit(Instruction::I32Const(0));
    self.emit(Instruction::I32LtS);
    self.br_if_exit();
  }

  /// Push an i32 local onto the WASM stack as f64.
  /// Common return pattern: `LocalGet(ptr) + F64ConvertI32U`.
  pub(super) fn ptr_to_f64(&mut self, local: u32) {
    self.emit(Instruction::LocalGet(local));
    self.emit(Instruction::F64ConvertI32U);
  }

  /// Store an i32 local as f64 into slot at `byte_offset` relative to `ptr`.
  /// Pattern: `LocalGet(ptr) + LocalGet(val) + F64ConvertI32U + F64Store(offset)`.
  pub(super) fn store_i32_as_f64(&mut self, ptr: u32, val: u32, byte_offset: u64) {
    self.emit(Instruction::LocalGet(ptr));
    self.emit(Instruction::LocalGet(val));
    self.emit(Instruction::F64ConvertI32U);
    self.emit(Instruction::F64Store(mem_arg_f64(byte_offset)));
  }

  /// Allocate a new i32 local and initialise it to a constant.
  /// Common pattern: `let x = alloc_local_typed(I32); I32Const(val); LocalSet(x)`.
  pub(super) fn alloc_i32(&mut self, val: i32) -> u32 {
    let local = self.alloc_local_typed(ValType::I32);
    self.emit(Instruction::I32Const(val));
    self.emit(Instruction::LocalSet(local));
    local
  }

  /// Allocate a new i32 local = `src + delta` (negative delta uses I32Sub).
  /// Replaces: `alloc_local_typed(I32) + LocalGet(src) + I32Const(|delta|) + I32Add/Sub + LocalSet`.
  pub(super) fn i32_offset(&mut self, src: u32, delta: i32) -> u32 {
    let local = self.alloc_local_typed(ValType::I32);
    self.emit(Instruction::LocalGet(src));
    self.emit(Instruction::I32Const(delta.unsigned_abs() as i32));
    if delta >= 0 {
      self.emit(Instruction::I32Add);
    } else {
      self.emit(Instruction::I32Sub);
    }
    self.emit(Instruction::LocalSet(local));
    local
  }
}

fn capture_inline_closure(ctx: &WasmGenCtx, expr: &Calcit) -> Option<Arc<InlineClosure>> {
  let (params, body) = try_extract_inline_lambda(expr)?;
  Some(Arc::new(InlineClosure {
    params,
    body,
    captured_locals: ctx.locals.clone(),
    captured_closures: ctx.lambda_locals.clone(),
  }))
}

fn resolve_inline_closure(ctx: &WasmGenCtx, expr: &Calcit) -> Option<Arc<InlineClosure>> {
  match expr {
    Calcit::Local(local) => ctx.lambda_locals.get(local.sym.as_ref()).cloned(),
    Calcit::Symbol { sym, .. } => ctx.lambda_locals.get(sym.as_ref()).cloned(),
    _ => capture_inline_closure(ctx, expr),
  }
}

fn emit_inline_closure_body(ctx: &mut WasmGenCtx, closure: &InlineClosure, param_locals: &[u32]) -> Result<(), String> {
  if closure.params.len() != param_locals.len() {
    return Err(format!(
      "inline closure expects {} argument(s), got {}",
      closure.params.len(),
      param_locals.len()
    ));
  }
  if closure.body.iter().any(check_uses_recur) {
    return Err("recur in a statically specialized closure is not yet supported in WASM codegen".into());
  }

  let caller_locals = std::mem::replace(&mut ctx.locals, closure.captured_locals.clone());
  let caller_closures = std::mem::replace(&mut ctx.lambda_locals, closure.captured_closures.clone());
  for (param, local) in closure.params.iter().zip(param_locals) {
    ctx.locals.insert(param.clone(), *local);
  }
  let result = emit_body(ctx, &closure.body);
  ctx.locals = caller_locals;
  ctx.lambda_locals = caller_closures;
  result
}

fn emit_inline_closure_call(ctx: &mut WasmGenCtx, closure: &InlineClosure, args: &[Calcit]) -> Result<(), String> {
  if closure.params.len() != args.len() {
    return Err(format!(
      "inline closure expects {} argument(s), got {}",
      closure.params.len(),
      args.len()
    ));
  }
  if closure.body.iter().any(check_uses_recur) {
    return Err("recur in a statically specialized closure is not yet supported in WASM codegen".into());
  }

  let mut bindings = Vec::with_capacity(args.len());
  for arg in args {
    if let Some(arg_closure) = resolve_inline_closure(ctx, arg) {
      bindings.push(InlineArgument::Closure(arg_closure));
    } else {
      emit_expr(ctx, arg)?;
      let idx = ctx.alloc_local();
      ctx.emit(Instruction::LocalSet(idx));
      bindings.push(InlineArgument::Value(idx));
    }
  }

  let caller_locals = std::mem::replace(&mut ctx.locals, closure.captured_locals.clone());
  let caller_closures = std::mem::replace(&mut ctx.lambda_locals, closure.captured_closures.clone());
  for (param, binding) in closure.params.iter().zip(bindings) {
    match binding {
      InlineArgument::Value(local) => {
        ctx.locals.insert(param.clone(), local);
      }
      InlineArgument::Closure(value) => {
        ctx.lambda_locals.insert(param.clone(), value);
      }
    }
  }
  let result = emit_body(ctx, &closure.body);
  ctx.locals = caller_locals;
  ctx.lambda_locals = caller_closures;
  result
}

fn emit_specialized_static_call(ctx: &mut WasmGenCtx, qualified: &str, args: &[Calcit]) -> Result<bool, String> {
  let Some(definition) = ctx.static_fn_defs.get(qualified).cloned() else {
    return Ok(false);
  };

  let closures = args
    .iter()
    .enumerate()
    .map(|(index, arg)| {
      let closure = resolve_inline_closure(ctx, arg);
      if let Some(closure) = &closure {
        let expected_arity = definition.callback_arities.get(&index).ok_or_else(|| {
          format!(
            "E_WASM_CLOSURE_SPECIALIZATION: `{qualified}` argument {} is a closure but its static parameter contract is not callable",
            index + 1
          )
        })?;
        if closure.params.len() != *expected_arity {
          return Err(format!(
            "E_WASM_CLOSURE_SPECIALIZATION: `{qualified}` callback argument {} expects {expected_arity} parameter(s), got {}",
            index + 1,
            closure.params.len()
          ));
        }
      }
      Ok(closure)
    })
    .collect::<Result<Vec<_>, String>>()?;
  if closures.iter().all(Option::is_none) {
    return Ok(false);
  }
  if !definition.fixed_arity || definition.params.len() != args.len() {
    return Err(format!(
      "E_WASM_CLOSURE_SPECIALIZATION: `{qualified}` requires a fixed known arity for closure specialization"
    ));
  }
  if definition.body.iter().any(check_uses_recur) || ctx.specialization_stack.iter().any(|active| active == qualified) {
    return Err(format!(
      "E_WASM_CLOSURE_SPECIALIZATION: recursive specialization of `{qualified}` is not supported"
    ));
  }

  let mut bindings = Vec::with_capacity(args.len());
  for (arg, closure) in args.iter().zip(closures) {
    if let Some(closure) = closure {
      bindings.push(InlineArgument::Closure(closure));
    } else {
      emit_expr(ctx, arg)?;
      let local = ctx.alloc_local();
      ctx.emit(Instruction::LocalSet(local));
      bindings.push(InlineArgument::Value(local));
    }
  }

  let caller_locals = std::mem::take(&mut ctx.locals);
  let caller_closures = std::mem::take(&mut ctx.lambda_locals);
  for (param, binding) in definition.params.iter().zip(bindings) {
    match binding {
      InlineArgument::Value(local) => {
        ctx.locals.insert(param.clone(), local);
      }
      InlineArgument::Closure(closure) => {
        ctx.lambda_locals.insert(param.clone(), closure);
      }
    }
  }
  ctx.specialization_stack.push(qualified.to_owned());
  let result = emit_body(ctx, &definition.body);
  ctx.specialization_stack.pop();
  ctx.locals = caller_locals;
  ctx.lambda_locals = caller_closures;
  result?;
  Ok(true)
}

/// Check that `args` has exactly `n` elements and return an error if not.
#[inline]
fn expect_arity(n: usize, args: &[Calcit], proc_name: &str) -> Result<(), String> {
  if args.len() != n {
    return Err(format!("{proc_name} expects {n} arg(s), got {}", args.len()));
  }
  Ok(())
}

/// Compute WASM arity for a function signature.
///
/// Returns `(wasm_arity, rest_fixed)`:
/// - `wasm_arity`: total number of f64 params the function takes
/// - `rest_fixed`: if the function has a rest arg, the number of fixed params
///   before `&` (`wasm_arity - 1`); otherwise `None`.
///
/// Optional marks (`?`) are transparent: callers pad nil for omitted optional args.
/// Rest args (`&`) are represented as a single f64 list-pointer param.
fn compute_fn_arity(args: &CalcitFnArgs) -> (u32, Option<u32>) {
  match args {
    CalcitFnArgs::Args(v) => (v.len() as u32, None),
    CalcitFnArgs::MarkedArgs(labels) => {
      let mut fixed: u32 = 0;
      let mut rest_param_count: u32 = 0;
      let mut rest_seen = false;
      for label in labels {
        match label {
          CalcitArgLabel::Idx(_) => {
            if rest_seen {
              rest_param_count += 1;
            } else {
              fixed += 1;
            }
          }
          CalcitArgLabel::OptionalMark => {}
          CalcitArgLabel::RestMark => {
            rest_seen = true;
          }
        }
      }
      if rest_seen && rest_param_count > 0 {
        (fixed + 1, Some(fixed))
      } else {
        (fixed, None)
      }
    }
  }
}

fn fn_param_names(args: &CalcitFnArgs) -> Vec<String> {
  match args {
    CalcitFnArgs::Args(indices) => indices.iter().map(|index| CalcitLocal::read_name(*index)).collect(),
    CalcitFnArgs::MarkedArgs(labels) => labels
      .iter()
      .filter_map(|label| match label {
        CalcitArgLabel::Idx(index) => Some(CalcitLocal::read_name(*index)),
        CalcitArgLabel::OptionalMark | CalcitArgLabel::RestMark => None,
      })
      .collect(),
  }
}

fn try_custom_def_impl(
  ns: &str,
  def_name: &str,
  export_name: &str,
  _args: &CalcitFnArgs,
  env: &WasmCompileEnv,
) -> Option<Result<CompiledFn, String>> {
  if ns != "calcit.core" {
    return None;
  }
  // Helpers for building a 2-param custom function body.
  type BodyFn = fn(&mut WasmGenCtx) -> Result<(), String>;
  let arity: u32 = 2;
  let build = |body_fn: BodyFn, en: &str, env: &WasmCompileEnv| -> Result<CompiledFn, String> {
    let mut ctx = WasmGenCtx::new(arity, env.clone());
    // Register param locals 0 and 1
    ctx.locals.insert("__p0__".into(), 0);
    ctx.locals.insert("__p1__".into(), 1);
    ctx.arg_indices.push(0);
    ctx.arg_indices.push(1);
    body_fn(&mut ctx)?;
    Ok(CompiledFn {
      export_name: Some(en.to_owned()),
      params: vec![ValType::F64; arity as usize],
      results: vec![ValType::F64],
      locals: ctx.extra_locals,
      instructions: ctx.instructions,
    })
  };
  match def_name {
    "repeat" => Some(build(|ctx| emit_repeat_from_locals(ctx, 0, 1), export_name, env)),
    "interleave" => Some(build(|ctx| emit_interleave_from_locals(ctx, 0, 1), export_name, env)),
    "zipmap" => Some(build(|ctx| emit_zipmap_from_locals(ctx, 0, 1), export_name, env)),
    "join" => Some(build(|ctx| emit_join_from_locals(ctx, 0, 1), export_name, env)),
    "join-str" => Some(build(|ctx| emit_join_str_from_locals(ctx, 0, 1), export_name, env)),
    _ => None,
  }
}

fn compile_fn(
  _name: &str,
  export_name: &str,
  args: &CalcitFnArgs,
  body: &[Calcit],
  env: &WasmCompileEnv,
) -> Result<CompiledFn, String> {
  let param_names = fn_param_names(args);

  let arity = param_names.len();
  let mut ctx = WasmGenCtx::new(arity as u32, env.clone());

  // Register parameter locals
  for (i, pname) in param_names.iter().enumerate() {
    ctx.locals.insert(pname.clone(), i as u32);
    ctx.arg_indices.push(i as u32);
  }

  // Check if body uses recur
  ctx.uses_recur = body.iter().any(check_uses_recur);

  if ctx.uses_recur {
    // loop $recur (result f64) ... end
    ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Result(ValType::F64)));
    emit_body(&mut ctx, body)?;
    ctx.emit(Instruction::End); // end loop
  } else {
    emit_body(&mut ctx, body)?;
  }

  Ok(CompiledFn {
    export_name: Some(export_name.to_owned()),
    params: vec![ValType::F64; arity],
    results: vec![ValType::F64],
    locals: ctx.extra_locals,
    instructions: ctx.instructions,
  })
}

fn check_uses_recur(expr: &Calcit) -> bool {
  match expr {
    Calcit::Proc(CalcitProc::Recur) => true,
    Calcit::List(xs) => {
      // Don't recurse into nested defn
      if let Some(Calcit::Syntax(CalcitSyntax::Defn, _)) = xs.first() {
        return false;
      }
      xs.iter().any(check_uses_recur)
    }
    _ => false,
  }
}

/// Emit instructions for a sequence of expressions (last is the return value).
fn emit_body(ctx: &mut WasmGenCtx, exprs: &[Calcit]) -> Result<(), String> {
  // Strict preprocessing may append a function contract after the executable
  // body. `hint-fn` is compile-time metadata, not the closure's return value.
  let executable = exprs
    .iter()
    .filter(|expr| !matches!(expr, Calcit::List(xs) if matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::HintFn, _)))))
    .collect::<Vec<_>>();
  if executable.is_empty() {
    ctx.emit(f64_const(0.0));
    return Ok(());
  }
  for (i, expr) in executable.iter().enumerate() {
    emit_expr(ctx, expr)?;
    if i < executable.len() - 1 {
      ctx.emit(Instruction::Drop);
    }
  }
  Ok(())
}

/// Emit an inline IIFE with TCO: `((defn name (params...) body...) init_args...)`.
///
/// - Allocates new f64 locals for each param, initializes from `init_args`.
/// - If the body uses `recur`, wraps with `loop...end` and sets up `ctx.arg_indices`
///   so that `recur` correctly jumps back to the loop start.
/// - Outer params of the same name are shadowed and restored after.
fn emit_inline_iife(ctx: &mut WasmGenCtx, params: &[String], body: &[Calcit], init_args: &[Calcit]) -> Result<(), String> {
  // Evaluate all init args and store in fresh locals (before modifying ctx.locals).
  let mut param_locals: Vec<u32> = Vec::new();
  for (i, _param_name) in params.iter().enumerate() {
    let tmp = ctx.alloc_local();
    if i < init_args.len() {
      emit_expr(ctx, &init_args[i])?;
    } else {
      ctx.emit(f64_const(0.0)); // nil for missing args
    }
    ctx.emit(Instruction::LocalSet(tmp));
    param_locals.push(tmp);
  }

  // Shadow outer locals with IIFE params (save old bindings for restoration).
  let mut saved: Vec<(String, Option<u32>)> = Vec::new();
  for (i, name) in params.iter().enumerate() {
    saved.push((name.clone(), ctx.locals.get(name).copied()));
    ctx.locals.insert(name.clone(), param_locals[i]);
  }

  // Emit body — with a loop wrapper if TCO recur is needed.
  let uses_recur = body.iter().any(check_uses_recur);
  if uses_recur {
    let old_arg_indices = std::mem::replace(&mut ctx.arg_indices, param_locals);
    let old_block_depth = ctx.block_depth;
    ctx.block_depth = 0;

    ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Result(ValType::F64)));
    emit_body(ctx, body)?;
    ctx.emit(Instruction::End); // end loop

    ctx.block_depth = old_block_depth;
    ctx.arg_indices = old_arg_indices;
  } else {
    emit_body(ctx, body)?;
  }

  // Restore shadowed locals.
  for (name, old) in saved {
    match old {
      Some(idx) => {
        ctx.locals.insert(name, idx);
      }
      None => {
        ctx.locals.remove(&name);
      }
    }
  }
  Ok(())
}

/// Emit instructions for a single Calcit expression.
fn emit_expr(ctx: &mut WasmGenCtx, expr: &Calcit) -> Result<(), String> {
  match expr {
    Calcit::Number(n) => {
      ctx.emit(f64_const(*n));
    }
    Calcit::Bool(true) => {
      ctx.emit(f64_const(1.0));
    }
    Calcit::Bool(false) | Calcit::Nil | Calcit::Unit => {
      ctx.emit(f64_const(0.0));
    }
    Calcit::List(xs) if xs.is_empty() => {
      emit_list_new(ctx, &[])?;
    }
    Calcit::Tag(t) => {
      let tag_str = t.to_string();
      let id = *ctx
        .tag_index
        .get(&tag_str)
        .ok_or_else(|| format!("unknown tag in WASM codegen: {tag_str}"))?;
      ctx.emit(f64_const(id as f64));
    }
    Calcit::StructDef(s) => {
      let tag_str = s.name.to_string();
      let id = *ctx
        .tag_index
        .get(&tag_str)
        .ok_or_else(|| format!("unknown struct tag in WASM codegen: {tag_str}"))?;
      ctx.emit(f64_const(id as f64));
    }
    Calcit::Local(local) => {
      let name = &*local.sym;
      if ctx.lambda_locals.contains_key(name) {
        return Err(format!(
          "E_WASM_CLOSURE_SPECIALIZATION: closure `{name}` escapes a statically specialized call"
        ));
      }
      let idx = *ctx.locals.get(name).ok_or_else(|| format!("undefined local variable: {name}"))?;
      ctx.emit(Instruction::LocalGet(idx));
    }
    Calcit::List(xs) if !xs.is_empty() => {
      emit_call_expr(ctx, xs)?;
    }
    // `do` appears as a bare (non-call) expression when used as a body sequencer in defn.
    // It's a no-op — just emit nil so it can be dropped by emit_body.
    Calcit::Import(import) if import.def.as_ref() == "do" => {
      ctx.emit(f64_const(0.0));
    }
    Calcit::Import(import) => {
      // Check if this is a reference to a known WASM global (defatom)
      let qualified = format!("{}/{}", import.ns, import.def);
      if let Some(&global_idx) = ctx.atom_globals.get(&qualified) {
        ctx.emit(Instruction::GlobalGet(global_idx));
      } else if import.def.as_ref() == "{}" {
        // `{}` used as a bare expression — evaluates to an empty map.
        emit_map_new(ctx, &[])?;
      } else if import.def.as_ref() == "[]" {
        // `[]` used as a bare expression — evaluates to an empty list.
        emit_list_new(ctx, &[])?;
      } else if let Ok(struct_def) = resolve_struct_ref(expr) {
        let tag_str = struct_def.name.to_string();
        let id = *ctx
          .tag_index
          .get(&tag_str)
          .ok_or_else(|| format!("unknown struct tag in WASM codegen: {tag_str}"))?;
        ctx.emit(f64_const(id as f64));
      } else if let Some(&slot) = ctx
        .fn_table_index
        .get(&qualified)
        .or_else(|| ctx.fn_table_index.get(import.def.as_ref()))
      {
        // Function reference used as a value — encode as f64 table slot index.
        ctx.emit(f64_const(slot as f64));
      } else if let Some(value_expr) = ctx.value_imports.get(&qualified).cloned() {
        // Imported top-level def value (e.g. a string constant). Inline its expression.
        emit_expr(ctx, &value_expr)?;
      } else {
        // defvar or otherwise non-function import — emit nil (0.0) as a placeholder.
        // This covers `def`-defined values that aren't representable as f64.
        // Such references appear mostly in initializer/registration paths that are
        // no-ops in WASM (e.g., &init-builtin-impls!).
        ctx.emit(f64_const(0.0));
      }
    }
    Calcit::Str(s) => {
      let ptr = ctx
        .string_pool
        .get(s.as_ref())
        .ok_or_else(|| format!("string literal not found in pool: {s}"))?;
      ctx.emit(f64_const(*ptr as f64));
    }
    Calcit::Struct(_) => return Err("Struct literals not supported in WASM codegen (use constructor)".into()),
    Calcit::Enum(_) => return Err("Enum literals not supported in WASM codegen (use constructor)".into()),
    // Function value (Fn with def_ref) — encode as f64 table slot index for call_indirect.
    Calcit::Fn { info, .. } => {
      if let Some(def_ref) = &info.def_ref {
        let qualified = format!("{}/{}", def_ref.def_ns, def_ref.def_name);
        let slot = ctx
          .fn_table_index
          .get(&qualified)
          .or_else(|| ctx.fn_table_index.get(def_ref.def_name.as_ref()))
          .copied()
          .ok_or_else(|| format!("fn value not in table: {qualified}"))?;
        ctx.emit(f64_const(slot as f64));
      } else {
        return Err("anonymous closure values are not yet supported in WASM codegen".into());
      }
    }
    // `[]` used as a bare expression (not in call position) evaluates to an empty list.
    // This arises from `cond` branches like `true []` where `[]` is the return value.
    Calcit::Proc(CalcitProc::List) => {
      emit_list_new(ctx, &[])?;
    }
    // `{}` used as a bare expression — evaluates to an empty map.
    Calcit::Proc(CalcitProc::NativeMap) => {
      emit_map_new(ctx, &[])?;
    }
    _ => return Err(format!("unsupported WASM expression: {expr}")),
  }
  Ok(())
}

/// Emit instructions for a call expression.
fn emit_call_expr(ctx: &mut WasmGenCtx, xs: &crate::calcit::CalcitList) -> Result<(), String> {
  let head = &xs[0];
  let args_list: Vec<Calcit> = xs.drop_left().to_vec();

  match head {
    Calcit::Syntax(syn, _) => match syn {
      CalcitSyntax::CallSpread => emit_call_spread(ctx, &args_list),
      CalcitSyntax::If => emit_if(ctx, &args_list),
      CalcitSyntax::CoreLet => emit_let(ctx, &args_list),
      CalcitSyntax::Match => emit_match(ctx, &args_list),
      CalcitSyntax::HintFn => {
        // hint-fn is metadata-only; emit nothing (0.0 placeholder)
        ctx.emit(f64_const(0.0));
        Ok(())
      }
      CalcitSyntax::AssertType => {
        // assert-type checks type at preprocess time; at runtime just evaluate the expression
        if args_list.is_empty() {
          return Err("assert-type expects at least 1 arg".into());
        }
        emit_expr(ctx, &args_list[0])
      }
      CalcitSyntax::UnsafeCoerce => {
        if args_list.is_empty() {
          return Err("unsafe-coerce expects at least 1 arg".into());
        }
        emit_expr(ctx, &args_list[0])
      }
      CalcitSyntax::TryParseCirruEdnAs => edn_parse::emit_try_parse_cirru_edn_as(ctx, &args_list),
      CalcitSyntax::ParseCirruEdnAs | CalcitSyntax::DecodeMapAs | CalcitSyntax::TryDecodeMapAs => {
        Err(format!("{syn} is not yet supported in WASM codegen"))
      }
      CalcitSyntax::Defn => Err("nested fn/defn closure values are not yet supported in WASM codegen".into()),
      CalcitSyntax::Quote | CalcitSyntax::Quasiquote => {
        // Quote creates a runtime value (quoted symbol/expression).
        // In WASM, emit nil as a placeholder — quote values appear mainly in
        // error-reporting paths (e.g., the assert macro formats the failing
        // expression via `format-to-lisp (quote ~xs)`) and don't affect the
        // normal execution path.
        ctx.emit(f64_const(0.0));
        Ok(())
      }
      CalcitSyntax::Reset => {
        // reset! atom new-value — set atom global and return new value
        if args_list.len() != 2 {
          return Err(format!("reset! expects 2 args, got {}", args_list.len()));
        }
        let qualified = match &args_list[0] {
          Calcit::Import(import) => format!("{}/{}", import.ns, import.def),
          _ => return Err(format!("reset! first arg must be an atom import, got: {}", args_list[0])),
        };
        let global_idx = *ctx
          .atom_globals
          .get(&qualified)
          .ok_or_else(|| format!("unknown atom in reset!: {qualified}"))?;
        emit_expr(ctx, &args_list[1])?;
        // Store new value in global, and leave it on stack as return value
        let tmp = ctx.alloc_local();
        ctx.emit(Instruction::LocalTee(tmp));
        ctx.emit(Instruction::GlobalSet(global_idx));
        ctx.emit(Instruction::LocalGet(tmp));
        Ok(())
      }
      _ => Err(format!("unsupported syntax in WASM: {syn}")),
    },
    Calcit::Proc(proc) => emit_proc_call(ctx, proc, &args_list),
    Calcit::Method(name, kind) => match kind {
      MethodKind::Invoke(_) => emit_method_invoke(ctx, name.as_ref(), &args_list),
      _ => Err(format!("unsupported method in WASM: .{name}")),
    },
    Calcit::Import(import) => {
      // `do` is a sequencing form in calcit.core — emit all args, return last
      if import.def.as_ref() == "do" {
        return emit_body(ctx, &args_list);
      }
      // High-level set wrappers (union/difference/include) use reduce+lambda which can't be
      // compiled to WASM directly. Intercept and inline native 2+ arg set operations.
      if import.ns.as_ref() == "calcit.core" {
        // Native placeholders may remain imports when WASM consumes a source
        // form instead of the fully lowered expression. Resolve their current
        // public names through CalcitProc so ABI renames do not silently turn
        // supported operations into skipped functions.
        if let Ok(proc) = import.def.parse::<CalcitProc>() {
          return emit_proc_call(ctx, &proc, &args_list);
        }
        match import.def.as_ref() {
          "union" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Union),
          "difference" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Difference),
          "include" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Include),
          // `reduce xs x0 f` → inline as foldl so proc/import callees work.
          "reduce" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
          // `foldl-compare xs acc f` → inline comparison loop.
          "foldl-compare" if args_list.len() == 3 => return emit_foldl_compare(ctx, &args_list),
          // Unary HOF intercepts — callee 'f' would be unresolvable inside core defs.
          "map" | "&list:map" if args_list.len() == 2 => return emit_map(ctx, &args_list),
          "map-indexed" | "&list:map-indexed" if args_list.len() == 2 => return emit_map_indexed(ctx, &args_list),
          "each" if args_list.len() == 2 => return emit_each(ctx, &args_list),
          "filter" | "&list:filter" | "&set:filter" if args_list.len() == 2 => return emit_filter(ctx, &args_list),
          "any?" if args_list.len() == 2 => return emit_any(ctx, &args_list),
          "every?" if args_list.len() == 2 => return emit_every(ctx, &args_list),
          "find" if args_list.len() == 2 => return emit_find(ctx, &args_list),
          "find-index" if args_list.len() == 2 => return emit_find_index(ctx, &args_list),
          // `concat` takes variadic lists — intercept to emit variadic list concat.
          "concat" => return emit_list_concat(ctx, &args_list),
          // `deref` — in WASM, all atoms are globals; just evaluate the atom expression.
          "deref" if args_list.len() == 1 => return emit_expr(ctx, &args_list[0]),
          // `str` / `str-spaced` — variadic string concat; core defs use (&syntax &) which is unsupported.
          "str" if !args_list.is_empty() => return emit_str_variadic(ctx, &args_list),
          "str-spaced" if !args_list.is_empty() => return emit_str_spaced(ctx, &args_list),
          // `foldl'` is an inline variant of `foldl` with same arg order (xs acc f).
          "foldl'" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
          // `filter-not` — keep elements where f(elem) is falsy.
          "filter-not" if args_list.len() == 2 => return emit_filter_not(ctx, &args_list),
          // `slice` — delegated to list slice (list-only, 2-3 args).
          "slice" if args_list.len() >= 2 && args_list.len() <= 3 => return emit_list_slice(ctx, &args_list),
          // `dissoc` — delegated to map dissoc for 2-arg calls.
          "dissoc" if args_list.len() == 2 => return emit_map_dissoc(ctx, &args_list),
          // `conj` — append one or more elements to a list.
          "conj" if args_list.len() >= 2 => return emit_conj(ctx, &args_list),
          // `update` — map update: new map with key set to f(old value).
          "update" if args_list.len() == 3 => return emit_update(ctx, &args_list),
          // `mapcat xs f` — map then flatten one level.
          "mapcat" if args_list.len() == 2 => return emit_mapcat(ctx, &args_list),
          // Simple loop implementations for HOF-based definitions.
          "repeat" if args_list.len() == 2 => return emit_repeat(ctx, &args_list),
          "interleave" if args_list.len() == 2 => return emit_interleave(ctx, &args_list),
          "zipmap" if args_list.len() == 2 => return emit_zipmap(ctx, &args_list),
          "join" if args_list.len() == 2 => return emit_join(ctx, &args_list),
          "join-str" if args_list.len() == 2 => return emit_join_str(ctx, &args_list),
          // `let` — multi-binding form: (let ((name val)...) body...).
          // The preprocessor normally expands this to nested `&let` forms, but intercept here
          // as a fallback for cases where the macro expansion hasn't occurred.
          "let" if !args_list.is_empty() => return emit_let_multi(ctx, &args_list),
          // `map-kv xs f` — apply a binary function to each map entry, returning a new map.
          "map-kv" if args_list.len() == 2 => return emit_map_kv(ctx, &args_list),
          // `filter-map-kv xs f` — consume typed :keep/:drop decisions.
          "filter-map-kv" if args_list.len() == 2 => return emit_filter_map_kv(ctx, &args_list),
          // `'` — list literal constructor (calcit.core/'), same as ([] a b c).
          "'" => return emit_list_new(ctx, &args_list),
          _ => {}
        }
      }
      // Try qualified "ns/def" first, then bare "def" as fallback
      let qualified = format!("{}/{}", import.ns, import.def);
      if emit_specialized_static_call(ctx, &qualified, &args_list)? {
        return Ok(());
      }
      let fn_idx = ctx
        .fn_index
        .get(&qualified)
        .or_else(|| ctx.fn_index.get(import.def.as_ref()))
        .ok_or_else(|| format!("unknown function: {qualified}"))?;
      let fn_idx = *fn_idx;
      let target_arity = ctx
        .fn_arity
        .get(&qualified)
        .or_else(|| ctx.fn_arity.get(import.def.as_ref()))
        .copied()
        .unwrap_or(args_list.len() as u32);
      let rest_fixed = ctx
        .fn_has_rest
        .get(&qualified)
        .or_else(|| ctx.fn_has_rest.get(import.def.as_ref()))
        .copied();
      emit_call_args(ctx, &args_list, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    Calcit::Symbol { sym, .. } => {
      let name = sym.as_ref();
      // IO functions: call host log_value for each arg, return nil
      if matches!(name, "println" | "eprintln" | "echo") {
        if ctx.target == WasmTarget::Wasi && ctx.boundary == WasmBoundary::Component {
          return emit_wasi_component_print(ctx, name, &args_list);
        }
        if ctx.target == WasmTarget::Wasi {
          return emit_wasi_print(ctx, name, &args_list);
        }
        let log_idx = resolve_host_import(ctx, "io", "log_value")?;
        for arg in &args_list {
          emit_expr(ctx, arg)?;
          ctx.emit(Instruction::Call(log_idx));
          ctx.emit(Instruction::Drop); // drop log_value's return
        }
        ctx.emit(f64_const(0.0)); // nil
        return Ok(());
      }
      if let Ok(proc) = name.parse::<CalcitProc>() {
        return emit_proc_call(ctx, &proc, &args_list);
      }
      // HOF interceptors — Symbol-head calls appear when preprocessor doesn't resolve
      // intra-namespace references to Import nodes (e.g. calcit.core internal calls).
      match name {
        "map" if args_list.len() == 2 => return emit_map(ctx, &args_list),
        "filter" | "&list:filter" | "&set:filter" if args_list.len() == 2 => return emit_filter(ctx, &args_list),
        "filter-not" if args_list.len() == 2 => return emit_filter_not(ctx, &args_list),
        "each" if args_list.len() == 2 => return emit_each(ctx, &args_list),
        "any?" if args_list.len() == 2 => return emit_any(ctx, &args_list),
        "every?" if args_list.len() == 2 => return emit_every(ctx, &args_list),
        "find" if args_list.len() == 2 => return emit_find(ctx, &args_list),
        "find-index" if args_list.len() == 2 => return emit_find_index(ctx, &args_list),
        "map-indexed" if args_list.len() == 2 => return emit_map_indexed(ctx, &args_list),
        "mapcat" if args_list.len() == 2 => return emit_mapcat(ctx, &args_list),
        "reduce" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
        "foldl'" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
        "update" if args_list.len() == 3 => return emit_update(ctx, &args_list),
        "map-kv" if args_list.len() == 2 => return emit_map_kv(ctx, &args_list),
        "filter-map-kv" if args_list.len() == 2 => return emit_filter_map_kv(ctx, &args_list),
        _ => {}
      }
      // Check if this symbol refers to an inline lambda captured in this scope.
      if let Some(closure) = ctx.lambda_locals.get(name).cloned() {
        return emit_inline_closure_call(ctx, &closure, &args_list);
      }
      if emit_specialized_static_call(ctx, name, &args_list)? {
        return Ok(());
      }
      let fn_idx = *ctx
        .fn_index
        .get(name)
        .ok_or_else(|| format!("unknown function symbol in direct call: {sym:?}"))?;
      let target_arity = ctx.fn_arity.get(name).copied().unwrap_or(args_list.len() as u32);
      let rest_fixed = ctx.fn_has_rest.get(name).copied();
      emit_call_args(ctx, &args_list, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    Calcit::Registered(name) => {
      // Registered procs (eprintln, println, echo, etc.)
      let name = name.as_ref();
      if matches!(name, "println" | "eprintln" | "echo") {
        if ctx.target == WasmTarget::Wasi && ctx.boundary == WasmBoundary::Component {
          return emit_wasi_component_print(ctx, name, &args_list);
        }
        if ctx.target == WasmTarget::Wasi {
          return emit_wasi_print(ctx, name, &args_list);
        }
        let log_idx = resolve_host_import(ctx, "io", "log_value")?;
        for arg in &args_list {
          emit_expr(ctx, arg)?;
          ctx.emit(Instruction::Call(log_idx));
          ctx.emit(Instruction::Drop);
        }
        ctx.emit(f64_const(0.0)); // nil
        return Ok(());
      }
      Err(format!("unsupported registered proc in WASM: {name}"))
    }
    Calcit::Fn { info, .. } => {
      let def_ref = info.def_ref.as_ref().ok_or_else(|| {
        format!(
          "function literal without def reference is not supported in WASM: {}/{}",
          info.def_ns, info.name
        )
      })?;
      // Apply HOF interceptors for calcit.core functions referenced as Fn values
      // (same logic as the Import arm, but using def_ref for the name lookup).
      if def_ref.def_ns.as_ref() == "calcit.core" {
        match def_ref.def_name.as_ref() {
          "map" if args_list.len() == 2 => return emit_map(ctx, &args_list),
          "filter" | "&list:filter" | "&set:filter" if args_list.len() == 2 => return emit_filter(ctx, &args_list),
          "filter-not" if args_list.len() == 2 => return emit_filter_not(ctx, &args_list),
          "each" if args_list.len() == 2 => return emit_each(ctx, &args_list),
          "any?" if args_list.len() == 2 => return emit_any(ctx, &args_list),
          "every?" if args_list.len() == 2 => return emit_every(ctx, &args_list),
          "find" if args_list.len() == 2 => return emit_find(ctx, &args_list),
          "find-index" if args_list.len() == 2 => return emit_find_index(ctx, &args_list),
          "map-indexed" if args_list.len() == 2 => return emit_map_indexed(ctx, &args_list),
          "mapcat" if args_list.len() == 2 => return emit_mapcat(ctx, &args_list),
          "reduce" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
          "foldl'" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
          "update" if args_list.len() == 3 => return emit_update(ctx, &args_list),
          "map-kv" if args_list.len() == 2 => return emit_map_kv(ctx, &args_list),
          "filter-map-kv" if args_list.len() == 2 => return emit_filter_map_kv(ctx, &args_list),
          _ => {}
        }
      }
      let qualified = format!("{}/{}", def_ref.def_ns, def_ref.def_name);
      if emit_specialized_static_call(ctx, &qualified, &args_list)? {
        return Ok(());
      }
      let fn_idx = ctx
        .fn_index
        .get(&qualified)
        .or_else(|| ctx.fn_index.get(def_ref.def_name.as_ref()))
        .copied()
        .ok_or_else(|| format!("unknown function literal target in WASM: {qualified}"))?;
      let target_arity = ctx
        .fn_arity
        .get(&qualified)
        .or_else(|| ctx.fn_arity.get(def_ref.def_name.as_ref()))
        .copied()
        .unwrap_or(args_list.len() as u32);
      let rest_fixed = ctx
        .fn_has_rest
        .get(&qualified)
        .or_else(|| ctx.fn_has_rest.get(def_ref.def_name.as_ref()))
        .copied();
      emit_call_args(ctx, &args_list, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    // Inline IIFE: call head is `(defn name (params...) body...)`
    // Generated by `apply-args` and similar macros for TCO-friendly recursive loops.
    Calcit::List(iife_items) if matches!(iife_items.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) => {
      let params = match iife_items.get(2) {
        Some(Calcit::List(param_list)) => param_list
          .iter()
          .filter_map(|p| match p {
            Calcit::Local(CalcitLocal { sym, .. }) => Some(sym.as_ref().to_owned()),
            Calcit::Symbol { sym, .. } => Some(sym.as_ref().to_owned()),
            _ => None,
          })
          .collect::<Vec<_>>(),
        other => return Err(format!("IIFE defn: params list expected, got: {other:?}")),
      };
      let body: Vec<Calcit> = iife_items.iter().skip(3).cloned().collect();
      emit_inline_iife(ctx, &params, &body, &args_list)
    }
    // Dynamic call via a local variable holding a function table index (f64).
    // Implements Rust-style `dyn Fn` dispatch via WASM call_indirect.
    Calcit::Local(local) => {
      let local_name = local.sym.as_ref().to_string();
      // If this local is an inline lambda, inline the call directly.
      if let Some(closure) = ctx.lambda_locals.get(&local_name).cloned() {
        return emit_inline_closure_call(ctx, &closure, &args_list);
      }
      if args_list.iter().any(|arg| resolve_inline_closure(ctx, arg).is_some()) {
        return Err(format!(
          "E_WASM_CLOSURE_SPECIALIZATION: dynamic callee `{local_name}` cannot receive an inline closure"
        ));
      }
      let local_idx = *ctx
        .locals
        .get(&local_name)
        .ok_or_else(|| format!("undefined local used as function: {}", local.sym))?;
      // Emit all arguments
      for arg in &args_list {
        emit_expr(ctx, arg)?;
      }
      // Load function table index (f64) and truncate to i32 for call_indirect
      ctx.emit(Instruction::LocalGet(local_idx));
      ctx.emit(Instruction::I32TruncF64S);
      // Canonical type index = number of args (type N = (f64×N) → f64)
      ctx.emit(Instruction::CallIndirect {
        type_index: args_list.len() as u32,
        table_index: 0,
      });
      Ok(())
    }
    _ => Err(format!("unsupported call head in WASM: {head}")),
  }
}

fn emit_call_spread(ctx: &mut WasmGenCtx, args_list: &[Calcit]) -> Result<(), String> {
  if args_list.is_empty() {
    return Err("&call-spread expects at least a callee".into());
  }

  let head = &args_list[0];
  let call_args = &args_list[1..];
  if call_args.iter().any(|arg| resolve_inline_closure(ctx, arg).is_some()) {
    return Err("E_WASM_CLOSURE_SPECIALIZATION: spread calls cannot receive inline closures".into());
  }

  match head {
    Calcit::Import(import) => {
      let qualified = format!("{}/{}", import.ns, import.def);
      let fn_idx = ctx
        .fn_index
        .get(&qualified)
        .or_else(|| ctx.fn_index.get(import.def.as_ref()))
        .copied()
        .ok_or_else(|| format!("unknown function: {qualified}"))?;
      let target_arity = ctx
        .fn_arity
        .get(&qualified)
        .or_else(|| ctx.fn_arity.get(import.def.as_ref()))
        .copied()
        .unwrap_or(call_args.len() as u32);
      let rest_fixed = ctx
        .fn_has_rest
        .get(&qualified)
        .or_else(|| ctx.fn_has_rest.get(import.def.as_ref()))
        .copied();
      emit_call_spread_args(ctx, call_args, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    Calcit::Symbol { sym, .. } => {
      let name = sym.as_ref();
      let fn_idx = *ctx
        .fn_index
        .get(name)
        .ok_or_else(|| format!("unknown function symbol in spread call: {sym:?}"))?;
      let target_arity = ctx.fn_arity.get(name).copied().unwrap_or(call_args.len() as u32);
      let rest_fixed = ctx.fn_has_rest.get(name).copied();
      emit_call_spread_args(ctx, call_args, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    Calcit::Fn { info, .. } => {
      let def_ref = info.def_ref.as_ref().ok_or_else(|| {
        format!(
          "function literal without def reference is not supported in WASM: {}/{}",
          info.def_ns, info.name
        )
      })?;
      let qualified = format!("{}/{}", def_ref.def_ns, def_ref.def_name);
      let fn_idx = ctx
        .fn_index
        .get(&qualified)
        .or_else(|| ctx.fn_index.get(def_ref.def_name.as_ref()))
        .copied()
        .ok_or_else(|| format!("unknown function literal target in WASM: {qualified}"))?;
      let target_arity = ctx
        .fn_arity
        .get(&qualified)
        .or_else(|| ctx.fn_arity.get(def_ref.def_name.as_ref()))
        .copied()
        .unwrap_or(call_args.len() as u32);
      let rest_fixed = ctx
        .fn_has_rest
        .get(&qualified)
        .or_else(|| ctx.fn_has_rest.get(def_ref.def_name.as_ref()))
        .copied();
      emit_call_spread_args(ctx, call_args, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    Calcit::Proc(proc) => {
      if matches!(proc, CalcitProc::Recur | CalcitProc::NativeListDissoc | CalcitProc::NativeMapDissoc) {
        // These procs handle ArgSpread markers themselves — pass call_args raw.
        emit_proc_call(ctx, proc, call_args)
      } else {
        // A proc used as the spread callee — just emit a regular proc call with spread args
        emit_call_spread_args_as_regular(ctx, proc, call_args)
      }
    }
    // Method spread call: e.g. `(.dissoc x & args)` — emit unreachable trap.
    // In practice reached only for struct/enum methods not yet implemented in WASM.
    // List/map cases are handled via their native procs before this point.
    Calcit::Method(_name, MethodKind::Invoke(_)) => {
      ctx.emit(Instruction::Unreachable);
      Ok(())
    }
    // Dynamic spread call via a local holding a function value — emit unreachable.
    Calcit::Local(_) => {
      ctx.emit(Instruction::Unreachable);
      Ok(())
    }
    _ => Err(format!("unsupported call head in WASM: {head}")),
  }
}

/// For proc callees in spread calls: collect args (handling `& spread-list`) and emit proc.
fn emit_call_spread_args_as_regular(ctx: &mut WasmGenCtx, proc: &CalcitProc, call_args: &[Calcit]) -> Result<(), String> {
  // Gather concrete args, expanding `& spread-list` if present
  let mut real_args: Vec<Calcit> = vec![];
  let mut i = 0;
  while i < call_args.len() {
    if matches!(call_args[i], Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
      i += 1; // skip the & marker, next is the spread list
    } else {
      real_args.push(call_args[i].clone());
    }
    i += 1;
  }
  emit_proc_call(ctx, proc, &real_args)
}

fn emit_call_spread_args(ctx: &mut WasmGenCtx, call_args: &[Calcit], target_arity: u32, rest_fixed: Option<u32>) -> Result<(), String> {
  let Some(fixed) = rest_fixed else {
    return Err("&call-spread in WASM currently requires the target function to accept rest args".into());
  };
  let fixed = fixed as usize;

  // Locate the ArgSpread marker (if any) to split explicit args from the spread list.
  let spread_pos = call_args
    .iter()
    .position(|a| matches!(a, Calcit::Syntax(CalcitSyntax::ArgSpread, _)));

  let (explicit_args, spread_list_opt): (&[Calcit], Option<&Calcit>) = if let Some(pos) = spread_pos {
    (&call_args[..pos], call_args.get(pos + 1))
  } else if call_args.len() == fixed + 1 {
    // Old-style: no ArgSpread marker; last arg is the rest list.
    (&call_args[..fixed], Some(&call_args[fixed]))
  } else {
    return Err(format!(
      "&call-spread in WASM expects {} fixed args plus `& spread-list`, got {} args",
      fixed,
      call_args.len()
    ));
  };

  let Some(spread_list_expr) = spread_list_opt else {
    return Err("&call-spread missing spread list expression".into());
  };

  let n_explicit = explicit_args.len();
  let n_from_spread = fixed.saturating_sub(n_explicit);

  // Emit explicit fixed args.
  for arg in explicit_args {
    emit_expr(ctx, arg)?;
  }

  if n_from_spread == 0 {
    // All fixed params already emitted; spread_list_expr IS the rest arg.
    emit_expr(ctx, spread_list_expr)?;
  } else {
    // Need to extract `n_from_spread` fixed params from the spread list,
    // then emit the remainder as the rest arg.
    emit_expr(ctx, spread_list_expr)?;
    let spread_f64 = ctx.alloc_local();
    ctx.emit(Instruction::LocalSet(spread_f64));
    let spread_i32 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(spread_f64));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(spread_i32));

    // Extract each required fixed param: list[i] = spread_i32[(1+i)*8]
    for i in 0..n_from_spread {
      ctx.emit(Instruction::LocalGet(spread_i32));
      ctx.emit(Instruction::I32Const(((1 + i) * 8) as i32));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    }

    // Emit the rest arg = list_slice(spread, n_from_spread).
    emit_list_slice_from_i32_local(ctx, spread_i32, n_from_spread)?;
  }

  let emitted_args = fixed + 1;
  for _ in emitted_args..(target_arity as usize) {
    ctx.emit(f64_const(0.0));
  }

  Ok(())
}

/// Emit a new list containing `src_i32[from_idx..]`.
/// `src_i32` is a WASM local holding the I32 pointer to the source list.
/// Leaves an f64 pointer to the new slice list on the stack.
fn emit_list_slice_from_i32_local(ctx: &mut WasmGenCtx, src_i32: u32, from_idx: usize) -> Result<(), String> {
  // old_count = load count field (f64 → i32)
  let old_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src_i32));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(old_count));

  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::I32Const(from_idx as i32));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(new_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, new_count, total_slots, "list");

  // src_base = src_i32 + 8 + from_idx*8
  let src_base = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src_i32));
  ctx.emit(Instruction::I32Const((8 + from_idx * 8) as i32));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_base));

  let dst_base = emit_addr_offset(ctx, dst, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, new_count);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Emit instructions for builtin proc calls.
fn emit_proc_call(ctx: &mut WasmGenCtx, proc: &CalcitProc, args: &[Calcit]) -> Result<(), String> {
  match proc {
    // Arithmetic
    CalcitProc::NativeAdd => emit_binary(ctx, Instruction::F64Add, args),
    CalcitProc::NativeMinus => emit_binary(ctx, Instruction::F64Sub, args),
    CalcitProc::NativeMultiply => emit_binary(ctx, Instruction::F64Mul, args),
    CalcitProc::NativeDivide => emit_binary(ctx, Instruction::F64Div, args),
    CalcitProc::NativeNumberRem => {
      // a - trunc(a/b) * b
      if args.len() != 2 {
        return Err("rem expects 2 args".into());
      }
      emit_expr(ctx, &args[0])?; // a
      emit_expr(ctx, &args[0])?; // a (again)
      emit_expr(ctx, &args[1])?; // b
      ctx.emit(Instruction::F64Div);
      ctx.emit(Instruction::F64Trunc);
      emit_expr(ctx, &args[1])?; // b (again)
      ctx.emit(Instruction::F64Mul);
      ctx.emit(Instruction::F64Sub);
      Ok(())
    }

    // Comparisons — produce f64 (1.0 or 0.0)
    CalcitProc::NativeLessThan => emit_cmp(ctx, Instruction::F64Lt, args),
    CalcitProc::NativeGreaterThan => emit_cmp(ctx, Instruction::F64Gt, args),
    CalcitProc::NativeEquals => emit_equals(ctx, args),
    CalcitProc::Identical => emit_cmp(ctx, Instruction::F64Eq, args),
    CalcitProc::NativeCompare => {
      // &compare a b → -1.0 if a<b, 0.0 if a==b, 1.0 if a>b
      expect_arity(2, args, "&compare")?;
      let a = ctx.alloc_local();
      let b = ctx.alloc_local();
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::LocalSet(a));
      emit_expr(ctx, &args[1])?;
      ctx.emit(Instruction::LocalSet(b));
      // if a < b then -1 else (if a > b then 1 else 0)
      ctx.emit(Instruction::LocalGet(a));
      ctx.emit(Instruction::LocalGet(b));
      ctx.emit(Instruction::F64Lt);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      ctx.emit(f64_const(-1.0));
      ctx.emit(Instruction::Else);
      ctx.emit(Instruction::LocalGet(a));
      ctx.emit(Instruction::LocalGet(b));
      ctx.emit(Instruction::F64Gt);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      ctx.emit(f64_const(1.0));
      ctx.emit(Instruction::Else);
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);
      Ok(())
    }
    CalcitProc::Not => {
      expect_arity(1, args, "not")?;
      // not: 0.0 → 1.0, else → 0.0
      ctx.emit(f64_const(1.0)); // true result
      ctx.emit(f64_const(0.0)); // false result
      emit_expr(ctx, &args[0])?;
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Eq); // i32 condition
      ctx.emit(Instruction::Select);
      Ok(())
    }

    // Math (unary)
    CalcitProc::Floor => emit_unary(ctx, Instruction::F64Floor, args),
    CalcitProc::Ceil => emit_unary(ctx, Instruction::F64Ceil, args),
    CalcitProc::Round => emit_unary(ctx, Instruction::F64Nearest, args),
    CalcitProc::Sqrt => emit_unary(ctx, Instruction::F64Sqrt, args),

    // round?: x == floor(x)
    CalcitProc::IsRound => {
      expect_arity(1, args, "round?")?;
      let v = ctx.alloc_local();
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::LocalSet(v));
      ctx.emit(Instruction::LocalGet(v));
      ctx.emit(Instruction::F64Floor);
      ctx.emit(Instruction::LocalGet(v));
      ctx.emit(Instruction::F64Eq);
      ctx.emit(Instruction::F64ConvertI32U);
      Ok(())
    }

    // &number:fract: x - floor(x)
    CalcitProc::NativeNumberFract => {
      expect_arity(1, args, "&number:fract")?;
      let v = ctx.alloc_local();
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::LocalSet(v));
      ctx.emit(Instruction::LocalGet(v));
      ctx.emit(Instruction::LocalGet(v));
      ctx.emit(Instruction::F64Floor);
      ctx.emit(Instruction::F64Sub);
      Ok(())
    }
    CalcitProc::NativeNumberFits => emit_number_fits(ctx, args),
    CalcitProc::Sin => emit_host_call(ctx, "sin", args),
    CalcitProc::Cos => emit_host_call(ctx, "cos", args),
    CalcitProc::Pow => emit_host_call(ctx, "pow", args),

    // type-of: reads the heap type header or returns :number for non-pointers.
    CalcitProc::TypeOf => emit_type_of(ctx, args),

    // type predicates
    CalcitProc::ListQuestion => emit_type_predicate(ctx, "list", args),
    CalcitProc::TagQuestion => emit_type_predicate(ctx, "tag", args),
    CalcitProc::SymbolQuestion => emit_type_predicate(ctx, "symbol", args),
    CalcitProc::NilQuestion => {
      expect_arity(1, args, "nil?")?;
      // Current WASM backend represents nil as 0.0.
      ctx.emit(f64_const(1.0));
      ctx.emit(f64_const(0.0));
      emit_expr(ctx, &args[0])?;
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Eq);
      ctx.emit(Instruction::Select);
      Ok(())
    }
    CalcitProc::StringQuestion => emit_type_predicate(ctx, "string", args),
    CalcitProc::MapQuestion => emit_type_predicate(ctx, "map", args),
    CalcitProc::NumberQuestion => emit_type_predicate(ctx, "number", args),
    CalcitProc::BoolQuestion => emit_type_predicate(ctx, "bool", args),
    CalcitProc::SetQuestion => emit_type_predicate(ctx, "set", args),
    // The current WASM heap ABI keeps its legacy storage tag numbers; these
    // strings are not exposed as Calcit language type names.
    CalcitProc::EnumQuestion => emit_type_predicate(ctx, "enum", args),
    CalcitProc::StructQuestion => emit_type_predicate(ctx, "struct", args),
    CalcitProc::FnQuestion => emit_type_predicate(ctx, "fn", args),

    // Recur
    CalcitProc::Recur => {
      // Check for spread: args may be [a1, ..., ak, ArgSpread, rest_list]
      let spread_pos = args.iter().position(|a| matches!(a, Calcit::Syntax(CalcitSyntax::ArgSpread, _)));

      if let Some(pos) = spread_pos {
        // Spread recur: unpack rest list into remaining param slots.
        let explicit = &args[..pos];
        let rest_expr = args.get(pos + 1).ok_or("recur spread: missing list after &")?;
        let total = ctx.arg_indices.len();
        let n_explicit = explicit.len();
        if n_explicit >= total {
          return Err(format!("recur spread: too many explicit args ({n_explicit} >= {total})"));
        }
        let n_remaining = total - n_explicit; // includes rest-list slot

        // Evaluate spread list into a temp local.
        emit_expr(ctx, rest_expr)?;
        let spread_f64 = ctx.alloc_local();
        ctx.emit(Instruction::LocalSet(spread_f64));
        let spread_i32 = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(spread_f64));
        ctx.emit(Instruction::I32TruncF64U);
        ctx.emit(Instruction::LocalSet(spread_i32));

        // Evaluate explicit prefix args into temps.
        let mut temps = Vec::new();
        for arg in explicit {
          let tmp = ctx.alloc_local();
          emit_expr(ctx, arg)?;
          ctx.emit(Instruction::LocalSet(tmp));
          temps.push(tmp);
        }

        // Extract (n_remaining - 1) individual elements from spread list for the remaining fixed slots.
        for i in 0..(n_remaining - 1) {
          let tmp = ctx.alloc_local();
          ctx.emit(Instruction::LocalGet(spread_i32));
          ctx.emit(Instruction::I32Const(((1 + i) * 8) as i32));
          ctx.emit(Instruction::I32Add);
          ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
          ctx.emit(Instruction::LocalSet(tmp));
          temps.push(tmp);
        }

        // Last slot: list_slice(spread, n_remaining-1) — the new rest list.
        let rest_slice_local = ctx.alloc_local();
        emit_list_slice_from_i32_local(ctx, spread_i32, n_remaining - 1)?;
        ctx.emit(Instruction::LocalSet(rest_slice_local));
        temps.push(rest_slice_local);

        if temps.len() != total {
          return Err(format!("recur spread: computed {} temps but need {}", temps.len(), total));
        }

        // Copy temps into arg locals.
        for (i, &tmp) in temps.iter().enumerate() {
          ctx.emit(Instruction::LocalGet(tmp));
          ctx.emit(Instruction::LocalSet(ctx.arg_indices[i]));
        }
        ctx.emit(Instruction::Br(ctx.block_depth));
        ctx.emit(Instruction::Unreachable);
        Ok(())
      } else {
        // Non-spread recur.
        if args.len() != ctx.arg_indices.len() {
          return Err(format!(
            "recur arity mismatch: expected {}, got {}",
            ctx.arg_indices.len(),
            args.len()
          ));
        }
        // Evaluate all args into temp locals first
        let mut temps = Vec::new();
        for arg in args {
          let tmp = ctx.alloc_local();
          emit_expr(ctx, arg)?;
          ctx.emit(Instruction::LocalSet(tmp));
          temps.push(tmp);
        }
        // Copy temps back to arg locals
        for (i, &tmp) in temps.iter().enumerate() {
          ctx.emit(Instruction::LocalGet(tmp));
          ctx.emit(Instruction::LocalSet(ctx.arg_indices[i]));
        }
        ctx.emit(Instruction::Br(ctx.block_depth)); // br to the recur loop
        // After unconditional br, mark as unreachable for the type checker
        ctx.emit(Instruction::Unreachable);
        Ok(())
      }
    }

    // Struct operations
    CalcitProc::NativeStruct => emit_struct_new(ctx, args),
    CalcitProc::NativeStructNth => emit_struct_nth(ctx, args),
    CalcitProc::NativeStructGet => emit_struct_get(ctx, args),
    CalcitProc::NativeStructCount => emit_struct_count(ctx, args),
    CalcitProc::NativeStructFieldTag => emit_struct_field_tag(ctx, args),
    CalcitProc::NativeStructDefinition => emit_struct_def(ctx, args),
    CalcitProc::NativeStructGetName => emit_struct_get_name(ctx, args),
    CalcitProc::NativeStructToMap => emit_struct_to_map(ctx, args),
    CalcitProc::NativeStructAssoc | CalcitProc::NativeStructAssocAt | CalcitProc::NativeStructWith => {
      Err(format!("{proc} not yet supported in WASM codegen"))
    }
    CalcitProc::NativeStructFromMap
    | CalcitProc::NativeStructExtendAs
    | CalcitProc::NativeStructImpls
    | CalcitProc::NativeStructWithAt
    | CalcitProc::NativeLooseStruct => Err(format!("Struct operation {proc} not yet supported in WASM codegen")),
    CalcitProc::NativeStructContains => emit_struct_contains(ctx, args),
    CalcitProc::NativeStructMatches => emit_struct_matches(ctx, args),

    // Enum operations
    CalcitProc::NativeEnum => emit_enum_new(ctx, args),
    CalcitProc::NativeEnumNth => emit_enum_nth(ctx, args),
    CalcitProc::NativeEnumCount => emit_enum_count(ctx, args),
    CalcitProc::NativeEnumValidate => ctx.stub_proc(args), // no-op in WASM
    // %:: enum variant constructor: (enum_class tag payload...) — ignore enum_class
    CalcitProc::NativeNamedEnumNew => emit_named_enum_new(ctx, args),
    CalcitProc::NativeEnumImpls
    | CalcitProc::NativeEnumParams
    | CalcitProc::NativeEnumDefinition
    | CalcitProc::NativeEnumValueImplTraits
    | CalcitProc::NativeEnumDefHasVariant
    | CalcitProc::NativeEnumDefVariantArity => Err(format!("Enum operation {proc} not yet supported in WASM codegen")),
    CalcitProc::NativeEnumAssoc => emit_enum_assoc(ctx, args),

    // Bitwise operations — convert to i32, operate, convert back to f64
    CalcitProc::BitShl => emit_bitwise_binary(ctx, Instruction::I32Shl, args),
    CalcitProc::BitShr => emit_bitwise_binary(ctx, Instruction::I32ShrS, args),
    CalcitProc::BitAnd => emit_bitwise_binary(ctx, Instruction::I32And, args),
    CalcitProc::BitOr => emit_bitwise_binary(ctx, Instruction::I32Or, args),
    CalcitProc::BitXor => emit_bitwise_binary(ctx, Instruction::I32Xor, args),
    CalcitProc::BitNot => {
      expect_arity(1, args, "bit-not")?;
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::I32TruncF64S);
      ctx.emit(Instruction::I32Const(-1)); // all bits set
      ctx.emit(Instruction::I32Xor);
      ctx.emit(Instruction::F64ConvertI32S);
      Ok(())
    }

    // Raise — terminates execution
    CalcitProc::Raise => {
      // `raise` aborts the program; any args are evaluated for side effects but discarded.
      for arg in args {
        emit_expr(ctx, arg)?;
        ctx.emit(Instruction::Drop);
      }
      ctx.emit(Instruction::Unreachable);
      Ok(())
    }
    CalcitProc::Todo => {
      if args.len() > 1 {
        return Err(format!("todo! expects 0~1 arguments, got {}", args.len()));
      }
      if args.first().is_some_and(|arg| !matches!(arg, Calcit::Str(_))) {
        return Err("todo! expects an optional static String message".to_owned());
      }
      if let Some(arg) = args.first() {
        emit_expr(ctx, arg)?;
        ctx.emit(Instruction::Drop);
      }
      ctx.emit(Instruction::Unreachable);
      Ok(())
    }

    // ------- List operations -------
    CalcitProc::List => emit_list_new(ctx, args),
    CalcitProc::Append => emit_list_append(ctx, args),
    CalcitProc::Prepend => emit_list_prepend(ctx, args),
    CalcitProc::Butlast => emit_list_butlast(ctx, args),
    CalcitProc::NativeListAppend => emit_list_append(ctx, args),
    CalcitProc::NativeListPrepend => emit_list_prepend(ctx, args),
    CalcitProc::NativeListButlast => emit_list_butlast(ctx, args),
    CalcitProc::NativeListLast => emit_list_last(ctx, args),
    CalcitProc::NativeListSort => emit_list_sort(ctx, args),
    CalcitProc::NativeListRange => emit_range(ctx, args),
    CalcitProc::NativeListFoldl => emit_foldl(ctx, args),
    CalcitProc::NativeListFoldlShortcut => emit_foldl_shortcut(ctx, args),
    CalcitProc::NativeListCount => emit_ds_count(ctx, args),
    CalcitProc::NativeListNth => emit_list_nth(ctx, args),
    CalcitProc::NativeListFirst => emit_list_first(ctx, args),
    CalcitProc::NativeListRest => emit_list_rest(ctx, args),
    CalcitProc::NativeListEmpty => emit_ds_empty(ctx, args),
    CalcitProc::NativeListSlice => emit_list_slice(ctx, args),
    CalcitProc::NativeListReverse => emit_list_reverse(ctx, args),
    CalcitProc::NativeListConcat => emit_list_concat(ctx, args),
    CalcitProc::NativeListAssoc => emit_list_assoc(ctx, args),
    CalcitProc::NativeListAssocBefore => emit_list_assoc_before(ctx, args),
    CalcitProc::NativeListAssocAfter => emit_list_assoc_after(ctx, args),
    CalcitProc::NativeListDissoc => emit_list_dissoc(ctx, args),
    CalcitProc::NativeListToSet => emit_list_to_set(ctx, args),
    CalcitProc::NativeListContains => emit_list_contains(ctx, args),
    CalcitProc::NativeListIncludes => emit_list_includes(ctx, args),
    CalcitProc::NativeListQ => emit_list_q(ctx, args),

    // ------- BufList operations -------
    CalcitProc::NativeBufListNew => emit_buf_list_new(ctx, args),
    CalcitProc::NativeBufListPush => emit_buf_list_push(ctx, args),
    CalcitProc::NativeBufListConcat => emit_buf_list_concat(ctx, args),
    CalcitProc::NativeBufListToList => emit_buf_list_to_list(ctx, args),
    CalcitProc::NativeBufListCount => emit_buf_list_count(ctx, args),

    // ------- Map operations -------
    CalcitProc::NativeMap => emit_map_new(ctx, args),
    CalcitProc::NativeMapGet => emit_map_get_op(ctx, args),
    CalcitProc::NativeMapAssoc => emit_map_assoc(ctx, args),
    CalcitProc::NativeMapDissoc => emit_map_dissoc(ctx, args),
    CalcitProc::NativeMapCount => emit_ds_count(ctx, args),
    CalcitProc::NativeMapEmpty => emit_ds_empty(ctx, args),
    CalcitProc::NativeMapContains => emit_map_contains(ctx, args),
    CalcitProc::NativeMapIncludes => emit_map_includes(ctx, args),
    CalcitProc::ToPairs => emit_map_to_pairs(ctx, args),
    CalcitProc::NativeMapToList => emit_map_to_list(ctx, args),

    // ------- Set operations -------
    CalcitProc::Set => emit_set_new(ctx, args),
    CalcitProc::NativeInclude => emit_set_include(ctx, args),
    CalcitProc::NativeExclude => emit_set_exclude(ctx, args),
    CalcitProc::NativeSetCount => emit_ds_count(ctx, args),
    CalcitProc::NativeSetEmpty => emit_ds_empty(ctx, args),
    CalcitProc::NativeSetIncludes => emit_set_includes(ctx, args),
    CalcitProc::NativeSetToList => emit_set_to_list(ctx, args),
    CalcitProc::NativeDifference => emit_set_difference(ctx, args),
    CalcitProc::NativeUnion => emit_set_union(ctx, args),
    CalcitProc::NativeSetIntersection => emit_set_intersection(ctx, args),
    CalcitProc::NativeSetDestruct => emit_set_destruct(ctx, args),
    CalcitProc::NativeMerge => emit_map_merge(ctx, args),
    CalcitProc::NativeMergeNonNil => emit_map_merge_non_nil(ctx, args),
    CalcitProc::NativeMapDiffNew => emit_map_diff_new(ctx, args),
    CalcitProc::NativeMapDiffKeys => emit_map_diff_keys(ctx, args),
    CalcitProc::NativeMapCommonKeys => emit_map_common_keys(ctx, args),
    CalcitProc::NativeMapDiffTriple => maps::emit_map_diff_triple(ctx, args),
    CalcitProc::NativeMapDestruct => emit_map_destruct(ctx, args),
    CalcitProc::NativeMapKeys => emit_map_keys(ctx, args),
    CalcitProc::NativeMapVals => emit_map_vals(ctx, args),
    CalcitProc::Range => emit_range(ctx, args),
    CalcitProc::NativeHash => emit_hash_proc(ctx, args),

    // ------- String operations -------
    CalcitProc::NativeStrCount => emit_str_count(ctx, args),
    CalcitProc::NativeStrUtf8ByteCount => emit_str_utf8_byte_count(ctx, args),
    CalcitProc::NativeStrEmpty => emit_str_empty(ctx, args),
    CalcitProc::NativeStrConcat => emit_str_concat(ctx, args),
    CalcitProc::NativeStrNth => emit_str_nth(ctx, args),
    CalcitProc::NativeStrFirst => emit_str_first(ctx, args),
    CalcitProc::NativeStrRest => emit_str_rest(ctx, args),
    CalcitProc::NativeStrSlice => emit_str_slice(ctx, args),
    CalcitProc::NativeStrCompare => emit_str_compare(ctx, args),
    CalcitProc::NativeStrContains => emit_str_contains(ctx, args),
    CalcitProc::NativeStrIncludes => emit_str_includes(ctx, args),
    CalcitProc::NativeStrFindIndex => emit_str_find_index(ctx, args),
    CalcitProc::NativeStrPadLeft => emit_str_pad_left(ctx, args),
    CalcitProc::NativeStrPadRight => emit_str_pad_right(ctx, args),
    CalcitProc::StartsWith => emit_str_starts_with(ctx, args),
    CalcitProc::EndsWith => emit_str_ends_with(ctx, args),
    CalcitProc::TurnString | CalcitProc::NativeStr => emit_turn_string(ctx, args),
    CalcitProc::Trim => emit_trim(ctx, args),
    CalcitProc::IsBlank => emit_blank(ctx, args),
    CalcitProc::GetCharCode => emit_get_char_code(ctx, args),
    CalcitProc::ParseFloat => emit_parse_float(ctx, args),
    CalcitProc::CharFromCode => emit_char_from_code(ctx, args),
    CalcitProc::NativeStrReplace => emit_str_replace(ctx, args),
    CalcitProc::NativeStrEscape => emit_str_escape(ctx, args),
    CalcitProc::FormatCirruEdn => emit_format_cirru_edn(ctx, args),
    CalcitProc::Split => emit_split(ctx, args),
    CalcitProc::SplitLines => emit_split_lines(ctx, args),

    // --- List higher-order and utility operations ---
    CalcitProc::NativeListDistinct => emit_list_distinct(ctx, args),

    // Higher-order list operations
    CalcitProc::Foldl => emit_foldl(ctx, args),
    CalcitProc::FoldlShortcut => emit_foldl_shortcut(ctx, args),
    CalcitProc::FoldrShortcut => emit_foldr_shortcut(ctx, args),

    // Format (stub — only used in raise/error paths)
    CalcitProc::FormatToLisp => emit_format_to_lisp(ctx, args),
    // to-lispy-string — stub, only used in raise/error message paths
    CalcitProc::PrStr => ctx.stub_proc(args),
    CalcitProc::GetEnv => {
      if !(1..=2).contains(&args.len()) {
        return Err(format!("get-env expects 1~2 args, got {}", args.len()));
      }
      emit_expr(ctx, &args[0])?;
      let name = ctx.alloc_local();
      ctx.emit(Instruction::LocalSet(name));
      let default = if let Some(default) = args.get(1) {
        emit_expr(ctx, default)?;
        let value = ctx.alloc_local();
        ctx.emit(Instruction::LocalSet(value));
        Some(value)
      } else {
        None
      };
      match ctx.target {
        WasmTarget::Core => {
          ctx.emit(Instruction::LocalGet(name));
          ctx.emit(Instruction::Call(resolve_host_import(ctx, "io", "get_env")?));
        }
        WasmTarget::Wasi => {
          ctx.emit(Instruction::LocalGet(name));
          ctx.emit(Instruction::I32TruncF64U);
          ctx.call_rt("__rt_wasi_get_env");
        }
      }
      if let Some(default) = default {
        let value = ctx.alloc_local();
        ctx.emit(Instruction::LocalSet(value));
        ctx.emit(Instruction::LocalGet(value));
        ctx.emit(f64_const(0.0));
        ctx.emit(Instruction::F64Eq);
        ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
        ctx.emit(Instruction::LocalGet(default));
        ctx.emit(Instruction::Else);
        ctx.emit(Instruction::LocalGet(value));
        ctx.emit(Instruction::End);
      }
      Ok(())
    }
    CalcitProc::GetArgs => {
      expect_arity(0, args, "get-args")?;
      if ctx.target != WasmTarget::Wasi {
        return Err("E_WASM_CAPABILITY: process arguments are unavailable for the core WASM target".into());
      }
      ctx.call_rt("__rt_wasi_get_args");
      Ok(())
    }
    CalcitProc::UnixTimeMs => {
      expect_arity(0, args, "unix-time-ms")?;
      emit_wasi_clock_ms(ctx, 0, "unix-time-ms")
    }
    CalcitProc::CpuTime => {
      expect_arity(0, args, "cpu-time")?;
      emit_wasi_clock_ms(ctx, 1, "cpu-time")
    }
    CalcitProc::NativeWaitMs => emit_wasi_wait_ms(ctx, args),
    CalcitProc::NativeSecureRandomBytes => emit_wasi_secure_random_bytes(ctx, args),
    CalcitProc::NativeFsReadText => emit_wasi_fs_read_text(ctx, args),
    CalcitProc::NativeFsReadDir => emit_wasi_fs_read_dir(ctx, args),
    CalcitProc::NativeFsWriteText => emit_wasi_fs_write_text(ctx, args),

    // @atom deref: just emit the argument (which should already be a GlobalGet)
    CalcitProc::AtomDeref => {
      expect_arity(1, args, "&atom:deref")?;
      emit_expr(ctx, &args[0])
    }

    CalcitProc::Quit => {
      expect_arity(1, args, "quit!")?;
      emit_expr(ctx, &args[0])?;
      let code = ctx.alloc_local();
      ctx.emit(Instruction::LocalSet(code));
      if ctx.target == WasmTarget::Wasi {
        ctx.emit(Instruction::LocalGet(code));
        ctx.emit(f64_const(0.0));
        ctx.emit(Instruction::F64Lt);
        ctx.emit(Instruction::LocalGet(code));
        ctx.emit(f64_const(255.0));
        ctx.emit(Instruction::F64Gt);
        ctx.emit(Instruction::I32Or);
        ctx.emit(Instruction::LocalGet(code));
        ctx.emit(Instruction::LocalGet(code));
        ctx.emit(Instruction::F64Trunc);
        ctx.emit(Instruction::F64Ne);
        ctx.emit(Instruction::I32Or);
        ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
        ctx.emit(Instruction::Unreachable);
        ctx.emit(Instruction::End);
        ctx.emit(Instruction::LocalGet(code));
        ctx.emit(Instruction::I32TruncF64U);
        let (module, name) = if ctx.boundary == WasmBoundary::Component {
          ("wasi:cli/exit@0.3.1", "exit-with-code")
        } else {
          ("wasi_snapshot_preview1", "proc_exit")
        };
        ctx.emit(Instruction::Call(resolve_host_import(ctx, module, name)?));
      }
      // Core WASM has no process-exit capability, and a conforming WASI host
      // never returns from either exit import. Trap if execution reaches here.
      ctx.emit(Instruction::Unreachable);
      ctx.emit(f64_const(0.0)); // unreachable, but keeps type stack valid
      Ok(())
    }

    // &get-calcit-backend — runtime-env query, not meaningful in WASM; return nil.
    CalcitProc::NativeGetCalcitBackend => {
      ctx.emit(f64_const(0.0));
      Ok(())
    }

    // turn-tag — converts a string to a tag at runtime; in WASM, return the string as-is.
    CalcitProc::TurnTag => {
      expect_arity(1, args, "turn-tag")?;
      emit_expr(ctx, &args[0])
    }

    // Runtime trait tables are intentionally not implemented in the internal
    // WASM validation backend. Preprocessing must eliminate these operations.
    CalcitProc::NativeStructImplTraits | CalcitProc::NativeEnumImplTraits => {
      Err("runtime trait registration must be eliminated before WASM codegen".into())
    }

    // register-calcit-builtin-impls — builtin impl registration; not meaningful in WASM; return nil.
    CalcitProc::RegisterCalcitBuiltinImpls => {
      eprintln!("[wasm warning] RegisterCalcitBuiltinImpls is ignored in WASM (builtin impls already registered)");
      ctx.silent_nil()
    }

    // &impl::new — only valid as compile-time metadata for WASM.
    CalcitProc::NativeImplNew => Err("runtime trait impl construction must be eliminated before WASM codegen".into()),

    // &assert-traits — a residual assertion would require a runtime trait table.
    CalcitProc::NativeAssertTraits => Err(
      "runtime trait assertions are not supported by the internal WASM backend; make the receiver type statically resolvable".into(),
    ),

    // &get-os — host OS info; not available in WASM; return nil.
    CalcitProc::NativeGetOs => ctx.stub_proc(args),

    // definition metadata — not available in WASM; return nil.
    CalcitProc::NativeGetDefDoc | CalcitProc::NativeGetDefSchema => ctx.stub_proc(args),

    // &number:display-by — radix string formatting.
    CalcitProc::NativeNumberDisplayBy => {
      if args.len() != 2 {
        return Err("&number:display-by expects 2 args".into());
      }
      emit_expr(ctx, &args[0])?; // value f64
      emit_expr(ctx, &args[1])?; // radix f64
      ctx.call_rt("__rt_display_by");
      Ok(())
    }

    // &number:format — formatting; stub in WASM.
    CalcitProc::NativeNumberFormat => ctx.stub_proc(args),

    CalcitProc::Sort => emit_list_sort(ctx, args),

    // Not yet supported
    _ => Err(format!("unsupported proc in WASM: {proc}")),
  }
}

/// Emit the runtime refinement predicate used by the checked numeric conversion helpers.
fn emit_number_fits(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&number:fits?")?;
  let value = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(value));

  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));

  let result = ctx.alloc_local_typed(ValType::I32);
  // Unknown runtime tags produce false. The Bool ABI has no recoverable error
  // payload, and a predicate must not turn dynamic input into a WASM trap.
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(result));

  for refinement in [
    CalcitNumericRefinement::Int8,
    CalcitNumericRefinement::UInt8,
    CalcitNumericRefinement::Int16,
    CalcitNumericRefinement::UInt16,
    CalcitNumericRefinement::Int32,
    CalcitNumericRefinement::UInt32,
    CalcitNumericRefinement::Int64,
    CalcitNumericRefinement::UInt64,
    CalcitNumericRefinement::Float32,
    CalcitNumericRefinement::Float64,
  ] {
    let tag_id = *ctx.tag_index.get(refinement.tag_name()).ok_or_else(|| {
      format!(
        "numeric refinement tag :{} is missing from the WASM tag index",
        refinement.tag_name()
      )
    })?;
    ctx.emit(Instruction::LocalGet(target));
    ctx.emit(f64_const(tag_id as f64));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    emit_number_refinement_test(ctx, value, refinement);
    ctx.emit(Instruction::LocalSet(result));
    ctx.emit(Instruction::End);
  }

  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Leave an i32 predicate result on the WASM stack for one numeric refinement.
fn emit_number_refinement_test(ctx: &mut WasmGenCtx, value: u32, refinement: CalcitNumericRefinement) {
  match refinement {
    CalcitNumericRefinement::Float64 => ctx.emit(Instruction::I32Const(1)),
    CalcitNumericRefinement::Float32 => {
      // Equality after an f32 roundtrip rejects NaN and values that lose precision.
      ctx.emit(Instruction::LocalGet(value));
      ctx.emit(Instruction::F32DemoteF64);
      ctx.emit(Instruction::F64PromoteF32);
      ctx.emit(Instruction::LocalGet(value));
      ctx.emit(Instruction::F64Eq);
    }
    refinement => {
      let (min, max) = match refinement {
        CalcitNumericRefinement::Int8 => (-128.0, 127.0),
        CalcitNumericRefinement::UInt8 => (0.0, 255.0),
        CalcitNumericRefinement::Int16 => (-32_768.0, 32_767.0),
        CalcitNumericRefinement::UInt16 => (0.0, 65_535.0),
        CalcitNumericRefinement::Int32 => (i32::MIN as f64, i32::MAX as f64),
        CalcitNumericRefinement::UInt32 => (0.0, u32::MAX as f64),
        CalcitNumericRefinement::Int64 => (-9_007_199_254_740_991.0, 9_007_199_254_740_991.0),
        CalcitNumericRefinement::UInt64 => (0.0, 9_007_199_254_740_991.0),
        CalcitNumericRefinement::Float32 | CalcitNumericRefinement::Float64 => unreachable!(),
      };
      ctx.emit(Instruction::LocalGet(value));
      ctx.emit(f64_const(min));
      ctx.emit(Instruction::F64Ge);
      ctx.emit(Instruction::LocalGet(value));
      ctx.emit(f64_const(max));
      ctx.emit(Instruction::F64Le);
      ctx.emit(Instruction::I32And);
      ctx.emit(Instruction::LocalGet(value));
      ctx.emit(Instruction::F64Trunc);
      ctx.emit(Instruction::LocalGet(value));
      ctx.emit(Instruction::F64Eq);
      ctx.emit(Instruction::I32And);
    }
  }
}

fn emit_unary(ctx: &mut WasmGenCtx, instr: Instruction<'static>, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    // keep explicit for clarity in math ops
    return Err(format!("{instr:?} expects 1 arg, got {}", args.len()));
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(instr);
  Ok(())
}

/// Emit `type-of v`. All heap objects carry a type tag at `raw_base` (ptr - 8),
/// so for values that look like heap pointers we read that tag; otherwise we
/// fall back to `:number`.
///
/// Pointer detection heuristic (enough for core-library usage patterns):
/// - value must be a finite integer (== trunc(v))
/// - value must be within `[HEAP_BASE + 8, current_heap_ptr)` (logical ptrs begin
///   8 bytes after the raw base)
/// - the i32 offset at `(ptr - 8)` must contain a registered type tag id
///
/// Values failing any check are reported as `:number`. This is a deliberate
/// simplification — distinguishing bool/nil/tag/number without NaN-boxing is
/// not supported and is unlikely to be needed by the subset of core functions
/// currently compiled.
fn emit_type_of(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err(format!("type-of expects 1 arg, got {}", args.len()));
  }
  let number_tag = get_type_tag(ctx, "number");
  let v_local = ctx.alloc_local_typed(ValType::F64);
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(v_local));

  let is_valid_ptr = ctx.alloc_local_typed(ValType::I32);
  let raw_base = ctx.alloc_local_typed(ValType::I32);

  // 1) integer-valued: v == trunc(v)
  ctx.emit(Instruction::LocalGet(v_local));
  ctx.emit(Instruction::LocalGet(v_local));
  ctx.emit(Instruction::F64Trunc);
  ctx.emit(Instruction::F64Eq);
  // 2) v >= (HEAP_BASE + 8) as f64  (lowest possible logical pointer)
  ctx.emit(Instruction::LocalGet(v_local));
  ctx.emit(f64_const((HEAP_BASE + 8) as f64));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::I32And);
  // 3) v < current heap_ptr
  ctx.emit(Instruction::LocalGet(v_local));
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(is_valid_ptr));

  // raw_base = trunc(v) - 8
  ctx.emit(Instruction::LocalGet(v_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(raw_base));

  // Short-circuit: only load memory when range is valid.
  ctx.emit(Instruction::LocalGet(is_valid_ptr));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  // Check magic at raw_base+0 == HEAP_MAGIC.
  ctx.emit(Instruction::LocalGet(raw_base));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(HEAP_MAGIC));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  // Load tag id (i32) at raw_base+4 and convert to f64.
  ctx.emit(Instruction::LocalGet(raw_base));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(number_tag));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(number_tag));
  ctx.emit(Instruction::End);
  Ok(())
}

/// Emit a type predicate like `list?`. Compares `type-of v` with the given
/// type tag and pushes 1.0 (true) or 0.0 (false).
fn emit_type_predicate(ctx: &mut WasmGenCtx, type_name: &str, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err(format!("{}? expects 1 arg, got {}", type_name, args.len()));
  }
  let expected_tag = get_type_tag(ctx, type_name);
  // Emit type-of, which leaves a tag f64 on the stack
  emit_type_of(ctx, args)?;
  // Compare with expected tag
  ctx.emit(f64_const(expected_tag));
  ctx.emit(Instruction::F64Eq);
  // Convert i32 boolean to f64
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Emit a call to a host-imported function by name.
fn emit_host_call(ctx: &mut WasmGenCtx, name: &str, args: &[Calcit]) -> Result<(), String> {
  let import = core_host_import(name).ok_or_else(|| format!("unknown host import: {name}"))?;
  let expected_arity = import.params.len();
  if args.len() != expected_arity {
    return Err(format!("{name} expects {expected_arity} args, got {}", args.len()));
  }
  for arg in args {
    emit_expr(ctx, arg)?;
  }
  ctx.emit(Instruction::Call(resolve_host_import(ctx, &import.module, &import.name)?));
  Ok(())
}

fn emit_wasi_write_string(ctx: &mut WasmGenCtx, fd: i32, ptr: u32) {
  let len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(len));
  ctx.emit(Instruction::I32Const(fd));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(len));
  ctx.call_rt("__rt_wasi_write_all");
}

fn emit_wasi_write_literal(ctx: &mut WasmGenCtx, fd: i32, text: &str) -> Result<(), String> {
  let ptr = *ctx
    .string_pool
    .get(text)
    .ok_or_else(|| format!("internal WASI output literal missing from string pool: {text:?}"))?;
  let ptr_local = ctx.alloc_i32(ptr as i32);
  emit_wasi_write_string(ctx, fd, ptr_local);
  Ok(())
}

/// Read a Preview 1 clock into the runtime scratch area and return milliseconds.
fn emit_wasi_clock_ms(ctx: &mut WasmGenCtx, clock_id: i32, proc_name: &str) -> Result<(), String> {
  if ctx.boundary == WasmBoundary::Component {
    return Err(format!("E_WASI_COMMAND_CAPABILITY: `{proc_name}` needs WASI 0.3 clocks lowering"));
  }
  if ctx.target != WasmTarget::Wasi {
    return Err(format!("E_WASM_CAPABILITY: {proc_name} is unavailable for the core WASM target"));
  }
  ctx.emit(Instruction::I32Const(clock_id));
  ctx.emit(Instruction::I64Const(1_000_000));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::Call(resolve_host_import(
    ctx,
    "wasi_snapshot_preview1",
    "clock_time_get",
  )?));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::F64ConvertI64U);
  ctx.emit(f64_const(1_000_000.0));
  ctx.emit(Instruction::F64Div);
  Ok(())
}

/// Lower the typed synchronous wait boundary through Preview 1 `poll_oneoff`.
fn emit_wasi_wait_ms(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if ctx.boundary == WasmBoundary::Component {
    return Err("E_WASI_COMMAND_CAPABILITY: `&wait-ms` needs WASI 0.3 clocks lowering".into());
  }
  expect_arity(3, args, "&wait-ms")?;
  if ctx.target != WasmTarget::Wasi {
    return Err("E_WASM_CAPABILITY: wait-ms is unavailable for the core WASM target".into());
  }

  let milliseconds = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(milliseconds));
  let host_error = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(host_error));

  ctx.emit(Instruction::LocalGet(milliseconds));
  ctx.emit(Instruction::LocalGet(milliseconds));
  ctx.emit(Instruction::F64Trunc);
  ctx.emit(Instruction::F64Ne);
  ctx.emit(Instruction::LocalGet(milliseconds));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::LocalGet(milliseconds));
  ctx.emit(f64_const(u32::MAX as f64));
  ctx.emit(Instruction::F64Gt);
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_result_enum(ctx, "err", host_error)?;
  ctx.emit(Instruction::Else);

  let unit = ctx.alloc_local();
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(unit));
  ctx.emit(Instruction::LocalGet(milliseconds));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_result_enum(ctx, "ok", unit)?;
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(milliseconds));
  ctx.emit(f64_const(1_000_000.0));
  ctx.emit(Instruction::F64Mul);
  ctx.emit(Instruction::I64TruncF64U);
  ctx.call_rt("__rt_wasi_wait");
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_result_enum(ctx, "ok", unit)?;
  ctx.emit(Instruction::Else);
  emit_result_enum(ctx, "err", host_error)?;
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

/// Fill a managed Buffer through Preview 1 and wrap it in the caller's Result type.
fn emit_wasi_secure_random_bytes(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if ctx.boundary == WasmBoundary::Component {
    return Err("E_WASI_COMMAND_CAPABILITY: `&secure-random-bytes` needs WASI 0.3 random lowering".into());
  }
  expect_arity(3, args, "&secure-random-bytes")?;
  if ctx.target != WasmTarget::Wasi {
    return Err("E_WASM_CAPABILITY: secure-random-bytes is unavailable for the core WASM target".into());
  }

  let size = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(size));
  let host_error = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(host_error));

  ctx.emit(Instruction::LocalGet(size));
  ctx.emit(Instruction::LocalGet(size));
  ctx.emit(Instruction::F64Trunc);
  ctx.emit(Instruction::F64Ne);
  ctx.emit(Instruction::LocalGet(size));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::LocalGet(size));
  ctx.emit(f64_const(65_536.0));
  ctx.emit(Instruction::F64Gt);
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_result_enum(ctx, "err", host_error)?;
  ctx.emit(Instruction::Else);

  let size_i32 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(size));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(size_i32));
  let allocation_size = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(size_i32));
  ctx.emit(Instruction::I32Const(7));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(-8));
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(allocation_size));
  let buffer_ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc_dynamic(ctx, allocation_size, buffer_ptr, "buffer");
  ctx.emit(Instruction::LocalGet(buffer_ptr));
  ctx.emit(Instruction::LocalGet(size));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(buffer_ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(size_i32));
  ctx.emit(Instruction::Call(resolve_host_import(ctx, "wasi_snapshot_preview1", "random_get")?));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_result_enum(ctx, "err", host_error)?;
  ctx.emit(Instruction::Else);
  let buffer = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(buffer_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(buffer));
  emit_result_enum(ctx, "ok", buffer)?;
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

/// Lower the typed filesystem read boundary through private Preview 1 helpers.
fn emit_wasi_fs_read_text(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if ctx.boundary == WasmBoundary::Component {
    return Err("E_WASI_COMMAND_CAPABILITY: `&fs-read-text` needs WASI 0.3 filesystem lowering".into());
  }
  expect_arity(3, args, "&fs-read-text")?;
  if ctx.target != WasmTarget::Wasi {
    return Err("E_WASM_CAPABILITY: filesystem reads are unavailable for the core WASM target".into());
  }
  let path = emit_ptr_to_i32(ctx, &args[1])?;
  let host_error = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(host_error));
  ctx.emit(Instruction::LocalGet(path));
  ctx.call_rt("__rt_wasi_read_text");
  let content_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalTee(content_ptr));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_result_enum(ctx, "err", host_error)?;
  ctx.emit(Instruction::Else);
  let content = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(content_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(content));
  emit_result_enum(ctx, "ok", content)?;
  ctx.emit(Instruction::End);
  Ok(())
}

/// Lower the typed filesystem directory boundary through a private Preview 1 helper.
fn emit_wasi_fs_read_dir(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if ctx.boundary == WasmBoundary::Component {
    return Err("E_WASI_COMMAND_CAPABILITY: `&fs-read-dir` needs WASI 0.3 filesystem lowering".into());
  }
  expect_arity(4, args, "&fs-read-dir")?;
  if ctx.target != WasmTarget::Wasi {
    return Err("E_WASM_CAPABILITY: filesystem directory reads are unavailable for the core WASM target".into());
  }
  let path = emit_ptr_to_i32(ctx, &args[2])?;
  let host_error = ctx.alloc_local();
  emit_expr(ctx, &args[3])?;
  ctx.emit(Instruction::LocalSet(host_error));
  ctx.emit(Instruction::LocalGet(path));
  ctx.call_rt("__rt_wasi_read_dir");
  let list = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalTee(list));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_result_enum(ctx, "err", host_error)?;
  ctx.emit(Instruction::Else);

  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(list));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(count));
  let index = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(index));
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  let value = ctx.alloc_local();
  emit_list_load_elem(ctx, list, index);
  ctx.emit(Instruction::LocalSet(value));
  let struct_ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, 24, struct_ptr, "struct");
  ctx.emit(Instruction::LocalGet(struct_ptr));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(struct_ptr));
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalGet(struct_ptr));
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::F64Store(mem_arg_f64(16)));
  ctx.emit(Instruction::LocalGet(list));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(struct_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(index));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  let list_value = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(list));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(list_value));
  emit_result_enum(ctx, "ok", list_value)?;
  ctx.emit(Instruction::End);
  Ok(())
}

/// Lower the typed filesystem write boundary through private Preview 1 helpers.
fn emit_wasi_fs_write_text(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if ctx.boundary == WasmBoundary::Component {
    return Err("E_WASI_COMMAND_CAPABILITY: `&fs-write-text` needs WASI 0.3 filesystem lowering".into());
  }
  expect_arity(4, args, "&fs-write-text")?;
  if ctx.target != WasmTarget::Wasi {
    return Err("E_WASM_CAPABILITY: filesystem writes are unavailable for the core WASM target".into());
  }
  let path = emit_ptr_to_i32(ctx, &args[1])?;
  let content = emit_ptr_to_i32(ctx, &args[2])?;
  let host_error = ctx.alloc_local();
  emit_expr(ctx, &args[3])?;
  ctx.emit(Instruction::LocalSet(host_error));
  ctx.emit(Instruction::LocalGet(path));
  ctx.emit(Instruction::LocalGet(content));
  ctx.call_rt("__rt_wasi_write_text");
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  let unit = ctx.alloc_local();
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(unit));
  emit_result_enum(ctx, "ok", unit)?;
  ctx.emit(Instruction::Else);
  emit_result_enum(ctx, "err", host_error)?;
  ctx.emit(Instruction::End);
  Ok(())
}

/// Lower Calcit console output through a private Preview 1 adapter.
fn emit_wasi_print(ctx: &mut WasmGenCtx, name: &str, args: &[Calcit]) -> Result<(), String> {
  let fd = if name == "eprintln" { 2 } else { 1 };
  for (index, arg) in args.iter().enumerate() {
    if index > 0 {
      emit_wasi_write_literal(ctx, fd, " ")?;
    }
    emit_expr(ctx, arg)?;
    let value = ctx.alloc_local();
    ctx.emit(Instruction::LocalSet(value));
    let ptr = emit_turn_string_from_local(ctx, value);
    emit_wasi_write_string(ctx, fd, ptr);
  }
  emit_wasi_write_literal(ctx, fd, "\n")?;
  ctx.emit(f64_const(0.0));
  Ok(())
}

fn emit_wasi_component_print(ctx: &mut WasmGenCtx, name: &str, args: &[Calcit]) -> Result<(), String> {
  let interface = if name == "eprintln" { "stderr" } else { "stdout" };
  for (index, arg) in args.iter().enumerate() {
    if index > 0 {
      emit_wasi_component_write_literal(ctx, interface, " ")?;
    }
    emit_expr(ctx, arg)?;
    let value = ctx.alloc_local();
    ctx.emit(Instruction::LocalSet(value));
    let ptr = emit_turn_string_from_local(ctx, value);
    emit_wasi_component_write_string(ctx, interface, ptr)?;
  }
  emit_wasi_component_write_literal(ctx, interface, "\n")?;
  ctx.emit(f64_const(0.0));
  Ok(())
}

fn emit_wasi_component_write_literal(ctx: &mut WasmGenCtx, interface: &str, text: &str) -> Result<(), String> {
  let ptr = *ctx
    .string_pool
    .get(text)
    .ok_or_else(|| format!("internal WASI output literal missing from string pool: {text:?}"))?;
  let ptr_local = ctx.alloc_i32(ptr as i32);
  emit_wasi_component_write_string(ctx, interface, ptr_local)
}

fn emit_wasi_component_write_string(ctx: &mut WasmGenCtx, interface: &str, ptr: u32) -> Result<(), String> {
  let module = format!("wasi:cli/{interface}@0.3.1");
  let new = resolve_host_import(ctx, &module, "[stream-new-0]write-via-stream")?;
  let write = resolve_host_import(ctx, &module, "[stream-write-0]write-via-stream")?;
  let drop_writer = resolve_host_import(ctx, &module, "[stream-drop-writable-0]write-via-stream")?;
  let drop_future = resolve_host_import(ctx, &module, "[future-drop-readable-1]write-via-stream")?;
  let output = resolve_host_import(ctx, &module, "write-via-stream")?;
  let pair = ctx.alloc_local_typed(ValType::I64);
  let writer = ctx.alloc_local_typed(ValType::I32);
  let future = ctx.alloc_local_typed(ValType::I32);
  let offset = ctx.alloc_local_typed(ValType::I32);
  let remaining = ctx.alloc_local_typed(ValType::I32);
  let count = ctx.alloc_local_typed(ValType::I32);
  let result = ctx.alloc_local_typed(ValType::I32);

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(remaining));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(offset));
  ctx.emit(Instruction::Call(new));
  ctx.emit(Instruction::LocalSet(pair));
  ctx.emit(Instruction::LocalGet(pair));
  ctx.emit(Instruction::I32WrapI64);
  ctx.emit(Instruction::Call(output));
  ctx.emit(Instruction::LocalSet(future));
  ctx.emit(Instruction::LocalGet(pair));
  ctx.emit(Instruction::I64Const(32));
  ctx.emit(Instruction::I64ShrU);
  ctx.emit(Instruction::I32WrapI64);
  ctx.emit(Instruction::LocalSet(writer));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(remaining));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(writer));
  ctx.emit(Instruction::LocalGet(offset));
  ctx.emit(Instruction::LocalGet(remaining));
  ctx.emit(Instruction::I32Const(1 << 28));
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  ctx.emit(Instruction::I32Const(1 << 28));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(remaining));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Call(write));
  ctx.emit(Instruction::LocalTee(result));
  ctx.emit(Instruction::I32Const(15));
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32ShrU);
  ctx.emit(Instruction::LocalTee(count));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(offset));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(offset));
  ctx.emit(Instruction::LocalGet(remaining));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(remaining));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(writer));
  ctx.emit(Instruction::Call(drop_writer));
  ctx.emit(Instruction::LocalGet(future));
  ctx.emit(Instruction::Call(drop_future));
  Ok(())
}

fn resolve_host_import(ctx: &WasmGenCtx, module: &str, name: &str) -> Result<u32, String> {
  ctx
    .host_imports
    .get(&(module.to_owned(), name.to_owned()))
    .copied()
    .ok_or_else(|| format!("E_WASM_CAPABILITY: host capability `{module}/{name}` is unavailable for this WASM target"))
}

fn index_host_imports(imports: &[HostImport]) -> HashMap<(String, String), u32> {
  let mut indices = HashMap::new();
  for (index, import) in imports.iter().enumerate() {
    indices.entry((import.module.clone(), import.name.clone())).or_insert(index as u32);
  }
  indices
}

fn emit_binary(ctx: &mut WasmGenCtx, instr: Instruction<'static>, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err(format!("{instr:?} expects 2 args, got {}", args.len()));
  }
  emit_expr(ctx, &args[0])?;
  emit_expr(ctx, &args[1])?;
  ctx.emit(instr);
  Ok(())
}

fn emit_cmp(ctx: &mut WasmGenCtx, instr: Instruction<'static>, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err(format!("{instr:?} expects 2 args, got {}", args.len()));
  }
  // select (f64.const 1) (f64.const 0) (cmp a b)
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  emit_expr(ctx, &args[0])?;
  emit_expr(ctx, &args[1])?;
  ctx.emit(instr);
  ctx.emit(Instruction::Select);
  Ok(())
}

/// Emit `=` with string content equality support.
///
/// Semantics:
/// - Fast path: `a == b` on raw f64 representation.
/// - If not equal and both look like heap strings, compare by `__rt_str_compare`.
/// - Otherwise false.
fn emit_equals(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err(format!("= expects 2 args, got {}", args.len()));
  }

  let left_buffer = buffer_literal_bytes(&args[0])?;
  let right_buffer = buffer_literal_bytes(&args[1])?;
  match (left_buffer, right_buffer) {
    (Some(left), Some(right)) => {
      ctx.emit(f64_const(if left == right { 1.0 } else { 0.0 }));
      return Ok(());
    }
    (Some(bytes), None) => return emit_buffer_literal_equals(ctx, &args[1], &bytes),
    (None, Some(bytes)) => return emit_buffer_literal_equals(ctx, &args[0], &bytes),
    (None, None) => {}
  }

  let a = ctx.alloc_local();
  let b = ctx.alloc_local();

  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(a));
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(b));

  let result = emit_equals_core(ctx, a, b)?;
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

fn buffer_literal_bytes(value: &Calcit) -> Result<Option<Vec<u8>>, String> {
  let Calcit::List(items) = value else {
    return Ok(None);
  };
  let Some(head) = items.first() else {
    return Ok(None);
  };
  let is_buffer = matches!(head, Calcit::Proc(CalcitProc::NativeBuffer))
    || matches!(head, Calcit::Import(import) if import.def.as_ref() == "&buffer")
    || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "&buffer");
  if !is_buffer {
    return Ok(None);
  }
  if items.len() == 1 {
    return Err("&buffer expects at least one byte".into());
  }
  let mut bytes = Vec::with_capacity(items.len() - 1);
  for item in items.iter().skip(1) {
    match item {
      Calcit::Number(value) => bytes.push(value.round() as u8),
      Calcit::Str(value) if value.len() == 2 => {
        bytes.push(u8::from_str_radix(value, 16).map_err(|error| format!("invalid &buffer byte {value:?}: {error}"))?)
      }
      _ => {
        return Err(format!(
          "component WASM buffer literals require numeric bytes or two-digit hex strings, got {item}"
        ));
      }
    }
  }
  Ok(Some(bytes))
}

fn emit_buffer_literal_equals(ctx: &mut WasmGenCtx, value: &Calcit, bytes: &[u8]) -> Result<(), String> {
  emit_expr(ctx, value)?;
  let value = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(value));
  let ptr = ctx.alloc_local_typed(ValType::I32);
  let result = ctx.alloc_local();
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(f64_const((HEAP_BASE + 8) as f64));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::MemorySize(0));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Shl);
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::F64Trunc);
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(HEAP_MAGIC));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(
    *ctx.tag_index.get("buffer").ok_or("buffer tag not found")? as i32
  ));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(f64_const(bytes.len() as f64));
  ctx.emit(Instruction::F64Eq);
  ctx.begin_block_if();
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::LocalSet(result));
  for (index, byte) in bytes.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(Instruction::I32Load8U(mem_arg_byte(8 + index as u64)));
    ctx.emit(Instruction::I32Const(i32::from(*byte)));
    ctx.emit(Instruction::I32Ne);
    ctx.begin_block_if();
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::LocalSet(result));
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// Core structural equality check given two pre-evaluated f64 locals.
/// Returns the f64 result local (0.0 = not equal, 1.0 = equal).
/// Uses structural set comparison (inline loop with shallow element equality).
#[allow(private_interfaces)]
pub(super) fn emit_equals_core(ctx: &mut WasmGenCtx, a: u32, b: u32) -> Result<u32, String> {
  emit_equals_core_impl(ctx, a, b, true)
}

/// Shallow version of emit_equals_core: for set comparison, uses __rt_set_find_elem
/// (pointer equality only). Call this from within emit_set_find_structural to
/// avoid infinite mutual recursion.
#[allow(private_interfaces)]
pub(super) fn emit_equals_core_shallow(ctx: &mut WasmGenCtx, a: u32, b: u32) -> Result<u32, String> {
  emit_equals_core_impl(ctx, a, b, false)
}

/// Internal implementation. When `structural_sets` is true, the set comparison
/// uses an inline structural search (calling `emit_equals_core_impl` with
/// `structural_sets=false` for elements, preventing infinite recursion).
/// When false, uses __rt_set_find_elem (F64Eq for elements).
fn emit_equals_core_impl(ctx: &mut WasmGenCtx, a: u32, b: u32, structural_sets: bool) -> Result<u32, String> {
  let result = ctx.alloc_local(); // 0.0 = not equal, 1.0 = equal

  let string_tag = *ctx.tag_index.get("string").ok_or("string tag not found")? as i32;
  let list_tag = *ctx.tag_index.get("list").ok_or("list tag not found")? as i32;
  let set_tag = *ctx.tag_index.get("set").ok_or("set tag not found")? as i32;
  let map_tag = *ctx.tag_index.get("map").ok_or("map tag not found")? as i32;
  let enum_tag = *ctx.tag_index.get("enum").ok_or("enum tag not found")? as i32;
  let rt_str_compare = *ctx
    .runtime_fn_index
    .get("__rt_str_compare")
    .ok_or("runtime helper __rt_str_compare not found")?;
  let rt_set_find_elem = *ctx
    .runtime_fn_index
    .get("__rt_set_find_elem")
    .ok_or("runtime helper __rt_set_find_elem not found")?;
  let rt_map_equal = *ctx
    .runtime_fn_index
    .get("__rt_map_equal")
    .ok_or("runtime helper __rt_map_equal not found")?;
  let rt_value_equal = *ctx
    .runtime_fn_index
    .get("__rt_value_equal")
    .ok_or("runtime helper __rt_value_equal not found")?;

  // Default: not equal
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));

  // --- Fast path: exact f64 equality ---
  ctx.emit(Instruction::LocalGet(a));
  ctx.emit(Instruction::LocalGet(b));
  ctx.emit(Instruction::F64Eq);
  ctx.begin_block_if();
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::End);

  // Skip heap comparison if already equal
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.begin_block_if(); // outer: only proceed if result is still 0.0

  // --- Check both values are heap pointers ---
  // A valid heap ptr must be >= HEAP_BASE+8 AND < memory_size (in bytes).
  // We use memory.size * 65536 as the upper bound to prevent OOB on large f64
  // values (e.g., from &hash) that pass the lower-bound check but exceed memory.
  let ptr_a = ctx.alloc_local_typed(ValType::I32);
  let ptr_b = ctx.alloc_local_typed(ValType::I32);
  let both_heap = ctx.alloc_local_typed(ValType::I32);
  let mem_size_f64 = ctx.alloc_local();

  // mem_size_f64 = f64(memory.size * 65536)
  ctx.emit(Instruction::MemorySize(0));
  ctx.emit(Instruction::I32Const(16)); // 2^16 = 65536
  ctx.emit(Instruction::I32Shl);
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(mem_size_f64));

  ctx.emit(Instruction::LocalGet(a));
  ctx.emit(f64_const((HEAP_BASE + 8) as f64));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::LocalGet(a));
  ctx.emit(Instruction::LocalGet(mem_size_f64));
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalGet(b));
  ctx.emit(f64_const((HEAP_BASE + 8) as f64));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::LocalGet(b));
  ctx.emit(Instruction::LocalGet(mem_size_f64));
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(both_heap));

  ctx.emit(Instruction::LocalGet(both_heap));
  ctx.begin_block_if(); // only proceed if both are heap ptrs

  ctx.emit(Instruction::LocalGet(a));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr_a));
  ctx.emit(Instruction::LocalGet(b));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr_b));

  // Read type tags
  let tag_a = ctx.alloc_local_typed(ValType::I32);
  let tag_b = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalSet(tag_a));
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalSet(tag_b));

  // Only compare if same type
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::LocalGet(tag_b));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();

  // --- String comparison ---
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::I32Const(string_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::Call(rt_str_compare));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq); // str_compare returns f64; 0.0 = equal
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::End); // end string if

  // Skip if already resolved
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::I32Const(string_tag));
  ctx.emit(Instruction::I32Ne);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();

  // --- List comparison: same count + structural element equality ---
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::I32Const(list_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  {
    let cnt_a = ctx.alloc_local_typed(ValType::I32);
    let cnt_b = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(ptr_a));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(cnt_a));
    ctx.emit(Instruction::LocalGet(ptr_b));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(cnt_b));

    // If counts match, compare element-by-element
    ctx.emit(Instruction::LocalGet(cnt_a));
    ctx.emit(Instruction::LocalGet(cnt_b));
    ctx.emit(Instruction::I32Eq);
    ctx.begin_block_if();
    {
      let all_eq = ctx.alloc_i32(1); // assume equal
      let li = ctx.alloc_i32(0);
      ctx.begin_block();
      ctx.begin_loop();
      ctx.loop_exit_if_ge(li, cnt_a);
      // elem_a = ptr_a[(1+li)*8], elem_b = ptr_b[(1+li)*8]
      let offset_a = ctx.alloc_local_typed(ValType::I32);
      let offset_b = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalGet(ptr_a));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalGet(li));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Mul);
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalSet(offset_a));
      ctx.emit(Instruction::LocalGet(ptr_b));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalGet(li));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Mul);
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalSet(offset_b));
      let elems_eq = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalGet(offset_a));
      ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
      ctx.emit(Instruction::LocalGet(offset_b));
      ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
      ctx.emit(Instruction::Call(rt_value_equal));
      ctx.emit(Instruction::LocalSet(elems_eq));
      // if NOT elems_eq → all_eq = 0, break
      ctx.emit(Instruction::LocalGet(elems_eq));
      ctx.emit(Instruction::I32Eqz);
      ctx.begin_block_if();
      // not equal → mark all_eq = 0 and break
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(all_eq));
      ctx.emit(Instruction::Br(2)); // break out of block
      ctx.emit(Instruction::End);
      ctx.i32_inc(li);
      ctx.emit(Instruction::Br(0));
      ctx.emit(Instruction::End); // loop
      ctx.emit(Instruction::End); // block
      // result = all_eq
      ctx.emit(Instruction::LocalGet(all_eq));
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalSet(result));
    }
    ctx.emit(Instruction::End); // count eq if
  }
  ctx.emit(Instruction::End); // list if

  // --- Set comparison: same count + every elem of A is in B ---
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::I32Const(set_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  {
    let cnt_a = ctx.alloc_local_typed(ValType::I32);
    let cnt_b = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(ptr_a));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(cnt_a));
    ctx.emit(Instruction::LocalGet(ptr_b));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(cnt_b));

    ctx.emit(Instruction::LocalGet(cnt_a));
    ctx.emit(Instruction::LocalGet(cnt_b));
    ctx.emit(Instruction::I32Eq);
    ctx.begin_block_if();
    {
      let all_eq = ctx.alloc_i32(1);
      let si = ctx.alloc_i32(0);
      ctx.begin_block();
      ctx.begin_loop();
      ctx.loop_exit_if_ge(si, cnt_a);
      // load elem from set A
      let offset_a = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalGet(ptr_a));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalGet(si));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Mul);
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalSet(offset_a));
      ctx.emit(Instruction::LocalGet(offset_a));
      ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
      let elem_a = ctx.alloc_local();
      ctx.emit(Instruction::LocalSet(elem_a));
      // check elem_a in set B
      let elem_not_found = if structural_sets {
        // Inline structural search: iterate B and compare with shallow equality
        let bj = ctx.alloc_i32(0);
        let found_b = ctx.alloc_i32(-1);
        ctx.begin_block();
        ctx.begin_loop();
        ctx.loop_exit_if_ge(bj, cnt_b);
        let b_off = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(ptr_b));
        ctx.emit(Instruction::I32Const(8));
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalGet(bj));
        ctx.emit(Instruction::I32Const(8));
        ctx.emit(Instruction::I32Mul);
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalSet(b_off));
        let b_elem = ctx.alloc_local();
        ctx.emit(Instruction::LocalGet(b_off));
        ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
        ctx.emit(Instruction::LocalSet(b_elem));
        let eq_r = emit_equals_core_impl(ctx, elem_a, b_elem, false)?;
        ctx.emit(Instruction::LocalGet(eq_r));
        ctx.emit(f64_const(0.0));
        ctx.emit(Instruction::F64Ne);
        ctx.begin_block_if();
        ctx.emit(Instruction::LocalGet(bj));
        ctx.emit(Instruction::LocalSet(found_b));
        ctx.emit(Instruction::Br(2));
        ctx.emit(Instruction::End);
        ctx.i32_inc(bj);
        ctx.emit(Instruction::Br(0));
        ctx.emit(Instruction::End); // loop
        ctx.emit(Instruction::End); // block
        // found_b is -1 if not found, or index if found
        ctx.emit(Instruction::LocalGet(found_b));
        ctx.emit(Instruction::I32Const(-1));
        ctx.emit(Instruction::I32Eq); // 1 if not found, 0 if found
        let not_found_flag = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalSet(not_found_flag));
        not_found_flag
      } else {
        // Shallow: use __rt_set_find_elem (F64Eq for elements)
        ctx.emit(Instruction::LocalGet(ptr_b));
        ctx.emit(Instruction::LocalGet(elem_a));
        ctx.emit(Instruction::Call(rt_set_find_elem));
        ctx.emit(Instruction::I32Const(-1));
        ctx.emit(Instruction::I32Eq); // 1 if not found, 0 if found
        let not_found_flag = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalSet(not_found_flag));
        not_found_flag
      };
      ctx.emit(Instruction::LocalGet(elem_not_found));
      ctx.begin_block_if();
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(all_eq));
      ctx.emit(Instruction::Br(2)); // break
      ctx.emit(Instruction::End);
      ctx.i32_inc(si);
      ctx.emit(Instruction::Br(0));
      ctx.emit(Instruction::End); // loop
      ctx.emit(Instruction::End); // block
      ctx.emit(Instruction::LocalGet(all_eq));
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalSet(result));
    }
    ctx.emit(Instruction::End); // count eq if
  }
  ctx.emit(Instruction::End); // set if

  // --- Map comparison: use __rt_map_equal runtime helper ---
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::I32Const(map_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::Call(rt_map_equal));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::End); // end map if

  // --- Enum comparison: variant tag and recursive payloads ---
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::I32Const(enum_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(a));
  ctx.emit(Instruction::LocalGet(b));
  ctx.emit(Instruction::Call(rt_value_equal));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::End); // end enum if

  ctx.emit(Instruction::End); // "not yet resolved" if
  ctx.emit(Instruction::End); // "same type tag" if
  ctx.emit(Instruction::End); // "both heap" if
  ctx.emit(Instruction::End); // "result is still 0.0" outer if

  Ok(result)
}

/// Emit a binary bitwise operation: convert both args to i32, apply op, convert back to f64.
fn emit_bitwise_binary(ctx: &mut WasmGenCtx, instr: Instruction<'static>, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err(format!("{instr:?} expects 2 args, got {}", args.len()));
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64S);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64S);
  ctx.emit(instr);
  ctx.emit(Instruction::F64ConvertI32S);
  Ok(())
}

/// Emit WASM for `if` expression.
fn emit_if(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() < 2 || args.len() > 3 {
    return Err(format!("if expects 2-3 args, got {}", args.len()));
  }
  // condition → i32
  emit_expr(ctx, &args[0])?;
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ne); // nonzero is truthy → i32

  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.block_depth += 1;
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::Else);
  if args.len() == 3 {
    emit_expr(ctx, &args[2])?;
  } else {
    ctx.emit(f64_const(0.0));
  }
  ctx.block_depth -= 1;
  ctx.emit(Instruction::End);
  Ok(())
}

/// Handle `calcit.core/let` in call position: `(let ((name val)...) body...)`.
/// First arg is a list of binding pairs; remaining args are the body expressions.
fn emit_let_multi(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    ctx.emit(f64_const(0.0));
    return Ok(());
  }
  let Calcit::List(pairs_list) = &args[0] else {
    return Err(format!("let expects a list of binding pairs, got: {}", args[0]));
  };
  let body = &args[1..];
  emit_let_pairs(ctx, &pairs_list.to_vec(), body)
}

fn emit_let_pairs(ctx: &mut WasmGenCtx, pairs: &[Calcit], body: &[Calcit]) -> Result<(), String> {
  if pairs.is_empty() {
    return emit_body(ctx, body);
  }
  let pair = &pairs[0];
  let Calcit::List(xs) = pair else {
    return Err(format!("let binding expects a pair, got: {pair}"));
  };
  if xs.len() != 2 {
    return Err(format!("let binding pair must have 2 elements, got {}", xs.len()));
  }
  let var_name = match &xs[0] {
    Calcit::Local(CalcitLocal { sym, .. }) => sym.to_string(),
    Calcit::Symbol { sym, .. } => sym.to_string(),
    other => return Err(format!("let binding expected symbol, got: {other}")),
  };
  if let Some(closure) = resolve_inline_closure(ctx, &xs[1]) {
    ctx.lambda_locals.insert(var_name.clone(), closure);
    ctx.declare_local(&var_name);
  } else {
    emit_expr(ctx, &xs[1])?;
    ctx.lambda_locals.remove(&var_name);
    let idx = ctx.declare_local(&var_name);
    ctx.emit(Instruction::LocalSet(idx));
  }
  emit_let_pairs(ctx, &pairs[1..], body)
}

/// Emit WASM for `let` expression.
fn emit_let(ctx: &mut WasmGenCtx, body: &[Calcit]) -> Result<(), String> {
  if body.is_empty() {
    ctx.emit(f64_const(0.0));
    return Ok(());
  }

  let pair = &body[0];
  let rest = &body[1..];

  match pair {
    Calcit::Nil | Calcit::Unit => emit_body(ctx, rest),
    Calcit::List(xs) if xs.is_empty() => emit_body(ctx, rest),
    Calcit::List(xs) if xs.len() == 2 => {
      let var_name = match &xs[0] {
        Calcit::Local(CalcitLocal { sym, .. }) => sym.to_string(),
        Calcit::Symbol { sym, .. } => sym.to_string(),
        other => return Err(format!("let binding expected symbol, got: {other}")),
      };

      // Check if the binding value is an inline lambda.
      // If so, store it for inlining at call sites instead of emitting as runtime value.
      if let Some(closure) = resolve_inline_closure(ctx, &xs[1]) {
        ctx.lambda_locals.insert(var_name.clone(), closure);
        // Allocate a local slot (unused at runtime) so shadowing cleanup works.
        ctx.declare_local(&var_name);
        // Flatten nested lets
        if rest.len() == 1
          && let Calcit::List(inner) = &rest[0]
          && let Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) = inner.first()
        {
          let inner_body: Vec<Calcit> = inner.drop_left().to_vec();
          return emit_let(ctx, &inner_body);
        }
        return emit_body(ctx, rest);
      }

      emit_expr(ctx, &xs[1])?;
      ctx.lambda_locals.remove(&var_name);
      let idx = ctx.declare_local(&var_name);
      ctx.emit(Instruction::LocalSet(idx));

      // Flatten nested lets
      if rest.len() == 1
        && let Calcit::List(inner) = &rest[0]
        && let Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) = inner.first()
      {
        let inner_body: Vec<Calcit> = inner.drop_left().to_vec();
        return emit_let(ctx, &inner_body);
      }

      emit_body(ctx, rest)
    }
    _ => Err(format!("unsupported let binding form: {pair}")),
  }
}

/// Emit WASM for `match` expression (pattern matching on enums).
///
/// Preprocessed form: [value_expr, (pattern body), (pattern body), ...]
/// Each pattern is either `_` (wildcard) or `(:tag binding1 binding2 ...)`.
/// The value must be an enum — we read its tag_id at offset 8 and compare.
///
/// Compilation strategy: nested if/else chain comparing the tag_id.
///   evaluate value → store pointer in temp local
///   load tag_id from pointer
///   if tag == :variant1_id then { bind payloads; body1 }
///   else if tag == :variant2_id then { bind payloads; body2 }
///   else { wildcard_body or 0.0 }
fn emit_match(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    return Err("match requires a value and branches".into());
  }

  // Evaluate the value expression (an enum) and store its f64 pointer
  emit_expr(ctx, &args[0])?;
  let ptr_f64 = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(ptr_f64));

  // Convert to i32 for memory access and load the tag_id (f64 at offset 8, after count)
  let tag_local = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(ptr_f64));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(tag_local));

  // Collect branches: separate tag branches and wildcard
  let branches: Vec<&Calcit> = if args.len() == 3
    && matches!(&args[1], Calcit::EnumDef(_))
    && let Calcit::List(table) = &args[2]
  {
    table.iter().filter(|branch| !matches!(branch, Calcit::Nil)).collect()
  } else {
    args[1..].iter().collect()
  };
  let mut tag_branches: Vec<(&Calcit, &Calcit)> = Vec::new(); // (pattern, body)
  let mut wildcard_body: Option<&Calcit> = None;

  for branch in branches {
    let Calcit::List(pair) = branch else {
      return Err(format!("match branch expected a pair, got: {branch}"));
    };
    if pair.len() != 2 {
      return Err(format!("match branch expected 2 elements, got {}", pair.len()));
    }
    let pattern = &pair[0];
    let body = &pair[1];

    match pattern {
      // Wildcard
      Calcit::Symbol { sym, .. } | Calcit::Local(CalcitLocal { sym, .. }) if sym.as_ref() == "_" => {
        wildcard_body = Some(body);
      }
      // Tag pattern: (:tag binding1 binding2 ...)
      Calcit::List(_) => {
        tag_branches.push((pattern, body));
      }
      other => return Err(format!("unsupported match pattern: {other}")),
    }
  }

  // Generate nested if/else chain
  let num_tag_branches = tag_branches.len();
  if num_tag_branches == 0 {
    // Only wildcard
    if let Some(body) = wildcard_body {
      emit_expr(ctx, body)?;
    } else {
      ctx.emit(f64_const(0.0));
    }
    return Ok(());
  }

  // For each tag branch we emit:
  //   if (tag_local == variant_tag_id) then { bind payloads; body }
  //   else { next branch or wildcard }
  for (i, (pattern, body)) in tag_branches.iter().enumerate() {
    let Calcit::List(pat_xs) = pattern else {
      return Err(format!("match pattern expected list, got: {pattern}"));
    };
    let tag_str = match &pat_xs[0] {
      Calcit::Tag(t) => t.to_string(),
      other => return Err(format!("match pattern expected tag, got: {other}")),
    };
    let tag_id = *ctx
      .tag_index
      .get(&tag_str)
      .ok_or_else(|| format!("unknown tag in match pattern: {tag_str}"))?;

    // Compare: tag_local == tag_id
    ctx.emit(Instruction::LocalGet(tag_local));
    ctx.emit(f64_const(tag_id as f64));
    ctx.emit(Instruction::F64Eq);

    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
    ctx.block_depth += 1;

    // Bind payload variables from enum fields
    let binding_count = pat_xs.len() - 1;
    for bind_idx in 0..binding_count {
      let binding = &pat_xs[1 + bind_idx];
      let bind_name = match binding {
        Calcit::Local(CalcitLocal { sym, .. }) => sym.to_string(),
        Calcit::Symbol { sym, .. } => sym.to_string(),
        other => return Err(format!("match binding expected symbol, got: {other}")),
      };
      // Payload at offset (2 + bind_idx) * 8 from enum pointer (skip count + tag)
      let offset = ((2 + bind_idx) * 8) as u64;
      ctx.emit(Instruction::LocalGet(ptr_f64));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::F64Load(mem_arg_f64(offset)));
      let idx = ctx.declare_local(&bind_name);
      ctx.emit(Instruction::LocalSet(idx));
    }

    // Emit body
    emit_expr(ctx, body)?;

    ctx.emit(Instruction::Else);
    // If this is the last tag branch, emit wildcard or default
    if i == num_tag_branches - 1 {
      if let Some(wb) = wildcard_body {
        emit_expr(ctx, wb)?;
      } else {
        ctx.emit(f64_const(0.0));
      }
    }
    // Otherwise the next iteration will emit the next if/else inside this else block
  }

  // Close all the if/else blocks (one End per branch)
  for _ in 0..num_tag_branches {
    ctx.block_depth -= 1;
    ctx.emit(Instruction::End);
  }

  Ok(())
}

// ---------------------------------------------------------------------------
// Tag collection
// ---------------------------------------------------------------------------

/// Collect all Tag values from function bodies (multi-namespace format) and build tag→id map.
/// Tag IDs start at 1 (0 is unused/reserved).
/// Builtin type tags always registered in tag_index, so `type-of` can return them
/// and heap objects can carry them in their header slot.
const BUILTIN_TYPE_TAGS: &[&str] = &[
  "buf-list",
  "list",
  "map",
  "set",
  "enum",
  "struct",
  "number",
  "bool",
  "nil",
  "tag",
  "fn",
  "string",
  "symbol",
  "buffer",
  "none",
  "some",
  "ok",
  "err",
  "total-limit",
];

fn collect_all_tags_from(
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  program_data: Option<&program::CompiledProgram>,
) -> HashMap<String, u32> {
  let mut tags: Vec<String> = Vec::new();
  // Always include builtin type tags — used by `type-of` and heap headers.
  for t in BUILTIN_TYPE_TAGS {
    tags.push((*t).to_string());
  }
  for (_, _, _, body) in fn_defs {
    for expr in body {
      collect_tags_from_expr(expr, &mut tags);
    }
  }
  if let Some(program_data) = program_data {
    for file in program_data.values() {
      for compiled in file.defs.values() {
        for code in compiled
          .source_code
          .iter()
          .chain([&compiled.preprocessed_code, &compiled.codegen_form])
        {
          let struct_def = match code {
            Calcit::StructDef(struct_def) => Some(struct_def.clone()),
            _ => try_parse_defrecord_form(code).or_else(|| match crate::calcit::type_annotation::resolve_type_def_from_code(code) {
              Some(Calcit::StructDef(struct_def)) => Some(struct_def),
              _ => None,
            }),
          };
          if let Some(struct_def) = struct_def {
            tags.push(struct_def.name.ref_str().to_owned());
            tags.extend(struct_def.fields.iter().map(|field| field.ref_str().to_owned()));
          }
          let enum_def = match code {
            Calcit::EnumDef(enum_def) => Some(enum_def.clone()),
            _ => match crate::calcit::type_annotation::resolve_type_def_from_code(code) {
              Some(Calcit::EnumDef(enum_def)) => Some(enum_def),
              _ => None,
            },
          };
          if let Some(enum_def) = enum_def {
            tags.extend(enum_def.variants().iter().map(|variant| variant.tag.ref_str().to_owned()));
          }
        }
      }
    }
  }
  tags.sort();
  tags.dedup();
  tags.into_iter().enumerate().map(|(i, t)| (t, (i + 1) as u32)).collect()
}

fn collect_tags_from_expr(expr: &Calcit, tags: &mut Vec<String>) {
  if let Some(graph) = DataShapeGraph::from_calcit_handle(expr) {
    for node in &graph.nodes {
      match node {
        DataShapeNode::Struct { nominal, fields, .. } => {
          tags.push(nominal.name.ref_str().to_owned());
          tags.extend(fields.iter().map(|(field, _)| field.ref_str().to_owned()));
        }
        DataShapeNode::Enum { variants, .. } => {
          tags.extend(variants.iter().map(|(variant, _)| variant.ref_str().to_owned()));
        }
        _ => {}
      }
    }
  }
  match expr {
    Calcit::Tag(t) => {
      tags.push(t.to_string());
    }
    Calcit::List(xs) => {
      for x in xs.iter() {
        collect_tags_from_expr(x, tags);
      }
    }
    Calcit::StructDef(s) => {
      tags.push(s.name.to_string());
      for f in s.fields.iter() {
        tags.push(f.to_string());
      }
    }
    // When struct refs are imports, resolve them to collect their tags
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      if let Ok(struct_def) = resolve_struct_ref(expr) {
        tags.push(struct_def.name.to_string());
        for f in struct_def.fields.iter() {
          tags.push(f.to_string());
        }
      }
      // Also try to collect tags from the expression in case it's used as a value
      let _ = (ns, def); // suppress unused warnings
    }
    _ => {}
  }
}

// ---------------------------------------------------------------------------
// Struct operations
// ---------------------------------------------------------------------------

/// Build a string literal pool for WASM linear memory.
///
/// Scans all function bodies for `Calcit::Str` literals, deduplicates them,
/// and lays them out in memory starting at `HEAP_BASE`.
///
/// Each string is stored as:
///   `[magic:i32][type_tag("string"):i32][byte_len:f64][utf8_bytes... padded to 8]`
///
/// Returns:
///   - `string_pool`: maps string content → logical pointer (offset of byte_len field)
///   - `data_segment`: raw bytes for the WASM data section
///   - `heap_start`: the new heap start offset (after all string data)
fn build_string_pool(
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  value_imports: &HashMap<String, Calcit>,
  tag_index: &HashMap<String, u32>,
  target: WasmTarget,
) -> (HashMap<String, u32>, Vec<u8>, i32) {
  let mut strings: Vec<String> = Vec::new();
  let mut needs_edn_format = false;
  let mut needs_edn_parse = false;
  for (_, _, _, body) in fn_defs {
    for expr in body {
      collect_strings_from_expr(expr, &mut strings);
      needs_edn_format |= expr_uses_cirru_edn_format(expr);
      needs_edn_parse |= expr_uses_typed_cirru_edn_parse(expr);
    }
  }
  for expr in value_imports.values() {
    collect_strings_from_expr(expr, &mut strings);
    needs_edn_format |= expr_uses_cirru_edn_format(expr);
    needs_edn_parse |= expr_uses_typed_cirru_edn_parse(expr);
  }
  if target == WasmTarget::Wasi {
    strings.push(" ".into());
    strings.push("\n".into());
  }
  if needs_edn_format {
    strings.extend(
      [
        "do ", "\n", "nil", "true", "false", "[]", "([]", "{}", "({}", "%{}", "(%{}", "%::", "(%::", ")", " ", " (",
      ]
      .into_iter()
      .map(String::from),
    );
    strings.extend(tag_index.keys().map(|tag| format!(":{tag}")));
    strings.extend(tag_index.keys().map(|tag| format!("'{tag}")));
  }
  if needs_edn_parse {
    strings.extend(
      [
        "do ",
        "nil",
        "true",
        "false",
        "E_WASM_EDN_INPUT_LIMIT: Cirru EDN input exceeds 65536 bytes",
        "E_WASM_EDN_SYNTAX: invalid Cirru EDN scalar",
        "E_WASM_EDN_RANGE: numeric value is outside the requested type",
        "E_WASM_EDN_TAG: tag is not present in the compiled program",
        "E_WASM_EDN_TOKEN_LIMIT: Cirru EDN list exceeds 4096 items",
        "E_WASM_EDN_MAP_LIMIT: Cirru EDN map exceeds 2048 entries",
        "E_WASM_EDN_ENUM: Cirru EDN enum type, variant, or payload does not match the requested type",
      ]
      .into_iter()
      .map(String::from),
    );
    strings.extend(tag_index.keys().map(|tag| format!(":{tag}")));
    strings.extend(tag_index.keys().map(|tag| format!("'{tag}")));
  }
  strings.sort();
  strings.dedup();

  if strings.is_empty() {
    return (HashMap::new(), Vec::new(), HEAP_BASE);
  }

  let string_tag_id = *tag_index.get("string").expect("string type tag must exist") as i32;
  let mut pool: HashMap<String, u32> = HashMap::new();
  let mut data: Vec<u8> = Vec::new();
  let mut offset = HEAP_BASE as u32; // current write position in linear memory

  for s in &strings {
    let byte_len = s.len() as u32;
    // Write header: magic (i32) + type_tag (i32)
    data.extend_from_slice(&(HEAP_MAGIC as u32).to_le_bytes());
    data.extend_from_slice(&(string_tag_id as u32).to_le_bytes());
    // Logical pointer = offset + 8 (after header)
    let logical_ptr = offset + 8;
    pool.insert(s.clone(), logical_ptr);
    // Write byte_len as f64
    data.extend_from_slice(&(byte_len as f64).to_le_bytes());
    // Write UTF-8 bytes
    data.extend_from_slice(s.as_bytes());
    // Pad to 8-byte alignment
    let padded_len = (byte_len + 7) & !7;
    data.extend(std::iter::repeat_n(0u8, (padded_len - byte_len) as usize));
    // Advance offset: 8 (header) + 8 (byte_len f64) + padded_len
    offset += 8 + 8 + padded_len;
  }

  let heap_start = offset as i32;
  (pool, data, heap_start)
}

fn collect_struct_field_tags_from_program(
  program_data: &program::CompiledProgram,
  tag_index: &HashMap<String, u32>,
) -> HashMap<u32, Vec<u32>> {
  let mut result = HashMap::new();

  for file_info in program_data.values() {
    for compiled in file_info.defs.values() {
      let struct_def = compiled
        .source_code
        .iter()
        .chain([&compiled.preprocessed_code, &compiled.codegen_form])
        .find_map(|code| {
          try_parse_defrecord_form(code).or_else(|| match crate::calcit::type_annotation::resolve_type_def_from_code(code) {
            Some(Calcit::StructDef(struct_def)) => Some(struct_def),
            _ => None,
          })
        });
      let Some(struct_def) = struct_def else {
        continue;
      };

      let Some(struct_tag_id) = tag_index.get(struct_def.name.ref_str()) else {
        continue;
      };

      let field_tag_ids = struct_def
        .fields
        .iter()
        .filter_map(|field| tag_index.get(field.ref_str()).copied())
        .collect::<Vec<_>>();
      result.insert(*struct_tag_id, field_tag_ids);
    }
  }

  result
}

/// If `expr` is a literal enum constructor `(NativeEnum :tag val0 val1...)` with only
/// literal args (Tag, Str, Number, Bool, Nil, Unit), return its lispy string representation.
/// Used both to pre-intern the string and to emit it as a constant in `emit_turn_string`.
pub(crate) fn try_format_enum_literal(expr: &Calcit) -> Option<String> {
  if let Calcit::List(list) = expr
    && !list.is_empty()
    && let Calcit::Proc(p) = &list[0]
    && *p == CalcitProc::NativeEnum
  {
    let mut s = String::from("(:: ");
    for (i, item) in list.iter().skip(1).enumerate() {
      if i > 0 {
        s.push(' ');
      }
      match item {
        Calcit::Tag(_) | Calcit::Str(_) | Calcit::Number(_) | Calcit::Bool(_) | Calcit::Nil | Calcit::Unit => {
          use std::fmt::Write;
          write!(s, "{item}").ok()?;
        }
        _ => return None,
      }
    }
    s.push(')');
    return Some(s);
  }
  None
}

fn collect_strings_from_data_shape(graph: &DataShapeGraph, strings: &mut Vec<String>) {
  for node in &graph.nodes {
    match node {
      DataShapeNode::Struct { nominal, fields, .. } => {
        strings.push(format!("'{}", nominal.name));
        strings.push(format!(":{}", nominal.name));
        for (field, _) in fields {
          strings.push(format!(":{field}"));
        }
      }
      DataShapeNode::Enum { nominal, variants, .. } => {
        strings.push(format!("'{}", nominal.name()));
        strings.push(format!(":{}", nominal.name()));
        for (variant, _) in variants {
          strings.push(format!("'{variant}"));
          strings.push(format!(":{variant}"));
        }
      }
      _ => {}
    }
  }
}

fn collect_strings_from_expr(expr: &Calcit, strings: &mut Vec<String>) {
  if let Some(graph) = DataShapeGraph::from_calcit_handle(expr) {
    collect_strings_from_data_shape(&graph, strings);
  }
  match expr {
    Calcit::Str(s) => {
      strings.push(s.to_string());
    }
    // Intern tag names as strings so that emit_ptr_to_i32 can convert tags to string ptrs
    // (used when literal tags are passed to string procs like starts-with?, ends-with?, etc.)
    Calcit::Tag(t) => {
      strings.push(t.to_string());
    }
    Calcit::List(xs) => {
      // Pre-intern strings produced by `(format-to-lisp (quote X))` at compile time.
      // The assert= macro expands to this pattern for the error message.
      if xs.len() == 2
        && let Calcit::Proc(p) = &xs[0]
        && matches!(p.as_ref(), "format-to-lisp")
        && let Calcit::List(inner) = &xs[1]
        && inner.len() >= 2
        && let Calcit::Syntax(CalcitSyntax::Quote, _) = &inner[0]
      {
        let s = crate::calcit::format_to_lisp(&inner[1]);
        strings.push(s);
      }
      // Pre-intern lispy strings for literal enum constructors (used by `str`/`turn-string`).
      if let Some(enum_str) = try_format_enum_literal(expr) {
        strings.push(enum_str);
      }
      if matches!(xs.first(), Some(Calcit::Proc(CalcitProc::FormatCirruEdn)))
        && let Some(value) = xs.get(1)
      {
        if let Some(formatted) = edn::try_format_cirru_edn_literal(value) {
          strings.push(formatted);
        }
        if let Some(value_type) = infer_static_type_from_expr(value)
          && let Ok(graph) = DataShapeGraph::build(value_type.as_ref(), "")
        {
          collect_strings_from_data_shape(&graph, strings);
        }
      }
      for x in xs.iter() {
        collect_strings_from_expr(x, strings);
      }
    }
    _ => {}
  }
}

fn expr_uses_cirru_edn_format(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(xs) => {
      matches!(xs.first(), Some(Calcit::Proc(CalcitProc::FormatCirruEdn))) || xs.iter().any(expr_uses_cirru_edn_format)
    }
    _ => false,
  }
}

fn expr_uses_typed_cirru_edn_parse(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(xs) => {
      matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::TryParseCirruEdnAs, _))) || xs.iter().any(expr_uses_typed_cirru_edn_parse)
    }
    _ => false,
  }
}

fn expr_uses_wasi_stdio(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(xs) => {
      let writes = match xs.first() {
        Some(Calcit::Symbol { sym, .. }) => matches!(sym.as_ref(), "println" | "eprintln" | "echo"),
        Some(Calcit::Registered(name)) => matches!(name.as_ref(), "println" | "eprintln" | "echo"),
        _ => false,
      };
      writes || xs.iter().any(expr_uses_wasi_stdio)
    }
    _ => false,
  }
}

#[cfg(test)]
mod tests {
  use std::collections::{BTreeMap, HashMap};
  use std::str::FromStr;
  use std::sync::Arc;

  use super::component::component_type_owns_memory;
  use super::{
    CompiledFn, ComponentAbiInvocation, ComponentAbiType, ComponentAsyncCanonicalImports, ComponentEnumType, ComponentEnumVariant,
    ComponentExportAdapter, ComponentExportRuntime, ComponentImportAdapter, ComponentStructType, ComponentValueCodecs, HostImport,
    WasmBoundary, WasmTarget, build_cabi_realloc_fn, build_component_export_adapter, build_component_import_adapter, build_string_pool,
    component_abi_type, component_export_needs_post_return, component_flat_types, component_import_signature, component_memory_layout,
    component_task_return_signature, host_imports_for_target, index_host_imports, must_reject_extraction_failure,
    reject_reachable_wasi_command_dependencies, validate_component_export_symbols, validate_component_flat_parameters,
    validate_component_import_symbols,
  };
  use crate::calcit::{
    Calcit, CalcitEnumDef, CalcitList, CalcitNumericRefinement, CalcitStructDef, CalcitStructValue, CalcitSyntax, CalcitTypeAnnotation,
  };
  use cirru_edn::EdnTag;
  use wasm_encoder::{Instruction, ValType};

  fn declaration(head: CalcitSyntax) -> Calcit {
    let items = [Calcit::Syntax(head, Arc::from("dependency.ns"))];
    Calcit::List(Arc::new(CalcitList::from(&items[..])))
  }

  fn component_export_runtime<'a>(
    list_codecs: &'a BTreeMap<ComponentAbiType, super::ComponentListCodec>,
    struct_codecs: &'a BTreeMap<ComponentAbiType, super::ComponentStructCodec>,
    variant_codecs: &'a BTreeMap<ComponentAbiType, super::ComponentVariantCodec>,
    drop_indices: &'a BTreeMap<ComponentAbiType, u32>,
  ) -> ComponentExportRuntime<'a> {
    ComponentExportRuntime {
      codecs: ComponentValueCodecs {
        str_new_index: 10,
        buffer_new_index: 11,
        cabi_realloc_index: 30,
        list_codecs,
        struct_codecs,
        variant_codecs,
      },
      cabi_free_index: 29,
      drop_indices,
    }
  }

  fn enum_definition(name: &str, generics: Vec<Arc<str>>, variants: Vec<(&str, Vec<CalcitTypeAnnotation>)>) -> Arc<CalcitEnumDef> {
    let fields = variants.iter().map(|(tag, _)| EdnTag::new(*tag)).collect::<Vec<_>>();
    let values = variants
      .iter()
      .map(|(_, payload)| {
        Calcit::List(Arc::new(CalcitList::Vector(
          payload.iter().map(CalcitTypeAnnotation::to_calcit).collect(),
        )))
      })
      .collect::<Vec<_>>();
    let prototype = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef {
        definition_ref: Some(Arc::from(format!("app.main/{name}"))),
        name: EdnTag::new(name),
        fields: Arc::new(fields),
        field_types: Arc::new(vec![crate::calcit::DYNAMIC_TYPE.clone(); values.len()]),
        generics: Arc::new(generics),
        where_bounds: Arc::new(vec![]),
        impls: vec![],
      }),
      values: Arc::new(values),
    };
    Arc::new(CalcitEnumDef::from_struct(prototype).expect("valid test Enum declaration"))
  }

  #[test]
  fn component_variant_tags_are_available_without_constructor_literals() {
    let tags = super::collect_all_tags_from(&[], None);
    for tag in ["none", "some", "ok", "err"] {
      assert!(tags.contains_key(tag), "missing built-in Component variant tag {tag}");
    }
  }

  #[test]
  fn typed_edn_shape_registers_nominal_struct_tags() {
    let nominal = Arc::new(CalcitStructDef {
      definition_ref: Some(Arc::from("app.main/Job")),
      name: EdnTag::new("Job"),
      fields: Arc::new(vec![EdnTag::new("count")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::Numeric(CalcitNumericRefinement::Int32))]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let shape = crate::calcit::data_shape::DataShapeGraph::build(&CalcitTypeAnnotation::Struct(nominal, Arc::new(vec![])), "app.main")
      .expect("derive Struct shape")
      .into_calcit_handle();
    let mut tags = vec![];
    super::collect_tags_from_expr(&shape, &mut tags);
    assert!(tags.iter().any(|tag| tag == "Job"));
    assert!(tags.iter().any(|tag| tag == "count"));
  }

  #[test]
  fn typed_edn_parser_literals_include_inlined_top_level_values() {
    let value_imports = HashMap::from([(String::from("app.main/parsed"), declaration(CalcitSyntax::TryParseCirruEdnAs))]);
    let tag_index = HashMap::from([(String::from("string"), 1)]);
    let (pool, _, _) = build_string_pool(&[], &value_imports, &tag_index, WasmTarget::Wasi);
    for literal in ["nil", "true", "false", "E_WASM_EDN_SYNTAX: invalid Cirru EDN scalar"] {
      assert!(pool.contains_key(literal), "missing typed parser literal {literal}");
    }
  }

  #[test]
  fn extraction_failure_rejects_target_and_explicit_dependency_export() {
    let ordinary_dependency = declaration(CalcitSyntax::Defn);
    let explicit_dependency_export = declaration(CalcitSyntax::DefWasmExport);

    assert!(must_reject_extraction_failure("target.ns", "target.ns", &ordinary_dependency));
    assert!(must_reject_extraction_failure(
      "target.ns",
      "dependency.ns",
      &explicit_dependency_export
    ));
    assert!(!must_reject_extraction_failure("target.ns", "dependency.ns", &ordinary_dependency));
  }

  #[test]
  fn wasm_target_parser_is_explicit_and_stable() {
    assert_eq!(WasmTarget::from_str("core"), Ok(WasmTarget::Core));
    assert_eq!(WasmTarget::from_str("wasi"), Ok(WasmTarget::Wasi));
    assert!(WasmTarget::from_str("browser").unwrap_err().starts_with("E_WASM_TARGET:"));
  }

  #[test]
  fn wasm_boundary_parser_is_explicit_and_stable() {
    assert_eq!(WasmBoundary::from_str("native"), Ok(WasmBoundary::Native));
    assert_eq!(WasmBoundary::from_str("component"), Ok(WasmBoundary::Component));
    assert!(WasmBoundary::from_str("wit").unwrap_err().starts_with("E_WASM_BOUNDARY:"));
  }

  #[test]
  fn wasi_command_rejects_reachable_failed_dependency_but_not_unrelated_core_code() {
    let function = |instructions| CompiledFn {
      export_name: None,
      params: vec![],
      results: vec![],
      locals: vec![],
      instructions,
    };
    let mut functions = [
      function(vec![Instruction::Call(3)]),
      function(vec![Instruction::Call(4)]),
      function(vec![Instruction::Unreachable]),
      function(vec![]),
    ];
    let unsupported = HashMap::from([(
      4,
      (
        "calcit.core/fs-path:read-text".into(),
        "E_WASI_COMMAND_CAPABILITY: `&fs-read-text` needs WASI 0.3 filesystem lowering".into(),
      ),
    )]);
    let error = reject_reachable_wasi_command_dependencies(&functions, 2, 2, &unsupported).unwrap_err();
    assert!(error.starts_with("E_WASI_COMMAND_CAPABILITY:"), "{error}");
    assert!(error.contains("calcit.core/fs-path:read-text"), "{error}");
    assert!(reject_reachable_wasi_command_dependencies(&functions, 2, 5, &unsupported).is_ok());
    functions[3].instructions.push(Instruction::CallIndirect {
      type_index: 0,
      table_index: 0,
    });
    let indirect = reject_reachable_wasi_command_dependencies(&functions, 2, 5, &unsupported).unwrap_err();
    assert!(indirect.starts_with("E_WASI_COMMAND_INDIRECT:"), "{indirect}");
  }

  #[test]
  fn component_export_adapters_use_canonical_value_and_byte_shapes() {
    let list_codecs = BTreeMap::new();
    let struct_codecs = BTreeMap::new();
    let variant_codecs = BTreeMap::new();
    let drop_indices = BTreeMap::new();
    let runtime = component_export_runtime(&list_codecs, &struct_codecs, &variant_codecs, &drop_indices);
    let allocator = build_cabi_realloc_fn(9, 10);
    assert_eq!(allocator.export_name.as_deref(), Some("cabi_realloc"));
    assert_eq!(allocator.params, vec![ValType::I32; 4]);
    assert_eq!(allocator.results, vec![ValType::I32]);

    let number = build_component_export_adapter(
      &ComponentExportAdapter {
        definition: "app.main/add-one".into(),
        symbol: "add-one".into(),
        target_index: 20,
        invocation: ComponentAbiInvocation::Sync,
        stackless_tail_import: None,
        stream_consumer: None,
        task_return_index: None,
        parameters: vec![ComponentAbiType::Number],
        result: ComponentAbiType::Number,
      },
      &runtime,
    );
    assert_eq!(number.params, vec![ValType::F64]);
    assert_eq!(number.results, vec![ValType::F64]);

    let string = build_component_export_adapter(
      &ComponentExportAdapter {
        definition: "app.main/echo".into(),
        symbol: "echo".into(),
        target_index: 21,
        invocation: ComponentAbiInvocation::Sync,
        stackless_tail_import: None,
        stream_consumer: None,
        task_return_index: None,
        parameters: vec![ComponentAbiType::String],
        result: ComponentAbiType::String,
      },
      &runtime,
    );
    assert_eq!(string.params, vec![ValType::I32, ValType::I32]);
    assert_eq!(string.results, vec![ValType::I32]);

    let buffer = build_component_export_adapter(
      &ComponentExportAdapter {
        definition: "app.main/echo-buffer".into(),
        symbol: "echo-buffer".into(),
        target_index: 22,
        invocation: ComponentAbiInvocation::Sync,
        stackless_tail_import: None,
        stream_consumer: None,
        task_return_index: None,
        parameters: vec![ComponentAbiType::Buffer],
        result: ComponentAbiType::Buffer,
      },
      &runtime,
    );
    assert_eq!(buffer.params, vec![ValType::I32, ValType::I32]);
    assert_eq!(buffer.results, vec![ValType::I32]);
  }

  #[test]
  fn component_async_export_uses_task_return_and_has_no_core_result() {
    let list_codecs = BTreeMap::new();
    let struct_codecs = BTreeMap::new();
    let variant_codecs = BTreeMap::new();
    let drop_indices = BTreeMap::new();
    let runtime = component_export_runtime(&list_codecs, &struct_codecs, &variant_codecs, &drop_indices);
    let adapter = ComponentExportAdapter {
      definition: "app.main/load-text".into(),
      symbol: "load-text".into(),
      target_index: 20,
      invocation: ComponentAbiInvocation::Async,
      stackless_tail_import: None,
      stream_consumer: None,
      task_return_index: Some(31),
      parameters: vec![ComponentAbiType::String],
      result: ComponentAbiType::String,
    };
    let compiled = build_component_export_adapter(&adapter, &runtime);

    assert_eq!(compiled.params, vec![ValType::I32, ValType::I32]);
    assert!(compiled.results.is_empty());
    assert_eq!(component_task_return_signature(&adapter.result), vec![ValType::I32, ValType::I32]);
    assert_eq!(
      compiled
        .instructions
        .iter()
        .filter(|instruction| matches!(instruction, Instruction::Call(31)))
        .count(),
      1
    );
  }

  #[test]
  fn component_import_adapters_use_canonical_value_and_byte_shapes() {
    let list_codecs = BTreeMap::new();
    let struct_codecs = BTreeMap::new();
    let variant_codecs = BTreeMap::new();
    let codecs = ComponentValueCodecs {
      str_new_index: 10,
      buffer_new_index: 11,
      cabi_realloc_index: 30,
      list_codecs: &list_codecs,
      struct_codecs: &struct_codecs,
      variant_codecs: &variant_codecs,
    };
    let number = ComponentImportAdapter {
      definition: "app.main/host-add-one".into(),
      module: "host".into(),
      symbol: "add-one".into(),
      raw_index: 2,
      source_arity: 1,
      invocation: ComponentAbiInvocation::Sync,
      parameters: vec![ComponentAbiType::Number],
      result: ComponentAbiType::Number,
    };
    assert_eq!(component_import_signature(&number), (vec![ValType::F64], vec![ValType::F64]));
    let number_adapter = build_component_import_adapter(&number, None, 12, &codecs);
    assert_eq!(number_adapter.params, vec![ValType::F64]);
    assert_eq!(number_adapter.results, vec![ValType::F64]);

    let string = ComponentImportAdapter {
      definition: "app.main/host-echo".into(),
      module: "host".into(),
      symbol: "echo".into(),
      raw_index: 3,
      source_arity: 1,
      invocation: ComponentAbiInvocation::Sync,
      parameters: vec![ComponentAbiType::String],
      result: ComponentAbiType::String,
    };
    assert_eq!(
      component_import_signature(&string),
      (vec![ValType::I32, ValType::I32, ValType::I32], vec![])
    );
    let string_adapter = build_component_import_adapter(&string, None, 12, &codecs);
    assert_eq!(string_adapter.params, vec![ValType::F64]);
    assert_eq!(string_adapter.results, vec![ValType::F64]);

    let buffer = ComponentImportAdapter {
      definition: "app.main/host-buffer".into(),
      module: "host".into(),
      symbol: "buffer".into(),
      raw_index: 4,
      source_arity: 1,
      invocation: ComponentAbiInvocation::Sync,
      parameters: vec![ComponentAbiType::Buffer],
      result: ComponentAbiType::Buffer,
    };
    assert_eq!(
      component_import_signature(&buffer),
      (vec![ValType::I32, ValType::I32, ValType::I32], vec![])
    );
    let buffer_adapter = build_component_import_adapter(&buffer, None, 12, &codecs);
    assert_eq!(buffer_adapter.params, vec![ValType::F64]);
    assert_eq!(buffer_adapter.results, vec![ValType::F64]);
  }

  #[test]
  fn component_async_import_uses_status_waitable_and_drop_shape() {
    let list_codecs = BTreeMap::new();
    let struct_codecs = BTreeMap::new();
    let variant_codecs = BTreeMap::new();
    let codecs = ComponentValueCodecs {
      str_new_index: 10,
      buffer_new_index: 11,
      cabi_realloc_index: 30,
      list_codecs: &list_codecs,
      struct_codecs: &struct_codecs,
      variant_codecs: &variant_codecs,
    };
    let canonical = ComponentAsyncCanonicalImports {
      waitable_set_new: 30,
      waitable_set_wait: 31,
      waitable_set_drop: 32,
      waitable_join: 33,
      subtask_drop: 34,
    };
    let adapter = ComponentImportAdapter {
      definition: "app.main/host-load".into(),
      module: "host".into(),
      symbol: "load".into(),
      raw_index: 2,
      source_arity: 1,
      invocation: ComponentAbiInvocation::Async,
      parameters: vec![ComponentAbiType::Number],
      result: ComponentAbiType::Number,
    };
    assert_eq!(
      component_import_signature(&adapter),
      (vec![ValType::F64, ValType::I32], vec![ValType::I32])
    );
    let compiled = build_component_import_adapter(&adapter, Some(&canonical), 12, &codecs);
    assert_eq!(compiled.params, vec![ValType::F64]);
    assert_eq!(compiled.results, vec![ValType::F64]);
    for index in [
      adapter.raw_index,
      canonical.waitable_set_wait,
      canonical.subtask_drop,
      canonical.waitable_set_drop,
    ] {
      assert!(
        compiled
          .instructions
          .iter()
          .any(|instruction| matches!(instruction, Instruction::Call(actual) if *actual == index)),
        "missing async import lifecycle call {index}"
      );
    }
  }

  #[test]
  fn component_async_import_uses_a_record_pointer_for_wide_parameters() {
    let adapter = ComponentImportAdapter {
      definition: "app.main/host-load".into(),
      module: "host".into(),
      symbol: "load".into(),
      raw_index: 1,
      source_arity: 3,
      invocation: ComponentAbiInvocation::Async,
      parameters: vec![ComponentAbiType::String, ComponentAbiType::String, ComponentAbiType::Number],
      result: ComponentAbiType::Number,
    };
    assert_eq!(
      component_import_signature(&adapter),
      (vec![ValType::I32, ValType::I32], vec![ValType::I32])
    );
  }

  #[test]
  fn component_numeric_refinements_use_canonical_flat_shapes_and_memory_layouts() {
    let cases = [
      (CalcitNumericRefinement::Int8, ValType::I32, 1, 1),
      (CalcitNumericRefinement::UInt8, ValType::I32, 1, 1),
      (CalcitNumericRefinement::Int16, ValType::I32, 2, 2),
      (CalcitNumericRefinement::UInt16, ValType::I32, 2, 2),
      (CalcitNumericRefinement::Int32, ValType::I32, 4, 4),
      (CalcitNumericRefinement::UInt32, ValType::I32, 4, 4),
      (CalcitNumericRefinement::Int64, ValType::I64, 8, 8),
      (CalcitNumericRefinement::UInt64, ValType::I64, 8, 8),
      (CalcitNumericRefinement::Float32, ValType::F32, 4, 4),
      (CalcitNumericRefinement::Float64, ValType::F64, 8, 8),
    ];
    for (kind, flat, size, alignment) in cases {
      let annotation = CalcitTypeAnnotation::Numeric(kind);
      let value_type = component_abi_type(&annotation, "app.main/numeric", "logical_schema.result")
        .expect("numeric refinement should cross the Component boundary");
      assert_eq!(value_type, ComponentAbiType::Numeric(kind));
      assert_eq!(component_flat_types(&value_type), vec![flat]);
      let layout = component_memory_layout(&value_type);
      assert_eq!((layout.size, layout.alignment), (size, alignment));
    }

    let joined = ComponentAbiType::Result(
      Box::new(ComponentAbiType::Numeric(CalcitNumericRefinement::Float32)),
      Box::new(ComponentAbiType::Numeric(CalcitNumericRefinement::Int32)),
    );
    assert_eq!(component_flat_types(&joined), vec![ValType::I32, ValType::I32]);

    let nested = ComponentAbiType::Result(
      Box::new(ComponentAbiType::Numeric(CalcitNumericRefinement::UInt64)),
      Box::new(ComponentAbiType::Numeric(CalcitNumericRefinement::Float32)),
    );
    assert_eq!(component_flat_types(&nested), vec![ValType::I32, ValType::I64]);
    let nested_layout = component_memory_layout(&nested);
    assert_eq!((nested_layout.size, nested_layout.alignment), (16, 8));

    let option = ComponentAbiType::Option(Box::new(ComponentAbiType::Numeric(CalcitNumericRefinement::Int16)));
    assert_eq!(component_flat_types(&option), vec![ValType::I32, ValType::I32]);
    let option_layout = component_memory_layout(&option);
    assert_eq!((option_layout.size, option_layout.alignment), (4, 2));

    let list = ComponentAbiType::List(Box::new(ComponentAbiType::Numeric(CalcitNumericRefinement::UInt64)));
    assert_eq!(component_flat_types(&list), vec![ValType::I32, ValType::I32]);
    let list_layout = component_memory_layout(&list);
    assert_eq!((list_layout.size, list_layout.alignment), (8, 4));
  }

  #[test]
  fn component_struct_shape_uses_normalized_fields_and_monomorphized_types() {
    let stats = Arc::new(CalcitStructDef {
      definition_ref: Some(Arc::from("app.main/ProfileStats")),
      name: cirru_edn::EdnTag::new("ProfileStats"),
      fields: Arc::new(vec![cirru_edn::EdnTag::new("score")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let profile = Arc::new(CalcitStructDef {
      definition_ref: Some(Arc::from("app.main/Profile")),
      name: cirru_edn::EdnTag::new("Profile"),
      fields: Arc::new(vec![
        cirru_edn::EdnTag::new("active"),
        cirru_edn::EdnTag::new("name"),
        cirru_edn::EdnTag::new("stats"),
      ]),
      field_types: Arc::new(vec![
        Arc::new(CalcitTypeAnnotation::Bool),
        Arc::new(CalcitTypeAnnotation::String),
        Arc::new(CalcitTypeAnnotation::Struct(stats, Arc::new(vec![]))),
      ]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let profile_type = component_abi_type(
      &CalcitTypeAnnotation::Struct(profile, Arc::new(vec![])),
      "app.main/echo-profile",
      "logical_schema.result",
    )
    .expect("derive Struct Component shape");
    assert_eq!(
      component_flat_types(&profile_type),
      vec![ValType::I32, ValType::I32, ValType::I32, ValType::F64]
    );
    assert_eq!(component_memory_layout(&profile_type).size, 24);
    assert_eq!(component_memory_layout(&profile_type).alignment, 8);

    let generic_box = Arc::new(CalcitStructDef {
      definition_ref: Some(Arc::from("app.main/Box")),
      name: cirru_edn::EdnTag::new("Box"),
      fields: Arc::new(vec![cirru_edn::EdnTag::new("value")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))]),
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let concrete_box = component_abi_type(
      &CalcitTypeAnnotation::Struct(generic_box, Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)])),
      "app.main/echo-box",
      "logical_schema.result",
    )
    .expect("monomorphize Struct Component shape");
    assert_eq!(component_flat_types(&concrete_box), vec![ValType::F64]);

    let wide_record = ComponentAbiType::Struct(ComponentStructType {
      id: "app.main/Wide".into(),
      tag: "Wide".into(),
      fields: (0..17).map(|index| (format!("field-{index}"), ComponentAbiType::Number)).collect(),
    });
    let error = validate_component_flat_parameters(&[wide_record], "app.main/echo-wide")
      .expect_err("wide Struct parameters must not bypass the Canonical ABI flat-parameter limit");
    assert!(error.contains("E_COMPONENT_ABI_FLAT_PARAMETER_LIMIT"));
    assert!(error.contains("flattens to 17 values"));
  }

  #[test]
  fn component_enum_shape_joins_payloads_and_rejects_open_shapes() {
    let event = enum_definition(
      "Event",
      vec![],
      vec![
        ("idle", vec![]),
        ("moved", vec![CalcitTypeAnnotation::Number, CalcitTypeAnnotation::Number]),
        ("named", vec![CalcitTypeAnnotation::String]),
      ],
    );
    let event_type = component_abi_type(
      &CalcitTypeAnnotation::Enum(event, Arc::new(vec![])),
      "app.main/echo-event",
      "logical_schema.result",
    )
    .expect("derive Enum Component shape");
    assert_eq!(component_flat_types(&event_type), vec![ValType::I32, ValType::I64, ValType::I64]);
    assert_eq!(component_memory_layout(&event_type).size, 24);
    assert_eq!(component_memory_layout(&event_type).alignment, 8);

    let generic = enum_definition(
      "Boxed",
      vec![Arc::from("T")],
      vec![("value", vec![CalcitTypeAnnotation::TypeVar(Arc::from("T"))])],
    );
    let generic_error = component_abi_type(
      &CalcitTypeAnnotation::Enum(generic, Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)])),
      "app.main/echo-boxed",
      "logical_schema.result",
    )
    .expect_err("generic Enum must remain outside the Component boundary");
    assert!(generic_error.contains("E_COMPONENT_ABI_UNSUPPORTED_GENERIC"));

    let explicit_unit = enum_definition("Bad", vec![], vec![("unit", vec![CalcitTypeAnnotation::Unit])]);
    let unit_error = component_abi_type(
      &CalcitTypeAnnotation::Enum(explicit_unit, Arc::new(vec![])),
      "app.main/echo-bad",
      "logical_schema.result",
    )
    .expect_err("explicit Unit payload must be omitted");
    assert!(unit_error.contains("E_COMPONENT_ABI_UNIT_ENUM_PAYLOAD"));
    assert!(unit_error.contains("variants[0].payload[0]"));

    let open_error = component_abi_type(&CalcitTypeAnnotation::AnonymousEnum, "app.main/echo-open", "logical_schema.result")
      .expect_err("anonymous Enum must remain outside the Component boundary");
    assert!(open_error.contains("E_COMPONENT_ABI_UNSUPPORTED_TYPE"));
    assert!(open_error.contains("logical_schema.result"));

    let enum_with_cases = |count| {
      ComponentAbiType::Enum(ComponentEnumType {
        id: format!("app.main/Wide{count}"),
        variants: (0..count)
          .map(|index| ComponentEnumVariant {
            tag: format!("case-{index}"),
            payload: vec![],
          })
          .collect(),
      })
    };
    let u16_layout = component_memory_layout(&enum_with_cases(257));
    assert_eq!((u16_layout.size, u16_layout.alignment), (2, 2));
    let u32_layout = component_memory_layout(&enum_with_cases(65_537));
    assert_eq!((u32_layout.size, u32_layout.alignment), (4, 4));
  }

  #[test]
  fn component_bool_adapters_use_canonical_i32_and_strict_conversion() {
    let list_codecs = BTreeMap::new();
    let struct_codecs = BTreeMap::new();
    let variant_codecs = BTreeMap::new();
    let drop_indices = BTreeMap::new();
    let runtime = component_export_runtime(&list_codecs, &struct_codecs, &variant_codecs, &drop_indices);
    assert_eq!(
      component_abi_type(&CalcitTypeAnnotation::Bool, "app.main/check", "logical_schema.result"),
      Ok(ComponentAbiType::Bool)
    );
    let export = build_component_export_adapter(
      &ComponentExportAdapter {
        definition: "app.main/not".into(),
        symbol: "not".into(),
        target_index: 20,
        invocation: ComponentAbiInvocation::Sync,
        stackless_tail_import: None,
        stream_consumer: None,
        task_return_index: None,
        parameters: vec![ComponentAbiType::Bool],
        result: ComponentAbiType::Bool,
      },
      &runtime,
    );
    assert_eq!(export.params, vec![ValType::I32]);
    assert_eq!(export.results, vec![ValType::I32]);
    assert!(
      export
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, wasm_encoder::Instruction::Unreachable))
    );

    let import = ComponentImportAdapter {
      definition: "app.main/host-not".into(),
      module: "host".into(),
      symbol: "not".into(),
      raw_index: 2,
      source_arity: 1,
      invocation: ComponentAbiInvocation::Sync,
      parameters: vec![ComponentAbiType::Bool],
      result: ComponentAbiType::Bool,
    };
    assert_eq!(component_import_signature(&import), (vec![ValType::I32], vec![ValType::I32]));
    let codecs = ComponentValueCodecs {
      str_new_index: 10,
      buffer_new_index: 11,
      cabi_realloc_index: 12,
      list_codecs: &list_codecs,
      struct_codecs: &struct_codecs,
      variant_codecs: &variant_codecs,
    };
    let import_adapter = build_component_import_adapter(&import, None, 12, &codecs);
    assert_eq!(import_adapter.params, vec![ValType::F64]);
    assert_eq!(import_adapter.results, vec![ValType::F64]);
    assert!(
      import_adapter
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, wasm_encoder::Instruction::Unreachable))
    );
  }

  #[test]
  fn component_adapter_accepts_buffer_and_unit() {
    assert_eq!(
      component_abi_type(&CalcitTypeAnnotation::Buffer, "app.main/read", "logical_schema.result"),
      Ok(ComponentAbiType::Buffer)
    );
    assert_eq!(
      component_abi_type(&CalcitTypeAnnotation::Unit, "app.main/read", "logical_schema.result"),
      Ok(ComponentAbiType::Unit)
    );
  }

  #[test]
  fn component_adapter_derives_recursive_list_types_and_precise_item_paths() {
    let nested = CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Number))));
    assert_eq!(
      component_abi_type(&nested, "app.main/echo", "logical_schema.parameters[0]"),
      Ok(ComponentAbiType::List(Box::new(ComponentAbiType::List(Box::new(
        ComponentAbiType::Number
      )))))
    );

    let unsupported = CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::String),
      Arc::new(CalcitTypeAnnotation::Number),
    )))));
    let error = component_abi_type(&unsupported, "app.main/echo", "logical_schema.parameters[0]")
      .expect_err("unsupported nested item types must not degrade to Dynamic");
    assert!(error.contains("logical_schema.parameters[0].item.item"));
  }

  #[test]
  fn component_post_return_tracks_nested_ownership_and_indirect_areas() {
    let scalar = ComponentAbiType::Struct(ComponentStructType {
      id: "app.main/Scalar".into(),
      tag: "Scalar".into(),
      fields: vec![("value".into(), ComponentAbiType::Number)],
    });
    let nested = ComponentAbiType::Struct(ComponentStructType {
      id: "app.main/Nested".into(),
      tag: "Nested".into(),
      fields: vec![(
        "value".into(),
        ComponentAbiType::Option(Box::new(ComponentAbiType::List(Box::new(ComponentAbiType::String)))),
      )],
    });
    assert!(!component_type_owns_memory(&scalar));
    assert!(component_type_owns_memory(&nested));

    let adapter = |invocation, result| ComponentExportAdapter {
      definition: "app.main/run".into(),
      symbol: "run".into(),
      target_index: 20,
      invocation,
      stackless_tail_import: None,
      stream_consumer: None,
      task_return_index: None,
      parameters: vec![],
      result,
    };
    assert!(!component_export_needs_post_return(&adapter(ComponentAbiInvocation::Sync, scalar)));
    assert!(component_export_needs_post_return(&adapter(
      ComponentAbiInvocation::Sync,
      ComponentAbiType::Struct(ComponentStructType {
        id: "app.main/Wide".into(),
        tag: "Wide".into(),
        fields: vec![
          ("left".into(), ComponentAbiType::Number),
          ("right".into(), ComponentAbiType::Number)
        ],
      })
    )));
    assert!(!component_export_needs_post_return(&adapter(
      ComponentAbiInvocation::Async,
      ComponentAbiType::Bool
    )));
  }

  #[test]
  fn component_adapter_derives_nominal_option_result_and_shared_variant_shapes() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let string = Arc::new(CalcitTypeAnnotation::String);
    let option = CalcitTypeAnnotation::TypeRef(Arc::from("calcit.core/Option"), Arc::new(vec![number.clone()]));
    let result = CalcitTypeAnnotation::TypeRef(Arc::from("calcit.core/Result"), Arc::new(vec![number, string]));
    assert_eq!(
      component_abi_type(&option, "app.main/option", "logical_schema.result"),
      Ok(ComponentAbiType::Option(Box::new(ComponentAbiType::Number)))
    );
    let result_type = component_abi_type(&result, "app.main/result", "logical_schema.result").expect("core Result must resolve");
    assert_eq!(
      result_type,
      ComponentAbiType::Result(Box::new(ComponentAbiType::Number), Box::new(ComponentAbiType::String))
    );
    assert_eq!(component_flat_types(&result_type), vec![ValType::I32, ValType::I64, ValType::I32]);
    let layout = component_memory_layout(&result_type);
    assert_eq!((layout.size, layout.alignment), (16, 8));
    let nested = ComponentAbiType::Result(Box::new(result_type), Box::new(ComponentAbiType::String));
    assert_eq!(
      component_flat_types(&nested),
      vec![ValType::I32, ValType::I32, ValType::I64, ValType::I32]
    );

    let legacy = CalcitTypeAnnotation::Optional(Arc::new(CalcitTypeAnnotation::Number));
    let error = component_abi_type(&legacy, "app.main/legacy", "logical_schema.result")
      .expect_err("legacy Optional must not degrade into nominal Option");
    assert!(error.contains("E_COMPONENT_ABI_UNSUPPORTED_TYPE"));
  }

  #[test]
  fn component_adapter_rejects_duplicate_export_symbols_deterministically() {
    let adapter = |definition: &str, symbol: &str, invocation: ComponentAbiInvocation| ComponentExportAdapter {
      definition: definition.into(),
      symbol: symbol.into(),
      target_index: 20,
      invocation,
      stackless_tail_import: None,
      stream_consumer: None,
      task_return_index: None,
      parameters: vec![ComponentAbiType::Number],
      result: ComponentAbiType::Number,
    };
    let error = validate_component_export_symbols(&[
      adapter("a.main/run", "run", ComponentAbiInvocation::Sync),
      adapter("b.main/run", "run", ComponentAbiInvocation::Sync),
    ])
    .expect_err("duplicate export symbols must fail before WASM encoding");
    assert_eq!(
      error,
      "E_COMPONENT_ABI_SYMBOL_CONFLICT: export symbol `run` is declared by a.main/run, b.main/run"
    );
  }

  #[test]
  fn component_adapter_rejects_export_symbols_that_collide_after_async_mangling() {
    let adapter = |definition: &str, symbol: &str, invocation: ComponentAbiInvocation| ComponentExportAdapter {
      definition: definition.into(),
      symbol: symbol.into(),
      target_index: 20,
      invocation,
      stackless_tail_import: None,
      stream_consumer: None,
      task_return_index: None,
      parameters: vec![ComponentAbiType::Number],
      result: ComponentAbiType::Number,
    };
    let error = validate_component_export_symbols(&[
      adapter("a.main/reserved", "[async-lift-stackful]run", ComponentAbiInvocation::Sync),
      adapter("b.main/run", "run", ComponentAbiInvocation::Async),
    ])
    .expect_err("emitted export names must be unique after async mangling");
    assert_eq!(
      error,
      "E_COMPONENT_ABI_SYMBOL_CONFLICT: export symbol `[async-lift-stackful]run` is declared by a.main/reserved, b.main/run"
    );
  }

  #[test]
  fn component_adapter_rejects_symbols_that_collide_with_stackless_callback() {
    let adapter = |definition: &str, symbol: &str, invocation: ComponentAbiInvocation| ComponentExportAdapter {
      definition: definition.into(),
      symbol: symbol.into(),
      target_index: 20,
      invocation,
      stackless_tail_import: None,
      stream_consumer: None,
      task_return_index: None,
      parameters: vec![ComponentAbiType::Number],
      result: ComponentAbiType::Number,
    };
    let mut stackless = adapter("b.main/run", "run", ComponentAbiInvocation::Async);
    stackless.stackless_tail_import = Some("host/run".into());
    let error = validate_component_export_symbols(&[
      adapter("a.main/reserved", "[callback][async-lift]run", ComponentAbiInvocation::Sync),
      stackless,
    ])
    .expect_err("stackless callback export names must be reserved");
    assert_eq!(
      error,
      "E_COMPONENT_ABI_SYMBOL_CONFLICT: export symbol `[callback][async-lift]run` is declared by a.main/reserved, b.main/run"
    );
  }

  #[test]
  fn component_adapter_rejects_symbols_that_collide_with_post_return() {
    let adapter = |definition: &str, symbol: &str, result: ComponentAbiType| ComponentExportAdapter {
      definition: definition.into(),
      symbol: symbol.into(),
      target_index: 20,
      invocation: ComponentAbiInvocation::Sync,
      stackless_tail_import: None,
      stream_consumer: None,
      task_return_index: None,
      parameters: vec![],
      result,
    };
    let error = validate_component_export_symbols(&[
      adapter("a.main/reserved", "cabi_post_run", ComponentAbiType::Number),
      adapter("b.main/run", "run", ComponentAbiType::String),
    ])
    .expect_err("post-return export names must be reserved");
    assert_eq!(
      error,
      "E_COMPONENT_ABI_SYMBOL_CONFLICT: export symbol `cabi_post_run` is declared by a.main/reserved, b.main/run"
    );
  }

  #[test]
  fn component_adapter_rejects_duplicate_import_symbols_deterministically() {
    let adapter = |definition: &str, symbol: &str, invocation: ComponentAbiInvocation| ComponentImportAdapter {
      definition: definition.into(),
      module: "host".into(),
      symbol: symbol.into(),
      raw_index: 0,
      source_arity: 1,
      invocation,
      parameters: vec![ComponentAbiType::Number],
      result: ComponentAbiType::Number,
    };
    let error = validate_component_import_symbols(&[
      adapter("a.main/run", "run", ComponentAbiInvocation::Sync),
      adapter("b.main/run", "run", ComponentAbiInvocation::Sync),
    ])
    .expect_err("duplicate import symbols must fail before WASM encoding");
    assert_eq!(
      error,
      "E_COMPONENT_ABI_SYMBOL_CONFLICT: import symbol `host/run` is declared by a.main/run, b.main/run"
    );
  }

  #[test]
  fn component_adapter_rejects_import_symbols_that_collide_after_async_mangling() {
    let adapter = |definition: &str, symbol: &str, invocation: ComponentAbiInvocation| ComponentImportAdapter {
      definition: definition.into(),
      module: "host".into(),
      symbol: symbol.into(),
      raw_index: 0,
      source_arity: 1,
      invocation,
      parameters: vec![ComponentAbiType::Number],
      result: ComponentAbiType::Number,
    };
    let error = validate_component_import_symbols(&[
      adapter("a.main/reserved", "[async-lower]run", ComponentAbiInvocation::Sync),
      adapter("b.main/run", "run", ComponentAbiInvocation::Async),
    ])
    .expect_err("emitted import names must be unique after async mangling");
    assert_eq!(
      error,
      "E_COMPONENT_ABI_SYMBOL_CONFLICT: import symbol `host/[async-lower]run` is declared by a.main/reserved, b.main/run"
    );
  }

  #[test]
  fn host_import_index_keeps_the_registered_capability() {
    let imports = [
      HostImport {
        module: "io".into(),
        name: "log_value".into(),
        params: vec![ValType::F64],
        results: vec![ValType::F64],
      },
      HostImport {
        module: "io".into(),
        name: "log_value".into(),
        params: vec![ValType::F64; 2],
        results: vec![ValType::F64],
      },
    ];
    assert_eq!(index_host_imports(&imports).get(&("io".into(), "log_value".into())), Some(&0));
  }

  #[test]
  fn wasi_target_registers_the_preview1_command_abi() {
    let imports = host_imports_for_target(WasmTarget::Wasi);
    let signatures = imports
      .iter()
      .map(|import| (import.name.as_str(), import.params.clone(), import.results.clone()))
      .collect::<Vec<_>>();
    assert_eq!(
      signatures,
      vec![
        ("fd_write", vec![ValType::I32; 4], vec![ValType::I32]),
        ("args_sizes_get", vec![ValType::I32; 2], vec![ValType::I32]),
        ("args_get", vec![ValType::I32; 2], vec![ValType::I32]),
        ("environ_sizes_get", vec![ValType::I32; 2], vec![ValType::I32]),
        ("environ_get", vec![ValType::I32; 2], vec![ValType::I32]),
        ("proc_exit", vec![ValType::I32], vec![]),
        ("clock_time_get", vec![ValType::I32, ValType::I64, ValType::I32], vec![ValType::I32],),
        ("poll_oneoff", vec![ValType::I32; 4], vec![ValType::I32]),
        ("random_get", vec![ValType::I32, ValType::I32], vec![ValType::I32]),
        ("fd_prestat_get", vec![ValType::I32; 2], vec![ValType::I32]),
        ("fd_prestat_dir_name", vec![ValType::I32; 3], vec![ValType::I32]),
        (
          "path_open",
          vec![
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I64,
            ValType::I64,
            ValType::I32,
            ValType::I32,
          ],
          vec![ValType::I32],
        ),
        ("fd_filestat_get", vec![ValType::I32; 2], vec![ValType::I32]),
        ("fd_read", vec![ValType::I32; 4], vec![ValType::I32]),
        (
          "fd_readdir",
          vec![ValType::I32, ValType::I32, ValType::I32, ValType::I64, ValType::I32],
          vec![ValType::I32],
        ),
        ("fd_close", vec![ValType::I32], vec![ValType::I32]),
      ]
    );
    assert!(imports.iter().all(|import| import.module == "wasi_snapshot_preview1"));
  }
}
