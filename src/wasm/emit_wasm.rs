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

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use wasm_encoder::{
  CodeSection, ConstExpr, ElementSection, Elements, ExportKind, ExportSection, Function, FunctionSection, GlobalSection, GlobalType,
  Ieee64, Instruction, MemorySection, MemoryType, Module, RefType, TableSection, TableType, TypeSection, ValType,
};

use crate::wasm::host::builtins::syntax::get_raw_args_fn;
use crate::wasm::host::calcit::data_shape::{DataShapeGraph, DataShapeNode};
use crate::wasm::host::calcit::type_annotation::TypeBindings;
use crate::wasm::host::calcit::{
  Calcit, CalcitArgLabel, CalcitEnumDef, CalcitFnArgs, CalcitFnTypeAnnotation, CalcitImport, CalcitLocal, CalcitNumericRefinement,
  CalcitProc, CalcitStructDef, CalcitSyntax, CalcitTypeAnnotation, DYNAMIC_TYPE, ImportInfo, MethodKind, NullableOrigin,
};
use crate::wasm::host::program;
use crate::wasm::host::runner::preprocess::{
  expression_definitely_diverges, infer_static_type_from_expr, resolve_runtime_method_callable, resolve_static_trait_callable,
};

mod any_box;
mod closure;
mod component;
mod dyn_trait;
mod edn_parse;
mod http_proxy;
mod link;
mod memory;
mod methods;
mod number_format;
mod runtime;
mod static_data;
mod structs;

use static_data::*;

use component::{
  ComponentListCodec, ComponentStructCodec, ComponentValueCodecs, ComponentVariantCodec, build_component_drop_fn,
  build_component_enum_lift_fn, build_component_enum_lower_fn, build_component_list_lift_fn, build_component_list_lower_fn,
  build_component_post_return_fn, build_component_struct_lift_fn, build_component_struct_lower_fn, build_component_variant_lift_fn,
  build_component_variant_lower_fn, collect_component_compound_types, collect_component_owned_types, component_fields_layout,
  component_memory_layout, push_component_result_reclaim, push_load_value_flat, push_store_value_flat,
};
use memory::{
  ComponentMemoryGlobals, PERSISTENT_COMPACT_THRESHOLD, PersistentPromoteFns, PromoteTypeTags, RelocateScratchFns, WasmGlobalLayout,
  WasmMemoryLayout, build_arena_reset_fn, build_rt_arena_alloc_fn, build_rt_commit_defref_fn, build_rt_compact_persistent_fn,
  build_rt_component_rewind_to_mark_fn, build_rt_component_turn_enter_fn, build_rt_component_turn_exit_fn, build_rt_fwd_get_fn,
  build_rt_fwd_grow_fn, build_rt_fwd_mark_flag_fn, build_rt_fwd_put_fn, build_rt_is_heap_ptr_fn, build_rt_persistent_alloc_fn,
  build_rt_promote_map_node_fn, build_rt_promote_value_fn, build_rt_relocate_scratch_map_node_fn, build_rt_relocate_scratch_refs_fn,
  build_rt_turn_end_fn, build_rt_turn_enter_fn,
};
use methods::{emit_call_args, emit_method_invoke, emit_type_of_local};
use runtime::{
  HostImport, ModuleFunctionLayout, RuntimeFns, WasiComponentReadDirImports, WasiComponentReadImports, WasiComponentWriteImports,
  build_runtime_fns, build_utf8_valid_fn, build_wasi_component_get_env_fn, build_wasi_component_open_at_fn,
  build_wasi_component_random_get_fn, build_wasi_component_read_bytes_fn, build_wasi_component_read_dir_fn,
  build_wasi_component_route_path_fn, build_wasi_component_select_preopen_fn, build_wasi_component_wait_fn,
  build_wasi_component_write_bytes_fn, build_wasi_get_args_fn, build_wasi_get_env_fn, build_wasi_open_path_fn, build_wasi_read_dir_fn,
  build_wasi_read_text_fn, build_wasi_wait_fn, build_wasi_write_all_fn, build_wasi_write_text_fn, build_wasm_module, core_host_import,
  host_imports_for_target,
};
#[allow(unused_imports)]
use structs::{
  EnumLayouts, StructFieldKind, StructFieldLayout, StructLayout, StructLayouts, collect_enum_layouts, collect_struct_layouts,
  component_struct_identity, emit_alloc_legacy_enum_from_f64_locals, emit_alloc_nominal_enum_from_f64_locals, emit_enum_assoc,
  emit_enum_count, emit_enum_definition, emit_enum_new, emit_enum_new_raw_i32, emit_enum_nth, emit_enum_nth_as_repr,
  emit_load_enum_payload_as_f64, emit_load_struct_field_as_repr, emit_named_enum_new, emit_named_enum_new_raw_i32,
  emit_store_compact_header, emit_store_enum_header, emit_store_struct_field_expr, emit_store_struct_field_from_f64_local,
  emit_struct_assoc_at, emit_struct_contains, emit_struct_count, emit_struct_def, emit_struct_field_tag, emit_struct_get,
  emit_struct_get_name, emit_struct_matches, emit_struct_new, emit_struct_new_raw_i32, emit_struct_nth, emit_struct_nth_as_repr,
  emit_struct_to_map, emit_struct_with_at, enum_identity_tag, option_niche_inner, qualify_for_definition_ref, resolve_enum_ref,
  resolve_struct_ref, try_parse_defrecord_form,
};

/// Low memory holds WASI scratch, the private Number formatter stack, and its
/// read-only decimal table. String literals begin after this reserved region.
const HEAP_BASE: i32 = 16 * 1024;
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

mod edn;
/// Emit a WASM binary module from the compiled program.
/// Processes functions from all namespaces in the program.
mod heap;
mod hof;
mod lists;
mod maps;
mod sets;
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WasiWorld {
  #[default]
  Command,
  Http,
}

impl FromStr for WasiWorld {
  type Err = String;

  fn from_str(value: &str) -> Result<Self, Self::Err> {
    match value {
      "command" => Ok(Self::Command),
      "http" => Ok(Self::Http),
      _ => Err(format!("E_WASI_WORLD: unknown WASI world `{value}`; expected `command` or `http`")),
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
  emit_wasm_with_world(init_ns, init_def, emit_path, target, boundary, WasiWorld::Command)
}

pub fn emit_wasm_with_world(
  init_ns: &str,
  init_def: &str,
  emit_path: &str,
  target: WasmTarget,
  boundary: WasmBoundary,
  world: WasiWorld,
) -> Result<(), String> {
  emit_wasm_impl(init_ns, init_def, emit_path, target, boundary, world, true)
}

pub fn check_wasm_command_component(init_ns: &str, init_def: &str) -> Result<(), String> {
  check_wasi_component(init_ns, init_def, WasiWorld::Command)
}

pub fn check_wasi_component(init_ns: &str, init_def: &str, world: WasiWorld) -> Result<(), String> {
  emit_wasm_impl(init_ns, init_def, "", WasmTarget::Wasi, WasmBoundary::Component, world, false)
}

fn emit_wasm_impl(
  init_ns: &str,
  init_def: &str,
  emit_path: &str,
  target: WasmTarget,
  boundary: WasmBoundary,
  world: WasiWorld,
  write_output: bool,
) -> Result<(), String> {
  let program_data = program::clone_compiled_program_snapshot()?;
  if target == WasmTarget::Wasi && world == WasiWorld::Http && boundary != WasmBoundary::Component {
    return Err("E_WASI_WORLD: `--world http` requires `--boundary component`; WASI Preview 1 has no HTTP proxy world".into());
  }
  validate_wasm_target_in_program(&program_data, init_ns, init_def, target, world)?;
  validate_stdin_boundary(&program_data, init_ns, init_def, target, boundary, world)?;
  let mut http_db_import_adapters = if target == WasmTarget::Wasi && boundary == WasmBoundary::Component {
    match world {
      WasiWorld::Command => {
        validate_wasm_command_component_in_program(&program_data, init_ns, init_def)?;
        Vec::new()
      }
      WasiWorld::Http => http_proxy::validate_wasi_http_component_in_program(&program_data, init_ns, init_def)?,
    }
  } else {
    Vec::new()
  };

  let wasi_command_component = target == WasmTarget::Wasi && boundary == WasmBoundary::Component && world == WasiWorld::Command;
  let wasi_http_proxy = target == WasmTarget::Wasi && boundary == WasmBoundary::Component && world == WasiWorld::Http;
  let wasi_http_stdio = wasi_http_proxy && reachable_wasi_effect(&program_data, init_ns, init_def, expr_uses_wasi_stdio);
  let wasi_command_file = wasi_command_component && reachable_wasi_effect(&program_data, init_ns, init_def, expr_uses_wasi_file);
  let wasi_command_read_dir =
    wasi_command_component && reachable_wasi_effect(&program_data, init_ns, init_def, expr_uses_wasi_read_dir);
  let wasi_command_fs_common = wasi_command_file || wasi_command_read_dir;
  let wasi_command_stdin = wasi_command_component && reachable_wasi_effect(&program_data, init_ns, init_def, expr_uses_wasi_stdin);
  let wasi_command_clocks = wasi_command_component && reachable_wasi_effect(&program_data, init_ns, init_def, expr_uses_wasi_clocks);
  let wasi_command_wait = wasi_command_component && reachable_wasi_effect(&program_data, init_ns, init_def, expr_uses_wasi_wait);
  let wasi_command_random = wasi_command_component && reachable_wasi_effect(&program_data, init_ns, init_def, expr_uses_wasi_random);

  let mut component_import_adapters = if boundary == WasmBoundary::Component && target == WasmTarget::Core {
    collect_component_import_adapters(&program_data)?
  } else if wasi_http_proxy {
    std::mem::take(&mut http_db_import_adapters)
  } else {
    Vec::new()
  };
  if wasi_command_component {
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

  let ReachableProgram {
    defs: reachable_defs,
    extra_exprs: reachable_extra_exprs,
  } = collect_reachable_program_defs(
    &program_data,
    ReachabilityConfig {
      init_ns,
      init_def,
      target,
      extra_imports: &component_import_adapters,
      wasi_command_component,
      wasi_http_proxy,
      wasi_command_read_dir,
    },
  );

  // First pass: extract reachable function signatures from all namespaces,
  // while counting pre-pruning fn_defs per bare name so Native export names
  // stay qualified if an uncalled same-named dependency exists.
  let mut fn_defs: Vec<(String, String, CalcitFnArgs, Vec<Calcit>)> = Vec::new(); // (ns, def_name, args, body)
  let mut native_export_name_counts: HashMap<String, usize> = HashMap::new();

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
        if reachable_defs.contains(&(ns.to_string(), def_name.to_string())) {
          parse_wasm_import_def(&compiled.preprocessed_code).ok_or_else(|| {
            format!("[wasm] invalid import declaration {ns}/{def_name}: expected `defwasm-import name (args) |module |field`")
          })?;
        }
        continue;
      }
      if !reachable_defs.contains(&(ns.to_string(), def_name.to_string())) {
        if extract_fn_parts(&compiled.preprocessed_code).is_ok() {
          *native_export_name_counts.entry(def_name.to_string()).or_insert(0) += 1;
        }
        continue;
      }
      match extract_fn_parts(&compiled.preprocessed_code) {
        Ok((args, body)) => {
          *native_export_name_counts.entry(def_name.to_string()).or_insert(0) += 1;
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
  let wasi_command_stdio = wasi_command_component
    && fn_defs.iter().any(|(namespace, name, _, body)| {
      (namespace != "calcit.core" || name.starts_with("&calcit:test:")) && body.iter().any(expr_uses_wasi_stdio)
    });
  let wasi_command_stackful = wasi_command_stdio || wasi_command_fs_common || wasi_command_stdin || wasi_command_wait;
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
    if wasi_command_component {
      host_imports.push(HostImport {
        module: "wasi:cli/exit@0.3.1".into(),
        name: "exit-with-code".into(),
        params: vec![ValType::I32],
        results: vec![],
      });
    } else if wasi_http_proxy {
      host_imports.extend(http_proxy::wasi_http_proxy_imports(wasi_http_stdio));
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
    if wasi_command_component {
      host_imports.push(HostImport {
        module: "wasi:cli/environment@0.3.1".into(),
        name: "get-environment".into(),
        params: vec![ValType::I32],
        results: vec![],
      });
      if wasi_command_file {
        host_imports.extend(wasi_component_file_imports());
      } else if wasi_command_read_dir {
        host_imports.extend(wasi_component_fs_common_imports());
      }
      if wasi_command_read_dir {
        host_imports.extend(wasi_component_read_dir_imports());
      }
      if wasi_command_stdin {
        host_imports.extend(wasi_component_stdin_imports());
      }
      if wasi_command_clocks {
        host_imports.push(HostImport {
          module: "wasi:clocks/monotonic-clock@0.3.1".into(),
          name: "now".into(),
          params: vec![],
          results: vec![ValType::I64],
        });
        host_imports.push(HostImport {
          module: "wasi:clocks/system-clock@0.3.1".into(),
          name: "now".into(),
          params: vec![ValType::I32],
          results: vec![],
        });
      }
      if wasi_command_wait {
        host_imports.push(HostImport {
          module: "wasi:clocks/monotonic-clock@0.3.1".into(),
          name: "[async-lower]wait-for".into(),
          params: vec![ValType::I64],
          results: vec![ValType::I32],
        });
      }
      if wasi_command_random {
        host_imports.push(HostImport {
          module: "wasi:random/random@0.3.1".into(),
          name: "get-random-bytes".into(),
          params: vec![ValType::I64, ValType::I32],
          results: vec![],
        });
      }
      if wasi_command_stdio {
        for interface in ["stdout", "stderr"] {
          let module = format!("wasi:cli/{interface}@0.3.1");
          for (name, params, results) in [
            ("[stream-new-0]write-via-stream", vec![], vec![ValType::I64]),
            ("[stream-write-0]write-via-stream", vec![ValType::I32; 3], vec![ValType::I32]),
            ("[stream-drop-writable-0]write-via-stream", vec![ValType::I32], vec![]),
            ("[future-read-1]write-via-stream", vec![ValType::I32; 2], vec![ValType::I32]),
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
      }
      if wasi_command_stackful {
        host_imports.push(HostImport {
          module: "[export]wasi:cli/run@0.3.1".into(),
          name: "[task-return]run".into(),
          params: vec![ValType::I32],
          results: vec![],
        });
      }
    }
    if wasi_command_fs_common
      || wasi_command_stdin
      || wasi_command_wait
      || component_import_adapters
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
  // Layout collection includes registered reachable definitions on every boundary, even
  // when their functions are not emitted. Their tags must use the same scope.
  let tag_index = collect_all_tags_from(&fn_defs, Some(&program_data), Some(&reachable_defs), &reachable_extra_exprs);
  if write_output {
    eprintln!("[wasm] tag index: {tag_index:?}");
  }

  // Collect top-level value defs and defref definitions before building the static pool
  // so both persistent initializers and Dual-Region memory layout are ready before runtime helpers.
  let mut value_imports: HashMap<String, Calcit> = HashMap::new();
  fn expr_evaluates_to_function(expr: &Calcit, program_data: &program::CompiledProgram, visited: &mut HashSet<String>) -> bool {
    let mut current = expr;
    while let Calcit::List(xs) = current
      && matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::AssertType, _)))
      && xs.len() >= 2
    {
      current = &xs[1];
    }
    match current {
      Calcit::Fn { .. } => true,
      Calcit::List(xs) if !xs.is_empty() => match &xs[0] {
        Calcit::Syntax(CalcitSyntax::Defn, _) => true,
        Calcit::Syntax(CalcitSyntax::CoreLet, _) => xs
          .iter()
          .last()
          .is_some_and(|tail| expr_evaluates_to_function(tail, program_data, visited)),
        Calcit::Syntax(CalcitSyntax::If, _) => {
          xs.get(2).is_some_and(|t| expr_evaluates_to_function(t, program_data, visited))
            || xs.get(3).is_some_and(|e| expr_evaluates_to_function(e, program_data, visited))
        }
        Calcit::Fn { info, .. } => info
          .body
          .last()
          .is_some_and(|tail| expr_evaluates_to_function(tail, program_data, visited)),
        Calcit::Import(imp) => {
          let q = format!("{}/{}", imp.ns, imp.def);
          if !visited.insert(q) {
            return false;
          }
          let Some(compiled) = program_data.get(imp.ns.as_ref()).and_then(|f| f.defs.get(imp.def.as_ref())) else {
            return false;
          };
          let schema = CalcitTypeAnnotation::expand_schema_alias(&compiled.schema);
          if matches!(
            schema.as_ref(),
            CalcitTypeAnnotation::Fn(sig) if matches!(CalcitTypeAnnotation::expand_schema_alias(&sig.return_type).as_ref(), CalcitTypeAnnotation::Fn(_))
          ) {
            return true;
          }
          if let Calcit::Fn { info, .. } = &compiled.preprocessed_code {
            return info
              .body
              .last()
              .is_some_and(|tail| expr_evaluates_to_function(tail, program_data, visited));
          }
          false
        }
        Calcit::Symbol { sym, info, .. } => {
          let q = format!("{}/{}", info.at_ns, sym);
          if !visited.insert(q) {
            return false;
          }
          let Some(compiled) = program_data.get(info.at_ns.as_ref()).and_then(|f| f.defs.get(sym.as_ref())) else {
            return false;
          };
          let schema = CalcitTypeAnnotation::expand_schema_alias(&compiled.schema);
          if matches!(
            schema.as_ref(),
            CalcitTypeAnnotation::Fn(sig) if matches!(CalcitTypeAnnotation::expand_schema_alias(&sig.return_type).as_ref(), CalcitTypeAnnotation::Fn(_))
          ) {
            return true;
          }
          if let Calcit::Fn { info, .. } = &compiled.preprocessed_code {
            return info
              .body
              .last()
              .is_some_and(|tail| expr_evaluates_to_function(tail, program_data, visited));
          }
          false
        }
        _ => false,
      },
      _ => false,
    }
  }

  let mut defref_defs: Vec<(String, Option<Calcit>)> = Vec::new();
  let mut defref_fn_valued: HashSet<String> = HashSet::new();
  let mut atom_is_scalar: HashMap<String, bool> = HashMap::new();
  let mut atom_is_scalar_vec: Vec<bool> = Vec::new();
  for ns in &ns_order {
    let Some(file_info) = program_data.get(ns.as_str()) else {
      continue;
    };
    for (def_name, compiled) in deterministic_definition_order(file_info) {
      if !reachable_defs.contains(&(ns.to_string(), def_name.to_string())) {
        continue;
      }
      let qualified = format!("{ns}/{def_name}");
      if matches!(compiled.kind, program::CompiledDefKind::Value | program::CompiledDefKind::LazyValue) {
        value_imports.insert(qualified.clone(), compiled.preprocessed_code.to_owned());
      }
      if let Calcit::List(xs) = &compiled.preprocessed_code
        && matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::Defref, _)))
      {
        let expanded_schema = CalcitTypeAnnotation::expand_schema_alias(&compiled.schema);
        let is_scalar = matches!(
          expanded_schema.as_ref(),
          CalcitTypeAnnotation::Ref(inner)
            if proven_scalar_kind(&CalcitTypeAnnotation::expand_schema_alias(inner)).is_some()
        );
        let schema_is_fn = matches!(
          expanded_schema.as_ref(),
          CalcitTypeAnnotation::Ref(inner)
            if matches!(CalcitTypeAnnotation::expand_schema_alias(inner).as_ref(), CalcitTypeAnnotation::Fn(_))
        );
        let init_expr = xs.get(2).cloned();
        let init_is_fn = schema_is_fn
          || init_expr
            .as_ref()
            .is_some_and(|expr| expr_evaluates_to_function(expr, &program_data, &mut HashSet::new()));
        if init_is_fn {
          defref_fn_valued.insert(qualified.clone());
        }
        atom_is_scalar.insert(qualified.clone(), is_scalar);
        atom_is_scalar_vec.push(is_scalar);
        defref_defs.push((qualified, init_expr));
      }
    }
  }
  let mut extra_pool_exprs: Vec<Calcit> = defref_defs.iter().filter_map(|(_, init)| init.clone()).collect();
  extra_pool_exprs.extend(reachable_extra_exprs.iter().cloned());
  let pool_options = detect_static_pool_options(
    &fn_defs,
    &value_imports,
    &extra_pool_exprs,
    target,
    wasi_http_proxy,
    wasi_command_stdio || wasi_http_stdio,
    wasi_command_read_dir,
  );
  let StaticDataAndAtoms {
    string_pool,
    mut string_data_segment,
    mut heap_start,
    mut atom_initial_values,
    atom_globals,
  } = build_static_data_and_atoms(
    &fn_defs,
    &value_imports,
    &tag_index,
    pool_options,
    &defref_defs,
    &reachable_extra_exprs,
  );
  let static_closure_singletons = build_static_closure_singletons(
    &fn_defs,
    &value_imports,
    &defref_defs,
    &defref_fn_valued,
    &reachable_extra_exprs,
    &wasm_import_names,
    &component_import_adapters,
    &tag_index,
    &mut string_data_segment,
    &mut heap_start,
    &mut atom_initial_values,
  );

  let component_types = if boundary == WasmBoundary::Component {
    collect_component_compound_types(&program_data, &fn_defs, &component_import_adapters)?
  } else {
    Vec::new()
  };
  let mut struct_layouts = collect_struct_layouts(
    &program_data,
    &fn_defs,
    &reachable_extra_exprs,
    &component_types,
    &tag_index,
    Some(&reachable_defs),
  )?;
  struct_layouts.serialize_descriptor_table(&mut string_data_segment, &mut heap_start);
  let mut enum_layouts = collect_enum_layouts(&program_data, &fn_defs, &reachable_extra_exprs, Some(&reachable_defs))?;
  enum_layouts.serialize_descriptor_table(&mut string_data_segment, &mut heap_start);
  let any_type_table = any_box::collect_any_type_table(&fn_defs, &value_imports, &reachable_extra_exprs);

  let atom_count = atom_initial_values.len() as u32;
  let memory_layout = WasmMemoryLayout::compute(heap_start, atom_count);
  let global_layout = WasmGlobalLayout::compute(atom_count, boundary == WasmBoundary::Component);
  let RuntimeFns {
    fns: mut compiled_fns,
    fn_index: mut runtime_fn_index,
    number_format_fn_count,
  } = build_runtime_fns(
    num_imports,
    &memory_layout,
    &global_layout,
    boundary,
    &tag_index,
    &atom_is_scalar_vec,
    (&struct_layouts, &enum_layouts),
  )?;
  if target == WasmTarget::Wasi && boundary == WasmBoundary::Native {
    let fd_write_idx = *index_host_imports(&host_imports)
      .get(&("wasi_snapshot_preview1".into(), "fd_write".into()))
      .expect("WASI fd_write import must be registered");
    let wasi_write_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_write_all".into(), wasi_write_idx);
    compiled_fns.push(build_wasi_write_all_fn(fd_write_idx));
  }

  let (component_cabi_free_index, component_cabi_realloc_index) = if let Some(comp) = global_layout.component {
    let free_index = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__cabi_free".into(), free_index);
    compiled_fns.push(build_cabi_free_fn(comp));
    let realloc_index = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__cabi_realloc".into(), realloc_index);
    compiled_fns.push(build_cabi_realloc_fn(free_index, comp));
    (Some(free_index), Some(realloc_index))
  } else {
    (None, None)
  };

  // Emit __str_new(src_ptr: i32, byte_len: i32) → f64 now that we know the string tag id.
  // This is a runtime helper exported for JS FFI: copies bytes into a tagged heap string.
  let str_tag_id = *tag_index.get("string").expect("string tag must exist") as i32;
  let arena_alloc_idx = *runtime_fn_index
    .get("__rt_arena_alloc")
    .expect("__rt_arena_alloc helper must exist");
  let str_new_idx = num_imports + compiled_fns.len() as u32;
  runtime_fn_index.insert("__str_new".to_string(), str_new_idx);
  compiled_fns.push(build_str_new_fn(str_tag_id, arena_alloc_idx));

  if wasi_command_fs_common || wasi_command_stdin || wasi_http_proxy {
    let utf8_valid_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_utf8_valid".into(), utf8_valid_idx);
    compiled_fns.push(build_utf8_valid_fn());
  }

  if wasi_command_fs_common {
    let utf8_valid_idx = *runtime_fn_index.get("__rt_utf8_valid").expect("UTF-8 validation helper must exist");
    let route_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_component_route_path".into(), route_idx);
    compiled_fns.push(build_wasi_component_route_path_fn());
    let import_indices = index_host_imports(&host_imports);
    let select_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_component_select_preopen".into(), select_idx);
    compiled_fns.push(build_wasi_component_select_preopen_fn(
      *import_indices
        .get(&("wasi:filesystem/preopens@0.3.1".into(), "get-directories".into()))
        .expect("WASI preopen import must be registered"),
      *import_indices
        .get(&("wasi:filesystem/types@0.3.1".into(), "[resource-drop]descriptor".into()))
        .expect("WASI descriptor drop import must be registered"),
      component_cabi_free_index.expect("Component boundary must install cabi_free"),
      route_idx,
    ));
    let open_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_component_open_at".into(), open_idx);
    let descriptor_module = "wasi:filesystem/types@0.3.1";
    compiled_fns.push(build_wasi_component_open_at_fn(
      select_idx,
      *import_indices
        .get(&(descriptor_module.into(), "[async-lower][method]descriptor.open-at".into()))
        .expect("WASI open-at import must be registered"),
      *import_indices
        .get(&(descriptor_module.into(), "[resource-drop]descriptor".into()))
        .expect("WASI descriptor drop import must be registered"),
      component_cabi_free_index.expect("Component boundary must install cabi_free"),
      component_cabi_realloc_index.expect("Component boundary must install cabi_realloc"),
      component_async_canonical_imports
        .as_ref()
        .expect("WASI async file operations must register canonical lifecycle imports"),
    ));
    let descriptor_import = |name: &str| {
      *import_indices
        .get(&(descriptor_module.into(), name.into()))
        .unwrap_or_else(|| panic!("WASI descriptor import {name} must be registered"))
    };
    if wasi_command_file {
      let read_idx = num_imports + compiled_fns.len() as u32;
      runtime_fn_index.insert("__rt_wasi_component_read_bytes".into(), read_idx);
      compiled_fns.push(build_wasi_component_read_bytes_fn(
        WasiComponentReadImports {
          source: runtime::WasiComponentReadSource::File {
            open: open_idx,
            descriptor_drop: descriptor_import("[resource-drop]descriptor"),
          },
          read_via_stream: descriptor_import("[method]descriptor.read-via-stream"),
          stream_read: descriptor_import("[async-lower][stream-read-0][method]descriptor.read-via-stream"),
          stream_drop: descriptor_import("[stream-drop-readable-0][method]descriptor.read-via-stream"),
          future_read: descriptor_import("[future-read-1][method]descriptor.read-via-stream"),
          future_drop: descriptor_import("[future-drop-readable-1][method]descriptor.read-via-stream"),
          free: component_cabi_free_index.expect("Component boundary must install cabi_free"),
          realloc: component_cabi_realloc_index.expect("Component boundary must install cabi_realloc"),
        },
        component_async_canonical_imports
          .as_ref()
          .expect("WASI async file operations must register canonical lifecycle imports"),
      ));
      let write_idx = num_imports + compiled_fns.len() as u32;
      runtime_fn_index.insert("__rt_wasi_component_write_bytes".into(), write_idx);
      compiled_fns.push(build_wasi_component_write_bytes_fn(
        WasiComponentWriteImports {
          open: open_idx,
          stream_new: descriptor_import("[stream-new-0][method]descriptor.write-via-stream"),
          stream_write: descriptor_import("[stream-write-0][method]descriptor.write-via-stream"),
          stream_drop: descriptor_import("[stream-drop-writable-0][method]descriptor.write-via-stream"),
          write_via_stream: descriptor_import("[method]descriptor.write-via-stream"),
          future_read: descriptor_import("[future-read-1][method]descriptor.write-via-stream"),
          future_drop: descriptor_import("[future-drop-readable-1][method]descriptor.write-via-stream"),
          descriptor_drop: descriptor_import("[resource-drop]descriptor"),
          free: component_cabi_free_index.expect("Component boundary must install cabi_free"),
          realloc: component_cabi_realloc_index.expect("Component boundary must install cabi_realloc"),
        },
        component_async_canonical_imports
          .as_ref()
          .expect("WASI async file operations must register canonical lifecycle imports"),
      ));
    }
    if wasi_command_read_dir {
      let read_dir_idx = num_imports + compiled_fns.len() as u32;
      runtime_fn_index.insert("__rt_wasi_read_dir".into(), read_dir_idx);
      compiled_fns.push(build_wasi_component_read_dir_fn(
        WasiComponentReadDirImports {
          open: open_idx,
          read_directory: descriptor_import("[method]descriptor.read-directory"),
          stream_read: descriptor_import("[async-lower][stream-read-0][method]descriptor.read-directory"),
          stream_drop: descriptor_import("[stream-drop-readable-0][method]descriptor.read-directory"),
          future_read: descriptor_import("[future-read-1][method]descriptor.read-directory"),
          future_drop: descriptor_import("[future-drop-readable-1][method]descriptor.read-directory"),
          descriptor_drop: descriptor_import("[resource-drop]descriptor"),
          utf8_valid: utf8_valid_idx,
          str_compare: *runtime_fn_index
            .get("__rt_str_compare")
            .expect("string comparison helper must exist"),
          free: component_cabi_free_index.expect("Component boundary must install cabi_free"),
          realloc: component_cabi_realloc_index.expect("Component boundary must install cabi_realloc"),
          list_tag: *tag_index.get("list").expect("list tag must exist") as i32,
          string_tag: str_tag_id,
        },
        component_async_canonical_imports
          .as_ref()
          .expect("WASI async file operations must register canonical lifecycle imports"),
      ));
    }
  }
  if wasi_command_wait {
    let wait_for_idx = *index_host_imports(&host_imports)
      .get(&("wasi:clocks/monotonic-clock@0.3.1".into(), "[async-lower]wait-for".into()))
      .expect("WASI 0.3 wait-for import must be registered");
    let wait_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_wait".into(), wait_idx);
    compiled_fns.push(build_wasi_component_wait_fn(
      wait_for_idx,
      component_cabi_realloc_index.expect("Component boundary must install cabi_realloc"),
      component_cabi_free_index.expect("Component boundary must install cabi_free"),
      component_async_canonical_imports
        .as_ref()
        .expect("WASI async wait operations must register canonical lifecycle imports"),
    ));
  }
  if wasi_command_random {
    let get_random_bytes_idx = *index_host_imports(&host_imports)
      .get(&("wasi:random/random@0.3.1".into(), "get-random-bytes".into()))
      .expect("WASI 0.3 random import must be registered");
    let random_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_component_random_get".into(), random_idx);
    compiled_fns.push(build_wasi_component_random_get_fn(
      get_random_bytes_idx,
      component_cabi_realloc_index.expect("Component boundary must install cabi_realloc"),
      component_cabi_free_index.expect("Component boundary must install cabi_free"),
    ));
  }

  if wasi_command_component {
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

  let (wasi_http_write_all_idx, wasi_http_send_plain_idx) = if wasi_http_proxy {
    let import_indices = index_host_imports(&host_imports);
    let write_all_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_http_write_all".into(), write_all_idx);
    compiled_fns.push(http_proxy::build_wasi_http_write_all_fn(&import_indices));
    let send_plain_idx = num_imports + compiled_fns.len() as u32;
    runtime_fn_index.insert("__rt_wasi_http_send_plain".into(), send_plain_idx);
    compiled_fns.push(http_proxy::build_wasi_http_send_plain_fn(
      &import_indices,
      &string_pool,
      write_all_idx,
    ));
    (Some(write_all_idx), Some(send_plain_idx))
  } else {
    (None, None)
  };

  if wasi_command_stdin {
    let indices = index_host_imports(&host_imports);
    let import = |name: &str| indices[&("wasi:cli/stdin@0.3.1".into(), name.into())];
    runtime_fn_index.insert("__rt_wasi_component_stdin_bytes".into(), num_imports + compiled_fns.len() as u32);
    compiled_fns.push(build_wasi_component_read_bytes_fn(
      WasiComponentReadImports {
        source: runtime::WasiComponentReadSource::Stdin,
        read_via_stream: import("read-via-stream"),
        stream_read: import("[async-lower][stream-read-0]read-via-stream"),
        stream_drop: import("[stream-drop-readable-0]read-via-stream"),
        future_read: import("[future-read-1]read-via-stream"),
        future_drop: import("[future-drop-readable-1]read-via-stream"),
        free: component_cabi_free_index.expect("stdin needs cabi_free"),
        realloc: component_cabi_realloc_index.expect("stdin needs cabi_realloc"),
      },
      component_async_canonical_imports
        .as_ref()
        .expect("stdin needs canonical lifecycle imports"),
    ));
  }

  let component_buffer_new_index = if boundary == WasmBoundary::Component {
    let buffer_tag_id = *tag_index.get("buffer").expect("buffer tag must exist") as i32;
    let index = num_imports + compiled_fns.len() as u32;
    compiled_fns.push(build_component_buffer_new_fn(buffer_tag_id, arena_alloc_idx));
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
    let enum_load_payload_index = *runtime_fn_index
      .get("__rt_enum_load_payload")
      .expect("__rt_enum_load_payload helper must exist");
    let option_from_niche_ptr_index = *runtime_fn_index
      .get("__rt_option_from_niche_ptr")
      .expect("__rt_option_from_niche_ptr helper must exist");
    let option_to_niche_ptr_index = *runtime_fn_index
      .get("__rt_option_to_niche_ptr")
      .expect("__rt_option_to_niche_ptr helper must exist");
    let enum_tag_id = *tag_index.get("enum").expect("enum tag must exist") as i32;
    for compound_type in component_types {
      let lift_index = num_imports + compiled_fns.len() as u32;
      let lower_index = lift_index + 1;
      match &compound_type {
        ComponentAbiType::List(_) => {
          component_list_codecs.insert(compound_type.clone(), ComponentListCodec { lift_index, lower_index });
          let codecs = ComponentValueCodecs {
            str_new_index: str_new_idx,
            buffer_new_index,
            arena_alloc_index: arena_alloc_idx,
            cabi_realloc_index,
            enum_load_payload_index,
            option_from_niche_ptr_index,
            option_to_niche_ptr_index,
            enum_layouts: &enum_layouts,
            list_codecs: &component_list_codecs,
            struct_codecs: &component_struct_codecs,
            variant_codecs: &component_variant_codecs,
          };
          compiled_fns.push(build_component_list_lift_fn(&compound_type, list_tag_id, &codecs));
          compiled_fns.push(build_component_list_lower_fn(&compound_type, cabi_realloc_index, &codecs));
        }
        ComponentAbiType::Struct(record) => {
          let nominal_tag_id = *struct_layouts
            .ids
            .get(&component_struct_identity(record))
            .ok_or_else(|| format!("E_COMPONENT_ABI_STRUCT_TAG: `{}` has no registered identity", record.id))?
            as i32;
          let struct_layout = struct_layouts.layout_by_id(nominal_tag_id as u32)?;
          component_struct_codecs.insert(compound_type.clone(), ComponentStructCodec { lift_index, lower_index });
          let codecs = ComponentValueCodecs {
            str_new_index: str_new_idx,
            buffer_new_index,
            arena_alloc_index: arena_alloc_idx,
            cabi_realloc_index,
            enum_load_payload_index,
            option_from_niche_ptr_index,
            option_to_niche_ptr_index,
            enum_layouts: &enum_layouts,
            list_codecs: &component_list_codecs,
            struct_codecs: &component_struct_codecs,
            variant_codecs: &component_variant_codecs,
          };
          compiled_fns.push(build_component_struct_lift_fn(
            &compound_type,
            nominal_tag_id,
            *tag_index.get("struct").expect("struct tag must exist") as i32,
            struct_layout,
            &codecs,
          ));
          compiled_fns.push(build_component_struct_lower_fn(
            &compound_type,
            nominal_tag_id,
            struct_layout,
            &codecs,
          ));
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
            arena_alloc_index: arena_alloc_idx,
            cabi_realloc_index,
            enum_load_payload_index,
            option_from_niche_ptr_index,
            option_to_niche_ptr_index,
            enum_layouts: &enum_layouts,
            list_codecs: &component_list_codecs,
            struct_codecs: &component_struct_codecs,
            variant_codecs: &component_variant_codecs,
          };
          compiled_fns.push(build_component_variant_lift_fn(&compound_type, tag_ids, enum_tag_id, &codecs));
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
            arena_alloc_index: arena_alloc_idx,
            cabi_realloc_index,
            enum_load_payload_index,
            option_from_niche_ptr_index,
            option_to_niche_ptr_index,
            enum_layouts: &enum_layouts,
            list_codecs: &component_list_codecs,
            struct_codecs: &component_struct_codecs,
            variant_codecs: &component_variant_codecs,
          };
          compiled_fns.push(build_component_enum_lift_fn(&compound_type, &tag_ids, enum_tag_id, &codecs));
          compiled_fns.push(build_component_enum_lower_fn(&compound_type, &tag_ids, &codecs));
        }
        _ => unreachable!("compound type collector returned a scalar type"),
      }
    }
  }
  let mut component_drop_indices = BTreeMap::new();
  if let Some(cabi_free_index) = component_cabi_free_index {
    for owned_type in collect_component_owned_types(&component_adapters, &component_import_adapters) {
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
      arena_alloc_index: arena_alloc_idx,
      cabi_realloc_index,
      enum_load_payload_index: *runtime_fn_index
        .get("__rt_enum_load_payload")
        .expect("__rt_enum_load_payload helper must exist"),
      option_from_niche_ptr_index: *runtime_fn_index
        .get("__rt_option_from_niche_ptr")
        .expect("__rt_option_from_niche_ptr helper must exist"),
      option_to_niche_ptr_index: *runtime_fn_index
        .get("__rt_option_to_niche_ptr")
        .expect("__rt_option_to_niche_ptr helper must exist"),
      enum_layouts: &enum_layouts,
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
        &component_drop_indices,
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

  let mut host_imports_map = index_host_imports(&host_imports);
  if boundary == WasmBoundary::Native
    && let Some(native) = global_layout.native
  {
    let wrap_raw_indices: std::collections::BTreeSet<u32> = wasm_import_names.values().copied().collect();
    let mut raw_to_wrapper: HashMap<u32, u32> = HashMap::new();
    for raw_idx in wrap_raw_indices {
      let imp = &host_imports[raw_idx as usize];
      let wrapper_idx = num_imports + compiled_fns.len() as u32;
      compiled_fns.push(build_native_host_import_wrapper(
        &imp.params,
        &imp.results,
        raw_idx,
        native.host_call_depth,
      ));
      raw_to_wrapper.insert(raw_idx, wrapper_idx);
    }
    for idx in wasm_import_names.values_mut() {
      if let Some(&wrapper_idx) = raw_to_wrapper.get(idx) {
        *idx = wrapper_idx;
      }
    }
    for idx in host_imports_map.values_mut() {
      if let Some(&wrapper_idx) = raw_to_wrapper.get(idx) {
        *idx = wrapper_idx;
      }
    }
  }

  let runtime_fn_count = compiled_fns.len() as u32;
  let mut export_name_counts: HashMap<String, usize> = HashMap::new();
  for (_, name, _, _) in &fn_defs {
    *export_name_counts.entry(name.clone()).or_insert(0) += 1;
  }
  let nil_sensitive_defs = collect_sensitive_defs(&fn_defs, &export_name_counts, |_, _, _, expr| direct_scalar_type_test(expr));
  let trait_sensitive_defs = collect_sensitive_defs(&fn_defs, &export_name_counts, |ns, name, args, expr| {
    let params_and_types = program_data
      .get(ns)
      .and_then(|file| file.defs.get(name))
      .and_then(|compiled| qualified_fn_schema(&compiled.schema, ns))
      .map(|sig| sig.arg_types.clone());
    expr_uses_trait_call_for_def(expr, params_and_types.as_deref(), Some(args))
  });
  let method_sensitive_defs = collect_sensitive_defs(&fn_defs, &export_name_counts, |_, _, _, expr| expr_uses_method_call(expr));
  let mut fn_index: HashMap<String, u32> = HashMap::new();
  let mut fn_arity: HashMap<String, u32> = HashMap::new();
  // Track functions with rest args: value is the fixed-arity count (params before `&`).
  // WASM arity for such functions is `fixed_arity + 1` (the rest list pointer).
  let mut fn_has_rest: HashMap<String, u32> = HashMap::new();
  for (name, index) in &wasm_import_names {
    fn_index.insert(name.clone(), *index);
    fn_arity.insert(
      name.clone(),
      *wasm_import_arities.get(name).expect("WASM import arity must be registered"),
    );
  }
  let has_boxed_json_scalars = program_has_boxed_json_scalars(&fn_defs, &value_imports);
  let component_handlers: HashSet<String> = component_adapters
    .iter()
    .flat_map(|adapter| {
      std::iter::once(adapter.definition.clone()).chain(adapter.stream_consumer.iter().map(|c| c.handler_definition.clone()))
    })
    .collect();
  let mut fn_signatures: HashMap<u32, InternalFnSig> = HashMap::new();
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

    let compiled_def = program_data.get(ns.as_str()).and_then(|file| file.defs.get(name.as_str()));
    let explicit_export = compiled_def.is_some_and(|compiled| is_wasm_export_def(&compiled.preprocessed_code));
    let is_custom_core = ns == "calcit.core"
      && matches!(
        name.as_str(),
        "repeat" | "interleave" | "zipmap" | "intersperse" | "join-string" | "try-parse-json" | "json:parse" | "keys-non-nil"
      );
    let is_wasi_init = target == WasmTarget::Wasi && ns == init_ns && name == init_def;
    if matches!(args, CalcitFnArgs::Args(_))
      && !explicit_export
      && !is_custom_core
      && !is_wasi_init
      && !component_handlers.contains(&qualified)
      && let Some(sig) = compiled_def.and_then(|compiled| qualified_fn_schema(&compiled.schema, ns))
      && sig.rest_type.is_none()
      && sig.arg_types.len() == arity as usize
    {
      let native_sig = InternalFnSig {
        params: sig.arg_types.iter().map(NativeValRepr::from_type_annotation).collect(),
        result: NativeValRepr::from_type_annotation(&sig.return_type),
      };
      if native_sig.has_native_slots() {
        fn_signatures.insert(idx, native_sig);
      }
    }
  }

  let mut static_fn_defs: HashMap<String, Arc<StaticFnDef>> = HashMap::new();
  for (ns, name, args, body) in &fn_defs {
    let Some(signature) = program_data
      .get(ns.as_str())
      .and_then(|file| file.defs.get(name.as_str()))
      .and_then(|compiled| qualified_fn_schema(&compiled.schema, ns))
    else {
      continue;
    };
    let params = fn_param_names(args);
    let callback_arities = signature
      .arg_types
      .iter()
      .enumerate()
      .filter_map(|(index, annotation)| {
        annotation
          .resolve_to_fn()
          .filter(|callback| callback.rest_type.is_none())
          .map(|callback| (index, callback.arg_types.len()))
          .or_else(|| {
            if nil_type_evidence_is_open(annotation) {
              params
                .get(index)
                .and_then(|param| infer_param_call_arity(param, body))
                .map(|arity| (index, arity))
            } else {
              None
            }
          })
      })
      .collect::<HashMap<_, _>>();
    let qualified = format!("{ns}/{name}");
    let definition = Arc::new(StaticFnDef {
      params,
      body: body.clone(),
      arg_types: signature.arg_types.clone(),
      callback_arities,
      fixed_arity: matches!(args, CalcitFnArgs::Args(_)),
      nil_sensitive: nil_sensitive_defs.contains(&qualified),
      trait_sensitive: trait_sensitive_defs.contains(&qualified),
      method_sensitive: method_sensitive_defs.contains(&qualified),
    });
    static_fn_defs.insert(qualified, definition.clone());
    if export_name_counts.get(name).copied() == Some(1) {
      static_fn_defs.insert(name.clone(), definition);
    }
  }

  let non_coherent_nominal_defs = Arc::new(program::snapshot_non_coherent_nominal_defs());
  let dyn_base_fn_idx = num_imports + runtime_fn_count + fn_defs.len() as u32;
  let next_dyn_fn_idx = Rc::new(RefCell::new(dyn_base_fn_idx));
  let mut closure_registry = closure::ClosureRegistry::new(next_dyn_fn_idx.clone());
  let mut fn_schemas = HashMap::new();
  for (ns, name, _, _) in &fn_defs {
    if let Some(sig) = program_data
      .get(ns.as_str())
      .and_then(|file| file.defs.get(name.as_str()))
      .and_then(|compiled| qualified_fn_schema(&compiled.schema, ns))
    {
      fn_schemas.insert(format!("{ns}/{name}"), sig);
    }
  }
  closure_registry.register_static_singletons(
    &static_closure_singletons,
    &fn_index,
    &fn_arity,
    &fn_has_rest,
    &fn_defs,
    &fn_schemas,
  );
  let env = WasmCompileEnv {
    function_value_dependencies: Rc::default(),
    dyn_traits: Rc::new(RefCell::new(dyn_trait::DynTraitRegistry::new_shared(next_dyn_fn_idx.clone()))),
    closures: Rc::new(RefCell::new(closure_registry)),
    fn_index,
    fn_arity,
    fn_has_rest,
    fn_signatures,
    has_boxed_json_scalars,
    runtime_fn_index,
    tag_index,
    struct_layouts,
    enum_layouts,
    non_coherent_nominal_defs,
    string_pool,
    atom_globals,
    atom_is_scalar,
    value_imports,
    static_fn_defs,
    host_imports: host_imports_map,
    target,
    boundary,
    world,
    any_type_table,
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
  struct NativeExportWrapperSpec {
    export_name: String,
    target_idx: u32,
    arity: usize,
    returns_proven_scalar: bool,
    all_scalar_params: bool,
  }
  let mut native_export_wrappers: Vec<NativeExportWrapperSpec> = Vec::new();
  // Second pass: target failures reject the artifact. Dependency failures keep
  // a trapping slot so preassigned call and table indices remain stable.
  for (ns, def_name, args, body) in &fn_defs {
    let function_signature = program_data
      .get(ns.as_str())
      .and_then(|file| file.defs.get(def_name.as_str()))
      .and_then(|compiled| qualified_fn_schema(&compiled.schema, ns));
    let explicit_export = program_data
      .get(ns.as_str())
      .and_then(|file| file.defs.get(def_name.as_str()))
      .is_some_and(|compiled| is_wasm_export_def(&compiled.preprocessed_code));
    let export_name = if native_export_name_counts.get(def_name).copied().unwrap_or(0) > 1 {
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
        .unwrap_or_else(|| compile_fn(ns, &export_name, args, body, function_signature.as_deref(), &env))
    };
    match result {
      Ok(mut func) => {
        // Ordinary `defn` values stay callable inside the module but are not part
        // of the public host ABI. Only explicit `defwasm-export` declarations
        // (and reserved runtime helpers emitted separately) reach the export
        // surface. Component adapters replace the explicit declaration itself,
        // while Native exports are wrapped by thin turn-reset epilogues below.
        if !explicit_export {
          func.export_name = None;
        }
        if boundary == WasmBoundary::Component && explicit_export {
          func.export_name = None;
        }
        if boundary == WasmBoundary::Native
          && explicit_export
          && let Some(exported) = func.export_name.take()
        {
          let arity = func.params.len();
          let returns_proven_scalar = function_signature
            .as_deref()
            .is_some_and(|sig| proven_scalar_kind(&CalcitTypeAnnotation::expand_schema_alias(&sig.return_type)).is_some());
          let all_scalar_params = arity == 0
            || function_signature.as_deref().is_some_and(|sig| {
              sig.rest_type.is_none()
                && sig.arg_types.len() == arity
                && sig
                  .arg_types
                  .iter()
                  .all(|ty| proven_scalar_kind(&CalcitTypeAnnotation::expand_schema_alias(ty)).is_some())
            });
          native_export_wrappers.push(NativeExportWrapperSpec {
            export_name: exported,
            target_idx: num_imports + compiled_fns.len() as u32,
            arity,
            returns_proven_scalar,
            all_scalar_params,
          });
        }
        compiled_fns.push(func);
      }
      Err(e) => {
        let qualified = format!("{ns}/{def_name}");
        let deferred_nil_specialization =
          (e.contains("E_WASM_NIL_TYPE_EVIDENCE") || e.contains("E_WASM_SET_ELEMENT_KIND") || e.contains("E_WASM_EDN_TYPE"))
            && !explicit_export
            && env
              .static_fn_defs
              .get(&qualified)
              .is_some_and(|definition| definition_requires_nil_specialization(definition));
        let deferred_trait_specialization = e.contains("E_WASM_TRAIT_TYPE_EVIDENCE")
          && !explicit_export
          && env
            .static_fn_defs
            .get(&qualified)
            .is_some_and(|definition| definition_requires_trait_specialization(definition));
        let deferred_wasi_capability = target == WasmTarget::Wasi
          && boundary == WasmBoundary::Component
          && (e.starts_with("E_WASI_COMMAND_CAPABILITY:") || e.starts_with("E_WASI_HTTP_CAPABILITY:"));
        if explicit_export
          || (ns == init_ns
            && (def_name == init_def || !(deferred_nil_specialization || deferred_trait_specialization || deferred_wasi_capability)))
        {
          return Err(format!("[wasm] target function {ns}/{def_name} is not compilable: {e}"));
        }
        unsupported_dependencies.insert(num_imports + compiled_fns.len() as u32, (format!("{ns}/{def_name}"), e.clone()));
        if write_output {
          eprintln!("[wasm] trapping unsupported dependency {ns}/{def_name}: {e}");
        }
        let (arity, _) = compute_fn_arity(args);
        let fallback_idx = num_imports + compiled_fns.len() as u32;
        let fallback_sig = env.fn_signatures.get(&fallback_idx);
        compiled_fns.push(CompiledFn {
          export_name: None,
          params: fallback_sig.map_or_else(|| vec![ValType::F64; arity as usize], InternalFnSig::wasm_params),
          results: vec![fallback_sig.map_or(ValType::F64, |s| s.result.val_type())],
          locals: vec![],
          instructions: vec![Instruction::Unreachable],
        });
      }
    }
  }

  {
    let mut dyn_slots = Vec::new();
    let mut dyn_counts = Vec::new();
    let mut closure_bodies = HashMap::new();
    let mut closure_trampolines = Vec::new();
    loop {
      let dyn_changed = dyn_trait::compile_dyn_trampolines_step(&env, &mut dyn_slots, &mut dyn_counts);
      let closure_changed = closure::compile_closures_step(
        &env,
        &compiled_fns,
        num_imports,
        &dyn_slots,
        &mut closure_bodies,
        &mut closure_trampolines,
        &mut unsupported_dependencies,
      );
      if !dyn_changed && !closure_changed {
        break;
      }
    }
    let dynamic_fn_count = (*next_dyn_fn_idx.borrow() - dyn_base_fn_idx) as usize;
    let mut dynamic_fns: Vec<Option<CompiledFn>> = vec![None; dynamic_fn_count];
    dyn_trait::finalize_dyn_trampolines(&env, dyn_slots, dyn_base_fn_idx, &mut dynamic_fns, &mut unsupported_dependencies);
    closure::finalize_closures(
      closure_bodies,
      closure_trampolines,
      dyn_base_fn_idx,
      &mut dynamic_fns,
      &mut unsupported_dependencies,
    );
    for (offset, func) in dynamic_fns.into_iter().enumerate() {
      compiled_fns.push(func.unwrap_or_else(|| panic!("dynamic function slot {} was not compiled", dyn_base_fn_idx + offset as u32)));
    }
  }

  if boundary == WasmBoundary::Native && !native_export_wrappers.is_empty() {
    let native = global_layout.native.expect("native turn globals must exist on Native boundary");
    let turn_enter_idx = *env
      .runtime_fn_index
      .get("__rt_turn_enter")
      .expect("__rt_turn_enter helper must exist");
    let turn_end_idx = *env.runtime_fn_index.get("__rt_turn_end").expect("__rt_turn_end helper must exist");
    for spec in native_export_wrappers {
      let mark = spec.arity as u32;
      let saved_turns = mark + 1;
      let mut instructions = vec![
        Instruction::GlobalGet(native.host_call_depth),
        Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)),
        Instruction::GlobalGet(global_layout.active_turns),
        Instruction::Else,
        Instruction::I32Const(0),
        Instruction::End,
        Instruction::LocalSet(saved_turns),
        Instruction::I32Const(if spec.all_scalar_params { 1 } else { 0 }),
        Instruction::Call(turn_enter_idx),
        Instruction::LocalSet(mark),
        Instruction::LocalGet(mark),
        Instruction::LocalGet(saved_turns),
        Instruction::I32Const(if spec.returns_proven_scalar { 0 } else { 1 }),
      ];
      for param_idx in 0..spec.arity {
        instructions.push(Instruction::LocalGet(param_idx as u32));
      }
      instructions.push(Instruction::Call(spec.target_idx));
      instructions.push(Instruction::Call(turn_end_idx));
      compiled_fns.push(CompiledFn {
        export_name: Some(spec.export_name),
        params: vec![ValType::F64; spec.arity],
        results: vec![ValType::F64],
        locals: vec![ValType::I32, ValType::I32],
        instructions,
      });
    }
  }

  if boundary == WasmBoundary::Component {
    let cabi_realloc_index = component_cabi_realloc_index.expect("Component boundary must install cabi_realloc");
    let buffer_new_index = component_buffer_new_index.expect("Component boundary must install the Buffer constructor");
    let turn_enter_idx = *env
      .runtime_fn_index
      .get("__rt_component_turn_enter")
      .expect("__rt_component_turn_enter helper must exist");
    let rewind_to_mark_idx = *env
      .runtime_fn_index
      .get("__rt_component_rewind_to_mark")
      .expect("__rt_component_rewind_to_mark helper must exist");
    let turn_exit_idx = *env
      .runtime_fn_index
      .get("__rt_component_turn_exit")
      .expect("__rt_component_turn_exit helper must exist");
    let export_runtime = ComponentExportRuntime {
      codecs: ComponentValueCodecs {
        str_new_index: str_new_idx,
        buffer_new_index,
        arena_alloc_index: arena_alloc_idx,
        cabi_realloc_index,
        enum_load_payload_index: *env
          .runtime_fn_index
          .get("__rt_enum_load_payload")
          .expect("__rt_enum_load_payload helper must exist"),
        option_from_niche_ptr_index: *env
          .runtime_fn_index
          .get("__rt_option_from_niche_ptr")
          .expect("__rt_option_from_niche_ptr helper must exist"),
        option_to_niche_ptr_index: *env
          .runtime_fn_index
          .get("__rt_option_to_niche_ptr")
          .expect("__rt_option_to_niche_ptr helper must exist"),
        enum_layouts: &env.enum_layouts,
        list_codecs: &component_list_codecs,
        struct_codecs: &component_struct_codecs,
        variant_codecs: &component_variant_codecs,
      },
      cabi_free_index: component_cabi_free_index.expect("Component export adapter must install cabi_free"),
      drop_indices: &component_drop_indices,
      turn_enter_index: turn_enter_idx,
      turn_exit_index: turn_exit_idx,
      rewind_to_mark_index: rewind_to_mark_idx,
      comp_globals: global_layout.component.expect("component globals must exist"),
      active_turns_global: global_layout.active_turns,
      pers_needs_compact_global: global_layout.persistent.map(|pers| pers.needs_compact),
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
          &global_layout,
          turn_enter_idx,
          turn_exit_idx,
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
          turn_enter_idx,
          turn_exit_idx,
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
          turn_exit_idx,
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
      .ok_or_else(|| format!("E_WASM_TARGET: WASI entry `{qualified_init}` was not compiled"))?;
    if boundary == WasmBoundary::Component {
      reject_reachable_wasm_dependencies(
        &compiled_fns,
        num_imports,
        init_index,
        &unsupported_dependencies,
        &env.function_value_dependencies.borrow(),
        true,
      )?;
    }
    if wasi_http_proxy {
      let layout_info = http_proxy::resolve_wasi_http_layout_info(&program_data, &env.struct_layouts)?;
      let comp_globals = global_layout
        .component
        .expect("WASI HTTP proxy Component boundary must install Component memory globals");
      compiled_fns.push(http_proxy::build_wasi_http_incoming_handler_fn(
        init_index,
        wasi_http_write_all_idx.expect("WASI HTTP proxy must emit write-all helper"),
        wasi_http_send_plain_idx.expect("WASI HTTP proxy must emit send-plain helper"),
        &layout_info,
        &env.host_imports,
        &env.runtime_fn_index,
        &env.tag_index,
        &env.string_pool,
        global_layout.active_turns,
        comp_globals,
      ));
    } else {
      let init_arity = env.fn_arity.get(&qualified_init).copied().unwrap_or(0);
      if init_arity != 0 {
        return Err(format!(
          "E_WASM_TARGET: WASI command entry `{qualified_init}` must take no arguments, got {init_arity}"
        ));
      }
      let component_command = boundary == WasmBoundary::Component;
      compiled_fns.push(CompiledFn {
        export_name: Some(if component_command && wasi_command_stackful {
          "[async-lift-stackful]wasi:cli/run@0.3.1#run".into()
        } else if component_command {
          "wasi:cli/run@0.3.1#run".into()
        } else {
          "_start".into()
        }),
        params: vec![],
        results: if component_command && !wasi_command_stackful {
          vec![ValType::I32]
        } else {
          vec![]
        },
        locals: vec![],
        instructions: if component_command && wasi_command_stackful {
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
    if boundary == WasmBoundary::Component {
      prune_unreachable_wasi_component_fns(&mut compiled_fns, num_imports, init_index);
    }
  }

  if compiled_fns.is_empty() {
    return Err("no functions could be compiled to WASM".into());
  }

  // Validate actual emitted calls from the configured entry and public exports.
  // Unreferenced dependency slots may remain traps without rejecting the target.
  if let Some(&entry) = env.fn_index.get(&format!("{init_ns}/{init_def}")) {
    reject_reachable_wasm_dependencies(
      &compiled_fns,
      num_imports,
      entry,
      &unsupported_dependencies,
      &env.function_value_dependencies.borrow(),
      false,
    )
    .map_err(|reason| format!("{reason} (entry `{init_ns}/{init_def}`)"))?;
  }
  for (index, function) in compiled_fns.iter().enumerate() {
    if function.export_name.is_some() {
      reject_reachable_wasm_dependencies(
        &compiled_fns,
        num_imports,
        num_imports + index as u32,
        &unsupported_dependencies,
        &env.function_value_dependencies.borrow(),
        false,
      )
      .map_err(|reason| format!("{reason} (export `{}`)", function.export_name.as_deref().unwrap_or_default()))?;
    }
  }

  debug_assert!(
    !compiled_fns
      .iter()
      .any(|f| f.instructions.iter().any(|i| matches!(i, Instruction::CallIndirect { .. }))),
    "emitted WASM module must not contain CallIndirect instructions"
  );
  let rest_adapters = HashMap::new();

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
      number_format_fn_count,
      table_fn_count: fn_defs.len() as u32,
      memory_layout,
      global_layout,
      rest_adapters,
      prune_unused_imports: true,
    },
  )?;

  let wasm_bytes = if target == WasmTarget::Wasi && boundary == WasmBoundary::Component {
    match world {
      WasiWorld::Command => calcit_bindgen::package_wasi_command(&wasm_bytes)?,
      WasiWorld::Http => {
        if component_import_adapters.is_empty() {
          calcit_bindgen::package_wasi_http_proxy(&wasm_bytes)?
        } else {
          calcit_bindgen::package_wasi_http_service(&wasm_bytes)?
        }
      }
    }
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
  validate_wasm_target_with_world(init_ns, init_def, target, WasiWorld::Command)
}

pub fn validate_wasm_target_with_world(init_ns: &str, init_def: &str, target: WasmTarget, world: WasiWorld) -> Result<(), String> {
  let program_data = program::clone_compiled_program_snapshot()?;
  validate_wasm_target_in_program(&program_data, init_ns, init_def, target, world)
}

pub fn validate_wasm_boundary(target: WasmTarget, boundary: WasmBoundary, init_ns: &str, init_def: &str) -> Result<(), String> {
  validate_wasm_boundary_with_world(target, boundary, WasiWorld::Command, init_ns, init_def)
}

pub fn validate_wasm_boundary_with_world(
  target: WasmTarget,
  boundary: WasmBoundary,
  world: WasiWorld,
  init_ns: &str,
  init_def: &str,
) -> Result<(), String> {
  let program_data = program::clone_compiled_program_snapshot()?;
  validate_stdin_boundary(&program_data, init_ns, init_def, target, boundary, world)?;
  if target == WasmTarget::Wasi {
    match (world, boundary) {
      (WasiWorld::Http, WasmBoundary::Native) => {
        return Err("E_WASI_WORLD: `--world http` requires `--boundary component`; WASI Preview 1 has no HTTP proxy world".into());
      }
      (WasiWorld::Http, WasmBoundary::Component) => {
        let _ = http_proxy::validate_wasi_http_component_in_program(&program_data, init_ns, init_def)?;
      }
      (WasiWorld::Command, WasmBoundary::Component) => {
        validate_wasm_command_component_in_program(&program_data, init_ns, init_def)?;
      }
      (WasiWorld::Command, WasmBoundary::Native) => {}
    }
    return Ok(());
  }
  if boundary == WasmBoundary::Native {
    return Ok(());
  }
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

fn find_external_object_union_in_union_def(
  union_def: &crate::wasm::host::calcit::CalcitUnionDef,
  visited: &mut HashSet<Arc<str>>,
) -> Option<(String, String)> {
  let union_name: Arc<str> = union_def
    .definition_ref()
    .cloned()
    .unwrap_or_else(|| Arc::from(union_def.name.ref_str()));
  if !visited.insert(union_name.clone()) {
    return None;
  }
  let members = union_def.checked_members().ok()?;
  for member in members {
    if member.kind == crate::wasm::host::calcit::union_def::UnionMemberKind::ExternalObject {
      return Some((union_name.to_string(), member.annotation.to_brief_string()));
    }
    if let Some(found) = find_external_object_union_in_annotation(member.annotation.as_ref(), visited) {
      return Some(found);
    }
  }
  None
}

fn find_external_object_union_in_annotation(
  annotation: &CalcitTypeAnnotation,
  visited: &mut HashSet<Arc<str>>,
) -> Option<(String, String)> {
  if let Some(union_def) = annotation.resolve_to_union() {
    return find_external_object_union_in_union_def(union_def.as_ref(), visited);
  }
  match annotation {
    CalcitTypeAnnotation::List(inner)
    | CalcitTypeAnnotation::Set(inner)
    | CalcitTypeAnnotation::Ref(inner)
    | CalcitTypeAnnotation::Nullable(inner, _)
    | CalcitTypeAnnotation::Variadic(inner) => find_external_object_union_in_annotation(inner.as_ref(), visited),
    CalcitTypeAnnotation::Map(key, value) => find_external_object_union_in_annotation(key.as_ref(), visited)
      .or_else(|| find_external_object_union_in_annotation(value.as_ref(), visited)),
    CalcitTypeAnnotation::Tuple(args)
    | CalcitTypeAnnotation::Struct(_, args)
    | CalcitTypeAnnotation::Enum(_, args)
    | CalcitTypeAnnotation::TypeRef(_, args) => args
      .iter()
      .find_map(|arg| find_external_object_union_in_annotation(arg.as_ref(), visited)),
    CalcitTypeAnnotation::Fn(signature) => signature
      .arg_types
      .iter()
      .find_map(|arg| find_external_object_union_in_annotation(arg.as_ref(), visited))
      .or_else(|| {
        signature
          .rest_type
          .as_ref()
          .and_then(|rest| find_external_object_union_in_annotation(rest.as_ref(), visited))
      })
      .or_else(|| find_external_object_union_in_annotation(signature.return_type.as_ref(), visited)),
    _ => None,
  }
}

fn find_external_object_union_in_expr(expr: &Calcit, visited: &mut HashSet<Arc<str>>) -> Option<(String, String)> {
  match expr {
    Calcit::Local(local) => find_external_object_union_in_annotation(local.type_info.as_ref(), visited),
    Calcit::UnionDef(union_def) => find_external_object_union_in_union_def(union_def.as_ref(), visited),
    Calcit::Method(
      _,
      MethodKind::Invoke(ty)
      | MethodKind::ExternalAccess(ty)
      | MethodKind::ExternalGet(ty)
      | MethodKind::ExternalSet(ty)
      | MethodKind::ExternalInvoke(ty),
    ) => find_external_object_union_in_annotation(ty.as_ref(), visited),
    Calcit::List(items) => items.iter().find_map(|item| find_external_object_union_in_expr(item, visited)),
    Calcit::Fn { info, .. } => info.body.iter().find_map(|item| find_external_object_union_in_expr(item, visited)),
    Calcit::Thunk(thunk) => find_external_object_union_in_expr(thunk.get_code(), visited),
    Calcit::Map(map) => map
      .iter()
      .find_map(|(k, v)| find_external_object_union_in_expr(k, visited).or_else(|| find_external_object_union_in_expr(v, visited))),
    Calcit::Set(set) => set.iter().find_map(|item| find_external_object_union_in_expr(item, visited)),
    Calcit::Recur(args) => args.iter().find_map(|item| find_external_object_union_in_expr(item, visited)),
    _ => None,
  }
}

fn annotation_contains_ffi_resource(annotation: &CalcitTypeAnnotation, visited: &mut HashSet<Arc<str>>) -> bool {
  match annotation {
    CalcitTypeAnnotation::FfiResource => true,
    CalcitTypeAnnotation::List(inner)
    | CalcitTypeAnnotation::Set(inner)
    | CalcitTypeAnnotation::Ref(inner)
    | CalcitTypeAnnotation::Nullable(inner, _)
    | CalcitTypeAnnotation::Variadic(inner) => annotation_contains_ffi_resource(inner, visited),
    CalcitTypeAnnotation::Tuple(items) => items.iter().any(|item| annotation_contains_ffi_resource(item, visited)),
    CalcitTypeAnnotation::Map(k, v) => annotation_contains_ffi_resource(k, visited) || annotation_contains_ffi_resource(v, visited),
    CalcitTypeAnnotation::Fn(signature) => {
      signature.arg_types.iter().any(|arg| annotation_contains_ffi_resource(arg, visited))
        || annotation_contains_ffi_resource(&signature.return_type, visited)
        || signature
          .rest_type
          .as_deref()
          .is_some_and(|rest| annotation_contains_ffi_resource(rest, visited))
    }
    CalcitTypeAnnotation::Struct(struct_def, args) => {
      args.iter().any(|arg| annotation_contains_ffi_resource(arg, visited))
        || struct_def
          .field_types
          .iter()
          .any(|field| annotation_contains_ffi_resource(field, visited))
    }
    CalcitTypeAnnotation::StructValue(struct_def) | CalcitTypeAnnotation::StructDef(struct_def) => struct_def
      .field_types
      .iter()
      .any(|field| annotation_contains_ffi_resource(field, visited)),
    CalcitTypeAnnotation::Enum(enum_def, args) => {
      args.iter().any(|arg| annotation_contains_ffi_resource(arg, visited))
        || enum_def.variants().iter().any(|variant| {
          variant
            .payload_types()
            .iter()
            .any(|payload| annotation_contains_ffi_resource(payload, visited))
        })
    }
    CalcitTypeAnnotation::EnumValue(enum_def) | CalcitTypeAnnotation::EnumDef(enum_def) => enum_def.variants().iter().any(|variant| {
      variant
        .payload_types()
        .iter()
        .any(|payload| annotation_contains_ffi_resource(payload, visited))
    }),
    CalcitTypeAnnotation::TypeRef(name, args) => {
      if args.iter().any(|arg| annotation_contains_ffi_resource(arg, visited)) {
        return true;
      }
      if !visited.insert(name.clone()) {
        return false;
      }
      if let Some(struct_def) = annotation.resolve_to_struct() {
        return struct_def
          .field_types
          .iter()
          .any(|field| annotation_contains_ffi_resource(field, visited));
      }
      if let Some(enum_def) = annotation.resolve_to_enum() {
        return enum_def.variants().iter().any(|variant| {
          variant
            .payload_types()
            .iter()
            .any(|payload| annotation_contains_ffi_resource(payload, visited))
        });
      }
      if let Some(union_def) = annotation.resolve_to_union() {
        return union_def
          .members()
          .iter()
          .any(|member| annotation_contains_ffi_resource(member, visited));
      }
      false
    }
    _ => false,
  }
}

fn def_uses_ffi_resource(compiled: &program::CompiledDef) -> bool {
  let mut visited = HashSet::new();
  if annotation_contains_ffi_resource(compiled.schema.as_ref(), &mut visited) {
    return true;
  }
  if let Some(type_def) = crate::wasm::host::calcit::type_annotation::resolve_type_def_from_code(&compiled.preprocessed_code) {
    match type_def {
      Calcit::StructDef(struct_def) => {
        return struct_def
          .field_types
          .iter()
          .any(|field| annotation_contains_ffi_resource(field, &mut visited));
      }
      Calcit::EnumDef(enum_def) => {
        return enum_def.variants().iter().any(|variant| {
          variant
            .payload_types()
            .iter()
            .any(|payload| annotation_contains_ffi_resource(payload, &mut visited))
        });
      }
      Calcit::UnionDef(union_def) => {
        return union_def
          .members()
          .iter()
          .any(|member| annotation_contains_ffi_resource(member, &mut visited));
      }
      _ => {}
    }
  }
  false
}

fn validate_wasm_target_in_program(
  program_data: &program::CompiledProgram,
  init_ns: &str,
  init_def: &str,
  target: WasmTarget,
  world: WasiWorld,
) -> Result<(), String> {
  for namespace in deterministic_namespace_order(program_data, None) {
    let Some(file) = program_data.get(namespace.as_str()) else {
      continue;
    };
    for (definition, compiled) in deterministic_definition_order(file) {
      if program::lookup_def_ffi(&namespace, definition)
        .as_ref()
        .is_some_and(|ffi| crate::wasm::host::js_ffi_source::parse_js_source(ffi).ok().flatten().is_some())
      {
        return Err(format!(
          "E_WASM_UNSUPPORTED_JS_FFI: `{namespace}/{definition}` has a JavaScript implementation; WASM cannot lower this definition"
        ));
      }
      let mut visited = HashSet::new();
      let self_ref = CalcitTypeAnnotation::TypeRef(Arc::from(format!("{namespace}/{definition}")), Arc::new(vec![]));
      if let Some((union_name, member_name)) = find_external_object_union_in_annotation(&self_ref, &mut visited)
        .or_else(|| find_external_object_union_in_annotation(compiled.schema.as_ref(), &mut visited))
        .or_else(|| find_external_object_union_in_expr(&compiled.preprocessed_code, &mut visited))
      {
        return Err(format!(
          "E_WASM_UNSUPPORTED_JS_FFI: `{namespace}/{definition}` uses union `{union_name}` with external-object host trait member `{member_name}`; WASM/WASI cannot lower JavaScript host handles"
        ));
      }
      if def_uses_ffi_resource(compiled) {
        return Err(format!(
          "E_WASM_UNSUPPORTED_FFI_RESOURCE: `{namespace}/{definition}` uses native `FfiResource`; WASM/WASI cannot lower native resource-v1 handles"
        ));
      }
    }
  }
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
        let allowed_http_db = world == WasiWorld::Http
          && parse_wasm_import_def(&compiled.preprocessed_code)
            .is_some_and(|(module, _, _)| http_proxy::is_wasi_http_db_module(&module));
        if !allowed_http_db {
          return Err(format!(
            "E_WASM_CAPABILITY: `{ns}/{def_name}` declares a custom WASM import; the `wasi` target only permits registered capabilities"
          ));
        }
      }
      if def_name.as_ref() == "_start" {
        return Err("E_WASM_TARGET: `_start` is reserved for the generated WASI command entry".into());
      }
    }
  }

  if world == WasiWorld::Http {
    return http_proxy::validate_wasi_http_target_in_program(program_data, init_ns, init_def);
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
#[derive(Clone)]
struct CompiledFn {
  export_name: Option<String>,
  params: Vec<ValType>,
  results: Vec<ValType>,
  /// All local variables (including temporaries), indexed by declaration order
  locals: Vec<ValType>,
  /// Instruction sequence for the function body
  instructions: Vec<Instruction<'static>>,
}

#[cfg(test)]
fn reject_reachable_wasi_command_dependencies(
  functions: &[CompiledFn],
  num_imports: u32,
  entry_index: u32,
  unsupported: &HashMap<u32, (String, String)>,
) -> Result<(), String> {
  reject_reachable_wasm_dependencies(functions, num_imports, entry_index, unsupported, &HashMap::new(), true)
}

fn reject_reachable_wasm_dependencies(
  functions: &[CompiledFn],
  num_imports: u32,
  entry_index: u32,
  unsupported: &HashMap<u32, (String, String)>,
  function_values: &HashMap<u32, HashSet<u32>>,
  wasi_command: bool,
) -> Result<(), String> {
  let mut pending = vec![entry_index];
  let mut visited = HashSet::new();
  while let Some(index) = pending.pop() {
    if !visited.insert(index) {
      continue;
    }
    if let Some((definition, reason)) = unsupported.get(&index) {
      let code = if !wasi_command {
        "E_WASM_TARGET"
      } else if reason.starts_with("E_WASI_HTTP_CAPABILITY:") {
        "E_WASI_HTTP_CAPABILITY"
      } else if reason.starts_with("E_WASI_COMMAND_CAPABILITY:") {
        "E_WASI_COMMAND_CAPABILITY"
      } else {
        "E_WASI_COMMAND_DEPENDENCY"
      };
      return Err(format!("{code}: reachable `{definition}` cannot compile: {reason}"));
    }
    if index < num_imports {
      continue;
    }
    if let Some(references) = function_values.get(&index) {
      pending.extend(references.iter().copied());
    }
    let function = functions
      .get((index - num_imports) as usize)
      .ok_or_else(|| format!("E_WASI_COMMAND_DEPENDENCY: missing function index {index}"))?;
    for instruction in &function.instructions {
      match instruction {
        Instruction::Call(callee) => pending.push(*callee),
        Instruction::CallIndirect { .. } if wasi_command => {
          return Err("E_WASI_COMMAND_INDIRECT: an indirect call may reach an unsupported host capability".into());
        }
        _ => {}
      }
    }
  }
  Ok(())
}

/// Replace unreachable non-exported function bodies with `unreachable` in WASI Components
/// so dead dependency `call_indirect` instructions do not pull funcref tables into pooling hosts.
fn prune_unreachable_wasi_component_fns(functions: &mut [CompiledFn], num_imports: u32, entry_index: u32) {
  let mut pending = vec![entry_index];
  for (i, func) in functions.iter().enumerate() {
    if func.export_name.is_some() {
      pending.push(num_imports + i as u32);
    }
  }
  let mut reachable = HashSet::new();
  while let Some(index) = pending.pop() {
    if !reachable.insert(index) || index < num_imports {
      continue;
    }
    let Some(func) = functions.get((index - num_imports) as usize) else {
      continue;
    };
    for instr in &func.instructions {
      match instr {
        Instruction::Call(callee) | Instruction::RefFunc(callee) => pending.push(*callee),
        Instruction::CallIndirect { .. } => return,
        _ => {}
      }
    }
  }
  for (i, func) in functions.iter_mut().enumerate() {
    if !reachable.contains(&(num_imports + i as u32)) {
      func.locals.clear();
      func.instructions = vec![Instruction::Unreachable];
    }
  }
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
  turn_enter_index: u32,
  turn_exit_index: u32,
  rewind_to_mark_index: u32,
  comp_globals: ComponentMemoryGlobals,
  active_turns_global: u32,
  pers_needs_compact_global: Option<u32>,
}

fn build_native_host_import_wrapper(
  params: &[ValType],
  results: &[ValType],
  raw_import_idx: u32,
  host_call_depth_global: u32,
) -> CompiledFn {
  let saved_depth = params.len() as u32;
  let mut instructions = vec![
    Instruction::GlobalGet(host_call_depth_global),
    Instruction::LocalTee(saved_depth),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::GlobalSet(host_call_depth_global),
  ];
  for idx in 0..params.len() {
    instructions.push(Instruction::LocalGet(idx as u32));
  }
  instructions.extend([
    Instruction::Call(raw_import_idx),
    Instruction::LocalGet(saved_depth),
    Instruction::GlobalSet(host_call_depth_global),
  ]);
  CompiledFn {
    export_name: None,
    params: params.to_vec(),
    results: results.to_vec(),
    locals: vec![ValType::I32],
    instructions,
  }
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
        Calcit::EnumDef(enum_def) => Some(enum_def.clone()),
        code => match crate::wasm::host::calcit::type_annotation::resolve_type_def_from_code(code) {
          Some(Calcit::EnumDef(enum_def)) => Some(enum_def),
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
        Calcit::StructDef(struct_def) => Some(struct_def.clone()),
        code => match crate::wasm::host::calcit::type_annotation::resolve_type_def_from_code(code) {
          Some(Calcit::StructDef(struct_def)) => Some(struct_def),
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
  collect_component_import_adapters_inner(program_data, false)
}

fn collect_component_import_adapters_inner(
  program_data: &program::CompiledProgram,
  wrap_http_db_errors: bool,
) -> Result<Vec<ComponentImportAdapter>, String> {
  let mut adapters = Vec::new();
  for namespace in deterministic_namespace_order(program_data, None) {
    let file = program_data.get(namespace.as_str()).expect("ordered namespace must exist");
    for (name, compiled) in deterministic_definition_order(file) {
      if !is_wasm_import_def(&compiled.preprocessed_code) {
        continue;
      }
      let definition = format!("{namespace}/{name}");
      let (module, symbol, args) = parse_wasm_import_def(&compiled.preprocessed_code)
        .ok_or_else(|| format!("E_COMPONENT_ABI_IMPORT: `{definition}` must use `defwasm-import name (args) |module |field`"))?;
      if wrap_http_db_errors && !http_proxy::is_wasi_http_db_module(&module) {
        return Err(format!(
          "E_WASM_CAPABILITY: `{definition}` declares custom WASM import `{module}`; only `calcit:db/sql@0.1.0` and `calcit:db/kv@0.1.0` are permitted on `calcit wasi --world http`"
        ));
      }
      if matches!(
        module.as_str(),
        COMPONENT_ASYNC_ROOT_IMPORT_MODULE | COMPONENT_ASYNC_EXPORT_IMPORT_MODULE
      ) {
        return Err(format!(
          "E_COMPONENT_ABI_RESERVED_IMPORT: `{definition}` cannot declare the compiler-reserved module `{module}`"
        ));
      }
      let map_db_err = |err: String| {
        if wrap_http_db_errors && http_proxy::is_wasi_http_db_module(&module) {
          let context = format!("{definition} ({module}.{symbol})");
          http_proxy::format_wasi_http_db_import_error(&context, &err)
        } else {
          err
        }
      };
      let source_arity = wasm_import_arity(&args)
        .map_err(|reason| format!("E_COMPONENT_ABI_UNSUPPORTED_ARITY: `{definition}` at `logical_schema.parameters` {reason}"))
        .map_err(&map_db_err)?;
      let (parameters, result, invocation) =
        component_function_schema(compiled, &definition, source_arity as usize, program_data, false, false).map_err(&map_db_err)?;
      if invocation == ComponentAbiInvocation::Sync {
        validate_component_flat_parameters(&parameters, &definition).map_err(&map_db_err)?;
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
  validate_component_import_symbols_inner(&adapters, wrap_http_db_errors)?;
  Ok(adapters)
}

fn direct_tail_import_definition(body: &[Calcit], args: &CalcitFnArgs) -> Option<String> {
  let expressions = body
    .iter()
    .filter(|item| !crate::wasm::host::builtins::syntax::is_function_metadata_hint(item))
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
    .filter(|item| !crate::wasm::host::builtins::syntax::is_function_metadata_hint(item))
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

#[cfg(test)]
fn validate_component_import_symbols(adapters: &[ComponentImportAdapter]) -> Result<(), String> {
  validate_component_import_symbols_inner(adapters, false)
}

fn validate_component_import_symbols_inner(adapters: &[ComponentImportAdapter], wrap_http_db_errors: bool) -> Result<(), String> {
  let mut symbol_owners = BTreeMap::<(&str, String), Vec<&str>>::new();
  for adapter in adapters {
    symbol_owners
      .entry((adapter.module.as_str(), component_import_emitted_symbol(adapter)))
      .or_default()
      .push(adapter.definition.as_str());
  }
  if let Some(((module, symbol), owners)) = symbol_owners.into_iter().find(|(_, owners)| owners.len() > 1) {
    let base_err = format!(
      "E_COMPONENT_ABI_SYMBOL_CONFLICT: import symbol `{module}/{symbol}` is declared by {}",
      owners.join(", ")
    );
    if wrap_http_db_errors && http_proxy::is_wasi_http_db_module(module) {
      let context = format!("{} ({module}.{symbol})", owners.join(", "));
      return Err(http_proxy::format_wasi_http_db_import_error(&context, &base_err));
    }
    return Err(base_err);
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

fn emit_track_component_live_change(
  instructions: &mut Vec<Instruction<'static>>,
  comp: ComponentMemoryGlobals,
  header_local: u32,
  allocate: bool,
) {
  if allocate {
    instructions.extend([
      Instruction::GlobalGet(comp.total_live),
      Instruction::I32Const(1),
      Instruction::I32Add,
      Instruction::GlobalSet(comp.total_live),
      Instruction::GlobalGet(comp.turn_mark),
      Instruction::I32Const(0),
      Instruction::I32Ne,
      Instruction::LocalGet(header_local),
      Instruction::GlobalGet(comp.turn_mark),
      Instruction::I32GeU,
      Instruction::I32And,
      Instruction::If(wasm_encoder::BlockType::Empty),
      Instruction::GlobalGet(comp.live_above_mark),
      Instruction::I32Const(1),
      Instruction::I32Add,
      Instruction::GlobalSet(comp.live_above_mark),
      Instruction::End,
    ]);
  } else {
    instructions.extend([
      Instruction::GlobalGet(comp.total_live),
      Instruction::I32Const(0),
      Instruction::I32GtU,
      Instruction::If(wasm_encoder::BlockType::Empty),
      Instruction::GlobalGet(comp.total_live),
      Instruction::I32Const(1),
      Instruction::I32Sub,
      Instruction::GlobalSet(comp.total_live),
      Instruction::End,
      Instruction::GlobalGet(comp.turn_mark),
      Instruction::I32Const(0),
      Instruction::I32Ne,
      Instruction::LocalGet(header_local),
      Instruction::GlobalGet(comp.turn_mark),
      Instruction::I32GeU,
      Instruction::I32And,
      Instruction::GlobalGet(comp.live_above_mark),
      Instruction::I32Const(0),
      Instruction::I32GtU,
      Instruction::I32And,
      Instruction::If(wasm_encoder::BlockType::Empty),
      Instruction::GlobalGet(comp.live_above_mark),
      Instruction::I32Const(1),
      Instruction::I32Sub,
      Instruction::GlobalSet(comp.live_above_mark),
      Instruction::End,
    ]);
  }
}

fn build_cabi_free_fn(comp: ComponentMemoryGlobals) -> CompiledFn {
  let header = 2;
  let capacity = 3;
  let mut instructions = vec![
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
    Instruction::GlobalGet(comp.free_head),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::LocalGet(header),
    Instruction::GlobalSet(comp.free_head),
  ];
  emit_track_component_live_change(&mut instructions, comp, header, false);
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32; 2],
    results: vec![],
    locals: vec![ValType::I32; 2],
    instructions,
  }
}

fn build_cabi_realloc_fn(cabi_free_index: u32, comp: ComponentMemoryGlobals) -> CompiledFn {
  let new_ptr = 4;
  let new_end = 5;
  let copy_len = 6;
  let current_pages = 7;
  let current_free = 8;
  let previous_free = 9;
  let next_free = 10;
  let free_capacity = 11;
  let header = 12;
  let aligned_end = 13;
  let mut instructions = vec![
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
    Instruction::GlobalGet(comp.free_head),
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
    Instruction::GlobalSet(comp.free_head),
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
  ];
  emit_track_component_live_change(&mut instructions, comp, current_free, true);
  instructions.extend([
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
    Instruction::LocalGet(new_end),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::LocalTee(aligned_end),
    Instruction::LocalGet(new_end),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(aligned_end),
    Instruction::I32Const(-8),
    Instruction::I32And,
    Instruction::LocalSet(aligned_end),
    Instruction::MemorySize(0),
    Instruction::LocalTee(current_pages),
    Instruction::I32Const(65_536),
    Instruction::I32Ne,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(aligned_end),
    Instruction::LocalGet(current_pages),
    Instruction::I32Const(16),
    Instruction::I32Shl,
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(aligned_end),
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
    Instruction::LocalGet(aligned_end),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
  ]);
  emit_track_component_live_change(&mut instructions, comp, header, true);
  instructions.extend([
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
  ]);
  CompiledFn {
    export_name: Some("cabi_realloc".into()),
    params: vec![ValType::I32; 4],
    results: vec![ValType::I32],
    locals: vec![ValType::I32; 10],
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
  codecs: &ComponentValueCodecs<'_>,
  lowered_owned_areas: &mut Vec<(ComponentAbiType, u32)>,
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
        Instruction::I32Const(8),
        Instruction::Call(codecs.arena_alloc_index),
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
      lowered_owned_areas.push((parameter.clone(), pair_ptr));
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
        Instruction::I32Const(layout.size),
        Instruction::Call(codecs.arena_alloc_index),
        Instruction::LocalSet(canonical_ptr),
        Instruction::LocalGet(parameter_index),
        Instruction::LocalGet(canonical_ptr),
        Instruction::Call(lower_index),
      ]);
      push_load_value_flat(instructions, parameter, canonical_ptr);
      for offset in (0..flat_types.len()).rev() {
        instructions.push(Instruction::LocalSet(flat_start + offset as u32));
      }
      lowered_owned_areas.push((parameter.clone(), canonical_ptr));
      flat_start
    }
  }
}

fn push_component_async_indirect_parameters(
  adapter: &ComponentImportAdapter,
  parameter_count: usize,
  locals: &mut Vec<ValType>,
  instructions: &mut Vec<Instruction<'static>>,
  codecs: &ComponentValueCodecs<'_>,
  lowered_owned_areas: &mut Vec<(ComponentAbiType, u32)>,
) {
  let (layout, offsets) = component_fields_layout(adapter.parameters.iter());
  let record_ptr = parameter_count as u32 + locals.len() as u32;
  locals.push(ValType::I32);
  instructions.extend([
    Instruction::I32Const(layout.size),
    Instruction::Call(codecs.arena_alloc_index),
    Instruction::LocalSet(record_ptr),
  ]);
  for (index, (parameter, offset)) in adapter.parameters.iter().zip(offsets).enumerate() {
    let flat_start = lower_component_import_parameter_to_flat_locals(
      parameter,
      index as u32,
      parameter_count,
      locals,
      instructions,
      codecs,
      lowered_owned_areas,
    );
    push_store_value_flat(instructions, parameter, flat_start, record_ptr, offset);
  }
  instructions.push(Instruction::LocalGet(record_ptr));
}

fn emit_reclaim_import_buffers(
  instructions: &mut Vec<Instruction<'static>>,
  params_len: usize,
  locals: &mut Vec<ValType>,
  drop_indices: &BTreeMap<ComponentAbiType, u32>,
  lowered_owned_areas: &[(ComponentAbiType, u32)],
  result_type: &ComponentAbiType,
  ret_ptr: Option<u32>,
) {
  let param_drops: Vec<(u32, u32)> = lowered_owned_areas
    .iter()
    .filter_map(|(ty, area_ptr)| drop_indices.get(ty).map(|&drop_idx| (*area_ptr, drop_idx)))
    .collect();
  let result_drop = ret_ptr.and_then(|ptr| drop_indices.get(result_type).map(|&drop_idx| (ptr, drop_idx)));
  if param_drops.is_empty() && result_drop.is_none() {
    return;
  }
  let lifted_result = params_len as u32 + locals.len() as u32;
  locals.push(ValType::F64);
  instructions.push(Instruction::LocalSet(lifted_result));
  for (area_ptr, drop_idx) in param_drops {
    instructions.extend([Instruction::LocalGet(area_ptr), Instruction::Call(drop_idx)]);
  }
  if let Some((ptr, drop_idx)) = result_drop {
    instructions.extend([Instruction::LocalGet(ptr), Instruction::Call(drop_idx)]);
  }
  instructions.push(Instruction::LocalGet(lifted_result));
}

fn build_component_import_adapter(
  adapter: &ComponentImportAdapter,
  async_imports: Option<&ComponentAsyncCanonicalImports>,
  drop_indices: &BTreeMap<ComponentAbiType, u32>,
  codecs: &ComponentValueCodecs<'_>,
) -> CompiledFn {
  let params = vec![ValType::F64; adapter.parameters.len()];
  let mut locals = Vec::new();
  let mut instructions = Vec::new();
  let mut lowered_owned_areas = Vec::new();

  if component_async_import_uses_indirect_parameters(adapter) {
    push_component_async_indirect_parameters(
      adapter,
      params.len(),
      &mut locals,
      &mut instructions,
      codecs,
      &mut lowered_owned_areas,
    );
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
            Instruction::I32Const(8),
            Instruction::Call(codecs.arena_alloc_index),
            Instruction::LocalSet(pair_ptr),
            Instruction::LocalGet(index as u32),
            Instruction::LocalGet(pair_ptr),
            Instruction::Call(codec.lower_index),
            Instruction::LocalGet(pair_ptr),
            Instruction::I32Load(mem_arg_i32(0)),
            Instruction::LocalGet(pair_ptr),
            Instruction::I32Load(mem_arg_i32(4)),
          ]);
          lowered_owned_areas.push((parameter.clone(), pair_ptr));
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
            Instruction::I32Const(layout.size),
            Instruction::Call(codecs.arena_alloc_index),
            Instruction::LocalSet(canonical_ptr),
            Instruction::LocalGet(index as u32),
            Instruction::LocalGet(canonical_ptr),
            Instruction::Call(codec.lower_index),
          ]);
          push_load_value_flat(&mut instructions, parameter, canonical_ptr);
          lowered_owned_areas.push((parameter.clone(), canonical_ptr));
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
            Instruction::I32Const(layout.size),
            Instruction::Call(codecs.arena_alloc_index),
            Instruction::LocalSet(canonical_ptr),
            Instruction::LocalGet(index as u32),
            Instruction::LocalGet(canonical_ptr),
            Instruction::Call(codec.lower_index),
          ]);
          push_load_value_flat(&mut instructions, parameter, canonical_ptr);
          lowered_owned_areas.push((parameter.clone(), canonical_ptr));
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
      drop_indices,
      codecs,
      &lowered_owned_areas,
    );
  }

  let mut sync_ret_ptr = None;
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
      let ret_ptr = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let bytes_new_index = match &adapter.result {
        ComponentAbiType::Buffer => codecs.buffer_new_index,
        ComponentAbiType::String => codecs.str_new_index,
        _ => unreachable!("byte result branch must use a byte-backed Component type"),
      };
      instructions.extend([
        Instruction::I32Const(8),
        Instruction::Call(codecs.arena_alloc_index),
        Instruction::LocalTee(ret_ptr),
        Instruction::Call(adapter.raw_index),
        Instruction::LocalGet(ret_ptr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalGet(ret_ptr),
        Instruction::I32Load(mem_arg_i32(4)),
        Instruction::Call(bytes_new_index),
      ]);
      sync_ret_ptr = Some(ret_ptr);
    }
    ComponentAbiType::List(_) => {
      let ret_ptr = params.len() as u32 + locals.len() as u32;
      locals.push(ValType::I32);
      let codec = codecs
        .list_codecs
        .get(&adapter.result)
        .expect("Component List import result codec must be registered");
      instructions.extend([
        Instruction::I32Const(8),
        Instruction::Call(codecs.arena_alloc_index),
        Instruction::LocalTee(ret_ptr),
        Instruction::Call(adapter.raw_index),
        Instruction::LocalGet(ret_ptr),
        Instruction::I32Load(mem_arg_i32(0)),
        Instruction::LocalGet(ret_ptr),
        Instruction::I32Load(mem_arg_i32(4)),
        Instruction::Call(codec.lift_index),
      ]);
      sync_ret_ptr = Some(ret_ptr);
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
        Instruction::I32Const(layout.size),
        Instruction::Call(codecs.arena_alloc_index),
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
      sync_ret_ptr = Some(ret_ptr);
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
        Instruction::I32Const(layout.size),
        Instruction::Call(codecs.arena_alloc_index),
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
      sync_ret_ptr = Some(ret_ptr);
    }
  }

  emit_reclaim_import_buffers(
    &mut instructions,
    params.len(),
    &mut locals,
    drop_indices,
    &lowered_owned_areas,
    &adapter.result,
    sync_ret_ptr,
  );

  CompiledFn {
    export_name: None,
    params,
    results: vec![ValType::F64],
    locals,
    instructions,
  }
}

#[allow(clippy::too_many_arguments)]
fn finish_component_async_import_adapter(
  adapter: &ComponentImportAdapter,
  canonical: &ComponentAsyncCanonicalImports,
  params: Vec<ValType>,
  mut locals: Vec<ValType>,
  mut instructions: Vec<Instruction<'static>>,
  drop_indices: &BTreeMap<ComponentAbiType, u32>,
  codecs: &ComponentValueCodecs<'_>,
  lowered_owned_areas: &[(ComponentAbiType, u32)],
) -> CompiledFn {
  let ret_ptr = if matches!(adapter.result, ComponentAbiType::Unit) {
    None
  } else {
    let local = params.len() as u32 + locals.len() as u32;
    locals.push(ValType::I32);
    let layout = component_memory_layout(&adapter.result);
    instructions.extend([
      Instruction::I32Const(layout.size),
      Instruction::Call(codecs.arena_alloc_index),
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
    Instruction::I32Const(8),
    Instruction::Call(codecs.arena_alloc_index),
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

  emit_reclaim_import_buffers(
    &mut instructions,
    params.len(),
    &mut locals,
    drop_indices,
    lowered_owned_areas,
    &adapter.result,
    ret_ptr,
  );

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
  instructions.push(Instruction::Call(runtime.turn_exit_index));
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
  turn_exit_index: u32,
  instructions: &mut Vec<Instruction<'static>>,
) {
  instructions.extend([
    Instruction::I32Const(result_discriminant),
    Instruction::I32Const(error_variant),
    Instruction::Call(adapter.task_return_index.expect("stream export must have task.return")),
    Instruction::Call(turn_exit_index),
    Instruction::I32Const(0),
    Instruction::Return,
  ]);
}

#[allow(clippy::too_many_arguments)]
fn build_component_stream_export(
  adapter: &ComponentExportAdapter,
  canonical: &ComponentAsyncCanonicalImports,
  stackless: &ComponentStacklessCanonicalImports,
  buffer_new_index: u32,
  cabi_free_index: u32,
  cabi_realloc_index: u32,
  step_index: u32,
  global_layout: &WasmGlobalLayout,
  turn_enter_index: u32,
  turn_exit_index: u32,
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
  let chunk_mark = local(&mut step_locals, ValType::I32);
  let saved_live_above = local(&mut step_locals, ValType::I32);
  let saved_free_head = local(&mut step_locals, ValType::I32);
  let mut step = vec![Instruction::Loop(wasm_encoder::BlockType::Empty)];
  step.extend([
    Instruction::LocalGet(1),
    Instruction::I32Const(-1),
    Instruction::I32Eq,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::Call(turn_exit_index),
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
  push_component_stream_task_return(adapter, 0, 0, turn_exit_index, &mut step);
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
  push_component_stream_task_return(adapter, 1, consumer.total_limit_variant as i32, turn_exit_index, &mut step);
  step.extend([
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::LocalGet(total),
    Instruction::I32Store(mem_arg_i32(STREAM_STATE_TOTAL)),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::LocalSet(chunk_mark),
  ]);
  if let Some(comp) = &global_layout.component {
    step.extend([
      Instruction::GlobalGet(comp.live_above_mark),
      Instruction::LocalSet(saved_live_above),
      Instruction::GlobalGet(comp.free_head),
      Instruction::LocalSet(saved_free_head),
    ]);
  }
  step.extend([
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
  ]);
  // Reclaim ephemeral chunk buffer allocations immediately between stream chunks
  // as long as no outer turn, deferred compaction, or unmatched cabi_realloc block needs them.
  step.extend([
    Instruction::GlobalGet(global_layout.active_turns),
    Instruction::I32Const(1),
    Instruction::I32Eq,
  ]);
  if let Some(pers) = &global_layout.persistent {
    step.extend([
      Instruction::GlobalGet(pers.needs_compact),
      Instruction::I32Const(2),
      Instruction::I32Ne,
      Instruction::I32And,
    ]);
  }
  if let Some(comp) = &global_layout.component {
    step.extend([
      Instruction::GlobalGet(comp.live_above_mark),
      Instruction::LocalGet(saved_live_above),
      Instruction::I32Eq,
      Instruction::I32And,
      Instruction::GlobalGet(comp.free_head),
      Instruction::LocalGet(saved_free_head),
      Instruction::I32Eq,
      Instruction::I32And,
    ]);
  }
  step.extend([
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(chunk_mark),
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
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
  push_component_stream_task_return(adapter, 0, 0, turn_exit_index, &mut step);
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
    Instruction::Call(turn_enter_index),
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
    Instruction::Call(turn_enter_index),
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
    Instruction::Call(turn_exit_index),
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
    Instruction::Call(turn_exit_index),
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
    Instruction::Call(turn_exit_index),
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

fn push_component_cancelled_result_cleanup(
  result: &ComponentAbiType,
  ret_ptr: u32,
  status_local: u32,
  cabi_free_index: u32,
  drop_indices: &BTreeMap<ComponentAbiType, u32>,
  instructions: &mut Vec<Instruction<'static>>,
) {
  instructions.extend([
    Instruction::LocalGet(status_local),
    Instruction::I32Const(2),
    Instruction::I32Eq,
    Instruction::If(wasm_encoder::BlockType::Empty),
  ]);
  push_component_result_reclaim(result, ret_ptr, cabi_free_index, drop_indices, instructions);
  instructions.extend([
    Instruction::Else,
    Instruction::LocalGet(ret_ptr),
    Instruction::I32Const(component_memory_layout(result).size),
    Instruction::Call(cabi_free_index),
    Instruction::End,
  ]);
}

#[allow(clippy::too_many_arguments)]
fn build_component_stackless_tail_export(
  adapter: &ComponentExportAdapter,
  imported: &ComponentImportAdapter,
  canonical: &ComponentAsyncCanonicalImports,
  stackless: &ComponentStacklessCanonicalImports,
  cabi_free_index: u32,
  cabi_realloc_index: u32,
  drop_indices: &BTreeMap<ComponentAbiType, u32>,
  turn_enter_index: u32,
  turn_exit_index: u32,
) -> [CompiledFn; 2] {
  const STATE_RET_PTR: u64 = 0;
  const STATE_SUBTASK: u64 = 4;
  const STATE_WAITABLE_SET: u64 = 8;
  const STATE_CANCEL_REQUESTED: u64 = 12;
  const STATE_PARAM_RECORD: u64 = 16;
  const STATE_SIZE: i32 = 20;

  let params = adapter.parameters.iter().flat_map(component_flat_types).collect::<Vec<_>>();
  let mut locals = Vec::new();
  let mut instructions = vec![Instruction::Call(turn_enter_index)];
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

  let indirect_param_record = if component_async_import_uses_indirect_parameters(imported) {
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
    Some((record_ptr, layout.size))
  } else {
    for index in 0..params.len() {
      instructions.push(Instruction::LocalGet(index as u32));
    }
    None
  };
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
  if let Some((record_ptr, record_size)) = indirect_param_record {
    instructions.extend([
      Instruction::LocalGet(record_ptr),
      Instruction::I32Const(record_size),
      Instruction::Call(cabi_free_index),
    ]);
  }
  instructions.extend([
    Instruction::Call(turn_exit_index),
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
    Instruction::I32Const(0),
    Instruction::I32Store(mem_arg_i32(STATE_PARAM_RECORD)),
  ]);
  if let Some((record_ptr, record_size)) = indirect_param_record {
    instructions.extend([
      Instruction::LocalGet(status_kind),
      Instruction::I32Eqz,
      Instruction::If(wasm_encoder::BlockType::Empty),
      Instruction::LocalGet(state),
      Instruction::LocalGet(record_ptr),
      Instruction::I32Store(mem_arg_i32(STATE_PARAM_RECORD)),
      Instruction::Else,
      Instruction::LocalGet(record_ptr),
      Instruction::I32Const(record_size),
      Instruction::Call(cabi_free_index),
      Instruction::End,
    ]);
  }
  instructions.extend([
    Instruction::LocalGet(state),
    Instruction::Call(stackless.context_set),
    Instruction::Call(turn_exit_index),
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
  let callback_param_record = callback_params.len() as u32 + callback_locals.len() as u32;
  callback_locals.push(ValType::I32);
  let emit_free_param_record = |insns: &mut Vec<Instruction<'static>>| {
    if let Some((_, record_size)) = indirect_param_record {
      insns.extend([
        Instruction::LocalGet(callback_param_record),
        Instruction::If(wasm_encoder::BlockType::Empty),
        Instruction::LocalGet(callback_state),
        Instruction::I32Const(0),
        Instruction::I32Store(mem_arg_i32(STATE_PARAM_RECORD)),
        Instruction::LocalGet(callback_param_record),
        Instruction::I32Const(record_size),
        Instruction::Call(cabi_free_index),
        Instruction::I32Const(0),
        Instruction::LocalSet(callback_param_record),
        Instruction::End,
      ]);
    }
  };
  let mut callback_instructions = vec![
    Instruction::Call(turn_enter_index),
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
    Instruction::LocalGet(callback_state),
    Instruction::I32Load(mem_arg_i32(STATE_PARAM_RECORD)),
    Instruction::LocalSet(callback_param_record),
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
    Instruction::Call(turn_exit_index),
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
  callback_instructions.extend([Instruction::I32Const(0), Instruction::Call(stackless.context_set)]);
  emit_free_param_record(&mut callback_instructions);
  callback_instructions.extend([
    Instruction::LocalGet(callback_state),
    Instruction::I32Const(STATE_SIZE),
    Instruction::Call(cabi_free_index),
  ]);
  if ret_ptr.is_some() {
    push_component_cancelled_result_cleanup(
      &adapter.result,
      callback_ret_ptr,
      cancel_status,
      cabi_free_index,
      drop_indices,
      &mut callback_instructions,
    );
  }
  callback_instructions.extend([
    Instruction::Call(stackless.task_cancel),
    Instruction::Call(turn_exit_index),
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
  ]);
  if indirect_param_record.is_some() {
    callback_instructions.extend([
      Instruction::LocalGet(2),
      Instruction::I32Const(1),
      Instruction::I32Eq,
      Instruction::If(wasm_encoder::BlockType::Empty),
    ]);
    emit_free_param_record(&mut callback_instructions);
    callback_instructions.push(Instruction::End);
  }
  callback_instructions.extend([
    Instruction::Call(turn_exit_index),
    Instruction::LocalGet(callback_set),
    Instruction::I32Const(4),
    Instruction::I32Shl,
    Instruction::I32Const(2),
    Instruction::I32Or,
    Instruction::Return,
    Instruction::End,
  ]);
  push_component_stackless_cleanup(canonical, callback_subtask, callback_set, &mut callback_instructions);
  callback_instructions.extend([Instruction::I32Const(0), Instruction::Call(stackless.context_set)]);
  emit_free_param_record(&mut callback_instructions);
  callback_instructions.extend([
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
  if ret_ptr.is_some() {
    push_component_result_reclaim(
      &adapter.result,
      callback_ret_ptr,
      cabi_free_index,
      drop_indices,
      &mut callback_instructions,
    );
  }
  callback_instructions.push(Instruction::Else);
  if ret_ptr.is_some() {
    push_component_cancelled_result_cleanup(
      &adapter.result,
      callback_ret_ptr,
      2,
      cabi_free_index,
      drop_indices,
      &mut callback_instructions,
    );
  }
  callback_instructions.extend([
    Instruction::Call(stackless.task_cancel),
    Instruction::End,
    Instruction::Call(turn_exit_index),
    Instruction::I32Const(0),
  ]);

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
  let arena_alloc_index = runtime.codecs.arena_alloc_index;
  let cabi_realloc_index = runtime.codecs.cabi_realloc_index;
  let list_codecs = runtime.codecs.list_codecs;
  let struct_codecs = runtime.codecs.struct_codecs;
  let variant_codecs = runtime.codecs.variant_codecs;
  let mut params = Vec::new();
  for parameter in &adapter.parameters {
    params.extend(component_flat_types(parameter));
  }

  let mut locals = Vec::new();
  let mut instructions = vec![Instruction::Call(runtime.turn_enter_index)];
  let needs_post_return = component_export_needs_post_return(adapter);
  let per_call_rewind = adapter.invocation == ComponentAbiInvocation::Sync && !needs_post_return;
  let (call_mark_local, saved_live_above_local) = if per_call_rewind {
    let call_mark = params.len() as u32 + locals.len() as u32;
    locals.push(ValType::I32);
    let saved_live = params.len() as u32 + locals.len() as u32;
    locals.push(ValType::I32);
    instructions.extend([Instruction::GlobalGet(HEAP_PTR_GLOBAL), Instruction::LocalSet(call_mark)]);
    (Some(call_mark), Some(saved_live))
  } else {
    (None, None)
  };
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
          Instruction::LocalGet(flat_index + 1),
          Instruction::If(wasm_encoder::BlockType::Empty),
          Instruction::LocalGet(flat_index),
          Instruction::LocalGet(flat_index + 1),
          Instruction::Call(runtime.cabi_free_index),
          Instruction::End,
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
        if let Some(&drop_idx) = runtime.drop_indices.get(parameter) {
          let pair_ptr = params.len() as u32 + locals.len() as u32;
          locals.push(ValType::I32);
          instructions.extend([
            Instruction::I32Const(8),
            Instruction::Call(arena_alloc_index),
            Instruction::LocalTee(pair_ptr),
            Instruction::LocalGet(flat_index),
            Instruction::I32Store(mem_arg_i32(0)),
            Instruction::LocalGet(pair_ptr),
            Instruction::LocalGet(flat_index + 1),
            Instruction::I32Store(mem_arg_i32(4)),
            Instruction::LocalGet(pair_ptr),
            Instruction::Call(drop_idx),
          ]);
        }
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
          Instruction::I32Const(layout.size),
          Instruction::Call(arena_alloc_index),
          Instruction::LocalSet(canonical_ptr),
        ]);
        push_store_value_flat(&mut instructions, parameter, flat_index, canonical_ptr, 0);
        instructions.extend([
          Instruction::LocalGet(canonical_ptr),
          Instruction::Call(codec.lift_index),
          Instruction::LocalSet(local),
        ]);
        if let Some(&drop_idx) = runtime.drop_indices.get(parameter) {
          instructions.extend([Instruction::LocalGet(canonical_ptr), Instruction::Call(drop_idx)]);
        }
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
          Instruction::I32Const(layout.size),
          Instruction::Call(arena_alloc_index),
          Instruction::LocalSet(canonical_ptr),
        ]);
        push_store_value_flat(&mut instructions, parameter, flat_index, canonical_ptr, 0);
        instructions.extend([
          Instruction::LocalGet(canonical_ptr),
          Instruction::Call(codec.lift_index),
          Instruction::LocalSet(local),
        ]);
        if let Some(&drop_idx) = runtime.drop_indices.get(parameter) {
          instructions.extend([Instruction::LocalGet(canonical_ptr), Instruction::Call(drop_idx)]);
        }
        lowered.push(local);
        flat_index += component_flat_types(parameter).len() as u32;
      }
    }
  }
  if let Some(saved_live) = saved_live_above_local {
    instructions.extend([
      Instruction::GlobalGet(runtime.comp_globals.live_above_mark),
      Instruction::LocalSet(saved_live),
    ]);
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
      let flat_types = component_flat_types(&adapter.result);
      instructions.push(Instruction::LocalSet(value));
      if flat_types.len() > 1 {
        instructions.extend([
          Instruction::I32Const(0),
          Instruction::I32Const(0),
          Instruction::I32Const(layout.alignment),
          Instruction::I32Const(layout.size),
          Instruction::Call(cabi_realloc_index),
        ]);
      } else {
        instructions.extend([Instruction::I32Const(layout.size), Instruction::Call(arena_alloc_index)]);
      }
      instructions.extend([
        Instruction::LocalSet(ret_ptr),
        Instruction::LocalGet(value),
        Instruction::LocalGet(ret_ptr),
        Instruction::Call(codec.lower_index),
      ]);
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
      let flat_types = component_flat_types(&adapter.result);
      instructions.push(Instruction::LocalSet(value));
      if flat_types.len() > 1 {
        instructions.extend([
          Instruction::I32Const(0),
          Instruction::I32Const(0),
          Instruction::I32Const(layout.alignment),
          Instruction::I32Const(layout.size),
          Instruction::Call(cabi_realloc_index),
        ]);
      } else {
        instructions.extend([Instruction::I32Const(layout.size), Instruction::Call(arena_alloc_index)]);
      }
      instructions.extend([
        Instruction::LocalSet(ret_ptr),
        Instruction::LocalGet(value),
        Instruction::LocalGet(ret_ptr),
        Instruction::Call(codec.lower_index),
      ]);
      if flat_types.len() > 1 {
        instructions.push(Instruction::LocalGet(ret_ptr));
        vec![ValType::I32]
      } else {
        push_load_value_flat(&mut instructions, &adapter.result, ret_ptr);
        flat_types
      }
    }
  };

  if !needs_post_return {
    let push_per_call_rewind = |insns: &mut Vec<Instruction<'static>>| {
      if let (Some(call_mark), Some(saved_live)) = (call_mark_local, saved_live_above_local) {
        insns.extend([
          Instruction::GlobalGet(runtime.active_turns_global),
          Instruction::I32Const(0),
          Instruction::I32GtU,
          Instruction::GlobalGet(runtime.comp_globals.live_above_mark),
          Instruction::LocalGet(saved_live),
          Instruction::I32Eq,
          Instruction::I32And,
        ]);
        if let Some(needs_compact) = runtime.pers_needs_compact_global {
          insns.extend([
            Instruction::GlobalGet(needs_compact),
            Instruction::I32Const(2),
            Instruction::I32Ne,
            Instruction::I32And,
          ]);
        }
        insns.extend([
          Instruction::If(wasm_encoder::BlockType::Empty),
          Instruction::LocalGet(call_mark),
          Instruction::Call(runtime.rewind_to_mark_index),
          Instruction::End,
        ]);
      }
    };
    if let Some(&result_ty) = results.first() {
      let ret_flat = params.len() as u32 + locals.len() as u32;
      locals.push(result_ty);
      instructions.extend([Instruction::LocalSet(ret_flat), Instruction::Call(runtime.turn_exit_index)]);
      push_per_call_rewind(&mut instructions);
      instructions.push(Instruction::LocalGet(ret_flat));
    } else {
      instructions.push(Instruction::Call(runtime.turn_exit_index));
      push_per_call_rewind(&mut instructions);
    }
  }

  CompiledFn {
    export_name: Some(component_export_emitted_symbol(adapter)),
    params,
    results,
    locals,
    instructions,
  }
}

/// Internal unboxed WASM value representation for closed scalars, integer
/// refinements, and known non-null heap pointers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum NativeValRepr {
  F64,
  BoolI32,
  TagI32,
  UnitI32,
  Int32S,
  UInt32,
  Int64S,
  UInt64,
  HeapPtrI32,
}

impl NativeValRepr {
  pub(super) fn val_type(self) -> ValType {
    match self {
      Self::F64 => ValType::F64,
      Self::Int64S | Self::UInt64 => ValType::I64,
      Self::BoolI32 | Self::TagI32 | Self::UnitI32 | Self::Int32S | Self::UInt32 | Self::HeapPtrI32 => ValType::I32,
    }
  }

  pub(super) fn from_type_annotation(annotation: &Arc<CalcitTypeAnnotation>) -> Self {
    let expanded = expand_wasm_value_type(annotation);
    match expanded.as_ref() {
      CalcitTypeAnnotation::Bool => Self::BoolI32,
      CalcitTypeAnnotation::Tag => Self::TagI32,
      CalcitTypeAnnotation::Unit => Self::UnitI32,
      CalcitTypeAnnotation::Numeric(refinement) => match refinement {
        CalcitNumericRefinement::Int8 | CalcitNumericRefinement::Int16 | CalcitNumericRefinement::Int32 => Self::Int32S,
        CalcitNumericRefinement::UInt8 | CalcitNumericRefinement::UInt16 | CalcitNumericRefinement::UInt32 => Self::UInt32,
        CalcitNumericRefinement::Int64 => Self::Int64S,
        CalcitNumericRefinement::UInt64 => Self::UInt64,
        CalcitNumericRefinement::Float32 | CalcitNumericRefinement::Float64 => Self::F64,
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
      | CalcitTypeAnnotation::AnonymousEnum
      | CalcitTypeAnnotation::Any => Self::HeapPtrI32,
      CalcitTypeAnnotation::TypeRef(_, _) if expanded.resolve_to_struct().is_some() || expanded.resolve_to_enum().is_some() => {
        Self::HeapPtrI32
      }
      _ => Self::F64,
    }
  }
}

/// Native WASM calling convention for internal fixed-arity functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct InternalFnSig {
  pub params: Vec<NativeValRepr>,
  pub result: NativeValRepr,
}

impl InternalFnSig {
  pub(super) fn has_native_slots(&self) -> bool {
    self.result != NativeValRepr::F64 || self.params.iter().any(|repr| *repr != NativeValRepr::F64)
  }

  pub(super) fn wasm_params(&self) -> Vec<ValType> {
    self.params.iter().map(|r| r.val_type()).collect()
  }
}

/// Qualify short nominal type references in a definition's schema before inspecting its
/// signature for WASM lowering so same-namespace struct/enum names resolve cleanly.
fn qualified_fn_schema(schema: &Arc<CalcitTypeAnnotation>, ns: &str) -> Option<Arc<CalcitFnTypeAnnotation>> {
  crate::wasm::host::calcit::type_refs::qualify_namespace_type_refs(schema.clone(), ns).resolve_to_fn()
}

#[derive(Clone)]
struct WasmCompileEnv {
  /// Function values actually emitted, keyed by their containing function index.
  function_value_dependencies: FunctionValueDependencies,
  fn_index: HashMap<String, u32>,
  fn_arity: HashMap<String, u32>,
  fn_has_rest: HashMap<String, u32>,
  /// Internal native signatures keyed by WASM function index.
  fn_signatures: HashMap<u32, InternalFnSig>,
  /// Whether any user-reachable definition may construct heap-boxed JSON scalars.
  has_boxed_json_scalars: bool,
  runtime_fn_index: HashMap<String, u32>,
  tag_index: HashMap<String, u32>,
  struct_layouts: StructLayouts,
  enum_layouts: EnumLayouts,
  any_type_table: any_box::AnyTypeTable,
  /// Top-level nominal `ns/def` definitions touched by dynamic/local `impl-traits`.
  non_coherent_nominal_defs: Arc<HashSet<String>>,
  string_pool: HashMap<String, u32>,
  /// qualified atom name ("ns/def") → WASM global index
  atom_globals: HashMap<String, u32>,
  /// qualified atom name ("ns/def") → whether its Ref inner type is a proven scalar
  atom_is_scalar: HashMap<String, bool>,
  /// qualified value name ("ns/def") → preprocessed expression for inlining.
  value_imports: HashMap<String, Calcit>,
  /// Statically resolved function definitions available for closure-argument specialization.
  static_fn_defs: HashMap<String, Arc<StaticFnDef>>,
  /// Registry for object-safe `(Dyn Trait)` fat pointers and `BrTable` dispatch trampolines.
  dyn_traits: Rc<RefCell<dyn_trait::DynTraitRegistry>>,
  /// Registry for static closure singletons, lifted closure bodies, and `__closure_call_<arity>` trampolines.
  closures: Rc<RefCell<closure::ClosureRegistry>>,
  /// Target-approved host imports keyed by `(module, field)`.
  host_imports: HashMap<(String, String), u32>,
  /// Selected output ABI. Surface Calcit calls stay target-independent.
  target: WasmTarget,
  boundary: WasmBoundary,
  world: WasiWorld,
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
    .filter(|item| !crate::wasm::host::builtins::syntax::is_function_metadata_hint(item))
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

#[derive(Clone, Eq)]
struct WasmTypeEvidence {
  annotation: Arc<CalcitTypeAnnotation>,
  /// Retains encoded-struct or uninhabited-enum constructor hints for map codec passes.
  codec_hint: Option<Arc<CalcitTypeAnnotation>>,
  /// Retains concrete element/key types for Set and Map literals widened to open Set/Map contracts.
  set_member_types: Option<Vec<Arc<CalcitTypeAnnotation>>>,
  /// Retains closed concrete element types for heterogeneous lists widened to `List Dynamic` (e.g. `&list:concat`).
  list_element_types: Option<Vec<Arc<CalcitTypeAnnotation>>>,
  /// Retains concrete scalar or Option payload origin across local `assert-type` widening.
  scalar_origin: Option<Arc<CalcitTypeAnnotation>>,
  /// Retains a known non-inline callable target carried through a local or parameter.
  known_function: Option<CalcitImport>,
  /// The value's actual construction/transport proves its attached impl table.
  /// A declared nominal data contract alone does not establish this identity.
  exact_impls: bool,
}

impl PartialEq for WasmTypeEvidence {
  fn eq(&self, other: &Self) -> bool {
    self.annotation == other.annotation && self.exact_impls == other.exact_impls
  }
}

impl WasmTypeEvidence {
  fn contract(annotation: Arc<CalcitTypeAnnotation>) -> Self {
    Self {
      annotation,
      codec_hint: None,
      set_member_types: None,
      list_element_types: None,
      scalar_origin: None,
      known_function: None,
      exact_impls: false,
    }
  }
}

fn push_unique_member_type(out: &mut Vec<Arc<CalcitTypeAnnotation>>, ty: &Arc<CalcitTypeAnnotation>) {
  let exp = CalcitTypeAnnotation::expand_schema_alias(ty);
  if !matches!(exp.as_ref(), CalcitTypeAnnotation::Never) && !out.contains(&exp) {
    out.push(exp);
  }
}

fn join_branch_member_types(
  left: Option<Vec<Arc<CalcitTypeAnnotation>>>,
  right: Option<Vec<Arc<CalcitTypeAnnotation>>>,
) -> Option<Vec<Arc<CalcitTypeAnnotation>>> {
  match (left, right) {
    (Some(l), Some(r)) => {
      let mut joined = Vec::with_capacity(l.len() + r.len());
      for ty in l.iter().chain(r.iter()) {
        push_unique_member_type(&mut joined, ty);
      }
      Some(joined)
    }
    _ => None,
  }
}

/// Unresolved bare generic slots on empty collection literals act as bottom types
/// until an element or key/value pair is inserted into them.
fn is_placeholder_collection_slot(ty: &Arc<CalcitTypeAnnotation>) -> bool {
  let exp = CalcitTypeAnnotation::expand_schema_alias(ty);
  match exp.as_ref() {
    CalcitTypeAnnotation::Never | CalcitTypeAnnotation::TypeVar(_) | CalcitTypeAnnotation::TypeSlot(_) => true,
    CalcitTypeAnnotation::TypeRef(_, args) => {
      args.is_empty() && exp.resolve_to_struct().is_none() && exp.resolve_to_enum().is_none() && exp.resolve_to_union().is_none()
    }
    _ => false,
  }
}

fn join_collection_slot_type(current: &Arc<CalcitTypeAnnotation>, added: &Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  let c = CalcitTypeAnnotation::expand_schema_alias(current);
  let a = CalcitTypeAnnotation::expand_schema_alias(added);
  if matches!(c.as_ref(), CalcitTypeAnnotation::Never) {
    a
  } else if matches!(a.as_ref(), CalcitTypeAnnotation::Never) || c == a {
    c
  } else if c.contains_type_var() || a.contains_type_var() {
    DYNAMIC_TYPE.clone()
  } else if a.is_proven_for(&c) {
    c
  } else if c.is_proven_for(&a) {
    a
  } else {
    DYNAMIC_TYPE.clone()
  }
}

fn effective_collection_slot(slot: &Arc<CalcitTypeAnnotation>, is_empty: bool) -> Arc<CalcitTypeAnnotation> {
  if is_empty && is_placeholder_collection_slot(slot) {
    crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone()
  } else {
    slot.clone()
  }
}

/// Strip `Nil` from non-first `merge-non-nil` operand values since runtime keeps earlier values when RHS is `nil`.
fn strip_nil_for_merge_non_nil(ty: &Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  match CalcitTypeAnnotation::expand_schema_alias(ty).as_ref() {
    CalcitTypeAnnotation::Nil => crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone(),
    CalcitTypeAnnotation::Nullable(inner, _) => inner.clone(),
    _ => ty.clone(),
  }
}

/// Join one `merge` / `merge-non-nil` operand's Map key and value types into the accumulated pair.
fn join_merge_map_operand(
  joined_map: &mut Option<(Arc<CalcitTypeAnnotation>, Arc<CalcitTypeAnnotation>)>,
  arg_ev: &WasmTypeEvidence,
  skip_rhs_nil: bool,
  arg_idx: usize,
) -> bool {
  let arg_ty = expand_wasm_value_type(arg_ev.scalar_origin.as_ref().unwrap_or(&arg_ev.annotation));
  let is_empty = arg_ev.set_member_types.as_deref() == Some(&[]);
  let CalcitTypeAnnotation::Map(k, v) = arg_ty.as_ref() else {
    return false;
  };
  let eff_k = effective_collection_slot(k, is_empty);
  let mut eff_v = effective_collection_slot(v, is_empty);
  if skip_rhs_nil && arg_idx > 0 {
    eff_v = strip_nil_for_merge_non_nil(&eff_v);
  }
  match joined_map {
    None => {
      *joined_map = Some((eff_k, eff_v));
    }
    Some((k_acc, v_acc)) => {
      *k_acc = join_collection_slot_type(k_acc, &eff_k);
      *v_acc = join_collection_slot_type(v_acc, &eff_v);
    }
  }
  true
}

fn join_collection_branch_evidences(left: &WasmTypeEvidence, right: &WasmTypeEvidence) -> Option<WasmTypeEvidence> {
  let l_exp = CalcitTypeAnnotation::expand_schema_alias(&left.annotation);
  let r_exp = CalcitTypeAnnotation::expand_schema_alias(&right.annotation);
  let l_empty = left.set_member_types.as_deref() == Some(&[]);
  let r_empty = right.set_member_types.as_deref() == Some(&[]);
  let annotation = match (l_exp.as_ref(), r_exp.as_ref()) {
    (CalcitTypeAnnotation::Map(lk, lv), CalcitTypeAnnotation::Map(rk, rv)) => Arc::new(CalcitTypeAnnotation::Map(
      join_collection_slot_type(&effective_collection_slot(lk, l_empty), &effective_collection_slot(rk, r_empty)),
      join_collection_slot_type(&effective_collection_slot(lv, l_empty), &effective_collection_slot(rv, r_empty)),
    )),
    (CalcitTypeAnnotation::Set(le), CalcitTypeAnnotation::Set(re)) => Arc::new(CalcitTypeAnnotation::Set(join_collection_slot_type(
      &effective_collection_slot(le, l_empty),
      &effective_collection_slot(re, r_empty),
    ))),
    _ => return None,
  };
  Some(WasmTypeEvidence {
    annotation,
    set_member_types: join_branch_member_types(left.set_member_types.clone(), right.set_member_types.clone()),
    ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
  })
}

/// Infer list evidence for `&list:concat` / `concat`, preserving closed heterogeneous
/// element types across lists and tuples without unwrapping explicit `assert-type` widening.
fn infer_list_concat_evidence(
  ctx: &WasmGenCtx,
  args: &[Calcit],
  local_types: &HashMap<String, WasmTypeEvidence>,
  closures: &HashMap<String, Arc<InlineClosure>>,
  visiting: &mut HashSet<String>,
) -> Option<WasmTypeEvidence> {
  let never = crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone();
  if args.is_empty() {
    return Some(WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::List(never))));
  }
  let mut elem_types = Vec::new();
  for arg in args {
    let arg_ev = infer_wasm_evidence_in_scope(ctx, arg, local_types, closures, visiting)?;
    if let Some(elems) = &arg_ev.list_element_types {
      for ty in elems {
        push_unique_member_type(&mut elem_types, ty);
      }
      continue;
    }
    match expand_wasm_value_type(&arg_ev.annotation).as_ref() {
      CalcitTypeAnnotation::List(inner) => push_unique_member_type(&mut elem_types, inner),
      CalcitTypeAnnotation::Tuple(slots) => {
        for slot in slots.iter() {
          push_unique_member_type(&mut elem_types, slot);
        }
      }
      _ => return None,
    }
  }
  if elem_types.is_empty() {
    return Some(WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::List(never))));
  }
  if elem_types.len() == 1 {
    return Some(WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::List(
      elem_types[0].clone(),
    ))));
  }
  let joined = elem_types.iter().fold(never, |acc, ty| join_collection_slot_type(&acc, ty));
  if !nil_type_evidence_is_open(&expand_wasm_value_type(&joined)) && !joined.contains_type_var() {
    return Some(WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::List(joined))));
  }
  let all_closed = elem_types
    .iter()
    .all(|ty| !nil_type_evidence_is_open(&expand_wasm_value_type(ty)) && !ty.contains_type_var());
  Some(WasmTypeEvidence {
    annotation: Arc::new(CalcitTypeAnnotation::List(DYNAMIC_TYPE.clone())),
    list_element_types: all_closed.then_some(elem_types),
    ..WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::List(DYNAMIC_TYPE.clone())))
  })
}

#[derive(Clone, Default)]
struct InlineClosure {
  /// Parameter names in declaration order, without `?` / `&` markers.
  params: Vec<String>,
  /// Leading parameters that need an argument. The parameters after them,
  /// except a trailing rest parameter, are `?` optional.
  required: usize,
  /// The last parameter is a `&` rest parameter.
  rest: bool,
  /// Fixed-arity trailing Option parameters bind omission to nominal None.
  option_types: Vec<Arc<CalcitTypeAnnotation>>,
  body: Vec<Calcit>,
  captured_locals: HashMap<String, u32>,
  captured_local_types: HashMap<String, WasmTypeEvidence>,
  captured_closures: HashMap<String, Arc<InlineClosure>>,
  self_name: Option<String>,
  materialized_local: Option<u32>,
  captured_self_closure: Option<Arc<(String, u32, closure::ClosureParamShape)>>,
}

impl InlineClosure {
  /// Parameters bound one-to-one to arguments: required and `?` optional ones.
  fn positional(&self) -> usize {
    self.params.len() - usize::from(self.rest)
  }

  /// Whether a call with `count` arguments binds without an arity error.
  fn accepts(&self, count: usize) -> bool {
    count >= self.required && (self.rest || count <= self.positional())
  }

  fn describe_arity(&self) -> String {
    let positional = self.positional();
    if self.rest {
      format!("at least {}", self.required)
    } else if self.required == positional {
      positional.to_string()
    } else {
      format!("{} to {positional}", self.required)
    }
  }
}

#[derive(Clone)]
struct StaticFnDef {
  params: Vec<String>,
  body: Vec<Calcit>,
  arg_types: Vec<Arc<CalcitTypeAnnotation>>,
  callback_arities: HashMap<usize, usize>,
  fixed_arity: bool,
  nil_sensitive: bool,
  trait_sensitive: bool,
  method_sensitive: bool,
}

type FunctionValueDependencies = Rc<RefCell<HashMap<u32, HashSet<u32>>>>;

enum InlineArgument {
  Value(u32),
  Closure(Arc<InlineClosure>, Option<u32>),
}

/// Context for WASM code generation within a single function.
struct WasmGenCtx {
  source_fn_index: Option<u32>,
  function_value_dependencies: FunctionValueDependencies,
  /// Namespace of the definition currently being emitted.
  source_ns: String,
  /// Map from local variable name to WASM local index
  locals: HashMap<String, u32>,
  /// Call-site-specialized source types for locals. Values keep the existing
  /// f64 ABI; this map only guides lowering that needs nominal type evidence.
  local_types: HashMap<String, WasmTypeEvidence>,
  /// Immutable let values with exact definition targets. Fresh WASM local
  /// indices preserve lexical identity across shadowing and captured scopes.
  local_function_values: HashMap<u32, CalcitImport>,
  /// Immutable let values bound to a compile-time Number literal. Fresh WASM
  /// local indices preserve lexical identity across shadowing and captured scopes.
  local_number_literals: HashMap<u32, f64>,
  /// Local declarations to add (beyond parameters)
  extra_locals: Vec<ValType>,
  /// Next local index (starts after parameters)
  next_local: u32,
  /// Whether this function uses recur (needs loop wrapping)
  uses_recur: bool,
  /// Argument local indices in order (for recur)
  arg_indices: Vec<u32>,
  /// `(required, rest)` parameter shape behind `arg_indices` when it has `?` /
  /// `&` parameters; a plain `recur` binds its arguments by the same rules as
  /// a call. `None` means every parameter is required.
  recur_shape: Option<(usize, bool)>,
  /// A specialized loop must preserve these actual argument contracts on every
  /// back edge; declared nominal shapes alone cannot preserve attached impls.
  recur_evidence: Option<Vec<WasmTypeEvidence>>,
  /// Collected instructions
  instructions: Vec<Instruction<'static>>,
  /// Function name → index map for cross-function calls
  fn_index: HashMap<String, u32>,
  /// Function name → WASM arity (number of f64 params, excluding markers)
  fn_arity: HashMap<String, u32>,
  /// Function name → fixed-arity (params before `&`) for functions with rest args.
  /// WASM arity for these is `fixed_arity + 1` (the rest list pointer).
  fn_has_rest: HashMap<String, u32>,
  /// Internal native signatures keyed by WASM function index.
  fn_signatures: HashMap<u32, InternalFnSig>,
  /// Native value representation for WASM locals that do not use the f64 slot ABI.
  local_reprs: HashMap<u32, NativeValRepr>,
  /// Whether the module may contain heap-boxed JSON scalars.
  has_boxed_json_scalars: bool,
  /// Internal runtime helper name → function index.
  runtime_fn_index: HashMap<String, u32>,
  /// Tag name → integer ID map (compile-time constant, shared across all functions)
  tag_index: HashMap<String, u32>,
  /// Stable definition identities, display names, and field layouts.
  struct_layouts: StructLayouts,
  /// Compact nominal Enum variant descriptors (`layout_id >= 1`).
  enum_layouts: EnumLayouts,
  /// Whole-program canonical `TypeId` table (`1..=N`, `0` = never-boxed).
  any_type_table: any_box::AnyTypeTable,
  /// Top-level nominal `ns/def` definitions touched by dynamic/local `impl-traits`.
  non_coherent_nominal_defs: Arc<HashSet<String>>,
  /// Current block nesting depth relative to the recur loop
  /// (0 = directly inside the loop, 1 = inside one if/block, etc.)
  block_depth: u32,
  /// qualified atom names whose unsupported initializers have been overwritten by a dominating `reset!`.
  reset_initialized_atoms: HashSet<String>,
  /// String literal pool: string content → logical pointer (f64).
  /// Strings are pre-allocated in a data segment before the heap.
  string_pool: HashMap<String, u32>,
  /// qualified atom name ("ns/def") → WASM global index
  atom_globals: HashMap<String, u32>,
  /// qualified atom name ("ns/def") → whether its Ref inner type is a proven scalar
  atom_is_scalar: HashMap<String, bool>,
  /// qualified value name ("ns/def") → preprocessed expression for inlining.
  value_imports: HashMap<String, Calcit>,
  /// Statically resolved function definitions available for closure-argument specialization.
  static_fn_defs: HashMap<String, Arc<StaticFnDef>>,
  /// Registry for object-safe `(Dyn Trait)` fat pointers and `BrTable` dispatch trampolines.
  dyn_traits: Rc<RefCell<dyn_trait::DynTraitRegistry>>,
  /// Registry for static closure singletons, lifted closure bodies, and `__closure_call_<arity>` trampolines.
  closures: Rc<RefCell<closure::ClosureRegistry>>,
  /// Self-binding for the lifted recursive closure currently being compiled: `(name, body_fn_idx, shape)`.
  self_closure: Option<(String, u32, closure::ClosureParamShape)>,
  /// Target-approved host imports keyed by `(module, field)`.
  host_imports: HashMap<(String, String), u32>,
  /// Selected output ABI. Surface Calcit calls stay target-independent.
  target: WasmTarget,
  boundary: WasmBoundary,
  world: WasiWorld,
  /// Statically known closures retain the lexical local bindings visible at creation.
  /// They are specialized at known call sites instead of receiving a dynamic heap ABI.
  lambda_locals: HashMap<String, Arc<InlineClosure>>,
  /// Active static specializations, used to reject recursive inlining deterministically.
  specialization_stack: Vec<String>,
  /// Value locals whose attached impls are one of a finite set of actual
  /// identities, chosen at runtime by an i32 selector local.
  impl_selectors: HashMap<u32, ImplSelector>,
  /// Per loop parameter: the impl alternatives carried across back edges.
  recur_selectors: Vec<Option<ImplSelector>>,
  /// Back-edge identities missing from the current loop's alternatives.
  recur_missing: Vec<(usize, WasmTypeEvidence)>,
  /// Parameters whose initial scalar origin was invalidated on a specialized back edge.
  recur_drop_scalar_origin: Vec<usize>,
  /// True while compiling a loop body inside `emit_selected_recur_loop`.
  in_selected_recur_loop: bool,
  /// Tracks expanded `DataShapeGraph` node visits during a single map codec emission.
  map_codec_expanded_nodes: usize,
}

/// An i32 local selecting one entry of `alternatives` for a value local.
/// Each alternative is exact evidence of a nominal value's attached impls.
#[derive(Clone)]
struct ImplSelector {
  local: u32,
  alternatives: Vec<WasmTypeEvidence>,
}

impl ImplSelector {
  fn index_of(&self, evidence: &WasmTypeEvidence) -> Option<usize> {
    self.alternatives.iter().position(|alternative| alternative == evidence)
  }
}

/// Internal marker, never surfaced: the loop is retried with wider
/// alternatives once a back edge reveals another impl.
const RECUR_IMPL_RETRY: &str = "RETRY_RECUR_IMPL_SELECTION";
/// Internal marker, never surfaced: the loop is retried with scalar origin cleared
/// once a back edge changes the runtime origin of an open loop parameter.
const RECUR_SCALAR_ORIGIN_RETRY: &str = "RETRY_RECUR_SCALAR_ORIGIN";
/// Ends the error for a loop parameter past `MAX_IMPL_ALTERNATIVES`.
const IMPL_LIMIT: &str = "implementation identities";
/// Bound the code duplicated per dispatch on a selected value.
const MAX_IMPL_ALTERNATIVES: usize = 8;

impl WasmGenCtx {
  fn record_function_value(&mut self, qualified: &str) {
    if let Some(&target) = self.fn_index.get(qualified) {
      self.record_function_value_idx(target);
    }
  }

  fn record_function_value_idx(&mut self, target: u32) {
    if let Some(source) = self.source_fn_index {
      self
        .function_value_dependencies
        .borrow_mut()
        .entry(source)
        .or_default()
        .insert(target);
    }
  }

  fn new(num_params: u32, env: WasmCompileEnv) -> Self {
    WasmGenCtx {
      source_fn_index: None,
      function_value_dependencies: env.function_value_dependencies,
      source_ns: crate::wasm::host::calcit::CORE_NS.to_owned(),
      locals: HashMap::new(),
      local_types: HashMap::new(),
      local_function_values: HashMap::new(),
      local_number_literals: HashMap::new(),
      extra_locals: Vec::new(),
      next_local: num_params,
      uses_recur: false,
      arg_indices: Vec::new(),
      recur_shape: None,
      recur_evidence: None,
      instructions: Vec::new(),
      fn_index: env.fn_index,
      fn_arity: env.fn_arity,
      fn_has_rest: env.fn_has_rest,
      fn_signatures: env.fn_signatures,
      local_reprs: HashMap::new(),
      has_boxed_json_scalars: env.has_boxed_json_scalars,
      runtime_fn_index: env.runtime_fn_index,
      tag_index: env.tag_index,
      struct_layouts: env.struct_layouts,
      enum_layouts: env.enum_layouts,
      any_type_table: env.any_type_table,
      non_coherent_nominal_defs: env.non_coherent_nominal_defs,
      block_depth: 0,
      string_pool: env.string_pool,
      atom_globals: env.atom_globals,
      atom_is_scalar: env.atom_is_scalar,
      reset_initialized_atoms: HashSet::new(),
      value_imports: env.value_imports,
      static_fn_defs: env.static_fn_defs,
      dyn_traits: env.dyn_traits,
      closures: env.closures,
      self_closure: None,
      host_imports: env.host_imports,
      target: env.target,
      boundary: env.boundary,
      world: env.world,
      lambda_locals: HashMap::new(),
      specialization_stack: vec![],
      impl_selectors: HashMap::new(),
      recur_selectors: vec![],
      recur_missing: vec![],
      recur_drop_scalar_origin: vec![],
      in_selected_recur_loop: false,
      map_codec_expanded_nodes: 0,
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
    self.declare_local_with_repr(name, NativeValRepr::F64)
  }

  fn local_repr(&self, index: u32) -> NativeValRepr {
    self.local_reprs.get(&index).copied().unwrap_or(NativeValRepr::F64)
  }

  fn declare_local_with_repr(&mut self, name: &str, repr: NativeValRepr) -> u32 {
    let idx = self.alloc_local_typed(repr.val_type());
    self.locals.insert(name.to_owned(), idx);
    if repr != NativeValRepr::F64 {
      self.local_reprs.insert(idx, repr);
    }
    idx
  }

  fn emit_local_as_f64(&mut self, index: u32) {
    self.emit_local_as_repr(index, NativeValRepr::F64);
  }

  fn emit_local_as_repr(&mut self, index: u32, dst: NativeValRepr) {
    let src = self.local_repr(index);
    self.emit(Instruction::LocalGet(index));
    emit_convert_repr(self, src, dst);
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

fn inline_option_types(ctx: &WasmGenCtx, expr: &Calcit, params: usize, shape: (usize, bool)) -> Vec<Arc<CalcitTypeAnnotation>> {
  if shape == (params, false) {
    infer_wasm_static_type(ctx, expr)
      .and_then(|annotation| annotation.resolve_to_fn())
      .map(|signature| {
        let count = crate::wasm::host::calcit::trailing_option_arg_count(&signature.arg_types, params);
        signature.arg_types.iter().skip(params - count).cloned().collect::<Vec<_>>()
      })
      .unwrap_or_default()
  } else {
    vec![]
  }
}

fn capture_inline_closure(ctx: &WasmGenCtx, expr: &Calcit) -> Option<Arc<InlineClosure>> {
  capture_inline_closure_in_scope(ctx, expr, &ctx.local_types, &ctx.lambda_locals)
}

fn capture_inline_closure_in_scope(
  ctx: &WasmGenCtx,
  expr: &Calcit,
  local_types: &HashMap<String, WasmTypeEvidence>,
  closures: &HashMap<String, Arc<InlineClosure>>,
) -> Option<Arc<InlineClosure>> {
  let (params, body) = try_extract_inline_lambda(expr)?;
  let (required, rest) = inline_lambda_shape(expr)?;
  let option_types = inline_option_types(ctx, expr, params.len(), (required, rest));
  let self_name = match expr {
    Calcit::Fn { info, .. } => {
      let n = info.name.as_ref();
      if n.is_empty() || n.starts_with("fn%") {
        None
      } else {
        Some(n.to_owned())
      }
    }
    Calcit::List(items) if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) => match items.get(1) {
      Some(Calcit::Symbol { sym, .. } | Calcit::Local(CalcitLocal { sym, .. })) => {
        let n = sym.as_ref();
        if n.is_empty() || n.starts_with("fn%") {
          None
        } else {
          Some(n.to_owned())
        }
      }
      _ => None,
    },
    _ => None,
  };
  Some(Arc::new(InlineClosure {
    required: required - option_types.len(),
    params,
    rest,
    option_types,
    body,
    captured_locals: ctx.locals.clone(),
    captured_local_types: local_types.clone(),
    captured_closures: closures.clone(),
    self_name,
    materialized_local: None,
    captured_self_closure: ctx.self_closure.as_ref().map(|s| Arc::new(s.clone())),
  }))
}

/// The `(required, rest)` parameter shape of a lambda accepted by
/// `try_extract_inline_lambda`, which drops the `?` / `&` markers from its
/// parameter names. Native binding makes every parameter after `?` optional
/// and binds the single parameter after `&` to the remaining arguments.
/// Returns `None` for a marker layout native binding rejects.
fn inline_lambda_shape(expr: &Calcit) -> Option<(usize, bool)> {
  let labels: Vec<CalcitArgLabel> = match expr {
    Calcit::Fn { info, .. } => match info.args.as_ref() {
      CalcitFnArgs::Args(idxs) => return Some((idxs.len(), false)),
      CalcitFnArgs::MarkedArgs(labels) => labels.clone(),
    },
    Calcit::List(items) => match items.get(2) {
      Some(Calcit::List(args)) => args
        .iter()
        .map(|arg| match arg {
          Calcit::Local(CalcitLocal { sym, .. }) | Calcit::Symbol { sym, .. } => Some(match sym.as_ref() {
            "&" => CalcitArgLabel::RestMark,
            "?" => CalcitArgLabel::OptionalMark,
            _ => CalcitArgLabel::Idx(0),
          }),
          Calcit::Syntax(CalcitSyntax::ArgSpread, _) => Some(CalcitArgLabel::RestMark),
          Calcit::Syntax(CalcitSyntax::ArgOptional, _) => Some(CalcitArgLabel::OptionalMark),
          _ => None,
        })
        .collect::<Option<Vec<_>>>()?,
      _ => return None,
    },
    _ => return None,
  };
  arg_labels_shape(&labels)
}

/// The `(required, rest)` shape of a marked parameter list; see
/// `inline_lambda_shape`.
fn arg_labels_shape(labels: &[CalcitArgLabel]) -> Option<(usize, bool)> {
  let (mut required, mut optional, mut rest, mut rest_params) = (0, false, false, 0);
  for label in labels {
    match label {
      CalcitArgLabel::Idx(_) if rest => rest_params += 1,
      CalcitArgLabel::Idx(_) if !optional => required += 1,
      CalcitArgLabel::Idx(_) => {}
      CalcitArgLabel::OptionalMark if !rest => optional = true,
      CalcitArgLabel::RestMark if !rest => rest = true,
      _ => return None,
    }
  }
  (!rest || rest_params == 1).then_some((required, rest))
}

type InlineBinding = (InlineArgument, Option<WasmTypeEvidence>);

/// Binds call arguments to an inline closure's parameters like native binding:
/// a missing `?` parameter is nil and a `&` parameter receives the trailing
/// arguments as a list. Returns one binding per parameter.
fn bind_inline_closure_args(
  ctx: &mut WasmGenCtx,
  closure: &InlineClosure,
  mut args: Vec<InlineBinding>,
) -> Result<Vec<InlineBinding>, String> {
  if !closure.accepts(args.len()) {
    return Err(format!(
      "inline closure expects {} argument(s), got {}",
      closure.describe_arity(),
      args.len()
    ));
  }
  let positional = closure.positional();
  let trailing = if args.len() > positional {
    args.split_off(positional)
  } else {
    vec![]
  };
  while args.len() < positional {
    let evidence = if args.len() >= positional - closure.option_types.len() {
      let none = nominal_option_none_expression()?;
      let evidence = infer_wasm_value_evidence(ctx, &none).ok_or("nominal None has no static type evidence")?;
      emit_expr(ctx, &none)?;
      evidence
    } else {
      ctx.emit(f64_const(0.0));
      WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Nil))
    };
    let local = ctx.alloc_local();
    ctx.emit(Instruction::LocalSet(local));
    args.push((InlineArgument::Value(local), Some(evidence)));
  }
  if closure.rest {
    let items = trailing
      .into_iter()
      .map(|(arg, _)| match arg {
        InlineArgument::Value(local) => Ok(local),
        InlineArgument::Closure(c, materialized) => match materialized {
          Some(local) => Ok(local),
          None => closure::emit_materialize_inline_closure(ctx, &c, None),
        },
      })
      .collect::<Result<Vec<_>, String>>()?;
    let list = emit_list_from_locals(ctx, &items);
    args.push((
      InlineArgument::Value(list),
      Some(WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::List(
        DYNAMIC_TYPE.clone(),
      )))),
    ));
  }
  Ok(args)
}

/// Installs bound arguments as the closure's parameters in the current scope.
fn install_inline_closure_bindings(ctx: &mut WasmGenCtx, closure: &InlineClosure, bindings: Vec<InlineBinding>) {
  for (param, (binding, annotation)) in closure.params.iter().zip(bindings) {
    match binding {
      InlineArgument::Value(local) => {
        ctx.lambda_locals.remove(param);
        ctx.locals.insert(param.clone(), local);
        if let Some(annotation) = annotation {
          ctx.local_types.insert(param.clone(), annotation);
        } else {
          ctx.local_types.remove(param);
        }
      }
      InlineArgument::Closure(value, materialized) => {
        if let Some(local) = materialized {
          ctx.locals.insert(param.clone(), local);
        } else {
          ctx.locals.remove(param);
        }
        ctx.local_types.remove(param);
        if closure::inline_closure_requires_materialization(&value) {
          ctx.lambda_locals.remove(param);
        } else {
          ctx.lambda_locals.insert(param.clone(), value);
        }
      }
    }
  }
}

fn resolve_inline_closure(ctx: &WasmGenCtx, expr: &Calcit) -> Option<Arc<InlineClosure>> {
  resolve_inline_closure_in_scope(ctx, expr, &ctx.local_types, &ctx.lambda_locals)
}

fn body_references_definition(exprs: &[Calcit], qualified: &str) -> bool {
  fn visit(expr: &Calcit, qualified: &str) -> bool {
    if direct_definition_reference(expr).as_deref() == Some(qualified) {
      return true;
    }
    match expr {
      Calcit::List(xs) => xs.iter().any(|x| visit(x, qualified)),
      _ => false,
    }
  }
  exprs.iter().any(|e| visit(e, qualified))
}

fn resolve_inline_closure_in_scope(
  ctx: &WasmGenCtx,
  expr: &Calcit,
  local_types: &HashMap<String, WasmTypeEvidence>,
  closures: &HashMap<String, Arc<InlineClosure>>,
) -> Option<Arc<InlineClosure>> {
  match expr {
    Calcit::Local(local) => closures.get(local.sym.as_ref()).cloned(),
    Calcit::Symbol { sym, .. } if closures.contains_key(sym.as_ref()) => closures.get(sym.as_ref()).cloned(),
    Calcit::Symbol { sym, .. } if local_types.contains_key(sym.as_ref()) || ctx.locals.contains_key(sym.as_ref()) => None,
    Calcit::Import(_) | Calcit::Symbol { .. } => {
      let qualified = direct_definition_reference(expr)?;
      if ctx.specialization_stack.iter().any(|active| active == &qualified)
        || (ctx.source_fn_index.is_some() && ctx.source_fn_index == ctx.fn_index.get(&qualified).copied())
      {
        return None;
      }
      let definition = ctx.static_fn_defs.get(&qualified)?;
      if !definition.fixed_arity
        || !(definition_requires_nil_specialization(definition) || definition_requires_trait_specialization(definition))
        || body_references_definition(&definition.body, &qualified)
      {
        return None;
      }
      // A known immutable definition alias can stay compiler-side until its
      // call supplies concrete parameter evidence, instead of materializing an
      // unspecialized function-table entry. It captures no caller bindings.
      let optional = crate::wasm::host::calcit::trailing_option_arg_count(&definition.arg_types, definition.params.len());
      Some(Arc::new(InlineClosure {
        params: definition.params.clone(),
        required: definition.params.len() - optional,
        rest: false,
        option_types: definition
          .arg_types
          .iter()
          .skip(definition.params.len() - optional)
          .cloned()
          .collect(),
        body: definition.body.clone(),
        captured_locals: HashMap::new(),
        captured_local_types: HashMap::new(),
        captured_closures: HashMap::new(),
        ..InlineClosure::default()
      }))
    }
    _ => capture_inline_closure_in_scope(ctx, expr, local_types, closures)
      .or_else(|| resolve_composed_inline_closure_in_scope(ctx, expr, local_types, closures)),
  }
}

fn coerce_to_inline_closure_in_scope(
  ctx: &WasmGenCtx,
  expr: &Calcit,
  arity: usize,
  local_types: &HashMap<String, WasmTypeEvidence>,
  closures: &HashMap<String, Arc<InlineClosure>>,
) -> Option<Arc<InlineClosure>> {
  let (effects, tail) = inline_closure_tail(expr);
  if !effects.is_empty() {
    return None;
  }
  if let Some(closure) = resolve_inline_closure_in_scope(ctx, tail, local_types, closures) {
    return closure.accepts(arity).then_some(closure);
  }
  if let Calcit::Proc(proc) = tail {
    let params = (0..arity).map(|i| format!("__fn_comp_arg_{i}__")).collect::<Vec<_>>();
    let mut call = Vec::with_capacity(arity + 1);
    call.push(Calcit::Proc(*proc));
    for param in &params {
      call.push(Calcit::Local(CalcitLocal {
        idx: 0,
        sym: Arc::from(param.as_str()),
        info: Arc::new(crate::wasm::host::calcit::CalcitSymbolInfo {
          at_ns: Arc::from(crate::wasm::host::calcit::CORE_NS),
          at_def: Arc::from(""),
        }),
        location: None,
        type_info: DYNAMIC_TYPE.clone(),
      }));
    }
    return Some(Arc::new(InlineClosure {
      params,
      required: arity,
      rest: false,
      option_types: vec![],
      body: vec![Calcit::from(call)],
      captured_locals: HashMap::new(),
      captured_local_types: HashMap::new(),
      captured_closures: HashMap::new(),
      ..InlineClosure::default()
    }));
  }
  if let Some(import) = resolve_known_function_value(ctx, tail) {
    let qualified = format!("{}/{}", import.ns, import.def);
    let accepts_arity = match (ctx.fn_arity.get(&qualified), ctx.fn_has_rest.get(&qualified)) {
      (Some(&n), None) => n as usize == arity,
      (_, Some(&fixed)) => (fixed as usize) <= arity,
      _ => false,
    };
    if !accepts_arity {
      return None;
    }
    let params = (0..arity).map(|i| format!("__fn_comp_arg_{i}__")).collect::<Vec<_>>();
    let mut call = Vec::with_capacity(arity + 1);
    call.push(Calcit::Import(import.clone()));
    for param in &params {
      call.push(Calcit::Local(CalcitLocal {
        idx: 0,
        sym: Arc::from(param.as_str()),
        info: Arc::new(crate::wasm::host::calcit::CalcitSymbolInfo {
          at_ns: import.ns.clone(),
          at_def: import.def.clone(),
        }),
        location: None,
        type_info: DYNAMIC_TYPE.clone(),
      }));
    }
    return Some(Arc::new(InlineClosure {
      params,
      required: arity,
      rest: false,
      option_types: vec![],
      body: vec![Calcit::from(call)],
      captured_locals: HashMap::new(),
      captured_local_types: HashMap::new(),
      captured_closures: HashMap::new(),
      ..InlineClosure::default()
    }));
  }
  None
}

fn resolve_composed_inline_closure_in_scope(
  ctx: &WasmGenCtx,
  expr: &Calcit,
  local_types: &HashMap<String, WasmTypeEvidence>,
  closures: &HashMap<String, Arc<InlineClosure>>,
) -> Option<Arc<InlineClosure>> {
  let Calcit::List(items) = expr else {
    return None;
  };
  if items.len() != 3 {
    return None;
  }
  let (target_def, first_arity, second_arity) = match items.first()? {
    Calcit::Import(import)
      if import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS
        && matches!(import.def.as_ref(), "&fn:map" | "&fn:mappend" | "&fn:bind" | "&fn:apply") =>
    {
      let second_arity = if matches!(import.def.as_ref(), "&fn:bind" | "&fn:apply") {
        2
      } else {
        1
      };
      (format!("{}/{}", import.ns, import.def), 1, second_arity)
    }
    Calcit::Symbol { sym, info, .. }
      if info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
        && !local_types.contains_key(sym.as_ref())
        && !ctx.locals.contains_key(sym.as_ref())
        && !closures.contains_key(sym.as_ref())
        && matches!(sym.as_ref(), "&fn:map" | "&fn:mappend" | "&fn:bind" | "&fn:apply") =>
    {
      let second_arity = if matches!(sym.as_ref(), "&fn:bind" | "&fn:apply") { 2 } else { 1 };
      (format!("{}/{}", info.at_ns, sym), 1, second_arity)
    }
    Calcit::Method(name, MethodKind::Invoke(_)) if matches!(name.as_ref(), "map" | "mappend" | "bind" | "apply") => {
      let second_arity = if matches!(name.as_ref(), "bind" | "apply") { 2 } else { 1 };
      (format!("{}/&fn:{}", crate::wasm::host::calcit::CORE_NS, name), 1, second_arity)
    }
    _ => return None,
  };
  let f_closure = coerce_to_inline_closure_in_scope(ctx, items.get(1)?, first_arity, local_types, closures)?;
  let g_closure = coerce_to_inline_closure_in_scope(ctx, items.get(2)?, second_arity, local_types, closures)?;
  let definition = ctx.static_fn_defs.get(&target_def)?;
  if definition.params.len() != 2 {
    return None;
  }
  let inner_lambda = definition.body.last()?;
  let base_closure = capture_inline_closure_in_scope(ctx, inner_lambda, local_types, closures)?;
  let mut composed = (*base_closure).clone();
  for param in &definition.params {
    composed.captured_locals.remove(param);
    composed.captured_local_types.remove(param);
  }
  composed.captured_closures.insert(definition.params[0].clone(), f_closure);
  composed.captured_closures.insert(definition.params[1].clone(), g_closure);
  Some(Arc::new(composed))
}

fn resolve_known_function_value(ctx: &WasmGenCtx, expr: &Calcit) -> Option<CalcitImport> {
  // Closure parameters can shadow captured scalar slots with the same name.
  if matches!(expr, Calcit::Local(local) if ctx.lambda_locals.contains_key(local.sym.as_ref()))
    || matches!(expr, Calcit::Symbol { sym, .. } if ctx.lambda_locals.contains_key(sym.as_ref()))
  {
    return None;
  }
  match expr {
    Calcit::Local(local) => ctx
      .locals
      .get(local.sym.as_ref())
      .and_then(|index| ctx.local_function_values.get(index))
      .cloned(),
    Calcit::Symbol { sym, .. } if ctx.locals.contains_key(sym.as_ref()) => ctx
      .locals
      .get(sym.as_ref())
      .and_then(|index| ctx.local_function_values.get(index))
      .cloned(),
    Calcit::Symbol { sym, info, .. } => {
      let qualified = format!("{}/{}", info.at_ns, sym);
      ctx.fn_index.contains_key(&qualified).then(|| CalcitImport {
        ns: info.at_ns.clone(),
        def: sym.clone(),
        info: Arc::new(ImportInfo::SameFile {
          at_def: info.at_def.clone(),
        }),
        def_id: None,
      })
    }
    Calcit::Import(import) => ctx
      .fn_index
      .contains_key(&format!("{}/{}", import.ns, import.def))
      .then(|| import.clone()),
    Calcit::Fn { info, .. } => info
      .def_ref
      .as_ref()
      .filter(|reference| {
        let qualified = format!("{}/{}", reference.def_ns, reference.def_name);
        let (arity, rest) = compute_fn_arity(info.args.as_ref());
        ctx.fn_index.contains_key(&qualified)
          && ctx.fn_arity.get(&qualified).copied() == Some(arity)
          && ctx.fn_has_rest.get(&qualified).copied() == rest
      })
      .map(|reference| CalcitImport {
        ns: reference.def_ns.clone(),
        def: reference.def_name.clone(),
        info: Arc::new(ImportInfo::SameFile { at_def: info.name.clone() }),
        def_id: None,
      }),
    _ => None,
  }
}

fn inline_closure_tail(expr: &Calcit) -> (Vec<&Calcit>, &Calcit) {
  let mut effects = vec![];
  let mut tail = expr;
  while let Calcit::List(items) = tail {
    // Only unroll sequencing without bindings; named lets may change the closure's captures.
    if !matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)))
      || !matches!(items.get(1), Some(Calcit::List(pair)) if pair.is_empty())
      || items.len() < 3
    {
      break;
    }
    effects.extend(items.iter().skip(2).take(items.len() - 3));
    tail = items.get(items.len() - 1).expect("nonempty CoreLet body");
  }
  (effects, tail)
}

fn emit_inline_closure_body_with_evidences(
  ctx: &mut WasmGenCtx,
  closure: &InlineClosure,
  param_locals: &[u32],
  param_evidences: &[Option<WasmTypeEvidence>],
) -> Result<(), String> {
  if closure.body.iter().any(check_uses_recur) {
    return Err("recur in a statically specialized closure is not yet supported in WASM codegen".into());
  }
  let args = param_locals
    .iter()
    .enumerate()
    .map(|(idx, local)| (InlineArgument::Value(*local), param_evidences.get(idx).and_then(|ev| ev.clone())))
    .collect();
  let bindings = bind_inline_closure_args(ctx, closure, args)?;
  emit_inline_closure_with_bindings(ctx, closure, bindings)
}

fn emit_inline_closure_call(
  ctx: &mut WasmGenCtx,
  closure: &InlineClosure,
  args: &[Calcit],
  expansion_key: Option<&str>,
) -> Result<(), String> {
  if !closure.accepts(args.len()) {
    return Err(format!(
      "inline closure expects {} argument(s), got {}",
      closure.describe_arity(),
      args.len()
    ));
  }
  let uses_recur = closure.body.iter().any(check_uses_recur);
  if uses_recur && (closure.rest || closure.required != closure.params.len()) {
    return Err(
      "recur in a statically specialized closure with optional or rest parameters is not yet supported in WASM codegen".into(),
    );
  }

  let mut bound = Vec::with_capacity(args.len());
  for (index, arg) in args.iter().enumerate() {
    if let Some(arg_closure) = resolve_inline_closure(ctx, arg) {
      let needs_materialization = index < closure.positional()
        && closure.params.get(index).is_some_and(|param| {
          closure::closure_name_may_escape(param, &closure.body) || closure::inline_closure_requires_materialization(&arg_closure)
        });
      if needs_materialization {
        let local = closure::emit_materialize_inline_closure(ctx, &arg_closure, Some(arg))?;
        let mut materialized = (*arg_closure).clone();
        materialized.materialized_local = Some(local);
        bound.push((InlineArgument::Closure(Arc::new(materialized), Some(local)), None));
      } else {
        bound.push((InlineArgument::Closure(arg_closure, None), None));
      }
    } else {
      let annotation = infer_wasm_value_evidence(ctx, arg);
      let function_value = resolve_known_function_value(ctx, arg);
      let number_literal = resolve_known_number_literal(ctx, arg);
      emit_expr(ctx, arg)?;
      let idx = ctx.alloc_local();
      ctx.emit(Instruction::LocalSet(idx));
      // This fresh argument slot belongs to a non-recursive inline invocation;
      // `recur` may rebind it to another value.
      if let Some(definition) = function_value.filter(|_| !uses_recur) {
        ctx.local_function_values.insert(idx, definition);
      }
      if let Some(n) = number_literal.filter(|_| !uses_recur) {
        ctx.local_number_literals.insert(idx, n);
      }
      bound.push((InlineArgument::Value(idx), annotation));
    }
  }
  let bindings = bind_inline_closure_args(ctx, closure, bound)?;
  // Arguments are already evaluated: a nested call there is not recursion in
  // this body. Reuse the existing specialization stack only during expansion.
  if let Some(key) = expansion_key {
    if ctx.specialization_stack.iter().any(|active| active == key) {
      return Err(format!(
        "E_WASM_CALL_SPECIALIZATION: recursive specialization of `{key}` is not supported"
      ));
    }
    ctx.specialization_stack.push(key.to_owned());
  }
  let result = if uses_recur {
    emit_inline_closure_loop(ctx, closure, bindings)
  } else {
    emit_inline_closure_with_bindings(ctx, closure, bindings)
  };
  if expansion_key.is_some() {
    ctx.specialization_stack.pop();
  }
  result
}

/// Emit a fixed-arity recursive closure body as a specialized loop whose
/// parameters start from the call's argument evidence.
fn emit_inline_closure_loop(ctx: &mut WasmGenCtx, closure: &InlineClosure, bindings: Vec<InlineBinding>) -> Result<(), String> {
  reject_non_tail_recur(&closure.body, "closure")?;
  let mut parameter_locals = Vec::with_capacity(bindings.len());
  let mut argument_types = Vec::with_capacity(bindings.len());
  for (index, (binding, evidence)) in bindings.iter().enumerate() {
    match (binding, evidence) {
      (InlineArgument::Value(local), Some(evidence)) => {
        parameter_locals.push(*local);
        argument_types.push(evidence.clone());
      }
      _ => {
        return Err(format!(
          "E_WASM_CALL_SPECIALIZATION: recur in a statically specialized closure requires value evidence for argument {}",
          index + 1
        ));
      }
    }
  }
  scoped_inline_closure(ctx, closure, bindings, |ctx| {
    emit_selected_recur_loop(ctx, &closure.params, parameter_locals, argument_types, &closure.body)
  })
}

fn emit_inline_closure_with_bindings(
  ctx: &mut WasmGenCtx,
  closure: &InlineClosure,
  bindings: Vec<InlineBinding>,
) -> Result<(), String> {
  scoped_inline_closure(ctx, closure, bindings, |ctx| emit_body(ctx, &closure.body))
}

/// Run `emit` in the closure's captured scope with its parameters bound.
fn scoped_inline_closure(
  ctx: &mut WasmGenCtx,
  closure: &InlineClosure,
  bindings: Vec<InlineBinding>,
  emit: impl FnOnce(&mut WasmGenCtx) -> Result<(), String>,
) -> Result<(), String> {
  let caller_locals = std::mem::replace(&mut ctx.locals, closure.captured_locals.clone());
  let caller_local_types = std::mem::replace(&mut ctx.local_types, closure.captured_local_types.clone());
  let caller_closures = std::mem::replace(&mut ctx.lambda_locals, closure.captured_closures.clone());
  install_inline_closure_bindings(ctx, closure, bindings);
  let result = emit(ctx);
  ctx.locals = caller_locals;
  ctx.local_types = caller_local_types;
  ctx.lambda_locals = caller_closures;
  result
}

fn infer_wasm_static_type(ctx: &WasmGenCtx, expr: &Calcit) -> Option<Arc<CalcitTypeAnnotation>> {
  infer_wasm_value_evidence(ctx, expr).map(|evidence| evidence.annotation)
}

fn infer_wasm_codec_hint(ctx: &WasmGenCtx, expr: &Calcit) -> Option<Arc<CalcitTypeAnnotation>> {
  infer_wasm_value_evidence(ctx, expr).map(|evidence| {
    let ty = evidence.codec_hint.unwrap_or(evidence.annotation);
    expand_wasm_value_type(&ty)
  })
}

fn infer_wasm_value_evidence(ctx: &WasmGenCtx, expr: &Calcit) -> Option<WasmTypeEvidence> {
  infer_wasm_evidence_in_scope(ctx, expr, &ctx.local_types, &ctx.lambda_locals, &mut HashSet::new())
}

fn infer_wasm_evidence(ctx: &WasmGenCtx, expr: &Calcit) -> WasmTypeEvidence {
  infer_wasm_value_evidence(ctx, expr).unwrap_or_else(|| WasmTypeEvidence::contract(DYNAMIC_TYPE.clone()))
}

fn is_option_none_expr(expr: &Calcit) -> bool {
  match expr {
    Calcit::Import(CalcitImport { ns, def, .. }) => ns.as_ref() == "calcit.core" && def.as_ref() == "None",
    Calcit::List(xs) if matches!(xs.first(), Some(Calcit::Proc(CalcitProc::NativeNamedEnumNew))) => {
      matches!(xs.get(2), Some(Calcit::Tag(tag)) if tag.ref_str() == "none")
    }
    _ => false,
  }
}

fn option_some_payload_expr(expr: &Calcit) -> Option<&Calcit> {
  match expr {
    Calcit::List(xs) if matches!(xs.first(), Some(Calcit::Proc(CalcitProc::NativeNamedEnumNew))) => {
      if matches!(xs.get(2), Some(Calcit::Tag(tag)) if tag.ref_str() == "some") {
        xs.get(3)
      } else {
        None
      }
    }
    Calcit::List(xs) => match xs.first() {
      Some(Calcit::Import(CalcitImport { ns, def, .. })) if ns.as_ref() == "calcit.core" && def.as_ref() == "Some" => xs.get(1),
      Some(Calcit::Symbol { sym, info, .. }) if info.at_ns.as_ref() == "calcit.core" && sym.as_ref() == "Some" => xs.get(1),
      _ => None,
    },
    _ => None,
  }
}

fn infer_literal_tag_map_codec_hint(
  ctx: &WasmGenCtx,
  pairs: &[(Calcit, Calcit)],
  annotation: &Arc<CalcitTypeAnnotation>,
  local_types: &HashMap<String, WasmTypeEvidence>,
  closures: &HashMap<String, Arc<InlineClosure>>,
  visiting: &mut HashSet<String>,
) -> Option<Arc<CalcitTypeAnnotation>> {
  if pairs.is_empty() || !pairs.iter().all(|(k, _)| matches!(k, Calcit::Tag(_))) {
    return None;
  }
  let mut fields: Vec<cirru_edn::EdnTag> = Vec::new();
  let mut field_types: Vec<Arc<CalcitTypeAnnotation>> = Vec::new();
  let mut any_val_hint = false;
  for (k_expr, v_expr) in pairs {
    let Calcit::Tag(tag) = k_expr else { continue };
    let v_ev = infer_wasm_evidence_in_scope(ctx, v_expr, local_types, closures, visiting);
    if v_ev.as_ref().is_some_and(|ev| ev.codec_hint.is_some()) {
      any_val_hint = true;
    }
    let val_ty = v_ev
      .map(|ev| expand_wasm_value_type(ev.codec_hint.as_ref().unwrap_or(&ev.annotation)))
      .unwrap_or_else(|| DYNAMIC_TYPE.clone());
    if let Some(pos) = fields.iter().position(|f| f == tag) {
      field_types[pos] = val_ty;
    } else {
      fields.push(tag.clone());
      field_types.push(val_ty);
    }
  }
  let is_dynamic_val = matches!(
    expand_wasm_value_type(annotation).as_ref(),
    CalcitTypeAnnotation::Map(_, v) if matches!(v.as_ref(), CalcitTypeAnnotation::Dynamic)
  );
  let heterogeneous = field_types.first().is_some_and(|first| field_types.iter().any(|t| t != first));
  if !(any_val_hint || is_dynamic_val || heterogeneous) {
    return None;
  }
  let synth = Arc::new(CalcitStructDef {
    definition_ref: None,
    name: cirru_edn::EdnTag::from("__wasm_literal_map__"),
    fields: Arc::new(fields),
    field_types: Arc::new(field_types),
    generics: Arc::new(vec![]),
    where_bounds: Arc::new(vec![]),
    field_defaults: Arc::new(vec![]),
    impls: vec![],
  });
  Some(Arc::new(CalcitTypeAnnotation::Map(
    Arc::new(CalcitTypeAnnotation::StructValue(synth.clone())),
    Arc::new(CalcitTypeAnnotation::Struct(synth, Arc::new(vec![]))),
  )))
}

fn prove_wasm_arg_bindings(actual: &Arc<CalcitTypeAnnotation>, expected: &CalcitTypeAnnotation, bindings: &mut TypeBindings) -> bool {
  let mut trial = bindings.clone();
  if !actual.prove_available_bindings(expected, &mut trial).is_mismatch() {
    *bindings = trial;
    return true;
  }
  if matches!(actual.as_ref(), CalcitTypeAnnotation::Tuple(_)) {
    let mut list_trial = bindings.clone();
    let open_list = Arc::new(CalcitTypeAnnotation::List(DYNAMIC_TYPE.clone()));
    if !open_list.prove_available_bindings(expected, &mut list_trial).is_mismatch() {
      *bindings = list_trial;
      return true;
    }
  }
  false
}

fn data_shape_node_to_annotation(graph: &DataShapeGraph, node_id: usize, depth: usize) -> Arc<CalcitTypeAnnotation> {
  if depth >= 32 {
    return DYNAMIC_TYPE.clone();
  }
  let Some(node) = graph.nodes.get(node_id) else {
    return DYNAMIC_TYPE.clone();
  };
  match node {
    DataShapeNode::Dynamic => DYNAMIC_TYPE.clone(),
    DataShapeNode::Nil => Arc::new(CalcitTypeAnnotation::Nil),
    DataShapeNode::Unit => Arc::new(CalcitTypeAnnotation::Unit),
    DataShapeNode::Bool => Arc::new(CalcitTypeAnnotation::Bool),
    DataShapeNode::Number => Arc::new(CalcitTypeAnnotation::Number),
    DataShapeNode::Numeric(kind) => Arc::new(CalcitTypeAnnotation::Numeric(*kind)),
    DataShapeNode::String => Arc::new(CalcitTypeAnnotation::String),
    DataShapeNode::Symbol => Arc::new(CalcitTypeAnnotation::Symbol),
    DataShapeNode::Tag => Arc::new(CalcitTypeAnnotation::Tag),
    DataShapeNode::Buffer => Arc::new(CalcitTypeAnnotation::Buffer),
    DataShapeNode::CirruQuote => Arc::new(CalcitTypeAnnotation::CirruQuote),
    DataShapeNode::FfiResource => Arc::new(CalcitTypeAnnotation::FfiResource),
    DataShapeNode::List(inner) => Arc::new(CalcitTypeAnnotation::List(data_shape_node_to_annotation(graph, *inner, depth + 1))),
    DataShapeNode::Tuple(items) => Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(
      items
        .iter()
        .map(|item| data_shape_node_to_annotation(graph, *item, depth + 1))
        .collect(),
    ))),
    DataShapeNode::Set(inner) => Arc::new(CalcitTypeAnnotation::Set(data_shape_node_to_annotation(graph, *inner, depth + 1))),
    DataShapeNode::Map { key, value } => Arc::new(CalcitTypeAnnotation::Map(
      data_shape_node_to_annotation(graph, *key, depth + 1),
      data_shape_node_to_annotation(graph, *value, depth + 1),
    )),
    DataShapeNode::Ref(inner) => Arc::new(CalcitTypeAnnotation::Ref(data_shape_node_to_annotation(graph, *inner, depth + 1))),
    DataShapeNode::MapOption { nominal, inner, .. } => Arc::new(CalcitTypeAnnotation::Enum(
      nominal.clone(),
      Arc::new(vec![data_shape_node_to_annotation(graph, *inner, depth + 1)]),
    )),
    DataShapeNode::Struct { nominal, type_args, .. } => {
      if type_args.is_empty() {
        Arc::new(CalcitTypeAnnotation::StructValue(nominal.clone()))
      } else {
        Arc::new(CalcitTypeAnnotation::Struct(nominal.clone(), Arc::new(type_args.to_vec())))
      }
    }
    DataShapeNode::Enum { nominal, type_args, .. } => {
      Arc::new(CalcitTypeAnnotation::Enum(nominal.clone(), Arc::new(type_args.to_vec())))
    }
    DataShapeNode::Union { name, nominal_path, .. } => {
      if let Some((ns, def)) = nominal_path {
        Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from(format!("{ns}/{def}")), Arc::new(vec![])))
      } else {
        Arc::new(CalcitTypeAnnotation::TypeRef(name.clone(), Arc::new(vec![])))
      }
    }
  }
}

fn resolve_codec_target_annotation(items: &crate::wasm::host::calcit::CalcitList) -> Option<Arc<CalcitTypeAnnotation>> {
  items
    .get(3)
    .and_then(DataShapeGraph::from_calcit_handle)
    .map(|graph| data_shape_node_to_annotation(&graph, graph.root, 0))
    .or_else(|| {
      items
        .get(2)
        .map(|form| CalcitTypeAnnotation::parse_type_annotation_form_with_generics(form, &[]))
    })
}

fn resolved_struct_fields_for_annotation(
  annotation: &Arc<CalcitTypeAnnotation>,
) -> Option<Vec<(cirru_edn::EdnTag, Arc<CalcitTypeAnnotation>)>> {
  let expanded = expand_wasm_value_type(annotation);
  match expanded.as_ref() {
    CalcitTypeAnnotation::Struct(nominal, args) => edn::resolved_struct_fields(nominal, args).ok(),
    CalcitTypeAnnotation::StructValue(nominal) => edn::resolved_struct_fields(nominal, &[]).ok(),
    CalcitTypeAnnotation::TypeRef(_, args) => expanded
      .resolve_to_struct()
      .and_then(|nominal| edn::resolved_struct_fields(&nominal, args).ok()),
    _ => None,
  }
}

/// Infer the static call result when a local variable refers to an inline closure,
/// a known non-inline callable target, or a typed function value.
fn infer_local_call_result(
  ctx: &WasmGenCtx,
  name: &str,
  items: &crate::wasm::host::calcit::CalcitList,
  local_types: &HashMap<String, WasmTypeEvidence>,
  closures: &HashMap<String, Arc<InlineClosure>>,
  visiting: &mut HashSet<String>,
) -> Option<WasmTypeEvidence> {
  if let Some(closure) = closures.get(name) {
    return infer_inline_closure_result(ctx, closure, &items.drop_left().to_vec(), local_types, closures, visiting);
  }
  let bound = local_types.get(name)?;
  if let Some(known_fn) = &bound.known_function {
    let mut direct = vec![Calcit::Import(known_fn.clone())];
    direct.extend(items.iter().skip(1).cloned());
    return infer_wasm_evidence_in_scope(ctx, &Calcit::from(direct), local_types, closures, visiting);
  }
  let fn_sig = bound.annotation.resolve_to_fn()?;
  (!fn_sig.return_type.contains_type_var()).then(|| WasmTypeEvidence::contract(fn_sig.return_type.clone()))
}

fn infer_wasm_evidence_in_scope(
  ctx: &WasmGenCtx,
  expr: &Calcit,
  local_types: &HashMap<String, WasmTypeEvidence>,
  closures: &HashMap<String, Arc<InlineClosure>>,
  visiting: &mut HashSet<String>,
) -> Option<WasmTypeEvidence> {
  if expression_definitely_diverges(expr)
    || matches!(expr, Calcit::List(items) if matches!(items.first(), Some(Calcit::Proc(CalcitProc::Recur))))
  {
    return Some(WasmTypeEvidence::contract(
      crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone(),
    ));
  }
  let mut result = match expr {
    Calcit::Local(local) => {
      if let Some(bound) = local_types.get(local.sym.as_ref()) {
        let bound_ty = expand_wasm_value_type(&bound.annotation);
        let local_ty = expand_wasm_value_type(&local.type_info);
        if (nil_type_evidence_is_open(&bound_ty)
          || matches!(
            bound_ty.as_ref(),
            CalcitTypeAnnotation::Nullable(_, _) | CalcitTypeAnnotation::Custom(_)
          ))
          && !nil_type_evidence_is_open(&local_ty)
          && !matches!(
            local_ty.as_ref(),
            CalcitTypeAnnotation::Nullable(_, _) | CalcitTypeAnnotation::Custom(_)
          )
        {
          Some(WasmTypeEvidence {
            annotation: local.type_info.clone(),
            ..bound.clone()
          })
        } else {
          Some(bound.clone())
        }
      } else {
        Some(WasmTypeEvidence {
          known_function: resolve_known_function_value(ctx, expr),
          ..WasmTypeEvidence::contract(local.type_info.clone())
        })
      }
    }
    Calcit::Symbol { sym, .. } if local_types.contains_key(sym.as_ref()) => local_types.get(sym.as_ref()).cloned(),
    Calcit::List(items) if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::If, _))) => {
      if let Some(taken) =
        static_predicate_condition_outcome_in_scope(ctx, items.get(1)?, false, local_types, closures, &HashMap::new())
      {
        return if taken {
          infer_wasm_evidence_in_scope(ctx, items.get(2)?, local_types, closures, visiting)
        } else {
          infer_wasm_evidence_in_scope(ctx, items.get(3).unwrap_or(&Calcit::Unit), local_types, closures, visiting)
        };
      }
      let consequent = infer_wasm_evidence_in_scope(ctx, items.get(2)?, local_types, closures, visiting)?;
      let alternative = infer_wasm_evidence_in_scope(ctx, items.get(3).unwrap_or(&Calcit::Unit), local_types, closures, visiting)?;
      // A data-shape join may discard attached implementations. Preserve a
      // call-site identity only when both live branches carry identical evidence.
      if matches!(alternative.annotation.as_ref(), CalcitTypeAnnotation::Never) {
        Some(consequent)
      } else if matches!(consequent.annotation.as_ref(), CalcitTypeAnnotation::Never) {
        Some(alternative)
      } else if consequent == alternative {
        let codec_hint = if consequent.codec_hint == alternative.codec_hint {
          consequent.codec_hint.clone()
        } else {
          None
        };
        let scalar_origin = if consequent.scalar_origin == alternative.scalar_origin {
          consequent.scalar_origin.clone()
        } else {
          None
        };
        let known_function = if consequent.known_function == alternative.known_function {
          consequent.known_function.clone()
        } else {
          None
        };
        let set_member_types = join_branch_member_types(consequent.set_member_types.clone(), alternative.set_member_types);
        let list_element_types = join_branch_member_types(consequent.list_element_types.clone(), alternative.list_element_types);
        Some(WasmTypeEvidence {
          codec_hint,
          scalar_origin,
          known_function,
          set_member_types,
          list_element_types,
          ..consequent
        })
      } else if let Some(merged) = join_collection_branch_evidences(&consequent, &alternative) {
        Some(merged)
      } else {
        infer_static_type_from_expr(expr).map(WasmTypeEvidence::contract)
      }
    }
    Calcit::List(items) if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::CoreLet, _))) => {
      let Calcit::List(pair) = items.get(1)? else { return None };
      let tail = items.iter().skip(2).last()?;
      if pair.is_empty() {
        return infer_wasm_evidence_in_scope(ctx, tail, local_types, closures, visiting);
      }
      let name = match pair.first()? {
        Calcit::Local(local) => local.sym.as_ref(),
        Calcit::Symbol { sym, .. } => sym.as_ref(),
        _ => return None,
      };
      let annotation = infer_wasm_evidence_in_scope(ctx, pair.get(1)?, local_types, closures, visiting)
        .unwrap_or_else(|| WasmTypeEvidence::contract(DYNAMIC_TYPE.clone()));
      let mut body_types = local_types.clone();
      body_types.insert(name.to_owned(), annotation);
      let mut body_closures = closures.clone();
      if let Some(closure) = resolve_inline_closure_in_scope(ctx, pair.get(1)?, local_types, closures) {
        body_closures.insert(name.to_owned(), closure);
      } else {
        body_closures.remove(name);
      }
      infer_wasm_evidence_in_scope(ctx, tail, &body_types, &body_closures, visiting)
    }
    Calcit::List(items) if matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeStruct))) => items
      .get(1)
      .and_then(|definition| resolve_struct_ref(definition).ok())
      .map(|definition| {
        let mut codec_hint = (!definition.generics.is_empty())
          .then(|| infer_static_type_from_expr(expr))
          .flatten();
        if definition.generics.is_empty()
          && items.len() == 2 + definition.fields.len() * 2
          && definition
            .field_types
            .iter()
            .any(|ty| nil_type_evidence_is_open(&expand_wasm_value_type(ty)))
        {
          let mut refined_field_types = (*definition.field_types).clone();
          let mut refined = false;
          let pair_items: Vec<&Calcit> = items.iter().skip(2).collect();
          for pair in pair_items.as_chunks::<2>().0 {
            if let [Calcit::Tag(tag), val_expr] = *pair
              && let Some(idx) = definition.fields.iter().position(|f| f == tag)
              && nil_type_evidence_is_open(&expand_wasm_value_type(&refined_field_types[idx]))
              && let Some(val_ev) = infer_wasm_evidence_in_scope(ctx, val_expr, local_types, closures, visiting)
            {
              let expanded = expand_wasm_value_type(val_ev.codec_hint.as_ref().unwrap_or(&val_ev.annotation));
              if !nil_type_evidence_is_open(&expanded) {
                refined_field_types[idx] = expanded;
                refined = true;
              }
            }
          }
          if refined {
            let mut refined_def = definition.clone();
            refined_def.field_types = Arc::new(refined_field_types);
            codec_hint = Some(Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(refined_def))));
          }
        }
        WasmTypeEvidence {
          annotation: Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(definition))),
          codec_hint,
          exact_impls: true,
          ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
        }
      }),
    Calcit::List(items) => items
      .first()
      .and_then(|head| match head {
        Calcit::Syntax(CalcitSyntax::AnyBox, _) => Some(WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Any))),
        Calcit::Syntax(CalcitSyntax::AnyDowncast, _) if items.len() >= 3 => {
          let lowered = items
            .get(3)
            .and_then(crate::wasm::host::calcit::CompiledAnyTypeId::from_calcit_handle)
            .is_some();
          let target = if lowered {
            CalcitTypeAnnotation::parse_type_annotation_form_with_generics(&items[2], &[])
          } else {
            let desugared = crate::wasm::host::calcit::type_expr::desugar_quoted_form(&items[2])
              .or_else(|| crate::wasm::host::calcit::type_expr::desugar_bare_form(&items[2]))
              .and_then(Result::ok);
            CalcitTypeAnnotation::parse_type_annotation_form_with_generics(desugared.as_ref().unwrap_or(&items[2]), &[])
          };
          let normalized = CalcitTypeAnnotation::normalize_any_downcast_target(&target);
          Some(WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::TypeRef(
            Arc::from("calcit.core/Option"),
            Arc::new(vec![normalized]),
          ))))
        }
        Calcit::Syntax(CalcitSyntax::Match, _) if items.len() >= 3 => {
          let subject_ev = infer_wasm_evidence_in_scope(ctx, &items[1], local_types, closures, visiting);
          let subject_ty = subject_ev
            .as_ref()
            .map(|ev| CalcitTypeAnnotation::expand_schema_alias(ev.scalar_origin.as_ref().unwrap_or(&ev.annotation)));
          let anon_enum = match &items[1] {
            Calcit::List(xs)
              if xs.len() >= 2 && matches!(xs.first(), Some(Calcit::Proc(CalcitProc::NativeEnum | CalcitProc::NativeEnumNew))) =>
            {
              if let Some(Calcit::Tag(t)) = xs.get(1) {
                let payloads = xs
                  .iter()
                  .skip(2)
                  .map(|p| {
                    infer_wasm_evidence_in_scope(ctx, p, local_types, closures, visiting)
                      .unwrap_or_else(|| WasmTypeEvidence::contract(DYNAMIC_TYPE.clone()))
                  })
                  .collect::<Vec<_>>();
                Some((t.ref_str().to_owned(), payloads))
              } else {
                None
              }
            }
            _ => None,
          };
          let branches: Vec<&Calcit> = if items.len() == 4
            && matches!(&items[2], Calcit::EnumDef(_))
            && let Calcit::List(table) = &items[3]
          {
            table.iter().filter(|branch| !matches!(branch, Calcit::Nil)).collect()
          } else {
            items.iter().skip(2).collect()
          };
          let mut joined: Option<WasmTypeEvidence> = None;
          for branch in branches {
            let Calcit::List(pair) = branch else { continue };
            if pair.len() != 2 {
              continue;
            }
            let pattern = &pair[0];
            let body = &pair[1];
            let mut branch_types = local_types.clone();
            if let Calcit::List(pat_xs) = pattern
              && let Some(Calcit::Tag(pat_tag)) = pat_xs.first()
            {
              let tag_name = pat_tag.ref_str();
              if let Some((anon_tag, anon_payloads)) = &anon_enum
                && anon_tag == tag_name
              {
                for (bind_idx, binding) in pat_xs.iter().skip(1).enumerate() {
                  let bind_name = match binding {
                    Calcit::Local(CalcitLocal { sym, .. }) | Calcit::Symbol { sym, .. } => sym.as_ref(),
                    _ => continue,
                  };
                  if let Some(ev) = anon_payloads.get(bind_idx) {
                    branch_types.insert(bind_name.to_owned(), ev.clone());
                  }
                }
              } else if let Some(ty) = &subject_ty
                && let Some(enum_def) = ty.resolve_to_enum()
                && let Some(variant) = enum_def.find_variant_by_name(tag_name)
              {
                let type_args = match ty.as_ref() {
                  CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args) => args.as_slice(),
                  _ => &[],
                };
                let bindings: TypeBindings = enum_def.generics().iter().cloned().zip(type_args.iter().cloned()).collect();
                for (bind_idx, binding) in pat_xs.iter().skip(1).enumerate() {
                  let bind_name = match binding {
                    Calcit::Local(CalcitLocal { sym, .. }) | Calcit::Symbol { sym, .. } => sym.as_ref(),
                    _ => continue,
                  };
                  if let Some(pt) = variant.payload_types().get(bind_idx) {
                    let sub_pt = replace_unbound_type_vars_with_never(&pt.substitute_type_vars(&bindings));
                    let chosen = resolve_match_payload_type(binding, sub_pt);
                    branch_types.insert(bind_name.to_owned(), WasmTypeEvidence::contract(chosen));
                  }
                }
              }
            }
            if let Some(body_ev) = infer_wasm_evidence_in_scope(ctx, body, &branch_types, closures, visiting) {
              if matches!(body_ev.annotation.as_ref(), CalcitTypeAnnotation::Never) {
                continue;
              }
              match &mut joined {
                None => joined = Some(body_ev),
                Some(prev) if prev.annotation == body_ev.annotation && prev.scalar_origin == body_ev.scalar_origin => {
                  if prev.codec_hint != body_ev.codec_hint {
                    prev.codec_hint = None;
                  }
                  prev.set_member_types = join_branch_member_types(prev.set_member_types.take(), body_ev.set_member_types);
                  prev.list_element_types = join_branch_member_types(prev.list_element_types.take(), body_ev.list_element_types);
                  prev.exact_impls = prev.exact_impls && body_ev.exact_impls;
                }
                Some(prev) => {
                  if let Some(merged) = join_collection_branch_evidences(prev, &body_ev) {
                    *prev = merged;
                  } else {
                    joined = infer_static_type_from_expr(expr).map(WasmTypeEvidence::contract);
                    break;
                  }
                }
              }
            } else {
              joined = infer_static_type_from_expr(expr).map(WasmTypeEvidence::contract);
              break;
            }
          }
          joined.or_else(|| infer_static_type_from_expr(expr).map(WasmTypeEvidence::contract))
        }
        Calcit::Syntax(CalcitSyntax::CallSpread, _) if items.len() >= 2 => {
          let mut plain = vec![items[1].clone()];
          let mut runtime_spreads = vec![];
          expand_literal_spread_args("spread", &items.drop_left().drop_left().to_vec(), &mut plain, &mut runtime_spreads).ok()?;
          if runtime_spreads.is_empty() {
            infer_wasm_evidence_in_scope(ctx, &Calcit::from(plain), local_types, closures, visiting)
          } else if items.len() >= 4
            && !matches!(items.get(2), Some(Calcit::Syntax(CalcitSyntax::ArgSpread, _)))
            && (direct_definition_reference(&items[1]).as_deref() == Some("calcit.core/dissoc")
              || matches!(items[1], Calcit::Proc(CalcitProc::NativeMapDissoc)))
            && let Some(base_ev) = infer_wasm_evidence_in_scope(ctx, &items[2], local_types, closures, visiting)
            && matches!(
              expand_wasm_value_type(&base_ev.annotation).as_ref(),
              CalcitTypeAnnotation::Map(_, _)
            )
          {
            Some(WasmTypeEvidence {
              annotation: base_ev.annotation,
              set_member_types: base_ev.set_member_types,
              ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
            })
          } else {
            None
          }
        }
        Calcit::Syntax(CalcitSyntax::UnsafeCoerce, _) if items.len() == 3 => {
          let target = CalcitTypeAnnotation::parse_type_annotation_form(&items[2]);
          let inner = infer_wasm_evidence_in_scope(ctx, &items[1], local_types, closures, visiting)?;
          if matches!(target.as_ref(), CalcitTypeAnnotation::Dynamic) {
            Some(inner)
          } else if inner.annotation == target || inner.annotation.is_proven_for(&target) {
            Some(WasmTypeEvidence {
              annotation: target,
              ..inner
            })
          } else {
            Some(WasmTypeEvidence::contract(target))
          }
        }
        Calcit::Syntax(CalcitSyntax::AssertType, _) if items.len() == 3 => {
          let target = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(&items[2], &[]);
          let inner = infer_wasm_evidence_in_scope(ctx, &items[1], local_types, closures, visiting);
          let expanded_target = CalcitTypeAnnotation::expand_schema_alias(&target);
          let scalar_origin = inner.as_ref().and_then(|ev| {
            ev.scalar_origin.clone().or_else(|| {
              let inner_ty = CalcitTypeAnnotation::expand_schema_alias(&ev.annotation);
              if matches!(expanded_target.as_ref(), CalcitTypeAnnotation::Dynamic) {
                proven_disjoint_value_kind(&inner_ty).map(|_| inner_ty)
              } else if expanded_target.resolve_to_enum().is_some_and(|ed| ed.name().ref_str() == "Option") {
                Some(inner_ty)
              } else {
                None
              }
            })
          });
          let set_member_types = inner.as_ref().and_then(|ev| ev.set_member_types.clone()).or_else(|| {
            if matches!(
              expanded_target.as_ref(),
              CalcitTypeAnnotation::Map(_, _) | CalcitTypeAnnotation::Set(_)
            ) && matches!(
              &items[1],
              Calcit::List(xs) if xs.len() == 1 && matches!(xs.first(), Some(Calcit::Proc(CalcitProc::NativeMap | CalcitProc::Set)))
            ) {
              Some(vec![])
            } else {
              None
            }
          });
          let known_function = inner.as_ref().and_then(|ev| ev.known_function.clone());
          Some(WasmTypeEvidence {
            annotation: target,
            codec_hint: inner.as_ref().and_then(|ev| ev.codec_hint.clone()),
            set_member_types,
            list_element_types: None,
            scalar_origin,
            known_function,
            exact_impls: inner.is_some_and(|ev| ev.exact_impls),
          })
        }
        Calcit::Syntax(CalcitSyntax::DynBox, _) if items.len() == 3 => {
          let file_ns = ctx
            .specialization_stack
            .last()
            .and_then(|qualified| qualified.rsplit_once('/').map(|(ns, _)| ns))
            .unwrap_or(&ctx.source_ns);
          dyn_trait::resolve_wasm_trait_def(&items[2], file_ns)
            .ok()
            .map(|trait_def| WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::DynTrait(trait_def))))
        }
        Calcit::Syntax(CalcitSyntax::EncodeMapAs, _) => {
          let target = resolve_codec_target_annotation(items)?;
          let expanded_target = expand_wasm_value_type(&target);
          let mut annotation = infer_encoded_map_type(&target);
          let mut codec_hint = infer_encoded_codec_hint(&target);
          if let Some(target_def) = expanded_target.resolve_to_struct()
            && target_def.generics.is_empty()
            && let Some(input_expr) = items.get(1)
            && let Some(input_ev) = infer_wasm_evidence_in_scope(ctx, input_expr, local_types, closures, visiting)
            && let Some(hint) = input_ev.codec_hint.as_ref()
            && let Some(hint_def) = hint.resolve_to_struct()
            && hint_def.name == target_def.name
            && hint_def.fields == target_def.fields
          {
            codec_hint = infer_encoded_codec_hint(hint);
          }
          if let Some(enum_def) = expanded_target.resolve_to_enum()
            && enum_def.name().ref_str() == "Option"
            && let CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args) = expanded_target.as_ref()
            && args.len() == 1
            && let Some(input_expr) = items.get(1)
          {
            let inner_option = args[0]
              .resolve_to_enum()
              .is_some_and(|inner_def| inner_def.name().ref_str() == "Option");
            if !inner_option {
              if is_option_none_expr(input_expr) {
                annotation = expanded_target.clone();
              } else if let Some(payload_expr) = option_some_payload_expr(input_expr) {
                let expanded_arg0 = expand_wasm_value_type(&args[0]);
                if matches!(expanded_arg0.as_ref(), CalcitTypeAnnotation::Dynamic) {
                  if let Some(payload_ev) = infer_wasm_evidence_in_scope(ctx, payload_expr, local_types, closures, visiting) {
                    let payload_is_option = payload_ev
                      .annotation
                      .resolve_to_enum()
                      .is_some_and(|e| e.name().ref_str() == "Option");
                    if payload_is_option {
                      annotation = expanded_target.clone();
                    } else {
                      annotation = infer_encoded_map_type(&payload_ev.annotation);
                      codec_hint = infer_encoded_codec_hint(payload_ev.codec_hint.as_ref().unwrap_or(&payload_ev.annotation));
                    }
                  }
                } else {
                  annotation = infer_encoded_map_type(&args[0]);
                  codec_hint = infer_encoded_codec_hint(&args[0]);
                }
              }
            }
          }
          Some(WasmTypeEvidence {
            annotation,
            codec_hint: Some(codec_hint),
            ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
          })
        }
        Calcit::Syntax(CalcitSyntax::DecodeMapAs | CalcitSyntax::ParseCirruEdnAs, _) => {
          let target = resolve_codec_target_annotation(items)?;
          Some(WasmTypeEvidence::contract(target))
        }
        Calcit::Syntax(CalcitSyntax::TryDecodeMapAs | CalcitSyntax::TryParseCirruEdnAs, _) => {
          let target = resolve_codec_target_annotation(items)?;
          Some(WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::TypeRef(
            Arc::from("calcit.core/Result"),
            Arc::new(vec![target, Arc::new(CalcitTypeAnnotation::String)]),
          ))))
        }
        Calcit::Method(name, MethodKind::Invoke(_)) => {
          let receiver = infer_wasm_evidence_in_scope(ctx, items.get(1)?, local_types, closures, visiting)?;
          let callable = resolve_runtime_method_callable(
            &receiver.annotation,
            has_effective_exact_impls(ctx, &receiver),
            name,
            &ctx.source_ns,
          )
          .ok()??;
          let mut direct = vec![callable];
          direct.extend(items.iter().skip(1).cloned());
          infer_wasm_evidence_in_scope(ctx, &Calcit::from(direct), local_types, closures, visiting)
        }
        Calcit::Local(local) => infer_local_call_result(ctx, local.sym.as_ref(), items, local_types, closures, visiting),
        Calcit::Symbol { sym, .. } if closures.contains_key(sym.as_ref()) || local_types.contains_key(sym.as_ref()) => {
          infer_local_call_result(ctx, sym.as_ref(), items, local_types, closures, visiting)
        }
        head
          if (matches!(head, Calcit::List(_)) || matches!(head, Calcit::Fn { .. })) && direct_definition_reference(head).is_none() =>
        {
          let closure = resolve_inline_closure_in_scope(ctx, head, local_types, closures)?;
          infer_inline_closure_result(ctx, &closure, &items.drop_left().to_vec(), local_types, closures, visiting)
        }
        Calcit::Import(_) | Calcit::Symbol { .. } | Calcit::Fn { .. } => {
          let aliased_proc = match head {
            Calcit::Import(import) => {
              let qualified = format!("{}/{}", import.ns, import.def);
              if import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS && import.def.as_ref() == "'" {
                Some(CalcitProc::List)
              } else if let Some(Calcit::Proc(proc)) = ctx.value_imports.get(&qualified) {
                Some(*proc)
              } else {
                None
              }
            }
            Calcit::Symbol { sym, info, .. }
              if !local_types.contains_key(sym.as_ref())
                && !ctx.locals.contains_key(sym.as_ref())
                && !closures.contains_key(sym.as_ref()) =>
            {
              let qualified = format!("{}/{}", info.at_ns, sym);
              if info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS && sym.as_ref() == "'" {
                Some(CalcitProc::List)
              } else if let Some(Calcit::Proc(proc)) = ctx.value_imports.get(&qualified) {
                Some(*proc)
              } else {
                None
              }
            }
            _ => None,
          };
          if let Some(proc) = aliased_proc {
            let mut rewritten = vec![Calcit::Proc(proc)];
            rewritten.extend(items.iter().skip(1).cloned());
            return infer_wasm_evidence_in_scope(ctx, &Calcit::from(rewritten), local_types, closures, visiting);
          }
          let def_name = match head {
            Calcit::Import(import) if import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS => Some(import.def.as_ref()),
            Calcit::Symbol { sym, info, .. }
              if info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
                && !local_types.contains_key(sym.as_ref())
                && !ctx.locals.contains_key(sym.as_ref())
                && !closures.contains_key(sym.as_ref()) =>
            {
              Some(sym.as_ref())
            }
            Calcit::Fn { info, .. } => {
              if let Some(def_ref) = &info.def_ref
                && def_ref.def_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
              {
                Some(def_ref.def_name.as_ref())
              } else if info.def_ns.as_ref() == crate::wasm::host::calcit::CORE_NS {
                Some(info.name.as_ref())
              } else {
                None
              }
            }
            _ => None,
          };
          if def_name == Some("concat")
            && let Some(concat_ev) = infer_list_concat_evidence(ctx, &items.drop_left().to_vec(), local_types, closures, visiting)
          {
            return Some(concat_ev);
          }
          let Some(mut ev) = infer_wasm_callable_result(ctx, expr, &items.drop_left().to_vec(), local_types, closures, visiting)
            .or_else(|| {
              matches!(def_name, Some("merge" | "merge-non-nil"))
                .then(|| WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Map(DYNAMIC_TYPE.clone(), DYNAMIC_TYPE.clone()))))
            })
          else {
            if def_name == Some("dissoc")
              && items.len() > 1
              && let Some(base_ev) = infer_wasm_evidence_in_scope(ctx, &items[1], local_types, closures, visiting)
              && matches!(
                expand_wasm_value_type(&base_ev.annotation).as_ref(),
                CalcitTypeAnnotation::Map(_, _) | CalcitTypeAnnotation::List(_)
              )
            {
              return Some(WasmTypeEvidence {
                annotation: base_ev.annotation,
                set_member_types: base_ev.set_member_types,
                ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
              });
            }
            return None;
          };
          if def_name == Some("pairs-map")
            && let Some(arg0) = items.get(1)
            && let Some(kv_pairs) = maps::extract_pairs_list_key_val_exprs(arg0)
          {
            let key_types = kv_pairs
              .iter()
              .map(|(k, _)| {
                infer_wasm_evidence_in_scope(ctx, k, local_types, closures, visiting)
                  .map(|k_ev| CalcitTypeAnnotation::expand_schema_alias(&k_ev.annotation))
                  .unwrap_or_else(|| DYNAMIC_TYPE.clone())
              })
              .collect::<Vec<_>>();
            if let Some(first_k) = key_types.first()
              && key_types.iter().all(|k| k == first_k)
              && let CalcitTypeAnnotation::Map(_, val_ty) = CalcitTypeAnnotation::expand_schema_alias(&ev.annotation).as_ref()
            {
              ev.annotation = Arc::new(CalcitTypeAnnotation::Map(first_k.clone(), val_ty.clone()));
            }
            if let Some(hint) = infer_literal_tag_map_codec_hint(ctx, &kv_pairs, &ev.annotation, local_types, closures, visiting) {
              ev.codec_hint = Some(hint);
            }
            ev.set_member_types = Some(key_types);
          } else if matches!(def_name, Some("result:unwrap-or" | "option:unwrap-or"))
            && let Some(arg0) = items.get(1)
            && let Some(arg_ev) = infer_wasm_evidence_in_scope(ctx, arg0, local_types, closures, visiting)
          {
            if arg_ev.codec_hint.is_some() {
              ev.codec_hint = arg_ev.codec_hint;
            }
            if nil_type_evidence_is_open(&ev.annotation)
              && let Some(arg1) = items.get(2)
              && let Some(fallback_ev) = infer_wasm_evidence_in_scope(ctx, arg1, local_types, closures, visiting)
            {
              let expanded_arg0 = expand_wasm_value_type(&arg_ev.annotation);
              if let CalcitTypeAnnotation::Enum(_, type_args) | CalcitTypeAnnotation::TypeRef(_, type_args) = expanded_arg0.as_ref()
                && let Some(payload_ty) = type_args.first()
              {
                let t0 = expand_wasm_value_type(payload_ty);
                let t1 = expand_wasm_value_type(&fallback_ev.annotation);
                if t0 == t1 || t1.is_proven_for(&t0) {
                  ev.annotation = t0;
                } else if t0.is_proven_for(&t1) {
                  ev.annotation = t1;
                }
              }
            }
          } else if def_name == Some("keys-non-nil")
            && let Some(arg0) = items.get(1)
            && let Some(arg_ev) = infer_wasm_evidence_in_scope(ctx, arg0, local_types, closures, visiting)
          {
            ev.set_member_types = arg_ev.set_member_types;
          } else if matches!(def_name, Some("&map:empty" | "&set:empty" | "empty"))
            && items.len() == 2
            && let Some(arg_ev) = infer_wasm_evidence_in_scope(ctx, &items[1], local_types, closures, visiting)
          {
            let expanded_arg = CalcitTypeAnnotation::expand_schema_alias(&arg_ev.annotation);
            let is_collection = match def_name {
              Some("&map:empty") => matches!(expanded_arg.as_ref(), CalcitTypeAnnotation::Map(_, _)),
              Some("&set:empty") => matches!(expanded_arg.as_ref(), CalcitTypeAnnotation::Set(_)),
              _ => matches!(
                expanded_arg.as_ref(),
                CalcitTypeAnnotation::Map(_, _) | CalcitTypeAnnotation::Set(_)
              ),
            };
            if is_collection {
              if ev.annotation.contains_type_var() && !arg_ev.annotation.contains_type_var() {
                ev.annotation = arg_ev.annotation;
              }
              ev.set_member_types = Some(vec![]);
            }
          } else if matches!(def_name, Some("include" | "exclude" | "assoc" | "dissoc"))
            && items.len() > 1
            && let Some(base_ev) = infer_wasm_evidence_in_scope(ctx, &items[1], local_types, closures, visiting)
          {
            let mut combined = base_ev.set_member_types.unwrap_or_else(|| {
              let expanded_base = CalcitTypeAnnotation::expand_schema_alias(&base_ev.annotation);
              match expanded_base.as_ref() {
                CalcitTypeAnnotation::Set(k) | CalcitTypeAnnotation::Map(k, _) => {
                  let k_exp = CalcitTypeAnnotation::expand_schema_alias(k);
                  if matches!(k_exp.as_ref(), CalcitTypeAnnotation::Never) {
                    vec![]
                  } else {
                    vec![k_exp]
                  }
                }
                _ => vec![DYNAMIC_TYPE.clone()],
              }
            });
            if def_name == Some("include") {
              for arg in items.iter().skip(2) {
                let elem_ty = infer_wasm_evidence_in_scope(ctx, arg, local_types, closures, visiting)
                  .map(|e| CalcitTypeAnnotation::expand_schema_alias(&e.annotation))
                  .unwrap_or_else(|| DYNAMIC_TYPE.clone());
                combined.push(elem_ty);
              }
            } else if def_name == Some("assoc") && items.len() >= 4 {
              for key_expr in items.iter().skip(2).step_by(2) {
                let key_ty = infer_wasm_evidence_in_scope(ctx, key_expr, local_types, closures, visiting)
                  .map(|e| CalcitTypeAnnotation::expand_schema_alias(&e.annotation))
                  .unwrap_or_else(|| DYNAMIC_TYPE.clone());
                combined.push(key_ty);
              }
            }
            if def_name == Some("dissoc")
              && nil_type_evidence_is_open(&ev.annotation)
              && matches!(
                expand_wasm_value_type(&base_ev.annotation).as_ref(),
                CalcitTypeAnnotation::Map(_, _) | CalcitTypeAnnotation::List(_)
              )
            {
              ev.annotation = base_ev.annotation;
            }
            ev.set_member_types = Some(combined);
          } else if matches!(def_name, Some("merge" | "merge-non-nil" | "union" | "difference" | "intersection")) && items.len() > 1 {
            let is_map_op = matches!(def_name, Some("merge" | "merge-non-nil"));
            let skip_rhs_nil = def_name == Some("merge-non-nil");
            let mut joined_map: Option<(Arc<CalcitTypeAnnotation>, Arc<CalcitTypeAnnotation>)> = None;
            let mut map_types_known = is_map_op;
            let mut combined = Vec::new();
            let mut all_known = true;
            for (arg_idx, arg) in items.iter().skip(1).enumerate() {
              if let Some(arg_ev) = infer_wasm_evidence_in_scope(ctx, arg, local_types, closures, visiting) {
                if map_types_known && !join_merge_map_operand(&mut joined_map, &arg_ev, skip_rhs_nil, arg_idx) {
                  map_types_known = false;
                }
                if let Some(members) = arg_ev.set_member_types {
                  combined.extend(members);
                  continue;
                }
                let expanded_arg = CalcitTypeAnnotation::expand_schema_alias(&arg_ev.annotation);
                match expanded_arg.as_ref() {
                  CalcitTypeAnnotation::Map(k, _) | CalcitTypeAnnotation::Set(k) => {
                    let k_exp = CalcitTypeAnnotation::expand_schema_alias(k);
                    if !matches!(k_exp.as_ref(), CalcitTypeAnnotation::Never) {
                      combined.push(k_exp);
                    }
                    continue;
                  }
                  _ => {}
                }
              }
              all_known = false;
              map_types_known = false;
              break;
            }
            if all_known {
              if combined.is_empty() {
                let never = crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone();
                if is_map_op {
                  ev.annotation = Arc::new(CalcitTypeAnnotation::Map(never.clone(), never));
                } else {
                  ev.annotation = Arc::new(CalcitTypeAnnotation::Set(never));
                }
              } else if map_types_known && let Some((k_ty, v_ty)) = joined_map {
                ev.annotation = Arc::new(CalcitTypeAnnotation::Map(k_ty, v_ty));
              }
              ev.set_member_types = Some(combined);
            } else if map_types_known && let Some((k_ty, v_ty)) = joined_map {
              ev.annotation = Arc::new(CalcitTypeAnnotation::Map(k_ty, v_ty));
            }
          }
          Some(ev)
        }
        Calcit::Proc(proc) => {
          if matches!(proc, CalcitProc::List) {
            let mut elem_types = Vec::with_capacity(items.len().saturating_sub(1));
            let mut elem_hints = Vec::with_capacity(items.len().saturating_sub(1));
            let mut any_elem_has_codec_hint = false;
            let mut all_elems_known = true;
            let mut combined_collection_members = Vec::new();
            let mut all_collection_members_known = items.len() > 1;
            for item in items.iter().skip(1) {
              if let Some(ev) = infer_wasm_evidence_in_scope(ctx, item, local_types, closures, visiting) {
                let expanded = expand_wasm_value_type(&ev.annotation);
                if matches!(expanded.as_ref(), CalcitTypeAnnotation::Dynamic) {
                  all_elems_known = false;
                }
                if ev.codec_hint.is_some() {
                  any_elem_has_codec_hint = true;
                }
                let item_hint = ev
                  .codec_hint
                  .as_ref()
                  .map(expand_wasm_value_type)
                  .unwrap_or_else(|| expanded.clone());
                elem_hints.push(item_hint);
                if let Some(members) = ev.set_member_types {
                  combined_collection_members.extend(members);
                } else {
                  match expanded.as_ref() {
                    CalcitTypeAnnotation::Map(k, _) | CalcitTypeAnnotation::Set(k) => {
                      let k_exp = CalcitTypeAnnotation::expand_schema_alias(k);
                      if !matches!(k_exp.as_ref(), CalcitTypeAnnotation::Never) {
                        combined_collection_members.push(k_exp);
                      }
                    }
                    _ => {
                      all_collection_members_known = false;
                    }
                  }
                }
                elem_types.push(expanded);
              } else {
                all_elems_known = false;
                all_collection_members_known = false;
                elem_types.push(DYNAMIC_TYPE.clone());
                elem_hints.push(DYNAMIC_TYPE.clone());
              }
            }
            let list_codec_hint = (any_elem_has_codec_hint && !elem_hints.is_empty()).then(|| {
              if elem_hints.iter().all(|h| h == &elem_hints[0]) {
                Arc::new(CalcitTypeAnnotation::List(elem_hints[0].clone()))
              } else {
                Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(elem_hints)))
              }
            });
            let annotation = if items.len() == 1 {
              Arc::new(CalcitTypeAnnotation::List(
                crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone(),
              ))
            } else {
              let static_ty =
                infer_static_type_from_expr(expr).unwrap_or_else(|| Arc::new(CalcitTypeAnnotation::List(DYNAMIC_TYPE.clone())));
              let preserves_tuple_slots = (matches!(
                static_ty.as_ref(),
                CalcitTypeAnnotation::List(inner) if wasm_type_requires_scalar_specialization(inner)
              ) && all_elems_known
                && !all_collection_members_known
                && elem_types.iter().any(|e| !wasm_type_requires_scalar_specialization(e)))
                || (matches!(
                  static_ty.as_ref(),
                  CalcitTypeAnnotation::Tuple(slots)
                    if static_ty.contains_type_var()
                      || slots.iter().any(|slot| nil_type_evidence_is_open(&expand_wasm_value_type(slot)))
                ) && all_elems_known);
              if preserves_tuple_slots {
                Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(elem_types)))
              } else if matches!(
                static_ty.as_ref(),
                CalcitTypeAnnotation::List(inner) if {
                  let exp_inner = expand_wasm_value_type(inner);
                  matches!(exp_inner.as_ref(), CalcitTypeAnnotation::Never)
                    || nil_type_evidence_is_open(&exp_inner)
                    || matches!(
                      (exp_inner.as_ref(), elem_types.first().map(|e| e.as_ref())),
                      (CalcitTypeAnnotation::List(list_elem), Some(CalcitTypeAnnotation::Tuple(_)))
                        if nil_type_evidence_is_open(&expand_wasm_value_type(list_elem))
                    )
                }
              ) && all_elems_known
                && elem_types
                  .first()
                  .is_some_and(|first| !first.contains_type_var() && elem_types.iter().all(|t| t == first))
              {
                Arc::new(CalcitTypeAnnotation::List(elem_types[0].clone()))
              } else {
                static_ty
              }
            };
            return Some(WasmTypeEvidence {
              annotation,
              codec_hint: list_codec_hint,
              set_member_types: all_collection_members_known.then_some(combined_collection_members),
              ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
            });
          }
          if matches!(proc, CalcitProc::NativeListNth)
            && items.len() == 3
            && let Some(list_ev) = infer_wasm_evidence_in_scope(ctx, &items[1], local_types, closures, visiting)
          {
            let elem_codec_hint = list_ev.codec_hint.as_ref().and_then(|hint| {
              let exp_hint = expand_wasm_value_type(hint);
              match exp_hint.as_ref() {
                CalcitTypeAnnotation::List(inner) => Some(inner.clone()),
                CalcitTypeAnnotation::Tuple(slots) => {
                  if let Calcit::Number(idx) = &items[2]
                    && idx.fract() == 0.0
                    && *idx >= 0.0
                  {
                    slots.get(*idx as usize).cloned()
                  } else {
                    slots.first().filter(|first| slots.iter().all(|s| s == *first)).cloned()
                  }
                }
                _ => None,
              }
            });
            let expanded = CalcitTypeAnnotation::expand_schema_alias(&list_ev.annotation);
            match expanded.as_ref() {
              CalcitTypeAnnotation::Tuple(slots) => {
                if let Calcit::Number(idx) = &items[2]
                  && idx.fract() == 0.0
                  && *idx >= 0.0
                  && let Some(slot_ty) = slots.get(*idx as usize)
                {
                  return Some(WasmTypeEvidence {
                    annotation: slot_ty.clone(),
                    codec_hint: elem_codec_hint,
                    ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
                  });
                }
              }
              CalcitTypeAnnotation::List(elem_ty) => {
                return Some(WasmTypeEvidence {
                  annotation: elem_ty.clone(),
                  codec_hint: elem_codec_hint,
                  set_member_types: list_ev.set_member_types,
                  ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
                });
              }
              _ => {}
            }
          }
          if matches!(proc, CalcitProc::Set) {
            let member_types = items
              .iter()
              .skip(1)
              .map(|item| {
                infer_wasm_evidence_in_scope(ctx, item, local_types, closures, visiting)
                  .map(|ev| CalcitTypeAnnotation::expand_schema_alias(&ev.annotation))
                  .unwrap_or_else(|| DYNAMIC_TYPE.clone())
              })
              .collect::<Vec<_>>();
            let annotation = if items.len() == 1 {
              Arc::new(CalcitTypeAnnotation::Set(
                crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone(),
              ))
            } else {
              infer_static_type_from_expr(expr).unwrap_or_else(|| Arc::new(CalcitTypeAnnotation::Set(DYNAMIC_TYPE.clone())))
            };
            return Some(WasmTypeEvidence {
              annotation,
              set_member_types: Some(member_types),
              ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
            });
          }
          if matches!(proc, CalcitProc::JsonParse) {
            return Some(WasmTypeEvidence::contract(DYNAMIC_TYPE.clone()));
          }
          if matches!(proc, CalcitProc::NativeStructNth)
            && items.len() >= 3
            && let Calcit::Number(idx) = &items[2]
            && idx.fract() == 0.0
            && *idx >= 0.0
            && let Some(struct_ev) = infer_wasm_evidence_in_scope(ctx, &items[1], local_types, closures, visiting)
            && let Some(fields) = resolved_struct_fields_for_annotation(&struct_ev.annotation)
            && let Some((_, field_ty)) = fields.get(*idx as usize)
          {
            return Some(WasmTypeEvidence::contract(field_ty.clone()));
          }
          if matches!(proc, CalcitProc::NativeStructToMap)
            && items.len() == 2
            && let Some(struct_ev) = infer_wasm_evidence_in_scope(ctx, &items[1], local_types, closures, visiting)
          {
            let resolved_fields = resolved_struct_fields_for_annotation(&struct_ev.annotation);
            let val_ty = if let Some(fields) = resolved_fields {
              if fields.is_empty() {
                crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone()
              } else {
                let first_ty = CalcitTypeAnnotation::expand_schema_alias(&fields[0].1);
                if fields[1..]
                  .iter()
                  .all(|(_, ty)| CalcitTypeAnnotation::expand_schema_alias(ty) == first_ty)
                {
                  first_ty
                } else {
                  DYNAMIC_TYPE.clone()
                }
              }
            } else {
              DYNAMIC_TYPE.clone()
            };
            let tag_ty = Arc::new(CalcitTypeAnnotation::Tag);
            return Some(WasmTypeEvidence {
              annotation: Arc::new(CalcitTypeAnnotation::Map(tag_ty.clone(), val_ty)),
              set_member_types: Some(vec![tag_ty]),
              ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
            });
          }
          if matches!(proc, CalcitProc::NativeMap) && (items.len() - 1).is_multiple_of(2) {
            let key_types = items
              .iter()
              .skip(1)
              .step_by(2)
              .map(|k| {
                infer_wasm_evidence_in_scope(ctx, k, local_types, closures, visiting)
                  .map(|ev| CalcitTypeAnnotation::expand_schema_alias(&ev.annotation))
                  .unwrap_or_else(|| DYNAMIC_TYPE.clone())
              })
              .collect::<Vec<_>>();
            let kv_pairs: Vec<(Calcit, Calcit)> = items
              .iter()
              .skip(1)
              .step_by(2)
              .cloned()
              .zip(items.iter().skip(2).step_by(2).cloned())
              .collect();
            let never = crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone();
            let annotation = if items.len() == 1 {
              Arc::new(CalcitTypeAnnotation::Map(never.clone(), never))
            } else {
              let inferred_k = key_types.iter().fold(never.clone(), |acc, k| join_collection_slot_type(&acc, k));
              let inferred_v = kv_pairs.iter().fold(never, |acc, (_, v_expr)| {
                let val_ty = infer_wasm_evidence_in_scope(ctx, v_expr, local_types, closures, visiting)
                  .map(|ev| CalcitTypeAnnotation::expand_schema_alias(&ev.annotation))
                  .unwrap_or_else(|| DYNAMIC_TYPE.clone());
                join_collection_slot_type(&acc, &val_ty)
              });
              let refine_slot = |static_slot: &Arc<CalcitTypeAnnotation>, inferred: &Arc<CalcitTypeAnnotation>| {
                if (nil_type_evidence_is_open(&expand_wasm_value_type(static_slot)) || static_slot.contains_type_var())
                  && !inferred.contains_type_var()
                  && !nil_type_evidence_is_open(&expand_wasm_value_type(inferred))
                {
                  inferred.clone()
                } else {
                  static_slot.clone()
                }
              };
              match infer_static_type_from_expr(expr) {
                Some(static_ty) => match CalcitTypeAnnotation::expand_schema_alias(&static_ty).as_ref() {
                  CalcitTypeAnnotation::Map(sk, sv) => Arc::new(CalcitTypeAnnotation::Map(
                    refine_slot(sk, &inferred_k),
                    refine_slot(sv, &inferred_v),
                  )),
                  _ => static_ty,
                },
                None => Arc::new(CalcitTypeAnnotation::Map(inferred_k, inferred_v)),
              }
            };
            let codec_hint = infer_literal_tag_map_codec_hint(ctx, &kv_pairs, &annotation, local_types, closures, visiting);
            return Some(WasmTypeEvidence {
              annotation,
              codec_hint,
              set_member_types: Some(key_types),
              ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
            });
          }
          if matches!(
            proc,
            CalcitProc::NativeInclude | CalcitProc::NativeExclude | CalcitProc::NativeMapAssoc | CalcitProc::NativeMapDissoc
          ) && items.len() >= 3
            && let Some(base_ev) = infer_wasm_evidence_in_scope(ctx, &items[1], local_types, closures, visiting)
          {
            let is_map_op = matches!(proc, CalcitProc::NativeMapAssoc | CalcitProc::NativeMapDissoc);
            let base_is_empty = base_ev.set_member_types.as_deref() == Some(&[]);
            let mut combined = Vec::new();
            if let Some(base_members) = base_ev.set_member_types {
              for ty in &base_members {
                push_unique_member_type(&mut combined, ty);
              }
            } else {
              let expanded_base = CalcitTypeAnnotation::expand_schema_alias(&base_ev.annotation);
              match expanded_base.as_ref() {
                CalcitTypeAnnotation::Set(k) | CalcitTypeAnnotation::Map(k, _) => push_unique_member_type(&mut combined, k),
                _ => push_unique_member_type(&mut combined, &DYNAMIC_TYPE),
              }
            }
            let expanded_base_ann = CalcitTypeAnnotation::expand_schema_alias(&base_ev.annotation);
            let base_matches_kind = if is_map_op {
              matches!(expanded_base_ann.as_ref(), CalcitTypeAnnotation::Map(_, _))
            } else {
              matches!(expanded_base_ann.as_ref(), CalcitTypeAnnotation::Set(_))
            };
            let annotation = if matches!(proc, CalcitProc::NativeInclude) {
              let mut elem_ann = match expanded_base_ann.as_ref() {
                CalcitTypeAnnotation::Set(base_e) => Some(effective_collection_slot(base_e, base_is_empty)),
                _ => None,
              };
              for arg in items.iter().skip(2) {
                let elem_ty = infer_wasm_evidence_in_scope(ctx, arg, local_types, closures, visiting)
                  .map(|e| e.annotation)
                  .unwrap_or_else(|| DYNAMIC_TYPE.clone());
                push_unique_member_type(&mut combined, &elem_ty);
                if let Some(acc) = &mut elem_ann {
                  *acc = join_collection_slot_type(acc, &elem_ty);
                }
              }
              elem_ann.map(|e| Arc::new(CalcitTypeAnnotation::Set(e)))
            } else if matches!(proc, CalcitProc::NativeMapAssoc) {
              let mut map_ann = match expanded_base_ann.as_ref() {
                CalcitTypeAnnotation::Map(base_k, base_v) => Some((
                  effective_collection_slot(base_k, base_is_empty),
                  effective_collection_slot(base_v, base_is_empty),
                )),
                _ => None,
              };
              let mut rest = items.iter().skip(2);
              while let Some(key_expr) = rest.next() {
                let key_ty = infer_wasm_evidence_in_scope(ctx, key_expr, local_types, closures, visiting)
                  .map(|e| e.annotation)
                  .unwrap_or_else(|| DYNAMIC_TYPE.clone());
                push_unique_member_type(&mut combined, &key_ty);
                let val_ty = rest
                  .next()
                  .and_then(|val_expr| infer_wasm_evidence_in_scope(ctx, val_expr, local_types, closures, visiting))
                  .map(|e| e.annotation)
                  .unwrap_or_else(|| DYNAMIC_TYPE.clone());
                if let Some((k_acc, v_acc)) = &mut map_ann {
                  *k_acc = join_collection_slot_type(k_acc, &key_ty);
                  *v_acc = join_collection_slot_type(v_acc, &val_ty);
                }
              }
              map_ann.map(|(k, v)| Arc::new(CalcitTypeAnnotation::Map(k, v)))
            } else if base_matches_kind {
              Some(base_ev.annotation.clone())
            } else {
              None
            }
            .unwrap_or_else(|| {
              infer_static_type_from_expr(expr)
                .filter(|ty| !ty.contains_type_var())
                .unwrap_or_else(|| {
                  if is_map_op {
                    Arc::new(CalcitTypeAnnotation::Map(DYNAMIC_TYPE.clone(), DYNAMIC_TYPE.clone()))
                  } else {
                    Arc::new(CalcitTypeAnnotation::Set(DYNAMIC_TYPE.clone()))
                  }
                })
            });
            return Some(WasmTypeEvidence {
              annotation,
              set_member_types: Some(combined),
              ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
            });
          }
          if matches!(
            proc,
            CalcitProc::NativeMerge
              | CalcitProc::NativeMergeNonNil
              | CalcitProc::NativeUnion
              | CalcitProc::NativeDifference
              | CalcitProc::NativeSetIntersection
          ) && items.len() > 1
          {
            let is_map_op = matches!(proc, CalcitProc::NativeMerge | CalcitProc::NativeMergeNonNil);
            let skip_rhs_nil = matches!(proc, CalcitProc::NativeMergeNonNil);
            let mut joined_map: Option<(Arc<CalcitTypeAnnotation>, Arc<CalcitTypeAnnotation>)> = None;
            let mut map_types_known = is_map_op;
            let mut combined = Vec::new();
            let mut all_known = true;
            for (arg_idx, arg) in items.iter().skip(1).enumerate() {
              if let Some(arg_ev) = infer_wasm_evidence_in_scope(ctx, arg, local_types, closures, visiting) {
                if map_types_known && !join_merge_map_operand(&mut joined_map, &arg_ev, skip_rhs_nil, arg_idx) {
                  map_types_known = false;
                }
                if let Some(members) = arg_ev.set_member_types {
                  for ty in &members {
                    push_unique_member_type(&mut combined, ty);
                  }
                  continue;
                }
                let expanded_arg = CalcitTypeAnnotation::expand_schema_alias(&arg_ev.annotation);
                match expanded_arg.as_ref() {
                  CalcitTypeAnnotation::Map(k, _) | CalcitTypeAnnotation::Set(k) => {
                    push_unique_member_type(&mut combined, k);
                    continue;
                  }
                  _ => {}
                }
              }
              all_known = false;
              map_types_known = false;
              break;
            }
            if all_known {
              let annotation = if combined.is_empty() {
                let never = crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone();
                if is_map_op {
                  Arc::new(CalcitTypeAnnotation::Map(never.clone(), never))
                } else {
                  Arc::new(CalcitTypeAnnotation::Set(never))
                }
              } else if map_types_known && let Some((k_ty, v_ty)) = joined_map {
                Arc::new(CalcitTypeAnnotation::Map(k_ty, v_ty))
              } else {
                infer_static_type_from_expr(expr).unwrap_or_else(|| {
                  if is_map_op {
                    Arc::new(CalcitTypeAnnotation::Map(DYNAMIC_TYPE.clone(), DYNAMIC_TYPE.clone()))
                  } else {
                    Arc::new(CalcitTypeAnnotation::Set(DYNAMIC_TYPE.clone()))
                  }
                })
              };
              return Some(WasmTypeEvidence {
                annotation,
                set_member_types: Some(combined),
                ..WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
              });
            }
          }
          if matches!(proc, CalcitProc::NativeListConcat)
            && let Some(ev) = infer_list_concat_evidence(ctx, &items.drop_left().to_vec(), local_types, closures, visiting)
          {
            return Some(ev);
          }
          let (ns, def) = proc.get_ns_def();
          let signature = program::lookup_def_schema(ns, def).resolve_to_fn().or_else(|| {
            proc
              .get_type_signature()
              .filter(|signature| signature.return_type.contains_type_var())
              .and_then(|signature| {
                CalcitTypeAnnotation::from_proc_parts(signature.arg_types.clone(), signature.return_type.clone(), proc.arity())
                  .resolve_to_fn()
              })
          })?;
          if !signature.return_type.contains_type_var() {
            // Retain inferred constructor identity and payload evidence only
            // when it proves the existing fixed result contract.
            return infer_static_type_from_expr(expr)
              .filter(|inferred| inferred.is_proven_for(signature.return_type.as_ref()))
              .map(|annotation| WasmTypeEvidence {
                exact_impls: matches!(proc, CalcitProc::NativeEnumNew | CalcitProc::NativeNamedEnumNew)
                  && annotation.resolve_to_enum().is_some(),
                ..WasmTypeEvidence::contract(annotation)
              })
              .or_else(|| Some(WasmTypeEvidence::contract(signature.return_type.clone())));
          }
          let mut bindings = TypeBindings::new();
          for (index, argument) in items.iter().skip(1).enumerate() {
            let expected = signature.arg_types.get(index).or(signature.rest_type.as_ref())?;
            let actual = infer_wasm_evidence_in_scope(ctx, argument, local_types, closures, visiting)?;
            if !prove_wasm_arg_bindings(&actual.annotation, expected, &mut bindings) {
              return None;
            }
          }
          let result = replace_unbound_type_vars_with_never(&signature.return_type.substitute_type_vars(&bindings));
          (!result.contains_type_var()).then(|| WasmTypeEvidence::contract(result))
        }
        _ => None,
      })
      .or_else(|| infer_static_type_from_expr(expr).map(WasmTypeEvidence::contract)),
    Calcit::Proc(CalcitProc::NativeMap) => {
      let never = crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone();
      Some(WasmTypeEvidence {
        set_member_types: Some(vec![]),
        ..WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Map(never.clone(), never)))
      })
    }
    Calcit::Proc(CalcitProc::Set) => {
      let never = crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone();
      Some(WasmTypeEvidence {
        set_member_types: Some(vec![]),
        ..WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Set(never)))
      })
    }
    Calcit::Enum(_) | Calcit::Struct(_) => infer_static_type_from_expr(expr).map(|annotation| WasmTypeEvidence {
      exact_impls: annotation.resolve_to_enum().is_some() || annotation.resolve_to_struct().is_some(),
      ..WasmTypeEvidence::contract(annotation)
    }),
    _ => {
      let known_function = resolve_known_function_value(ctx, expr);
      infer_static_type_from_expr(expr)
        .or_else(|| known_function.is_some().then(|| DYNAMIC_TYPE.clone()))
        .map(|annotation| WasmTypeEvidence {
          known_function,
          ..WasmTypeEvidence::contract(annotation)
        })
    }
  };
  if let Some(ref mut evidence) = result
    && evidence.codec_hint.is_none()
  {
    evidence.codec_hint = infer_enum_constructor_codec_hint(ctx, expr, &evidence.annotation, local_types, closures, visiting);
  }
  result
}

fn infer_enum_constructor_codec_hint(
  ctx: &WasmGenCtx,
  expr: &Calcit,
  base_annotation: &Arc<CalcitTypeAnnotation>,
  local_types: &HashMap<String, WasmTypeEvidence>,
  closures: &HashMap<String, Arc<InlineClosure>>,
  visiting: &mut HashSet<String>,
) -> Option<Arc<CalcitTypeAnnotation>> {
  match expr {
    Calcit::Import(CalcitImport { ns, def, .. }) if ns.as_ref() == "calcit.core" && def.as_ref() == "None" => {
      let enum_def = base_annotation.resolve_to_enum()?;
      Some(Arc::new(CalcitTypeAnnotation::Enum(
        Arc::new(enum_def),
        Arc::new(vec![crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone()]),
      )))
    }
    Calcit::List(items) if matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeNamedEnumNew))) => {
      let Calcit::Tag(variant_tag) = items.get(2)? else {
        return None;
      };
      let enum_def = base_annotation.resolve_to_enum()?;
      if enum_def.generics().is_empty() {
        return None;
      }
      let variant = enum_def.find_variant(variant_tag)?;
      let mut payload_hints = Vec::new();
      let mut any_custom_codec_hint = false;
      for payload_expr in items.iter().skip(3) {
        let evidence = infer_wasm_evidence_in_scope(ctx, payload_expr, local_types, closures, visiting)?;
        if evidence.codec_hint.is_some() {
          any_custom_codec_hint = true;
        }
        payload_hints.push(evidence.codec_hint.unwrap_or(evidence.annotation));
      }
      if !any_custom_codec_hint {
        return None;
      }
      let base_args: &[Arc<CalcitTypeAnnotation>] = match base_annotation.as_ref() {
        CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args) => args.as_slice(),
        _ => &[],
      };
      let mut bindings = TypeBindings::new();
      for (actual_ty, expected_ty) in payload_hints.into_iter().zip(variant.payload_types().iter()) {
        match expected_ty.as_ref() {
          CalcitTypeAnnotation::TypeVar(name) if enum_def.generics().contains(name) => {
            bindings.insert(name.clone(), actual_ty);
          }
          CalcitTypeAnnotation::TypeRef(name, args) if args.is_empty() && enum_def.generics().contains(name) => {
            bindings.insert(name.clone(), actual_ty);
          }
          _ => {
            let _ = actual_ty.prove_available_bindings(expected_ty, &mut bindings);
          }
        }
      }
      let args = enum_def
        .generics()
        .iter()
        .enumerate()
        .map(|(idx, g)| {
          bindings
            .get(g)
            .cloned()
            .or_else(|| base_args.get(idx).cloned())
            .unwrap_or_else(|| crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone())
        })
        .collect();
      Some(Arc::new(CalcitTypeAnnotation::Enum(Arc::new(enum_def), Arc::new(args))))
    }
    _ => None,
  }
}

fn infer_encoded_struct_field_type_with_seen(
  def: &CalcitStructDef,
  args: &[Arc<CalcitTypeAnnotation>],
  idx: usize,
  visiting: &mut HashSet<String>,
) -> Option<Arc<CalcitTypeAnnotation>> {
  let raw_field = def.field_types.get(idx)?;
  let substituted = if def.generics.is_empty() || args.is_empty() {
    CalcitTypeAnnotation::expand_schema_alias(raw_field)
  } else {
    let bindings: TypeBindings = def
      .generics
      .iter()
      .zip(args.iter())
      .map(|(name, arg)| (name.clone(), CalcitTypeAnnotation::expand_schema_alias(arg)))
      .collect();
    CalcitTypeAnnotation::expand_schema_alias(&raw_field.substitute_type_vars(&bindings))
  };
  if let Some(enum_def) = substituted.resolve_to_enum()
    && enum_def.name().ref_str() == "Option"
    && let CalcitTypeAnnotation::Enum(_, opt_args) | CalcitTypeAnnotation::TypeRef(_, opt_args) = substituted.as_ref()
    && opt_args.len() == 1
  {
    let inner = CalcitTypeAnnotation::expand_schema_alias(&opt_args[0]);
    let inner_is_option = inner
      .resolve_to_enum()
      .is_some_and(|inner_enum| inner_enum.name().ref_str() == "Option");
    if inner_is_option {
      return Some(Arc::new(CalcitTypeAnnotation::Enum(
        Arc::new(enum_def),
        Arc::new(vec![infer_encoded_map_type_with_seen(&inner, visiting)]),
      )));
    }
    return Some(infer_encoded_map_type_with_seen(&inner, visiting));
  }
  Some(infer_encoded_map_type_with_seen(&substituted, visiting))
}

fn infer_encoded_struct_map_value_type_with_seen(
  def: &CalcitStructDef,
  args: &[Arc<CalcitTypeAnnotation>],
  visiting: &mut HashSet<String>,
) -> Arc<CalcitTypeAnnotation> {
  if def.field_types.is_empty() {
    return DYNAMIC_TYPE.clone();
  }
  let struct_key = def.definition_ref.as_deref().unwrap_or_else(|| def.name.ref_str()).to_owned();
  if !visiting.insert(struct_key.clone()) {
    return DYNAMIC_TYPE.clone();
  }
  let mut common: Option<Arc<CalcitTypeAnnotation>> = None;
  let mut matched = true;
  for idx in 0..def.field_types.len() {
    let Some(field_ty) = infer_encoded_struct_field_type_with_seen(def, args, idx, visiting) else {
      matched = false;
      break;
    };
    match &common {
      None => common = Some(field_ty),
      Some(prev) if prev == &field_ty => {}
      Some(_) => {
        matched = false;
        break;
      }
    }
  }
  visiting.remove(&struct_key);
  if matched {
    common.unwrap_or_else(|| DYNAMIC_TYPE.clone())
  } else {
    DYNAMIC_TYPE.clone()
  }
}

fn infer_encoded_struct_map_value_type(def: &CalcitStructDef, args: &[Arc<CalcitTypeAnnotation>]) -> Arc<CalcitTypeAnnotation> {
  infer_encoded_struct_map_value_type_with_seen(def, args, &mut HashSet::new())
}

fn infer_encoded_map_type_with_seen(
  annotation: &Arc<CalcitTypeAnnotation>,
  visiting: &mut HashSet<String>,
) -> Arc<CalcitTypeAnnotation> {
  let expanded = expand_wasm_value_type(annotation);
  match expanded.as_ref() {
    CalcitTypeAnnotation::List(inner) => Arc::new(CalcitTypeAnnotation::List(infer_encoded_map_type_with_seen(inner, visiting))),
    CalcitTypeAnnotation::Set(inner) => Arc::new(CalcitTypeAnnotation::Set(infer_encoded_map_type_with_seen(inner, visiting))),
    CalcitTypeAnnotation::Map(k, v) => Arc::new(CalcitTypeAnnotation::Map(
      infer_encoded_map_type_with_seen(k, visiting),
      infer_encoded_map_type_with_seen(v, visiting),
    )),
    CalcitTypeAnnotation::Tuple(slots) => Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(
      slots.iter().map(|s| infer_encoded_map_type_with_seen(s, visiting)).collect(),
    ))),
    CalcitTypeAnnotation::Struct(def, args) => Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::Tag),
      infer_encoded_struct_map_value_type_with_seen(def, args, visiting),
    )),
    CalcitTypeAnnotation::StructValue(def) => Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::Tag),
      infer_encoded_struct_map_value_type_with_seen(def, &[], visiting),
    )),
    CalcitTypeAnnotation::Enum(def, args) => {
      if def.name().ref_str() == "Option" {
        DYNAMIC_TYPE.clone()
      } else {
        Arc::new(CalcitTypeAnnotation::Enum(
          def.clone(),
          Arc::new(args.iter().map(|a| infer_encoded_map_type_with_seen(a, visiting)).collect()),
        ))
      }
    }
    CalcitTypeAnnotation::TypeRef(_, args) => {
      if let Some(def) = expanded.resolve_to_struct() {
        Arc::new(CalcitTypeAnnotation::Map(
          Arc::new(CalcitTypeAnnotation::Tag),
          infer_encoded_struct_map_value_type_with_seen(&def, args, visiting),
        ))
      } else if let Some(def) = expanded.resolve_to_enum() {
        if def.name().ref_str() == "Option" {
          DYNAMIC_TYPE.clone()
        } else {
          Arc::new(CalcitTypeAnnotation::Enum(
            Arc::new(def),
            Arc::new(args.iter().map(|a| infer_encoded_map_type_with_seen(a, visiting)).collect()),
          ))
        }
      } else {
        DYNAMIC_TYPE.clone()
      }
    }
    CalcitTypeAnnotation::Nil
    | CalcitTypeAnnotation::Unit
    | CalcitTypeAnnotation::Bool
    | CalcitTypeAnnotation::Number
    | CalcitTypeAnnotation::Numeric(_)
    | CalcitTypeAnnotation::String
    | CalcitTypeAnnotation::Tag
    | CalcitTypeAnnotation::Symbol
    | CalcitTypeAnnotation::Buffer
    | CalcitTypeAnnotation::CirruQuote => expanded,
    _ => DYNAMIC_TYPE.clone(),
  }
}

fn infer_encoded_map_type(annotation: &Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  infer_encoded_map_type_with_seen(annotation, &mut HashSet::new())
}

fn infer_encoded_codec_hint(annotation: &Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  let expanded = expand_wasm_value_type(annotation);
  match expanded.as_ref() {
    CalcitTypeAnnotation::List(inner) => Arc::new(CalcitTypeAnnotation::List(infer_encoded_codec_hint(inner))),
    CalcitTypeAnnotation::Set(inner) => Arc::new(CalcitTypeAnnotation::Set(infer_encoded_codec_hint(inner))),
    CalcitTypeAnnotation::Map(k, v) => Arc::new(CalcitTypeAnnotation::Map(infer_encoded_codec_hint(k), infer_encoded_codec_hint(v))),
    CalcitTypeAnnotation::Tuple(slots) => Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(
      slots.iter().map(infer_encoded_codec_hint).collect(),
    ))),
    CalcitTypeAnnotation::Struct(def, args) => Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::StructValue(def.clone())),
      Arc::new(CalcitTypeAnnotation::Struct(def.clone(), args.clone())),
    )),
    CalcitTypeAnnotation::StructValue(def) => Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::StructValue(def.clone())),
      Arc::new(CalcitTypeAnnotation::Struct(def.clone(), Arc::new(vec![]))),
    )),
    CalcitTypeAnnotation::Enum(def, args) => Arc::new(CalcitTypeAnnotation::Enum(
      def.clone(),
      Arc::new(args.iter().map(infer_encoded_codec_hint).collect()),
    )),
    CalcitTypeAnnotation::TypeRef(_, args) => {
      if let Some(def) = expanded.resolve_to_struct() {
        let def = Arc::new(def);
        Arc::new(CalcitTypeAnnotation::Map(
          Arc::new(CalcitTypeAnnotation::StructValue(def.clone())),
          Arc::new(CalcitTypeAnnotation::Struct(def, args.clone())),
        ))
      } else if let Some(def) = expanded.resolve_to_enum() {
        Arc::new(CalcitTypeAnnotation::Enum(
          Arc::new(def),
          Arc::new(args.iter().map(infer_encoded_codec_hint).collect()),
        ))
      } else {
        DYNAMIC_TYPE.clone()
      }
    }
    _ => infer_encoded_map_type(&expanded),
  }
}

/// A declared data result is not a witness for value-carried implementations.
/// Recover that witness from the checked body with the actual argument evidence.
fn replace_unbound_type_vars_with_never(annotation: &Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  if !annotation.contains_type_var() {
    return annotation.clone();
  }
  match annotation.as_ref() {
    CalcitTypeAnnotation::TypeVar(_) | CalcitTypeAnnotation::TypeSlot(_) => {
      crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone()
    }
    CalcitTypeAnnotation::List(inner) => Arc::new(CalcitTypeAnnotation::List(replace_unbound_type_vars_with_never(inner))),
    CalcitTypeAnnotation::Set(inner) => Arc::new(CalcitTypeAnnotation::Set(replace_unbound_type_vars_with_never(inner))),
    CalcitTypeAnnotation::Map(k, v) => Arc::new(CalcitTypeAnnotation::Map(
      replace_unbound_type_vars_with_never(k),
      replace_unbound_type_vars_with_never(v),
    )),
    _ => annotation.clone(),
  }
}

fn join_optional_recur_evidence(init_ev: WasmTypeEvidence, r_ev: Option<WasmTypeEvidence>) -> WasmTypeEvidence {
  let Some(r_ev) = r_ev else {
    return WasmTypeEvidence::contract(DYNAMIC_TYPE.clone());
  };
  let init_is_option_none = init_ev.annotation.resolve_to_enum().is_some_and(|e| e.name().ref_str() == "Option")
    && matches!(
      init_ev.annotation.as_ref(),
      CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args)
        if args.first().is_some_and(|inner| matches!(inner.as_ref(), CalcitTypeAnnotation::Never))
    );
  if init_is_option_none && r_ev.annotation.resolve_to_enum().is_some_and(|e| e.name().ref_str() == "Option") {
    return WasmTypeEvidence {
      exact_impls: init_ev.exact_impls && r_ev.exact_impls,
      ..r_ev
    };
  }
  if init_ev.annotation == r_ev.annotation && init_ev.scalar_origin == r_ev.scalar_origin {
    let codec_hint = if init_ev.codec_hint == r_ev.codec_hint {
      init_ev.codec_hint
    } else {
      None
    };
    let known_function = if init_ev.known_function == r_ev.known_function {
      init_ev.known_function.clone()
    } else {
      None
    };
    let set_member_types = join_branch_member_types(init_ev.set_member_types, r_ev.set_member_types);
    let list_element_types = join_branch_member_types(init_ev.list_element_types, r_ev.list_element_types);
    return WasmTypeEvidence {
      codec_hint,
      known_function,
      set_member_types,
      list_element_types,
      exact_impls: init_ev.exact_impls && r_ev.exact_impls,
      ..init_ev
    };
  }
  let init_expanded = expand_wasm_value_type(&init_ev.annotation);
  let r_expanded = expand_wasm_value_type(&r_ev.annotation);
  if matches!(init_expanded.as_ref(), CalcitTypeAnnotation::Nil) {
    if matches!(r_expanded.as_ref(), CalcitTypeAnnotation::Nullable(_, NullableOrigin::Inferred)) {
      return WasmTypeEvidence::contract(r_ev.annotation);
    }
    if !nil_type_evidence_is_open(&r_expanded) && !matches!(r_expanded.as_ref(), CalcitTypeAnnotation::Never) {
      return WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Nullable(r_ev.annotation, NullableOrigin::Inferred)));
    }
  }
  WasmTypeEvidence::contract(DYNAMIC_TYPE.clone())
}

fn find_first_recur_args(expr: &Calcit) -> Option<Vec<Calcit>> {
  let Calcit::List(xs) = expr else { return None };
  if matches!(xs.first(), Some(Calcit::Proc(CalcitProc::Recur))) {
    return Some(xs.drop_left().to_vec());
  }
  if matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _) | Calcit::Fn { .. })) {
    return None;
  }
  xs.iter().find_map(find_first_recur_args)
}

fn infer_inline_closure_result_with_evidences(
  ctx: &WasmGenCtx,
  closure: &Arc<InlineClosure>,
  arg_evidences: &[WasmTypeEvidence],
  visiting: &mut HashSet<String>,
) -> Option<WasmTypeEvidence> {
  if !closure.accepts(arg_evidences.len()) || visiting.len() >= 16 {
    return None;
  }
  let visit_key = format!("inline-closure:{:p}", Arc::as_ptr(closure));
  if !visiting.insert(visit_key.clone()) {
    return None;
  }
  let mut body_types = closure.captured_local_types.clone();
  let body_closures = closure.captured_closures.clone();
  let positional = closure.positional();
  for (idx, param) in closure.params.iter().take(positional).enumerate() {
    if let Some(ev) = arg_evidences.get(idx) {
      body_types.insert(param.clone(), ev.clone());
    } else if idx >= positional - closure.option_types.len() {
      if let Ok(none) = nominal_option_none_expression()
        && let Some(ev) = infer_wasm_value_evidence(ctx, &none)
      {
        body_types.insert(param.clone(), ev);
      } else {
        body_types.insert(param.clone(), WasmTypeEvidence::contract(DYNAMIC_TYPE.clone()));
      }
    } else {
      body_types.insert(param.clone(), WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Nil)));
    }
  }
  if closure.rest {
    body_types.insert(
      closure.params[positional].clone(),
      WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::List(DYNAMIC_TYPE.clone()))),
    );
  }
  let executable = closure
    .body
    .iter()
    .filter(|expr| !crate::wasm::host::builtins::syntax::is_function_metadata_hint(expr))
    .collect::<Vec<_>>();
  let result = executable
    .last()
    .and_then(|tail| infer_wasm_evidence_in_scope(ctx, tail, &body_types, &body_closures, visiting));
  visiting.remove(&visit_key);
  result
}

fn infer_inline_closure_result(
  ctx: &WasmGenCtx,
  closure: &Arc<InlineClosure>,
  args: &[Calcit],
  local_types: &HashMap<String, WasmTypeEvidence>,
  closures: &HashMap<String, Arc<InlineClosure>>,
  visiting: &mut HashSet<String>,
) -> Option<WasmTypeEvidence> {
  if !closure.accepts(args.len()) || visiting.len() >= 16 {
    return None;
  }
  let visit_key = format!("inline-closure:{:p}", Arc::as_ptr(closure));
  if !visiting.insert(visit_key.clone()) {
    return None;
  }
  let mut body_types = closure.captured_local_types.clone();
  let mut body_closures = closure.captured_closures.clone();
  let positional = closure.positional();
  for (idx, param) in closure.params.iter().take(positional).enumerate() {
    if let Some(arg) = args.get(idx) {
      if let Some(arg_closure) = resolve_inline_closure_in_scope(ctx, arg, local_types, closures) {
        body_types.remove(param);
        body_closures.insert(param.clone(), arg_closure);
      } else {
        body_closures.remove(param);
        let evidence = infer_wasm_evidence_in_scope(ctx, arg, local_types, closures, visiting)
          .unwrap_or_else(|| WasmTypeEvidence::contract(DYNAMIC_TYPE.clone()));
        body_types.insert(param.clone(), evidence);
      }
    } else {
      body_closures.remove(param);
      if idx >= positional - closure.option_types.len() {
        if let Ok(none) = nominal_option_none_expression()
          && let Some(ev) = infer_wasm_value_evidence(ctx, &none)
        {
          body_types.insert(param.clone(), ev);
        } else {
          body_types.insert(param.clone(), WasmTypeEvidence::contract(DYNAMIC_TYPE.clone()));
        }
      } else {
        body_types.insert(param.clone(), WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Nil)));
      }
    }
  }
  if closure.rest {
    body_closures.remove(&closure.params[positional]);
    body_types.insert(
      closure.params[positional].clone(),
      WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::List(DYNAMIC_TYPE.clone()))),
    );
  }
  if args.len() < positional
    && let Some(recur_args) = closure.body.iter().find_map(find_first_recur_args)
  {
    let mut joined_opt = Vec::new();
    for (idx, param) in closure.params.iter().enumerate().take(positional).skip(args.len()) {
      if let Some(r_arg) = recur_args.get(idx)
        && let Some(init_ev) = body_types.get(param).cloned()
      {
        let r_ev = infer_wasm_evidence_in_scope(ctx, r_arg, &body_types, &body_closures, visiting);
        joined_opt.push((param.clone(), join_optional_recur_evidence(init_ev, r_ev)));
      }
    }
    for (param, ev) in joined_opt {
      body_types.insert(param, ev);
    }
  }
  let executable = closure
    .body
    .iter()
    .filter(|expr| !crate::wasm::host::builtins::syntax::is_function_metadata_hint(expr))
    .collect::<Vec<_>>();
  let result = executable
    .last()
    .and_then(|tail| infer_wasm_evidence_in_scope(ctx, tail, &body_types, &body_closures, visiting));
  visiting.remove(&visit_key);
  result
}

fn infer_param_call_arity(param_name: &str, exprs: &[Calcit]) -> Option<usize> {
  fn scan(param_name: &str, expr: &Calcit, found: &mut Option<usize>) -> bool {
    let Calcit::List(items) = expr else {
      return true;
    };
    if let Some(Calcit::Local(local)) = items.first()
      && local.sym.as_ref() == param_name
    {
      let arity = items.len() - 1;
      match *found {
        None => *found = Some(arity),
        Some(prev) if prev != arity => return false,
        _ => {}
      }
    }
    items.iter().all(|child| scan(param_name, child, found))
  }
  let mut found = None;
  for expr in exprs {
    if !scan(param_name, expr, &mut found) {
      return None;
    }
  }
  found
}

fn infer_wasm_callable_result(
  ctx: &WasmGenCtx,
  expr: &Calcit,
  args: &[Calcit],
  locals: &HashMap<String, WasmTypeEvidence>,
  closures: &HashMap<String, Arc<InlineClosure>>,
  visiting: &mut HashSet<String>,
) -> Option<WasmTypeEvidence> {
  let static_declared = infer_static_type_from_expr(expr);
  let mut declared = static_declared.clone().unwrap_or_else(|| DYNAMIC_TYPE.clone());
  let mut inferred_from_signature = false;
  let Calcit::List(items) = expr else {
    return static_declared.map(WasmTypeEvidence::contract);
  };
  if let Some(head) = items.first()
    && let Some(key) = direct_definition_reference(head)
    && let Some((ns, def)) = key.rsplit_once('/')
    && let Some(signature) = program::lookup_def_schema(ns, def).resolve_to_fn()
    && signature.return_type.contains_type_var()
  {
    let mut bindings = TypeBindings::new();
    let mut matched = true;
    for (index, argument) in args.iter().enumerate() {
      let Some(expected) = signature.arg_types.get(index).or(signature.rest_type.as_ref()) else {
        matched = false;
        break;
      };
      if let Some(expected_fn) = expected.resolve_to_fn()
        && let Some(closure) = resolve_inline_closure_in_scope(ctx, inline_closure_tail(argument).1, locals, closures)
      {
        let param_evs = expected_fn
          .arg_types
          .iter()
          .map(|t| WasmTypeEvidence::contract(replace_unbound_type_vars_with_never(&t.substitute_type_vars(&bindings))))
          .collect::<Vec<_>>();
        if let Some(ret_ev) = infer_inline_closure_result_with_evidences(ctx, &closure, &param_evs, visiting)
          && !ret_ev
            .annotation
            .prove_available_bindings(&expected_fn.return_type, &mut bindings)
            .is_mismatch()
        {
          continue;
        }
      }
      let Some(actual) = infer_wasm_evidence_in_scope(ctx, argument, locals, closures, visiting) else {
        matched = false;
        break;
      };
      let actual_ty = actual.scalar_origin.as_ref().unwrap_or(&actual.annotation);
      if !prove_wasm_arg_bindings(actual_ty, expected, &mut bindings) {
        matched = false;
        break;
      }
    }
    if matched {
      let substituted = replace_unbound_type_vars_with_never(&signature.return_type.substitute_type_vars(&bindings));
      if !substituted.contains_type_var()
        && (declared.contains_type_var()
          || matches!(
            declared.as_ref(),
            CalcitTypeAnnotation::Dynamic
              | CalcitTypeAnnotation::List(_)
              | CalcitTypeAnnotation::Map(_, _)
              | CalcitTypeAnnotation::Set(_)
          ))
      {
        declared = substituted;
        inferred_from_signature = true;
      }
    }
  }
  if declared.resolve_to_enum().is_none()
    && declared.resolve_to_struct().is_none()
    && !matches!(declared.as_ref(), CalcitTypeAnnotation::AnonymousEnum)
    && !wasm_type_requires_scalar_specialization(&declared)
  {
    return (static_declared.is_some() || inferred_from_signature).then(|| WasmTypeEvidence::contract(declared));
  }
  let (key, params, body, expected) = match items.first()? {
    Calcit::Fn { id, info } if info.scope.is_empty() && matches!(info.args.as_ref(), CalcitFnArgs::Args(_)) => (
      format!("fn:{id}"),
      fn_param_names(&info.args),
      info.body.clone(),
      info.arg_types.clone(),
    ),
    head => {
      let Some(key) = direct_definition_reference(head) else {
        return (static_declared.is_some() || inferred_from_signature).then(|| WasmTypeEvidence::contract(declared));
      };
      let Some(definition) = ctx.static_fn_defs.get(&key).filter(|definition| definition.fixed_arity) else {
        return (static_declared.is_some() || inferred_from_signature).then(|| WasmTypeEvidence::contract(declared));
      };
      (
        key,
        definition.params.clone(),
        definition.body.clone(),
        definition.arg_types.clone(),
      )
    }
  };
  if params.len() != args.len() || expected.len() != args.len() || visiting.contains(&key) {
    return (static_declared.is_some() || inferred_from_signature).then(|| WasmTypeEvidence::contract(declared));
  }
  let mut body_closures = HashMap::new();
  let scope = (|| {
    let mut bindings = TypeBindings::new();
    let mut scope = HashMap::with_capacity(params.len());
    for ((name, argument), expected) in params.iter().zip(args).zip(&expected) {
      if let Some(closure) = resolve_inline_closure_in_scope(ctx, argument, locals, closures) {
        body_closures.insert(name.clone(), closure);
      }
      let actual = infer_wasm_evidence_in_scope(ctx, argument, locals, closures, visiting)?;
      if !prove_wasm_arg_bindings(&actual.annotation, expected, &mut bindings) {
        return None;
      }
      scope.insert(name.clone(), actual);
    }
    Some(scope)
  })();
  let Some(scope) = scope else {
    return (static_declared.is_some() || inferred_from_signature).then(|| WasmTypeEvidence::contract(declared));
  };
  visiting.insert(key.clone());
  let inferred = body
    .iter()
    .rfind(|form| !crate::wasm::host::builtins::syntax::is_function_metadata_hint(form))
    .and_then(|tail| infer_wasm_evidence_in_scope(ctx, tail, &scope, &body_closures, visiting));
  visiting.remove(&key);
  inferred
    .filter(|actual| {
      (static_declared.is_none() && !inferred_from_signature)
        || actual.annotation.is_proven_for(&declared)
        || matches!(
          (actual.annotation.as_ref(), declared.as_ref()),
          (CalcitTypeAnnotation::Tuple(slots), CalcitTypeAnnotation::List(inner))
            if slots.iter().all(|slot| slot.is_proven_for(inner))
        )
    })
    .or_else(|| (static_declared.is_some() || inferred_from_signature).then(|| WasmTypeEvidence::contract(declared)))
}

fn proven_scalar_kind(annotation: &CalcitTypeAnnotation) -> Option<&'static str> {
  match annotation {
    CalcitTypeAnnotation::Number | CalcitTypeAnnotation::Numeric(_) => Some("number"),
    CalcitTypeAnnotation::Bool => Some("bool"),
    CalcitTypeAnnotation::Nil => Some("nil"),
    CalcitTypeAnnotation::Unit => Some("unit"),
    CalcitTypeAnnotation::Tag => Some("tag"),
    _ => None,
  }
}

fn nil_type_evidence_is_open(annotation: &CalcitTypeAnnotation) -> bool {
  match annotation {
    CalcitTypeAnnotation::Dynamic
    | CalcitTypeAnnotation::Nullable(_, NullableOrigin::JsHost)
    | CalcitTypeAnnotation::TypeVar(_)
    | CalcitTypeAnnotation::TypeSlot(_)
    | CalcitTypeAnnotation::Trait(_)
    | CalcitTypeAnnotation::TraitSet(_) => true,
    CalcitTypeAnnotation::Nullable(inner, NullableOrigin::Inferred) => nil_type_evidence_is_open(inner),
    _ => false,
  }
}

fn wasm_non_nil_value_is_nonzero(annotation: &CalcitTypeAnnotation) -> bool {
  matches!(
    annotation,
    CalcitTypeAnnotation::String
      | CalcitTypeAnnotation::Tag
      | CalcitTypeAnnotation::List(_)
      | CalcitTypeAnnotation::Map(_, _)
      | CalcitTypeAnnotation::Set(_)
      | CalcitTypeAnnotation::Buffer
      | CalcitTypeAnnotation::StructValue(_)
      | CalcitTypeAnnotation::EnumValue(_)
      | CalcitTypeAnnotation::AnonymousEnum
      | CalcitTypeAnnotation::Struct(_, _)
      | CalcitTypeAnnotation::Enum(_, _)
      | CalcitTypeAnnotation::StructDef(_)
      | CalcitTypeAnnotation::EnumDef(_)
      | CalcitTypeAnnotation::Any
      | CalcitTypeAnnotation::DynTrait(_)
  )
}

/// A scalar condition is either statically decided or has an unambiguous
/// nonzero representation. Numeric zero is never a false Calcit value.
fn wasm_type_has_ambiguous_nil_join(annotation: &CalcitTypeAnnotation) -> bool {
  match annotation {
    CalcitTypeAnnotation::Nullable(inner, NullableOrigin::Inferred) => {
      let expanded = expand_wasm_value_type(inner);
      matches!(
        expanded.as_ref(),
        CalcitTypeAnnotation::Bool
          | CalcitTypeAnnotation::Number
          | CalcitTypeAnnotation::Numeric(_)
          | CalcitTypeAnnotation::Dynamic
          | CalcitTypeAnnotation::TypeVar(_)
      ) || wasm_type_has_ambiguous_nil_join(expanded.as_ref())
    }
    CalcitTypeAnnotation::Nullable(_, _) => true,
    _ => false,
  }
}

fn wasm_type_requires_scalar_specialization(annotation: &CalcitTypeAnnotation) -> bool {
  let expanded = expand_wasm_value_type(&Arc::new(annotation.clone()));
  if nil_type_evidence_is_open(expanded.as_ref()) || wasm_type_has_ambiguous_nil_join(expanded.as_ref()) {
    return true;
  }
  match expanded.as_ref() {
    CalcitTypeAnnotation::Custom(_) => true,
    CalcitTypeAnnotation::Set(elem) => {
      sets::set_comparison_can_erase_kind(elem, elem, 0) || wasm_type_requires_scalar_specialization(elem)
    }
    CalcitTypeAnnotation::Map(k, v) => {
      sets::set_comparison_can_erase_kind(k, k, 0)
        || wasm_type_requires_scalar_specialization(k)
        || wasm_type_requires_scalar_specialization(v)
    }
    CalcitTypeAnnotation::List(elem) => wasm_type_requires_scalar_specialization(elem),
    CalcitTypeAnnotation::Tuple(items) => items.iter().any(|item| wasm_type_requires_scalar_specialization(item)),
    CalcitTypeAnnotation::TypeRef(_, args) | CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::Struct(_, args) => {
      args.iter().any(|arg| wasm_type_requires_scalar_specialization(arg))
    }
    _ => false,
  }
}

fn definition_requires_nil_specialization(definition: &StaticFnDef) -> bool {
  definition.nil_sensitive
    && definition
      .arg_types
      .iter()
      .any(|annotation| wasm_type_requires_scalar_specialization(annotation.as_ref()))
}

fn definition_requires_trait_specialization(definition: &StaticFnDef) -> bool {
  definition.trait_sensitive && definition.arg_types.iter().any(|annotation| annotation.contains_type_var())
}

fn function_requires_nil_specialization(ctx: &WasmGenCtx, qualified: &str, fallback: &str) -> bool {
  ctx
    .static_fn_defs
    .get(qualified)
    .or_else(|| ctx.static_fn_defs.get(fallback))
    .is_some_and(|definition| definition_requires_nil_specialization(definition))
}

fn function_requires_trait_specialization(ctx: &WasmGenCtx, qualified: &str, fallback: &str) -> bool {
  ctx
    .static_fn_defs
    .get(qualified)
    .or_else(|| ctx.static_fn_defs.get(fallback))
    .is_some_and(|definition| definition_requires_trait_specialization(definition))
}

fn reject_nil_specialized_spread_call(ctx: &WasmGenCtx, qualified: &str, fallback: &str) -> Result<(), String> {
  if function_requires_nil_specialization(ctx, qualified, fallback) {
    Err(format!(
      "E_WASM_NIL_TYPE_EVIDENCE: `{qualified}` cannot be called through a spread call before its generic argument type is known"
    ))
  } else {
    Ok(())
  }
}

fn reject_trait_specialized_spread_call(ctx: &WasmGenCtx, qualified: &str, fallback: &str) -> Result<(), String> {
  if function_requires_trait_specialization(ctx, qualified, fallback) {
    Err(format!(
      "E_WASM_TRAIT_TYPE_EVIDENCE: `{qualified}` cannot be called through a spread call before its generic argument type is known"
    ))
  } else {
    Ok(())
  }
}

fn resolve_static_specialization_types(ctx: &WasmGenCtx, definition: &StaticFnDef, args: &[Calcit]) -> Option<Vec<WasmTypeEvidence>> {
  if definition.arg_types.len() != args.len() {
    return None;
  }
  let mut bindings = TypeBindings::new();
  // Retain actual evidence, including an intentionally open transported value.
  // The operation that consumes it (nil?, numeric ordering or trait dispatch)
  // must prove its own requirements; an unrelated parameter need not be closed.
  let actuals = args
    .iter()
    .zip(definition.arg_types.iter())
    .map(|(arg, expected)| {
      let actual = infer_wasm_value_evidence(ctx, arg)?;
      if let Some(expected_fn) = expected.resolve_to_fn()
        && let Some(closure) = resolve_inline_closure(ctx, inline_closure_tail(arg).1)
      {
        let param_evs = expected_fn
          .arg_types
          .iter()
          .map(|t| WasmTypeEvidence::contract(replace_unbound_type_vars_with_never(&t.substitute_type_vars(&bindings))))
          .collect::<Vec<_>>();
        if let Some(ret_ev) = infer_inline_closure_result_with_evidences(ctx, &closure, &param_evs, &mut HashSet::new()) {
          let _ = ret_ev.annotation.prove_available_bindings(&expected_fn.return_type, &mut bindings);
        }
        return Some(actual);
      }
      if !prove_wasm_arg_bindings(&actual.annotation, expected.as_ref(), &mut bindings) {
        return None;
      }
      Some(actual)
    })
    .collect::<Option<Vec<_>>>()?;

  fn widens_closed_element_to_dynamic(actual: &Arc<CalcitTypeAnnotation>, expected: &Arc<CalcitTypeAnnotation>) -> bool {
    let actual = CalcitTypeAnnotation::expand_schema_alias(actual);
    let expected = CalcitTypeAnnotation::expand_schema_alias(expected);
    if matches!(expected.as_ref(), CalcitTypeAnnotation::Dynamic)
      && !matches!(actual.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::Never)
    {
      return true;
    }
    match (actual.as_ref(), expected.as_ref()) {
      (CalcitTypeAnnotation::List(a), CalcitTypeAnnotation::List(e)) | (CalcitTypeAnnotation::Set(a), CalcitTypeAnnotation::Set(e)) => {
        widens_closed_element_to_dynamic(a, e)
      }
      (CalcitTypeAnnotation::Map(ak, av), CalcitTypeAnnotation::Map(ek, ev)) => {
        widens_closed_element_to_dynamic(ak, ek) || widens_closed_element_to_dynamic(av, ev)
      }
      (CalcitTypeAnnotation::Tuple(items), CalcitTypeAnnotation::List(e)) => {
        items.iter().any(|item| widens_closed_element_to_dynamic(item, e))
      }
      (CalcitTypeAnnotation::Tuple(a_items), CalcitTypeAnnotation::Tuple(e_items)) => a_items
        .iter()
        .zip(e_items.iter())
        .any(|(a, e)| widens_closed_element_to_dynamic(a, e)),
      _ => false,
    }
  }

  Some(
    actuals
      .into_iter()
      .zip(definition.arg_types.iter())
      .map(|(actual, expected)| {
        let substituted = replace_unbound_type_vars_with_never(&expected.substitute_type_vars(&bindings));
        if matches!(
          actual.annotation.as_ref(),
          CalcitTypeAnnotation::Tuple(_)
            | CalcitTypeAnnotation::List(_)
            | CalcitTypeAnnotation::Set(_)
            | CalcitTypeAnnotation::Map(_, _)
        ) && !substituted.contains_type_var()
          && actual.annotation.is_proven_for(&substituted)
          && !substituted.is_proven_for(&actual.annotation)
          && !matches!(substituted.as_ref(), CalcitTypeAnnotation::Dynamic)
          && !widens_closed_element_to_dynamic(&actual.annotation, &substituted)
        {
          WasmTypeEvidence {
            annotation: substituted,
            ..actual
          }
        } else {
          actual
        }
      })
      .collect(),
  )
}

fn nominal_option_none_expression() -> Result<Calcit, String> {
  let Calcit::Enum(none) = crate::wasm::host::builtins::option_value::none().map_err(|error| error.to_string())? else {
    unreachable!("the core Option helper returns a nominal enum value")
  };
  let definition = none.sum_type.expect("the core Option value retains its definition");
  Ok(Calcit::from(vec![
    Calcit::Proc(CalcitProc::NativeNamedEnumNew),
    Calcit::EnumDef(definition),
    Calcit::tag("none"),
  ]))
}

fn nominal_option_some_expression(payload: Calcit) -> Result<Calcit, String> {
  let Calcit::Enum(none) = crate::wasm::host::builtins::option_value::none().map_err(|error| error.to_string())? else {
    unreachable!("the core Option helper returns a nominal enum value")
  };
  let definition = none.sum_type.expect("the core Option value retains its definition");
  Ok(Calcit::from(vec![
    Calcit::Proc(CalcitProc::NativeNamedEnumNew),
    Calcit::EnumDef(definition),
    Calcit::tag("some"),
    payload,
  ]))
}

fn expr_is_definitely_nil_in_ctx(ctx: &WasmGenCtx, expr: &Calcit) -> bool {
  matches!(expr, Calcit::Nil)
    || infer_wasm_static_type(ctx, expr)
      .is_some_and(|ty| matches!(CalcitTypeAnnotation::expand_schema_alias(&ty).as_ref(), CalcitTypeAnnotation::Nil))
}

fn wrap_nil_expr_as_none(expr: &Calcit) -> Result<Calcit, String> {
  let none = nominal_option_none_expression()?;
  if matches!(expr, Calcit::Nil) {
    return Ok(none);
  }
  // Preserve side effects or runtime failures inside definitely-nil forms
  // (e.g. `(&let () (println ...) nil)`) before returning `Option :none`.
  Ok(Calcit::from(vec![
    Calcit::Syntax(CalcitSyntax::CoreLet, Arc::from(crate::wasm::host::calcit::CORE_NS)),
    Calcit::from(Vec::<Calcit>::new()),
    expr.clone(),
    none,
  ]))
}

fn lower_nil_to_option_target(ctx: &WasmGenCtx, arg: &Calcit) -> Result<Option<Calcit>, String> {
  if expr_is_definitely_nil_in_ctx(ctx, arg) {
    return wrap_nil_expr_as_none(arg).map(Some);
  }
  if let Calcit::List(items) = arg
    && matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::If, _)))
    && matches!(items.len(), 3 | 4)
  {
    let cond = items[1].clone();
    let Some(then_lowered) = lower_nil_to_option_target(ctx, &items[2])? else {
      return Ok(None);
    };
    let else_lowered = match items.get(3) {
      Some(branch) => match lower_nil_to_option_target(ctx, branch)? {
        Some(lowered) => lowered,
        None => return Ok(None),
      },
      None => nominal_option_none_expression()?,
    };
    return Ok(Some(Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::If, Arc::from(crate::wasm::host::calcit::CORE_NS)),
      cond,
      then_lowered,
      else_lowered,
    ])));
  }
  if let Some(ty) = infer_wasm_static_type(ctx, arg) {
    let expanded = expand_wasm_value_type(&ty);
    if !nil_type_evidence_is_open(expanded.as_ref())
      && !matches!(expanded.as_ref(), CalcitTypeAnnotation::Nil | CalcitTypeAnnotation::Nullable(..))
    {
      return nominal_option_some_expression(arg.clone()).map(Some);
    }
  }
  Ok(None)
}

fn is_direct_nil_to_option_call_on_var(expr: &Calcit, var_name: &str) -> bool {
  let Calcit::List(items) = expr else {
    return false;
  };
  if items.len() != 2 {
    return false;
  }
  let is_nil_to_option = match &items[0] {
    Calcit::Import(import) => import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS && import.def.as_ref() == "nil->option",
    Calcit::Symbol { sym, info, .. } => info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS && sym.as_ref() == "nil->option",
    _ => false,
  };
  if !is_nil_to_option {
    return false;
  }
  match &items[1] {
    Calcit::Local(local) => local.sym.as_ref() == var_name,
    Calcit::Symbol { sym, .. } => sym.as_ref() == var_name,
    _ => false,
  }
}

fn try_emit_direct_niche_option_i32(ctx: &mut WasmGenCtx, expr: &Calcit) -> Result<Option<(u32, Arc<CalcitTypeAnnotation>)>, String> {
  let Calcit::List(items) = expr else {
    return Ok(None);
  };
  let Some(head) = items.first() else {
    return Ok(None);
  };
  if matches!(head, Calcit::Syntax(CalcitSyntax::AssertType, _)) && items.len() >= 2 {
    return try_emit_direct_niche_option_i32(ctx, &items[1]);
  }
  let is_struct_nth = matches!(head, Calcit::Proc(CalcitProc::NativeStructNth))
    || matches!(head, Calcit::Import(imp) if imp.ns.as_ref() == "calcit.core" && imp.def.as_ref() == "&struct:nth");
  if is_struct_nth
    && items.len() >= 3
    && let Calcit::Number(n) = &items[2]
    && n.is_finite()
    && *n >= 0.0
    && n.fract() == 0.0
  {
    let idx = *n as usize;
    if let Some(struct_def) = infer_wasm_static_type(ctx, &items[1]).and_then(|ty| ty.resolve_to_struct())
      && let Ok(layout_id) = ctx.struct_layouts.id(&struct_def)
      && let Ok(layout) = ctx.struct_layouts.layout_by_id(layout_id).cloned()
      && let Some(&field_layout) = layout.fields.get(idx)
      && field_layout.kind == StructFieldKind::OptionHeapPtrI32
      && let Some(field_ty) = struct_def.field_types.get(idx)
      && let Some(inner_ty) = option_niche_inner(&qualify_for_definition_ref(field_ty, struct_def.definition_ref.as_deref()))
    {
      let ptr_local = emit_ptr_to_i32(ctx, &items[1])?;
      let raw_opt = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalGet(ptr_local));
      ctx.emit(Instruction::I32Load(mem_arg_i32(u64::from(field_layout.offset))));
      ctx.emit(Instruction::LocalSet(raw_opt));
      return Ok(Some((raw_opt, inner_ty)));
    }
  }

  let is_enum_nth = matches!(head, Calcit::Proc(CalcitProc::NativeEnumNth))
    || matches!(head, Calcit::Import(imp) if imp.ns.as_ref() == "calcit.core" && imp.def.as_ref() == "&enum:nth");
  if is_enum_nth
    && items.len() == 3
    && let Calcit::Number(n) = &items[2]
    && n.is_finite()
    && *n >= 1.0
    && n.fract() == 0.0
    && *n <= f64::from(u16::MAX)
  {
    let idx = *n as usize;
    let payload_idx = idx - 1;
    if let Some(enum_def) = infer_wasm_static_type(ctx, &items[1]).and_then(|ty| ty.resolve_to_enum())
      && enum_def.generics().is_empty()
    {
      let def_ref = enum_def.definition_ref().map(|r| r.as_ref());
      let mut common: Option<(u16, Arc<CalcitTypeAnnotation>)> = None;
      let mut all_match = false;
      for variant in enum_def.variants() {
        if variant.payload_types().len() <= payload_idx {
          continue;
        }
        let Some((_, layout)) = ctx
          .enum_layouts
          .get_for_def(&enum_def, variant.tag.ref_str(), variant.payload_types().len())
        else {
          all_match = false;
          break;
        };
        let field_layout = layout.fields[payload_idx];
        if field_layout.kind != StructFieldKind::OptionHeapPtrI32 {
          all_match = false;
          break;
        }
        let qualified = qualify_for_definition_ref(&variant.payload_types()[payload_idx], def_ref);
        let Some(inner_ty) = option_niche_inner(&qualified) else {
          all_match = false;
          break;
        };
        match &common {
          None => {
            common = Some((field_layout.offset, inner_ty));
            all_match = true;
          }
          Some((prev_off, prev_ty)) if *prev_off == field_layout.offset && *prev_ty == inner_ty => {}
          Some(_) => {
            all_match = false;
            break;
          }
        }
      }
      if all_match && let Some((offset, inner_ty)) = common {
        let ptr_local = emit_ptr_to_i32(ctx, &items[1])?;
        ctx.emit(Instruction::LocalGet(ptr_local));
        ctx.emit(Instruction::I32Load16U(mem_arg_i16(0)));
        ctx.emit(Instruction::I32Const(idx as i32));
        ctx.emit(Instruction::I32LtU);
        ctx.begin_block_if();
        ctx.emit(Instruction::Unreachable);
        ctx.emit(Instruction::End);

        let raw_opt = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(ptr_local));
        ctx.emit(Instruction::I32Load16U(mem_arg_i16(2)));
        ctx.emit(Instruction::I32Eqz);
        ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
        ctx.emit(Instruction::LocalGet(ptr_local));
        ctx.emit(Instruction::F64Load(mem_arg_f64((idx * 8) as u64)));
        ctx.emit(Instruction::I32TruncF64U);
        ctx.call_rt("__rt_option_to_niche_ptr");
        ctx.emit(Instruction::Else);
        ctx.emit(Instruction::LocalGet(ptr_local));
        ctx.emit(Instruction::I32Load(mem_arg_i32(u64::from(offset))));
        ctx.emit(Instruction::End);
        ctx.emit(Instruction::LocalSet(raw_opt));
        return Ok(Some((raw_opt, inner_ty)));
      }
    }
  }

  Ok(None)
}

fn try_emit_direct_niche_option_call(
  ctx: &mut WasmGenCtx,
  qualified: &str,
  args: &[Calcit],
  dst_repr: NativeValRepr,
) -> Result<bool, String> {
  match (qualified, args.len()) {
    ("calcit.core/option:some?", 1) => {
      if let Some((raw_opt, _)) = try_emit_direct_niche_option_i32(ctx, &args[0])? {
        ctx.emit(Instruction::LocalGet(raw_opt));
        ctx.emit(Instruction::I32Const(0));
        ctx.emit(Instruction::I32Ne);
        emit_convert_repr(ctx, NativeValRepr::BoolI32, dst_repr);
        return Ok(true);
      }
    }
    ("calcit.core/option:none?", 1) => {
      if let Some((raw_opt, _)) = try_emit_direct_niche_option_i32(ctx, &args[0])? {
        ctx.emit(Instruction::LocalGet(raw_opt));
        ctx.emit(Instruction::I32Eqz);
        emit_convert_repr(ctx, NativeValRepr::BoolI32, dst_repr);
        return Ok(true);
      }
    }
    ("calcit.core/option:unwrap", 1) => {
      if let Some((raw_opt, _)) = try_emit_direct_niche_option_i32(ctx, &args[0])? {
        ctx.emit(Instruction::LocalGet(raw_opt));
        ctx.emit(Instruction::I32Eqz);
        ctx.begin_block_if();
        ctx.emit(Instruction::Unreachable);
        ctx.emit(Instruction::End);
        ctx.emit(Instruction::LocalGet(raw_opt));
        emit_convert_repr(ctx, NativeValRepr::HeapPtrI32, dst_repr);
        return Ok(true);
      }
    }
    ("calcit.core/option:unwrap-or", 2) => {
      if let Some((raw_opt, _)) = try_emit_direct_niche_option_i32(ctx, &args[0])? {
        let fallback = ctx.alloc_local_typed(dst_repr.val_type());
        emit_expr_as_repr(ctx, &args[1], dst_repr)?;
        ctx.emit(Instruction::LocalSet(fallback));
        ctx.emit(Instruction::LocalGet(raw_opt));
        ctx.emit(Instruction::I32Eqz);
        ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(dst_repr.val_type())));
        ctx.emit(Instruction::LocalGet(fallback));
        ctx.emit(Instruction::Else);
        ctx.emit(Instruction::LocalGet(raw_opt));
        emit_convert_repr(ctx, NativeValRepr::HeapPtrI32, dst_repr);
        ctx.emit(Instruction::End);
        return Ok(true);
      }
    }
    _ => {}
  }
  Ok(false)
}

fn emit_specialized_static_call(ctx: &mut WasmGenCtx, qualified: &str, args: &[Calcit]) -> Result<bool, String> {
  if try_emit_direct_niche_option_call(ctx, qualified, args, NativeValRepr::F64)? {
    return Ok(true);
  }
  let Some(definition) = ctx.static_fn_defs.get(qualified).cloned() else {
    return Ok(false);
  };

  let optional = crate::wasm::host::calcit::trailing_option_arg_count(&definition.arg_types, definition.params.len());
  let completed_args =
    if definition.fixed_arity && args.len() < definition.params.len() && args.len() >= definition.params.len() - optional {
      // Complete the language's trailing Option convention before selecting a
      // specialization. Legacy `?` parameters still use their existing nil path.
      let mut completed = args.to_vec();
      completed.resize(definition.params.len(), nominal_option_none_expression()?);
      Some(completed)
    } else {
      None
    };
  if ctx.specialization_stack.is_empty() && ctx.source_fn_index.is_some() && ctx.source_fn_index == ctx.fn_index.get(qualified).copied()
  {
    if let Some(completed) = completed_args.as_deref() {
      let function = *ctx
        .fn_index
        .get(qualified)
        .ok_or_else(|| format!("unknown function: {qualified}"))?;
      emit_direct_fn_call(ctx, function, completed.len() as u32, None, completed, NativeValRepr::F64)?;
      return Ok(true);
    }
    return Ok(false);
  }
  let args = completed_args.as_deref().unwrap_or(args);

  let needs_nil_specialization = definition_requires_nil_specialization(&definition);
  let needs_trait_specialization = definition_requires_trait_specialization(&definition);
  let nil_argument_types = if needs_nil_specialization {
    Some(resolve_static_specialization_types(ctx, &definition, args).ok_or_else(|| {
      format!("E_WASM_NIL_TYPE_EVIDENCE: `{qualified}` requires concrete argument types so nil can be distinguished from false and 0")
    })?)
  } else {
    None
  };
  let trait_argument_types = if needs_trait_specialization && nil_argument_types.is_none() {
    Some(resolve_static_specialization_types(ctx, &definition, args).ok_or_else(|| {
      format!("E_WASM_TRAIT_TYPE_EVIDENCE: `{qualified}` requires concrete argument types to select a trait implementation")
    })?)
  } else {
    None
  };
  // A nominal data contract alone does not identify its attached impls for non-coherent types.
  // Preserve the actual argument's identity only when needed.
  let method_argument_types = if (definition.method_sensitive || definition.trait_sensitive)
    && definition.fixed_arity
    && !ctx.specialization_stack.iter().any(|active| active == qualified)
    && nil_argument_types.is_none()
    && trait_argument_types.is_none()
  {
    resolve_static_specialization_types(ctx, &definition, args).filter(|actuals| {
      actuals.iter().zip(definition.arg_types.iter()).any(|(actual, expected)| {
        actual.exact_impls
          && (actual.annotation.resolve_to_enum().is_some() || actual.annotation.resolve_to_struct().is_some())
          && !(!expected.contains_type_var()
            && is_statically_coherent_nominal(ctx, &actual.annotation)
            && is_statically_coherent_nominal(ctx, expected))
      })
    })
  } else {
    None
  };

  let uses_recur = definition.body.iter().any(check_uses_recur);
  let can_inline_callbacks = !uses_recur
    && definition.fixed_arity
    && definition.params.len() == args.len()
    && !ctx.specialization_stack.iter().any(|active| active == qualified);
  let closure_args = args
    .iter()
    .enumerate()
    .map(|(index, arg)| {
      if !can_inline_callbacks {
        return None;
      }
      let (effects, tail) = inline_closure_tail(arg);
      let closure = resolve_inline_closure(ctx, tail)?;
      let expected_arity = definition.callback_arities.get(&index)?;
      closure.accepts(*expected_arity).then_some((effects, tail))
    })
    .collect::<Vec<_>>();
  // A known callback also needs call-site specialization when no scalar or
  // trait proof requires it. Its signature alone cannot identify the target.
  let known_callback = !uses_recur
    && definition.fixed_arity
    && definition.params.len() == args.len()
    && !ctx.specialization_stack.iter().any(|active| active == qualified)
    && definition
      .callback_arities
      .keys()
      .any(|index| args.get(*index).and_then(|arg| resolve_known_function_value(ctx, arg)).is_some());
  if closure_args.iter().all(Option::is_none)
    && nil_argument_types.is_none()
    && trait_argument_types.is_none()
    && method_argument_types.is_none()
    && !known_callback
  {
    let Some(completed) = completed_args.as_deref() else {
      return Ok(false);
    };
    // Omission alone does not require inlining. Preserve the compiled body's
    // ordinary recursive calls and recur loop while passing nominal None.
    let function = *ctx
      .fn_index
      .get(qualified)
      .ok_or_else(|| format!("unknown function: {qualified}"))?;
    emit_direct_fn_call(ctx, function, completed.len() as u32, None, completed, NativeValRepr::F64)?;
    return Ok(true);
  }
  if !definition.fixed_arity || definition.params.len() != args.len() {
    return Err(format!(
      "E_WASM_CALL_SPECIALIZATION: `{qualified}` requires a fixed known arity for call-site specialization"
    ));
  }
  if ctx.specialization_stack.iter().any(|active| active == qualified) {
    return Err(format!(
      "E_WASM_CALL_SPECIALIZATION: recursive specialization of `{qualified}` is not supported"
    ));
  }

  let argument_types = nil_argument_types
    .or(trait_argument_types)
    .or(method_argument_types)
    .unwrap_or_else(|| {
      args
        .iter()
        .map(|arg| infer_wasm_value_evidence(ctx, arg))
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
    });
  if uses_recur && (closure_args.iter().any(Option::is_some) || argument_types.len() != args.len()) {
    return Err(format!(
      "E_WASM_CALL_SPECIALIZATION: `{qualified}` requires value arguments with proven loop-carried types"
    ));
  }
  let mut bindings = Vec::with_capacity(args.len());
  for (index, (arg, closure_arg)) in args.iter().zip(closure_args).enumerate() {
    if let Some((effects, tail)) = closure_arg {
      for effect in effects {
        emit_expr(ctx, effect)?;
        ctx.emit(Instruction::Drop);
      }
      let closure = resolve_inline_closure(ctx, tail).expect("validated inline closure");
      let param_name = &definition.params[index];
      let needs_materialization =
        closure::closure_name_may_escape(param_name, &definition.body) || closure::inline_closure_requires_materialization(&closure);
      if needs_materialization {
        let local = closure::emit_materialize_inline_closure(ctx, &closure, Some(tail))?;
        let mut materialized = (*closure).clone();
        materialized.materialized_local = Some(local);
        bindings.push(InlineArgument::Closure(Arc::new(materialized), Some(local)));
      } else {
        bindings.push(InlineArgument::Closure(closure, None));
      }
    } else {
      let function_value = (!uses_recur).then(|| resolve_known_function_value(ctx, arg)).flatten();
      let number_literal = (!uses_recur).then(|| resolve_known_number_literal(ctx, arg)).flatten();
      emit_expr(ctx, arg)?;
      let local = ctx.alloc_local();
      ctx.emit(Instruction::LocalSet(local));
      // Non-recursive specialization gives this immutable parameter a fresh
      // slot, just like an inline closure argument. A recur slot may change.
      if let Some(definition) = function_value {
        ctx.local_function_values.insert(local, definition);
      }
      if let Some(n) = number_literal {
        ctx.local_number_literals.insert(local, n);
      }
      bindings.push(InlineArgument::Value(local));
    }
  }

  let caller_locals = std::mem::take(&mut ctx.locals);
  let caller_local_types = std::mem::take(&mut ctx.local_types);
  let caller_closures = std::mem::take(&mut ctx.lambda_locals);
  let mut parameter_locals = Vec::with_capacity(definition.params.len());
  for (index, (param, binding)) in definition.params.iter().zip(bindings).enumerate() {
    match binding {
      InlineArgument::Value(local) => {
        parameter_locals.push(local);
        ctx.locals.insert(param.clone(), local);
        if let Some(annotation) = argument_types.get(index) {
          ctx.local_types.insert(param.clone(), annotation.clone());
        }
      }
      InlineArgument::Closure(closure, materialized) => {
        let self_recursive = closure::inline_closure_requires_materialization(&closure);
        if !self_recursive {
          ctx.lambda_locals.insert(param.clone(), closure);
        }
        if let Some(local) = materialized {
          ctx.locals.insert(param.clone(), local);
        }
      }
    }
  }
  ctx.specialization_stack.push(qualified.to_owned());
  let result = if uses_recur {
    reject_non_tail_recur(&definition.body, qualified)
      .and_then(|()| emit_selected_recur_loop(ctx, &definition.params, parameter_locals, argument_types, &definition.body))
  } else {
    emit_body(ctx, &definition.body)
  };
  ctx.specialization_stack.pop();
  ctx.locals = caller_locals;
  ctx.local_types = caller_local_types;
  ctx.lambda_locals = caller_closures;
  result.map_err(|error| format!("{error} (specializing `{qualified}`)"))?;
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

fn enum_contains_has_open_key(receiver: Option<&CalcitTypeAnnotation>, key: Option<&CalcitTypeAnnotation>) -> bool {
  let Some(receiver) = receiver else {
    return false;
  };
  if !matches!(
    receiver,
    CalcitTypeAnnotation::AnonymousEnum | CalcitTypeAnnotation::EnumValue(_) | CalcitTypeAnnotation::Enum(_, _)
  ) && receiver.resolve_to_enum().is_none()
  {
    return false;
  }
  key.is_none_or(|annotation| {
    !annotation.is_proven_for(&CalcitTypeAnnotation::Number)
      && (nil_type_evidence_is_open(annotation)
        || matches!(
          annotation,
          CalcitTypeAnnotation::Nullable(_, _) | CalcitTypeAnnotation::TypeRef(_, _)
        ))
  })
}

fn direct_scalar_type_test(expr: &Calcit) -> bool {
  let any_open_operand = |items: &crate::wasm::host::calcit::CalcitList| {
    items.iter().skip(1).any(|operand| {
      infer_static_type_from_expr(operand).is_none_or(|annotation| wasm_type_requires_scalar_specialization(&annotation))
    })
  };
  match expr {
    Calcit::Proc(CalcitProc::NilQuestion) => true,
    Calcit::List(items)
      if matches!(
        items.first(),
        Some(Calcit::Proc(
          CalcitProc::NumberQuestion | CalcitProc::TagQuestion | CalcitProc::BoolQuestion | CalcitProc::TypeOf
        ))
      ) =>
    {
      let proven_scalar = items
        .get(1)
        .and_then(infer_static_type_from_expr)
        .is_some_and(|annotation| annotation.is_proven_for(&CalcitTypeAnnotation::Number) || proven_scalar_kind(&annotation).is_some());
      // Do not revisit the proc head as a bare predicate value. Its proven
      // operand needs no specialization of unrelated generic parameters.
      !proven_scalar || items.iter().skip(1).any(direct_scalar_type_test)
    }
    Calcit::List(items) => {
      let direct = match items.first() {
        Some(Calcit::Proc(CalcitProc::NilQuestion)) => true,
        Some(Calcit::Proc(CalcitProc::NativeLessThan | CalcitProc::NativeGreaterThan)) => items.iter().skip(1).any(|operand| {
          infer_static_type_from_expr(operand).is_none_or(|annotation| !annotation.is_proven_for(&CalcitTypeAnnotation::Number))
        }),
        Some(Calcit::Proc(
          CalcitProc::NativeEquals
          | CalcitProc::Set
          | CalcitProc::NativeInclude
          | CalcitProc::NativeExclude
          | CalcitProc::NativeSetIncludes
          | CalcitProc::NativeUnion
          | CalcitProc::NativeDifference
          | CalcitProc::NativeSetIntersection
          | CalcitProc::NativeListToSet
          | CalcitProc::NativeMap
          | CalcitProc::NativeMapGet
          | CalcitProc::NativeMapContains
          | CalcitProc::NativeMapAssoc
          | CalcitProc::NativeMapDissoc
          | CalcitProc::NativeMerge
          | CalcitProc::NativeMergeNonNil
          | CalcitProc::NativeMapDiffNew
          | CalcitProc::NativeMapDiffKeys
          | CalcitProc::NativeMapCommonKeys
          | CalcitProc::NativeMapDiffTriple
          | CalcitProc::FormatCirruEdn
          | CalcitProc::JsonStringify
          | CalcitProc::JsonPretty,
        )) => any_open_operand(items),
        // Reuse scalar specialization for open conditions instead of treating
        // the erased f64 payload as a Boolean or adding a dynamic runtime tag.
        Some(Calcit::Syntax(CalcitSyntax::If, _)) => items
          .get(1)
          .and_then(infer_static_type_from_expr)
          .is_none_or(|annotation| wasm_type_requires_scalar_specialization(&annotation)),
        Some(Calcit::Import(import)) if import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS => match import.def.as_ref() {
          "nil?" => true,
          "=" | "not=" | "union" | "difference" | "intersection" | "include" | "exclude" | "pairs-map" | "merge" | "zipmap"
          | "dissoc" => any_open_operand(items),
          _ => false,
        },
        Some(Calcit::Symbol { sym, info, .. }) if info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS => match sym.as_ref() {
          "nil?" => true,
          "=" | "not=" | "union" | "difference" | "intersection" | "include" | "exclude" | "pairs-map" | "merge" | "zipmap"
          | "dissoc" => any_open_operand(items),
          _ => false,
        },
        Some(Calcit::Method(name, MethodKind::Invoke(_))) if name.as_ref() == "contains?" => enum_contains_has_open_key(
          items.get(1).and_then(infer_static_type_from_expr).as_deref(),
          items.get(2).and_then(infer_static_type_from_expr).as_deref(),
        ),
        _ => false,
      };
      direct || items.iter().any(direct_scalar_type_test)
    }
    _ => false,
  }
}

fn static_call_target(
  head: &Calcit,
  current_ns: &str,
  qualified_names: &HashSet<String>,
  unique_names: &HashMap<String, String>,
) -> Option<String> {
  match head {
    Calcit::Import(import) => Some(format!("{}/{}", import.ns, import.def)),
    Calcit::Symbol { sym, info, .. } => {
      let scoped = format!("{}/{}", info.at_ns, sym);
      if qualified_names.contains(&scoped) {
        Some(scoped)
      } else {
        let local = format!("{current_ns}/{sym}");
        qualified_names
          .contains(&local)
          .then_some(local)
          .or_else(|| unique_names.get(sym.as_ref()).cloned())
      }
    }
    Calcit::Fn { info, .. } => info
      .def_ref
      .as_ref()
      .map(|reference| format!("{}/{}", reference.def_ns, reference.def_name)),
    _ => None,
  }
}

fn expression_calls_sensitive(
  expr: &Calcit,
  current_ns: &str,
  sensitive: &HashSet<String>,
  qualified_names: &HashSet<String>,
  unique_names: &HashMap<String, String>,
) -> bool {
  let Calcit::List(items) = expr else {
    return false;
  };
  if items
    .first()
    .and_then(|head| static_call_target(head, current_ns, qualified_names, unique_names))
    .is_some_and(|target| sensitive.contains(&target))
  {
    return true;
  }
  items
    .iter()
    .any(|item| expression_calls_sensitive(item, current_ns, sensitive, qualified_names, unique_names))
}

fn collect_sensitive_defs(
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  export_name_counts: &HashMap<String, usize>,
  direct_predicate: impl Fn(&str, &str, &CalcitFnArgs, &Calcit) -> bool,
) -> HashSet<String> {
  let qualified_names = fn_defs
    .iter()
    .map(|(ns, name, _, _)| format!("{ns}/{name}"))
    .collect::<HashSet<_>>();
  let unique_names = fn_defs
    .iter()
    .filter(|(_, name, _, _)| export_name_counts.get(name.as_str()).copied() == Some(1))
    .map(|(ns, name, _, _)| (name.clone(), format!("{ns}/{name}")))
    .collect::<HashMap<_, _>>();
  let mut sensitive = fn_defs
    .iter()
    .filter(|(ns, name, args, body)| body.iter().any(|expr| direct_predicate(ns, name, args, expr)))
    .map(|(ns, name, _, _)| format!("{ns}/{name}"))
    .collect::<HashSet<_>>();

  loop {
    let mut changed = false;
    for (ns, name, _, body) in fn_defs {
      let qualified = format!("{ns}/{name}");
      if sensitive.contains(&qualified) {
        continue;
      }
      if body
        .iter()
        .any(|expr| expression_calls_sensitive(expr, ns, &sensitive, &qualified_names, &unique_names))
      {
        sensitive.insert(qualified);
        changed = true;
      }
    }
    if !changed {
      return sensitive;
    }
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
    "intersperse" => Some(build(|ctx| emit_join_from_locals(ctx, 0, 1), export_name, env)),
    "join-string" => Some(build(|ctx| emit_join_string_from_locals(ctx, 0, 1), export_name, env)),
    "try-parse-json" => Some((|| {
      let mut ctx = WasmGenCtx::new(1, env.clone());
      ctx.locals.insert("__p0__".into(), 0);
      ctx.arg_indices.push(0);
      edn_parse::emit_try_parse_json_from_local(&mut ctx, 0)?;
      Ok(CompiledFn {
        export_name: Some(export_name.to_owned()),
        params: vec![ValType::F64],
        results: vec![ValType::F64],
        locals: ctx.extra_locals,
        instructions: ctx.instructions,
      })
    })()),
    "json:parse" => Some((|| {
      let mut ctx = WasmGenCtx::new(1, env.clone());
      ctx.locals.insert("__p0__".into(), 0);
      ctx.arg_indices.push(0);
      edn_parse::emit_try_parse_json_value_from_local(&mut ctx, 0)?;
      Ok(CompiledFn {
        export_name: Some(export_name.to_owned()),
        params: vec![ValType::F64],
        results: vec![ValType::F64],
        locals: ctx.extra_locals,
        instructions: ctx.instructions,
      })
    })()),
    _ => None,
  }
}

fn program_has_boxed_json_scalars(
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  value_imports: &HashMap<String, Calcit>,
) -> bool {
  fn expr_uses_json_parse(expr: &Calcit) -> bool {
    match expr {
      Calcit::Import(imp) => {
        imp.ns.as_ref() == crate::wasm::host::calcit::CORE_NS
          && matches!(imp.def.as_ref(), "try-parse-json" | "&try-parse-json" | "json-parse")
      }
      Calcit::Symbol { sym, .. } => matches!(sym.as_ref(), "try-parse-json" | "&try-parse-json" | "json-parse"),
      Calcit::Method(name, MethodKind::Invoke(_)) => name.as_ref() == "parse-json",
      Calcit::Proc(CalcitProc::JsonParse | CalcitProc::NativeTryParseJson) => true,
      Calcit::List(items) => items.iter().any(expr_uses_json_parse),
      _ => false,
    }
  }
  for (ns, def_name, _, body) in fn_defs {
    if ns == crate::wasm::host::calcit::CORE_NS
      && matches!(
        def_name.as_str(),
        "try-parse-json" | "&try-parse-json" | "json-parse" | "parse-json" | "&core-string-methods"
      )
    {
      continue;
    }
    if body.iter().any(expr_uses_json_parse) {
      return true;
    }
  }
  for (qualified, expr) in value_imports {
    if matches!(
      qualified.as_str(),
      "calcit.core/try-parse-json"
        | "calcit.core/&try-parse-json"
        | "calcit.core/json-parse"
        | "calcit.core/&core-string-methods"
        | "calcit.core/String"
    ) {
      continue;
    }
    if expr_uses_json_parse(expr) {
      return true;
    }
  }
  false
}

fn expr_is_guaranteed_unboxed_scalar(ctx: &WasmGenCtx, expr: &Calcit) -> bool {
  if !ctx.has_boxed_json_scalars {
    return true;
  }
  match expr {
    Calcit::Bool(_) | Calcit::Number(_) | Calcit::Nil | Calcit::Unit | Calcit::Tag(_) => true,
    Calcit::Local(local) => ctx
      .locals
      .get(local.sym.as_ref())
      .is_some_and(|&idx| ctx.local_repr(idx) != NativeValRepr::F64),
    Calcit::Symbol { sym, .. } => ctx
      .locals
      .get(sym.as_ref())
      .is_some_and(|&idx| ctx.local_repr(idx) != NativeValRepr::F64),
    Calcit::List(items) if !items.is_empty() => match &items[0] {
      Calcit::Syntax(CalcitSyntax::AssertType, _) if items.len() == 3 => {
        let target = expand_wasm_value_type(&CalcitTypeAnnotation::parse_type_annotation_form(&items[2]));
        matches!(
          target.as_ref(),
          CalcitTypeAnnotation::Bool
            | CalcitTypeAnnotation::Nil
            | CalcitTypeAnnotation::Unit
            | CalcitTypeAnnotation::Tag
            | CalcitTypeAnnotation::Number
            | CalcitTypeAnnotation::Numeric(_)
        )
      }
      Calcit::Proc(proc) => matches!(
        proc,
        CalcitProc::NativeAdd
          | CalcitProc::NativeMinus
          | CalcitProc::NativeMultiply
          | CalcitProc::NativeDivide
          | CalcitProc::NativeNumberRem
          | CalcitProc::NativeLessThan
          | CalcitProc::NativeGreaterThan
          | CalcitProc::NativeEquals
          | CalcitProc::Identical
          | CalcitProc::Not
          | CalcitProc::NativeCompare
          | CalcitProc::BitAnd
          | CalcitProc::BitOr
          | CalcitProc::BitXor
          | CalcitProc::BitShl
          | CalcitProc::BitShr
          | CalcitProc::BitNot
          | CalcitProc::NilQuestion
          | CalcitProc::BoolQuestion
          | CalcitProc::NumberQuestion
          | CalcitProc::StringQuestion
          | CalcitProc::TagQuestion
          | CalcitProc::ListQuestion
          | CalcitProc::MapQuestion
          | CalcitProc::SetQuestion
          | CalcitProc::SymbolQuestion
          | CalcitProc::StructQuestion
          | CalcitProc::EnumQuestion
          | CalcitProc::FnQuestion
          | CalcitProc::NativeStructCount
          | CalcitProc::NativeEnumCount
          | CalcitProc::NativeStructMatches
          | CalcitProc::NativeStructContains
          | CalcitProc::NativeListEmpty
          | CalcitProc::NativeListCount
          | CalcitProc::NativeMapEmpty
          | CalcitProc::NativeMapCount
          | CalcitProc::NativeMapContains
          | CalcitProc::NativeMapIncludes
          | CalcitProc::NativeSetEmpty
          | CalcitProc::NativeSetCount
          | CalcitProc::NativeSetIncludes
      ),
      Calcit::Import(imp) => {
        let qualified = format!("{}/{}", imp.ns, imp.def);
        ctx
          .fn_index
          .get(&qualified)
          .or_else(|| ctx.fn_index.get(imp.def.as_ref()))
          .and_then(|idx| ctx.fn_signatures.get(idx))
          .is_some_and(|sig| sig.result != NativeValRepr::F64)
      }
      _ => false,
    },
    _ => false,
  }
}

fn emit_box_repr_to_f64(ctx: &mut WasmGenCtx, src: NativeValRepr) {
  match src {
    NativeValRepr::F64 => {}
    NativeValRepr::BoolI32 | NativeValRepr::TagI32 | NativeValRepr::UInt32 | NativeValRepr::HeapPtrI32 => {
      ctx.emit(Instruction::F64ConvertI32U);
    }
    NativeValRepr::UnitI32 => {
      ctx.emit(Instruction::Drop);
      ctx.emit(f64_const(0.0));
    }
    NativeValRepr::Int32S => ctx.emit(Instruction::F64ConvertI32S),
    NativeValRepr::Int64S => ctx.emit(Instruction::F64ConvertI64S),
    NativeValRepr::UInt64 => ctx.emit(Instruction::F64ConvertI64U),
  }
}

fn emit_unbox_f64_to_repr(ctx: &mut WasmGenCtx, dst: NativeValRepr) {
  match dst {
    NativeValRepr::F64 => {}
    NativeValRepr::BoolI32 => {
      if ctx.has_boxed_json_scalars && ctx.tag_index.contains_key("bool") {
        let tmp = ctx.alloc_local();
        ctx.emit(Instruction::LocalSet(tmp));
        emit_unbox_scalar_if_tag(ctx, tmp, "bool", false);
        ctx.emit(Instruction::LocalGet(tmp));
      }
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Ne);
    }
    NativeValRepr::TagI32 | NativeValRepr::UInt32 | NativeValRepr::HeapPtrI32 => {
      ctx.emit(Instruction::I32TruncF64U);
    }
    NativeValRepr::UnitI32 => {
      ctx.emit(Instruction::Drop);
      ctx.emit(Instruction::I32Const(0));
    }
    NativeValRepr::Int32S => ctx.emit(Instruction::I32TruncF64S),
    NativeValRepr::Int64S => ctx.emit(Instruction::I64TruncF64S),
    NativeValRepr::UInt64 => ctx.emit(Instruction::I64TruncF64U),
  }
}

fn emit_convert_repr(ctx: &mut WasmGenCtx, src: NativeValRepr, dst: NativeValRepr) {
  if src == dst {
    return;
  }
  match (src, dst) {
    (_, NativeValRepr::F64) => emit_box_repr_to_f64(ctx, src),
    (NativeValRepr::F64, _) => emit_unbox_f64_to_repr(ctx, dst),
    (_, NativeValRepr::UnitI32) => {
      ctx.emit(Instruction::Drop);
      ctx.emit(Instruction::I32Const(0));
    }
    (NativeValRepr::UnitI32, NativeValRepr::BoolI32)
    | (NativeValRepr::UnitI32, NativeValRepr::TagI32)
    | (NativeValRepr::UnitI32, NativeValRepr::UInt32)
    | (NativeValRepr::UnitI32, NativeValRepr::HeapPtrI32)
    | (NativeValRepr::UnitI32, NativeValRepr::Int32S) => {
      // Already i32(0) on stack
    }
    (NativeValRepr::BoolI32, NativeValRepr::UInt32 | NativeValRepr::TagI32 | NativeValRepr::Int32S)
    | (
      NativeValRepr::TagI32 | NativeValRepr::UInt32 | NativeValRepr::HeapPtrI32,
      NativeValRepr::TagI32 | NativeValRepr::UInt32 | NativeValRepr::HeapPtrI32,
    ) => {
      // Same unsigned 32-bit bit pattern
    }
    (NativeValRepr::Int32S, NativeValRepr::Int64S) => ctx.emit(Instruction::I64ExtendI32S),
    (
      NativeValRepr::BoolI32 | NativeValRepr::TagI32 | NativeValRepr::UInt32 | NativeValRepr::HeapPtrI32,
      NativeValRepr::Int64S | NativeValRepr::UInt64,
    ) => {
      ctx.emit(Instruction::I64ExtendI32U);
    }
    (NativeValRepr::TagI32 | NativeValRepr::UInt32 | NativeValRepr::HeapPtrI32, NativeValRepr::BoolI32) => {
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::I32Ne);
    }
    _ => {
      emit_box_repr_to_f64(ctx, src);
      emit_unbox_f64_to_repr(ctx, dst);
    }
  }
}

/// A funcref table entry for the `&` rest-parameter function `target` with
/// `fixed` leading parameters, typed `(f64 x arity) -> f64` like every
/// function-value call with `arity` arguments: it packs the arguments after
/// the fixed ones into a new list and calls `target` (#1972).
#[cfg(test)]
fn build_rest_adapter(ctx: &mut WasmGenCtx, target: u32, fixed: u32, arity: u32) -> CompiledFn {
  ctx.instructions.clear();
  ctx.extra_locals.clear();
  ctx.next_local = arity;
  let rest = emit_list_from_locals(ctx, &(fixed..arity).collect::<Vec<_>>());
  for param in 0..fixed {
    ctx.emit(Instruction::LocalGet(param));
  }
  ctx.emit(Instruction::LocalGet(rest));
  ctx.emit(Instruction::Call(target));
  CompiledFn {
    export_name: None,
    params: vec![ValType::F64; arity as usize],
    results: vec![ValType::F64],
    locals: std::mem::take(&mut ctx.extra_locals),
    instructions: std::mem::take(&mut ctx.instructions),
  }
}

fn compile_fn(
  source_ns: &str,
  export_name: &str,
  args: &CalcitFnArgs,
  body: &[Calcit],
  signature: Option<&CalcitFnTypeAnnotation>,
  env: &WasmCompileEnv,
) -> Result<CompiledFn, String> {
  let param_names = fn_param_names(args);

  let arity = param_names.len();
  let mut ctx = WasmGenCtx::new(arity as u32, env.clone());
  source_ns.clone_into(&mut ctx.source_ns);
  ctx.source_fn_index = env
    .fn_index
    .get(&format!("{source_ns}/{export_name}"))
    .or_else(|| env.fn_index.get(export_name))
    .copied();
  let native_sig = ctx.source_fn_index.and_then(|idx| env.fn_signatures.get(&idx)).cloned();
  let param_reprs: Vec<NativeValRepr> = native_sig
    .as_ref()
    .map_or_else(|| vec![NativeValRepr::F64; arity], |s| s.params.clone());
  let ret_repr = native_sig.as_ref().map_or(NativeValRepr::F64, |s| s.result);

  // Register parameter locals
  for (i, pname) in param_names.iter().enumerate() {
    ctx.locals.insert(pname.clone(), i as u32);
    if param_reprs[i] != NativeValRepr::F64 {
      ctx.local_reprs.insert(i as u32, param_reprs[i]);
    }
    if let Some(signature) = signature {
      let annotation = signature.arg_types.get(i).cloned().or_else(|| {
        signature
          .rest_type
          .as_ref()
          .map(|element| Arc::new(CalcitTypeAnnotation::List(element.clone())))
      });
      if let Some(annotation) = annotation {
        ctx.local_types.insert(pname.clone(), WasmTypeEvidence::contract(annotation));
      }
    }
    ctx.arg_indices.push(i as u32);
  }
  if let CalcitFnArgs::MarkedArgs(labels) = args {
    ctx.recur_shape = arg_labels_shape(labels).filter(|&(required, rest)| rest || required != arity);
  }

  if source_ns == "calcit.core" && export_name == "keys-non-nil" && arity == 1 {
    let arg0 = Calcit::Symbol {
      sym: param_names[0].clone().into(),
      info: Arc::new(crate::wasm::host::calcit::CalcitSymbolInfo {
        at_ns: source_ns.into(),
        at_def: export_name.into(),
      }),
      location: None,
    };
    emit_map_keys_non_nil(&mut ctx, &[arg0])?;
    return Ok(CompiledFn {
      export_name: None,
      params: vec![ValType::F64; arity],
      results: vec![ValType::F64],
      locals: std::mem::take(&mut ctx.extra_locals),
      instructions: std::mem::take(&mut ctx.instructions),
    });
  }
  reject_non_tail_recur(body, &format!("{source_ns}/{export_name}"))?;
  // Check if body uses recur
  ctx.uses_recur = body.iter().any(check_uses_recur);

  if ctx.uses_recur {
    ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Result(ret_repr.val_type())));
    emit_body_as_repr(&mut ctx, body, ret_repr)?;
    ctx.emit(Instruction::End); // end loop
  } else {
    emit_body_as_repr(&mut ctx, body, ret_repr)?;
  }

  Ok(CompiledFn {
    export_name: Some(export_name.to_owned()),
    params: param_reprs.into_iter().map(NativeValRepr::val_type).collect(),
    results: vec![ret_repr.val_type()],
    locals: ctx.extra_locals,
    instructions: ctx.instructions,
  })
}

/// `recur` lowers to a branch back to the loop start, which matches its semantics only in
/// tail position; anywhere else it would silently restart the loop, so it is rejected with
/// the same tail-position rule preprocessing uses.
fn reject_non_tail_recur(body: &[Calcit], owner: &str) -> Result<(), String> {
  match crate::wasm::host::runner::preprocess::non_tail_recur_forms(body).first() {
    Some(form) => Err(format!(
      "recur must be called directly in tail position of the function body in {owner}, got: {}",
      form.lisp_str()
    )),
    None => Ok(()),
  }
}

/// Scope a loop's argument slots and proof independently of enclosing loops.
fn emit_recur_loop(
  ctx: &mut WasmGenCtx,
  parameter_locals: Vec<u32>,
  shape: Option<(usize, bool)>,
  evidence: Option<Vec<WasmTypeEvidence>>,
  body: &[Calcit],
) -> Result<(), String> {
  let old_arg_indices = std::mem::replace(&mut ctx.arg_indices, parameter_locals);
  let old_recur_shape = std::mem::replace(&mut ctx.recur_shape, shape);
  let old_evidence = std::mem::replace(&mut ctx.recur_evidence, evidence);
  let old_block_depth = std::mem::replace(&mut ctx.block_depth, 0);
  let pre_loop_resets = ctx.reset_initialized_atoms.clone();
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Result(ValType::F64)));
  let result = emit_body(ctx, body);
  ctx.emit(Instruction::End);
  ctx.reset_initialized_atoms = pre_loop_resets;
  ctx.block_depth = old_block_depth;
  ctx.arg_indices = old_arg_indices;
  ctx.recur_shape = old_recur_shape;
  ctx.recur_evidence = old_evidence;
  result
}

/// Check loop-carried actual evidence before retaining it for another iteration.
/// Returns the selector assignment of each argument whose loop parameter
/// carries one of several actual impl identities.
fn validate_recur_evidence(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<Vec<Option<(u32, usize)>>, String> {
  let Some(expected) = ctx.recur_evidence.clone() else {
    return Ok(vec![]);
  };
  if args.len() != expected.len() || args.iter().any(|arg| matches!(arg, Calcit::Syntax(CalcitSyntax::ArgSpread, _))) {
    return Err("E_WASM_CALL_SPECIALIZATION: specialized recur requires a fixed proven argument list".into());
  }
  let mut assignments = Vec::with_capacity(args.len());
  let mut changed = None;
  let mut changed_origin = None;
  for (index, (arg, expected)) in args.iter().zip(&expected).enumerate() {
    let actual = infer_wasm_value_evidence(ctx, arg).ok_or_else(|| {
      format!(
        "E_WASM_CALL_SPECIALIZATION: recur argument {} has no loop-carried type evidence",
        index + 1
      )
    })?;
    let proven = actual.annotation.is_proven_for(&expected.annotation);
    let nominal_identity = actual.exact_impls && is_nominal_value_annotation(&actual.annotation);
    let same_scalar_origin = match &expected.scalar_origin {
      None => true,
      Some(expected_origin) => {
        let actual_origin = actual.scalar_origin.as_ref().unwrap_or(&actual.annotation);
        let exp_ty = expand_wasm_value_type(expected_origin);
        let act_ty = expand_wasm_value_type(actual_origin);
        act_ty == exp_ty || act_ty.is_proven_for(&exp_ty)
      }
    };
    if let Some(selector) = ctx.recur_selectors.get(index).cloned().flatten()
      && proven
      && nominal_identity
    {
      match selector.index_of(&actual) {
        Some(position) => assignments.push(Some((selector.local, position))),
        None => {
          ctx.recur_missing.push((index, actual));
          changed.get_or_insert(index);
          assignments.push(None);
        }
      }
      continue;
    }
    let both_coherent_same_nominal = is_statically_coherent_nominal(ctx, &expected.annotation)
      && is_statically_coherent_nominal(ctx, &actual.annotation)
      && nominal_type_def_ref(&expected.annotation) == nominal_type_def_ref(&actual.annotation);
    let same_impls = !expected.exact_impls
      || both_coherent_same_nominal
      || (actual.exact_impls
        && match (expected.annotation.resolve_to_enum(), actual.annotation.resolve_to_enum()) {
          (Some(expected), Some(actual)) => expected.impls == actual.impls,
          _ => match (expected.annotation.resolve_to_struct(), actual.annotation.resolve_to_struct()) {
            (Some(expected), Some(actual)) => expected.impls == actual.impls,
            _ => false,
          },
        });
    if proven && !same_impls && expected.exact_impls && nominal_identity {
      // Another actual identity reaches this back edge: retry the loop with
      // a selector over every identity instead of guessing the first one.
      ctx.recur_missing.push((index, actual));
      changed.get_or_insert(index);
      if !same_scalar_origin {
        ctx.recur_drop_scalar_origin.push(index);
        changed_origin.get_or_insert(index);
      }
      assignments.push(None);
      continue;
    }
    if !proven || !same_impls {
      return Err(format!(
        "E_WASM_CALL_SPECIALIZATION: recur argument {} does not preserve the loop-carried type and implementation identity",
        index + 1
      ));
    }
    if !same_scalar_origin {
      if ctx.in_selected_recur_loop {
        ctx.recur_drop_scalar_origin.push(index);
        changed_origin.get_or_insert(index);
        assignments.push(None);
        continue;
      }
      return Err(format!(
        "E_WASM_CALL_SPECIALIZATION: recur argument {} does not preserve the loop-carried scalar origin",
        index + 1
      ));
    }
    assignments.push(None);
  }
  if let Some(index) = changed {
    return Err(format!(
      "{RECUR_IMPL_RETRY}: recur argument {} changes the loop-carried implementation identity",
      index + 1
    ));
  }
  if let Some(index) = changed_origin {
    return Err(format!(
      "{RECUR_SCALAR_ORIGIN_RETRY}: recur argument {} changes the loop-carried scalar origin",
      index + 1
    ));
  }
  Ok(assignments)
}

fn nominal_type_def_ref(ty: &CalcitTypeAnnotation) -> Option<String> {
  let expanded = expand_wasm_value_type(&Arc::new(ty.clone()));
  match expanded.as_ref() {
    CalcitTypeAnnotation::StructDef(def) => def.definition_ref.as_deref().map(str::to_owned),
    CalcitTypeAnnotation::EnumDef(def) => def.definition_ref().map(|r| r.as_ref().to_owned()),
    CalcitTypeAnnotation::Struct(def, _) | CalcitTypeAnnotation::StructValue(def) => def.definition_ref.as_deref().map(str::to_owned),
    CalcitTypeAnnotation::Enum(def, _) | CalcitTypeAnnotation::EnumValue(def) => def.definition_ref().map(|r| r.as_ref().to_owned()),
    CalcitTypeAnnotation::TypeRef(_, _) => {
      if let Some((def, path)) = expanded.resolve_to_struct_with_ref() {
        def
          .definition_ref
          .as_deref()
          .map(str::to_owned)
          .or_else(|| path.map(|(ns, def)| format!("{ns}/{def}")))
      } else if let Some((def, path)) = expanded.resolve_to_enum_with_ref() {
        def
          .definition_ref()
          .map(|r| r.as_ref().to_owned())
          .or_else(|| path.map(|(ns, def)| format!("{ns}/{def}")))
      } else {
        None
      }
    }
    _ => None,
  }
}

fn is_statically_coherent_nominal(ctx: &WasmGenCtx, ty: &CalcitTypeAnnotation) -> bool {
  nominal_type_def_ref(ty).is_some_and(|def_ref| !ctx.non_coherent_nominal_defs.contains(&def_ref))
}

fn has_effective_exact_impls(ctx: &WasmGenCtx, ev: &WasmTypeEvidence) -> bool {
  ev.exact_impls || is_statically_coherent_nominal(ctx, &ev.annotation)
}

fn is_nominal_value_annotation(annotation: &CalcitTypeAnnotation) -> bool {
  annotation.resolve_to_enum().is_some() || annotation.resolve_to_struct().is_some()
}

/// Emit a specialized loop, widening each parameter's actual impl identities
/// to every identity its back edges carry, selected at runtime.
fn emit_selected_recur_loop(
  ctx: &mut WasmGenCtx,
  params: &[String],
  parameter_locals: Vec<u32>,
  argument_types: Vec<WasmTypeEvidence>,
  body: &[Calcit],
) -> Result<(), String> {
  let mut effective_types = argument_types;
  let mut alternatives: Vec<Vec<WasmTypeEvidence>> = effective_types.iter().map(|evidence| vec![evidence.clone()]).collect();
  let start = ctx.instructions.len();
  loop {
    ctx.instructions.truncate(start);
    let selectors: Vec<Option<ImplSelector>> = alternatives
      .iter()
      .zip(&effective_types)
      .map(|(identities, effective)| {
        (!is_statically_coherent_nominal(ctx, &effective.annotation) && identities.len() > 1).then(|| ImplSelector {
          local: ctx.alloc_local_typed(ValType::I32),
          alternatives: identities.clone(),
        })
      })
      .collect();
    let mut old_types = vec![];
    for ((selector, local), (param, initial)) in selectors.iter().zip(&parameter_locals).zip(params.iter().zip(&effective_types)) {
      if let Some(selector) = selector {
        // The argument value enters the loop as the first identity.
        ctx.emit(Instruction::I32Const(0));
        ctx.emit(Instruction::LocalSet(selector.local));
        ctx.impl_selectors.insert(*local, selector.clone());
        old_types.push((
          param.clone(),
          ctx.local_types.insert(
            param.clone(),
            WasmTypeEvidence {
              scalar_origin: initial.scalar_origin.clone(),
              ..WasmTypeEvidence::contract(initial.annotation.clone())
            },
          ),
        ));
      } else {
        old_types.push((param.clone(), ctx.local_types.insert(param.clone(), initial.clone())));
      }
    }
    let old_selectors = std::mem::replace(&mut ctx.recur_selectors, selectors.clone());
    let old_missing = std::mem::take(&mut ctx.recur_missing);
    let old_drop_origin = std::mem::take(&mut ctx.recur_drop_scalar_origin);
    let old_in_selected = std::mem::replace(&mut ctx.in_selected_recur_loop, true);
    let result = emit_recur_loop(ctx, parameter_locals.clone(), None, Some(effective_types.clone()), body);
    ctx.in_selected_recur_loop = old_in_selected;
    let missing = std::mem::replace(&mut ctx.recur_missing, old_missing);
    let dropped_origins = std::mem::replace(&mut ctx.recur_drop_scalar_origin, old_drop_origin);
    ctx.recur_selectors = old_selectors;
    for (selector, local) in selectors.iter().zip(&parameter_locals) {
      if selector.is_some() {
        ctx.impl_selectors.remove(local);
      }
    }
    for (param, old) in old_types {
      match old {
        Some(evidence) => ctx.local_types.insert(param, evidence),
        None => ctx.local_types.remove(&param),
      };
    }
    match result {
      Err(error) if error.starts_with(RECUR_IMPL_RETRY) && !missing.is_empty() => {
        for (index, evidence) in missing {
          if !alternatives[index].contains(&evidence) {
            alternatives[index].push(evidence);
          }
          if alternatives[index].len() > MAX_IMPL_ALTERNATIVES {
            return Err(format!(
              "E_WASM_CALL_SPECIALIZATION: recur argument {} carries more than {MAX_IMPL_ALTERNATIVES} {IMPL_LIMIT}",
              index + 1
            ));
          }
        }
        for index in dropped_origins {
          effective_types[index].scalar_origin = None;
          for alt in &mut alternatives[index] {
            alt.scalar_origin = None;
          }
        }
      }
      Err(error) if error.starts_with(RECUR_SCALAR_ORIGIN_RETRY) && !dropped_origins.is_empty() => {
        for index in dropped_origins {
          effective_types[index].scalar_origin = None;
          for alt in &mut alternatives[index] {
            alt.scalar_origin = None;
          }
        }
      }
      Err(error) if error.starts_with(RECUR_IMPL_RETRY) => {
        return Err(error.replacen(RECUR_IMPL_RETRY, "E_WASM_CALL_SPECIALIZATION", 1));
      }
      Err(error) if error.starts_with(RECUR_SCALAR_ORIGIN_RETRY) => {
        return Err(error.replacen(RECUR_SCALAR_ORIGIN_RETRY, "E_WASM_CALL_SPECIALIZATION", 1));
      }
      other => return other,
    }
  }
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

fn dyn_box_operand_needs_specialization(
  expr: &Calcit,
  specialized_params: Option<&HashSet<String>>,
  local_sensitivity: &HashMap<String, bool>,
) -> bool {
  match expr {
    Calcit::Number(_) | Calcit::Bool(_) | Calcit::Str(_) | Calcit::Tag(_) | Calcit::Nil | Calcit::Unit => false,
    Calcit::Import(_) | Calcit::StructDef(_) | Calcit::EnumDef(_) | Calcit::Trait(_) | Calcit::Proc(_) => false,
    Calcit::Local(local) => {
      if let Some(&sensitive) = local_sensitivity.get(local.sym.as_ref()) {
        return sensitive;
      }
      if let Some(params) = specialized_params {
        if params.contains(local.sym.as_ref()) {
          return true;
        }
        if !local.type_info.contains_type_var()
          && !nil_type_evidence_is_open(&local.type_info)
          && !is_nominal_value_annotation(&local.type_info)
        {
          return false;
        }
      }
      true
    }
    Calcit::Symbol { sym, .. } => {
      if let Some(&sensitive) = local_sensitivity.get(sym.as_ref()) {
        return sensitive;
      }
      if let Some(params) = specialized_params
        && params.contains(sym.as_ref())
      {
        return true;
      }
      true
    }
    Calcit::List(items) if !items.is_empty() => match &items[0] {
      Calcit::Syntax(CalcitSyntax::CoreLet, _) => {
        let mut scoped = local_sensitivity.clone();
        if let Some(Calcit::List(binding)) = items.get(1) {
          let record_pair = |pair: &Calcit, scoped: &mut HashMap<String, bool>| {
            if let Calcit::List(kv) = pair
              && kv.len() == 2
              && let Some(name) = match &kv[0] {
                Calcit::Local(l) => Some(l.sym.as_ref().to_owned()),
                Calcit::Symbol { sym, .. } => Some(sym.as_ref().to_owned()),
                _ => None,
              }
            {
              let rhs_sensitive = dyn_box_operand_needs_specialization(&kv[1], specialized_params, scoped);
              scoped.insert(name, rhs_sensitive);
            }
          };
          if binding.len() == 2 && !matches!(binding[0], Calcit::List(_)) {
            record_pair(items.get(1).expect("checked"), &mut scoped);
          } else {
            for pair in binding.iter() {
              record_pair(pair, &mut scoped);
            }
          }
        }
        items
          .drop_left()
          .drop_left()
          .iter()
          .any(|item| dyn_box_operand_needs_specialization(item, specialized_params, &scoped))
      }
      Calcit::Syntax(CalcitSyntax::Quote, _) => false,
      Calcit::Syntax(CalcitSyntax::Defn, _) => true,
      Calcit::Syntax(_, _) | Calcit::Proc(_) | Calcit::Method(_, _) | Calcit::Import(_) | Calcit::StructDef(_) | Calcit::EnumDef(_) => {
        items
          .drop_left()
          .iter()
          .any(|arg| dyn_box_operand_needs_specialization(arg, specialized_params, local_sensitivity))
      }
      _ => items
        .iter()
        .any(|item| dyn_box_operand_needs_specialization(item, specialized_params, local_sensitivity)),
    },
    _ => false,
  }
}

fn expr_uses_trait_call_for_def(expr: &Calcit, arg_types: Option<&[Arc<CalcitTypeAnnotation>]>, args: Option<&CalcitFnArgs>) -> bool {
  let specialized_params = match (args, arg_types) {
    (Some(fn_args), Some(types)) => {
      let names = fn_param_names(fn_args);
      let set = names
        .into_iter()
        .zip(types.iter())
        .filter(|(_, ty)| ty.contains_type_var() || nil_type_evidence_is_open(ty) || is_nominal_value_annotation(ty))
        .map(|(name, _)| name)
        .collect::<HashSet<_>>();
      Some(set)
    }
    _ => None,
  };
  let mut scoped_sensitivity = HashMap::new();
  expr_uses_trait_call_inner(expr, specialized_params.as_ref(), &mut scoped_sensitivity)
}

fn expr_uses_trait_call_inner(
  expr: &Calcit,
  specialized_params: Option<&HashSet<String>>,
  local_sensitivity: &mut HashMap<String, bool>,
) -> bool {
  match expr {
    Calcit::List(items) => match items.first() {
      Some(Calcit::Proc(CalcitProc::NativeTraitCall)) => true,
      Some(Calcit::Syntax(CalcitSyntax::DynBox, _)) => {
        items
          .get(1)
          .is_some_and(|operand| dyn_box_operand_needs_specialization(operand, specialized_params, local_sensitivity))
          || items
            .drop_left()
            .iter()
            .any(|item| expr_uses_trait_call_inner(item, specialized_params, local_sensitivity))
      }
      Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) => {
        let mut scoped = local_sensitivity.clone();
        if let Some(Calcit::List(binding)) = items.get(1) {
          let mut visit_pair = |pair: &Calcit| -> bool {
            if let Calcit::List(kv) = pair
              && kv.len() == 2
            {
              if expr_uses_trait_call_inner(&kv[1], specialized_params, &mut scoped) {
                return true;
              }
              if let Some(name) = match &kv[0] {
                Calcit::Local(l) => Some(l.sym.as_ref().to_owned()),
                Calcit::Symbol { sym, .. } => Some(sym.as_ref().to_owned()),
                _ => None,
              } {
                let rhs_sensitive = dyn_box_operand_needs_specialization(&kv[1], specialized_params, &scoped);
                scoped.insert(name, rhs_sensitive);
              }
            }
            false
          };
          if binding.len() == 2 && !matches!(binding[0], Calcit::List(_)) {
            if visit_pair(items.get(1).expect("checked")) {
              return true;
            }
          } else {
            for pair in binding.iter() {
              if visit_pair(pair) {
                return true;
              }
            }
          }
        }
        items
          .drop_left()
          .drop_left()
          .iter()
          .any(|item| expr_uses_trait_call_inner(item, specialized_params, &mut scoped))
      }
      _ => items
        .iter()
        .any(|item| expr_uses_trait_call_inner(item, specialized_params, local_sensitivity)),
    },
    _ => false,
  }
}

fn expr_uses_method_call(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(items) => {
      matches!(items.first(), Some(Calcit::Method(_, MethodKind::Invoke(_)))) || items.iter().any(expr_uses_method_call)
    }
    _ => false,
  }
}

/// Emit instructions for a sequence of expressions (last is the return value).
fn emit_body(ctx: &mut WasmGenCtx, exprs: &[Calcit]) -> Result<(), String> {
  emit_body_as_repr(ctx, exprs, NativeValRepr::F64)
}

fn emit_unit_const_repr(ctx: &mut WasmGenCtx, dst_repr: NativeValRepr) {
  match dst_repr {
    NativeValRepr::BoolI32
    | NativeValRepr::TagI32
    | NativeValRepr::UnitI32
    | NativeValRepr::Int32S
    | NativeValRepr::UInt32
    | NativeValRepr::HeapPtrI32 => ctx.emit(Instruction::I32Const(0)),
    NativeValRepr::Int64S | NativeValRepr::UInt64 => ctx.emit(Instruction::I64Const(0)),
    NativeValRepr::F64 => ctx.emit(f64_const(0.0)),
  }
}

fn emit_body_as_repr(ctx: &mut WasmGenCtx, exprs: &[Calcit], dst_repr: NativeValRepr) -> Result<(), String> {
  // Strict preprocessing may append a function contract after the executable
  // body. `hint-fn` is compile-time metadata, not the closure's return value.
  let executable = exprs
    .iter()
    .filter(|expr| !crate::wasm::host::builtins::syntax::is_function_metadata_hint(expr))
    .collect::<Vec<_>>();
  if executable.is_empty() {
    emit_unit_const_repr(ctx, dst_repr);
    return Ok(());
  }
  for (i, expr) in executable.iter().enumerate() {
    let is_last = i == executable.len() - 1;
    // A quoted name before the result, such as the return label in
    // `fn (x) 'Number x`, has no effect and no runtime quote value to build.
    if !is_last && is_quoted_leaf(expr) {
      continue;
    }
    if !is_last && is_inline_closure_assert_type(ctx, expr) {
      continue;
    }
    if is_last {
      emit_expr_as_repr(ctx, expr, dst_repr)?;
    } else {
      emit_expr(ctx, expr)?;
      ctx.emit(Instruction::Drop);
    }
  }
  Ok(())
}

fn is_quoted_leaf(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(items) => {
      items.len() == 2
        && matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Quote, _)))
        && matches!(items.get(1), Some(Calcit::Symbol { .. }))
    }
    _ => false,
  }
}

fn is_inline_closure_assert_type(ctx: &WasmGenCtx, expr: &Calcit) -> bool {
  match expr {
    // Preprocessing lowers `assert-type <local> <type>` to `<local>`, which is
    // dropped in non-tail statement position without escaping.
    Calcit::Local(local) => ctx.lambda_locals.contains_key(local.sym.as_ref()),
    Calcit::Symbol { sym, .. } => ctx.lambda_locals.contains_key(sym.as_ref()),
    Calcit::List(items) if items.len() == 3 && matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::AssertType, _))) => {
      match items.get(1) {
        Some(Calcit::Local(local)) => ctx.lambda_locals.contains_key(local.sym.as_ref()),
        Some(Calcit::Symbol { sym, .. }) => ctx.lambda_locals.contains_key(sym.as_ref()),
        _ => false,
      }
    }
    _ => false,
  }
}

/// Emit an inline IIFE with TCO: `((defn name (params...) body...) init_args...)`.
///
/// - Allocates new f64 locals for each param, initializes from `init_args`.
/// - If the body uses `recur`, wraps with `loop...end` and sets up `ctx.arg_indices`
///   so that `recur` correctly jumps back to the loop start.
/// - Outer params of the same name are shadowed and restored after.
///
/// `shape` is the `(required, rest)` shape of a parameter list with `?` / `&`
/// markers; `params` then excludes the markers, and both the initial call and
/// `recur` bind arguments like native.
/// Fixed-arity trailing Option metadata supplies nominal None for omission.
fn emit_inline_iife(
  ctx: &mut WasmGenCtx,
  params: &[String],
  body: &[Calcit],
  init_args: &[Calcit],
  shape: Option<(usize, bool)>,
  option_types: &[Arc<CalcitTypeAnnotation>],
) -> Result<(), String> {
  // Evaluate all init args and store in fresh locals (before modifying ctx.locals).
  let mut param_locals: Vec<u32> = Vec::new();
  let mut param_types = init_args.iter().map(|arg| infer_wasm_value_evidence(ctx, arg)).collect::<Vec<_>>();
  if let Some((required, rest)) = shape {
    let positional = params.len() - usize::from(rest);
    if init_args.len() < required || (!rest && init_args.len() > positional) {
      return Err(format!(
        "inline function expects {} argument(s), got {}",
        if rest {
          format!("at least {required}")
        } else {
          format!("{required} to {positional}")
        },
        init_args.len()
      ));
    }
    for arg in init_args {
      let tmp = ctx.alloc_local();
      emit_expr(ctx, arg)?;
      ctx.emit(Instruction::LocalSet(tmp));
      param_locals.push(tmp);
    }
    let trailing = if param_locals.len() > positional {
      param_locals.split_off(positional)
    } else {
      vec![]
    };
    param_types.truncate(positional);
    while param_locals.len() < positional {
      let nil = ctx.alloc_local();
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::LocalSet(nil));
      param_locals.push(nil);
      // `recur` may rebind it to a non-nil value.
      param_types.push(None);
    }
    if rest {
      param_locals.push(emit_list_from_locals(ctx, &trailing));
      param_types.push(None);
    }
  } else {
    for (i, _param_name) in params.iter().enumerate() {
      let tmp = ctx.alloc_local();
      if i < init_args.len() {
        emit_expr(ctx, &init_args[i])?;
      } else if i >= params.len() - option_types.len() {
        emit_expr(ctx, &nominal_option_none_expression()?)?;
        param_types.push(infer_wasm_value_evidence(ctx, &nominal_option_none_expression()?));
      } else {
        ctx.emit(f64_const(0.0)); // nil for missing args
        param_types.push(None);
      }
      ctx.emit(Instruction::LocalSet(tmp));
      param_locals.push(tmp);
    }
  }

  // Shadow outer locals with IIFE params (save old bindings for restoration).
  let mut saved: Vec<(String, Option<u32>, Option<WasmTypeEvidence>)> = Vec::new();
  let uses_recur = body.iter().any(check_uses_recur);
  // Fixed parameters with nominal impl identities run as a selected loop, so
  // each back edge carries its actual identity to later iterations.
  let selected_types = (uses_recur && shape.is_none())
    .then(|| param_types.iter().cloned().collect::<Option<Vec<_>>>())
    .flatten()
    .filter(|types| {
      types.len() == params.len()
        && types.iter().any(|evidence| {
          (evidence.exact_impls
            && is_nominal_value_annotation(&evidence.annotation)
            && !is_statically_coherent_nominal(ctx, &evidence.annotation))
            || evidence.scalar_origin.is_some()
        })
    });
  if uses_recur && selected_types.is_none() {
    // The initial call cannot establish an impl identity or scalar origin for later loop values.
    for evidence in param_types.iter_mut().flatten() {
      evidence.exact_impls = false;
      evidence.scalar_origin = None;
    }
  }
  for (i, name) in params.iter().enumerate() {
    saved.push((name.clone(), ctx.locals.get(name).copied(), ctx.local_types.get(name).cloned()));
    ctx.locals.insert(name.clone(), param_locals[i]);
    if let Some(Some(annotation)) = param_types.get(i) {
      ctx.local_types.insert(name.clone(), annotation.clone());
    } else {
      ctx.local_types.remove(name);
    }
  }

  // Emit body — with a loop wrapper if TCO recur is needed.
  reject_non_tail_recur(body, "inline function")?;
  let start = ctx.instructions.len();
  let selected = match selected_types {
    Some(types) => match emit_selected_recur_loop(ctx, params, param_locals.clone(), types, body) {
      Ok(()) => true,
      // Back edges without nominal evidence keep the unselected loop, which
      // dispatches by contract as before.
      // Past the identity limit the selected loop is required, so report it.
      Err(error) if error.starts_with("E_WASM_CALL_SPECIALIZATION:") && !error.ends_with(IMPL_LIMIT) => {
        ctx.instructions.truncate(start);
        for (name, evidence) in params.iter().zip(&param_types) {
          if let Some(evidence) = evidence {
            ctx.local_types.insert(
              name.clone(),
              WasmTypeEvidence {
                exact_impls: false,
                scalar_origin: None,
                ..evidence.clone()
              },
            );
          }
        }
        false
      }
      Err(error) => return Err(error),
    },
    None => false,
  };
  if selected {
    // Already emitted as a selected loop.
  } else if uses_recur {
    emit_recur_loop(ctx, param_locals, shape, None, body)?;
  } else {
    emit_body(ctx, body)?;
  }

  // Restore shadowed locals.
  for (name, old, old_type) in saved {
    match old {
      Some(idx) => {
        ctx.locals.insert(name.clone(), idx);
      }
      None => {
        ctx.locals.remove(&name);
      }
    }
    match old_type {
      Some(annotation) => {
        ctx.local_types.insert(name, annotation);
      }
      None => {
        ctx.local_types.remove(&name);
      }
    }
  }
  Ok(())
}

/// Loads a global `defref` slot, rejecting uninitialized closure initializers and recording any
/// static function target for reachability/specialization validation.
fn emit_atom_global_load(ctx: &mut WasmGenCtx, qualified: &str, global_idx: u32) -> Result<(), String> {
  let reset_replaced = ctx.reset_initialized_atoms.contains(qualified);
  if !reset_replaced {
    let err_opt = ctx.closures.borrow().atom_uninitialized_closures.get(qualified).cloned();
    if let Some(err) = err_opt {
      return Err(err);
    }
    let fn_target = ctx.closures.borrow().atom_initial_fn_targets.get(qualified).cloned();
    if let Some(target_fn) = fn_target {
      let short_name = target_fn.rsplit_once('/').map_or(target_fn.as_str(), |(_, n)| n);
      closure::validate_static_closure_target(ctx, &target_fn, short_name)?;
      ctx.record_function_value(&target_fn);
    }
  }
  ctx.emit(Instruction::GlobalGet(global_idx));
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
      let id = ctx.struct_layouts.id(s)?;
      ctx.emit(f64_const(id as f64));
    }
    Calcit::EnumDef(e) => {
      let key = enum_identity_tag(e);
      let id = *ctx
        .tag_index
        .get(&key)
        .ok_or_else(|| format!("unknown enum definition tag in WASM: {}", e.name()))?;
      ctx.emit(f64_const(id as f64));
    }
    Calcit::Local(local) => {
      let name = &*local.sym;
      if let Some(closure) = ctx.lambda_locals.get(name).cloned() {
        closure::emit_inline_closure_value(ctx, &closure, Some(expr))?;
      } else if let Some(&idx) = ctx.locals.get(name) {
        ctx.emit_local_as_f64(idx);
      } else if let Some((ref self_name, _, _)) = ctx.self_closure
        && self_name == name
      {
        ctx.emit(Instruction::LocalGet(0));
        ctx.emit(Instruction::F64ConvertI32U);
      } else {
        return Err(format!("undefined local variable: {name}"));
      }
    }
    Calcit::Symbol { sym, info, .. } => {
      let name = sym.as_ref();
      let qualified = format!("{}/{}", info.at_ns, name);
      if let Some(closure) = ctx.lambda_locals.get(name).cloned() {
        closure::emit_inline_closure_value(ctx, &closure, Some(expr))?;
      } else if let Some(&idx) = ctx.locals.get(name) {
        ctx.emit_local_as_f64(idx);
      } else if let Some((ref self_name, _, _)) = ctx.self_closure
        && self_name == name
      {
        ctx.emit(Instruction::LocalGet(0));
        ctx.emit(Instruction::F64ConvertI32U);
      } else if name == "do" && info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS && !ctx.fn_index.contains_key(&qualified) {
        ctx.emit(f64_const(0.0));
      } else if let Some(&global_idx) = ctx.atom_globals.get(&qualified) {
        emit_atom_global_load(ctx, &qualified, global_idx)?;
      } else if ctx.fn_index.contains_key(&qualified) {
        closure::emit_static_closure_value(ctx, &qualified, name)?;
      } else if let Some(value_expr) = ctx.value_imports.get(&qualified).cloned() {
        emit_expr(ctx, &value_expr).map_err(|reason| format!("{reason} (symbol value `{qualified}`)"))?;
      } else if ctx.fn_index.contains_key(name) {
        closure::emit_static_closure_value(ctx, &qualified, name)?;
      } else {
        return Err(format!("unsupported WASM expression: {expr}"));
      }
    }
    Calcit::List(xs) if !xs.is_empty() => {
      emit_call_expr(ctx, xs)?;
    }
    // `do` appears as a bare (non-call) expression when used as a body sequencer in defn.
    // It's a no-op — just emit nil so it can be dropped by emit_body.
    Calcit::Import(import)
      if import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS
        && import.def.as_ref() == "do"
        && !ctx.fn_index.contains_key(&format!("{}/{}", import.ns, import.def)) =>
    {
      ctx.emit(f64_const(0.0));
    }
    Calcit::Import(import) => {
      // Check if this is a reference to a known WASM global (defref)
      let qualified = format!("{}/{}", import.ns, import.def);
      if let Some(&global_idx) = ctx.atom_globals.get(&qualified) {
        emit_atom_global_load(ctx, &qualified, global_idx)?;
      } else if import.def.as_ref() == "{}" {
        // `{}` used as a bare expression — evaluates to an empty map.
        emit_map_new(ctx, &[])?;
      } else if import.def.as_ref() == "[]" {
        // `[]` used as a bare expression — evaluates to an empty list.
        emit_list_new(ctx, &[])?;
      } else if let Ok(struct_def) = resolve_struct_ref(expr) {
        let id = ctx.struct_layouts.id(&struct_def)?;
        ctx.emit(f64_const(id as f64));
      } else if let Ok(enum_def) = resolve_enum_ref(expr) {
        let key = enum_identity_tag(&enum_def);
        let id = *ctx
          .tag_index
          .get(&key)
          .ok_or_else(|| format!("unknown enum definition tag in WASM: {}", enum_def.name()))?;
        ctx.emit(f64_const(id as f64));
      } else if ctx.fn_index.contains_key(&qualified) {
        closure::emit_static_closure_value(ctx, &qualified, import.def.as_ref())?;
      } else if let Some(value_expr) = ctx.value_imports.get(&qualified).cloned() {
        // Imported top-level def value (e.g. a string constant). Inline its expression.
        emit_expr(ctx, &value_expr).map_err(|reason| format!("{reason} (imported value `{qualified}`)"))?;
      } else if ctx.fn_index.contains_key(import.def.as_ref()) {
        closure::emit_static_closure_value(ctx, &qualified, import.def.as_ref())?;
      } else {
        return Err(format!("unsupported value import in WASM: {qualified}"));
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
    Calcit::Fn { info, .. } => {
      if let Some(def_ref) = &info.def_ref {
        let qualified = format!("{}/{}", def_ref.def_ns, def_ref.def_name);
        closure::emit_static_closure_value(ctx, &qualified, def_ref.def_name.as_ref())?;
      } else {
        closure::emit_fn_value(ctx, info, expr)?;
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

/// Evaluate an expression directly into `dst_repr`, avoiding redundant f64 boxing
/// on locals, literals, calls with native signatures, and unboxed struct/enum ops.
fn emit_expr_as_repr(ctx: &mut WasmGenCtx, expr: &Calcit, dst_repr: NativeValRepr) -> Result<(), String> {
  if dst_repr == NativeValRepr::F64 {
    return emit_expr(ctx, expr);
  }
  match expr {
    Calcit::Bool(b) if dst_repr == NativeValRepr::BoolI32 => {
      ctx.emit(Instruction::I32Const(i32::from(*b)));
      return Ok(());
    }
    Calcit::Nil | Calcit::Unit if matches!(dst_repr, NativeValRepr::UnitI32 | NativeValRepr::BoolI32) => {
      ctx.emit(Instruction::I32Const(0));
      return Ok(());
    }
    Calcit::Number(n) if n.is_finite() && n.fract() == 0.0 => match dst_repr {
      NativeValRepr::Int32S if (-2_147_483_648.0..=2_147_483_647.0).contains(n) => {
        ctx.emit(Instruction::I32Const(*n as i32));
        return Ok(());
      }
      NativeValRepr::UInt32 if (0.0..=4_294_967_295.0).contains(n) => {
        ctx.emit(Instruction::I32Const((*n as u32) as i32));
        return Ok(());
      }
      NativeValRepr::Int64S if (-9_007_199_254_740_991.0..=9_007_199_254_740_991.0).contains(n) => {
        ctx.emit(Instruction::I64Const(*n as i64));
        return Ok(());
      }
      NativeValRepr::UInt64 if (0.0..=9_007_199_254_740_991.0).contains(n) => {
        ctx.emit(Instruction::I64Const((*n as u64) as i64));
        return Ok(());
      }
      _ => {}
    },
    Calcit::Tag(t) if dst_repr == NativeValRepr::TagI32 => {
      let tag_str = t.to_string();
      let id = *ctx
        .tag_index
        .get(&tag_str)
        .ok_or_else(|| format!("unknown tag in WASM codegen: {tag_str}"))?;
      ctx.emit(Instruction::I32Const(id as i32));
      return Ok(());
    }
    Calcit::Str(s) if dst_repr == NativeValRepr::HeapPtrI32 => {
      let ptr = *ctx
        .string_pool
        .get(s.as_ref())
        .ok_or_else(|| format!("string literal not found in pool: {s}"))?;
      ctx.emit(Instruction::I32Const(ptr as i32));
      return Ok(());
    }
    Calcit::Local(local) => {
      let name = &*local.sym;
      if ctx.lambda_locals.contains_key(name) {
        emit_expr(ctx, expr)?;
        emit_unbox_f64_to_repr(ctx, dst_repr);
        return Ok(());
      }
      let idx = *ctx.locals.get(name).ok_or_else(|| format!("undefined local variable: {name}"))?;
      if dst_repr == NativeValRepr::BoolI32
        && ctx.local_repr(idx) == NativeValRepr::F64
        && !expr_is_guaranteed_unboxed_scalar(ctx, expr)
      {
        emit_value_is_truthy_i32(ctx, idx);
      } else {
        ctx.emit_local_as_repr(idx, dst_repr);
      }
      return Ok(());
    }
    Calcit::Import(import)
      if import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS
        && import.def.as_ref() == "do"
        && !ctx.fn_index.contains_key(&format!("{}/{}", import.ns, import.def)) =>
    {
      emit_unit_const_repr(ctx, dst_repr);
      return Ok(());
    }
    Calcit::Import(import) => {
      let qualified = format!("{}/{}", import.ns, import.def);
      if !ctx.atom_globals.contains_key(&qualified)
        && import.def.as_ref() != "{}"
        && import.def.as_ref() != "[]"
        && resolve_struct_ref(expr).is_err()
        && resolve_enum_ref(expr).is_err()
        && !ctx.fn_index.contains_key(&qualified)
        && let Some(value_expr) = ctx.value_imports.get(&qualified).cloned()
      {
        return emit_expr_as_repr(ctx, &value_expr, dst_repr).map_err(|reason| format!("{reason} (imported value `{qualified}`)"));
      }
    }
    Calcit::List(xs) if !xs.is_empty() => {
      return emit_call_expr_as_repr(ctx, xs, dst_repr);
    }
    _ => {}
  }

  emit_expr(ctx, expr)?;
  if dst_repr == NativeValRepr::BoolI32 && !expr_is_guaranteed_unboxed_scalar(ctx, expr) {
    let tmp = ctx.alloc_local();
    ctx.emit(Instruction::LocalSet(tmp));
    emit_value_is_truthy_i32(ctx, tmp);
  } else {
    emit_unbox_f64_to_repr(ctx, dst_repr);
  }
  Ok(())
}

fn is_core_special_call_name(name: &str) -> bool {
  name.parse::<CalcitProc>().is_ok()
    || matches!(
      name,
      "="
        | "not="
        | "union"
        | "difference"
        | "intersection"
        | "include"
        | "exclude"
        | "pairs-map"
        | "merge"
        | "merge-non-nil"
        | "fold"
        | "reduce"
        | "foldl-compare"
        | "map"
        | "&list:map"
        | "map-indexed"
        | "&list:map-indexed"
        | "each"
        | "filter"
        | "&list:filter"
        | "&set:filter"
        | "any?"
        | "every?"
        | "find"
        | "find-index"
        | "concat"
        | "deref"
        | "str"
        | "str-spaced"
        | "filter-not"
        | "dissoc"
        | "conj"
        | "update"
        | "mapcat"
        | "repeat"
        | "interleave"
        | "zipmap"
        | "intersperse"
        | "join-string"
        | "let"
        | "filter-map-kv"
        | "keys-non-nil"
        | "try-parse-json"
        | "json:parse"
        | "nil->option"
        | "'"
    )
}

/// Emit instructions for a call expression with an expected native representation.
fn emit_call_expr_as_repr(
  ctx: &mut WasmGenCtx,
  xs: &crate::wasm::host::calcit::CalcitList,
  dst_repr: NativeValRepr,
) -> Result<(), String> {
  let head = &xs[0];
  let args_list: Vec<Calcit> = xs.drop_left().to_vec();

  if dst_repr != NativeValRepr::F64
    && (matches!(head, Calcit::Local(_)) || matches!(head, Calcit::Symbol { sym, .. } if ctx.locals.contains_key(sym.as_ref())))
    && let Some(definition) = resolve_known_function_value(ctx, head)
  {
    let mut call = vec![Calcit::Import(definition)];
    call.extend(args_list);
    return emit_call_expr_as_repr(ctx, &crate::wasm::host::calcit::CalcitList::from(&call[..]), dst_repr);
  }

  if dst_repr != NativeValRepr::F64
    && !matches!(head, Calcit::Syntax(syn, _) if *syn != CalcitSyntax::DynBox)
    && !args_list
      .iter()
      .any(|arg| matches!(arg, Calcit::Syntax(CalcitSyntax::ArgSpread, _)))
    && (args_list.iter().any(|arg| selected_local(ctx, arg).is_some())
      || args_list.iter().any(|arg| impl_alternatives(ctx, arg).is_some()))
  {
    emit_call_expr(ctx, xs)?;
    emit_unbox_f64_to_repr(ctx, dst_repr);
    return Ok(());
  }

  match head {
    Calcit::Syntax(CalcitSyntax::If, _) if dst_repr != NativeValRepr::F64 => return emit_if_as_repr(ctx, &args_list, dst_repr),
    Calcit::Syntax(CalcitSyntax::CoreLet, _) if dst_repr != NativeValRepr::F64 => {
      return emit_let_as_repr(ctx, &args_list, dst_repr);
    }
    Calcit::Syntax(CalcitSyntax::HintFn, _) if dst_repr == NativeValRepr::UnitI32 => {
      ctx.emit(Instruction::I32Const(0));
      return Ok(());
    }
    Calcit::Proc(CalcitProc::Not) if dst_repr == NativeValRepr::BoolI32 => {
      return emit_not_as_i32(ctx, &args_list);
    }
    Calcit::Proc(CalcitProc::NativeLessThan) if dst_repr == NativeValRepr::BoolI32 && args_list.len() == 2 => {
      return emit_numeric_cmp_as_i32(ctx, true, &args_list);
    }
    Calcit::Proc(CalcitProc::NativeGreaterThan) if dst_repr == NativeValRepr::BoolI32 && args_list.len() == 2 => {
      return emit_numeric_cmp_as_i32(ctx, false, &args_list);
    }
    Calcit::Proc(CalcitProc::Identical) if dst_repr == NativeValRepr::BoolI32 && args_list.len() == 2 => {
      return emit_identical_as_i32(ctx, &args_list);
    }
    Calcit::Proc(CalcitProc::NativeEquals) if dst_repr == NativeValRepr::BoolI32 && args_list.len() == 2 => {
      return emit_equals_as_i32(ctx, &args_list);
    }
    Calcit::Proc(CalcitProc::NativeStruct) if dst_repr == NativeValRepr::HeapPtrI32 => {
      let ptr = emit_struct_new_raw_i32(ctx, &args_list)?;
      ctx.emit(Instruction::LocalGet(ptr));
      return Ok(());
    }
    Calcit::Proc(CalcitProc::NativeStructNth) if dst_repr != NativeValRepr::F64 => {
      return emit_struct_nth_as_repr(ctx, &args_list, dst_repr);
    }
    Calcit::Proc(CalcitProc::NativeEnum) if dst_repr == NativeValRepr::HeapPtrI32 => {
      let ptr = emit_enum_new_raw_i32(ctx, &args_list)?;
      ctx.emit(Instruction::LocalGet(ptr));
      return Ok(());
    }
    Calcit::Proc(CalcitProc::NativeNamedEnumNew) if dst_repr == NativeValRepr::HeapPtrI32 => {
      let ptr = emit_named_enum_new_raw_i32(ctx, &args_list)?;
      ctx.emit(Instruction::LocalGet(ptr));
      return Ok(());
    }
    Calcit::Proc(CalcitProc::NativeEnumNth) if dst_repr != NativeValRepr::F64 => {
      return emit_enum_nth_as_repr(ctx, &args_list, dst_repr);
    }
    Calcit::Method(name, MethodKind::Invoke(_))
      if dst_repr != NativeValRepr::F64 && matches!(name.as_ref(), "some?" | "none?" | "unwrap" | "unwrap-or") =>
    {
      let qualified = format!("calcit.core/option:{name}");
      if try_emit_direct_niche_option_call(ctx, &qualified, &args_list, dst_repr)? {
        return Ok(());
      }
    }
    Calcit::Import(import)
      if dst_repr != NativeValRepr::F64
        && import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS
        && matches!(
          import.def.as_ref(),
          "option:some?" | "option:none?" | "option:unwrap" | "option:unwrap-or"
        ) =>
    {
      let qualified = format!("calcit.core/{}", import.def);
      if try_emit_direct_niche_option_call(ctx, &qualified, &args_list, dst_repr)? {
        return Ok(());
      }
    }
    Calcit::Import(import)
      if dst_repr != NativeValRepr::F64 && import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS && import.def.as_ref() == "do" =>
    {
      return emit_body_as_repr(ctx, &args_list, dst_repr);
    }
    Calcit::Import(import)
      if dst_repr == NativeValRepr::BoolI32
        && import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS
        && import.def.as_ref() == "="
        && args_list.len() == 2 =>
    {
      return emit_equals_as_i32(ctx, &args_list);
    }
    Calcit::Import(import)
      if dst_repr == NativeValRepr::BoolI32
        && import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS
        && import.def.as_ref() == "not="
        && args_list.len() == 2 =>
    {
      emit_equals_as_i32(ctx, &args_list)?;
      ctx.emit(Instruction::I32Eqz);
      return Ok(());
    }
    _ => {}
  }

  // Try direct call to a function with native signature when dst_repr != F64
  if dst_repr != NativeValRepr::F64 && !args_list.iter().any(|a| resolve_inline_closure(ctx, a).is_some()) {
    match head {
      Calcit::Import(import) => {
        let qualified = format!("{}/{}", import.ns, import.def);
        let is_core_special =
          import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS && is_core_special_call_name(import.def.as_ref());
        if !is_core_special
          && !ctx.value_imports.contains_key(&qualified)
          && let Some(&fn_idx) = ctx.fn_index.get(&qualified).or_else(|| ctx.fn_index.get(import.def.as_ref()))
        {
          if emit_specialized_static_call(ctx, &qualified, &args_list)? {
            emit_unbox_f64_to_repr(ctx, dst_repr);
            return Ok(());
          }
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
          return emit_direct_fn_call(ctx, fn_idx, target_arity, rest_fixed, &args_list, dst_repr);
        }
      }
      Calcit::Symbol { sym, info, .. } => {
        let name = sym.as_ref();
        let qualified = format!("{}/{}", info.at_ns, name);
        let is_reserved = matches!(name, "println" | "eprintln" | "echo")
          || ctx.lambda_locals.contains_key(name)
          || ctx.locals.contains_key(name)
          || is_core_special_call_name(name)
          || ctx.value_imports.contains_key(&qualified);
        let target_name = if ctx.fn_index.contains_key(&qualified) {
          qualified.as_str()
        } else {
          name
        };
        if !is_reserved && let Some(&fn_idx) = ctx.fn_index.get(target_name) {
          if emit_specialized_static_call(ctx, target_name, &args_list)? {
            emit_unbox_f64_to_repr(ctx, dst_repr);
            return Ok(());
          }
          let target_arity = ctx.fn_arity.get(target_name).copied().unwrap_or(args_list.len() as u32);
          let rest_fixed = ctx.fn_has_rest.get(target_name).copied();
          return emit_direct_fn_call(ctx, fn_idx, target_arity, rest_fixed, &args_list, dst_repr);
        }
      }
      Calcit::Fn { info, .. } => {
        if let Some(def_ref) = &info.def_ref {
          let qualified = format!("{}/{}", def_ref.def_ns, def_ref.def_name);
          let is_core_hof =
            def_ref.def_ns.as_ref() == crate::wasm::host::calcit::CORE_NS && is_core_special_call_name(def_ref.def_name.as_ref());
          if !is_core_hof && let Some(&fn_idx) = ctx.fn_index.get(&qualified).or_else(|| ctx.fn_index.get(def_ref.def_name.as_ref())) {
            if emit_specialized_static_call(ctx, &qualified, &args_list)? {
              emit_unbox_f64_to_repr(ctx, dst_repr);
              return Ok(());
            }
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
            return emit_direct_fn_call(ctx, fn_idx, target_arity, rest_fixed, &args_list, dst_repr);
          }
        }
      }
      _ => {}
    }
  }

  emit_call_expr(ctx, xs)?;
  if dst_repr == NativeValRepr::BoolI32 && !expr_is_guaranteed_unboxed_scalar(ctx, &Calcit::List(Arc::new(xs.clone()))) {
    let tmp = ctx.alloc_local();
    ctx.emit(Instruction::LocalSet(tmp));
    emit_value_is_truthy_i32(ctx, tmp);
  } else {
    emit_unbox_f64_to_repr(ctx, dst_repr);
  }
  Ok(())
}

/// Call `fn_idx`, unboxing arguments into `fn_signatures[fn_idx]` slots when present
/// and adapting its return value into `dst_repr`.
fn emit_direct_fn_call(
  ctx: &mut WasmGenCtx,
  fn_idx: u32,
  target_arity: u32,
  rest_fixed: Option<u32>,
  args: &[Calcit],
  dst_repr: NativeValRepr,
) -> Result<(), String> {
  if rest_fixed.is_none()
    && let Some(sig) = ctx.fn_signatures.get(&fn_idx).cloned()
  {
    if args.len() != sig.params.len() {
      return Err(format!("call arity mismatch: expected {}, got {}", sig.params.len(), args.len()));
    }
    for (arg, &param_repr) in args.iter().zip(&sig.params) {
      emit_expr_as_repr(ctx, arg, param_repr)?;
    }
    ctx.emit(Instruction::Call(fn_idx));
    emit_convert_repr(ctx, sig.result, dst_repr);
    return Ok(());
  }
  emit_call_args(ctx, args, target_arity, rest_fixed)?;
  ctx.emit(Instruction::Call(fn_idx));
  emit_unbox_f64_to_repr(ctx, dst_repr);
  Ok(())
}

/// Adapt `target_arity` pre-pushed `f64` stack arguments when calling `fn_idx` from spread dispatch.
fn emit_call_fn_with_f64_stack_args(ctx: &mut WasmGenCtx, fn_idx: u32, target_arity: u32) {
  let Some(sig) = ctx.fn_signatures.get(&fn_idx).cloned() else {
    ctx.emit(Instruction::Call(fn_idx));
    return;
  };
  let mut temps = Vec::with_capacity(target_arity as usize);
  for _ in 0..target_arity {
    temps.push(ctx.alloc_local());
  }
  for &tmp in temps.iter().rev() {
    ctx.emit(Instruction::LocalSet(tmp));
  }
  for (idx, tmp) in temps.into_iter().enumerate() {
    let dst = sig.params.get(idx).copied().unwrap_or(NativeValRepr::F64);
    ctx.emit(Instruction::LocalGet(tmp));
    emit_unbox_f64_to_repr(ctx, dst);
  }
  ctx.emit(Instruction::Call(fn_idx));
  emit_box_repr_to_f64(ctx, sig.result);
}

/// Emit instructions for a call expression.
fn emit_call_expr(ctx: &mut WasmGenCtx, xs: &crate::wasm::host::calcit::CalcitList) -> Result<(), String> {
  let head = &xs[0];
  let args_list: Vec<Calcit> = xs.drop_left().to_vec();

  if (matches!(head, Calcit::Local(_)) || matches!(head, Calcit::Symbol { sym, .. } if ctx.locals.contains_key(sym.as_ref())))
    && let Some(definition) = resolve_known_function_value(ctx, head)
  {
    let mut call = vec![Calcit::Import(definition)];
    call.extend(args_list);
    return emit_call_expr(ctx, &crate::wasm::host::calcit::CalcitList::from(&call[..]));
  }

  if !matches!(head, Calcit::Syntax(syn, _) if *syn != CalcitSyntax::DynBox)
    && !args_list
      .iter()
      .any(|arg| matches!(arg, Calcit::Syntax(CalcitSyntax::ArgSpread, _)))
  {
    if let Some((name, index)) = args_list.iter().find_map(|arg| selected_local(ctx, arg)) {
      let name = name.to_owned();
      return emit_call_per_selected_impl(ctx, xs, &name, index);
    }
    if let Some(join) = args_list.iter().position(|arg| impl_alternatives(ctx, arg).is_some()) {
      return emit_call_binding_impl_join(ctx, xs, join);
    }
  }

  match head {
    Calcit::Syntax(syn, _) => match syn {
      CalcitSyntax::CallSpread => emit_call_spread(ctx, &args_list),
      CalcitSyntax::If => emit_if(ctx, &args_list),
      CalcitSyntax::CoreLet => emit_let(ctx, &args_list),
      CalcitSyntax::AnyBox => any_box::emit_any_box(ctx, &args_list),
      CalcitSyntax::AnyDowncast => any_box::emit_any_downcast(ctx, &args_list),
      CalcitSyntax::Match => emit_match(ctx, &args_list),
      CalcitSyntax::HintFn => {
        // hint-fn is metadata-only; emit nothing (0.0 placeholder)
        ctx.emit(f64_const(0.0));
        Ok(())
      }
      CalcitSyntax::AssertType => {
        // assert-type checks type at preprocess time; unbox open JSON scalar boxes
        // when narrowing to a concrete scalar representation.
        if args_list.is_empty() {
          return Err("assert-type expects at least 1 arg".into());
        }
        if matches!(&args_list[0], Calcit::Local(local) if ctx.lambda_locals.contains_key(local.sym.as_ref()))
          || matches!(&args_list[0], Calcit::Symbol { sym, .. } if ctx.lambda_locals.contains_key(sym.as_ref()))
        {
          return emit_expr(ctx, &args_list[0]);
        }
        if args_list.len() == 2 {
          let src_ev = infer_wasm_evidence(ctx, &args_list[0]);
          let src_ty = expand_wasm_value_type(src_ev.scalar_origin.as_ref().unwrap_or(&src_ev.annotation));
          if nil_type_evidence_is_open(&src_ty) {
            let target = expand_wasm_value_type(&CalcitTypeAnnotation::parse_type_annotation_form_with_generics(&args_list[1], &[]));
            emit_expr(ctx, &args_list[0])?;
            let value_local = ctx.alloc_local();
            ctx.emit(Instruction::LocalSet(value_local));
            match target.as_ref() {
              CalcitTypeAnnotation::Bool => emit_unbox_scalar_if_tag(ctx, value_local, "bool", false),
              CalcitTypeAnnotation::Nil => emit_unbox_scalar_if_tag(ctx, value_local, "nil", true),
              CalcitTypeAnnotation::Number | CalcitTypeAnnotation::Numeric(_) => {
                emit_unbox_scalar_if_tag(ctx, value_local, "number", false)
              }
              _ => {}
            }
            ctx.emit(Instruction::LocalGet(value_local));
            return Ok(());
          }
        }
        emit_expr(ctx, &args_list[0])
      }
      // Proven widening and erasure preserve the existing scalar value.
      // Reinterpretation cannot restore the identity erased by the f64 ABI.
      CalcitSyntax::UnsafeCoerce => {
        if args_list.len() != 2 {
          return Err("unsafe-coerce expects a value and type annotation".into());
        }
        let target = CalcitTypeAnnotation::parse_type_annotation_form(&args_list[1]);
        if matches!(target.as_ref(), CalcitTypeAnnotation::Dynamic)
          || infer_wasm_static_type(ctx, &args_list[0]).is_some_and(|actual| actual == target || actual.is_proven_for(&target))
        {
          emit_expr(ctx, &args_list[0])
        } else {
          Err("E_WASM_UNSUPPORTED_JS_FFI: unsafe-coerce cannot preserve value identity in the current scalar WASM ABI".into())
        }
      }
      CalcitSyntax::TryParseCirruEdnAs => edn_parse::emit_try_parse_cirru_edn_as(ctx, &args_list),
      CalcitSyntax::ParseCirruEdnAs => edn_parse::emit_parse_cirru_edn_as(ctx, &args_list),
      CalcitSyntax::TryDecodeMapAs => edn_parse::emit_try_decode_map_as(ctx, &args_list),
      CalcitSyntax::DecodeMapAs => edn_parse::emit_decode_map_as(ctx, &args_list),
      CalcitSyntax::EncodeMapAs => edn_parse::emit_encode_map_as(ctx, &args_list),
      CalcitSyntax::DynBox => dyn_trait::emit_dyn_box(ctx, &args_list),
      CalcitSyntax::JsCast => Err(format!("{syn} is not yet supported in WASM codegen")),
      CalcitSyntax::Defn => {
        let full_expr = Calcit::List(Arc::new(xs.clone()));
        closure::emit_defn_value(ctx, &xs.to_vec(), &full_expr)
      }
      CalcitSyntax::Quote | CalcitSyntax::Quasiquote => Err(format!("unsupported runtime {syn} value in WASM")),
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
        let scalar_reset = ctx.atom_is_scalar.get(&qualified) == Some(&true)
          || infer_wasm_static_type(ctx, &args_list[1])
            .is_some_and(|ty| proven_scalar_kind(&CalcitTypeAnnotation::expand_schema_alias(&ty)).is_some());
        emit_expr(ctx, &args_list[1])?;
        if !scalar_reset && let Some(&commit_idx) = ctx.runtime_fn_index.get("__rt_commit_defref") {
          ctx.emit(Instruction::Call(commit_idx));
        }
        // Store new (possibly promoted) value in global, and leave it on stack as return value
        let tmp = ctx.alloc_local();
        ctx.emit(Instruction::LocalTee(tmp));
        ctx.emit(Instruction::GlobalSet(global_idx));
        let resets_to_null = matches!(&args_list[1], Calcit::Nil | Calcit::Unit | Calcit::Bool(false))
          || matches!(&args_list[1], Calcit::Number(n) if *n == 0.0);
        if resets_to_null {
          ctx.reset_initialized_atoms.remove(&qualified);
        } else {
          ctx.reset_initialized_atoms.insert(qualified);
        }
        ctx.emit(Instruction::LocalGet(tmp));
        Ok(())
      }
      CalcitSyntax::Try => Err(
        "E_WASM_UNSUPPORTED_TRY: `try` is not supported on WASM/WASI; return nominal `Result` (`Result :ok` / `Result :err` with `match`) for recoverable errors and reserve `raise` for unrecoverable traps".into(),
      ),
      _ => Err(format!("unsupported syntax in WASM: {syn}")),
    },
    Calcit::Proc(proc) => emit_proc_call(ctx, proc, &args_list),
    Calcit::Method(name, kind) => match kind {
      MethodKind::Invoke(_) => {
        if let Some(receiver) = args_list.first().and_then(|arg| infer_wasm_value_evidence(ctx, arg))
          && let Some(callable) = resolve_runtime_method_callable(
            &receiver.annotation,
            has_effective_exact_impls(ctx, &receiver),
            name,
            &ctx.source_ns,
          )
          .map_err(|error| format!("E_WASM_TRAIT_RESOLUTION: {error}"))?
        {
          return emit_resolved_method_call(ctx, callable, &args_list);
        }
        emit_method_invoke(ctx, name.as_ref(), &args_list)
      }
      _ => Err(format!("unsupported method in WASM: .{name}")),
    },
    Calcit::Import(import) => {
      // `do` is a sequencing form in calcit.core — emit all args, return last
      if import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS && import.def.as_ref() == "do" {
        return emit_body(ctx, &args_list);
      }
      // High-level set wrappers use reduce+lambda which can't be
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
          "=" => return emit_variadic_equals(ctx, &args_list),
          "not=" => return emit_not_equals(ctx, &args_list),
          "union" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Union),
          "difference" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Difference),
          "intersection" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Intersection),
          "include" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Include),
          "exclude" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Exclude),
          "pairs-map" => return emit_pairs_map(ctx, &args_list),
          "merge" => return emit_map_merge_variadic(ctx, &args_list),
          "merge-non-nil" => return emit_map_merge_non_nil_variadic(ctx, &args_list),
          // Seeded fold aliases lower to the same loop for proc/import callees.
          "fold" | "reduce" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
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
          // `filter-not` — keep elements where f(elem) is falsy.
          "filter-not" if args_list.len() == 2 => return emit_filter_not(ctx, &args_list),
          // `dissoc` — delegated to map dissoc when the receiver is statically a Map.
          "dissoc"
            if args_list.len() >= 2
              && infer_wasm_static_type(ctx, &args_list[0]).is_some_and(|ty| {
                matches!(
                  CalcitTypeAnnotation::expand_schema_alias(&ty).as_ref(),
                  CalcitTypeAnnotation::Map(_, _)
                )
              }) =>
          {
            return emit_map_dissoc(ctx, &args_list);
          }
          // `conj` — append one or more elements to a list.
          "conj" if args_list.len() >= 2 => return emit_conj(ctx, &args_list),
          // `update` — rewrite a List/Map/Enum slot with f(old value) when it exists.
          "update" if args_list.len() == 3 => return emit_update(ctx, &args_list),
          // `mapcat xs f` — map then flatten one level.
          "mapcat" if args_list.len() == 2 => return emit_mapcat(ctx, &args_list),
          // Simple loop implementations for HOF-based definitions.
          "repeat" if args_list.len() == 2 => return emit_repeat(ctx, &args_list),
          "interleave" if args_list.len() == 2 => return emit_interleave(ctx, &args_list),
          "zipmap" if args_list.len() == 2 => return emit_zipmap(ctx, &args_list),
          "intersperse" if args_list.len() == 2 => return emit_join(ctx, &args_list),
          "join-string" if args_list.len() == 2 => return emit_join_string(ctx, &args_list),
          // `let` — multi-binding form: (let ((name val)...) body...).
          // The preprocessor normally expands this to nested `&let` forms, but intercept here
          // as a fallback for cases where the macro expansion hasn't occurred.
          "let" if !args_list.is_empty() => return emit_let_multi(ctx, &args_list),
          // `filter-map-kv xs f` — consume typed :keep/:drop decisions.
          "filter-map-kv" if args_list.len() == 2 => return emit_filter_map_kv(ctx, &args_list),
          "keys-non-nil" if args_list.len() == 1 => return emit_map_keys_non_nil(ctx, &args_list),
          "try-parse-json" if args_list.len() == 1 => return edn_parse::emit_try_parse_json(ctx, &args_list),
          "json:parse" if args_list.len() == 1 => return edn_parse::emit_try_parse_json_value(ctx, &args_list),
          "nil->option" if args_list.len() == 1 => {
            if let Some(lowered) = lower_nil_to_option_target(ctx, &args_list[0])? {
              return emit_expr(ctx, &lowered);
            }
          }
          // `'` — list literal constructor (calcit.core/'), same as ([] a b c).
          "'" => return emit_list_new(ctx, &args_list),
          _ => {}
        }
      }
      // Try qualified "ns/def" first, then bare "def" as fallback
      let qualified = format!("{}/{}", import.ns, import.def);
      if let Some(Calcit::Proc(proc)) = ctx.value_imports.get(&qualified).cloned() {
        return emit_proc_call(ctx, &proc, &args_list);
      }
      if !ctx.fn_index.contains_key(&qualified)
        && (ctx.value_imports.contains_key(&qualified) || ctx.atom_globals.contains_key(&qualified))
      {
        return closure::emit_closure_call_expr(ctx, |ctx| emit_expr(ctx, head), &args_list);
      }
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
      emit_direct_fn_call(ctx, fn_idx, target_arity, rest_fixed, &args_list, NativeValRepr::F64)
    }
    Calcit::Symbol { sym, info, .. } => {
      let name = sym.as_ref();
      let qualified = format!("{}/{}", info.at_ns, name);
      // IO functions return the backend's Unit representation after logging.
      if matches!(name, "println" | "eprintln" | "echo") {
        if ctx.target == WasmTarget::Wasi && ctx.boundary == WasmBoundary::Component {
          return emit_wasi_component_print(ctx, name, &args_list);
        }
        if ctx.target == WasmTarget::Wasi {
          return emit_wasi_print(ctx, name, &args_list);
        }
        return emit_core_print(ctx, name, &args_list);
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
        "fold" | "reduce" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
        "update" if args_list.len() == 3 => return emit_update(ctx, &args_list),
        "filter-map-kv" if args_list.len() == 2 => return emit_filter_map_kv(ctx, &args_list),
        "keys-non-nil"
          if args_list.len() == 1
            && info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
            && !ctx.lambda_locals.contains_key(name)
            && !ctx.locals.contains_key(name) =>
        {
          return emit_map_keys_non_nil(ctx, &args_list);
        }
        "="
          if info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
            && !ctx.lambda_locals.contains_key(name)
            && !ctx.locals.contains_key(name) =>
        {
          return emit_variadic_equals(ctx, &args_list);
        }
        "not="
          if info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
            && !ctx.lambda_locals.contains_key(name)
            && !ctx.locals.contains_key(name) =>
        {
          return emit_not_equals(ctx, &args_list);
        }
        "exclude"
          if info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
            && !ctx.lambda_locals.contains_key(name)
            && !ctx.locals.contains_key(name) =>
        {
          return emit_set_op_variadic(ctx, &args_list, SetOpKind::Exclude);
        }
        "pairs-map"
          if info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
            && !ctx.lambda_locals.contains_key(name)
            && !ctx.locals.contains_key(name) =>
        {
          return emit_pairs_map(ctx, &args_list);
        }
        "merge"
          if info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
            && !ctx.lambda_locals.contains_key(name)
            && !ctx.locals.contains_key(name) =>
        {
          return emit_map_merge_variadic(ctx, &args_list);
        }
        "merge-non-nil"
          if info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
            && !ctx.lambda_locals.contains_key(name)
            && !ctx.locals.contains_key(name) =>
        {
          return emit_map_merge_non_nil_variadic(ctx, &args_list);
        }
        "try-parse-json"
          if args_list.len() == 1
            && info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
            && !ctx.lambda_locals.contains_key(name)
            && !ctx.locals.contains_key(name) =>
        {
          return edn_parse::emit_try_parse_json(ctx, &args_list);
        }
        "json:parse"
          if args_list.len() == 1
            && info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
            && !ctx.lambda_locals.contains_key(name)
            && !ctx.locals.contains_key(name) =>
        {
          return edn_parse::emit_try_parse_json_value(ctx, &args_list);
        }
        "nil->option"
          if args_list.len() == 1
            && info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
            && !ctx.lambda_locals.contains_key(name)
            && !ctx.locals.contains_key(name) =>
        {
          if let Some(lowered) = lower_nil_to_option_target(ctx, &args_list[0])? {
            return emit_expr(ctx, &lowered);
          }
        }
        _ => {}
      }
      // Check if this symbol refers to an inline lambda, local closure, or self-recursive closure in scope.
      if let Some(closure) = ctx.lambda_locals.get(name).cloned()
        && !closure::inline_closure_requires_materialization(&closure)
      {
        return emit_inline_closure_call(ctx, &closure, &args_list, None);
      }
      if let Some(&local_idx) = ctx.locals.get(name) {
        return closure::emit_closure_call_expr(
          ctx,
          |ctx| {
            ctx.emit_local_as_f64(local_idx);
            Ok(())
          },
          &args_list,
        );
      }
      if let Some((ref self_name, body_fn_idx, ref shape)) = ctx.self_closure.clone()
        && self_name == name
      {
        return closure::emit_self_closure_call(ctx, body_fn_idx, shape, &args_list);
      }
      if let Some(Calcit::Proc(proc)) = ctx.value_imports.get(&qualified).cloned() {
        return emit_proc_call(ctx, &proc, &args_list);
      }
      let target_name = if ctx.fn_index.contains_key(&qualified) {
        qualified.as_str()
      } else {
        name
      };
      if emit_specialized_static_call(ctx, target_name, &args_list)? {
        return Ok(());
      }
      let fn_idx = *ctx
        .fn_index
        .get(target_name)
        .ok_or_else(|| format!("unknown function symbol in direct call: {sym:?}"))?;
      let target_arity = ctx.fn_arity.get(target_name).copied().unwrap_or(args_list.len() as u32);
      let rest_fixed = ctx.fn_has_rest.get(target_name).copied();
      emit_direct_fn_call(ctx, fn_idx, target_arity, rest_fixed, &args_list, NativeValRepr::F64)
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
        return emit_core_print(ctx, name, &args_list);
      }
      Err(format!("unsupported registered proc in WASM: {name}"))
    }
    Calcit::Fn { info, .. } => {
      let Some(def_ref) = info.def_ref.as_ref() else {
        return closure::emit_closure_call_expr(ctx, |ctx| closure::emit_fn_value(ctx, info, head), &args_list);
      };
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
          "fold" | "reduce" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
          "update" if args_list.len() == 3 => return emit_update(ctx, &args_list),
          "filter-map-kv" if args_list.len() == 2 => return emit_filter_map_kv(ctx, &args_list),
          "keys-non-nil" if args_list.len() == 1 => return emit_map_keys_non_nil(ctx, &args_list),
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
      emit_direct_fn_call(ctx, fn_idx, target_arity, rest_fixed, &args_list, NativeValRepr::F64)
    }
    // Inline IIFE: call head is `(defn name (params...) body...)`
    // Generated by `apply-args` and similar macros for TCO-friendly recursive loops.
    Calcit::List(iife_items) if matches!(iife_items.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) => {
      let params = match iife_items.get(2) {
        Some(Calcit::List(param_list)) => param_list
          .iter()
          .filter_map(|p| match p {
            Calcit::Local(CalcitLocal { sym, .. }) | Calcit::Symbol { sym, .. } if !matches!(sym.as_ref(), "&" | "?") => {
              Some(sym.as_ref().to_owned())
            }
            _ => None,
          })
          .collect::<Vec<_>>(),
        other => return Err(format!("IIFE defn: params list expected, got: {other:?}")),
      };
      let body: Vec<Calcit> = iife_items.iter().skip(3).cloned().collect();
      let shape = inline_lambda_shape(head);
      if let Some((required, false)) = shape
        && required == params.len()
      {
        let option_types = inline_option_types(ctx, head, params.len(), (required, false));
        return emit_inline_iife(ctx, &params, &body, &args_list, None, &option_types);
      }
      // `&` / `?` parameters bind like native. With `recur`, the loop lowering
      // rebinds them by the same rules (#1972); otherwise the inline closure
      // path also specializes closure arguments.
      if body.iter().any(check_uses_recur) {
        return match shape {
          Some(shape) => emit_inline_iife(ctx, &params, &body, &args_list, Some(shape), &[]),
          None => {
            ctx.emit(Instruction::Unreachable);
            Ok(())
          }
        };
      }
      match capture_inline_closure(ctx, head) {
        Some(closure) => emit_inline_closure_call(ctx, &closure, &args_list, None),
        None => {
          ctx.emit(Instruction::Unreachable);
          Ok(())
        }
      }
    }
    Calcit::List(_)
      if resolve_inline_closure(ctx, head)
        .as_ref()
        .is_some_and(|c| !closure::inline_closure_requires_materialization(c)) =>
    {
      let closure = resolve_inline_closure(ctx, head).expect("checked inline closure head");
      emit_inline_closure_call(ctx, &closure, &args_list, None)
    }
    // Dynamic call via a local variable holding a closure pointer (f64).
    Calcit::Local(local) => {
      let local_name = local.sym.as_ref().to_string();
      // If this local is an inline lambda, inline the call directly.
      if let Some(closure) = ctx.lambda_locals.get(&local_name).cloned()
        && !closure::inline_closure_requires_materialization(&closure)
      {
        return emit_inline_closure_call(ctx, &closure, &args_list, None);
      }
      if let Some(&local_idx) = ctx.locals.get(&local_name) {
        return closure::emit_closure_call_expr(
          ctx,
          |ctx| {
            ctx.emit_local_as_f64(local_idx);
            Ok(())
          },
          &args_list,
        );
      }
      if let Some((ref self_name, body_fn_idx, ref shape)) = ctx.self_closure.clone()
        && self_name == &local_name
      {
        return closure::emit_self_closure_call(ctx, body_fn_idx, shape, &args_list);
      }
      Err(format!("undefined local used as function: {}", local.sym))
    }
    Calcit::List(_) => closure::emit_closure_call_expr(ctx, |ctx| emit_expr(ctx, head), &args_list),
    _ => Err(format!("unsupported call head in WASM: {head}")),
  }
}

fn emit_call_spread(ctx: &mut WasmGenCtx, args_list: &[Calcit]) -> Result<(), String> {
  if args_list.is_empty() {
    return Err("&call-spread expects at least a callee".into());
  }

  let head = &args_list[0];
  let call_args = &args_list[1..];
  // A known target and literal spreads carry ordinary-call evidence. Reuse
  // specialization (for example Map `.merge`), while retaining the ABI guard
  // below for unknown function-table targets whose Fn type does not prove arity.
  if let Some(definition) = resolve_known_function_value(ctx, head) {
    if definition.ns.as_ref() == "calcit.core"
      && definition.def.as_ref() == "dissoc"
      && call_args.len() >= 2
      && !matches!(call_args.first(), Some(Calcit::Syntax(CalcitSyntax::ArgSpread, _)))
      && infer_wasm_static_type(ctx, &call_args[0]).is_some_and(|ty| {
        matches!(
          CalcitTypeAnnotation::expand_schema_alias(&ty).as_ref(),
          CalcitTypeAnnotation::Map(_, _)
        )
      })
    {
      return emit_map_dissoc(ctx, call_args);
    }
    let mut expanded = vec![head.clone()];
    let mut runtime_spreads = vec![];
    expand_literal_spread_args("call", call_args, &mut expanded, &mut runtime_spreads)?;
    if runtime_spreads.is_empty() {
      let qualified = format!("{}/{}", definition.ns, definition.def);
      reject_nil_specialized_spread_call(ctx, &qualified, definition.def.as_ref())?;
      reject_trait_specialized_spread_call(ctx, &qualified, definition.def.as_ref())?;
      let rest = ctx
        .fn_has_rest
        .get(&qualified)
        .or_else(|| ctx.fn_has_rest.get(definition.def.as_ref()));
      if rest.is_none()
        && let Some(arity) = ctx.fn_arity.get(&qualified).or_else(|| ctx.fn_arity.get(definition.def.as_ref()))
        && expanded.len() - 1 > *arity as usize
      {
        return Err(format!(
          "E_WASM_CALL_SPECIALIZATION: `{qualified}` expects at most {arity} args, got {}",
          expanded.len() - 1
        ));
      }
      return emit_call_expr(ctx, &crate::wasm::host::calcit::CalcitList::from(expanded.as_slice()));
    }
  }
  if (matches!(head, Calcit::Local(_)) || matches!(head, Calcit::Symbol { sym, .. } if ctx.locals.contains_key(sym.as_ref())))
    && let Some(definition) = resolve_known_function_value(ctx, head)
  {
    let mut call = vec![Calcit::Import(definition)];
    call.extend_from_slice(call_args);
    return emit_call_spread(ctx, &call);
  }

  match head {
    Calcit::Import(import) => {
      let qualified = format!("{}/{}", import.ns, import.def);
      reject_nil_specialized_spread_call(ctx, &qualified, import.def.as_ref())?;
      reject_trait_specialized_spread_call(ctx, &qualified, import.def.as_ref())?;
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
      emit_call_fn_with_f64_stack_args(ctx, fn_idx, target_arity);
      Ok(())
    }
    Calcit::Symbol { sym, .. } => {
      let name = sym.as_ref();
      reject_nil_specialized_spread_call(ctx, name, name)?;
      reject_trait_specialized_spread_call(ctx, name, name)?;
      let fn_idx = *ctx
        .fn_index
        .get(name)
        .ok_or_else(|| format!("unknown function symbol in spread call: {sym:?}"))?;
      let target_arity = ctx.fn_arity.get(name).copied().unwrap_or(call_args.len() as u32);
      let rest_fixed = ctx.fn_has_rest.get(name).copied();
      emit_call_spread_args(ctx, call_args, target_arity, rest_fixed)?;
      emit_call_fn_with_f64_stack_args(ctx, fn_idx, target_arity);
      Ok(())
    }
    Calcit::Fn { info, .. } => {
      let Some(def_ref) = info.def_ref.as_ref() else {
        let mut expanded = Vec::new();
        let mut runtime_spreads = Vec::new();
        expand_literal_spread_args("call", call_args, &mut expanded, &mut runtime_spreads)?;
        if runtime_spreads.is_empty() {
          return closure::emit_closure_call_expr(ctx, |ctx| closure::emit_fn_value(ctx, info, head), &expanded);
        }
        return Err("unsupported runtime spread call on anonymous function in WASM".into());
      };
      let qualified = format!("{}/{}", def_ref.def_ns, def_ref.def_name);
      reject_nil_specialized_spread_call(ctx, &qualified, def_ref.def_name.as_ref())?;
      reject_trait_specialized_spread_call(ctx, &qualified, def_ref.def_name.as_ref())?;
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
      emit_call_fn_with_f64_stack_args(ctx, fn_idx, target_arity);
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
    // Dynamic spread call via a local holding a function value. It lowers as an
    // ordinary call only when every spread is a `([] ...)` literal and the local
    // is an inline lambda accepting the expanded argument count; the ordinary
    // call binds `?` / `&` parameters like native. A function-table-backed local
    // traps: its declared Fn type does not prove a fixed arity (a rest-parameter
    // function satisfies a fixed Fn type), and a runtime spread has no static
    // length (#1964).
    Calcit::Local(local) => {
      let mut plain = vec![head.clone()];
      let mut runtime_spreads = vec![];
      expand_literal_spread_args(&local.sym, call_args, &mut plain, &mut runtime_spreads)?;
      let arg_count = plain.len() - 1;
      let accepting_inline_lambda = ctx
        .lambda_locals
        .get(local.sym.as_ref())
        .is_some_and(|closure| closure.accepts(arg_count));
      if runtime_spreads.is_empty() && accepting_inline_lambda {
        return emit_call_expr(ctx, &crate::wasm::host::calcit::CalcitList::from(&plain[..]));
      }
      ctx.emit(Instruction::Unreachable);
      Ok(())
    }
    _ => Err(format!("unsupported call head in WASM: {head}")),
  }
}

/// For proc callees in spread calls such as `(proc a & xs b & ys)`.
///
/// A spread list is never dropped. Every `([] ...)` literal after `&` is
/// inlined as ordinary arguments, matching native argument expansion, so any
/// number of literal spreads may appear at any position. A runtime spread has
/// no static length: `&list:concat a ... & xs` lowers it directly, and every
/// other proc is rejected because WASM proc calls need a fixed arity.
fn emit_call_spread_args_as_regular(ctx: &mut WasmGenCtx, proc: &CalcitProc, call_args: &[Calcit]) -> Result<(), String> {
  let mut plain: Vec<Calcit> = Vec::with_capacity(call_args.len());
  let mut runtime_spreads: Vec<(usize, Calcit)> = vec![];
  expand_literal_spread_args(proc.as_ref(), call_args, &mut plain, &mut runtime_spreads)?;
  match runtime_spreads.as_slice() {
    [] => emit_proc_call(ctx, proc, &plain),
    [(pos, spread)] if matches!(proc, CalcitProc::NativeListConcat) && *pos == plain.len() => {
      emit_list_concat_spread(ctx, &plain, spread)
    }
    _ => Err(format!(
      "E_WASM_CALL_SPECIALIZATION: WASM cannot spread a runtime list into proc `{proc}`; pass arguments explicitly or use a literal list"
    )),
  }
}

/// Splits spread-call arguments into ordinary arguments and runtime spreads.
///
/// Items of `([] ...)` literals after `&` are spliced in place (recursively, so
/// a literal that itself contains `&` keeps its inner spreads). Each runtime
/// spread is recorded with its index in `plain` at that point.
fn expand_literal_spread_args(
  callee: &str,
  args: &[Calcit],
  plain: &mut Vec<Calcit>,
  runtime_spreads: &mut Vec<(usize, Calcit)>,
) -> Result<(), String> {
  let mut iter = args.iter();
  while let Some(arg) = iter.next() {
    if !matches!(arg, Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
      plain.push(arg.clone());
      continue;
    }
    let spread = match iter.next() {
      Some(Calcit::Syntax(CalcitSyntax::ArgSpread, _)) | None => {
        return Err(format!(
          "E_WASM_CALL_SPECIALIZATION: spread call to `{callee}` expects a list after `&`"
        ));
      }
      Some(spread) => spread,
    };
    match list_literal_items(spread) {
      Some(items) => expand_literal_spread_args(callee, &items, plain, runtime_spreads)?,
      None => runtime_spreads.push((plain.len(), spread.clone())),
    }
  }
  Ok(())
}

/// Items of a `([] a b ...)` list literal, which may still contain `&` markers.
///
/// Preprocessing rewrites a literal with its own spread, `([] a & ys)`, into
/// `(&call-spread [] a & ys)`, so both forms are recognized.
fn list_literal_items(expr: &Calcit) -> Option<Vec<Calcit>> {
  let Calcit::List(xs) = expr else {
    return None;
  };
  let items = xs.to_vec();
  match items.as_slice() {
    [Calcit::Proc(CalcitProc::List), rest @ ..] => Some(rest.to_vec()),
    [
      Calcit::Syntax(CalcitSyntax::CallSpread, _),
      Calcit::Proc(CalcitProc::List),
      rest @ ..,
    ] => Some(rest.to_vec()),
    _ => None,
  }
}

fn emit_packed_spread_args_list(ctx: &mut WasmGenCtx, tail_args: &[Calcit]) -> Result<u32, String> {
  let mut acc_i32: Option<u32> = None;
  let mut run_locals: Vec<u32> = Vec::new();
  let flush_run = |ctx: &mut WasmGenCtx, run_locals: &mut Vec<u32>, acc_i32: &mut Option<u32>| {
    if run_locals.is_empty() {
      return;
    }
    let seg_f64 = emit_list_from_locals(ctx, run_locals);
    run_locals.clear();
    let seg_i32 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(seg_f64));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(seg_i32));
    *acc_i32 = Some(match *acc_i32 {
      Some(prev) => emit_list_concat_two_i32(ctx, prev, seg_i32),
      None => seg_i32,
    });
  };

  let mut iter = tail_args.iter();
  while let Some(arg) = iter.next() {
    if matches!(arg, Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
      flush_run(ctx, &mut run_locals, &mut acc_i32);
      let spread_expr = iter
        .next()
        .filter(|item| !matches!(item, Calcit::Syntax(CalcitSyntax::ArgSpread, _)))
        .ok_or("&call-spread missing spread list expression")?;
      let spread_i32 = emit_ptr_to_i32(ctx, spread_expr)?;
      acc_i32 = Some(match acc_i32 {
        Some(prev) => emit_list_concat_two_i32(ctx, prev, spread_i32),
        None => spread_i32,
      });
    } else {
      let local = ctx.alloc_local();
      emit_expr(ctx, arg)?;
      ctx.emit(Instruction::LocalSet(local));
      run_locals.push(local);
    }
  }
  flush_run(ctx, &mut run_locals, &mut acc_i32);
  acc_i32.ok_or_else(|| "&call-spread missing spread list expression".into())
}

fn emit_call_spread_args(ctx: &mut WasmGenCtx, call_args: &[Calcit], target_arity: u32, rest_fixed: Option<u32>) -> Result<(), String> {
  let Some(fixed) = rest_fixed else {
    return Err("&call-spread in WASM currently requires the target function to accept rest args".into());
  };
  let fixed = fixed as usize;

  // Locate the first ArgSpread marker (if any) to split leading explicit args from the spread tail.
  let spread_pos = call_args
    .iter()
    .position(|a| matches!(a, Calcit::Syntax(CalcitSyntax::ArgSpread, _)));

  let (explicit_args, tail_args): (&[Calcit], &[Calcit]) = if let Some(pos) = spread_pos {
    let prefix_fixed = fixed.min(pos);
    (&call_args[..prefix_fixed], &call_args[prefix_fixed..])
  } else if call_args.len() == fixed + 1 {
    // Old-style: no ArgSpread marker; last arg is the rest list.
    (&call_args[..fixed], &call_args[fixed..])
  } else {
    return Err(format!(
      "&call-spread in WASM expects {} fixed args plus `& spread-list`, got {} args",
      fixed,
      call_args.len()
    ));
  };

  let n_explicit = explicit_args.len();
  let n_from_spread = fixed.saturating_sub(n_explicit);

  // Emit leading explicit fixed args.
  for arg in explicit_args {
    emit_expr(ctx, arg)?;
  }

  if spread_pos.is_none() {
    // Old-style single rest list expression at `tail_args[0]`.
    emit_expr(ctx, &tail_args[0])?;
  } else if n_from_spread == 0 && tail_args.len() == 2 && matches!(tail_args[0], Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
    // Fast path: all fixed params already emitted and a single `& spread_list` is the entire rest arg.
    let spread_list_expr = &tail_args[1];
    if matches!(spread_list_expr, Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
      return Err("&call-spread missing spread list expression".into());
    }
    emit_expr(ctx, spread_list_expr)?;
  } else {
    let combined_i32 = emit_packed_spread_args_list(ctx, tail_args)?;
    if n_from_spread == 0 {
      ctx.ptr_to_f64(combined_i32);
    } else {
      let combined_count = emit_load_count_i32(ctx, combined_i32);
      ctx.emit(Instruction::LocalGet(combined_count));
      ctx.emit(Instruction::I32Const(n_from_spread as i32));
      ctx.emit(Instruction::I32LtU);
      ctx.begin_block_if();
      ctx.emit(Instruction::Unreachable);
      ctx.emit(Instruction::End);

      // Extract each required fixed param: list[i] = combined_i32[(1+i)*8]
      for i in 0..n_from_spread {
        ctx.emit(Instruction::LocalGet(combined_i32));
        ctx.emit(Instruction::I32Const(((1 + i) * 8) as i32));
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
      }

      // Emit the rest arg = list_slice(combined, n_from_spread).
      emit_list_slice_from_i32_local(ctx, combined_i32, n_from_spread)?;
    }
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

/// Both ordinary and explicit-origin dispatch use the actual selected callable.
fn emit_resolved_method_call(ctx: &mut WasmGenCtx, callable: Calcit, args: &[Calcit]) -> Result<(), String> {
  match &callable {
    Calcit::Import(import) if import.ns.as_ref() == "calcit.core" => {
      let qualified = format!("calcit.core/{}", import.def);
      if try_emit_direct_niche_option_call(ctx, &qualified, args, NativeValRepr::F64)? {
        return Ok(());
      }
    }
    Calcit::Fn { info, .. } => {
      if let Some(def_ref) = &info.def_ref
        && def_ref.def_ns.as_ref() == "calcit.core"
      {
        let qualified = format!("calcit.core/{}", def_ref.def_name);
        if try_emit_direct_niche_option_call(ctx, &qualified, args, NativeValRepr::F64)? {
          return Ok(());
        }
      }
    }
    _ => {}
  }
  if let Calcit::Fn { id, info } = &callable
    && info.scope.is_empty()
    && resolve_known_function_value(ctx, &callable).is_none()
    && let Some(closure) = capture_inline_closure(ctx, &callable)
  {
    let closure = InlineClosure {
      captured_locals: HashMap::new(),
      captured_local_types: HashMap::new(),
      captured_closures: HashMap::new(),
      ..(*closure).clone()
    };
    return emit_inline_closure_call(ctx, &closure, args, Some(&format!("method:{id}")));
  }
  let mut direct = vec![callable];
  direct.extend(args.iter().cloned());
  let Calcit::List(items) = Calcit::from(direct) else {
    unreachable!("selected method call is nonempty")
  };
  emit_call_expr(ctx, &items)
}

/// Lower a trait-origin-preserving call through its proven receiver type.
fn emit_static_trait_call(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() < 3 {
    return Err("E_WASM_TRAIT_CALL: &trait-call expects a trait, method tag, and receiver".into());
  }
  let Calcit::Tag(method_name) = &args[1] else {
    return Err("E_WASM_TRAIT_CALL: method name must be a compile-time tag".into());
  };
  let receiver = infer_wasm_value_evidence(ctx, &args[2])
    .filter(|evidence| !nil_type_evidence_is_open(&evidence.annotation) && !evidence.annotation.contains_type_var())
    .ok_or_else(|| "E_WASM_TRAIT_TYPE_EVIDENCE: &trait-call requires a concrete receiver type".to_owned())?;
  let receiver_type = receiver.annotation;
  let expanded_receiver = expand_wasm_value_type(&receiver_type);
  if let CalcitTypeAnnotation::DynTrait(boxed_trait) = expanded_receiver.as_ref() {
    return dyn_trait::emit_dyn_trait_dispatch(ctx, boxed_trait, &args[0], method_name.ref_str(), &args[2..]);
  }
  let nominal = receiver_type.resolve_to_enum().is_some()
    || receiver_type.resolve_to_struct().is_some()
    || matches!(receiver_type.as_ref(), CalcitTypeAnnotation::AnonymousEnum)
    || matches!(receiver_type.as_ref(), CalcitTypeAnnotation::Custom(value) if matches!(value.as_ref(), Calcit::Tag(tag) if tag.ref_str() == "struct"));
  if nominal && !receiver.exact_impls && !is_statically_coherent_nominal(ctx, receiver_type.as_ref()) {
    return Err(
      "E_WASM_TRAIT_TYPE_EVIDENCE: &trait-call requires the receiver's actual implementation identity, not only a data contract".into(),
    );
  }
  let file_ns = ctx
    .specialization_stack
    .last()
    .and_then(|qualified| qualified.rsplit_once('/').map(|(ns, _)| ns))
    .unwrap_or(&ctx.source_ns);
  let callable = resolve_static_trait_callable(&args[0], method_name.ref_str(), receiver_type.as_ref(), file_ns)
    .map_err(|error| format!("E_WASM_TRAIT_RESOLUTION: {error}"))?;
  emit_resolved_method_call(ctx, callable, &args[2..])
}

fn emit_not_as_i32(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "not")?;
  let ev = infer_wasm_evidence(ctx, &args[0]);
  let ty = expand_wasm_value_type(ev.scalar_origin.as_ref().unwrap_or(&ev.annotation));
  match ty.as_ref() {
    CalcitTypeAnnotation::Never => {
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::Drop);
      ctx.emit(Instruction::I32Const(0));
      Ok(())
    }
    CalcitTypeAnnotation::Bool => {
      emit_expr_as_repr(ctx, &args[0], NativeValRepr::BoolI32)?;
      ctx.emit(Instruction::I32Eqz);
      Ok(())
    }
    _ => Err(format!(
      "E_NON_BOOL_CONDITION: not expects Bool in WASM, got `{}`",
      ty.to_brief_string()
    )),
  }
}

/// Emit instructions for builtin proc calls.
fn emit_proc_call(ctx: &mut WasmGenCtx, proc: &CalcitProc, args: &[Calcit]) -> Result<(), String> {
  match proc {
    CalcitProc::NativeTraitCall => emit_static_trait_call(ctx, args),
    // Arithmetic
    CalcitProc::NativeAdd => emit_binary(ctx, Instruction::F64Add, args),
    CalcitProc::NativeMinus => emit_binary(ctx, Instruction::F64Sub, args),
    CalcitProc::NativeMultiply => emit_binary(ctx, Instruction::F64Mul, args),
    CalcitProc::NativeDivide => emit_binary(ctx, Instruction::F64Div, args),
    CalcitProc::NativeNumberRem => {
      // Truncated remainder over safe integers; traps where native and JS raise an error.
      if args.len() != 2 {
        return Err("rem expects 2 args".into());
      }
      let a = ctx.alloc_local();
      let b = ctx.alloc_local();
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::LocalSet(a));
      emit_expr(ctx, &args[1])?;
      ctx.emit(Instruction::LocalSet(b));
      let mut checks = Vec::new();
      for operand in [a, b] {
        checks.extend([
          Instruction::LocalGet(operand),
          Instruction::F64Trunc,
          Instruction::LocalGet(operand),
          Instruction::F64Ne,
          Instruction::LocalGet(operand),
          Instruction::F64Abs,
          f64_const(9_007_199_254_740_991.0),
          Instruction::F64Gt,
          Instruction::I32Or,
        ]);
        component_trap_if(&mut checks);
      }
      checks.extend([Instruction::LocalGet(b), f64_const(0.0), Instruction::F64Eq]);
      component_trap_if(&mut checks);
      for instruction in checks {
        ctx.emit(instruction);
      }
      // Safe integers convert to i64 exactly, so the integer remainder is exact and never `-0`.
      ctx.emit(Instruction::LocalGet(a));
      ctx.emit(Instruction::I64TruncF64S);
      ctx.emit(Instruction::LocalGet(b));
      ctx.emit(Instruction::I64TruncF64S);
      ctx.emit(Instruction::I64RemS);
      ctx.emit(Instruction::F64ConvertI64S);
      Ok(())
    }

    // Comparisons — produce f64 (1.0 or 0.0)
    CalcitProc::NativeLessThan => emit_numeric_cmp(ctx, Instruction::F64Lt, args),
    CalcitProc::NativeGreaterThan => emit_numeric_cmp(ctx, Instruction::F64Gt, args),
    CalcitProc::NativeEquals => emit_equals(ctx, args),
    CalcitProc::Identical => {
      emit_identical_as_i32(ctx, args)?;
      ctx.emit(Instruction::F64ConvertI32U);
      Ok(())
    }
    CalcitProc::NativeCompare => {
      // &compare a b → -1.0 if a<b, 0.0 if a==b, 1.0 if a>b
      expect_arity(2, args, "&compare")?;
      let a = ctx.alloc_local();
      let b = ctx.alloc_local();
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::LocalSet(a));
      emit_expr(ctx, &args[1])?;
      ctx.emit(Instruction::LocalSet(b));
      let a_ty = infer_wasm_static_type(ctx, &args[0]).map(|ty| CalcitTypeAnnotation::expand_schema_alias(&ty));
      let b_ty = infer_wasm_static_type(ctx, &args[1]).map(|ty| CalcitTypeAnnotation::expand_schema_alias(&ty));
      let scalar_compare_rank = |ty: Option<&CalcitTypeAnnotation>| -> Option<u8> {
        match ty {
          Some(CalcitTypeAnnotation::Nil) => Some(0),
          Some(CalcitTypeAnnotation::Unit) => Some(1),
          Some(CalcitTypeAnnotation::Bool) => Some(2),
          Some(CalcitTypeAnnotation::Number | CalcitTypeAnnotation::Numeric(_)) => Some(3),
          Some(CalcitTypeAnnotation::Tag) => Some(4),
          Some(CalcitTypeAnnotation::String) => Some(5),
          _ => None,
        }
      };
      if let (Some(left_rank), Some(right_rank)) = (scalar_compare_rank(a_ty.as_deref()), scalar_compare_rank(b_ty.as_deref()))
        && left_rank != right_rank
      {
        ctx.emit(f64_const(if left_rank < right_rank { -1.0 } else { 1.0 }));
        return Ok(());
      }
      let is_proven_numeric_scalar = |ty: Option<&CalcitTypeAnnotation>| {
        matches!(
          ty,
          Some(
            CalcitTypeAnnotation::Number
              | CalcitTypeAnnotation::Numeric(_)
              | CalcitTypeAnnotation::Bool
              | CalcitTypeAnnotation::Nil
              | CalcitTypeAnnotation::Unit
          )
        )
      };
      // `Tag` values are interned into monotonically increasing UTF-8-sorted IDs
      // in `tag_index`, so comparing two proven `Tag` operands compares UTF-8 order.
      if (is_proven_numeric_scalar(a_ty.as_deref()) && is_proven_numeric_scalar(b_ty.as_deref()))
        || (matches!(a_ty.as_deref(), Some(CalcitTypeAnnotation::Tag)) && matches!(b_ty.as_deref(), Some(CalcitTypeAnnotation::Tag)))
      {
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
        ctx.emit(Instruction::LocalGet(a));
        ctx.emit(Instruction::LocalGet(a));
        ctx.emit(Instruction::F64Ne);
        ctx.emit(Instruction::F64ConvertI32U);
        ctx.emit(Instruction::LocalGet(b));
        ctx.emit(Instruction::LocalGet(b));
        ctx.emit(Instruction::F64Ne);
        ctx.emit(Instruction::F64ConvertI32U);
        ctx.emit(Instruction::F64Sub);
        ctx.emit(Instruction::End);
        ctx.emit(Instruction::End);
        return Ok(());
      }
      if matches!(a_ty.as_deref(), Some(CalcitTypeAnnotation::String)) && matches!(b_ty.as_deref(), Some(CalcitTypeAnnotation::String))
      {
        ctx.emit(Instruction::LocalGet(a));
        ctx.emit(Instruction::I32TruncF64U);
        ctx.emit(Instruction::LocalGet(b));
        ctx.emit(Instruction::I32TruncF64U);
        ctx.call_rt("__rt_str_compare");
        return Ok(());
      }
      // Strings are heap pointers: ordering them as numbers would compare
      // allocation addresses, so dispatch to the lexicographic runtime compare.
      // A string sorts after every non-string, matching native ordering.
      let a_is_string = emit_is_string(ctx, a)?;
      let b_is_string = emit_is_string(ctx, b)?;
      ctx.emit(Instruction::LocalGet(a_is_string));
      ctx.emit(Instruction::LocalGet(b_is_string));
      ctx.emit(Instruction::I32Or);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      ctx.emit(Instruction::LocalGet(a_is_string));
      ctx.emit(Instruction::LocalGet(b_is_string));
      ctx.emit(Instruction::I32And);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      ctx.emit(Instruction::LocalGet(a));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::LocalGet(b));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.call_rt("__rt_str_compare");
      ctx.emit(Instruction::Else);
      ctx.emit(f64_const(1.0));
      ctx.emit(f64_const(-1.0));
      ctx.emit(Instruction::LocalGet(a_is_string));
      ctx.emit(Instruction::Select);
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::Else);
      // if a < b then -1 else (if a > b then 1 else isnan(a) - isnan(b))
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
      // neither below nor above: equal, or NaN involved; NaN sorts after every number
      ctx.emit(Instruction::LocalGet(a));
      ctx.emit(Instruction::LocalGet(a));
      ctx.emit(Instruction::F64Ne);
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalGet(b));
      ctx.emit(Instruction::LocalGet(b));
      ctx.emit(Instruction::F64Ne);
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::F64Sub);
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);
      Ok(())
    }
    CalcitProc::Not => {
      expect_arity(1, args, "not")?;
      let ev = infer_wasm_evidence(ctx, &args[0]);
      let ty = expand_wasm_value_type(ev.scalar_origin.as_ref().unwrap_or(&ev.annotation));
      match ty.as_ref() {
        CalcitTypeAnnotation::Never => emit_expr(ctx, &args[0]),
        CalcitTypeAnnotation::Bool => {
          emit_not_as_i32(ctx, args)?;
          ctx.emit(Instruction::F64ConvertI32U);
          Ok(())
        }
        _ => Err(format!(
          "E_NON_BOOL_CONDITION: not expects Bool in WASM, got `{}`",
          ty.to_brief_string()
        )),
      }
    }

    // Math (unary)
    CalcitProc::Floor => emit_unary(ctx, Instruction::F64Floor, args),
    CalcitProc::Ceil => emit_unary(ctx, Instruction::F64Ceil, args),
    CalcitProc::Round => emit_round_half_away(ctx, args),
    CalcitProc::Sqrt => emit_unary(ctx, Instruction::F64Sqrt, args),

    // integer?: a finite Number with no fractional part, not a safe-integer bound.
    CalcitProc::IsInteger => {
      expect_arity(1, args, "integer?")?;
      let v = ctx.alloc_local();
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::LocalSet(v));
      ctx.emit(Instruction::LocalGet(v));
      ctx.emit(Instruction::LocalGet(v));
      ctx.emit(Instruction::F64Trunc);
      // Infinities produce NaN here, so they cannot compare equal to zero.
      ctx.emit(Instruction::F64Sub);
      ctx.emit(f64_const(0.0));
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
    CalcitProc::Pow => emit_pow(ctx, args),

    // type-of: reads the heap type header or returns :number for non-pointers.
    CalcitProc::TypeOf => emit_type_of(ctx, args),

    // type predicates
    CalcitProc::ListQuestion => emit_type_predicate(ctx, "list", args),
    CalcitProc::TagQuestion => emit_type_predicate(ctx, "tag", args),
    CalcitProc::SymbolQuestion => emit_type_predicate(ctx, "symbol", args),
    CalcitProc::NilQuestion => emit_nil_predicate(ctx, args),
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
      let selections = validate_recur_evidence(ctx, args)?;
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

        // Evaluate explicit prefix args into temps matching each parameter's native repr.
        let mut temps = Vec::new();
        for (idx, arg) in explicit.iter().enumerate() {
          let target_repr = ctx.arg_indices.get(idx).map_or(NativeValRepr::F64, |&local| ctx.local_repr(local));
          let tmp = ctx.alloc_local_typed(target_repr.val_type());
          emit_expr_as_repr(ctx, arg, target_repr)?;
          ctx.emit(Instruction::LocalSet(tmp));
          temps.push(tmp);
        }

        // Extract (n_remaining - 1) individual elements from spread list for the remaining fixed slots.
        for i in 0..(n_remaining - 1) {
          let slot = explicit.len() + i;
          let target_repr = ctx.arg_indices.get(slot).map_or(NativeValRepr::F64, |&local| ctx.local_repr(local));
          let tmp = ctx.alloc_local_typed(target_repr.val_type());
          ctx.emit(Instruction::LocalGet(spread_i32));
          ctx.emit(Instruction::I32Const(((1 + i) * 8) as i32));
          ctx.emit(Instruction::I32Add);
          ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
          emit_unbox_f64_to_repr(ctx, target_repr);
          ctx.emit(Instruction::LocalSet(tmp));
          temps.push(tmp);
        }

        // Last slot: list_slice(spread, n_remaining-1) — the new rest list.
        let rest_repr = ctx
          .arg_indices
          .get(total - 1)
          .map_or(NativeValRepr::F64, |&local| ctx.local_repr(local));
        let rest_slice_local = ctx.alloc_local_typed(rest_repr.val_type());
        emit_list_slice_from_i32_local(ctx, spread_i32, n_remaining - 1)?;
        emit_unbox_f64_to_repr(ctx, rest_repr);
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
        emit_recur_selections(ctx, &selections);
        ctx.emit(Instruction::Br(ctx.block_depth));
        ctx.emit(Instruction::Unreachable);
        Ok(())
      } else {
        // Non-spread recur binds its arguments like a call (#1972): a missing
        // `?` parameter is nil and a `&` parameter receives the trailing
        // arguments as a new list.
        let total = ctx.arg_indices.len();
        let (required, rest) = ctx.recur_shape.unwrap_or((total, false));
        let positional = total - usize::from(rest);
        if args.len() < required || (!rest && args.len() > positional) {
          let expected = if rest {
            format!("at least {required}")
          } else if required == positional {
            positional.to_string()
          } else {
            format!("{required} to {positional}")
          };
          return Err(format!("recur arity mismatch: expected {expected}, got {}", args.len()));
        }
        // Evaluate all args into temp locals matching each parameter's native repr
        let mut temps = Vec::new();
        for (i, arg) in args.iter().enumerate() {
          let target_repr = if i < positional {
            ctx.arg_indices.get(i).map_or(NativeValRepr::F64, |&idx| ctx.local_repr(idx))
          } else {
            NativeValRepr::F64
          };
          let tmp = ctx.alloc_local_typed(target_repr.val_type());
          emit_expr_as_repr(ctx, arg, target_repr)?;
          ctx.emit(Instruction::LocalSet(tmp));
          temps.push(tmp);
        }
        let trailing = if temps.len() > positional {
          temps.split_off(positional)
        } else {
          vec![]
        };
        while temps.len() < positional {
          let nil = ctx.alloc_local();
          ctx.emit(f64_const(0.0));
          ctx.emit(Instruction::LocalSet(nil));
          temps.push(nil);
        }
        if rest {
          let list = emit_list_from_locals(ctx, &trailing);
          temps.push(list);
        }
        // Copy temps back to arg locals
        for (i, &tmp) in temps.iter().enumerate() {
          ctx.emit(Instruction::LocalGet(tmp));
          ctx.emit(Instruction::LocalSet(ctx.arg_indices[i]));
        }
        emit_recur_selections(ctx, &selections);
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
    CalcitProc::NativeStructAssocAt => emit_struct_assoc_at(ctx, args),
    CalcitProc::NativeStructWithAt => emit_struct_with_at(ctx, args),
    CalcitProc::NativeStructAssoc | CalcitProc::NativeStructWith => Err(format!("{proc} not yet supported in WASM codegen")),
    CalcitProc::NativeStructFromMap
    | CalcitProc::NativeStructExtendAs
    | CalcitProc::NativeStructImpls
    | CalcitProc::NativeLooseStruct => Err(format!("Struct operation {proc} not yet supported in WASM codegen")),
    CalcitProc::NativeStructContains => emit_struct_contains(ctx, args),
    CalcitProc::NativeStructMatches => emit_struct_matches(ctx, args),

    // Enum operations
    CalcitProc::NativeEnum => emit_enum_new(ctx, args),
    CalcitProc::NativeEnumNth => emit_enum_nth(ctx, args),
    CalcitProc::NativeEnumCount => emit_enum_count(ctx, args),
    CalcitProc::NativeEnumValidate => Err("runtime enum validation is not supported in WASM".into()),
    // %:: enum variant constructor: (enum_class tag payload...) — ignore enum_class
    CalcitProc::NativeNamedEnumNew => emit_named_enum_new(ctx, args),
    CalcitProc::NativeEnumDefinition => emit_enum_definition(ctx, args),
    CalcitProc::NativeEnumImpls
    | CalcitProc::NativeEnumParams
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
      emit_i32_operand(ctx, &args[0])?;
      ctx.emit(Instruction::I32Const(-1)); // all bits set
      ctx.emit(Instruction::I32Xor);
      ctx.emit(Instruction::F64ConvertI32S);
      Ok(())
    }

    // Raise — terminates execution
    CalcitProc::Raise | CalcitProc::Unreachable | CalcitProc::NativeUnreachable => {
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
    CalcitProc::NativeMapFoldKv => hof::emit_map_fold_kv(ctx, args),

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
    CalcitProc::TurnString => emit_turn_string(ctx, args),
    CalcitProc::NativeStr => {
      let Some((value, trailing)) = args.split_first() else {
        return Err("&str requires at least one argument".into());
      };
      emit_turn_string(ctx, std::slice::from_ref(value))?;
      for arg in trailing {
        emit_expr(ctx, arg)?;
        ctx.emit(Instruction::Drop);
      }
      Ok(())
    }
    CalcitProc::Trim => emit_trim(ctx, args),
    CalcitProc::IsBlank => emit_blank(ctx, args),
    CalcitProc::GetCharCode => emit_get_char_code(ctx, args),
    CalcitProc::ParseFloat => emit_parse_float(ctx, args),
    CalcitProc::CharFromCode => emit_char_from_code(ctx, args),
    CalcitProc::NativeStrReplace => emit_str_replace(ctx, args),
    CalcitProc::NativeStrEscape => emit_str_escape(ctx, args),
    CalcitProc::FormatCirruEdn => emit_format_cirru_edn(ctx, args),
    CalcitProc::JsonStringify => edn::emit_json_stringify(ctx, args, false),
    CalcitProc::JsonPretty => edn::emit_json_stringify(ctx, args, true),
    CalcitProc::JsonParse => edn_parse::emit_json_parse(ctx, args),
    CalcitProc::NativeTryParseJson => {
      expect_arity(2, args, "&try-parse-json")?;
      let prototype_type = infer_wasm_static_type(ctx, &args[0]);
      if !prototype_type.as_ref().is_some_and(|annotation| {
        let expanded = expand_wasm_value_type(annotation);
        matches!(expanded.as_ref(), CalcitTypeAnnotation::Never) || proven_disjoint_value_kind(&expanded) == Some("enum-def")
      }) {
        return Err("E_WASM_EDN_TYPE: &try-parse-json requires a statically known enum definition".into());
      }
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::Drop);
      edn_parse::emit_try_parse_json(ctx, &args[1..2])
    }
    CalcitProc::NativeTryParseJsonValue => {
      expect_arity(3, args, "&try-parse-json-value")?;
      for (idx, label) in [(0, "Result"), (1, "JsonValue")] {
        let prototype_type = infer_wasm_static_type(ctx, &args[idx]);
        if !prototype_type.as_ref().is_some_and(|annotation| {
          let expanded = expand_wasm_value_type(annotation);
          matches!(expanded.as_ref(), CalcitTypeAnnotation::Never) || proven_disjoint_value_kind(&expanded) == Some("enum-def")
        }) {
          return Err(format!(
            "E_WASM_EDN_TYPE: &try-parse-json-value requires a statically known {label} enum definition"
          ));
        }
        emit_expr(ctx, &args[idx])?;
        ctx.emit(Instruction::Drop);
      }
      edn_parse::emit_try_parse_json_value(ctx, &args[2..3])
    }
    CalcitProc::NativeTryParseEdnValue => {
      expect_arity(3, args, "&try-parse-edn-value")?;
      for (idx, label) in [(0, "Result"), (1, "EdnValue")] {
        let prototype_type = infer_wasm_static_type(ctx, &args[idx]);
        if !prototype_type.as_ref().is_some_and(|annotation| {
          let expanded = expand_wasm_value_type(annotation);
          matches!(expanded.as_ref(), CalcitTypeAnnotation::Never) || proven_disjoint_value_kind(&expanded) == Some("enum-def")
        }) {
          return Err(format!(
            "E_WASM_EDN_TYPE: &try-parse-edn-value requires a statically known {label} enum definition"
          ));
        }
        emit_expr(ctx, &args[idx])?;
        ctx.emit(Instruction::Drop);
      }
      let fn_idx = *ctx
        .fn_index
        .get("calcit.core/&edn:parse-pure")
        .ok_or_else(|| "missing calcit.core/&edn:parse-pure in WASM module".to_string())?;
      emit_direct_fn_call(ctx, fn_idx, 1, None, &args[2..3], NativeValRepr::F64)
    }
    CalcitProc::Split => emit_split(ctx, args),
    CalcitProc::SplitLines => emit_split_lines(ctx, args),

    // --- List higher-order and utility operations ---
    CalcitProc::NativeListDistinct => emit_list_distinct(ctx, args),

    // Higher-order list operations
    CalcitProc::Foldl => emit_foldl(ctx, args),
    CalcitProc::FoldrShortcut => emit_foldr_shortcut(ctx, args),

    // Static quoted expressions can be formatted without runtime reflection.
    CalcitProc::FormatToLisp => emit_format_to_lisp(ctx, args),
    CalcitProc::GetEnv => {
      expect_arity(1, args, "&get-env")?;
      if ctx.target == WasmTarget::Wasi && ctx.world == WasiWorld::Http {
        return Err("E_WASI_HTTP_CAPABILITY: `&get-env` is not supported in `--world http` (`wasi:http/proxy`); only HTTP incoming requests, response building, and stdio logging are available".into());
      }
      emit_expr(ctx, &args[0])?;
      let name = ctx.alloc_local();
      ctx.emit(Instruction::LocalSet(name));
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
      // The host reports a missing variable as 0.0; present text (even empty) is a heap pointer.
      let value = ctx.alloc_local();
      ctx.emit(Instruction::LocalSet(value));
      ctx.emit(Instruction::LocalGet(value));
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Eq);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      emit_option_enum(ctx, None)?;
      ctx.emit(Instruction::Else);
      emit_option_enum(ctx, Some(value))?;
      ctx.emit(Instruction::End);
      Ok(())
    }
    CalcitProc::GetArgs => {
      expect_arity(0, args, "get-args")?;
      if ctx.target != WasmTarget::Wasi {
        return Err("E_WASM_CAPABILITY: process arguments are unavailable for the core WASM target".into());
      }
      if ctx.world == WasiWorld::Http {
        return Err("E_WASI_HTTP_CAPABILITY: `get-args` is not supported in `--world http` (`wasi:http/proxy`); only HTTP incoming requests, response building, and stdio logging are available".into());
      }
      ctx.call_rt("__rt_wasi_get_args");
      Ok(())
    }
    CalcitProc::UnixTimeMs => {
      expect_arity(0, args, "unix-time-ms")?;
      emit_wasi_clock_ms(ctx, 0, "unix-time-ms")
    }
    CalcitProc::MonotonicTimeMs => {
      expect_arity(0, args, "monotonic-time-ms")?;
      emit_wasi_clock_ms(ctx, 1, "monotonic-time-ms")
    }
    CalcitProc::NativeWaitMs => emit_wasi_wait_ms(ctx, args),
    CalcitProc::NativeSecureRandomBytes => emit_wasi_secure_random_bytes(ctx, args),
    CalcitProc::NativeFsReadText => emit_wasi_read_text(ctx, args, false),
    CalcitProc::NativeReadStdinText => emit_wasi_read_text(ctx, args, true),
    CalcitProc::NativeFsReadDir => emit_wasi_fs_read_dir(ctx, args),
    CalcitProc::NativeFsWriteText => emit_wasi_fs_write_text(ctx, args),

    // @atom deref: just emit the argument (which should already be a GlobalGet)
    CalcitProc::AtomDeref => {
      expect_arity(1, args, "&atom:deref")?;
      emit_expr(ctx, &args[0])
    }

    CalcitProc::Quit => {
      expect_arity(1, args, "quit!")?;
      if ctx.target == WasmTarget::Wasi && ctx.world == WasiWorld::Http {
        return Err("E_WASI_HTTP_CAPABILITY: `quit!` is not supported in `--world http` (`wasi:http/proxy`); return `Result :err` from the handler instead".into());
      }
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

    // Runtime tag interning is not represented by the static WASM tag table.
    CalcitProc::TurnTag => {
      expect_arity(1, args, "turn-tag")?;
      Err("E_WASM_TAG_CONVERSION: turn-tag/to-tag requires runtime tag interning, which WASM does not support".into())
    }

    // Runtime trait tables are intentionally not implemented in the internal
    // WASM validation backend. Preprocessing must eliminate these operations.
    CalcitProc::NativeStructImplTraits | CalcitProc::NativeEnumImplTraits => {
      Err("runtime trait registration must be eliminated before WASM codegen".into())
    }

    CalcitProc::RegisterCalcitBuiltinImpls => Err("runtime builtin impl registration must be eliminated before WASM codegen".into()),

    // &impl::new — only valid as compile-time metadata for WASM.
    CalcitProc::NativeImplNew => Err("runtime trait impl construction must be eliminated before WASM codegen".into()),

    // &assert-traits — a residual assertion would require a runtime trait table.
    CalcitProc::NativeAssertTraits => Err(
      "runtime trait assertions are not supported by the internal WASM backend; make the receiver type statically resolvable".into(),
    ),

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

/// Emit `round`: nearest integer with ties away from zero (`f64.nearest` rounds
/// ties to even). Computes `trunc(x) + copysign(|x - trunc(x)| >= 0.5, x)`; the
/// fractional part is exact, `-0.4` keeps `-0`, and NaN/Infinity pass through.
fn emit_round_half_away(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "round")?;
  emit_expr(ctx, &args[0])?;
  let x = ctx.alloc_local();
  let t = ctx.alloc_local();
  ctx.emit(Instruction::LocalTee(x));
  ctx.emit(Instruction::F64Trunc);
  ctx.emit(Instruction::LocalTee(t));
  ctx.emit(Instruction::LocalGet(x));
  ctx.emit(Instruction::LocalGet(t));
  ctx.emit(Instruction::F64Sub);
  ctx.emit(Instruction::F64Abs);
  ctx.emit(f64_const(0.5));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalGet(x));
  ctx.emit(Instruction::F64Copysign);
  ctx.emit(Instruction::F64Add);
  Ok(())
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

/// Emit `type-of v`, preserving proven scalar kinds before inspecting memory.
/// Heap objects carry a type tag at `raw_base` (ptr - 8); values without
/// concrete scalar evidence retain the existing pointer heuristic below.
///
/// Pointer detection heuristic (enough for core-library usage patterns):
/// - value must be a finite integer (== trunc(v))
/// - value must be within `[HEAP_BASE + 8, current_heap_ptr)` (logical ptrs begin
///   8 bytes after the raw base)
/// - the i32 offset at `(ptr - 8)` must contain a registered type tag id
///
/// Open values failing these checks retain the existing `:number` fallback.
/// Nullable/open scalar values remain ambiguous under the erased f64 ABI.
fn emit_type_of(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err(format!("type-of expects 1 arg, got {}", args.len()));
  }
  if let Some(static_ty) = infer_wasm_static_type(ctx, &args[0]) {
    let expanded = expand_wasm_value_type(&static_ty);
    if matches!(expanded.as_ref(), CalcitTypeAnnotation::Fn(_) | CalcitTypeAnnotation::DynFn) {
      let tag = get_type_tag(ctx, "fn");
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::Drop);
      ctx.emit(f64_const(tag));
      return Ok(());
    }
    if let Some(kind) = proven_scalar_kind(&static_ty) {
      let tag = get_type_tag(ctx, kind);
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::Drop);
      ctx.emit(f64_const(tag));
      return Ok(());
    }
  }
  let v_local = ctx.alloc_local_typed(ValType::F64);
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(v_local));
  methods::emit_type_of_local(ctx, v_local);
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

/// Lower nil? from static type evidence. The current scalar WASM ABI uses the
/// same f64 bits for nil, false, and numeric zero, so value comparison cannot
/// preserve Calcit semantics. Concrete types make the result decidable while
/// the expression is still evaluated exactly once for effects.
fn emit_nil_predicate(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "nil?")?;
  let annotation = infer_wasm_static_type(ctx, &args[0])
    .ok_or_else(|| "E_WASM_NIL_TYPE_EVIDENCE: nil? requires a statically proven argument type in the current scalar ABI".to_owned())?;
  match annotation.as_ref() {
    CalcitTypeAnnotation::Nil => {
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::Drop);
      ctx.emit(f64_const(1.0));
    }
    CalcitTypeAnnotation::Nullable(inner, NullableOrigin::Inferred) if wasm_non_nil_value_is_nonzero(inner) => {
      if ctx.has_boxed_json_scalars && ctx.tag_index.contains_key("nil") {
        let v = ctx.alloc_local();
        emit_expr(ctx, &args[0])?;
        ctx.emit(Instruction::LocalSet(v));
        let is_boxed_nil = emit_has_heap_tag(ctx, v, "nil");
        ctx.emit(Instruction::LocalGet(v));
        ctx.emit(f64_const(0.0));
        ctx.emit(Instruction::F64Eq);
        ctx.emit(Instruction::LocalGet(is_boxed_nil));
        ctx.emit(Instruction::I32Or);
        ctx.emit(Instruction::F64ConvertI32U);
      } else {
        emit_expr(ctx, &args[0])?;
        ctx.emit(f64_const(0.0));
        ctx.emit(Instruction::F64Eq);
        ctx.emit(Instruction::F64ConvertI32U);
      }
    }
    CalcitTypeAnnotation::Nullable(inner, NullableOrigin::Inferred) => {
      return Err(format!(
        "E_WASM_NIL_TYPE_EVIDENCE: nil? cannot distinguish nil from a zero-valued nil-join {} payload in the current scalar ABI",
        inner.to_brief_string()
      ));
    }
    annotation if nil_type_evidence_is_open(annotation) => {
      return Err(format!(
        "E_WASM_NIL_TYPE_EVIDENCE: nil? cannot distinguish nil from false or 0 for open type `{}`",
        annotation.to_brief_string()
      ));
    }
    _ => {
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::Drop);
      ctx.emit(f64_const(0.0));
    }
  }
  Ok(())
}

/// `pow` through the host import, except the IEEE 754 cases hosts like JS `Math.pow`
/// get wrong: `pow 1 y` is 1 for any y, and `pow -1 ±inf` is 1.
fn emit_pow(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "pow")?;
  let x = ctx.alloc_local();
  let y = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(x));
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(y));
  ctx.emit(Instruction::LocalGet(x));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::LocalGet(x));
  ctx.emit(Instruction::F64Abs);
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::LocalGet(y));
  ctx.emit(Instruction::F64Abs);
  ctx.emit(f64_const(f64::INFINITY));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(x));
  ctx.emit(Instruction::LocalGet(y));
  ctx.emit(Instruction::Call(resolve_host_import(ctx, "math", "pow")?));
  ctx.emit(Instruction::End);
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

/// Read a WASI clock and return milliseconds.
fn emit_wasi_clock_ms(ctx: &mut WasmGenCtx, clock_id: i32, proc_name: &str) -> Result<(), String> {
  if ctx.target != WasmTarget::Wasi {
    return Err(format!("E_WASM_CAPABILITY: {proc_name} is unavailable for the core WASM target"));
  }
  if ctx.world == WasiWorld::Http {
    return Err(format!(
      "E_WASI_HTTP_CAPABILITY: `{proc_name}` is not supported in `--world http` (`wasi:http/proxy`); only HTTP incoming requests, response building, and stdio logging are available"
    ));
  }
  if ctx.boundary == WasmBoundary::Component {
    if clock_id == 1 {
      let now_idx = resolve_host_import(ctx, "wasi:clocks/monotonic-clock@0.3.1", "now")
        .map_err(|_| format!("E_WASI_COMMAND_CAPABILITY: `{proc_name}` needs a reachable WASI 0.3 clocks effect"))?;
      ctx.emit(Instruction::Call(now_idx));
      ctx.emit(Instruction::F64ConvertI64U);
      ctx.emit(f64_const(1_000_000.0));
      ctx.emit(Instruction::F64Div);
      return Ok(());
    }
    let now_idx = resolve_host_import(ctx, "wasi:clocks/system-clock@0.3.1", "now")
      .map_err(|_| format!("E_WASI_COMMAND_CAPABILITY: `{proc_name}` needs a reachable WASI 0.3 clocks effect"))?;
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::Call(now_idx));
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::I64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::F64ConvertI64S);
    ctx.emit(f64_const(1000.0));
    ctx.emit(Instruction::F64Mul);
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::I32Load(mem_arg_i32(8)));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(f64_const(1_000_000.0));
    ctx.emit(Instruction::F64Div);
    ctx.emit(Instruction::F64Add);
    return Ok(());
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

/// Lower the typed synchronous wait boundary through `__rt_wasi_wait`.
fn emit_wasi_wait_ms(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&wait-ms")?;
  if ctx.target != WasmTarget::Wasi {
    return Err("E_WASM_CAPABILITY: wait-ms is unavailable for the core WASM target".into());
  }
  if ctx.world == WasiWorld::Http {
    return Err(
      "E_WASI_HTTP_CAPABILITY: `&wait-ms` is not supported in `--world http` (`wasi:http/proxy`); only HTTP incoming requests, response building, and stdio logging are available"
        .into(),
    );
  }
  if ctx.boundary == WasmBoundary::Component && !ctx.runtime_fn_index.contains_key("__rt_wasi_wait") {
    return Err("E_WASI_COMMAND_CAPABILITY: `&wait-ms` needs a reachable WASI 0.3 clocks effect".into());
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

/// Fill a managed Buffer through WASI random and wrap it in the caller's Result type.
fn emit_wasi_secure_random_bytes(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&secure-random-bytes")?;
  if ctx.target != WasmTarget::Wasi {
    return Err("E_WASM_CAPABILITY: secure-random-bytes is unavailable for the core WASM target".into());
  }
  if ctx.world == WasiWorld::Http {
    return Err(
      "E_WASI_HTTP_CAPABILITY: `&secure-random-bytes` is not supported in `--world http` (`wasi:http/proxy`); only HTTP incoming requests, response building, and stdio logging are available"
        .into(),
    );
  }
  if ctx.boundary == WasmBoundary::Component && !ctx.runtime_fn_index.contains_key("__rt_wasi_component_random_get") {
    return Err("E_WASI_COMMAND_CAPABILITY: `&secure-random-bytes` needs a reachable WASI 0.3 random effect".into());
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
  if ctx.boundary == WasmBoundary::Component {
    ctx.call_rt("__rt_wasi_component_random_get");
  } else {
    ctx.emit(Instruction::Call(resolve_host_import(ctx, "wasi_snapshot_preview1", "random_get")?));
  }
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

/// Share strict UTF-8 decoding and Result construction for file and stdin reads.
fn emit_wasi_read_text(ctx: &mut WasmGenCtx, args: &[Calcit], stdin: bool) -> Result<(), String> {
  expect_arity(
    if stdin { 2 } else { 3 },
    args,
    if stdin { "&read-stdin-text" } else { "&fs-read-text" },
  )?;
  if ctx.target != WasmTarget::Wasi {
    return Err("E_WASM_CAPABILITY: host text reads are unavailable for the core WASM target".into());
  }
  if ctx.world == WasiWorld::Http {
    let op = if stdin { "read-stdin-text" } else { "&fs-read-text" };
    return Err(format!(
      "E_WASI_HTTP_CAPABILITY: `{op}` is not supported in `--world http` (`wasi:http/proxy`); only HTTP incoming requests, response building, and stdio logging are available"
    ));
  }
  if stdin && ctx.boundary != WasmBoundary::Component {
    return Err("E_WASI_COMMAND_CAPABILITY: read-stdin-text requires `calcit wasi --boundary component`".into());
  }
  if ctx.boundary == WasmBoundary::Component {
    let reader = if stdin {
      "__rt_wasi_component_stdin_bytes"
    } else {
      "__rt_wasi_component_read_bytes"
    };
    if !ctx.runtime_fn_index.contains_key(reader) {
      return Err("E_WASI_COMMAND_CAPABILITY: text reading needs a reachable WASI 0.3 input effect".into());
    }
    let path = if stdin { None } else { Some(emit_ptr_to_i32(ctx, &args[1])?) };
    let host_error = ctx.alloc_local();
    emit_expr(ctx, args.last().expect("read error argument"))?;
    ctx.emit(Instruction::LocalSet(host_error));
    let output = ctx.alloc_local_typed(ValType::I32);
    let data = ctx.alloc_local_typed(ValType::I32);
    let length = ctx.alloc_local_typed(ValType::I32);
    let success = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::I32Const(4));
    ctx.emit(Instruction::I32Const(8));
    ctx.call_rt("__cabi_realloc");
    ctx.emit(Instruction::LocalSet(output));
    // This expression can be inlined into a loop, so a failed read must not reuse prior data.
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::LocalSet(data));
    if let Some(path) = path {
      ctx.emit(Instruction::LocalGet(path));
    }
    ctx.emit(Instruction::LocalGet(output));
    ctx.call_rt(reader);
    ctx.emit(Instruction::LocalSet(success));
    ctx.emit(Instruction::LocalGet(success));
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    ctx.emit(Instruction::LocalGet(output));
    ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
    ctx.emit(Instruction::LocalSet(data));
    ctx.emit(Instruction::LocalGet(output));
    ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
    ctx.emit(Instruction::LocalSet(length));
    ctx.emit(Instruction::LocalGet(data));
    ctx.emit(Instruction::LocalGet(length));
    ctx.call_rt("__rt_utf8_valid");
    ctx.emit(Instruction::LocalSet(success));
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::LocalGet(success));
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
    ctx.emit(Instruction::LocalGet(data));
    ctx.emit(Instruction::LocalGet(length));
    ctx.call_rt("__str_new");
    let content = ctx.alloc_local();
    ctx.emit(Instruction::LocalSet(content));
    ctx.emit(Instruction::LocalGet(data));
    ctx.emit(Instruction::I32Const(runtime::WASI_TEXT_FILE_LIMIT as i32 + 1));
    ctx.call_rt("__cabi_free");
    ctx.emit(Instruction::LocalGet(output));
    ctx.emit(Instruction::I32Const(8));
    ctx.call_rt("__cabi_free");
    emit_result_enum(ctx, "ok", content)?;
    ctx.emit(Instruction::Else);
    ctx.emit(Instruction::LocalGet(data));
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    ctx.emit(Instruction::LocalGet(data));
    ctx.emit(Instruction::I32Const(runtime::WASI_TEXT_FILE_LIMIT as i32 + 1));
    ctx.call_rt("__cabi_free");
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::LocalGet(output));
    ctx.emit(Instruction::I32Const(8));
    ctx.call_rt("__cabi_free");
    emit_result_enum(ctx, "err", host_error)?;
    ctx.emit(Instruction::End);
    return Ok(());
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

/// Lower the typed filesystem directory boundary through `__rt_wasi_read_dir`.
fn emit_wasi_fs_read_dir(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if !(4..=5).contains(&args.len()) {
    return Err(format!("&fs-read-dir expected 4 or 5 arguments, got {}", args.len()));
  }
  if ctx.target != WasmTarget::Wasi {
    return Err("E_WASM_CAPABILITY: filesystem directory reads are unavailable for the core WASM target".into());
  }
  if ctx.world == WasiWorld::Http {
    return Err(
      "E_WASI_HTTP_CAPABILITY: `&fs-read-dir` is not supported in `--world http` (`wasi:http/proxy`); only HTTP incoming requests, response building, and stdio logging are available"
        .into(),
    );
  }
  if args.len() == 5 && matches!(args[4], Calcit::Bool(true)) {
    return Err("E_WASM_CAPABILITY: recursive `&fs-read-dir` (`FsPath .walk-dir`) is not yet supported on WASI".into());
  }
  if ctx.boundary == WasmBoundary::Component && !ctx.runtime_fn_index.contains_key("__rt_wasi_read_dir") {
    return Err("E_WASI_COMMAND_CAPABILITY: `&fs-read-dir` needs a reachable WASI 0.3 filesystem effect".into());
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::Drop);
  let path_type = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(path_type));
  let path = emit_ptr_to_i32(ctx, &args[2])?;
  let host_error = ctx.alloc_local();
  emit_expr(ctx, &args[3])?;
  ctx.emit(Instruction::LocalSet(host_error));
  let dynamic_recursive = args.len() == 5 && !matches!(args[4], Calcit::Bool(false));
  if dynamic_recursive {
    emit_if_condition(ctx, &args[4])?;
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
    let message_ptr = *ctx
      .string_pool
      .get("E_WASM_CAPABILITY: recursive directory reads are not supported on WASI")
      .ok_or_else(|| "missing recursive directory capability message in WASM string pool".to_owned())?;
    let message = ctx.alloc_local();
    ctx.emit(f64_const(message_ptr as f64));
    ctx.emit(Instruction::LocalSet(message));
    emit_result_enum(ctx, "err", message)?;
    ctx.emit(Instruction::Else);
  }
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
  let fs_path_layout = ctx
    .struct_layouts
    .ids
    .iter()
    .find(|(id, _)| id.name == "FsPath" && id.definition.as_deref() == Some("calcit.core/FsPath"))
    .or_else(|| ctx.struct_layouts.ids.iter().find(|(id, _)| id.name == "FsPath"))
    .and_then(|(_, layout_id)| ctx.struct_layouts.layouts.get(layout_id).cloned());
  let (fs_path_bytes, fs_path_field) = match fs_path_layout {
    Some(layout) if !layout.fields.is_empty() => (layout.payload_bytes as i32, layout.fields[0]),
    _ => (
      16,
      StructFieldLayout {
        offset: 8,
        kind: StructFieldKind::HeapPtrI32,
      },
    ),
  };
  let struct_ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, fs_path_bytes, struct_ptr, "struct");
  ctx.emit(Instruction::LocalGet(struct_ptr));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalGet(struct_ptr));
  ctx.emit(Instruction::LocalGet(path_type));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Store(mem_arg_i32(4)));
  emit_store_struct_field_from_f64_local(ctx, struct_ptr, fs_path_field, value);
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
  if dynamic_recursive {
    ctx.emit(Instruction::End);
  }
  Ok(())
}

/// Lower the typed filesystem write boundary through private Preview 1 helpers.
fn emit_wasi_fs_write_text(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(4, args, "&fs-write-text")?;
  if ctx.target != WasmTarget::Wasi {
    return Err("E_WASM_CAPABILITY: filesystem writes are unavailable for the core WASM target".into());
  }
  if ctx.world == WasiWorld::Http {
    return Err(
      "E_WASI_HTTP_CAPABILITY: `&fs-write-text` is not supported in `--world http` (`wasi:http/proxy`); only HTTP incoming requests, response building, and stdio logging are available"
        .into(),
    );
  }
  if ctx.boundary == WasmBoundary::Component {
    if !ctx.runtime_fn_index.contains_key("__rt_wasi_component_write_bytes") {
      return Err("E_WASI_COMMAND_CAPABILITY: `&fs-write-text` needs a reachable WASI 0.3 filesystem effect".into());
    }
    let path = emit_ptr_to_i32(ctx, &args[1])?;
    let content = emit_ptr_to_i32(ctx, &args[2])?;
    let host_error = ctx.alloc_local();
    emit_expr(ctx, &args[3])?;
    ctx.emit(Instruction::LocalSet(host_error));
    ctx.emit(Instruction::LocalGet(path));
    ctx.emit(Instruction::LocalGet(content));
    ctx.call_rt("__rt_wasi_component_write_bytes");
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
    let unit = ctx.alloc_local();
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::LocalSet(unit));
    emit_result_enum(ctx, "ok", unit)?;
    ctx.emit(Instruction::Else);
    emit_result_enum(ctx, "err", host_error)?;
    ctx.emit(Instruction::End);
    return Ok(());
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

/// Preserve call evaluation before any logging side effect begins.
fn emit_print_arguments(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<Vec<u32>, String> {
  let mut values = Vec::with_capacity(args.len());
  for arg in args {
    emit_expr(ctx, arg)?;
    let value = ctx.alloc_local();
    ctx.emit(Instruction::LocalSet(value));
    values.push(value);
  }
  Ok(values)
}

/// Evaluate every argument before invoking the embedded host logger.
fn emit_core_print(ctx: &mut WasmGenCtx, name: &str, args: &[Calcit]) -> Result<(), String> {
  let log_idx = resolve_host_import(ctx, "io", "log_line")?;
  let type_infos = args
    .iter()
    .map(|arg| infer_wasm_static_type(ctx, arg).unwrap_or_else(|| DYNAMIC_TYPE.clone()))
    .collect::<Vec<_>>();
  let values = emit_print_arguments(ctx, args)?;
  let visible_values = values.into_iter().zip(type_infos).collect::<Vec<_>>();
  let line = emit_str_spaced_locals(ctx, &visible_values);
  ctx.emit(f64_const(if name == "eprintln" { 2.0 } else { 1.0 }));
  ctx.emit(Instruction::LocalGet(line));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::Call(log_idx));
  ctx.emit(f64_const(0.0)); // Unit
  Ok(())
}

/// Lower Calcit console output through a private Preview 1 adapter.
fn emit_wasi_print(ctx: &mut WasmGenCtx, name: &str, args: &[Calcit]) -> Result<(), String> {
  let fd = if name == "eprintln" { 2 } else { 1 };
  let type_infos = args
    .iter()
    .map(|arg| infer_wasm_static_type(ctx, arg).unwrap_or_else(|| DYNAMIC_TYPE.clone()))
    .collect::<Vec<_>>();
  let values = emit_print_arguments(ctx, args)?;
  for (index, (value, type_info)) in values.into_iter().zip(type_infos).enumerate() {
    if index > 0 {
      emit_wasi_write_literal(ctx, fd, " ")?;
    }
    let ptr = emit_print_string_from_local_with_type(ctx, value, type_info);
    emit_wasi_write_string(ctx, fd, ptr);
  }
  emit_wasi_write_literal(ctx, fd, "\n")?;
  ctx.emit(f64_const(0.0));
  Ok(())
}

fn emit_wasi_component_print(ctx: &mut WasmGenCtx, name: &str, args: &[Calcit]) -> Result<(), String> {
  let interface = if name == "eprintln" { "stderr" } else { "stdout" };
  let type_infos = args
    .iter()
    .map(|arg| infer_wasm_static_type(ctx, arg).unwrap_or_else(|| DYNAMIC_TYPE.clone()))
    .collect::<Vec<_>>();
  let values = emit_print_arguments(ctx, args)?;
  for (index, (value, type_info)) in values.into_iter().zip(type_infos).enumerate() {
    if index > 0 {
      emit_wasi_component_write_literal(ctx, interface, " ")?;
    }
    let ptr = emit_print_string_from_local_with_type(ctx, value, type_info);
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
  if ctx.world == WasiWorld::Http {
    let cli_module = format!("wasi:cli/{interface}@0.2.12");
    let map_missing = |_| {
      "E_WASI_HTTP_CAPABILITY: stdio logging requires a reachable `println` / `eprintln` / `echo` call from the HTTP handler".to_owned()
    };
    let get_stream = resolve_host_import(ctx, &cli_module, &format!("get-{interface}")).map_err(map_missing)?;
    let drop_stream = resolve_host_import(ctx, "wasi:io/streams@0.2.12", "[resource-drop]output-stream").map_err(map_missing)?;
    let stream = ctx.alloc_local_typed(ValType::I32);
    let retptr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::Call(get_stream));
    ctx.emit(Instruction::LocalSet(stream));
    ctx.emit(Instruction::I32Const(16));
    ctx.call_rt("__rt_arena_alloc");
    ctx.emit(Instruction::LocalSet(retptr));
    ctx.emit(Instruction::LocalGet(stream));
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalGet(retptr));
    ctx.call_rt("__rt_wasi_http_write_all");
    ctx.emit(Instruction::LocalGet(stream));
    ctx.emit(Instruction::Call(drop_stream));
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    ctx.emit(Instruction::Unreachable);
    ctx.emit(Instruction::End);
    return Ok(());
  }
  let module = format!("wasi:cli/{interface}@0.3.1");
  let new = resolve_host_import(ctx, &module, "[stream-new-0]write-via-stream")?;
  let write = resolve_host_import(ctx, &module, "[stream-write-0]write-via-stream")?;
  let drop_writer = resolve_host_import(ctx, &module, "[stream-drop-writable-0]write-via-stream")?;
  let read_future = resolve_host_import(ctx, &module, "[future-read-1]write-via-stream")?;
  let drop_future = resolve_host_import(ctx, &module, "[future-drop-readable-1]write-via-stream")?;
  let output = resolve_host_import(ctx, &module, "write-via-stream")?;
  let pair = ctx.alloc_local_typed(ValType::I64);
  let writer = ctx.alloc_local_typed(ValType::I32);
  let future = ctx.alloc_local_typed(ValType::I32);
  let offset = ctx.alloc_local_typed(ValType::I32);
  let remaining = ctx.alloc_local_typed(ValType::I32);
  let count = ctx.alloc_local_typed(ValType::I32);
  let result = ctx.alloc_local_typed(ValType::I32);
  let completion = ctx.alloc_local_typed(ValType::I32);

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
  // The host completes the write only after the writable end closes.
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Const(8));
  ctx.call_rt("__cabi_realloc");
  ctx.emit(Instruction::LocalSet(completion));
  ctx.emit(Instruction::LocalGet(future));
  ctx.emit(Instruction::LocalGet(completion));
  ctx.emit(Instruction::Call(read_future));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32Ne);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(completion));
  ctx.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32Ne);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(completion));
  ctx.emit(Instruction::I32Const(8));
  ctx.call_rt("__cabi_free");
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

/// Numeric ordering cannot recover a Number proof from an erased scalar:
/// nil, Bool and Tag can have exactly the same f64 representation.
fn emit_numeric_cmp(ctx: &mut WasmGenCtx, instr: Instruction<'static>, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "numeric comparison")?;
  let mut mismatch = false;
  for operand in args {
    let annotation = infer_wasm_static_type(ctx, operand)
      .ok_or_else(|| "E_WASM_NIL_TYPE_EVIDENCE: numeric comparison requires concrete operand types before scalar erasure".to_owned())?;
    let proof = annotation.prove_with_bindings(&CalcitTypeAnnotation::Number, &mut TypeBindings::new());
    if proof.is_mismatch() {
      mismatch = true;
    } else if !proof.is_proven() {
      return Err(format!(
        "E_WASM_NIL_TYPE_EVIDENCE: numeric comparison cannot distinguish Number from an open `{}` operand",
        annotation.to_brief_string()
      ));
    }
  }
  if mismatch {
    // Native evaluates every argument, in order, before the proc rejects its
    // types. Retain those effects and errors rather than folding to a trap early.
    for operand in args {
      emit_expr(ctx, operand)?;
      ctx.emit(Instruction::Drop);
    }
    ctx.emit(Instruction::Unreachable);
    Ok(())
  } else {
    emit_cmp(ctx, instr, args)
  }
}

/// Emits an i32 local that is 1 when the f64 value is a live heap value: an
/// integer inside the allocated heap whose header (ptr-8) carries `HEAP_MAGIC`.
/// This is the same test `type-of` uses. The cheap range check comes first and
/// memory is read only after the range and integer checks, so plain numbers
/// (including fractions in the heap address range) never touch it.
#[allow(private_interfaces)]
pub(super) fn emit_is_heap_value(ctx: &mut WasmGenCtx, value: u32) -> u32 {
  let result = ctx.alloc_local_typed(ValType::I32);
  if let Some(&is_heap_idx) = ctx.runtime_fn_index.get("__rt_is_heap_ptr") {
    ctx.emit(Instruction::LocalGet(value));
    ctx.emit(Instruction::Call(is_heap_idx));
    ctx.emit(Instruction::LocalSet(result));
    return result;
  }
  let in_heap = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(result));

  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(f64_const((HEAP_BASE + 8) as f64));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::F64Trunc);
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(in_heap));

  ctx.emit(Instruction::LocalGet(in_heap));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(HEAP_MAGIC));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::End);
  result
}

/// Emits an i32 boolean (`1` = truthy, `0` = falsy) on the stack for a
/// runtime f64 value, inspecting heap tags for boxed `nil` and `bool` values.
#[allow(private_interfaces)]
pub(super) fn emit_value_is_truthy_i32(ctx: &mut WasmGenCtx, value: u32) {
  let nil_tag = ctx.tag_index.get("nil").copied().map(|tag| tag as i32);
  let bool_tag = ctx.tag_index.get("bool").copied().map(|tag| tag as i32);
  if nil_tag.is_none() && bool_tag.is_none() {
    ctx.emit(Instruction::LocalGet(value));
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Ne);
    return;
  }
  let is_heap = emit_is_heap_value(ctx, value);
  ctx.emit(Instruction::LocalGet(is_heap));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  let tag = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalSet(tag));
  let mut opened_if = 0usize;
  if let Some(nil_tag) = nil_tag {
    ctx.emit(Instruction::LocalGet(tag));
    ctx.emit(Instruction::I32Const(nil_tag));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::Else);
    opened_if += 1;
  }
  if let Some(bool_tag) = bool_tag {
    ctx.emit(Instruction::LocalGet(tag));
    ctx.emit(Instruction::I32Const(bool_tag));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
    ctx.emit(Instruction::LocalGet(value));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Ne);
    ctx.emit(Instruction::Else);
    opened_if += 1;
  }
  ctx.emit(Instruction::I32Const(1));
  for _ in 0..opened_if {
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ne);
  ctx.emit(Instruction::End);
}

fn emit_has_heap_tag(ctx: &mut WasmGenCtx, value: u32, tag_name: &str) -> u32 {
  let expected_tag = get_type_tag(ctx, tag_name) as i32;
  let result = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(result));
  let is_heap = emit_is_heap_value(ctx, value);
  ctx.emit(Instruction::LocalGet(is_heap));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(expected_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::End);
  result
}

fn emit_unbox_scalar_if_tag(ctx: &mut WasmGenCtx, local: u32, tag_name: &str, replace_with_zero: bool) {
  if !ctx.tag_index.contains_key(tag_name) {
    return;
  }
  let has_tag = emit_has_heap_tag(ctx, local, tag_name);
  ctx.emit(Instruction::LocalGet(has_tag));
  ctx.begin_block_if();
  if replace_with_zero {
    ctx.emit(f64_const(0.0));
  } else {
    ctx.emit(Instruction::LocalGet(local));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  }
  ctx.emit(Instruction::LocalSet(local));
  ctx.emit(Instruction::End);
}

fn emit_normalize_proven_scalar_pair(ctx: &mut WasmGenCtx, left_expr: &Calcit, right_expr: &Calcit, a: u32, b: u32) {
  let left_ev = infer_wasm_evidence(ctx, left_expr);
  let right_ev = infer_wasm_evidence(ctx, right_expr);
  let left_type = expand_wasm_value_type(left_ev.scalar_origin.as_ref().unwrap_or(&left_ev.annotation));
  let right_type = expand_wasm_value_type(right_ev.scalar_origin.as_ref().unwrap_or(&right_ev.annotation));
  match (proven_scalar_kind(&left_type), proven_scalar_kind(&right_type)) {
    (Some("bool"), Some("bool")) => {
      if !expr_is_guaranteed_unboxed_scalar(ctx, left_expr) {
        emit_unbox_scalar_if_tag(ctx, a, "bool", false);
      }
      if !expr_is_guaranteed_unboxed_scalar(ctx, right_expr) {
        emit_unbox_scalar_if_tag(ctx, b, "bool", false);
      }
    }
    (Some("nil"), Some("nil")) => {
      if !expr_is_guaranteed_unboxed_scalar(ctx, left_expr) {
        emit_unbox_scalar_if_tag(ctx, a, "nil", true);
      }
      if !expr_is_guaranteed_unboxed_scalar(ctx, right_expr) {
        emit_unbox_scalar_if_tag(ctx, b, "nil", true);
      }
    }
    _ => {}
  }
}

fn is_proven_number_pair(ctx: &WasmGenCtx, left_expr: &Calcit, right_expr: &Calcit) -> bool {
  let is_proven_num = |expr: &Calcit| {
    let ev = infer_wasm_evidence(ctx, expr);
    let ty = expand_wasm_value_type(ev.scalar_origin.as_ref().unwrap_or(&ev.annotation));
    proven_scalar_kind(&ty) == Some("number")
  };
  is_proven_num(left_expr) && is_proven_num(right_expr)
}

/// Emits an i32 local that is 1 when the f64 value is a live string heap
/// pointer: a heap value (see `emit_is_heap_value`) carrying the string tag.
fn emit_is_string(ctx: &mut WasmGenCtx, value: u32) -> Result<u32, String> {
  ctx.tag_index.get("string").ok_or("string tag not found")?;
  Ok(emit_has_heap_tag(ctx, value, "string"))
}

/// Emit `=` with string content equality support.
///
/// Semantics:
/// - Fast path: `a == b` on raw f64 representation.
/// - If not equal and both look like heap strings, compare by `__rt_str_compare`.
/// - Otherwise false.
pub(super) fn proven_disjoint_value_kind(annotation: &CalcitTypeAnnotation) -> Option<&'static str> {
  if let Some(scalar) = proven_scalar_kind(annotation) {
    return Some(scalar);
  }
  match annotation {
    CalcitTypeAnnotation::String => Some("string"),
    CalcitTypeAnnotation::Buffer => Some("buffer"),
    CalcitTypeAnnotation::Any => Some("any-box"),
    CalcitTypeAnnotation::Symbol => Some("symbol"),
    CalcitTypeAnnotation::List(_) | CalcitTypeAnnotation::Tuple(_) => Some("list"),
    CalcitTypeAnnotation::Set(_) => Some("set"),
    CalcitTypeAnnotation::Map(_, _) => Some("map"),
    CalcitTypeAnnotation::Struct(_, _) | CalcitTypeAnnotation::StructValue(_) => Some("struct"),
    CalcitTypeAnnotation::StructDef(_) => Some("struct-def"),
    CalcitTypeAnnotation::Enum(_, _) | CalcitTypeAnnotation::EnumValue(_) | CalcitTypeAnnotation::AnonymousEnum => Some("enum"),
    CalcitTypeAnnotation::EnumDef(_) => Some("enum-def"),
    CalcitTypeAnnotation::Custom(value) if matches!(value.as_ref(), Calcit::Tag(tag) if tag.ref_str() == "struct-def") => {
      Some("struct-def")
    }
    CalcitTypeAnnotation::Custom(value) if matches!(value.as_ref(), Calcit::Tag(tag) if tag.ref_str() == "enum-def") => {
      Some("enum-def")
    }
    CalcitTypeAnnotation::Fn(_) | CalcitTypeAnnotation::DynFn => Some("fn"),
    CalcitTypeAnnotation::DynTrait(_) => Some("dyn-trait"),
    CalcitTypeAnnotation::TypeRef(_, _) => {
      if annotation.resolve_to_struct().is_some() {
        Some("struct")
      } else if annotation.resolve_to_enum().is_some() {
        Some("enum")
      } else if annotation.resolve_to_nonoptional_fn().is_some() {
        Some("fn")
      } else {
        None
      }
    }
    _ => None,
  }
}

fn is_nullable_nonzero_equality_pair(left: &CalcitTypeAnnotation, right: &CalcitTypeAnnotation) -> bool {
  match (left, right) {
    (CalcitTypeAnnotation::Nullable(inner, NullableOrigin::Inferred), other)
    | (other, CalcitTypeAnnotation::Nullable(inner, NullableOrigin::Inferred)) => {
      let inner = CalcitTypeAnnotation::expand_schema_alias(inner);
      if !wasm_non_nil_value_is_nonzero(&inner) {
        return false;
      }
      match other {
        CalcitTypeAnnotation::Nil => true,
        CalcitTypeAnnotation::Nullable(other_inner, NullableOrigin::Inferred) => {
          let other_inner = CalcitTypeAnnotation::expand_schema_alias(other_inner);
          wasm_non_nil_value_is_nonzero(&other_inner)
            && proven_disjoint_value_kind(&inner).is_some()
            && proven_disjoint_value_kind(&inner) == proven_disjoint_value_kind(&other_inner)
        }
        _ => proven_disjoint_value_kind(&inner).is_some() && proven_disjoint_value_kind(&inner) == proven_disjoint_value_kind(other),
      }
    }
    _ => false,
  }
}

fn resolve_match_payload_type(binding: &Calcit, sub_pt: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  let sub_ty = expand_wasm_value_type(&sub_pt);
  match binding {
    Calcit::Local(local) => {
      let local_ty = expand_wasm_value_type(&local.type_info);
      if !nil_type_evidence_is_open(&local_ty)
        && !matches!(
          local_ty.as_ref(),
          CalcitTypeAnnotation::Nullable(_, _) | CalcitTypeAnnotation::Custom(_) | CalcitTypeAnnotation::Never
        )
        && matches!(
          sub_ty.as_ref(),
          CalcitTypeAnnotation::TypeRef(..) | CalcitTypeAnnotation::Custom(_) | CalcitTypeAnnotation::Dynamic
        )
      {
        local.type_info.clone()
      } else {
        sub_pt
      }
    }
    _ => sub_pt,
  }
}

pub(super) fn expand_wasm_value_type(ty: &Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  CalcitTypeAnnotation::expand_value_type_aliases(ty)
}

fn extract_fixed_list_slot_types(
  ctx: &WasmGenCtx,
  expr: &Calcit,
  ty: &Arc<CalcitTypeAnnotation>,
) -> Option<Vec<Arc<CalcitTypeAnnotation>>> {
  if let CalcitTypeAnnotation::Tuple(slots) = ty.as_ref() {
    return Some(slots.iter().map(expand_wasm_value_type).collect());
  }
  if let Calcit::List(items) = expr
    && matches!(items.first(), Some(Calcit::Proc(CalcitProc::List)))
  {
    return Some(
      items
        .iter()
        .skip(1)
        .map(|item| {
          let ev = infer_wasm_evidence(ctx, item);
          expand_wasm_value_type(ev.scalar_origin.as_ref().unwrap_or(&ev.annotation))
        })
        .collect(),
    );
  }
  None
}

fn fixed_list_slots_are_statically_disjoint(
  left_slots: &[Arc<CalcitTypeAnnotation>],
  right_slots: &[Arc<CalcitTypeAnnotation>],
  depth: usize,
) -> bool {
  if depth >= 32 {
    return false;
  }
  if left_slots.len() != right_slots.len() {
    return true;
  }
  left_slots.iter().zip(right_slots.iter()).any(|(l, r)| {
    if let (Some(lk), Some(rk)) = (proven_disjoint_value_kind(l), proven_disjoint_value_kind(r))
      && lk != rk
    {
      return true;
    }
    if let (CalcitTypeAnnotation::Tuple(ls), CalcitTypeAnnotation::Tuple(rs)) = (l.as_ref(), r.as_ref()) {
      return fixed_list_slots_are_statically_disjoint(ls, rs, depth + 1);
    }
    false
  })
}

fn is_safe_heap_type_against_open_dynamic(annotation: &Arc<CalcitTypeAnnotation>, depth: usize) -> bool {
  if depth >= 32 {
    return false;
  }
  let expanded = expand_wasm_value_type(annotation);
  match expanded.as_ref() {
    CalcitTypeAnnotation::Never | CalcitTypeAnnotation::String | CalcitTypeAnnotation::Buffer | CalcitTypeAnnotation::Any => true,
    CalcitTypeAnnotation::Struct(_, _)
    | CalcitTypeAnnotation::StructValue(_)
    | CalcitTypeAnnotation::Enum(_, _)
    | CalcitTypeAnnotation::EnumValue(_)
    | CalcitTypeAnnotation::TypeRef(_, _) => {
      if let Some(def) = expanded.resolve_to_struct() {
        if def.generics.is_empty() {
          def
            .field_types
            .iter()
            .all(|field| !sets::set_comparison_can_erase_kind(field, field, depth + 1))
        } else {
          resolved_struct_fields_for_annotation(&expanded)
            .is_some_and(|fields| fields.iter().all(|(_, ty)| is_safe_heap_type_against_open_dynamic(ty, depth + 1)))
        }
      } else if let Some(def) = expanded.resolve_to_enum() {
        if def.generics().is_empty() {
          def.variants().iter().all(|variant| {
            variant
              .payload_types()
              .iter()
              .all(|payload| !sets::set_comparison_can_erase_kind(payload, payload, depth + 1))
          })
        } else {
          let type_args: &[Arc<CalcitTypeAnnotation>] = match expanded.as_ref() {
            CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args) => args.as_slice(),
            _ => &[],
          };
          if type_args.len() != def.generics().len() {
            return false;
          }
          let bindings: TypeBindings = def.generics().iter().cloned().zip(type_args.iter().cloned()).collect();
          def.variants().iter().all(|variant| {
            variant.payload_types().iter().all(|payload| {
              let substituted = expand_wasm_value_type(&payload.substitute_type_vars(&bindings));
              is_safe_heap_type_against_open_dynamic(&substituted, depth + 1)
            })
          })
        }
      } else {
        false
      }
    }
    CalcitTypeAnnotation::List(inner) | CalcitTypeAnnotation::Set(inner) => is_safe_heap_type_against_open_dynamic(inner, depth + 1),
    CalcitTypeAnnotation::Tuple(slots) => slots.iter().all(|slot| is_safe_heap_type_against_open_dynamic(slot, depth + 1)),
    CalcitTypeAnnotation::Map(key, value) => {
      is_safe_heap_type_against_open_dynamic(key, depth + 1) && is_safe_heap_type_against_open_dynamic(value, depth + 1)
    }
    _ => false,
  }
}

fn list_element_types_have_scalar_collision(left: &Arc<CalcitTypeAnnotation>, right: &Arc<CalcitTypeAnnotation>, depth: usize) -> bool {
  use CalcitTypeAnnotation as T;

  if depth >= 32 {
    return false;
  }
  let left = expand_wasm_value_type(left);
  let right = expand_wasm_value_type(right);
  if matches!(left.as_ref(), T::Never) || matches!(right.as_ref(), T::Never) {
    return false;
  }
  if (nil_type_evidence_is_open(&left) && !is_safe_heap_type_against_open_dynamic(&right, depth + 1))
    || (nil_type_evidence_is_open(&right) && !is_safe_heap_type_against_open_dynamic(&left, depth + 1))
  {
    return true;
  }
  if let (Some(l), Some(r)) = (proven_scalar_kind(&left), proven_scalar_kind(&right)) {
    return l != r && !matches!((l, r), ("tag", "nil" | "unit") | ("nil" | "unit", "tag"));
  }
  let cmp = |a: &Arc<T>, b: &Arc<T>| list_element_types_have_scalar_collision(a, b, depth + 1);
  match (left.as_ref(), right.as_ref()) {
    (T::Nullable(inner, NullableOrigin::Inferred), _) => cmp(inner, &right) || cmp(&Arc::new(T::Nil), &right),
    (_, T::Nullable(inner, NullableOrigin::Inferred)) => cmp(&left, inner) || cmp(&left, &Arc::new(T::Nil)),
    (T::List(l), T::List(r)) | (T::Set(l), T::Set(r)) => cmp(l, r),
    (T::Tuple(ls), T::Tuple(rs)) => ls.len() == rs.len() && ls.iter().zip(rs.iter()).any(|(l, r)| cmp(l, r)),
    (T::List(m), T::Tuple(slots)) | (T::Tuple(slots), T::List(m)) => slots.iter().any(|slot| cmp(m, slot)),
    (T::Map(lk, lv), T::Map(rk, rv)) => cmp(lk, rk) || cmp(lv, rv),
    _ => false,
  }
}

fn concrete_types_have_scalar_collision(left: &Arc<CalcitTypeAnnotation>, right: &Arc<CalcitTypeAnnotation>, depth: usize) -> bool {
  let mut visited = HashSet::new();
  concrete_types_have_scalar_collision_inner(left, right, depth, &mut visited)
}

fn concrete_types_have_scalar_collision_inner(
  left: &Arc<CalcitTypeAnnotation>,
  right: &Arc<CalcitTypeAnnotation>,
  depth: usize,
  visited: &mut HashSet<(String, String)>,
) -> bool {
  use CalcitTypeAnnotation as T;

  if depth >= 32 {
    return false;
  }
  let left = expand_wasm_value_type(left);
  let right = expand_wasm_value_type(right);
  if matches!(left.as_ref(), T::Never) || matches!(right.as_ref(), T::Never) {
    return false;
  }
  if let (Some(l), Some(r)) = (proven_scalar_kind(&left), proven_scalar_kind(&right)) {
    return l != r && !matches!((l, r), ("tag", "nil" | "unit") | ("nil" | "unit", "tag"));
  }
  if let (Some(ld), Some(rd)) = (left.resolve_to_enum(), right.resolve_to_enum()) {
    if enum_identity_tag(&ld) != enum_identity_tag(&rd) {
      return false;
    }
    fn extract_args(ty: &CalcitTypeAnnotation) -> &[Arc<CalcitTypeAnnotation>] {
      match ty {
        T::Enum(_, args) | T::TypeRef(_, args) => args.as_slice(),
        _ => &[],
      }
    }
    let key = (
      sets::nominal_cycle_key(&left, ld.definition_ref().map(AsRef::as_ref), ld.name().ref_str()),
      sets::nominal_cycle_key(&right, rd.definition_ref().map(AsRef::as_ref), rd.name().ref_str()),
    );
    if !visited.insert(key.clone()) {
      return false;
    }
    let l_bindings: TypeBindings = ld.generics().iter().cloned().zip(extract_args(&left).iter().cloned()).collect();
    let r_bindings: TypeBindings = rd.generics().iter().cloned().zip(extract_args(&right).iter().cloned()).collect();
    let has_collision = ld.variants().iter().zip(rd.variants().iter()).any(|(lv, rv)| {
      lv.tag == rv.tag
        && lv.payload_types().len() == rv.payload_types().len()
        && lv.payload_types().iter().zip(rv.payload_types().iter()).any(|(lp, rp)| {
          let l_sub = expand_wasm_value_type(&lp.substitute_type_vars(&l_bindings));
          let r_sub = expand_wasm_value_type(&rp.substitute_type_vars(&r_bindings));
          concrete_types_have_scalar_collision_inner(&l_sub, &r_sub, depth + 1, visited)
        })
    });
    visited.remove(&key);
    return has_collision;
  }
  match (left.as_ref(), right.as_ref()) {
    (T::Nullable(inner, NullableOrigin::Inferred), _) => {
      concrete_types_have_scalar_collision_inner(inner, &right, depth + 1, visited)
        || concrete_types_have_scalar_collision_inner(&Arc::new(T::Nil), &right, depth + 1, visited)
    }
    (_, T::Nullable(inner, NullableOrigin::Inferred)) => {
      concrete_types_have_scalar_collision_inner(&left, inner, depth + 1, visited)
        || concrete_types_have_scalar_collision_inner(&left, &Arc::new(T::Nil), depth + 1, visited)
    }
    (T::List(l), T::List(r)) | (T::Set(l), T::Set(r)) => concrete_types_have_scalar_collision_inner(l, r, depth + 1, visited),
    (T::Tuple(ls), T::Tuple(rs)) => {
      ls.len() == rs.len()
        && ls
          .iter()
          .zip(rs.iter())
          .any(|(l, r)| concrete_types_have_scalar_collision_inner(l, r, depth + 1, visited))
    }
    (T::List(m), T::Tuple(slots)) | (T::Tuple(slots), T::List(m)) => slots
      .iter()
      .any(|slot| concrete_types_have_scalar_collision_inner(m, slot, depth + 1, visited)),
    (T::Map(lk, lv), T::Map(rk, rv)) => {
      concrete_types_have_scalar_collision_inner(lk, rk, depth + 1, visited)
        || concrete_types_have_scalar_collision_inner(lv, rv, depth + 1, visited)
    }
    _ => false,
  }
}

/// Resolve a compile-time Number literal, unwrapping immutable locals and
/// identity-preserving `assert-type` / `unsafe-coerce` wrappers.
fn resolve_known_number_literal(ctx: &WasmGenCtx, expr: &Calcit) -> Option<f64> {
  match expr {
    Calcit::Number(n) => Some(*n),
    Calcit::Local(local) => ctx
      .locals
      .get(local.sym.as_ref())
      .and_then(|index| ctx.local_number_literals.get(index))
      .copied(),
    Calcit::Symbol { sym, .. } if ctx.locals.contains_key(sym.as_ref()) => ctx
      .locals
      .get(sym.as_ref())
      .and_then(|index| ctx.local_number_literals.get(index))
      .copied(),
    Calcit::List(items) if items.len() == 3 => match items.first() {
      Some(Calcit::Syntax(CalcitSyntax::AssertType, _)) => resolve_known_number_literal(ctx, &items[1]),
      Some(Calcit::Syntax(CalcitSyntax::UnsafeCoerce, _))
        if matches!(
          CalcitTypeAnnotation::parse_type_annotation_form(&items[2]).as_ref(),
          CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::Number | CalcitTypeAnnotation::Numeric(_)
        ) =>
      {
        resolve_known_number_literal(ctx, &items[1])
      }
      _ => None,
    },
    _ => None,
  }
}

/// A compile-time Number literal that cannot collide with either raw `nil`
/// (`0.0`) or a heap-boxed `nil` pointer in `[HEAP_BASE + 8, u32::MAX]`.
fn is_non_nil_colliding_number_literal(ctx: &WasmGenCtx, expr: &Calcit) -> bool {
  resolve_known_number_literal(ctx, expr)
    .is_some_and(|n| n != 0.0 && (!n.is_finite() || n.fract() != 0.0 || n < (HEAP_BASE + 8) as f64 || n > u32::MAX as f64))
}

enum MapValueShape {
  Fields(std::collections::BTreeMap<String, Arc<CalcitTypeAnnotation>>),
  Uniform(Arc<CalcitTypeAnnotation>),
}

impl MapValueShape {
  fn from_evidence(ev: &WasmTypeEvidence) -> Self {
    let hint_or_ann = ev
      .codec_hint
      .as_ref()
      .map(expand_wasm_value_type)
      .unwrap_or_else(|| expand_wasm_value_type(&ev.annotation));
    if let CalcitTypeAnnotation::Map(k, v) = hint_or_ann.as_ref()
      && matches!(k.as_ref(), CalcitTypeAnnotation::StructValue(_))
      && let Some(def) = v.resolve_to_struct()
    {
      let type_args: &[Arc<CalcitTypeAnnotation>] = match v.as_ref() {
        CalcitTypeAnnotation::Struct(_, args) | CalcitTypeAnnotation::TypeRef(_, args) => args.as_slice(),
        _ => &[],
      };
      let bindings: TypeBindings = def.generics.iter().cloned().zip(type_args.iter().cloned()).collect();
      let unwrap_option = def.name.ref_str() != "__wasm_literal_map__";
      let mut fields = std::collections::BTreeMap::new();
      for (name, field_ty) in def.fields.iter().zip(def.field_types.iter()) {
        let mut resolved = expand_wasm_value_type(&field_ty.substitute_type_vars(&bindings));
        if unwrap_option
          && resolved.resolve_to_enum().is_some_and(|e| e.name().ref_str() == "Option")
          && let CalcitTypeAnnotation::Enum(_, opt_args) | CalcitTypeAnnotation::TypeRef(_, opt_args) = resolved.as_ref()
          && opt_args.len() == 1
        {
          let inner = expand_wasm_value_type(&opt_args[0]);
          if !inner.resolve_to_enum().is_some_and(|e| e.name().ref_str() == "Option") {
            resolved = inner;
          }
        }
        fields.insert(name.ref_str().to_owned(), resolved);
      }
      return Self::Fields(fields);
    }
    match expand_wasm_value_type(&ev.annotation).as_ref() {
      CalcitTypeAnnotation::Map(_, val_ty) => Self::Uniform(val_ty.clone()),
      _ => Self::Uniform(DYNAMIC_TYPE.clone()),
    }
  }
}

fn map_value_types_have_scalar_collision(left_ev: &WasmTypeEvidence, right_ev: &WasmTypeEvidence) -> bool {
  let left = MapValueShape::from_evidence(left_ev);
  let right = MapValueShape::from_evidence(right_ev);
  match (left, right) {
    (MapValueShape::Fields(lm), MapValueShape::Fields(rm)) => lm
      .iter()
      .any(|(k, lv)| rm.get(k).is_some_and(|rv| list_element_types_have_scalar_collision(lv, rv, 0))),
    (MapValueShape::Fields(fm), MapValueShape::Uniform(u)) | (MapValueShape::Uniform(u), MapValueShape::Fields(fm)) => {
      fm.values().any(|fv| list_element_types_have_scalar_collision(fv, &u, 0))
    }
    (MapValueShape::Uniform(lv), MapValueShape::Uniform(rv)) => list_element_types_have_scalar_collision(&lv, &rv, 0),
  }
}

fn collect_list_equality_element_types(
  ev: &WasmTypeEvidence,
  ty: &Arc<CalcitTypeAnnotation>,
) -> Option<Vec<Arc<CalcitTypeAnnotation>>> {
  if let Some(elems) = &ev.list_element_types {
    return Some(elems.clone());
  }
  match expand_wasm_value_type(ty).as_ref() {
    CalcitTypeAnnotation::List(inner) => Some(vec![inner.clone()]),
    CalcitTypeAnnotation::Tuple(slots) => Some(slots.as_ref().clone()),
    _ => None,
  }
}

fn list_evidence_has_scalar_collision(
  left_ev: &WasmTypeEvidence,
  left_type: &Arc<CalcitTypeAnnotation>,
  right_ev: &WasmTypeEvidence,
  right_type: &Arc<CalcitTypeAnnotation>,
) -> bool {
  if left_ev.list_element_types.is_none() && right_ev.list_element_types.is_none() {
    return list_element_types_have_scalar_collision(left_type, right_type, 0);
  }
  let (Some(left_elems), Some(right_elems)) = (
    collect_list_equality_element_types(left_ev, left_type),
    collect_list_equality_element_types(right_ev, right_type),
  ) else {
    return list_element_types_have_scalar_collision(left_type, right_type, 0);
  };
  left_elems
    .iter()
    .any(|l| right_elems.iter().any(|r| list_element_types_have_scalar_collision(l, r, 0)))
}

fn validate_wasm_equality_operands(
  ctx: &mut WasmGenCtx,
  left_expr: &Calcit,
  right_expr: &Calcit,
  op_name: &str,
) -> Result<Option<bool>, String> {
  let left_ev = infer_wasm_evidence(ctx, left_expr);
  let right_ev = infer_wasm_evidence(ctx, right_expr);
  let left_type = expand_wasm_value_type(left_ev.scalar_origin.as_ref().unwrap_or(&left_ev.annotation));
  let right_type = expand_wasm_value_type(right_ev.scalar_origin.as_ref().unwrap_or(&right_ev.annotation));
  if matches!(left_type.as_ref(), CalcitTypeAnnotation::Never) || matches!(right_type.as_ref(), CalcitTypeAnnotation::Never) {
    return Ok(None);
  }

  if let (Some(left_kind), Some(right_kind)) = (proven_disjoint_value_kind(&left_type), proven_disjoint_value_kind(&right_type)) {
    if left_kind != right_kind {
      return Ok(Some(false));
    }
    if left_kind == "set" {
      sets::check_set_union_types(ctx, left_expr, right_expr)?;
    } else if left_kind == "map" {
      maps::check_map_pair_key_types(ctx, left_expr, right_expr, op_name)?;
      if map_value_types_have_scalar_collision(&left_ev, &right_ev) {
        return Err(format!(
          "E_WASM_NIL_TYPE_EVIDENCE: {op_name} requires statically non-colliding Map value types in the current scalar ABI, found `{}` and `{}`",
          left_type.to_brief_string(),
          right_type.to_brief_string()
        ));
      }
    } else if left_kind == "list" {
      if let (Some(left_slots), Some(right_slots)) = (
        extract_fixed_list_slot_types(ctx, left_expr, &left_type),
        extract_fixed_list_slot_types(ctx, right_expr, &right_type),
      ) && fixed_list_slots_are_statically_disjoint(&left_slots, &right_slots, 0)
      {
        return Ok(Some(false));
      }
      if list_evidence_has_scalar_collision(&left_ev, &left_type, &right_ev, &right_type) {
        return Err(format!(
          "E_WASM_NIL_TYPE_EVIDENCE: {op_name} requires statically non-colliding list element types in the current scalar ABI, found `{}` and `{}`",
          left_type.to_brief_string(),
          right_type.to_brief_string()
        ));
      }
    } else if left_kind == "enum" && concrete_types_have_scalar_collision(&left_type, &right_type, 0) {
      return Err(format!(
        "E_WASM_NIL_TYPE_EVIDENCE: {op_name} requires statically non-colliding enum payload types in the current scalar ABI, found `{}` and `{}`",
        left_type.to_brief_string(),
        right_type.to_brief_string()
      ));
    }
    return Ok(None);
  }

  if is_nullable_nonzero_equality_pair(&left_type, &right_type) {
    if list_element_types_have_scalar_collision(&left_type, &right_type, 0) {
      return Err(format!(
        "E_WASM_NIL_TYPE_EVIDENCE: {op_name} requires statically non-colliding list element types in the current scalar ABI, found `{}` and `{}`",
        left_type.to_brief_string(),
        right_type.to_brief_string()
      ));
    }
    return Ok(None);
  }

  let is_nullable_number_pair = |nullable_ty: &CalcitTypeAnnotation, num_ty: &CalcitTypeAnnotation, num_expr: &Calcit| {
    matches!(
      nullable_ty,
      CalcitTypeAnnotation::Nullable(inner, NullableOrigin::Inferred)
        if matches!(
          expand_wasm_value_type(inner).as_ref(),
          CalcitTypeAnnotation::Number | CalcitTypeAnnotation::Numeric(_)
        )
    ) && matches!(num_ty, CalcitTypeAnnotation::Number | CalcitTypeAnnotation::Numeric(_))
      && is_non_nil_colliding_number_literal(ctx, num_expr)
  };
  if is_nullable_number_pair(&left_type, &right_type, right_expr) || is_nullable_number_pair(&right_type, &left_type, left_expr) {
    return Ok(None);
  }

  if is_safe_heap_type_against_open_dynamic(&left_type, 0) || is_safe_heap_type_against_open_dynamic(&right_type, 0) {
    if matches!(left_type.as_ref(), CalcitTypeAnnotation::Set(_)) {
      sets::check_set_union_types(ctx, left_expr, left_expr)?;
    }
    if matches!(right_type.as_ref(), CalcitTypeAnnotation::Set(_)) {
      sets::check_set_union_types(ctx, right_expr, right_expr)?;
    }
    return Ok(None);
  }

  Err(format!(
    "E_WASM_NIL_TYPE_EVIDENCE: {op_name} requires statically non-colliding scalar types in the current scalar ABI, found `{}` and `{}`",
    left_type.to_brief_string(),
    right_type.to_brief_string()
  ))
}

fn emit_numeric_cmp_as_i32(ctx: &mut WasmGenCtx, is_lt: bool, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "numeric comparison")?;
  let mut mismatch = false;
  let mut operand_types = Vec::with_capacity(2);
  for operand in args {
    let annotation = infer_wasm_static_type(ctx, operand)
      .ok_or_else(|| "E_WASM_NIL_TYPE_EVIDENCE: numeric comparison requires concrete operand types before scalar erasure".to_owned())?;
    let proof = annotation.prove_with_bindings(&CalcitTypeAnnotation::Number, &mut TypeBindings::new());
    if proof.is_mismatch() {
      mismatch = true;
    } else if !proof.is_proven() {
      return Err(format!(
        "E_WASM_NIL_TYPE_EVIDENCE: numeric comparison cannot distinguish Number from an open `{}` operand",
        annotation.to_brief_string()
      ));
    }
    operand_types.push(expand_wasm_value_type(&annotation));
  }
  if mismatch {
    for operand in args {
      emit_expr(ctx, operand)?;
      ctx.emit(Instruction::Drop);
    }
    ctx.emit(Instruction::Unreachable);
    return Ok(());
  }
  let left_repr = NativeValRepr::from_type_annotation(&operand_types[0]);
  let right_repr = NativeValRepr::from_type_annotation(&operand_types[1]);
  match (left_repr, right_repr) {
    (NativeValRepr::Int32S, NativeValRepr::Int32S) => {
      emit_expr_as_repr(ctx, &args[0], NativeValRepr::Int32S)?;
      emit_expr_as_repr(ctx, &args[1], NativeValRepr::Int32S)?;
      ctx.emit(if is_lt { Instruction::I32LtS } else { Instruction::I32GtS });
    }
    (NativeValRepr::UInt32, NativeValRepr::UInt32) => {
      emit_expr_as_repr(ctx, &args[0], NativeValRepr::UInt32)?;
      emit_expr_as_repr(ctx, &args[1], NativeValRepr::UInt32)?;
      ctx.emit(if is_lt { Instruction::I32LtU } else { Instruction::I32GtU });
    }
    (NativeValRepr::Int64S, NativeValRepr::Int64S) => {
      emit_expr_as_repr(ctx, &args[0], NativeValRepr::Int64S)?;
      emit_expr_as_repr(ctx, &args[1], NativeValRepr::Int64S)?;
      ctx.emit(if is_lt { Instruction::I64LtS } else { Instruction::I64GtS });
    }
    (NativeValRepr::UInt64, NativeValRepr::UInt64) => {
      emit_expr_as_repr(ctx, &args[0], NativeValRepr::UInt64)?;
      emit_expr_as_repr(ctx, &args[1], NativeValRepr::UInt64)?;
      ctx.emit(if is_lt { Instruction::I64LtU } else { Instruction::I64GtU });
    }
    _ => {
      emit_expr(ctx, &args[0])?;
      emit_expr(ctx, &args[1])?;
      ctx.emit(if is_lt { Instruction::F64Lt } else { Instruction::F64Gt });
    }
  }
  Ok(())
}

fn emit_identical_as_i32(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "identical?")?;
  let left_ty = infer_wasm_static_type(ctx, &args[0]).map(|ty| expand_wasm_value_type(&ty));
  let right_ty = infer_wasm_static_type(ctx, &args[1]).map(|ty| expand_wasm_value_type(&ty));
  if let (Some(l), Some(r)) = (left_ty.as_ref(), right_ty.as_ref()) {
    if matches!(
      (l.as_ref(), r.as_ref()),
      (CalcitTypeAnnotation::String, CalcitTypeAnnotation::String)
    ) {
      let a_ptr = emit_ptr_to_i32(ctx, &args[0])?;
      let b_ptr = emit_ptr_to_i32(ctx, &args[1])?;
      ctx.emit(Instruction::LocalGet(a_ptr));
      ctx.emit(Instruction::LocalGet(b_ptr));
      ctx.emit(Instruction::I32Eq);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
      ctx.emit(Instruction::I32Const(1));
      ctx.emit(Instruction::Else);
      ctx.emit(Instruction::LocalGet(a_ptr));
      ctx.emit(Instruction::LocalGet(b_ptr));
      ctx.call_rt("__rt_str_compare");
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Eq);
      ctx.emit(Instruction::End);
      return Ok(());
    }
    let lr = NativeValRepr::from_type_annotation(l);
    let rr = NativeValRepr::from_type_annotation(r);
    if lr == rr
      && lr.val_type() == ValType::I32
      && expr_is_guaranteed_unboxed_scalar(ctx, &args[0])
      && expr_is_guaranteed_unboxed_scalar(ctx, &args[1])
    {
      emit_expr_as_repr(ctx, &args[0], lr)?;
      emit_expr_as_repr(ctx, &args[1], rr)?;
      ctx.emit(Instruction::I32Eq);
      return Ok(());
    }
    if lr == rr && lr.val_type() == ValType::I64 {
      emit_expr_as_repr(ctx, &args[0], lr)?;
      emit_expr_as_repr(ctx, &args[1], rr)?;
      ctx.emit(Instruction::I64Eq);
      return Ok(());
    }
  }
  let left_non_string = left_ty
    .as_deref()
    .and_then(proven_disjoint_value_kind)
    .is_some_and(|kind| kind != "string");
  let right_non_string = right_ty
    .as_deref()
    .and_then(proven_disjoint_value_kind)
    .is_some_and(|kind| kind != "string");
  if left_non_string || right_non_string {
    emit_expr(ctx, &args[0])?;
    emit_expr(ctx, &args[1])?;
    ctx.emit(Instruction::F64Eq);
    return Ok(());
  }

  let a_val = ctx.alloc_local_typed(ValType::F64);
  let b_val = ctx.alloc_local_typed(ValType::F64);
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(a_val));
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(b_val));
  ctx.emit(Instruction::LocalGet(a_val));
  ctx.emit(Instruction::LocalGet(b_val));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::Else);
  let a_is_str = emit_is_string(ctx, a_val)?;
  let b_is_str = emit_is_string(ctx, b_val)?;
  ctx.emit(Instruction::LocalGet(a_is_str));
  ctx.emit(Instruction::LocalGet(b_is_str));
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  ctx.emit(Instruction::LocalGet(a_val));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalGet(b_val));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.call_rt("__rt_str_compare");
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

/// Evaluate `=` on two operands, leaving `i32` (`1` = equal, `0` = not equal)
/// without `HEAP_MAGIC` probes for statically proven closed types.
fn emit_equals_as_i32(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err(format!("= expects 2 args, got {}", args.len()));
  }

  if let Some(false) = validate_wasm_equality_operands(ctx, &args[0], &args[1], "=")? {
    emit_expr(ctx, &args[0])?;
    ctx.emit(Instruction::Drop);
    emit_expr(ctx, &args[1])?;
    ctx.emit(Instruction::Drop);
    ctx.emit(Instruction::I32Const(0));
    return Ok(());
  }

  let left_buffer = buffer_literal_bytes(&args[0])?;
  let right_buffer = buffer_literal_bytes(&args[1])?;
  match (left_buffer, right_buffer) {
    (Some(left), Some(right)) => {
      ctx.emit(Instruction::I32Const(i32::from(left == right)));
      return Ok(());
    }
    (Some(bytes), None) => {
      emit_buffer_literal_equals_i32(ctx, &args[1], &bytes)?;
      return Ok(());
    }
    (None, Some(bytes)) => {
      emit_buffer_literal_equals_i32(ctx, &args[0], &bytes)?;
      return Ok(());
    }
    (None, None) => {}
  }

  let left_ty = infer_wasm_static_type(ctx, &args[0]).map(|ty| expand_wasm_value_type(&ty));
  let right_ty = infer_wasm_static_type(ctx, &args[1]).map(|ty| expand_wasm_value_type(&ty));
  if let (Some(l), Some(r)) = (left_ty.as_ref(), right_ty.as_ref()) {
    match (l.as_ref(), r.as_ref()) {
      (CalcitTypeAnnotation::String, CalcitTypeAnnotation::String) | (CalcitTypeAnnotation::Buffer, CalcitTypeAnnotation::Buffer) => {
        let a_ptr = emit_ptr_to_i32(ctx, &args[0])?;
        let b_ptr = emit_ptr_to_i32(ctx, &args[1])?;
        ctx.emit(Instruction::LocalGet(a_ptr));
        ctx.emit(Instruction::LocalGet(b_ptr));
        ctx.emit(Instruction::I32Eq);
        ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
        ctx.emit(Instruction::I32Const(1));
        ctx.emit(Instruction::Else);
        ctx.emit(Instruction::LocalGet(a_ptr));
        ctx.emit(Instruction::LocalGet(b_ptr));
        ctx.call_rt("__rt_str_compare");
        ctx.emit(f64_const(0.0));
        ctx.emit(Instruction::F64Eq);
        ctx.emit(Instruction::End);
        return Ok(());
      }
      (CalcitTypeAnnotation::Bool, CalcitTypeAnnotation::Bool) => {
        emit_expr_as_repr(ctx, &args[0], NativeValRepr::BoolI32)?;
        emit_expr_as_repr(ctx, &args[1], NativeValRepr::BoolI32)?;
        ctx.emit(Instruction::I32Eq);
        return Ok(());
      }
      (CalcitTypeAnnotation::Tag, CalcitTypeAnnotation::Tag) => {
        emit_expr_as_repr(ctx, &args[0], NativeValRepr::TagI32)?;
        emit_expr_as_repr(ctx, &args[1], NativeValRepr::TagI32)?;
        ctx.emit(Instruction::I32Eq);
        return Ok(());
      }
      (CalcitTypeAnnotation::Unit, CalcitTypeAnnotation::Unit) => {
        emit_expr(ctx, &args[0])?;
        ctx.emit(Instruction::Drop);
        emit_expr(ctx, &args[1])?;
        ctx.emit(Instruction::Drop);
        ctx.emit(Instruction::I32Const(1));
        return Ok(());
      }
      (CalcitTypeAnnotation::Nil, CalcitTypeAnnotation::Nil)
        if expr_is_guaranteed_unboxed_scalar(ctx, &args[0]) && expr_is_guaranteed_unboxed_scalar(ctx, &args[1]) =>
      {
        emit_expr(ctx, &args[0])?;
        ctx.emit(Instruction::Drop);
        emit_expr(ctx, &args[1])?;
        ctx.emit(Instruction::Drop);
        ctx.emit(Instruction::I32Const(1));
        return Ok(());
      }
      _ => {
        let lr = NativeValRepr::from_type_annotation(l);
        let rr = NativeValRepr::from_type_annotation(r);
        match (lr, rr) {
          (NativeValRepr::Int32S, NativeValRepr::Int32S) | (NativeValRepr::UInt32, NativeValRepr::UInt32) => {
            emit_expr_as_repr(ctx, &args[0], lr)?;
            emit_expr_as_repr(ctx, &args[1], rr)?;
            ctx.emit(Instruction::I32Eq);
            return Ok(());
          }
          (NativeValRepr::Int64S, NativeValRepr::Int64S) | (NativeValRepr::UInt64, NativeValRepr::UInt64) => {
            emit_expr_as_repr(ctx, &args[0], lr)?;
            emit_expr_as_repr(ctx, &args[1], rr)?;
            ctx.emit(Instruction::I64Eq);
            return Ok(());
          }
          _ => {}
        }
      }
    }
  }

  let a = ctx.alloc_local();
  let b = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(a));
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(b));

  if is_proven_number_pair(ctx, &args[0], &args[1]) {
    ctx.emit(Instruction::LocalGet(a));
    ctx.emit(Instruction::LocalGet(b));
    emit_f64_value_eq(ctx);
    return Ok(());
  }

  emit_normalize_proven_scalar_pair(ctx, &args[0], &args[1], a, b);

  ctx.emit(Instruction::LocalGet(a));
  ctx.emit(Instruction::LocalGet(b));
  ctx.call_rt("__rt_value_equal");
  Ok(())
}

fn emit_equals(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  emit_equals_as_i32(ctx, args)?;
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

fn emit_variadic_equals(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() == 1 {
    emit_expr(ctx, &args[0])?;
    ctx.emit(Instruction::Drop);
    ctx.emit(f64_const(1.0));
    return Ok(());
  }
  if args.len() <= 2 {
    return emit_equals(ctx, args);
  }
  let mut any_statically_false = false;
  for pair in args.windows(2) {
    if let Some(false) = validate_wasm_equality_operands(ctx, &pair[0], &pair[1], "=")? {
      any_statically_false = true;
    }
  }
  let mut locals = Vec::with_capacity(args.len());
  for arg in args {
    let local = ctx.alloc_local();
    emit_expr(ctx, arg)?;
    ctx.emit(Instruction::LocalSet(local));
    locals.push(local);
  }
  if any_statically_false {
    ctx.emit(f64_const(0.0));
    return Ok(());
  }
  let all_eq = ctx.alloc_i32(1);
  let pair_eq = ctx.alloc_local_typed(ValType::I32);
  for idx in 0..locals.len().saturating_sub(1) {
    if is_proven_number_pair(ctx, &args[idx], &args[idx + 1]) {
      ctx.emit(Instruction::LocalGet(locals[idx]));
      ctx.emit(Instruction::LocalGet(locals[idx + 1]));
      emit_f64_value_eq(ctx);
      ctx.emit(Instruction::LocalSet(pair_eq));
    } else {
      emit_normalize_proven_scalar_pair(ctx, &args[idx], &args[idx + 1], locals[idx], locals[idx + 1]);
      ctx.emit(Instruction::LocalGet(locals[idx]));
      ctx.emit(Instruction::LocalGet(locals[idx + 1]));
      ctx.call_rt("__rt_value_equal");
      ctx.emit(Instruction::LocalSet(pair_eq));
    }
    ctx.emit(Instruction::LocalGet(all_eq));
    ctx.emit(Instruction::LocalGet(pair_eq));
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::LocalSet(all_eq));
  }
  ctx.emit(Instruction::LocalGet(all_eq));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

fn emit_not_equals(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "not=")?;
  emit_equals(ctx, args)?;
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::F64ConvertI32U);
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

fn emit_buffer_literal_equals_i32(ctx: &mut WasmGenCtx, value: &Calcit, bytes: &[u8]) -> Result<(), String> {
  let result = ctx.alloc_i32(0);
  let known_buffer =
    infer_wasm_static_type(ctx, value).is_some_and(|ty| matches!(expand_wasm_value_type(&ty).as_ref(), CalcitTypeAnnotation::Buffer));
  let ptr = if known_buffer {
    emit_ptr_to_i32(ctx, value)?
  } else {
    emit_expr(ctx, value)?;
    let val_local = ctx.alloc_local();
    ctx.emit(Instruction::LocalSet(val_local));
    let ptr = ctx.alloc_local_typed(ValType::I32);
    let is_heap = emit_is_heap_value(ctx, val_local);
    ctx.emit(Instruction::LocalGet(is_heap));
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(val_local));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(ptr));
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(Instruction::I32Const(4));
    ctx.emit(Instruction::I32Sub);
    ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
    ctx.emit(Instruction::I32Const(
      *ctx.tag_index.get("buffer").ok_or("buffer tag not found")? as i32
    ));
    ctx.emit(Instruction::I32Eq);
    ctx.begin_block_if();
    ptr
  };
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(f64_const(bytes.len() as f64));
  ctx.emit(Instruction::F64Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(result));
  for (index, byte) in bytes.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(Instruction::I32Load8U(mem_arg_byte(8 + index as u64)));
    ctx.emit(Instruction::I32Const(i32::from(*byte)));
    ctx.emit(Instruction::I32Ne);
    ctx.begin_block_if();
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::LocalSet(result));
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::End);
  if !known_buffer {
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

#[allow(dead_code)]
fn emit_buffer_literal_equals(ctx: &mut WasmGenCtx, value: &Calcit, bytes: &[u8]) -> Result<(), String> {
  emit_buffer_literal_equals_i32(ctx, value, bytes)?;
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Emit a binary bitwise operation: convert both args to i32, apply op, convert back to f64.
fn emit_bitwise_binary(ctx: &mut WasmGenCtx, instr: Instruction<'static>, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err(format!("{instr:?} expects 2 args, got {}", args.len()));
  }
  emit_i32_operand(ctx, &args[0])?;
  emit_i32_operand(ctx, &args[1])?;
  ctx.emit(instr);
  ctx.emit(Instruction::F64ConvertI32S);
  Ok(())
}

/// Bitwise operands are integers within i32 (docs/data/number.md): a fractional or NaN
/// operand traps here, and `i32.trunc_f64_s` traps on infinities and values outside i32.
fn emit_i32_operand(ctx: &mut WasmGenCtx, arg: &Calcit) -> Result<(), String> {
  let value = ctx.alloc_local();
  emit_expr(ctx, arg)?;
  ctx.emit(Instruction::LocalSet(value));
  let mut checks = vec![
    Instruction::LocalGet(value),
    Instruction::F64Trunc,
    Instruction::LocalGet(value),
    Instruction::F64Ne,
  ];
  component_trap_if(&mut checks);
  for instruction in checks {
    ctx.emit(instruction);
  }
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64S);
  Ok(())
}

fn static_predicate_condition_outcome(ctx: &WasmGenCtx, condition: &Calcit, allow_bool_leaf: bool) -> Option<bool> {
  static_predicate_condition_outcome_in_scope(
    ctx,
    condition,
    allow_bool_leaf,
    &ctx.local_types,
    &ctx.lambda_locals,
    &HashMap::new(),
  )
}

fn static_predicate_condition_outcome_in_scope(
  ctx: &WasmGenCtx,
  condition: &Calcit,
  allow_bool_leaf: bool,
  local_types: &HashMap<String, WasmTypeEvidence>,
  closures: &HashMap<String, Arc<InlineClosure>>,
  bound_bools: &HashMap<String, Option<bool>>,
) -> Option<bool> {
  if allow_bool_leaf {
    match condition {
      Calcit::Bool(b) => return Some(*b),
      Calcit::Local(local) => return bound_bools.get(local.sym.as_ref()).copied().flatten(),
      Calcit::Symbol { sym, .. } => return bound_bools.get(sym.as_ref()).copied().flatten(),
      _ => {}
    }
  }
  let Calcit::List(items) = condition else {
    return None;
  };
  let head = items.first()?;
  if matches!(head, Calcit::Syntax(CalcitSyntax::CoreLet, _))
    && items.len() == 3
    && let Some(Calcit::List(pair)) = items.get(1)
  {
    if pair.is_empty() {
      return static_predicate_condition_outcome_in_scope(ctx, &items[2], allow_bool_leaf, local_types, closures, bound_bools);
    }
    if pair.len() == 2 {
      let bound_sym = match &pair[0] {
        Calcit::Local(local) => local.sym.as_ref(),
        Calcit::Symbol { sym, .. } => sym.as_ref(),
        _ => return None,
      };
      let rhs_evidence = infer_wasm_evidence_in_scope(ctx, &pair[1], local_types, closures, &mut HashSet::new())
        .unwrap_or_else(|| WasmTypeEvidence::contract(DYNAMIC_TYPE.clone()));
      let rhs_bool = static_predicate_condition_outcome_in_scope(ctx, &pair[1], true, local_types, closures, bound_bools);
      let mut body_types = local_types.clone();
      body_types.insert(bound_sym.to_owned(), rhs_evidence);
      let mut body_closures = closures.clone();
      if let Some(closure) = resolve_inline_closure_in_scope(ctx, &pair[1], local_types, closures) {
        body_closures.insert(bound_sym.to_owned(), closure);
      } else {
        body_closures.remove(bound_sym);
      }
      let mut body_bools = bound_bools.clone();
      body_bools.insert(bound_sym.to_owned(), rhs_bool);
      return static_predicate_condition_outcome_in_scope(ctx, &items[2], allow_bool_leaf, &body_types, &body_closures, &body_bools);
    }
    return None;
  }
  if matches!(head, Calcit::Syntax(CalcitSyntax::If, _)) && (3..=4).contains(&items.len()) {
    let cond_val = static_predicate_condition_outcome_in_scope(ctx, items.get(1)?, true, local_types, closures, bound_bools)?;
    return if cond_val {
      static_predicate_condition_outcome_in_scope(ctx, items.get(2)?, true, local_types, closures, bound_bools)
    } else if let Some(alt) = items.get(3) {
      static_predicate_condition_outcome_in_scope(ctx, alt, true, local_types, closures, bound_bools)
    } else {
      Some(false)
    };
  }
  if items.len() == 3 {
    let eq_kind = match head {
      Calcit::Proc(CalcitProc::NativeEquals) => Some(true),
      Calcit::Import(import) if import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS => match import.def.as_ref() {
        "=" | "&=" => Some(true),
        "not=" => Some(false),
        _ => None,
      },
      Calcit::Symbol { sym, info, .. }
        if info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
          && !local_types.contains_key(sym.as_ref())
          && !ctx.locals.contains_key(sym.as_ref())
          && !closures.contains_key(sym.as_ref()) =>
      {
        match sym.as_ref() {
          "=" | "&=" => Some(true),
          "not=" => Some(false),
          _ => None,
        }
      }
      _ => None,
    }?;
    let left_ev = infer_wasm_evidence_in_scope(ctx, items.get(1)?, local_types, closures, &mut HashSet::new())?;
    let right_ev = infer_wasm_evidence_in_scope(ctx, items.get(2)?, local_types, closures, &mut HashSet::new())?;
    let left_ty = expand_wasm_value_type(left_ev.scalar_origin.as_ref().unwrap_or(&left_ev.annotation));
    let right_ty = expand_wasm_value_type(right_ev.scalar_origin.as_ref().unwrap_or(&right_ev.annotation));
    let left_kind = proven_disjoint_value_kind(&left_ty)?;
    let right_kind = proven_disjoint_value_kind(&right_ty)?;
    if left_kind != right_kind {
      return Some(!eq_kind);
    }
    if matches!(left_kind, "nil" | "unit") {
      return Some(eq_kind);
    }
    return None;
  }
  if items.len() != 2 {
    return None;
  }
  let proc = match head {
    Calcit::Proc(p) => Some(*p),
    Calcit::Import(import) if import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS => import.def.parse::<CalcitProc>().ok(),
    Calcit::Symbol { sym, info, .. }
      if info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
        && !local_types.contains_key(sym.as_ref())
        && !ctx.locals.contains_key(sym.as_ref())
        && !closures.contains_key(sym.as_ref()) =>
    {
      sym.parse::<CalcitProc>().ok()
    }
    _ => None,
  }?;
  if proc == CalcitProc::Not {
    return static_predicate_condition_outcome_in_scope(ctx, items.get(1)?, true, local_types, closures, bound_bools).map(|b| !b);
  }
  let operand = items.get(1)?;
  if matches!(proc, CalcitProc::FnQuestion)
    && (resolve_inline_closure_in_scope(ctx, operand, local_types, closures).is_some()
      || resolve_known_function_value(ctx, operand).is_some())
  {
    return Some(true);
  }
  let ty = infer_wasm_evidence_in_scope(ctx, operand, local_types, closures, &mut HashSet::new())
    .map(|ev| expand_wasm_value_type(&ev.annotation))?;
  if nil_type_evidence_is_open(&ty)
    || matches!(ty.as_ref(), CalcitTypeAnnotation::Nullable(_, _) | CalcitTypeAnnotation::Custom(_))
    || ty.resolve_to_union().is_some()
  {
    return None;
  }
  match proc {
    CalcitProc::TagQuestion => Some(matches!(ty.as_ref(), CalcitTypeAnnotation::Tag)),
    CalcitProc::NilQuestion => Some(matches!(ty.as_ref(), CalcitTypeAnnotation::Nil)),
    CalcitProc::ListQuestion => Some(matches!(
      ty.as_ref(),
      CalcitTypeAnnotation::List(_) | CalcitTypeAnnotation::Tuple(_)
    )),
    CalcitProc::MapQuestion => Some(matches!(ty.as_ref(), CalcitTypeAnnotation::Map(_, _))),
    CalcitProc::SetQuestion => Some(matches!(ty.as_ref(), CalcitTypeAnnotation::Set(_))),
    CalcitProc::StringQuestion => Some(matches!(ty.as_ref(), CalcitTypeAnnotation::String)),
    CalcitProc::NumberQuestion => Some(matches!(
      ty.as_ref(),
      CalcitTypeAnnotation::Number | CalcitTypeAnnotation::Numeric(_)
    )),
    CalcitProc::BoolQuestion => Some(matches!(ty.as_ref(), CalcitTypeAnnotation::Bool)),
    CalcitProc::SymbolQuestion => Some(matches!(ty.as_ref(), CalcitTypeAnnotation::Symbol)),
    CalcitProc::FnQuestion => {
      Some(matches!(ty.as_ref(), CalcitTypeAnnotation::Fn(_) | CalcitTypeAnnotation::DynFn) || ty.resolve_to_fn().is_some())
    }
    CalcitProc::StructQuestion => Some(
      matches!(
        ty.as_ref(),
        CalcitTypeAnnotation::Struct(_, _) | CalcitTypeAnnotation::StructValue(_)
      ) || ty.resolve_to_struct().is_some(),
    ),
    CalcitProc::EnumQuestion => Some(
      matches!(
        ty.as_ref(),
        CalcitTypeAnnotation::Enum(_, _) | CalcitTypeAnnotation::EnumValue(_) | CalcitTypeAnnotation::AnonymousEnum
      ) || ty.resolve_to_enum().is_some(),
    ),
    _ => None,
  }
}

/// Emit WASM for `if` expression returning `F64`.
fn emit_if(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  emit_if_as_repr(ctx, args, NativeValRepr::F64)
}

/// Emit WASM for `if` expression returning `dst_repr`.
fn emit_if_as_repr(ctx: &mut WasmGenCtx, args: &[Calcit], dst_repr: NativeValRepr) -> Result<(), String> {
  if args.len() < 2 || args.len() > 3 {
    return Err(format!("if expects 2-3 args, got {}", args.len()));
  }
  if let Some(taken) = static_predicate_condition_outcome(ctx, &args[0], false) {
    emit_if_condition(ctx, &args[0])?;
    ctx.emit(Instruction::Drop);
    if taken {
      return emit_expr_as_repr(ctx, &args[1], dst_repr);
    } else if args.len() == 3 {
      return emit_expr_as_repr(ctx, &args[2], dst_repr);
    } else {
      emit_unit_const_repr(ctx, dst_repr);
      return Ok(());
    }
  }
  emit_if_condition(ctx, &args[0])?;
  let after_cond_resets = ctx.reset_initialized_atoms.clone();
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(dst_repr.val_type())));
  ctx.block_depth += 1;
  emit_expr_as_repr(ctx, &args[1], dst_repr)?;
  let then_resets = std::mem::replace(&mut ctx.reset_initialized_atoms, after_cond_resets);
  ctx.emit(Instruction::Else);
  if args.len() == 3 {
    emit_expr_as_repr(ctx, &args[2], dst_repr)?;
  } else {
    emit_unit_const_repr(ctx, dst_repr);
  }
  ctx.reset_initialized_atoms.retain(|k| then_resets.contains(k));
  ctx.block_depth -= 1;
  ctx.emit(Instruction::End);
  Ok(())
}

/// Leave an `if` condition's truthiness on the stack as i32.
fn emit_if_condition(ctx: &mut WasmGenCtx, condition: &Calcit) -> Result<(), String> {
  let annotation = infer_wasm_static_type(ctx, condition)
    .ok_or_else(|| "E_NON_BOOL_CONDITION: if requires a proven Bool condition type in WASM".to_owned())?;
  let expanded = expand_wasm_value_type(&annotation);
  match expanded.as_ref() {
    CalcitTypeAnnotation::Never => {
      emit_expr(ctx, condition)?;
      ctx.emit(Instruction::Drop);
      ctx.emit(Instruction::I32Const(0));
      Ok(())
    }
    CalcitTypeAnnotation::Bool => emit_expr_as_repr(ctx, condition, NativeValRepr::BoolI32),
    other => Err(format!(
      "E_NON_BOOL_CONDITION: if condition expects Bool in WASM, got `{}`",
      other.to_brief_string()
    )),
  }
}

/// A local argument whose attached impls are chosen at runtime.
fn selected_local<'a>(ctx: &WasmGenCtx, arg: &'a Calcit) -> Option<(&'a str, u32)> {
  let name = match arg {
    Calcit::Local(local) => local.sym.as_ref(),
    Calcit::Symbol { sym, .. } => sym.as_ref(),
    _ => return None,
  };
  let index = *ctx.locals.get(name)?;
  ctx.impl_selectors.contains_key(&index).then_some((name, index))
}

/// Evidence that construction or transport fixed a nominal value's impls.
fn exact_nominal_evidence(ctx: &WasmGenCtx, expr: &Calcit) -> Option<WasmTypeEvidence> {
  infer_wasm_value_evidence(ctx, expr).filter(|evidence| evidence.exact_impls && is_nominal_value_annotation(&evidence.annotation))
}

/// The finite actual impl identities of a value whose `if` branches disagree.
/// A data-shape join would discard them; `None` keeps ordinary lowering.
fn impl_alternatives(ctx: &WasmGenCtx, expr: &Calcit) -> Option<Vec<WasmTypeEvidence>> {
  if let Some((_, index)) = selected_local(ctx, expr) {
    return Some(ctx.impl_selectors[&index].alternatives.clone());
  }
  let Calcit::List(items) = expr else {
    return None;
  };
  if items.len() != 4 || !matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::If, _))) {
    return None;
  }
  let mut alternatives: Vec<WasmTypeEvidence> = vec![];
  for branch in [&items[2], &items[3]] {
    if expression_definitely_diverges(branch) {
      continue;
    }
    let identities = impl_alternatives(ctx, branch).or_else(|| exact_nominal_evidence(ctx, branch).map(|evidence| vec![evidence]))?;
    for identity in identities {
      if !alternatives.contains(&identity) {
        alternatives.push(identity);
      }
    }
  }
  if alternatives.len() > 1
    && let Some(first_ref) = alternatives.first().and_then(|first| nominal_type_def_ref(&first.annotation))
    && !ctx.non_coherent_nominal_defs.contains(&first_ref)
    && alternatives
      .iter()
      .all(|alt| nominal_type_def_ref(&alt.annotation).as_deref() == Some(first_ref.as_str()))
  {
    return None;
  }
  (alternatives.len() > 1 && alternatives.len() <= MAX_IMPL_ALTERNATIVES).then_some(alternatives)
}

/// Emit a value from `impl_alternatives`, recording its identity in `selector`.
fn emit_selecting_value(ctx: &mut WasmGenCtx, expr: &Calcit, selector: &ImplSelector) -> Result<(), String> {
  if let Some((_, index)) = selected_local(ctx, expr) {
    let source = ctx.impl_selectors[&index].clone();
    ctx.emit(Instruction::LocalGet(index));
    for (from, identity) in source.alternatives.iter().enumerate() {
      let to = selector
        .index_of(identity)
        .ok_or("E_WASM_TRAIT_TYPE_EVIDENCE: selected implementation is missing from its join")?;
      ctx.emit(Instruction::LocalGet(source.local));
      ctx.emit(Instruction::I32Const(from as i32));
      ctx.emit(Instruction::I32Eq);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
      ctx.emit(Instruction::I32Const(to as i32));
      ctx.emit(Instruction::LocalSet(selector.local));
      ctx.emit(Instruction::End);
    }
    return Ok(());
  }
  let Calcit::List(items) = expr else {
    return Err("E_WASM_TRAIT_TYPE_EVIDENCE: selected implementation requires a branch join".into());
  };
  emit_if_condition(ctx, &items[1])?;
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.block_depth += 1;
  let result = emit_selecting_branch(ctx, &items[2], selector).and_then(|()| {
    ctx.emit(Instruction::Else);
    emit_selecting_branch(ctx, &items[3], selector)
  });
  ctx.block_depth -= 1;
  result?;
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_selecting_branch(ctx: &mut WasmGenCtx, branch: &Calcit, selector: &ImplSelector) -> Result<(), String> {
  if expression_definitely_diverges(branch) {
    return emit_expr(ctx, branch);
  }
  if impl_alternatives(ctx, branch).is_some() {
    return emit_selecting_value(ctx, branch, selector);
  }
  let position = exact_nominal_evidence(ctx, branch)
    .and_then(|evidence| selector.index_of(&evidence))
    .ok_or("E_WASM_TRAIT_TYPE_EVIDENCE: branch implementation identity changed while lowering its join")?;
  emit_expr(ctx, branch)?;
  ctx.emit(Instruction::I32Const(position as i32));
  ctx.emit(Instruction::LocalSet(selector.local));
  Ok(())
}

/// Lower a call reading a selected local once per actual identity, so method
/// selection and call-site specialization see exact evidence in each branch.
/// Only the matching branch runs, keeping evaluation order and effects.
fn emit_call_per_selected_impl(
  ctx: &mut WasmGenCtx,
  xs: &crate::wasm::host::calcit::CalcitList,
  name: &str,
  index: u32,
) -> Result<(), String> {
  let selector = ctx.impl_selectors.remove(&index).expect("selected local has a selector");
  let old_type = ctx.local_types.get(name).cloned();
  let old_depth = ctx.block_depth;
  let last = selector.alternatives.len() - 1;
  let result = (|| {
    for (position, identity) in selector.alternatives.iter().enumerate() {
      if position < last {
        ctx.emit(Instruction::LocalGet(selector.local));
        ctx.emit(Instruction::I32Const(position as i32));
        ctx.emit(Instruction::I32Eq);
        ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
        ctx.block_depth += 1;
      }
      ctx.local_types.insert(name.to_owned(), identity.clone());
      emit_call_expr(ctx, xs)?;
      if position < last {
        ctx.emit(Instruction::Else);
      }
    }
    for _ in 0..last {
      ctx.emit(Instruction::End);
    }
    Ok(())
  })();
  ctx.block_depth = old_depth;
  match old_type {
    Some(evidence) => ctx.local_types.insert(name.to_owned(), evidence),
    None => ctx.local_types.remove(name),
  };
  ctx.impl_selectors.insert(index, selector);
  result
}

/// Bind call arguments up to a branch join in order, so the join becomes a
/// selected local without moving any argument's evaluation. Imported values
/// can inline effectful initializers and must be bound too. Other atoms stay
/// in place because some heads read constructor/trait metadata syntactically.
fn emit_call_binding_impl_join(ctx: &mut WasmGenCtx, xs: &crate::wasm::host::calcit::CalcitList, join: usize) -> Result<(), String> {
  let ns: Arc<str> = Arc::from(ctx.source_ns.as_str());
  let mut call = vec![xs[0].clone()];
  let mut pairs = vec![];
  for (position, arg) in xs.drop_left().iter().enumerate() {
    let metadata = position == 0
      && matches!(
        xs[0],
        Calcit::Proc(CalcitProc::NativeStruct | CalcitProc::NativeNamedEnumNew | CalcitProc::NativeTraitCall)
      );
    let runtime_value = !metadata
      && matches!(arg, Calcit::Import(import) if {
        let qualified = format!("{}/{}", import.ns, import.def);
        ctx.value_imports.contains_key(&qualified) && !ctx.atom_globals.contains_key(&qualified)
      });
    if position > join || (position < join && !matches!(arg, Calcit::List(_)) && !runtime_value) {
      call.push(arg.clone());
      continue;
    }
    let name = Calcit::Local(CalcitLocal {
      idx: 0,
      sym: Arc::from(format!("&impl-join-arg:{}:{position}", ctx.next_local)),
      info: Arc::new(crate::wasm::host::calcit::CalcitSymbolInfo {
        at_ns: ns.clone(),
        at_def: Arc::from(""),
      }),
      location: None,
      type_info: DYNAMIC_TYPE.clone(),
    });
    pairs.push(Calcit::from(vec![name.clone(), arg.clone()]));
    call.push(name);
  }
  emit_let_pairs(ctx, &pairs, &[Calcit::from(call)])
}

/// Write the impl identities a back edge carries into its loop selectors.
fn emit_recur_selections(ctx: &mut WasmGenCtx, selections: &[Option<(u32, usize)>]) {
  for (local, position) in selections.iter().flatten() {
    ctx.emit(Instruction::I32Const(*position as i32));
    ctx.emit(Instruction::LocalSet(*local));
  }
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

/// Keep let value, type and closure lookup metadata inside its lexical body.
fn emit_scoped_let(
  ctx: &mut WasmGenCtx,
  name: &str,
  value: &Calcit,
  may_escape: bool,
  body: impl FnOnce(&mut WasmGenCtx) -> Result<(), String>,
) -> Result<(), String> {
  let old_local = ctx.locals.get(name).copied();
  let old_type = ctx.local_types.get(name).cloned();
  let old_closure = ctx.lambda_locals.get(name).cloned();
  let result = (|| {
    let binding_type = infer_wasm_value_evidence(ctx, value);
    let function_value = resolve_known_function_value(ctx, value);
    let number_literal = resolve_known_number_literal(ctx, value);
    // Resolve/evaluate the RHS against the outer scope before binding its name.
    if let Some(closure) = resolve_inline_closure(ctx, value) {
      let self_recursive = closure::inline_closure_requires_materialization(&closure);
      if may_escape || self_recursive {
        let local = closure::emit_materialize_inline_closure(ctx, &closure, Some(value))?;
        if self_recursive {
          ctx.lambda_locals.remove(name);
        } else {
          let mut materialized = (*closure).clone();
          materialized.materialized_local = Some(local);
          ctx.lambda_locals.insert(name.to_owned(), Arc::new(materialized));
        }
        ctx.locals.insert(name.to_owned(), local);
      } else {
        ctx.lambda_locals.insert(name.to_owned(), closure);
        ctx.locals.remove(name);
      }
    } else if let Some(alternatives) = impl_alternatives(ctx, value) {
      let selector = ImplSelector {
        local: ctx.alloc_local_typed(ValType::I32),
        alternatives,
      };
      emit_selecting_value(ctx, value, &selector)?;
      ctx.lambda_locals.remove(name);
      let index = ctx.declare_local(name);
      ctx.emit(Instruction::LocalSet(index));
      ctx.impl_selectors.insert(index, selector);
    } else {
      let repr = binding_type
        .as_ref()
        .map_or(NativeValRepr::F64, |ev| NativeValRepr::from_type_annotation(&ev.annotation));
      emit_expr_as_repr(ctx, value, repr)?;
      ctx.lambda_locals.remove(name);
      let index = ctx.declare_local_with_repr(name, repr);
      ctx.emit(Instruction::LocalSet(index));
      if let Some(definition) = function_value {
        ctx.local_function_values.insert(index, definition);
      }
      if let Some(n) = number_literal {
        ctx.local_number_literals.insert(index, n);
      }
    }
    if let Some(annotation) = binding_type {
      ctx.local_types.insert(name.to_owned(), annotation);
    } else {
      ctx.local_types.remove(name);
    }
    body(ctx)
  })();
  // Restore lookup metadata even on failure; emitted local slots stay allocated.
  match old_local {
    Some(index) => {
      ctx.locals.insert(name.to_owned(), index);
    }
    None => {
      ctx.locals.remove(name);
    }
  }
  match old_type {
    Some(annotation) => {
      ctx.local_types.insert(name.to_owned(), annotation);
    }
    None => {
      ctx.local_types.remove(name);
    }
  }
  match old_closure {
    Some(closure) => {
      ctx.lambda_locals.insert(name.to_owned(), closure);
    }
    None => {
      ctx.lambda_locals.remove(name);
    }
  }
  result
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
  let may_escape = closure::let_pairs_may_escape(&var_name, &pairs[1..], body);
  emit_scoped_let(ctx, &var_name, &xs[1], may_escape, |ctx| emit_let_pairs(ctx, &pairs[1..], body))
}

/// Emit WASM for `let` expression returning `F64`.
fn emit_let(ctx: &mut WasmGenCtx, body: &[Calcit]) -> Result<(), String> {
  emit_let_as_repr(ctx, body, NativeValRepr::F64)
}

fn emit_let_as_repr(ctx: &mut WasmGenCtx, body: &[Calcit], dst_repr: NativeValRepr) -> Result<(), String> {
  if body.is_empty() {
    emit_unit_const_repr(ctx, dst_repr);
    return Ok(());
  }

  let pair = &body[0];
  let rest = &body[1..];

  match pair {
    Calcit::Nil | Calcit::Unit => emit_body_as_repr(ctx, rest, dst_repr),
    Calcit::List(xs) if xs.is_empty() => emit_body_as_repr(ctx, rest, dst_repr),
    Calcit::List(xs) if xs.len() == 2 => {
      let var_name = match &xs[0] {
        Calcit::Local(CalcitLocal { sym, .. }) => sym.to_string(),
        Calcit::Symbol { sym, .. } => sym.to_string(),
        other => return Err(format!("let binding expected symbol, got: {other}")),
      };
      if rest.len() == 1
        && is_direct_nil_to_option_call_on_var(&rest[0], &var_name)
        && let Some(lowered) = lower_nil_to_option_target(ctx, &xs[1])?
      {
        return emit_expr_as_repr(ctx, &lowered, dst_repr);
      }
      let may_escape = closure::closure_name_may_escape(&var_name, rest);
      emit_scoped_let(ctx, &var_name, &xs[1], may_escape, |ctx| {
        // Flatten nested lets while retaining each binding's lexical scope.
        if rest.len() == 1
          && let Calcit::List(inner) = &rest[0]
          && let Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) = inner.first()
        {
          let inner_body: Vec<Calcit> = inner.drop_left().to_vec();
          return emit_let_as_repr(ctx, &inner_body, dst_repr);
        }
        emit_body_as_repr(ctx, rest, dst_repr)
      })
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

  if let Some(shape) = crate::wasm::host::codegen::destruct_list_match_shape(args) {
    return emit_destruct_list_match(ctx, &shape);
  }

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

  // Direct consumer fast path: match directly on an OptionHeapPtrI32 field/payload
  // using `i32.eqz` / raw ptr unwrapping without materializing a heap Option.
  let is_option_match = tag_branches.iter().all(|(pattern, _)| {
    let Calcit::List(pat_xs) = pattern else {
      return false;
    };
    matches!(
      (pat_xs.first(), pat_xs.len()),
      (Some(Calcit::Tag(t)), 1) if t.ref_str() == "none"
    ) || matches!(
      (pat_xs.first(), pat_xs.len()),
      (Some(Calcit::Tag(t)), 2) if t.ref_str() == "some"
    )
  });
  if is_option_match && let Some((raw_opt, inner_ty)) = try_emit_direct_niche_option_i32(ctx, &args[0])? {
    let num_tag_branches = tag_branches.len();
    if num_tag_branches == 0 {
      if let Some(body) = wildcard_body {
        emit_expr(ctx, body)?;
      } else {
        ctx.emit(f64_const(0.0));
      }
      return Ok(());
    }
    let after_subject_resets = ctx.reset_initialized_atoms.clone();
    let mut joined_resets: Option<HashSet<String>> = None;
    for (i, (pattern, body)) in tag_branches.iter().enumerate() {
      let Calcit::List(pat_xs) = pattern else { unreachable!() };
      let Calcit::Tag(t) = &pat_xs[0] else { unreachable!() };
      let is_none = t.ref_str() == "none";
      ctx.emit(Instruction::LocalGet(raw_opt));
      if is_none {
        ctx.emit(Instruction::I32Eqz);
      } else {
        ctx.emit(Instruction::I32Const(0));
        ctx.emit(Instruction::I32Ne);
      }
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      ctx.block_depth += 1;

      let mut saved_binding: Option<(String, Option<WasmTypeEvidence>)> = None;
      if !is_none {
        let binding = &pat_xs[1];
        let bind_name = match binding {
          Calcit::Local(CalcitLocal { sym, .. }) => sym.to_string(),
          Calcit::Symbol { sym, .. } => sym.to_string(),
          other => return Err(format!("match binding expected symbol, got: {other}")),
        };
        let bound_ev = WasmTypeEvidence::contract(resolve_match_payload_type(binding, inner_ty.clone()));
        let bind_repr = NativeValRepr::from_type_annotation(&bound_ev.annotation);
        ctx.emit(Instruction::LocalGet(raw_opt));
        emit_convert_repr(ctx, NativeValRepr::HeapPtrI32, bind_repr);
        let idx = ctx.declare_local_with_repr(&bind_name, bind_repr);
        saved_binding = Some((bind_name.clone(), ctx.local_types.get(&bind_name).cloned()));
        ctx.local_types.insert(bind_name, bound_ev);
        ctx.emit(Instruction::LocalSet(idx));
      }

      ctx.reset_initialized_atoms.clone_from(&after_subject_resets);
      emit_expr(ctx, body)?;
      match &mut joined_resets {
        Some(acc) => acc.retain(|k| ctx.reset_initialized_atoms.contains(k)),
        None => joined_resets = Some(ctx.reset_initialized_atoms.clone()),
      }
      if let Some((bind_name, prev_ev)) = saved_binding {
        if let Some(ev) = prev_ev {
          ctx.local_types.insert(bind_name, ev);
        } else {
          ctx.local_types.remove(&bind_name);
        }
      }

      ctx.emit(Instruction::Else);
      if i == num_tag_branches - 1 {
        ctx.reset_initialized_atoms.clone_from(&after_subject_resets);
        if let Some(wb) = wildcard_body {
          emit_expr(ctx, wb)?;
        } else {
          ctx.emit(f64_const(0.0));
        }
        match &mut joined_resets {
          Some(acc) => acc.retain(|k| ctx.reset_initialized_atoms.contains(k)),
          None => joined_resets = Some(ctx.reset_initialized_atoms.clone()),
        }
      }
    }
    ctx.reset_initialized_atoms = joined_resets.unwrap_or(after_subject_resets);
    for _ in 0..num_tag_branches {
      ctx.block_depth -= 1;
      ctx.emit(Instruction::End);
    }
    return Ok(());
  }

  // Evaluate the enum subject as an i32 pointer and read `[count:u16 @ 0][layout_id:u16 @ 2][tag_id:u32 @ 4]`.
  let enum_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count_local = emit_load_enum_count_i32(ctx, enum_ptr);
  let tag_local = emit_load_enum_tag_i32(ctx, enum_ptr);
  let layout_local = (ctx.enum_layouts.max_layout_id > 0).then(|| emit_load_enum_layout_id_i32(ctx, enum_ptr));

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

  let subject_ev = infer_wasm_value_evidence(ctx, &args[0]);
  let subject_ty = subject_ev
    .as_ref()
    .map(|ev| expand_wasm_value_type(ev.scalar_origin.as_ref().unwrap_or(&ev.annotation)));
  let anon_enum = match &args[0] {
    Calcit::List(xs)
      if xs.len() >= 2 && matches!(xs.first(), Some(Calcit::Proc(CalcitProc::NativeEnum | CalcitProc::NativeEnumNew))) =>
    {
      if let Some(Calcit::Tag(t)) = xs.get(1) {
        let payloads = xs.iter().skip(2).map(|p| infer_wasm_value_evidence(ctx, p)).collect::<Vec<_>>();
        Some((t.ref_str().to_owned(), payloads))
      } else {
        None
      }
    }
    _ => None,
  };

  // For each tag branch we emit:
  //   if (tag_local == variant_tag_id) then { bind payloads; body }
  //   else { next branch or wildcard }
  let after_subject_resets = ctx.reset_initialized_atoms.clone();
  let mut joined_resets: Option<HashSet<String>> = None;
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

    // Compare: tag_local == tag_id && count_local == binding_count
    let binding_count = pat_xs.len() - 1;
    ctx.emit(Instruction::LocalGet(tag_local));
    ctx.emit(Instruction::I32Const(tag_id as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::LocalGet(count_local));
    ctx.emit(Instruction::I32Const(binding_count as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::I32And);

    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
    ctx.block_depth += 1;

    let known_enum_def = subject_ty.as_ref().and_then(|ty| ty.resolve_to_enum());
    let compact_variant = known_enum_def
      .as_ref()
      .and_then(|def| ctx.enum_layouts.get_for_def(def, &tag_str, binding_count))
      .map(|(id, l)| (id, l.clone()));

    // Bind payload variables from enum fields
    let mut saved_types: Vec<(String, Option<WasmTypeEvidence>)> = Vec::with_capacity(binding_count);
    for bind_idx in 0..binding_count {
      let binding = &pat_xs[1 + bind_idx];
      let bind_name = match binding {
        Calcit::Local(CalcitLocal { sym, .. }) => sym.to_string(),
        Calcit::Symbol { sym, .. } => sym.to_string(),
        other => return Err(format!("match binding expected symbol, got: {other}")),
      };
      let bound_ev = if let Some((anon_tag, anon_payloads)) = &anon_enum
        && anon_tag == &tag_str
      {
        anon_payloads.get(bind_idx).cloned().flatten()
      } else if let Some(ty) = &subject_ty
        && let Some(enum_def) = known_enum_def.as_ref()
        && let Some(variant) = enum_def.find_variant_by_name(&tag_str)
      {
        let type_args = match ty.as_ref() {
          CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args) => args.as_slice(),
          _ => &[],
        };
        let bindings: TypeBindings = enum_def.generics().iter().cloned().zip(type_args.iter().cloned()).collect();
        variant
          .payload_types()
          .get(bind_idx)
          .map(|pt| replace_unbound_type_vars_with_never(&pt.substitute_type_vars(&bindings)))
          .filter(|pt| !pt.contains_type_var())
          .map(|sub_pt| WasmTypeEvidence::contract(resolve_match_payload_type(binding, sub_pt)))
      } else {
        None
      };
      let bind_repr = bound_ev
        .as_ref()
        .map_or(NativeValRepr::F64, |ev| NativeValRepr::from_type_annotation(&ev.annotation));
      if let (Some(layout_id_local), Some((expected_layout_id, compact_layout))) = (layout_local, &compact_variant) {
        ctx.emit(Instruction::LocalGet(layout_id_local));
        ctx.emit(Instruction::I32Const(i32::from(*expected_layout_id)));
        ctx.emit(Instruction::I32Eq);
        ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(bind_repr.val_type())));
        emit_load_struct_field_as_repr(ctx, enum_ptr, compact_layout.fields[bind_idx], bind_repr);
        ctx.emit(Instruction::Else);
        emit_load_enum_payload_as_f64(ctx, enum_ptr, bind_idx);
        emit_unbox_f64_to_repr(ctx, bind_repr);
        ctx.emit(Instruction::End);
      } else {
        emit_load_enum_payload_as_f64(ctx, enum_ptr, bind_idx);
        emit_unbox_f64_to_repr(ctx, bind_repr);
      }
      let idx = ctx.declare_local_with_repr(&bind_name, bind_repr);
      saved_types.push((bind_name.clone(), ctx.local_types.get(&bind_name).cloned()));
      if let Some(ev) = bound_ev {
        ctx.local_types.insert(bind_name, ev);
      } else {
        ctx.local_types.remove(&bind_name);
      }
      ctx.emit(Instruction::LocalSet(idx));
    }

    // Emit body
    ctx.reset_initialized_atoms.clone_from(&after_subject_resets);
    emit_expr(ctx, body)?;
    match &mut joined_resets {
      Some(acc) => acc.retain(|k| ctx.reset_initialized_atoms.contains(k)),
      None => joined_resets = Some(ctx.reset_initialized_atoms.clone()),
    }

    for (bind_name, prev_ev) in saved_types {
      if let Some(ev) = prev_ev {
        ctx.local_types.insert(bind_name, ev);
      } else {
        ctx.local_types.remove(&bind_name);
      }
    }

    ctx.emit(Instruction::Else);
    // If this is the last tag branch, emit wildcard or default
    if i == num_tag_branches - 1 {
      ctx.reset_initialized_atoms.clone_from(&after_subject_resets);
      if let Some(wb) = wildcard_body {
        emit_expr(ctx, wb)?;
      } else {
        ctx.emit(f64_const(0.0));
      }
      match &mut joined_resets {
        Some(acc) => acc.retain(|k| ctx.reset_initialized_atoms.contains(k)),
        None => joined_resets = Some(ctx.reset_initialized_atoms.clone()),
      }
    }
    // Otherwise the next iteration will emit the next if/else inside this else block
  }
  ctx.reset_initialized_atoms = joined_resets.unwrap_or(after_subject_resets);

  // Close all the if/else blocks (one End per branch)
  for _ in 0..num_tag_branches {
    ctx.block_depth -= 1;
    ctx.emit(Instruction::End);
  }

  Ok(())
}

/// Lowers `match (destruct-list xs) ((:none) a) ((:some h t) b)` with the same
/// list primitives `destruct-list` uses, without allocating the enum.
fn emit_destruct_list_match(ctx: &mut WasmGenCtx, shape: &crate::wasm::host::codegen::DestructListMatch<'_>) -> Result<(), String> {
  let subject_ev = infer_wasm_value_evidence(ctx, shape.subject);
  let elem_ev = subject_ev.as_ref().and_then(|ev| {
    let expanded = CalcitTypeAnnotation::expand_schema_alias(&ev.annotation);
    match expanded.as_ref() {
      CalcitTypeAnnotation::List(elem) => Some(WasmTypeEvidence {
        set_member_types: ev.set_member_types.clone(),
        ..WasmTypeEvidence::contract(elem.clone())
      }),
      _ => None,
    }
  });
  emit_ds_empty(ctx, std::slice::from_ref(shape.subject))?;
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ne);
  let after_cond_resets = ctx.reset_initialized_atoms.clone();
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.block_depth += 1;
  emit_expr(ctx, shape.none_body)?;
  let none_resets = std::mem::replace(&mut ctx.reset_initialized_atoms, after_cond_resets);
  ctx.emit(Instruction::Else);
  let prev_head = shape.head.and_then(|h| ctx.local_types.get(h.as_ref()).cloned());
  let prev_tail = shape.tail.and_then(|t| ctx.local_types.get(t.as_ref()).cloned());
  if let Some(head) = shape.head {
    emit_list_nth(ctx, &[shape.subject.to_owned(), Calcit::Number(0.0)])?;
    let idx = ctx.declare_local(head);
    if let Some(ev) = &elem_ev {
      ctx.local_types.insert(head.to_string(), ev.clone());
    } else {
      ctx.local_types.remove(head.as_ref());
    }
    ctx.emit(Instruction::LocalSet(idx));
  }
  if let Some(tail) = shape.tail {
    emit_list_rest(ctx, std::slice::from_ref(shape.subject))?;
    let idx = ctx.declare_local(tail);
    let tail_ev = subject_ev.as_ref().map(|ev| {
      let rest_ty = |ty: &Arc<CalcitTypeAnnotation>| -> Arc<CalcitTypeAnnotation> {
        match expand_wasm_value_type(ty).as_ref() {
          CalcitTypeAnnotation::Tuple(slots) => {
            if slots.len() > 1 {
              Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(slots[1..].to_vec())))
            } else {
              Arc::new(CalcitTypeAnnotation::List(
                crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone(),
              ))
            }
          }
          _ => ty.clone(),
        }
      };
      WasmTypeEvidence {
        annotation: rest_ty(&ev.annotation),
        codec_hint: ev.codec_hint.as_ref().map(rest_ty),
        ..ev.clone()
      }
    });
    if let Some(ev) = &tail_ev {
      ctx.local_types.insert(tail.to_string(), ev.clone());
    } else {
      ctx.local_types.remove(tail.as_ref());
    }
    ctx.emit(Instruction::LocalSet(idx));
  }
  emit_expr(ctx, shape.some_body)?;
  ctx.reset_initialized_atoms.retain(|k| none_resets.contains(k));
  if let Some(head) = shape.head {
    if let Some(ev) = prev_head {
      ctx.local_types.insert(head.to_string(), ev);
    } else {
      ctx.local_types.remove(head.as_ref());
    }
  }
  if let Some(tail) = shape.tail {
    if let Some(ev) = prev_tail {
      ctx.local_types.insert(tail.to_string(), ev);
    } else {
      ctx.local_types.remove(tail.as_ref());
    }
  }
  ctx.block_depth -= 1;
  ctx.emit(Instruction::End);
  Ok(())
}

fn expr_uses_wasi_stdio(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(xs) => {
      let writes = match xs.first() {
        Some(Calcit::Symbol { sym, .. }) => matches!(sym.as_ref(), "println" | "eprintln" | "echo"),
        Some(Calcit::Registered(name)) => matches!(name.as_ref(), "println" | "eprintln" | "echo"),
        Some(Calcit::Import(import)) => {
          import.ns.as_ref() == "calcit.core" && matches!(import.def.as_ref(), "println" | "eprintln" | "echo")
        }
        _ => false,
      };
      writes || xs.iter().any(expr_uses_wasi_stdio)
    }
    Calcit::Fn { info, .. } => info.body.iter().any(expr_uses_wasi_stdio),
    _ => false,
  }
}

fn expr_uses_wasi_file(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(xs) => xs.iter().any(expr_uses_wasi_file),
    Calcit::Fn { info, .. } => info.body.iter().any(expr_uses_wasi_file),
    Calcit::Proc(CalcitProc::NativeFsReadText | CalcitProc::NativeFsWriteText) => true,
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      ns.as_ref() == "calcit.core" && matches!(def.as_ref(), "fs-path:read-text" | "fs-path:write-text")
    }
    Calcit::Method(name, MethodKind::Invoke(receiver)) => {
      matches!(name.as_ref(), "read-text" | "write-text!")
        && matches!(receiver.as_ref(), CalcitTypeAnnotation::TypeRef(type_name, _) if type_name.as_ref() == "calcit.core/FsPath")
    }
    _ => false,
  }
}

fn expr_uses_wasi_read_dir(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(xs) => xs.iter().any(expr_uses_wasi_read_dir),
    Calcit::Fn { info, .. } => info.body.iter().any(expr_uses_wasi_read_dir),
    Calcit::Proc(CalcitProc::NativeFsReadDir) => true,
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      ns.as_ref() == "calcit.core" && matches!(def.as_ref(), "fs-path:read-dir" | "fs-path:walk-dir" | "try-read-dir")
    }
    Calcit::Method(name, MethodKind::Invoke(receiver)) => {
      name.as_ref() == "read-dir"
        && matches!(receiver.as_ref(), CalcitTypeAnnotation::TypeRef(type_name, _) if type_name.as_ref() == "calcit.core/FsPath")
    }
    _ => false,
  }
}

fn expr_uses_wasi_clocks(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(xs) => xs.iter().any(expr_uses_wasi_clocks),
    Calcit::Fn { info, .. } => info.body.iter().any(expr_uses_wasi_clocks),
    Calcit::Proc(CalcitProc::UnixTimeMs | CalcitProc::MonotonicTimeMs) => true,
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      ns.as_ref() == "calcit.core" && matches!(def.as_ref(), "unix-time-ms" | "monotonic-time-ms")
    }
    _ => false,
  }
}

fn expr_uses_wasi_wait(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(xs) => xs.iter().any(expr_uses_wasi_wait),
    Calcit::Fn { info, .. } => info.body.iter().any(expr_uses_wasi_wait),
    Calcit::Proc(CalcitProc::NativeWaitMs) => true,
    Calcit::Import(CalcitImport { ns, def, .. }) => ns.as_ref() == "calcit.core" && def.as_ref() == "wait-ms",
    _ => false,
  }
}

fn expr_uses_wasi_random(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(xs) => xs.iter().any(expr_uses_wasi_random),
    Calcit::Fn { info, .. } => info.body.iter().any(expr_uses_wasi_random),
    Calcit::Proc(CalcitProc::NativeSecureRandomBytes) => true,
    Calcit::Import(CalcitImport { ns, def, .. }) => ns.as_ref() == "calcit.core" && def.as_ref() == "secure-random-bytes",
    _ => false,
  }
}

fn validate_stdin_boundary(
  program_data: &program::CompiledProgram,
  init_ns: &str,
  init_def: &str,
  target: WasmTarget,
  boundary: WasmBoundary,
  world: WasiWorld,
) -> Result<(), String> {
  if target == WasmTarget::Wasi
    && boundary == WasmBoundary::Component
    && world == WasiWorld::Http
    && reachable_wasi_effect(program_data, init_ns, init_def, expr_uses_wasi_stdin)
  {
    return Err("E_WASI_HTTP_CAPABILITY: `read-stdin-text` is not supported in `--world http` (`wasi:http/proxy`)".into());
  }
  if (target != WasmTarget::Wasi || boundary != WasmBoundary::Component)
    && reachable_wasi_effect(program_data, init_ns, init_def, expr_uses_wasi_stdin)
  {
    return Err("E_WASI_COMMAND_CAPABILITY: read-stdin-text requires `calcit wasi --boundary component`".into());
  }
  Ok(())
}

fn expr_uses_wasi_stdin(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(xs) => xs.iter().any(expr_uses_wasi_stdin),
    Calcit::Fn { info, .. } => info.body.iter().any(expr_uses_wasi_stdin),
    Calcit::Proc(CalcitProc::NativeReadStdinText) => true,
    Calcit::Import(CalcitImport { ns, def, .. }) => ns.as_ref() == "calcit.core" && def.as_ref() == "read-stdin-text",
    _ => false,
  }
}

fn reachable_wasi_effect(
  program_data: &program::CompiledProgram,
  init_ns: &str,
  init_def: &str,
  uses_effect: fn(&Calcit) -> bool,
) -> bool {
  let definitions = program_data
    .iter()
    .flat_map(|(namespace, file)| {
      file
        .defs
        .iter()
        .map(move |(def_name, definition)| (definition.def_id, (namespace.as_ref(), def_name.as_ref(), definition)))
    })
    .collect::<HashMap<_, _>>();
  let Some(entry) = program_data.get(init_ns).and_then(|file| file.defs.get(init_def)) else {
    return false;
  };
  let mut pending = vec![entry.def_id];
  let mut visited = HashSet::new();
  while let Some(id) = pending.pop() {
    if !visited.insert(id) {
      continue;
    }
    let Some((namespace, def_name, definition)) = definitions.get(&id) else {
      continue;
    };
    if (*namespace != "calcit.core" || def_name.starts_with("&calcit:test:")) && uses_effect(&definition.preprocessed_code) {
      return true;
    }
    pending.extend(definition.deps.iter().copied());
  }
  false
}

fn wasi_component_stdin_imports() -> Vec<HostImport> {
  [
    ("read-via-stream", vec![ValType::I32], vec![]),
    (
      "[async-lower][stream-read-0]read-via-stream",
      vec![ValType::I32; 3],
      vec![ValType::I32],
    ),
    ("[stream-drop-readable-0]read-via-stream", vec![ValType::I32], vec![]),
    ("[future-read-1]read-via-stream", vec![ValType::I32; 2], vec![ValType::I32]),
    ("[future-drop-readable-1]read-via-stream", vec![ValType::I32], vec![]),
  ]
  .into_iter()
  .map(|(name, params, results)| HostImport {
    module: "wasi:cli/stdin@0.3.1".into(),
    name: name.into(),
    params,
    results,
  })
  .collect()
}

fn wasi_component_fs_common_imports() -> Vec<HostImport> {
  vec![
    HostImport {
      module: "wasi:filesystem/preopens@0.3.1".into(),
      name: "get-directories".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[resource-drop]descriptor".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[async-lower][method]descriptor.open-at".into(),
      params: vec![ValType::I32, ValType::I32],
      results: vec![ValType::I32],
    },
  ]
}

fn wasi_component_file_imports() -> Vec<HostImport> {
  // Keep descriptors, streams, and futures below the FsPath Result boundary.
  // WIT counts the write stream as slot 0, so its returned future is slot 1.
  let mut imports = wasi_component_fs_common_imports();
  imports.extend([
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[method]descriptor.read-via-stream".into(),
      params: vec![ValType::I32, ValType::I64, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[async-lower][stream-read-0][method]descriptor.read-via-stream".into(),
      params: vec![ValType::I32; 3],
      results: vec![ValType::I32],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[stream-drop-readable-0][method]descriptor.read-via-stream".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[future-read-1][method]descriptor.read-via-stream".into(),
      params: vec![ValType::I32; 2],
      results: vec![ValType::I32],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[future-drop-readable-1][method]descriptor.read-via-stream".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[stream-new-0][method]descriptor.write-via-stream".into(),
      params: vec![],
      results: vec![ValType::I64],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[stream-write-0][method]descriptor.write-via-stream".into(),
      params: vec![ValType::I32; 3],
      results: vec![ValType::I32],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[stream-drop-writable-0][method]descriptor.write-via-stream".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[method]descriptor.write-via-stream".into(),
      params: vec![ValType::I32, ValType::I32, ValType::I64],
      results: vec![ValType::I32],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[future-read-1][method]descriptor.write-via-stream".into(),
      params: vec![ValType::I32; 2],
      results: vec![ValType::I32],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[future-drop-readable-1][method]descriptor.write-via-stream".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
  ]);
  imports
}

fn wasi_component_read_dir_imports() -> Vec<HostImport> {
  vec![
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[method]descriptor.read-directory".into(),
      params: vec![ValType::I32, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[async-lower][stream-read-0][method]descriptor.read-directory".into(),
      params: vec![ValType::I32; 3],
      results: vec![ValType::I32],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[stream-drop-readable-0][method]descriptor.read-directory".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[future-read-1][method]descriptor.read-directory".into(),
      params: vec![ValType::I32; 2],
      results: vec![ValType::I32],
    },
    HostImport {
      module: "wasi:filesystem/types@0.3.1".into(),
      name: "[future-drop-readable-1][method]descriptor.read-directory".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
  ]
}

#[cfg(test)]
fn tests_option_enum_def_for_codec() -> Arc<CalcitEnumDef> {
  let prototype = crate::wasm::host::calcit::CalcitStructValue {
    struct_ref: Arc::new(CalcitStructDef {
      field_defaults: Default::default(),
      definition_ref: Some(Arc::from("calcit.core/Option")),
      name: cirru_edn::EdnTag::new("Option"),
      fields: Arc::new(vec![cirru_edn::EdnTag::new("none"), cirru_edn::EdnTag::new("some")]),
      field_types: Arc::new(vec![DYNAMIC_TYPE.clone(); 2]),
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    }),
    values: Arc::new(vec![
      Calcit::from(vec![]),
      Calcit::from(vec![CalcitTypeAnnotation::TypeVar(Arc::from("T")).to_calcit()]),
    ]),
  };
  Arc::new(CalcitEnumDef::from_struct(prototype).expect("valid test Option enum"))
}

#[cfg(test)]
fn tests_flag_enum_def_with_none() -> Arc<CalcitEnumDef> {
  let prototype = crate::wasm::host::calcit::CalcitStructValue {
    struct_ref: Arc::new(CalcitStructDef {
      field_defaults: Default::default(),
      definition_ref: Some(Arc::from("app.main/Flag")),
      name: cirru_edn::EdnTag::new("Flag"),
      fields: Arc::new(vec![cirru_edn::EdnTag::new("none")]),
      field_types: Arc::new(vec![DYNAMIC_TYPE.clone()]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    }),
    values: Arc::new(vec![Calcit::from(vec![])]),
  };
  Arc::new(CalcitEnumDef::from_struct(prototype).expect("valid test Flag enum"))
}

#[cfg(test)]
fn tests_state_enum_def_for_codec() -> Arc<CalcitEnumDef> {
  let prototype = crate::wasm::host::calcit::CalcitStructValue {
    struct_ref: Arc::new(CalcitStructDef {
      field_defaults: Default::default(),
      definition_ref: Some(Arc::from("app.main/State")),
      name: cirru_edn::EdnTag::new("State"),
      fields: Arc::new(vec![cirru_edn::EdnTag::new("ready")]),
      field_types: Arc::new(vec![DYNAMIC_TYPE.clone()]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    }),
    values: Arc::new(vec![Calcit::from(vec![])]),
  };
  Arc::new(CalcitEnumDef::from_struct(prototype).expect("valid test State enum"))
}

#[cfg(test)]
mod tests {
  use std::collections::{BTreeMap, HashMap};
  use std::fs;
  use std::process::Command;
  use std::str::FromStr;
  use std::sync::Arc;

  use super::component::component_type_owns_memory;
  use super::runtime::call_indirect;
  use super::{
    CompiledFn, ComponentAbiInvocation, ComponentAbiType, ComponentAsyncCanonicalImports, ComponentEnumType, ComponentEnumVariant,
    ComponentExportAdapter, ComponentExportRuntime, ComponentImportAdapter, ComponentStructType, ComponentValueCodecs, HEAP_BASE,
    HostImport, InlineClosure, ModuleFunctionLayout, StaticFnDef, WasiComponentReadImports, WasiWorld, WasmBoundary, WasmCompileEnv,
    WasmGenCtx, WasmGlobalLayout, WasmMemoryLayout, WasmTarget, WasmTypeEvidence, build_cabi_free_fn, build_cabi_realloc_fn,
    build_component_export_adapter, build_component_import_adapter, build_rest_adapter, build_string_pool,
    build_wasi_component_open_at_fn, build_wasi_component_read_bytes_fn, build_wasi_component_route_path_fn,
    build_wasi_component_select_preopen_fn, build_wasm_module, component_abi_type, component_export_needs_post_return,
    component_flat_types, component_import_signature, component_memory_layout, component_task_return_signature, emit_call_expr,
    emit_call_spread, emit_call_spread_args_as_regular, emit_inline_closure_call, emit_proc_call, emit_recur_loop,
    expr_uses_wasi_clocks, expr_uses_wasi_file, expr_uses_wasi_read_dir, expr_uses_wasi_wait, host_imports_for_target,
    index_host_imports, mem_arg_f64, must_reject_extraction_failure, reject_non_tail_recur, reject_reachable_wasi_command_dependencies,
    validate_component_export_symbols, validate_component_flat_parameters, validate_component_import_symbols, validate_recur_evidence,
    wasi_component_file_imports, wasi_component_read_dir_imports, wasm_non_nil_value_is_nonzero,
  };
  use crate::wasm::host::calcit::{
    Calcit, CalcitEnumDef, CalcitFnTypeAnnotation, CalcitImpl, CalcitImport, CalcitList, CalcitLocal, CalcitNumericRefinement,
    CalcitProc, CalcitStructDef, CalcitStructValue, CalcitSymbolInfo, CalcitSyntax, CalcitTypeAnnotation, ImportInfo, MethodKind,
    SchemaKind,
  };
  use cirru_edn::EdnTag;
  use wasm_encoder::{Instruction, ValType};
  use wasmtime::{Caller, Engine, Extern, Func, Instance, Module, Store};

  /// Branch lowering must reject invalid back edges and restore enclosing loop state.
  #[test]
  fn recur_lowering_preserves_tail_and_loop_evidence_boundaries() {
    let recur = || Calcit::from(vec![Calcit::Proc(CalcitProc::Recur), Calcit::Number(1.0)]);
    let tail = [Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.wasm")),
      Calcit::Bool(true),
      Calcit::Number(0.0),
      recur(),
    ])];
    assert!(reject_non_tail_recur(&tail, "tests.wasm/f").is_ok());

    let escaping = [Calcit::from(vec![Calcit::Proc(CalcitProc::List), recur()])];
    let error = reject_non_tail_recur(&escaping, "tests.wasm/f").expect_err("non-tail recur");
    assert!(error.contains("tail position"), "{error}");

    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::new(),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Arc::new(std::collections::HashSet::from([
        String::from("tests.wasm/LoopProbe"),
        String::from("tests.wasm/LimitProbe"),
      ])),
      any_type_table: Default::default(),
    };
    let mut ctx = WasmGenCtx::new(1, env.clone());
    let numeric = WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Number));
    ctx.recur_evidence = Some(vec![numeric.clone()]);
    validate_recur_evidence(&mut ctx, &[Calcit::Number(1.0)]).expect("same scalar contract");
    let error = validate_recur_evidence(&mut ctx, &[Calcit::Bool(false)]).expect_err("erased false is not Number zero");
    assert!(error.starts_with("E_WASM_CALL_SPECIALIZATION:"), "{error}");

    let mut left = CalcitStructDef::from_fields(EdnTag::new("LoopProbe"), vec![]);
    left.definition_ref = Some(Arc::from("tests.wasm/LoopProbe"));
    left.impls.push(Arc::new(CalcitImpl {
      name: EdnTag::new("LoopProbeImpl"),
      origin: None,
      fields: Arc::new(vec![EdnTag::new("flag?")]),
      values: Arc::new(vec![Calcit::Proc(CalcitProc::BoolQuestion)]),
    }));
    let mut right = left.clone();
    right.impls[0] = Arc::new(CalcitImpl {
      values: Arc::new(vec![Calcit::Proc(CalcitProc::StructQuestion)]),
      ..(*left.impls[0]).clone()
    });
    let expected = WasmTypeEvidence {
      exact_impls: true,
      ..WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(left))))
    };
    let changed = WasmTypeEvidence {
      exact_impls: true,
      ..WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(right))))
    };
    assert!(
      changed.annotation.is_proven_for(&expected.annotation),
      "data shape alone accepts both values"
    );
    let receiver = Calcit::Symbol {
      sym: Arc::from("receiver"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.wasm"),
        at_def: Arc::from("caller"),
      }),
      location: None,
    };
    ctx.locals.insert("receiver".to_owned(), 0);
    ctx.recur_evidence = Some(vec![expected.clone()]);
    ctx.local_types.insert("receiver".to_owned(), changed);
    let error = validate_recur_evidence(&mut ctx, std::slice::from_ref(&receiver)).expect_err("same data, different impl");
    assert!(
      error.starts_with(super::RECUR_IMPL_RETRY) && error.contains("implementation identity"),
      "{error}"
    );
    assert_eq!(ctx.recur_missing.len(), 1, "the changed identity is kept for a selected retry");
    ctx.local_types.insert("receiver".to_owned(), expected);
    validate_recur_evidence(&mut ctx, &[receiver]).expect("the original actual implementation");

    ctx.arg_indices = vec![0];
    ctx.recur_shape = Some((1, false));
    ctx.recur_evidence = Some(vec![numeric.clone()]);
    ctx.block_depth = 3;
    let inner = ctx.alloc_local();
    let error = emit_recur_loop(
      &mut ctx,
      vec![inner],
      None,
      Some(vec![WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Bool))]),
      &[recur()],
    )
    .expect_err("a nested loop changes its Bool argument to Number");
    assert!(error.starts_with("E_WASM_CALL_SPECIALIZATION:"), "{error}");
    assert_eq!(ctx.arg_indices, [0]);
    assert_eq!(ctx.recur_shape, Some((1, false)));
    assert_eq!(ctx.recur_evidence.as_ref().unwrap().len(), 1);
    assert!(ctx.recur_evidence.as_ref().unwrap()[0] == numeric);
    assert_eq!(ctx.block_depth, 3);

    // The IIFE fallback must not erase a selected loop's resource-limit error.
    let count = super::MAX_IMPL_ALTERNATIVES + 1;
    let mut limit_ctx = WasmGenCtx::new((count * 2 - 1) as u32, env);
    let info = Arc::new(CalcitSymbolInfo {
      at_ns: Arc::from("tests.wasm"),
      at_def: Arc::from("impl-limit"),
    });
    let local = |name: &str, type_info: Arc<CalcitTypeAnnotation>| {
      Calcit::Local(CalcitLocal {
        idx: 0,
        sym: Arc::from(name),
        info: info.clone(),
        location: None,
        type_info,
      })
    };
    let choices = (0..count)
      .map(|index| {
        if index > 0 {
          let condition = format!("choose{index}");
          limit_ctx.locals.insert(condition.clone(), (count + index - 1) as u32);
          limit_ctx
            .local_types
            .insert(condition, WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Bool)));
        }
        let name = format!("choice{index}");
        let mut prototype = CalcitStructDef::from_fields(EdnTag::new("LimitProbe"), vec![]);
        prototype.definition_ref = Some(Arc::from("tests.wasm/LimitProbe"));
        prototype.impls.push(Arc::new(CalcitImpl {
          name: EdnTag::new(format!("LimitProbeImpl{index}")),
          origin: None,
          fields: Arc::new(vec![EdnTag::new("flag?")]),
          values: Arc::new(vec![Calcit::Proc(CalcitProc::BoolQuestion)]),
        }));
        let annotation = Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(prototype)));
        limit_ctx.locals.insert(name.clone(), index as u32);
        limit_ctx.local_types.insert(
          name.clone(),
          WasmTypeEvidence {
            exact_impls: true,
            ..WasmTypeEvidence::contract(annotation.clone())
          },
        );
        local(&name, annotation)
      })
      .collect::<Vec<_>>();
    let body = |choices: &[Calcit]| {
      choices
        .iter()
        .enumerate()
        .skip(1)
        .rev()
        .fold(Calcit::Number(0.0), |tail, (index, choice)| {
          Calcit::from(vec![
            Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.wasm")),
            local(&format!("choose{index}"), Arc::new(CalcitTypeAnnotation::Bool)),
            Calcit::from(vec![Calcit::Proc(CalcitProc::Recur), choice.clone()]),
            tail,
          ])
        })
    };
    super::emit_inline_iife(
      &mut limit_ctx,
      &["receiver".to_owned()],
      &[body(&choices[..super::MAX_IMPL_ALTERNATIVES])],
      &choices[..1],
      None,
      &[],
    )
    .expect("eight implementation identities still compile");
    let error = super::emit_inline_iife(
      &mut limit_ctx,
      &["receiver".to_owned()],
      &[body(&choices)],
      &choices[..1],
      None,
      &[],
    )
    .expect_err("the ninth implementation identity must retain its limit diagnostic");
    assert_eq!(
      error,
      format!(
        "E_WASM_CALL_SPECIALIZATION: recur argument 1 carries more than {} implementation identities",
        super::MAX_IMPL_ALTERNATIVES
      )
    );
  }

  #[test]
  fn coherent_nominal_recur_bypasses_impl_selector_and_variant_cap() {
    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::new(),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Default::default(),
      any_type_table: Default::default(),
    };
    let count = super::MAX_IMPL_ALTERNATIVES + 3;
    let mut ctx = WasmGenCtx::new((count * 2 - 1) as u32, env);
    let info = Arc::new(CalcitSymbolInfo {
      at_ns: Arc::from("tests.wasm"),
      at_def: Arc::from("coherent-limit"),
    });
    let local = |name: &str, type_info: Arc<CalcitTypeAnnotation>| {
      Calcit::Local(CalcitLocal {
        idx: 0,
        sym: Arc::from(name),
        info: info.clone(),
        location: None,
        type_info,
      })
    };
    let choices = (0..count)
      .map(|index| {
        if index > 0 {
          let condition = format!("choose{index}");
          ctx.locals.insert(condition.clone(), (count + index - 1) as u32);
          ctx
            .local_types
            .insert(condition, WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Bool)));
        }
        let name = format!("choice{index}");
        let mut prototype = CalcitStructDef::from_fields(EdnTag::new("CoherentProbe"), vec![]);
        prototype.definition_ref = Some(Arc::from("tests.wasm/CoherentProbe"));
        prototype.impls.push(Arc::new(CalcitImpl {
          name: EdnTag::new(format!("CoherentProbeImpl{index}")),
          origin: None,
          fields: Arc::new(vec![EdnTag::new("flag?")]),
          values: Arc::new(vec![Calcit::Proc(CalcitProc::BoolQuestion)]),
        }));
        let annotation = Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(prototype)));
        ctx.locals.insert(name.clone(), index as u32);
        ctx.local_types.insert(
          name.clone(),
          WasmTypeEvidence {
            exact_impls: true,
            ..WasmTypeEvidence::contract(annotation.clone())
          },
        );
        local(&name, annotation)
      })
      .collect::<Vec<_>>();

    let body = choices
      .iter()
      .enumerate()
      .skip(1)
      .rev()
      .fold(choices[0].clone(), |tail, (index, choice)| {
        Calcit::from(vec![
          Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.wasm")),
          local(&format!("choose{index}"), Arc::new(CalcitTypeAnnotation::Bool)),
          Calcit::from(vec![Calcit::Proc(CalcitProc::Recur), choice.clone()]),
          tail,
        ])
      });

    assert!(
      super::impl_alternatives(&ctx, &choices[0]).is_none() && super::impl_alternatives(&ctx, &body).is_none(),
      "statically coherent nominal types must not synthesize ImplSelector alternatives"
    );

    let locals_before = ctx.next_local;
    super::emit_inline_iife(&mut ctx, &["receiver".to_owned()], &[body], &choices[..1], None, &[])
      .expect("coherent nominal recur loop must bypass MAX_IMPL_ALTERNATIVES cap");
    assert_eq!(
      ctx.next_local,
      locals_before + count as u32,
      "only the receiver param and recur arg temps should be allocated, with zero ImplSelector locals"
    );
    assert!(
      !ctx.instructions.iter().any(|ins| matches!(ins, Instruction::BrTable(..))),
      "coherent nominal loop must not emit an ImplSelector br_table dispatch"
    );
  }

  #[test]
  fn callee_identity_preserves_namespaces_and_specialized_arguments() {
    let mut fn_index = HashMap::from([
      ("tests.alpha/helper".to_string(), 4),
      ("tests.beta/helper".to_string(), 5),
      ("helper".to_string(), 5),
    ]);
    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: fn_index.clone(),
      fn_arity: HashMap::from([("tests.alpha/helper".to_string(), 0), ("helper".to_string(), 0)]),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::new(),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Default::default(),
      any_type_table: Default::default(),
    };
    let mut ctx = WasmGenCtx::new(0, env);
    ctx.static_fn_defs.insert(
      "helper".to_string(),
      Arc::new(StaticFnDef {
        params: vec!["x".to_string()],
        body: vec![],
        arg_types: vec![Arc::new(CalcitTypeAnnotation::Dynamic)],
        callback_arities: HashMap::new(),
        fixed_arity: true,
        nil_sensitive: true,
        trait_sensitive: false,
        method_sensitive: false,
      }),
    );
    let call = Calcit::from(vec![Calcit::Symbol {
      sym: Arc::from("helper"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.alpha"),
        at_def: Arc::from("caller"),
      }),
      location: None,
    }]);
    let Calcit::List(items) = call else {
      panic!("constructed call must be a list");
    };
    emit_call_expr(&mut ctx, &items).expect("qualified symbol call");
    assert!(matches!(ctx.instructions.last(), Some(Instruction::Call(4))));

    let other_call = Calcit::from(vec![Calcit::Symbol {
      sym: Arc::from("helper"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.beta"),
        at_def: Arc::from("caller"),
      }),
      location: None,
    }]);
    ctx.instructions.clear();
    emit_proc_call(&mut ctx, &CalcitProc::NativeNumberRem, &[Calcit::List(items.clone()), other_call])
      .expect("remainder with callable arguments");
    let calls = ctx
      .instructions
      .iter()
      .filter_map(|instruction| match instruction {
        Instruction::Call(idx) => Some(*idx),
        _ => None,
      })
      .collect::<Vec<_>>();
    assert_eq!(calls, [4, 5], "remainder must evaluate each operand once, in source order");

    ctx.static_fn_defs.remove("helper");
    fn_index.remove("tests.alpha/helper");
    ctx.fn_index = fn_index;
    ctx.instructions.clear();
    emit_call_expr(&mut ctx, &items).expect("legacy bare symbol fallback");
    assert!(matches!(ctx.instructions.last(), Some(Instruction::Call(5))));

    // A signature proves call shape, not the target or its host capabilities.
    let callable = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      open_reason: Default::default(),
      cases: Default::default(),
      guard: None,
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: Arc::new(CalcitTypeAnnotation::Number),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(Default::default()),
      runtime_arity: None,
    })));
    let local = |name: &str| {
      Calcit::Local(CalcitLocal {
        idx: CalcitLocal::track_sym(&Arc::from(name)),
        sym: Arc::from(name),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.alpha"),
          at_def: Arc::from("caller"),
        }),
        location: None,
        type_info: callable.clone(),
      })
    };
    let reference = |name: &str| CalcitImport {
      ns: Arc::from("tests.beta"),
      def: Arc::from(name),
      info: Arc::new(ImportInfo::SameFile {
        at_def: Arc::from("caller"),
      }),
      def_id: None,
    };
    ctx.static_fn_defs.insert(
      "tests.beta/apply".into(),
      Arc::new(StaticFnDef {
        params: vec!["callback".into()],
        body: vec![Calcit::from(vec![local("callback")])],
        arg_types: vec![Arc::new(CalcitTypeAnnotation::Dynamic)],
        callback_arities: HashMap::new(),
        fixed_arity: true,
        nil_sensitive: true,
        trait_sensitive: false,
        method_sensitive: false,
      }),
    );
    let source = ctx.alloc_local();
    ctx.locals.insert("source".into(), source);
    ctx.local_function_values.insert(source, reference("helper"));
    let Calcit::List(apply) = Calcit::from(vec![Calcit::Import(reference("apply")), local("source")]) else {
      unreachable!()
    };
    let emitted = |instructions| CompiledFn {
      export_name: None,
      params: vec![],
      results: vec![ValType::F64],
      locals: vec![],
      instructions,
    };
    ctx.instructions.clear();
    emit_call_expr(&mut ctx, &apply).expect("known actual callback target");
    assert!(matches!(ctx.instructions.last(), Some(Instruction::Call(5))));
    let unsupported = HashMap::from([(
      5,
      (
        "tests.beta/helper".into(),
        "E_WASI_COMMAND_CAPABILITY: native-only host probe".into(),
      ),
    )]);
    let host = reject_reachable_wasi_command_dependencies(&[emitted(ctx.instructions.clone())], 0, 0, &unsupported)
      .expect_err("a known target must not bypass host validation");
    assert!(host.starts_with("E_WASI_COMMAND_CAPABILITY:"), "{host}");

    // Callback identity must select specialization even without a scalar or
    // trait-sensitive body. An otherwise identical unknown value must not.
    let mut ordinary = ctx.static_fn_defs["tests.beta/apply"].as_ref().clone();
    ordinary.arg_types = vec![callable.clone()];
    ordinary.callback_arities.insert(0, 0);
    ordinary.nil_sensitive = false;
    ctx.static_fn_defs.insert("tests.beta/apply-ordinary".into(), Arc::new(ordinary));
    ctx.fn_index.insert("tests.beta/apply-ordinary".into(), 6);
    ctx.fn_arity.insert("tests.beta/apply-ordinary".into(), 1);
    let Calcit::List(ordinary_call) = Calcit::from(vec![Calcit::Import(reference("apply-ordinary")), local("source")]) else {
      unreachable!()
    };
    ctx.instructions.clear();
    emit_call_expr(&mut ctx, &ordinary_call).expect("known callback alone selects specialization");
    assert!(matches!(ctx.instructions.last(), Some(Instruction::Call(5))));
    let host = reject_reachable_wasi_command_dependencies(&[emitted(ctx.instructions.clone())], 0, 0, &unsupported)
      .expect_err("ordinary callback specialization still checks host capabilities");
    assert!(host.starts_with("E_WASI_COMMAND_CAPABILITY:"), "{host}");

    ctx.specialization_stack.push("tests.beta/apply-ordinary".into());
    ctx.instructions.clear();
    emit_call_expr(&mut ctx, &ordinary_call).expect("an active expansion is not recursively specialized");
    assert!(matches!(ctx.instructions.last(), Some(Instruction::Call(6))));
    ctx.specialization_stack.pop();

    ctx.local_function_values.remove(&source);
    ctx.instructions.clear();
    emit_call_expr(&mut ctx, &ordinary_call).expect("an unknown callback keeps the ordinary compiled body");
    assert!(matches!(ctx.instructions.last(), Some(Instruction::Call(6))));
    ctx.instructions.clear();
    emit_call_expr(&mut ctx, &apply).expect("the same signature without target evidence");
    assert!(
      !ctx
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::CallIndirect { .. })),
      "dynamic call must dispatch via BrTable closure trampoline instead of CallIndirect"
    );
    assert!(
      ctx
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::Call(0))),
      "dynamic call must invoke allocated __closure_call_0 trampoline"
    );
    reject_reachable_wasi_command_dependencies(&[emitted(ctx.instructions.clone()), emitted(vec![])], 0, 0, &HashMap::new())
      .expect("BrTable closure trampoline without unsupported arms passes WASI dependency validation");

    // Loop parameters are mutable slots, unlike the immutable bindings above.
    // Do not retain an initial function target without a back-edge proof.
    let mut recursive = ctx.static_fn_defs["tests.beta/apply"].as_ref().clone();
    recursive.body = vec![Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.beta")),
      Calcit::Bool(false),
      Calcit::from(vec![Calcit::Proc(CalcitProc::Recur), local("callback")]),
      Calcit::from(vec![local("callback")]),
    ])];
    ctx.static_fn_defs.insert("tests.beta/apply-recur".into(), Arc::new(recursive));
    ctx.local_function_values.insert(source, reference("helper"));
    let Calcit::List(recursive_call) = Calcit::from(vec![Calcit::Import(reference("apply-recur")), local("source")]) else {
      unreachable!()
    };
    ctx.instructions.clear();
    emit_call_expr(&mut ctx, &recursive_call).expect("loop-carried function value uses closure trampoline");
    assert!(
      !ctx
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::CallIndirect { .. }))
    );
    assert!(
      ctx
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::Call(0)))
    );
    assert!(
      !ctx
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::Call(5)))
    );
  }

  /// A runtime spread has no static length, so WASM must reject it for a
  /// fixed-arity proc instead of dropping the list. Native accepts the same
  /// call, which is why this stays a codegen test rather than a core `:tests` entry.
  #[test]
  fn proc_spread_inlines_literals_and_rejects_runtime_lists() {
    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::new(),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Default::default(),
      any_type_table: Default::default(),
    };
    let mut ctx = WasmGenCtx::new(0, env);
    let spread = || Calcit::Syntax(CalcitSyntax::ArgSpread, Arc::from("tests.wasm"));
    let literal = |items: &[f64]| {
      let mut xs = vec![Calcit::Proc(CalcitProc::List)];
      xs.extend(items.iter().map(|n| Calcit::Number(*n)));
      Calcit::from(xs)
    };
    let adds = |ctx: &WasmGenCtx| {
      ctx
        .instructions
        .iter()
        .filter(|instruction| matches!(instruction, Instruction::F64Add))
        .count()
    };

    // `&+ 1 & ([] 3)` inlines the literal as the second argument.
    emit_call_spread_args_as_regular(&mut ctx, &CalcitProc::NativeAdd, &[Calcit::Number(1.0), spread(), literal(&[3.0])])
      .expect("literal spread after an explicit argument");
    assert_eq!(adds(&ctx), 1);

    // `&+ & ([] 1) & ([] 3)` accepts several literal spreads.
    ctx.instructions.clear();
    emit_call_spread_args_as_regular(
      &mut ctx,
      &CalcitProc::NativeAdd,
      &[spread(), literal(&[1.0]), spread(), literal(&[3.0])],
    )
    .expect("two literal spreads");
    assert_eq!(adds(&ctx), 1);

    // `&+ & ([]) 1 3` keeps arguments after a spread.
    ctx.instructions.clear();
    emit_call_spread_args_as_regular(
      &mut ctx,
      &CalcitProc::NativeAdd,
      &[spread(), literal(&[]), Calcit::Number(1.0), Calcit::Number(3.0)],
    )
    .expect("arguments after an empty literal spread");
    assert_eq!(adds(&ctx), 1);

    // `&+ 1 & xs` with a runtime list has no fixed arity.
    let runtime = Calcit::Symbol {
      sym: Arc::from("xs"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.wasm"),
        at_def: Arc::from("caller"),
      }),
      location: None,
    };
    for proc in [CalcitProc::NativeAdd, CalcitProc::NativeListConcat] {
      // `&list:concat` lowers a trailing runtime spread, but not one followed by more arguments.
      let args = if proc == CalcitProc::NativeListConcat {
        vec![spread(), runtime.clone(), literal(&[1.0])]
      } else {
        vec![Calcit::Number(1.0), spread(), runtime.clone()]
      };
      let error = emit_call_spread_args_as_regular(&mut ctx, &proc, &args).expect_err("runtime spread into a fixed-arity proc");
      assert!(error.starts_with("E_WASM_CALL_SPECIALIZATION:"), "{error}");
      assert!(
        error.contains(&format!("cannot spread a runtime list into proc `{proc}`")),
        "{error}"
      );
    }

    let error = emit_call_spread_args_as_regular(&mut ctx, &CalcitProc::NativeAdd, &[Calcit::Number(1.0), spread()])
      .expect_err("missing spread list");
    assert!(error.starts_with("E_WASM_CALL_SPECIALIZATION:"), "{error}");
    assert!(error.contains("expects a list after `&`"), "{error}");

    // `&+ & ([] 2 & ([] 3))`: preprocessing turns the inner literal into
    // `(&call-spread [] 2 & ([] 3))`, which is still a literal spread.
    ctx.instructions.clear();
    let nested = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::CallSpread, Arc::from("tests.wasm")),
      Calcit::Proc(CalcitProc::List),
      Calcit::Number(2.0),
      spread(),
      literal(&[3.0]),
    ]);
    emit_call_spread_args_as_regular(&mut ctx, &CalcitProc::NativeAdd, &[spread(), nested]).expect("nested literal spread");
    assert_eq!(adds(&ctx), 1);
  }

  /// Guard compiler-side body expansion, not finite nested argument evaluation.
  #[test]
  fn anonymous_method_expansion_guard_is_scoped_to_the_body() {
    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::new(),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Default::default(),
      any_type_table: Default::default(),
    };
    let mut ctx = WasmGenCtx::new(0, env);
    let closure = InlineClosure {
      params: vec!["argument".to_owned()],
      required: 1,
      rest: false,
      option_types: vec![],
      body: vec![Calcit::Number(7.0)],
      captured_locals: HashMap::new(),
      captured_local_types: HashMap::new(),
      captured_closures: HashMap::new(),
      ..InlineClosure::default()
    };
    ctx.specialization_stack.push("method:test".to_owned());
    let error = emit_inline_closure_call(&mut ctx, &closure, &[Calcit::Number(42.0)], Some("method:test"))
      .expect_err("recursive body expansion must be rejected");
    assert!(error.starts_with("E_WASM_CALL_SPECIALIZATION: recursive specialization"));
    assert_eq!(ctx.specialization_stack, ["method:test"]);
    assert!(matches!(ctx.instructions.first(), Some(Instruction::F64Const(value)) if f64::from(*value) == 42.0));
    assert!(
      !ctx
        .instructions
        .iter()
        .any(|op| matches!(op, Instruction::F64Const(value) if f64::from(*value) == 7.0))
    );
    ctx.specialization_stack.clear();
    emit_inline_closure_call(&mut ctx, &closure, &[Calcit::Number(42.0)], Some("method:test"))
      .expect("a nonrecursive body must still compile");
    assert!(ctx.specialization_stack.is_empty());
  }

  /// A literal spread into a local function lowers to a plain call only for an
  /// inline lambda accepting the expanded argument count; the plain call binds
  /// `?` / `&` parameters like native (#1964). A function-table-backed local
  /// traps: a declared Fn type is not proof, since a rest-parameter function is
  /// accepted where a fixed Fn type is declared.
  #[test]
  fn local_spread_call_requires_inline_lambda() {
    let fn_type = |arg_types: Vec<Arc<CalcitTypeAnnotation>>, rest: bool| {
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        open_reason: Default::default(),
        cases: Default::default(),
        guard: None,
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types,
        return_type: Arc::new(CalcitTypeAnnotation::Dynamic),
        fn_kind: SchemaKind::Fn,
        rest_type: rest.then(|| Arc::new(CalcitTypeAnnotation::Dynamic)),
        features: Arc::new(Default::default()),
        runtime_arity: None,
      })))
    };
    let local = |type_info: Arc<CalcitTypeAnnotation>| CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("f")),
      sym: Arc::from("f"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.wasm"),
        at_def: Arc::from("caller"),
      }),
      location: None,
      type_info,
    };
    let dynamic_type = || Arc::new(CalcitTypeAnnotation::Dynamic);
    let fixed_type = || fn_type(vec![dynamic_type(), dynamic_type()], false);

    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::from([("list".to_string(), 1)]),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Default::default(),
      any_type_table: Default::default(),
    };
    let mut ctx = WasmGenCtx::new(0, env);
    let list_literal = |items: &[f64]| {
      let mut nodes = vec![Calcit::Proc(CalcitProc::List)];
      nodes.extend(items.iter().map(|item| Calcit::Number(*item)));
      Calcit::from(nodes)
    };
    let literal = list_literal(&[1.0, 2.0]);
    let spread_call = |ctx: &mut WasmGenCtx, callee: CalcitLocal, spread: &Calcit| {
      ctx.instructions.clear();
      let call = [
        Calcit::Local(callee),
        Calcit::Syntax(CalcitSyntax::ArgSpread, Arc::from("tests.wasm")),
        spread.clone(),
      ];
      emit_call_spread(ctx, &call).expect("spread call to a local");
    };
    let assert_traps = |ctx: &mut WasmGenCtx, callee: CalcitLocal| {
      spread_call(ctx, callee, &literal);
      assert!(matches!(ctx.instructions.as_slice(), [Instruction::Unreachable]));
    };

    // Function-table-backed locals trap whatever their declared type says,
    // including a fixed two-argument Fn type.
    for type_info in [
      fixed_type(),
      fn_type(vec![dynamic_type()], true),
      fn_type(
        vec![
          dynamic_type(),
          Arc::new(CalcitTypeAnnotation::Nullable(
            dynamic_type(),
            crate::wasm::host::calcit::NullableOrigin::Inferred,
          )),
        ],
        false,
      ),
      dynamic_type(),
    ] {
      assert_traps(&mut ctx, local(type_info));
    }

    // An inline lambda declared as `fn (a & xs)` lowers to an inline call that
    // packs the two trailing arguments into a rest list.
    ctx.lambda_locals.insert(
      "f".to_string(),
      Arc::new(InlineClosure {
        params: vec!["a".to_string(), "xs".to_string()],
        required: 1,
        rest: true,
        option_types: vec![],
        body: vec![Calcit::Number(0.0)],
        captured_locals: HashMap::new(),
        captured_local_types: HashMap::new(),
        captured_closures: HashMap::new(),
        ..InlineClosure::default()
      }),
    );
    spread_call(&mut ctx, local(fixed_type()), &list_literal(&[5.0, 6.0, 7.0]));
    assert!(
      ctx
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, Instruction::F64Const(count) if f64::from(*count) == 2.0)),
      "rest list stores its item count"
    );
    assert!(!matches!(ctx.instructions.as_slice(), [Instruction::Unreachable]));
    // Without the required argument the spread call still traps.
    spread_call(&mut ctx, local(fixed_type()), &list_literal(&[]));
    assert!(matches!(ctx.instructions.as_slice(), [Instruction::Unreachable]));
  }

  /// A function value is its table slot, and a call with N arguments goes
  /// through funcref table N. There, the slot of a `&` rest-parameter function
  /// holds an adapter typed `(f64 x N) -> f64` that packs the trailing
  /// arguments into the rest list (#1972); a fixed-arity slot holds the
  /// function itself, so calling it with another arity fails the signature check.
  #[test]
  fn rest_function_table_slots_pack_trailing_arguments() {
    let function = |params: usize, export_name: Option<&str>, instructions: Vec<Instruction<'static>>| CompiledFn {
      export_name: export_name.map(str::to_owned),
      params: vec![ValType::F64; params],
      results: vec![ValType::F64],
      locals: vec![],
      instructions,
    };
    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::from([("list".to_string(), 1)]),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Default::default(),
      any_type_table: Default::default(),
    };
    let mut ctx = WasmGenCtx::new(0, env);
    // Calls `call_indirect` on `slot` with `arity` arguments 10, 20, ...
    let caller = |name: &str, slot: i32, arity: u32| {
      let mut instructions: Vec<Instruction<'static>> = (1..=arity).map(|i| super::f64_const(10.0 * f64::from(i))).collect();
      instructions.push(Instruction::I32Const(slot));
      instructions.push(call_indirect(arity));
      function(0, Some(name), instructions)
    };
    // Table slot 0 is `(a & xs)` returning `a + count xs`; slot 1 is `(a b)`
    // returning `b`. Functions 4 and 5 are rest adapters for arities 3 and 1.
    let fns = [
      function(
        2,
        None,
        vec![
          Instruction::LocalGet(1),
          Instruction::I32TruncF64U,
          Instruction::F64Load(mem_arg_f64(0)),
          Instruction::LocalGet(0),
          Instruction::F64Add,
        ],
      ),
      function(2, None, vec![Instruction::LocalGet(1)]),
      caller("rest3", 0, 3),
      caller("rest1", 0, 1),
      build_rest_adapter(&mut ctx, 0, 1, 3),
      build_rest_adapter(&mut ctx, 0, 1, 1),
      caller("fixed2", 1, 2),
      caller("fixed3", 1, 3),
    ];
    let bytes = build_wasm_module(
      &fns,
      &[],
      16384,
      &[],
      &[],
      1,
      ModuleFunctionLayout {
        runtime_fn_count: 0,
        number_format_fn_count: 0,
        table_fn_count: 2,
        memory_layout: WasmMemoryLayout::compute(16384, 0),
        global_layout: WasmGlobalLayout::compute(0, false),
        rest_adapters: HashMap::from([((3, 0), 4), ((1, 0), 5)]),
        prune_unused_imports: false,
      },
    )
    .expect("valid module");
    let engine = Engine::default();
    let module = Module::new(&engine, &bytes).expect("valid module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate");
    let mut call = |name: &str| {
      let function = instance.get_typed_func::<(), f64>(&mut store, name).expect("caller export");
      function.call(&mut store, ())
    };
    assert_eq!(call("rest3").expect("packs two trailing arguments"), 12.0);
    assert_eq!(call("rest1").expect("packs an empty rest list"), 10.0);
    assert_eq!(call("fixed2").expect("fixed-arity slot is called directly"), 20.0);
    let error = call("fixed3").expect_err("wrong arity traps");
    assert_eq!(
      error.downcast_ref::<wasmtime::Trap>(),
      Some(&wasmtime::Trap::BadSignature),
      "{error:?}"
    );
  }

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
        arena_alloc_index: 40,
        cabi_realloc_index: 30,
        enum_load_payload_index: 44,
        option_from_niche_ptr_index: 45,
        option_to_niche_ptr_index: 46,
        enum_layouts: Box::leak(Box::new(super::EnumLayouts::default())),
        list_codecs,
        struct_codecs,
        variant_codecs,
      },
      cabi_free_index: 29,
      drop_indices,
      turn_enter_index: 41,
      turn_exit_index: 42,
      rewind_to_mark_index: 43,
      comp_globals: super::WasmGlobalLayout::compute(0, true)
        .component
        .expect("component layout must include component globals"),
      active_turns_global: super::WasmGlobalLayout::compute(0, true).active_turns,
      pers_needs_compact_global: None,
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
        field_defaults: Default::default(),
        definition_ref: Some(Arc::from(format!("app.main/{name}"))),
        name: EdnTag::new(name),
        fields: Arc::new(fields),
        field_types: Arc::new(vec![crate::wasm::host::calcit::DYNAMIC_TYPE.clone(); values.len()]),
        generics: Arc::new(generics),
        where_bounds: Arc::new(vec![]),
        impls: vec![],
      }),
      values: Arc::new(values),
    };
    Arc::new(CalcitEnumDef::from_struct(prototype).expect("valid test Enum declaration"))
  }

  #[test]
  fn builtin_scalar_and_component_tags_are_available_without_source_literals() {
    let tags = super::collect_all_tags_from(&[], None, None, &[]);
    for tag in ["none", "some", "ok", "err", "number", "bool", "nil", "unit"] {
      assert!(tags.contains_key(tag), "missing built-in runtime tag {tag}");
    }
  }

  #[test]
  fn typed_edn_shape_registers_nominal_struct_tags() {
    let nominal = Arc::new(CalcitStructDef {
      field_defaults: Default::default(),
      definition_ref: Some(Arc::from("app.main/Job")),
      name: EdnTag::new("Job"),
      fields: Arc::new(vec![EdnTag::new("count")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::Numeric(CalcitNumericRefinement::Int32))]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let shape = crate::wasm::host::calcit::data_shape::DataShapeGraph::build(
      &CalcitTypeAnnotation::Struct(nominal, Arc::new(vec![])),
      "app.main",
    )
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
  fn wasi_file_effects_require_stackful_command_for_read_write_and_directory_calls() {
    let fs_path = Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("calcit.core/FsPath"), Arc::new(vec![])));
    let read = Calcit::Method(Arc::from("read-text"), MethodKind::Invoke(fs_path.clone()));
    let retired_write = Calcit::Method(Arc::from("write-text"), MethodKind::Invoke(fs_path.clone()));
    let write_effect = Calcit::Method(Arc::from("write-text!"), MethodKind::Invoke(fs_path.clone()));
    let directory = Calcit::Method(Arc::from("read-dir"), MethodKind::Invoke(fs_path));
    assert!(expr_uses_wasi_file(&Calcit::from(vec![read])));
    assert!(expr_uses_wasi_file(&Calcit::from(vec![Calcit::from(vec![write_effect.clone()])])));
    assert!(expr_uses_wasi_file(&write_effect));
    assert!(
      !expr_uses_wasi_file(&retired_write),
      "the retired `.write-text` alias no longer reaches the filesystem effect"
    );
    assert!(expr_uses_wasi_file(&Calcit::Proc(CalcitProc::NativeFsReadText)));
    assert!(expr_uses_wasi_file(&Calcit::Proc(CalcitProc::NativeFsWriteText)));
    assert!(!expr_uses_wasi_file(&Calcit::Proc(CalcitProc::NativeFsReadDir)));
    assert!(!expr_uses_wasi_file(&directory));
    assert!(expr_uses_wasi_read_dir(&Calcit::Proc(CalcitProc::NativeFsReadDir)));
    assert!(expr_uses_wasi_read_dir(&directory));
    assert!(!expr_uses_wasi_clocks(&Calcit::Proc(CalcitProc::NativeWaitMs)));
    assert!(expr_uses_wasi_wait(&Calcit::Proc(CalcitProc::NativeWaitMs)));
    assert!(expr_uses_wasi_clocks(&Calcit::Proc(CalcitProc::UnixTimeMs)));
    assert!(expr_uses_wasi_clocks(&Calcit::Proc(CalcitProc::MonotonicTimeMs)));
  }

  #[test]
  fn wasi_03_filesystem_preopen_imports_match_pinned_command_wit() {
    let mut imports = wasi_component_file_imports();
    imports.extend(wasi_component_read_dir_imports());
    let fns = vec![
      CompiledFn {
        export_name: Some("cabi_realloc".into()),
        params: vec![ValType::I32; 4],
        results: vec![ValType::I32],
        locals: vec![],
        instructions: vec![Instruction::I32Const(32)],
      },
      CompiledFn {
        export_name: Some("cabi_free".into()),
        params: vec![ValType::I32; 2],
        results: vec![],
        locals: vec![],
        instructions: vec![],
      },
      CompiledFn {
        export_name: Some("wasi:cli/run@0.3.1#run".into()),
        params: vec![],
        results: vec![ValType::I32],
        locals: vec![],
        instructions: vec![Instruction::I32Const(0)],
      },
    ];
    let core = build_wasm_module(
      &fns,
      &imports,
      16,
      &[],
      &[],
      1,
      ModuleFunctionLayout {
        runtime_fn_count: 0,
        number_format_fn_count: 0,
        table_fn_count: 0,
        memory_layout: WasmMemoryLayout::compute(16, 0),
        global_layout: WasmGlobalLayout::compute(0, false),
        rest_adapters: HashMap::new(),
        prune_unused_imports: false,
      },
    )
    .expect("valid core module");
    calcit_bindgen::package_wasi_command(&core).expect("pinned WASI 0.3.1 WIT accepts filesystem imports");
  }

  #[test]
  fn wasi_03_route_path_enforces_guest_preopen_boundaries() {
    let mut route_fn = build_wasi_component_route_path_fn();
    route_fn.export_name = Some("route".into());
    let bytes = build_wasm_module(
      &[route_fn],
      &[],
      16384,
      &[],
      &[],
      1,
      ModuleFunctionLayout {
        runtime_fn_count: 0,
        number_format_fn_count: 0,
        table_fn_count: 0,
        memory_layout: WasmMemoryLayout::compute(16384, 0),
        global_layout: WasmGlobalLayout::compute(0, false),
        rest_adapters: HashMap::new(),
        prune_unused_imports: false,
      },
    )
    .expect("valid route helper module");
    let engine = Engine::default();
    let module = Module::new(&engine, &bytes).expect("valid route helper");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate route helper");
    let memory = instance.get_memory(&mut store, "memory").expect("module memory");
    let route = instance
      .get_typed_func::<(i32, i32, i32), i32>(&mut store, "route")
      .expect("route function");
    for (path, name, expected) in [
      ("workspace/input.cirru", "/workspace", 10),
      ("workspace/input.cirru", ".", 0),
      ("workspace/input.cirru", "/", 0),
      ("workspace2/input.cirru", "/workspace", -1),
      ("workspace", "/workspace", -1),
      ("/workspace/input.cirru", "/workspace", -1),
      ("workspace/../input.cirru", "/workspace", -1),
      ("workspace/a..b", "/workspace", 10),
      ("workspace/\0file", "/workspace", -1),
      ("项目/input.cirru", "/项目", 7),
      ("workspace/input.cirru", "", -1),
    ] {
      memory
        .write(&mut store, 64, &(path.len() as f64).to_le_bytes())
        .expect("path length");
      memory.write(&mut store, 72, path.as_bytes()).expect("path bytes");
      memory.write(&mut store, 8192, name.as_bytes()).expect("preopen name");
      assert_eq!(
        route.call(&mut store, (64, 8192, name.len() as i32)).expect("route call"),
        expected,
        "path={path:?}, preopen={name:?}"
      );
    }
  }

  #[test]
  fn wasi_03_select_preopen_releases_unselected_resources() {
    #[derive(Default)]
    struct HostState {
      directories: Vec<(i32, String)>,
      dropped: Vec<i32>,
      freed: Vec<(i32, i32)>,
    }
    let imports = vec![
      HostImport {
        module: "test".into(),
        name: "get-directories".into(),
        params: vec![ValType::I32],
        results: vec![],
      },
      HostImport {
        module: "test".into(),
        name: "drop-descriptor".into(),
        params: vec![ValType::I32],
        results: vec![],
      },
      HostImport {
        module: "test".into(),
        name: "free".into(),
        params: vec![ValType::I32; 2],
        results: vec![],
      },
    ];
    let route = build_wasi_component_route_path_fn();
    let mut select = build_wasi_component_select_preopen_fn(0, 1, 2, 3);
    select.export_name = Some("select".into());
    let bytes = build_wasm_module(
      &[route, select],
      &imports,
      16384,
      &[],
      &[],
      1,
      ModuleFunctionLayout {
        runtime_fn_count: 0,
        number_format_fn_count: 0,
        table_fn_count: 0,
        memory_layout: WasmMemoryLayout::compute(16384, 0),
        global_layout: WasmGlobalLayout::compute(0, false),
        rest_adapters: HashMap::new(),
        prune_unused_imports: false,
      },
    )
    .expect("valid preopen selector module");
    let engine = Engine::default();
    let module = Module::new(&engine, &bytes).expect("valid preopen selector");
    let mut store = Store::new(&engine, HostState::default());
    let get_directories = Func::wrap(&mut store, |mut caller: Caller<'_, HostState>, retptr: i32| {
      let directories = caller.data().directories.clone();
      let memory = caller.get_export("memory").expect("memory export").into_memory().expect("memory");
      let mut name_ptr = 8192usize;
      for (index, (handle, name)) in directories.iter().enumerate() {
        let entry = 4096 + index * 12;
        memory.write(&mut caller, entry, &handle.to_le_bytes()).expect("descriptor handle");
        memory
          .write(&mut caller, entry + 4, &(name_ptr as i32).to_le_bytes())
          .expect("name pointer");
        memory
          .write(&mut caller, entry + 8, &(name.len() as i32).to_le_bytes())
          .expect("name length");
        memory.write(&mut caller, name_ptr, name.as_bytes()).expect("name bytes");
        name_ptr += name.len().max(1);
      }
      let result = retptr as usize;
      let list_ptr: i32 = if directories.is_empty() { 0 } else { 4096 };
      memory.write(&mut caller, result, &list_ptr.to_le_bytes()).expect("list pointer");
      memory
        .write(&mut caller, result + 4, &(directories.len() as i32).to_le_bytes())
        .expect("list length");
    });
    let drop_descriptor = Func::wrap(&mut store, |mut caller: Caller<'_, HostState>, descriptor: i32| {
      caller.data_mut().dropped.push(descriptor);
    });
    let free = Func::wrap(&mut store, |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
      caller.data_mut().freed.push((ptr, len));
    });
    let instance = Instance::new(
      &mut store,
      &module,
      &[Extern::Func(get_directories), Extern::Func(drop_descriptor), Extern::Func(free)],
    )
    .expect("instantiate selector");
    let memory = instance.get_memory(&mut store, "memory").expect("module memory");
    let select = instance
      .get_typed_func::<(i32, i32), i32>(&mut store, "select")
      .expect("select function");

    store.data_mut().directories = vec![(11, ".".into()), (22, "/workspace".into()), (33, "/workspace2".into())];
    let path = "workspace/input.cirru";
    memory
      .write(&mut store, 64, &(path.len() as f64).to_le_bytes())
      .expect("path length");
    memory.write(&mut store, 72, path.as_bytes()).expect("path bytes");
    assert_eq!(select.call(&mut store, (64, 2048)).expect("select preopen"), 1);
    let output = &memory.data(&store)[2048..2060];
    assert_eq!(i32::from_le_bytes(output[0..4].try_into().unwrap()), 22);
    assert_eq!(i32::from_le_bytes(output[4..8].try_into().unwrap()), 82);
    assert_eq!(i32::from_le_bytes(output[8..12].try_into().unwrap()), 11);
    assert_eq!(store.data().dropped, [11, 33]);
    assert_eq!(store.data().freed.len(), 4, "all names and the list must be freed");

    store.data_mut().dropped.clear();
    store.data_mut().freed.clear();
    let invalid = "workspace/../secret";
    memory
      .write(&mut store, 64, &(invalid.len() as f64).to_le_bytes())
      .expect("path length");
    memory.write(&mut store, 72, invalid.as_bytes()).expect("path bytes");
    assert_eq!(select.call(&mut store, (64, 2048)).expect("reject invalid path"), 0);
    assert_eq!(store.data().dropped, [11, 22, 33]);
    assert_eq!(store.data().freed.len(), 4);
    assert_eq!(&memory.data(&store)[2048..2060], &[0; 12]);

    store.data_mut().dropped.clear();
    store.data_mut().freed.clear();
    store.data_mut().directories = (0..65).map(|index| (index, ".".into())).collect();
    assert_eq!(select.call(&mut store, (64, 2048)).expect("reject oversized table"), 0);
    assert_eq!(store.data().dropped.len(), 65);
    assert_eq!(store.data().freed.len(), 66);

    store.data_mut().dropped.clear();
    store.data_mut().freed.clear();
    store.data_mut().directories.clear();
    assert_eq!(select.call(&mut store, (64, 2048)).expect("reject empty preopens"), 0);
    assert!(store.data().dropped.is_empty());
    assert_eq!(store.data().freed, [(0, 0)]);
    assert_eq!(&memory.data(&store)[2048..2060], &[0; 12]);
  }

  #[test]
  fn wasi_03_select_preopen_runs_with_real_wasmtime_host() {
    let Some(cli) = std::env::var_os("WASMTIME_CLI") else {
      return;
    };
    let host = tempfile::tempdir().expect("host directory");
    let output = tempfile::tempdir().expect("Component output directory");
    for (path, should_succeed) in [("workspace/input.cirru", true), ("other/input.cirru", false)] {
      let imports = vec![
        HostImport {
          module: "wasi:filesystem/preopens@0.3.1".into(),
          name: "get-directories".into(),
          params: vec![ValType::I32],
          results: vec![],
        },
        HostImport {
          module: "wasi:filesystem/types@0.3.1".into(),
          name: "[resource-drop]descriptor".into(),
          params: vec![ValType::I32],
          results: vec![],
        },
      ];
      let global_layout = WasmGlobalLayout::compute(0, true);
      let comp = global_layout.component.expect("component layout must exist");
      let mut free = build_cabi_free_fn(comp);
      free.export_name = Some("cabi_free".into());
      let mut realloc = build_cabi_realloc_fn(2, comp);
      realloc.export_name = Some("cabi_realloc".into());
      let run = CompiledFn {
        export_name: Some("wasi:cli/run@0.3.1#run".into()),
        params: vec![],
        results: vec![ValType::I32],
        locals: vec![],
        instructions: vec![
          Instruction::I32Const(HEAP_BASE),
          Instruction::I32Const(2048),
          Instruction::Call(5),
          Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)),
          Instruction::I32Const(2048),
          Instruction::I32Load(super::mem_arg_i32(0)),
          Instruction::Call(1),
          Instruction::I32Const(0),
          Instruction::Else,
          Instruction::I32Const(1),
          Instruction::End,
        ],
      };
      let mut string_data = (path.len() as f64).to_le_bytes().to_vec();
      string_data.extend_from_slice(path.as_bytes());
      let heap_start = HEAP_BASE + ((string_data.len() + 7) & !7) as i32;
      let core = build_wasm_module(
        &[
          free,
          realloc,
          build_wasi_component_route_path_fn(),
          build_wasi_component_select_preopen_fn(0, 1, 2, 4),
          run,
        ],
        &imports,
        heap_start,
        &string_data,
        &[],
        1,
        ModuleFunctionLayout {
          runtime_fn_count: 4,
          number_format_fn_count: 0,
          table_fn_count: 0,
          memory_layout: WasmMemoryLayout::compute(heap_start, 0),
          global_layout,
          rest_adapters: HashMap::new(),
          prune_unused_imports: false,
        },
      )
      .expect("valid core command");
      let component = calcit_bindgen::package_wasi_command(&core).expect("package real preopen command");
      let module = output.path().join("program.wasm");
      fs::write(&module, component).expect("write test Component");
      let result = Command::new(&cli)
        .args(["run", "-S", "p3", "--dir"])
        .arg(format!("{}::/workspace", host.path().display()))
        .arg(&module)
        .output()
        .expect("run WASI 0.3 preopen command");
      assert_eq!(
        result.status.success(),
        should_succeed,
        "path {path:?}: {}",
        String::from_utf8_lossy(&result.stderr)
      );
    }
  }

  #[test]
  fn wasi_03_async_open_at_runs_with_real_wasmtime_host() {
    let Some(cli) = std::env::var_os("WASMTIME_CLI") else {
      return;
    };
    let host = tempfile::tempdir().expect("host directory");
    fs::write(host.path().join("input.cirru"), "hello").expect("host input file");
    let output = tempfile::tempdir().expect("Component output directory");
    for (path, should_succeed) in [
      ("workspace/input.cirru", true),
      ("workspace/missing.cirru", false),
      ("other/input.cirru", false),
      ("workspace/../input.cirru", false),
    ] {
      let mut imports = vec![
        HostImport {
          module: "wasi:filesystem/preopens@0.3.1".into(),
          name: "get-directories".into(),
          params: vec![ValType::I32],
          results: vec![],
        },
        HostImport {
          module: "wasi:filesystem/types@0.3.1".into(),
          name: "[resource-drop]descriptor".into(),
          params: vec![ValType::I32],
          results: vec![],
        },
        HostImport {
          module: "wasi:filesystem/types@0.3.1".into(),
          name: "[async-lower][method]descriptor.open-at".into(),
          params: vec![ValType::I32; 2],
          results: vec![ValType::I32],
        },
      ];
      let canonical = super::register_component_async_canonical_imports(&mut imports);
      let task_return = imports.len() as u32;
      imports.push(HostImport {
        module: "[export]wasi:cli/run@0.3.1".into(),
        name: "[task-return]run".into(),
        params: vec![ValType::I32],
        results: vec![],
      });
      let free_index = imports.len() as u32;
      let global_layout = WasmGlobalLayout::compute(0, true);
      let comp = global_layout.component.expect("component layout must exist");
      let mut free = build_cabi_free_fn(comp);
      free.export_name = Some("cabi_free".into());
      let mut realloc = build_cabi_realloc_fn(free_index, comp);
      realloc.export_name = Some("cabi_realloc".into());
      let open_index = free_index + 4;
      let instructions = vec![
        Instruction::I32Const(HEAP_BASE),
        Instruction::I32Const(0),
        Instruction::I32Const(1),
        Instruction::I32Const(2048),
        Instruction::Call(open_index),
        Instruction::If(wasm_encoder::BlockType::Empty),
        Instruction::I32Const(2048),
        Instruction::I32Load(super::mem_arg_i32(0)),
        Instruction::Call(1),
        Instruction::I32Const(0),
        Instruction::Call(task_return),
        Instruction::Else,
        Instruction::I32Const(1),
        Instruction::Call(task_return),
        Instruction::End,
      ];
      let run = CompiledFn {
        export_name: Some("[async-lift-stackful]wasi:cli/run@0.3.1#run".into()),
        params: vec![],
        results: vec![],
        locals: vec![],
        instructions,
      };
      let mut string_data = (path.len() as f64).to_le_bytes().to_vec();
      string_data.extend_from_slice(path.as_bytes());
      let heap_start = HEAP_BASE + ((string_data.len() + 7) & !7) as i32;
      let core = build_wasm_module(
        &[
          free,
          realloc,
          build_wasi_component_route_path_fn(),
          build_wasi_component_select_preopen_fn(0, 1, free_index, free_index + 2),
          build_wasi_component_open_at_fn(free_index + 3, 2, 1, free_index, free_index + 1, &canonical),
          run,
        ],
        &imports,
        heap_start,
        &string_data,
        &[],
        1,
        ModuleFunctionLayout {
          runtime_fn_count: 5,
          number_format_fn_count: 0,
          table_fn_count: 0,
          memory_layout: WasmMemoryLayout::compute(heap_start, 0),
          global_layout,
          rest_adapters: HashMap::new(),
          prune_unused_imports: false,
        },
      )
      .expect("valid async open-at core command");
      let component = calcit_bindgen::package_wasi_command(&core).expect("package async open-at command");
      let module = output.path().join("open-at.wasm");
      fs::write(&module, component).expect("write async open-at Component");
      let result = Command::new(&cli)
        .args(["run", "-S", "p3", "-W", "component-model-async-stackful=y", "--dir"])
        .arg(format!("{}::/workspace", host.path().display()))
        .arg(&module)
        .output()
        .expect("run WASI 0.3 open-at command");
      assert!(
        result.stderr.is_empty(),
        "unexpected host trap for {path:?}: {}",
        String::from_utf8_lossy(&result.stderr)
      );
      assert_eq!(
        result.status.success(),
        should_succeed,
        "path {path:?}: {}",
        String::from_utf8_lossy(&result.stderr)
      );
    }
  }

  #[test]
  fn wasi_03_read_stream_runs_with_real_wasmtime_host() {
    let Some(cli) = std::env::var_os("WASMTIME_CLI") else {
      return;
    };
    let host = tempfile::tempdir().expect("host directory");
    fs::write(host.path().join("input.cirru"), "hello").expect("host input file");
    let output = tempfile::tempdir().expect("Component output directory");
    let mut imports = wasi_component_file_imports();
    let canonical = super::register_component_async_canonical_imports(&mut imports);
    let task_return = imports.len() as u32;
    imports.push(HostImport {
      module: "[export]wasi:cli/run@0.3.1".into(),
      name: "[task-return]run".into(),
      params: vec![ValType::I32],
      results: vec![],
    });
    let import_indices = index_host_imports(&imports);
    let descriptor_module = "wasi:filesystem/types@0.3.1";
    let import = |name: &str| {
      *import_indices
        .get(&(descriptor_module.into(), name.into()))
        .expect("filesystem import")
    };
    let free_index = imports.len() as u32;
    let global_layout = WasmGlobalLayout::compute(0, true);
    let comp = global_layout.component.expect("component layout must exist");
    let mut free = build_cabi_free_fn(comp);
    free.export_name = Some("cabi_free".into());
    let mut realloc = build_cabi_realloc_fn(free_index, comp);
    realloc.export_name = Some("cabi_realloc".into());
    let open_index = free_index + 4;
    let run = CompiledFn {
      export_name: Some("[async-lift-stackful]wasi:cli/run@0.3.1#run".into()),
      params: vec![],
      results: vec![],
      locals: vec![ValType::I32; 5],
      instructions: vec![
        Instruction::I32Const(HEAP_BASE),
        Instruction::I32Const(0),
        Instruction::I32Const(1),
        Instruction::I32Const(2048),
        Instruction::Call(open_index),
        Instruction::I32Eqz,
        Instruction::If(wasm_encoder::BlockType::Empty),
        Instruction::I32Const(1),
        Instruction::Call(task_return),
        Instruction::Return,
        Instruction::End,
        Instruction::I32Const(2048),
        Instruction::I32Load(super::mem_arg_i32(0)),
        Instruction::I64Const(0),
        Instruction::I32Const(1024),
        Instruction::Call(import("[method]descriptor.read-via-stream")),
        Instruction::I32Const(1024),
        Instruction::I32Load(super::mem_arg_i32(0)),
        Instruction::LocalSet(0),
        Instruction::I32Const(1028),
        Instruction::I32Load(super::mem_arg_i32(0)),
        Instruction::LocalSet(1),
        Instruction::LocalGet(0),
        Instruction::I32Const(4096),
        Instruction::I32Const(16),
        Instruction::Call(import("[async-lower][stream-read-0][method]descriptor.read-via-stream")),
        Instruction::LocalSet(2),
        Instruction::LocalGet(0),
        Instruction::I32Const(4104),
        Instruction::I32Const(16),
        Instruction::Call(import("[async-lower][stream-read-0][method]descriptor.read-via-stream")),
        Instruction::LocalSet(3),
        Instruction::LocalGet(0),
        Instruction::Call(import("[stream-drop-readable-0][method]descriptor.read-via-stream")),
        Instruction::LocalGet(1),
        Instruction::I32Const(3072),
        Instruction::Call(import("[future-read-1][method]descriptor.read-via-stream")),
        Instruction::LocalSet(4),
        Instruction::LocalGet(1),
        Instruction::Call(import("[future-drop-readable-1][method]descriptor.read-via-stream")),
        Instruction::I32Const(2048),
        Instruction::I32Load(super::mem_arg_i32(0)),
        Instruction::Call(import("[resource-drop]descriptor")),
        Instruction::LocalGet(2),
        Instruction::I32Const(80),
        Instruction::I32Eq,
        Instruction::LocalGet(3),
        Instruction::I32Const(1),
        Instruction::I32Eq,
        Instruction::I32And,
        Instruction::LocalGet(4),
        Instruction::I32Eqz,
        Instruction::I32And,
        Instruction::I32Const(3072),
        Instruction::I32Load8U(super::mem_arg_byte(0)),
        Instruction::I32Eqz,
        Instruction::I32And,
        Instruction::I32Const(4096),
        Instruction::I32Load(super::mem_arg_i32(0)),
        Instruction::I32Const(0x6c6c6568),
        Instruction::I32Eq,
        Instruction::I32And,
        Instruction::I32Const(4100),
        Instruction::I32Load8U(super::mem_arg_byte(0)),
        Instruction::I32Const(b'o' as i32),
        Instruction::I32Eq,
        Instruction::I32And,
        Instruction::If(wasm_encoder::BlockType::Empty),
        Instruction::I32Const(0),
        Instruction::Call(task_return),
        Instruction::Else,
        Instruction::I32Const(1),
        Instruction::Call(task_return),
        Instruction::End,
      ],
    };
    let mut string_data = ("workspace/input.cirru".len() as f64).to_le_bytes().to_vec();
    string_data.extend_from_slice(b"workspace/input.cirru");
    let heap_start = HEAP_BASE + ((string_data.len() + 7) & !7) as i32;
    let core = build_wasm_module(
      &[
        free,
        realloc,
        build_wasi_component_route_path_fn(),
        build_wasi_component_select_preopen_fn(
          *import_indices
            .get(&("wasi:filesystem/preopens@0.3.1".into(), "get-directories".into()))
            .expect("preopen import"),
          import("[resource-drop]descriptor"),
          free_index,
          free_index + 2,
        ),
        build_wasi_component_open_at_fn(
          free_index + 3,
          import("[async-lower][method]descriptor.open-at"),
          import("[resource-drop]descriptor"),
          free_index,
          free_index + 1,
          &canonical,
        ),
        run,
      ],
      &imports,
      heap_start,
      &string_data,
      &[],
      1,
      ModuleFunctionLayout {
        runtime_fn_count: 5,
        number_format_fn_count: 0,
        table_fn_count: 0,
        memory_layout: WasmMemoryLayout::compute(heap_start, 0),
        global_layout,
        rest_adapters: HashMap::new(),
        prune_unused_imports: false,
      },
    )
    .expect("valid file stream core command");
    let component = calcit_bindgen::package_wasi_command(&core).expect("package file stream command");
    let module = output.path().join("read-stream.wasm");
    fs::write(&module, component).expect("write file stream Component");
    let result = Command::new(&cli)
      .args([
        "run",
        "-S",
        "p3",
        "-W",
        "component-model-async-stackful=y",
        "-W",
        "component-model-more-async-builtins=y",
        "--dir",
      ])
      .arg(format!("{}::/workspace", host.path().display()))
      .arg(&module)
      .output()
      .expect("run WASI 0.3 file stream command");
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
  }

  #[test]
  fn wasi_03_bounded_read_bytes_runs_with_real_wasmtime_host() {
    let Some(cli) = std::env::var_os("WASMTIME_CLI") else {
      return;
    };
    let host = tempfile::tempdir().expect("host directory");
    fs::write(host.path().join("short.cirru"), "hello").expect("short input");
    fs::write(host.path().join("large.cirru"), vec![b'a'; 128 * 1024]).expect("multi-chunk input");
    fs::write(host.path().join("empty.cirru"), []).expect("empty input");
    fs::write(host.path().join("limit.cirru"), vec![b'e'; 4 * 1024 * 1024]).expect("boundary input");
    fs::write(host.path().join("oversized.cirru"), vec![b'x'; 4 * 1024 * 1024 + 1]).expect("oversized input");
    let output = tempfile::tempdir().expect("Component output directory");
    for (path, expected_len, expected_first) in [
      ("workspace/short.cirru", Some(5), Some(b'h')),
      ("workspace/large.cirru", Some(128 * 1024), Some(b'a')),
      ("workspace/empty.cirru", Some(0), None),
      ("workspace/limit.cirru", Some(4 * 1024 * 1024), Some(b'e')),
      ("workspace/oversized.cirru", None, None),
      ("workspace/missing.cirru", None, None),
    ] {
      let mut imports = wasi_component_file_imports();
      let canonical = super::register_component_async_canonical_imports(&mut imports);
      let task_return = imports.len() as u32;
      imports.push(HostImport {
        module: "[export]wasi:cli/run@0.3.1".into(),
        name: "[task-return]run".into(),
        params: vec![ValType::I32],
        results: vec![],
      });
      let import_indices = index_host_imports(&imports);
      let import = |name: &str| {
        *import_indices
          .get(&("wasi:filesystem/types@0.3.1".into(), name.into()))
          .expect("filesystem import")
      };
      let free_index = imports.len() as u32;
      let global_layout = WasmGlobalLayout::compute(0, true);
      let comp = global_layout.component.expect("component layout must exist");
      let mut free = build_cabi_free_fn(comp);
      free.export_name = Some("cabi_free".into());
      let mut realloc = build_cabi_realloc_fn(free_index, comp);
      realloc.export_name = Some("cabi_realloc".into());
      let mut instructions = vec![
        Instruction::I32Const(HEAP_BASE),
        Instruction::I32Const(2048),
        Instruction::Call(free_index + 5),
        Instruction::LocalSet(0),
      ];
      if let Some(expected_len) = expected_len {
        instructions.extend([
          Instruction::LocalGet(0),
          Instruction::If(wasm_encoder::BlockType::Empty),
          Instruction::I32Const(2052),
          Instruction::I32Load(super::mem_arg_i32(0)),
          Instruction::I32Const(expected_len),
          Instruction::I32Eq,
          Instruction::LocalSet(1),
        ]);
        if let Some(expected_first) = expected_first {
          instructions.extend([
            Instruction::LocalGet(1),
            Instruction::I32Const(2048),
            Instruction::I32Load(super::mem_arg_i32(0)),
            Instruction::I32Load8U(super::mem_arg_byte(0)),
            Instruction::I32Const(expected_first as i32),
            Instruction::I32Eq,
            Instruction::I32And,
            Instruction::LocalSet(1),
          ]);
        }
        instructions.extend([
          Instruction::I32Const(2048),
          Instruction::I32Load(super::mem_arg_i32(0)),
          Instruction::I32Const(4 * 1024 * 1024 + 1),
          Instruction::Call(free_index),
          Instruction::End,
        ]);
      } else {
        instructions.extend([
          Instruction::LocalGet(0),
          Instruction::I32Eqz,
          Instruction::LocalSet(1),
          Instruction::LocalGet(0),
          Instruction::If(wasm_encoder::BlockType::Empty),
          Instruction::I32Const(2048),
          Instruction::I32Load(super::mem_arg_i32(0)),
          Instruction::I32Const(4 * 1024 * 1024 + 1),
          Instruction::Call(free_index),
          Instruction::End,
        ]);
      }
      instructions.extend([
        Instruction::LocalGet(1),
        Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)),
        Instruction::I32Const(0),
        Instruction::Else,
        Instruction::I32Const(1),
        Instruction::End,
        Instruction::Call(task_return),
      ]);
      let run = CompiledFn {
        export_name: Some("[async-lift-stackful]wasi:cli/run@0.3.1#run".into()),
        params: vec![],
        results: vec![],
        locals: vec![ValType::I32; 2],
        instructions,
      };
      let mut string_data = (path.len() as f64).to_le_bytes().to_vec();
      string_data.extend_from_slice(path.as_bytes());
      let heap_start = HEAP_BASE + ((string_data.len() + 7) & !7) as i32;
      let core = build_wasm_module(
        &[
          free,
          realloc,
          build_wasi_component_route_path_fn(),
          build_wasi_component_select_preopen_fn(
            *import_indices
              .get(&("wasi:filesystem/preopens@0.3.1".into(), "get-directories".into()))
              .expect("preopen import"),
            import("[resource-drop]descriptor"),
            free_index,
            free_index + 2,
          ),
          build_wasi_component_open_at_fn(
            free_index + 3,
            import("[async-lower][method]descriptor.open-at"),
            import("[resource-drop]descriptor"),
            free_index,
            free_index + 1,
            &canonical,
          ),
          build_wasi_component_read_bytes_fn(
            WasiComponentReadImports {
              source: super::runtime::WasiComponentReadSource::File {
                open: free_index + 4,
                descriptor_drop: import("[resource-drop]descriptor"),
              },
              read_via_stream: import("[method]descriptor.read-via-stream"),
              stream_read: import("[async-lower][stream-read-0][method]descriptor.read-via-stream"),
              stream_drop: import("[stream-drop-readable-0][method]descriptor.read-via-stream"),
              future_read: import("[future-read-1][method]descriptor.read-via-stream"),
              future_drop: import("[future-drop-readable-1][method]descriptor.read-via-stream"),
              free: free_index,
              realloc: free_index + 1,
            },
            &canonical,
          ),
          run,
        ],
        &imports,
        heap_start,
        &string_data,
        &[],
        1,
        ModuleFunctionLayout {
          runtime_fn_count: 6,
          number_format_fn_count: 0,
          table_fn_count: 0,
          memory_layout: WasmMemoryLayout::compute(heap_start, 0),
          global_layout,
          rest_adapters: HashMap::new(),
          prune_unused_imports: false,
        },
      )
      .expect("valid bounded file reader core command");
      let component = calcit_bindgen::package_wasi_command(&core).expect("package bounded file reader");
      let module = output.path().join("bounded-read.wasm");
      fs::write(&module, component).expect("write bounded file reader Component");
      let result = Command::new(&cli)
        .args([
          "run",
          "-S",
          "p3",
          "-W",
          "component-model-async-stackful=y",
          "-W",
          "component-model-more-async-builtins=y",
          "--dir",
        ])
        .arg(format!("{}::/workspace", host.path().display()))
        .arg(&module)
        .output()
        .expect("run bounded file reader");
      assert!(
        result.status.success(),
        "path {path:?}: {}",
        String::from_utf8_lossy(&result.stderr)
      );
    }
  }

  #[test]
  fn wasi_03_bounded_stdin_runs_with_real_wasmtime_host() {
    use std::io::Write;
    use std::process::Stdio;

    let Some(cli) = std::env::var_os("WASMTIME_CLI") else {
      return;
    };
    let output = tempfile::tempdir().expect("Component output directory");
    let mut imports = super::wasi_component_stdin_imports();
    let canonical = super::register_component_async_canonical_imports(&mut imports);
    let task_return = imports.len() as u32;
    imports.push(HostImport {
      module: "[export]wasi:cli/run@0.3.1".into(),
      name: "[task-return]run".into(),
      params: vec![ValType::I32],
      results: vec![],
    });
    let free_index = imports.len() as u32;
    for (name, data, expected_len) in [
      ("empty", vec![], Some(0)),
      ("unicode", "a😀中".as_bytes().to_vec(), Some(8)),
      ("chunks", vec![b'x'; 128 * 1024], Some(128 * 1024)),
      ("limit", vec![b'x'; 4 * 1024 * 1024], Some(4 * 1024 * 1024)),
      ("oversized", vec![b'x'; 4 * 1024 * 1024 + 1], None),
      ("host-error", vec![], None),
    ] {
      if name == "host-error" && !cfg!(unix) {
        continue;
      }
      let global_layout = WasmGlobalLayout::compute(0, true);
      let comp = global_layout.component.expect("component layout must exist");
      let mut free = build_cabi_free_fn(comp);
      free.export_name = Some("cabi_free".into());
      let mut realloc = build_cabi_realloc_fn(free_index, comp);
      realloc.export_name = Some("cabi_realloc".into());
      let mut instructions = vec![
        Instruction::I32Const(2048),
        Instruction::Call(free_index + 2),
        Instruction::LocalSet(0),
      ];
      if let Some(length) = expected_len {
        instructions.extend([
          Instruction::LocalGet(0),
          Instruction::If(wasm_encoder::BlockType::Empty),
          Instruction::I32Const(2052),
          Instruction::I32Load(super::mem_arg_i32(0)),
          Instruction::I32Const(length),
          Instruction::I32Eq,
          Instruction::LocalSet(1),
        ]);
        if data.len() <= 32 {
          for (offset, byte) in data.iter().enumerate() {
            instructions.extend([
              Instruction::LocalGet(1),
              Instruction::I32Const(2048),
              Instruction::I32Load(super::mem_arg_i32(0)),
              Instruction::I32Load8U(super::mem_arg_byte(offset as u64)),
              Instruction::I32Const(*byte as i32),
              Instruction::I32Eq,
              Instruction::I32And,
              Instruction::LocalSet(1),
            ]);
          }
        }
        instructions.extend([
          Instruction::I32Const(2048),
          Instruction::I32Load(super::mem_arg_i32(0)),
          Instruction::I32Const(4 * 1024 * 1024 + 1),
          Instruction::Call(free_index),
          Instruction::End,
        ]);
      } else {
        instructions.extend([Instruction::LocalGet(0), Instruction::I32Eqz, Instruction::LocalSet(1)]);
      }
      instructions.extend([Instruction::LocalGet(1), Instruction::I32Eqz, Instruction::Call(task_return)]);
      let run = CompiledFn {
        export_name: Some("[async-lift-stackful]wasi:cli/run@0.3.1#run".into()),
        params: vec![],
        results: vec![],
        locals: vec![ValType::I32; 2],
        instructions,
      };
      let core = build_wasm_module(
        &[
          free,
          realloc,
          build_wasi_component_read_bytes_fn(
            WasiComponentReadImports {
              source: super::runtime::WasiComponentReadSource::Stdin,
              read_via_stream: 0,
              stream_read: 1,
              stream_drop: 2,
              future_read: 3,
              future_drop: 4,
              free: free_index,
              realloc: free_index + 1,
            },
            &canonical,
          ),
          run,
        ],
        &imports,
        16384,
        &[],
        &[],
        1,
        ModuleFunctionLayout {
          runtime_fn_count: 3,
          number_format_fn_count: 0,
          table_fn_count: 0,
          memory_layout: WasmMemoryLayout::compute(16384, 0),
          global_layout,
          rest_adapters: HashMap::new(),
          prune_unused_imports: false,
        },
      )
      .expect("valid bounded stdin core command");
      let component = calcit_bindgen::package_wasi_command(&core).expect("package stdin reader");
      let module = output.path().join(format!("stdin-{name}.wasm"));
      fs::write(&module, component).expect("write stdin reader Component");
      let mut child = Command::new(&cli)
        .args([
          "run",
          "-S",
          "p3",
          "-W",
          "component-model-async-stackful=y",
          "-W",
          "component-model-more-async-builtins=y",
        ])
        .arg(&module)
        .stdin(if name == "host-error" {
          // Reading a directory as stdin fails with EISDIR on the Unix hosts
          // used by this suite. EOF alone must not turn that failure into ok.
          Stdio::from(fs::File::open(output.path()).expect("directory input descriptor"))
        } else {
          Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run stdin reader");
      let writer = child.stdin.take().map(|mut stdin| {
        std::thread::spawn(move || {
          for chunk in data.chunks(8191) {
            stdin.write_all(chunk).expect("write bounded pipe input");
          }
        })
      });
      let result = child.wait_with_output().expect("wait for stdin reader");
      if let Some(writer) = writer {
        writer.join().expect("stdin writer");
      }
      assert!(result.status.success(), "{name}: {}", String::from_utf8_lossy(&result.stderr));
    }
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
    let comp = WasmGlobalLayout::compute(0, true).component.expect("component layout must exist");
    let allocator = build_cabi_realloc_fn(9, comp);
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
    let drop_indices = BTreeMap::new();
    let enum_layouts = super::EnumLayouts::default();
    let codecs = ComponentValueCodecs {
      str_new_index: 10,
      buffer_new_index: 11,
      arena_alloc_index: 40,
      cabi_realloc_index: 30,
      enum_load_payload_index: 44,
      option_from_niche_ptr_index: 45,
      option_to_niche_ptr_index: 46,
      enum_layouts: &enum_layouts,
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
    let number_adapter = build_component_import_adapter(&number, None, &drop_indices, &codecs);
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
    let string_adapter = build_component_import_adapter(&string, None, &drop_indices, &codecs);
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
    let buffer_adapter = build_component_import_adapter(&buffer, None, &drop_indices, &codecs);
    assert_eq!(buffer_adapter.params, vec![ValType::F64]);
    assert_eq!(buffer_adapter.results, vec![ValType::F64]);
  }

  #[test]
  fn component_async_import_uses_status_waitable_and_drop_shape() {
    let list_codecs = BTreeMap::new();
    let struct_codecs = BTreeMap::new();
    let variant_codecs = BTreeMap::new();
    let drop_indices = BTreeMap::new();
    let enum_layouts = super::EnumLayouts::default();
    let codecs = ComponentValueCodecs {
      str_new_index: 10,
      buffer_new_index: 11,
      arena_alloc_index: 40,
      cabi_realloc_index: 30,
      enum_load_payload_index: 44,
      option_from_niche_ptr_index: 45,
      option_to_niche_ptr_index: 46,
      enum_layouts: &enum_layouts,
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
    let compiled = build_component_import_adapter(&adapter, Some(&canonical), &drop_indices, &codecs);
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
      field_defaults: Default::default(),
      definition_ref: Some(Arc::from("app.main/ProfileStats")),
      name: cirru_edn::EdnTag::new("ProfileStats"),
      fields: Arc::new(vec![cirru_edn::EdnTag::new("score")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let profile = Arc::new(CalcitStructDef {
      field_defaults: Default::default(),
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
      field_defaults: Default::default(),
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
    let enum_layouts = super::EnumLayouts::default();
    let codecs = ComponentValueCodecs {
      str_new_index: 10,
      buffer_new_index: 11,
      arena_alloc_index: 40,
      cabi_realloc_index: 12,
      enum_load_payload_index: 44,
      option_from_niche_ptr_index: 45,
      option_to_niche_ptr_index: 46,
      enum_layouts: &enum_layouts,
      list_codecs: &list_codecs,
      struct_codecs: &struct_codecs,
      variant_codecs: &variant_codecs,
    };
    let import_adapter = build_component_import_adapter(&import, None, &drop_indices, &codecs);
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
  fn component_stackless_tail_cancellation_drops_returned_result_and_frees_unreturned_area() {
    let adapter = ComponentExportAdapter {
      definition: "app.main/fetch".into(),
      symbol: "fetch".into(),
      target_index: 20,
      invocation: ComponentAbiInvocation::Async,
      stackless_tail_import: Some("host/fetch".into()),
      stream_consumer: None,
      task_return_index: Some(15),
      parameters: vec![],
      result: ComponentAbiType::String,
    };
    let imported = ComponentImportAdapter {
      definition: "host/fetch".into(),
      module: "host".into(),
      symbol: "fetch".into(),
      raw_index: 3,
      source_arity: 0,
      invocation: ComponentAbiInvocation::Async,
      parameters: vec![],
      result: ComponentAbiType::String,
    };
    let canonical = ComponentAsyncCanonicalImports {
      waitable_set_new: 10,
      waitable_set_wait: 11,
      waitable_set_drop: 12,
      waitable_join: 13,
      subtask_drop: 14,
    };
    let stackless = super::ComponentStacklessCanonicalImports {
      context_get: 16,
      context_set: 17,
      subtask_cancel: 18,
      task_cancel: 19,
    };
    let drop_indices = BTreeMap::from([(ComponentAbiType::String, 77u32)]);
    let [entry, callback] =
      super::build_component_stackless_tail_export(&adapter, &imported, &canonical, &stackless, 29, 30, &drop_indices, 41, 42);

    assert!(entry.instructions.iter().any(|insn| matches!(insn, Instruction::Call(41))));
    assert!(entry.instructions.iter().any(|insn| matches!(insn, Instruction::Call(42))));
    assert!(entry.instructions.iter().any(|insn| matches!(insn, Instruction::Call(77))));

    // Cancellation paths inside the stackless callback must call the type drop helper when the subtask RETURNED (2)
    // and fall back to freeing only the uninitialized result buffer via cabi_free (29) otherwise.
    let callback_drop_calls = callback
      .instructions
      .iter()
      .filter(|insn| matches!(insn, Instruction::Call(77)))
      .count();
    let callback_free_calls = callback
      .instructions
      .iter()
      .filter(|insn| matches!(insn, Instruction::Call(29)))
      .count();
    let callback_turn_enter_calls = callback
      .instructions
      .iter()
      .filter(|insn| matches!(insn, Instruction::Call(41)))
      .count();
    let callback_turn_exit_calls = callback
      .instructions
      .iter()
      .filter(|insn| matches!(insn, Instruction::Call(42)))
      .count();
    assert_eq!(
      callback_drop_calls, 3,
      "callback drops result on immediate cancel, delayed cancel, and normal completion"
    );
    assert!(
      callback_free_calls >= 5,
      "callback frees result areas and state on cancel/completion"
    );
    assert_eq!(callback_turn_enter_calls, 1, "callback re-acquires active_turns once at entry");
    assert!(
      callback_turn_exit_calls >= 2,
      "callback releases active_turns on both WAIT paths and cancellation"
    );
  }

  #[test]
  fn component_stackless_tail_wide_params_tracks_record_with_cabi_realloc_until_started_or_terminal() {
    let adapter = ComponentExportAdapter {
      definition: "app.main/wide".into(),
      symbol: "wide".into(),
      target_index: 20,
      invocation: ComponentAbiInvocation::Async,
      stackless_tail_import: Some("host/wide".into()),
      stream_consumer: None,
      task_return_index: Some(15),
      parameters: vec![ComponentAbiType::Number; 5],
      result: ComponentAbiType::String,
    };
    let imported = ComponentImportAdapter {
      definition: "host/wide".into(),
      module: "host".into(),
      symbol: "wide".into(),
      raw_index: 3,
      source_arity: 5,
      invocation: ComponentAbiInvocation::Async,
      parameters: vec![ComponentAbiType::Number; 5],
      result: ComponentAbiType::String,
    };
    let canonical = ComponentAsyncCanonicalImports {
      waitable_set_new: 10,
      waitable_set_wait: 11,
      waitable_set_drop: 12,
      waitable_join: 13,
      subtask_drop: 14,
    };
    let stackless = super::ComponentStacklessCanonicalImports {
      context_get: 16,
      context_set: 17,
      subtask_cancel: 18,
      task_cancel: 19,
    };
    let drop_indices = BTreeMap::from([(ComponentAbiType::String, 77u32)]);
    let [entry, callback] =
      super::build_component_stackless_tail_export(&adapter, &imported, &canonical, &stackless, 29, 30, &drop_indices, 41, 42);

    let entry_realloc_calls = entry
      .instructions
      .iter()
      .filter(|insn| matches!(insn, Instruction::Call(30)))
      .count();
    let entry_free_calls = entry
      .instructions
      .iter()
      .filter(|insn| matches!(insn, Instruction::Call(29)))
      .count();
    let entry_stores_param_record = entry
      .instructions
      .iter()
      .filter(|insn| matches!(insn, Instruction::I32Store(mem) if mem.offset == 16))
      .count();
    assert_eq!(
      entry_realloc_calls, 3,
      "entry allocates ret_ptr, indirect param record, and callback state via cabi_realloc (30)"
    );
    assert_eq!(
      entry_free_calls, 3,
      "entry frees ret_ptr + record on RETURNED and frees record immediately on STARTED"
    );
    assert_eq!(
      entry_stores_param_record, 2,
      "entry zeroes STATE_PARAM_RECORD (offset 16) then persists record_ptr across STARTING WAIT"
    );

    let callback_loads_param_record = callback
      .instructions
      .iter()
      .filter(|insn| matches!(insn, Instruction::I32Load(mem) if mem.offset == 16))
      .count();
    let callback_clears_param_record = callback
      .instructions
      .iter()
      .filter(|insn| matches!(insn, Instruction::I32Store(mem) if mem.offset == 16))
      .count();
    let callback_free_calls = callback
      .instructions
      .iter()
      .filter(|insn| matches!(insn, Instruction::Call(29)))
      .count();
    assert_eq!(callback_loads_param_record, 1);
    assert_eq!(
      callback_clears_param_record, 3,
      "callback clears STATE_PARAM_RECORD on cancel cleanup, STARTED event, and terminal subtask"
    );
    assert!(
      callback_free_calls >= 8,
      "callback frees param record on STARTED, non-blocked cancel, and completion"
    );
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

    let nil_join = CalcitTypeAnnotation::Nullable(
      Arc::new(CalcitTypeAnnotation::Number),
      crate::wasm::host::calcit::NullableOrigin::Inferred,
    );
    let error = component_abi_type(&nil_join, "app.main/nil-join", "logical_schema.result")
      .expect_err("an inferred nil join must not degrade into nominal Option");
    assert!(error.contains("E_COMPONENT_ABI_UNSUPPORTED_TYPE"));
  }

  #[test]
  fn nullable_wasm_payloads_require_a_nonzero_representation() {
    assert!(wasm_non_nil_value_is_nonzero(&CalcitTypeAnnotation::String));
    assert!(wasm_non_nil_value_is_nonzero(&CalcitTypeAnnotation::List(Arc::new(
      CalcitTypeAnnotation::Number
    ))));
    assert!(!wasm_non_nil_value_is_nonzero(&CalcitTypeAnnotation::Number));
    assert!(!wasm_non_nil_value_is_nonzero(&CalcitTypeAnnotation::Bool));
    assert!(!wasm_non_nil_value_is_nonzero(&CalcitTypeAnnotation::Dynamic));
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
  fn core_log_line_import_carries_channel_and_string_without_status_result() {
    let import = host_imports_for_target(WasmTarget::Core)
      .into_iter()
      .find(|import| import.module == "io" && import.name == "log_line")
      .expect("core target registers the line logger");
    assert_eq!(import.params, vec![ValType::F64; 2]);
    assert!(import.results.is_empty());
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

  #[test]
  fn enum_identity_and_collection_helper_guards_preserve_type_evidence() {
    let make_enum = |payloads: Vec<Calcit>| {
      CalcitEnumDef::from_struct(CalcitStructValue {
        struct_ref: Arc::new(CalcitStructDef::from_fields(EdnTag::from("Entry"), vec![EdnTag::from("item")])),
        values: Arc::new(vec![Calcit::from(payloads)]),
      })
      .expect("valid enum def")
    };
    let original = make_enum(vec![Calcit::Tag(EdnTag::from("string"))]);
    let equivalent = make_enum(vec![Calcit::Tag(EdnTag::from("string"))]);
    let different = make_enum(vec![Calcit::Tag(EdnTag::from("number"))]);
    let arity = make_enum(vec![Calcit::Tag(EdnTag::from("string")), Calcit::Tag(EdnTag::from("string"))]);

    assert_eq!(super::enum_identity_tag(&original), super::enum_identity_tag(&equivalent));
    assert_ne!(super::enum_identity_tag(&original), super::enum_identity_tag(&different));
    assert_ne!(super::enum_identity_tag(&original), super::enum_identity_tag(&arity));
    assert!(super::enum_identity_tag(&original) > super::enum_identity_tag(&different));
    assert!(super::enum_identity_tag(&original) < super::enum_identity_tag(&arity));

    let list_number_enum = make_enum(vec![Calcit::from(vec![
      Calcit::Symbol {
        sym: Arc::from("::"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.enum"),
          at_def: Arc::from("test"),
        }),
        location: None,
      },
      Calcit::Tag(EdnTag::from("list")),
      Calcit::Tag(EdnTag::from("number")),
    ])]);
    let list_string_enum = make_enum(vec![Calcit::from(vec![
      Calcit::Symbol {
        sym: Arc::from("::"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.enum"),
          at_def: Arc::from("test"),
        }),
        location: None,
      },
      Calcit::Tag(EdnTag::from("list")),
      Calcit::Tag(EdnTag::from("string")),
    ])]);
    assert_ne!(
      super::enum_identity_tag(&list_number_enum),
      super::enum_identity_tag(&list_string_enum)
    );

    let list_bool = Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Bool)));
    let list_dyn = Arc::new(CalcitTypeAnnotation::List(crate::wasm::host::calcit::DYNAMIC_TYPE.clone()));
    let mut ctx = WasmGenCtx::new(
      0,
      WasmCompileEnv {
        function_value_dependencies: Default::default(),
        dyn_traits: Default::default(),
        fn_index: HashMap::new(),
        fn_arity: HashMap::new(),
        fn_has_rest: HashMap::new(),
        fn_signatures: HashMap::new(),
        has_boxed_json_scalars: true,
        runtime_fn_index: HashMap::from([("__rt_copy_f64_slots".into(), 0)]),
        tag_index: HashMap::from([("list".into(), 1)]),
        struct_layouts: Default::default(),
        enum_layouts: Default::default(),
        string_pool: HashMap::new(),
        atom_globals: HashMap::new(),
        atom_is_scalar: HashMap::new(),
        value_imports: HashMap::new(),
        static_fn_defs: HashMap::from([(
          "tests.spec/check-list".into(),
          Arc::new(StaticFnDef {
            params: vec!["xs".into(), "tag".into()],
            arg_types: vec![list_dyn, crate::wasm::host::calcit::DYNAMIC_TYPE.clone()],
            callback_arities: HashMap::new(),
            fixed_arity: true,
            body: vec![Calcit::from(vec![
              Calcit::Symbol {
                sym: Arc::from("map"),
                info: Arc::new(CalcitSymbolInfo {
                  at_ns: Arc::from(crate::wasm::host::calcit::CORE_NS),
                  at_def: Arc::from("check-list"),
                }),
                location: None,
              },
              Calcit::Local(CalcitLocal {
                idx: 0,
                sym: Arc::from("xs"),
                info: Arc::new(CalcitSymbolInfo {
                  at_ns: Arc::from("tests.spec"),
                  at_def: Arc::from("check-list"),
                }),
                location: None,
                type_info: crate::wasm::host::calcit::DYNAMIC_TYPE.clone(),
              }),
              Calcit::Proc(CalcitProc::NilQuestion),
            ])],
            nil_sensitive: true,
            trait_sensitive: false,
            method_sensitive: false,
          }),
        )]),
        closures: Default::default(),
        host_imports: HashMap::new(),
        target: WasmTarget::Core,
        boundary: WasmBoundary::Native,
        world: WasiWorld::Command,
        non_coherent_nominal_defs: Default::default(),
        any_type_table: Default::default(),
      },
    );
    ctx.locals.insert("arg".into(), 0);
    ctx.local_types.insert("arg".into(), WasmTypeEvidence::contract(list_bool.clone()));
    let arg_expr = Calcit::Local(CalcitLocal {
      idx: 0,
      sym: Arc::from("arg"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.spec"),
        at_def: Arc::from("caller"),
      }),
      location: None,
      type_info: list_bool,
    });
    assert!(
      super::emit_specialized_static_call(&mut ctx, "tests.spec/check-list", &[arg_expr, Calcit::Number(1.0)])
        .expect("List<Bool> passed to List<Dynamic> parameter must preserve closed element evidence for nil?")
    );
  }

  #[test]
  fn closure_inference_and_static_predicate_respect_lexical_let_and_parameter_shadowing() {
    let mut ctx = WasmGenCtx::new(
      0,
      WasmCompileEnv {
        function_value_dependencies: Default::default(),
        dyn_traits: Default::default(),
        fn_index: HashMap::new(),
        fn_arity: HashMap::new(),
        fn_has_rest: HashMap::new(),
        fn_signatures: HashMap::new(),
        has_boxed_json_scalars: true,
        runtime_fn_index: HashMap::new(),
        tag_index: HashMap::new(),
        struct_layouts: Default::default(),
        enum_layouts: Default::default(),
        string_pool: HashMap::new(),
        atom_globals: HashMap::new(),
        atom_is_scalar: HashMap::new(),
        value_imports: HashMap::new(),
        static_fn_defs: HashMap::new(),
        closures: Default::default(),
        host_imports: HashMap::new(),
        target: WasmTarget::Core,
        boundary: WasmBoundary::Native,
        world: WasiWorld::Command,
        non_coherent_nominal_defs: Default::default(),
        any_type_table: Default::default(),
      },
    );
    let local_node = |name: &str| {
      Calcit::Local(CalcitLocal {
        idx: 0,
        sym: Arc::from(name),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.shadow"),
          at_def: Arc::from("main"),
        }),
        location: None,
        type_info: crate::wasm::host::calcit::DYNAMIC_TYPE.clone(),
      })
    };
    let make_closure = |params: &[&str], body: Calcit, captured: HashMap<String, Arc<InlineClosure>>| {
      Arc::new(InlineClosure {
        params: params.iter().map(|s| (*s).to_owned()).collect(),
        required: params.len(),
        rest: false,
        option_types: vec![],
        body: vec![body],
        captured_locals: HashMap::new(),
        captured_local_types: HashMap::new(),
        captured_closures: captured,
        ..InlineClosure::default()
      })
    };
    let outer_f = make_closure(&["x"], Calcit::Number(1.0), HashMap::new());
    ctx.lambda_locals.insert("f".into(), outer_f.clone());
    let inner_fn_form = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Defn, Arc::from(crate::wasm::host::calcit::CORE_NS)),
      Calcit::Symbol {
        sym: Arc::from("inner"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.shadow"),
          at_def: Arc::from("main"),
        }),
        location: None,
      },
      Calcit::from(vec![local_node("x")]),
      Calcit::Bool(false),
    ]);
    let shadowed_let = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::CoreLet, Arc::from(crate::wasm::host::calcit::CORE_NS)),
      Calcit::from(vec![local_node("f"), inner_fn_form]),
      Calcit::from(vec![local_node("f"), Calcit::Number(0.0)]),
    ]);
    assert_eq!(
      super::infer_wasm_static_type(&ctx, &shadowed_let).as_deref(),
      Some(&CalcitTypeAnnotation::Bool)
    );

    let bool_closure = make_closure(&["x"], Calcit::Bool(false), HashMap::new());
    ctx.lambda_locals.insert("g".into(), bool_closure);
    let h_closure = make_closure(
      &["f"],
      Calcit::from(vec![local_node("f"), Calcit::Number(0.0)]),
      HashMap::from([("f".into(), outer_f)]),
    );
    ctx.lambda_locals.insert("h".into(), h_closure);
    let param_shadowed_call = Calcit::from(vec![local_node("h"), local_node("g")]);
    assert_eq!(
      super::infer_wasm_static_type(&ctx, &param_shadowed_call).as_deref(),
      Some(&CalcitTypeAnnotation::Bool)
    );

    let nil_check = Calcit::from(vec![Calcit::Proc(CalcitProc::NilQuestion), Calcit::Nil]);
    let non_forwarding_let = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::CoreLet, Arc::from(crate::wasm::host::calcit::CORE_NS)),
      Calcit::from(vec![local_node("x"), nil_check.clone()]),
      Calcit::Bool(false),
    ]);
    assert_eq!(super::static_predicate_condition_outcome(&ctx, &non_forwarding_let, false), None);

    let and_expanded_let = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::CoreLet, Arc::from(crate::wasm::host::calcit::CORE_NS)),
      Calcit::from(vec![local_node("v"), nil_check]),
      Calcit::from(vec![
        Calcit::Syntax(CalcitSyntax::If, Arc::from(crate::wasm::host::calcit::CORE_NS)),
        local_node("v"),
        local_node("v"),
        Calcit::Bool(false),
      ]),
    ]);
    assert_eq!(
      super::static_predicate_condition_outcome(&ctx, &and_expanded_let, false),
      Some(true)
    );

    ctx
      .local_types
      .insert("v".into(), WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Nil)));
    let shadowed_or_let = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::CoreLet, Arc::from(crate::wasm::host::calcit::CORE_NS)),
      Calcit::from(vec![
        local_node("v"),
        Calcit::from(vec![Calcit::Proc(CalcitProc::NilQuestion), Calcit::Number(1.0)]),
      ]),
      Calcit::from(vec![
        Calcit::Syntax(CalcitSyntax::If, Arc::from(crate::wasm::host::calcit::CORE_NS)),
        local_node("v"),
        local_node("v"),
        Calcit::from(vec![Calcit::Proc(CalcitProc::NilQuestion), local_node("v")]),
      ]),
    ]);
    assert_eq!(
      super::static_predicate_condition_outcome(&ctx, &shadowed_or_let, false),
      Some(false)
    );

    let self_apply = make_closure(&["f"], Calcit::from(vec![local_node("f"), local_node("f")]), HashMap::new());
    ctx.lambda_locals.insert("self_apply".into(), self_apply);
    let recursive_call = Calcit::from(vec![local_node("self_apply"), local_node("self_apply")]);
    assert_eq!(
      super::infer_wasm_static_type(&ctx, &recursive_call).as_deref(),
      Some(&CalcitTypeAnnotation::Dynamic)
    );

    let guarded_self_apply = make_closure(
      &["f"],
      Calcit::from(vec![
        Calcit::Syntax(CalcitSyntax::If, Arc::from(crate::wasm::host::calcit::CORE_NS)),
        Calcit::from(vec![Calcit::Proc(CalcitProc::NumberQuestion), Calcit::Number(1.0)]),
        Calcit::Number(1.0),
        Calcit::from(vec![local_node("f"), local_node("f")]),
      ]),
      HashMap::new(),
    );
    ctx.lambda_locals.insert("guarded_self_apply".into(), guarded_self_apply);
    let guarded_call = Calcit::from(vec![local_node("guarded_self_apply"), local_node("guarded_self_apply")]);
    assert_eq!(
      super::infer_wasm_static_type(&ctx, &guarded_call).as_deref(),
      Some(&CalcitTypeAnnotation::Number)
    );
  }

  #[test]
  fn parse_float_matches_native_f64_rounding_on_long_and_halfway_inputs() {
    use super::runtime::{build_rt_bigint_mul_add, build_rt_parse_float, build_rt_parse_float_exact_midpoint};

    let decimal_mul_small = |digits: &mut Vec<u8>, mul: u32| {
      let mut carry = 0u32;
      for d in digits.iter_mut() {
        let v = (*d as u32) * mul + carry;
        *d = (v % 10) as u8;
        carry = v / 10;
      }
      while carry > 0 {
        digits.push((carry % 10) as u8);
        carry /= 10;
      }
    };
    let pow5_fraction = |coeff: u32, exp: usize| -> String {
      let mut digits = vec![(coeff % 10) as u8];
      let mut c = coeff / 10;
      while c > 0 {
        digits.push((c % 10) as u8);
        c /= 10;
      }
      for _ in 0..exp {
        decimal_mul_small(&mut digits, 5);
      }
      let frac_digits: String = digits.iter().rev().map(|d| (b'0' + d) as char).collect();
      let zeros = "0".repeat(exp - digits.len());
      format!("0.{zeros}{frac_digits}")
    };
    let overflow_midpoint = || -> String {
      let mut m = (1u64 << 54) - 1;
      let mut digits = Vec::new();
      while m > 0 {
        digits.push((m % 10) as u8);
        m /= 10;
      }
      for _ in 0..970 {
        decimal_mul_small(&mut digits, 2);
      }
      digits.iter().rev().map(|d| (b'0' + d) as char).collect()
    };

    let exact_sub_mid = pow5_fraction(1, 1075);
    let above_sub_mid = format!("{exact_sub_mid}0000000000000000000001");
    let exact_sub_mid_3 = pow5_fraction(3, 1075);
    let exact_over_mid = overflow_midpoint();
    let mut below_over_bytes = exact_over_mid.clone().into_bytes();
    let last = below_over_bytes.last_mut().expect("nonempty midpoint");
    assert_ne!(*last, b'0');
    *last -= 1;
    let below_over_mid = String::from_utf8(below_over_bytes).expect("ascii");
    let many_zeros = format!("1{}e-309", "0".repeat(309));

    let samples: Vec<String> = vec![
      "0.3".into(),
      "1.00000000000000033306690738754696212708950042724609375".into(),
      "-1.00000000000000033306690738754696212708950042724609375".into(),
      "1.00000000000000033306690738754696212708950042724609374999999999999999999999999".into(),
      "1.00000000000000011102230246251565404236316680908203125".into(),
      "1.00000000000000011102230246251565404236316680908203125000000000000000000000001".into(),
      "1.1955648945602832e-75".into(),
      "1.7976931348623157e308".into(),
      "1e75".into(),
      "5e-324".into(),
      "-5e-324".into(),
      "9007199254740993.0".into(),
      "0.000000000000000000000000012345678901234567".into(),
      many_zeros,
      exact_sub_mid,
      above_sub_mid,
      exact_sub_mid_3,
      exact_over_mid,
      below_over_mid,
    ];

    let mut parse_float = build_rt_parse_float(1);
    parse_float.export_name = Some("parse_float".into());
    let bytes = build_wasm_module(
      &[build_rt_bigint_mul_add(), build_rt_parse_float_exact_midpoint(0), parse_float],
      &[],
      HEAP_BASE,
      &[],
      &[],
      1,
      ModuleFunctionLayout {
        runtime_fn_count: 3,
        number_format_fn_count: 0,
        table_fn_count: 0,
        memory_layout: WasmMemoryLayout::compute(HEAP_BASE, 0),
        global_layout: WasmGlobalLayout::compute(0, false),
        rest_adapters: HashMap::new(),
        prune_unused_imports: false,
      },
    )
    .expect("valid parse_float module");

    let engine = Engine::default();
    let module = Module::new(&engine, &bytes).expect("valid wasm");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate");
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let parse_fn = instance
      .get_typed_func::<f64, f64>(&mut store, "parse_float")
      .expect("parse_float export");

    for input in samples {
      memory
        .write(&mut store, 64, &(input.len() as f64).to_le_bytes())
        .expect("write length");
      memory.write(&mut store, 72, input.as_bytes()).expect("write bytes");
      let got = parse_fn.call(&mut store, 64.0).expect("call parse_float");
      let expected = input.parse::<f64>().expect("native parse::<f64>");
      assert_eq!(
        got.to_bits(),
        expected.to_bits(),
        "input={input:?}: wasm={got:?} ({:#018x}) != native={expected:?} ({:#018x})",
        got.to_bits(),
        expected.to_bits()
      );
    }
  }

  #[test]
  fn closed_json_value_parse_does_not_enable_boxed_json_scalars() {
    use super::{CalcitFnArgs, program_has_boxed_json_scalars};

    let closed_body = vec![Calcit::from(vec![
      Calcit::Proc(CalcitProc::NativeTryParseJsonValue),
      Calcit::Nil,
      Calcit::Nil,
      Calcit::new_str("{}"),
    ])];
    let closed_fn_defs = vec![("app.main".to_owned(), "parse".to_owned(), CalcitFnArgs::Args(vec![]), closed_body)];
    assert!(
      !program_has_boxed_json_scalars(&closed_fn_defs, &HashMap::new()),
      "closed JsonValue parsing must not trigger boxed dynamic JSON scalar checks"
    );

    let open_body = vec![Calcit::from(vec![
      Calcit::Proc(CalcitProc::NativeTryParseJson),
      Calcit::Nil,
      Calcit::new_str("{}"),
    ])];
    let open_fn_defs = vec![("app.main".to_owned(), "parse".to_owned(), CalcitFnArgs::Args(vec![]), open_body)];
    assert!(program_has_boxed_json_scalars(&open_fn_defs, &HashMap::new()));
  }

  #[test]
  fn not_if_and_nil_handle_boxed_json_bool_and_nil_under_assert_type() {
    use super::HEAP_MAGIC;
    use super::emit_expr;
    use crate::wasm::host::calcit::NullableOrigin;

    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::from([("nil".into(), 10), ("bool".into(), 11)]),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Default::default(),
      any_type_table: Default::default(),
    };
    let local_x = |type_info: Arc<CalcitTypeAnnotation>| {
      Calcit::Local(CalcitLocal {
        idx: 0,
        sym: Arc::from("x"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.truthy"),
          at_def: Arc::from("check"),
        }),
        location: None,
        type_info,
      })
    };
    let compile_fn = |export: &str, expr: Calcit, local_ty: Arc<CalcitTypeAnnotation>| -> CompiledFn {
      let mut ctx = WasmGenCtx::new(1, env.clone());
      ctx.locals.insert("x".into(), 0);
      ctx.local_types.insert("x".into(), WasmTypeEvidence::contract(local_ty));
      emit_expr(&mut ctx, &expr).expect("emit expr");
      CompiledFn {
        export_name: Some(export.into()),
        params: vec![ValType::F64],
        results: vec![ValType::F64],
        locals: ctx.extra_locals,
        instructions: ctx.instructions,
      }
    };

    let nullable_str_ty = Arc::new(CalcitTypeAnnotation::Nullable(
      Arc::new(CalcitTypeAnnotation::String),
      NullableOrigin::Inferred,
    ));
    let sym_bool = Calcit::Symbol {
      sym: Arc::from("Bool"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.truthy"),
        at_def: Arc::from("check"),
      }),
      location: None,
    };
    let sym_nil = Calcit::Symbol {
      sym: Arc::from("Nil"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.truthy"),
        at_def: Arc::from("check"),
      }),
      location: None,
    };
    let assert_bool = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::AssertType, Arc::from(crate::wasm::host::calcit::CORE_NS)),
      local_x(crate::wasm::host::calcit::DYNAMIC_TYPE.clone()),
      sym_bool,
    ]);
    let assert_nil = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::AssertType, Arc::from(crate::wasm::host::calcit::CORE_NS)),
      local_x(crate::wasm::host::calcit::DYNAMIC_TYPE.clone()),
      sym_nil,
    ]);

    let fns = vec![
      compile_fn(
        "not_assert_bool",
        Calcit::from(vec![Calcit::Proc(CalcitProc::Not), assert_bool.clone()]),
        crate::wasm::host::calcit::DYNAMIC_TYPE.clone(),
      ),
      compile_fn(
        "if_assert_bool",
        Calcit::from(vec![
          Calcit::Syntax(CalcitSyntax::If, Arc::from(crate::wasm::host::calcit::CORE_NS)),
          assert_bool,
          Calcit::Number(1.0),
          Calcit::Number(2.0),
        ]),
        crate::wasm::host::calcit::DYNAMIC_TYPE.clone(),
      ),
      compile_fn(
        "nil_q_nullable",
        Calcit::from(vec![Calcit::Proc(CalcitProc::NilQuestion), local_x(nullable_str_ty.clone())]),
        nullable_str_ty.clone(),
      ),
    ];

    // Strict Bool enforcement rejects `not` and `if` on Nil, Nullable, and Dynamic inputs (#2881).
    for (label, expr, local_ty) in [
      (
        "not on assert-type Nil",
        Calcit::from(vec![Calcit::Proc(CalcitProc::Not), assert_nil.clone()]),
        crate::wasm::host::calcit::DYNAMIC_TYPE.clone(),
      ),
      (
        "if on assert-type Nil",
        Calcit::from(vec![
          Calcit::Syntax(CalcitSyntax::If, Arc::from(crate::wasm::host::calcit::CORE_NS)),
          assert_nil,
          Calcit::Number(1.0),
          Calcit::Number(2.0),
        ]),
        crate::wasm::host::calcit::DYNAMIC_TYPE.clone(),
      ),
      (
        "if on Nullable(String)",
        Calcit::from(vec![
          Calcit::Syntax(CalcitSyntax::If, Arc::from(crate::wasm::host::calcit::CORE_NS)),
          local_x(nullable_str_ty.clone()),
          Calcit::Number(1.0),
          Calcit::Number(2.0),
        ]),
        nullable_str_ty,
      ),
      (
        "if on Dynamic",
        Calcit::from(vec![
          Calcit::Syntax(CalcitSyntax::If, Arc::from(crate::wasm::host::calcit::CORE_NS)),
          local_x(crate::wasm::host::calcit::DYNAMIC_TYPE.clone()),
          Calcit::Number(1.0),
          Calcit::Number(2.0),
        ]),
        crate::wasm::host::calcit::DYNAMIC_TYPE.clone(),
      ),
    ] {
      let mut bad_ctx = WasmGenCtx::new(1, env.clone());
      bad_ctx.locals.insert("x".into(), 0);
      bad_ctx.local_types.insert("x".into(), WasmTypeEvidence::contract(local_ty));
      let err = emit_expr(&mut bad_ctx, &expr).expect_err(label);
      assert!(err.contains("E_NON_BOOL_CONDITION"), "{label}: {err}");
    }

    let bytes = build_wasm_module(
      &fns,
      &[],
      HEAP_BASE + 96,
      &[],
      &[],
      1,
      ModuleFunctionLayout {
        runtime_fn_count: 0,
        number_format_fn_count: 0,
        table_fn_count: 0,
        memory_layout: WasmMemoryLayout::compute(HEAP_BASE + 96, 0),
        global_layout: WasmGlobalLayout::compute(0, false),
        rest_adapters: HashMap::new(),
        prune_unused_imports: false,
      },
    )
    .expect("valid strict bool module");

    let engine = Engine::default();
    let module = Module::new(&engine, &bytes).expect("valid wasm");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate");
    let memory = instance.get_memory(&mut store, "memory").expect("memory");

    let write_boxed = |store: &mut Store<()>, offset: usize, tag: u32, payload: f64| {
      let base = HEAP_BASE as usize + offset;
      memory.write(&mut *store, base, &HEAP_MAGIC.to_le_bytes()).expect("magic");
      memory.write(&mut *store, base + 4, &tag.to_le_bytes()).expect("tag");
      memory.write(&mut *store, base + 8, &payload.to_le_bytes()).expect("payload");
      (base + 8) as f64
    };
    let boxed_nil = write_boxed(&mut store, 0, 10, 0.0);
    let boxed_false = write_boxed(&mut store, 24, 11, 0.0);
    let boxed_true = write_boxed(&mut store, 48, 11, 1.0);

    let not_bool = instance
      .get_typed_func::<f64, f64>(&mut store, "not_assert_bool")
      .expect("not_assert_bool");
    let if_bool = instance
      .get_typed_func::<f64, f64>(&mut store, "if_assert_bool")
      .expect("if_assert_bool");
    let nil_q = instance
      .get_typed_func::<f64, f64>(&mut store, "nil_q_nullable")
      .expect("nil_q_nullable");

    assert_eq!(not_bool.call(&mut store, boxed_false).unwrap(), 1.0);
    assert_eq!(not_bool.call(&mut store, boxed_true).unwrap(), 0.0);
    assert_eq!(not_bool.call(&mut store, 0.0).unwrap(), 1.0);
    assert_eq!(not_bool.call(&mut store, 1.0).unwrap(), 0.0);

    assert_eq!(if_bool.call(&mut store, boxed_false).unwrap(), 2.0);
    assert_eq!(if_bool.call(&mut store, boxed_true).unwrap(), 1.0);
    assert_eq!(if_bool.call(&mut store, 0.0).unwrap(), 2.0);
    assert_eq!(if_bool.call(&mut store, 1.0).unwrap(), 1.0);

    assert_eq!(nil_q.call(&mut store, boxed_nil).unwrap(), 1.0);
    assert_eq!(nil_q.call(&mut store, 0.0).unwrap(), 1.0);
    assert_eq!(nil_q.call(&mut store, 1.0).unwrap(), 0.0);
  }

  #[test]
  fn nullable_number_equality_allows_safe_nonzero_literal_and_rejects_zero_or_heap_range() {
    use super::validate_wasm_equality_operands;
    use crate::wasm::host::calcit::NullableOrigin;

    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::new(),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Default::default(),
      any_type_table: Default::default(),
    };
    let mut ctx = WasmGenCtx::new(0, env);
    let nullable_num_ty = Arc::new(CalcitTypeAnnotation::Nullable(
      Arc::new(CalcitTypeAnnotation::Number),
      NullableOrigin::Inferred,
    ));
    let info = Arc::new(CalcitSymbolInfo {
      at_ns: Arc::from("tests.eq"),
      at_def: Arc::from("check"),
    });
    let x_local = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("x")),
      sym: Arc::from("x"),
      info: info.clone(),
      location: None,
      type_info: nullable_num_ty.clone(),
    });
    let y_local = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("y")),
      sym: Arc::from("y"),
      info: info.clone(),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Number),
    });
    let va_idx = ctx.alloc_local();
    ctx.locals.insert("va".to_owned(), va_idx);
    ctx.local_number_literals.insert(va_idx, 1.0);
    let va_local = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("va")),
      sym: Arc::from("va"),
      info: info.clone(),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Number),
    });
    let vz_idx = ctx.alloc_local();
    ctx.locals.insert("vz".to_owned(), vz_idx);
    ctx.local_number_literals.insert(vz_idx, 0.0);
    let vz_local = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("vz")),
      sym: Arc::from("vz"),
      info,
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Number),
    });
    let dyn_form = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from("tests.eq")),
      Calcit::Symbol {
        sym: Arc::from("Dynamic"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.eq"),
          at_def: Arc::from("check"),
        }),
        location: None,
      },
    ]);
    let unsafe_wrap = |inner: Calcit| {
      Calcit::from(vec![
        Calcit::Syntax(CalcitSyntax::UnsafeCoerce, Arc::from("tests.eq")),
        inner,
        dyn_form.clone(),
      ])
    };
    ctx.local_types.insert("x".to_owned(), WasmTypeEvidence::contract(nullable_num_ty));
    ctx
      .local_types
      .insert("y".to_owned(), WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Number)));
    ctx
      .local_types
      .insert("va".to_owned(), WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Number)));
    ctx
      .local_types
      .insert("vz".to_owned(), WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Number)));

    assert_eq!(
      validate_wasm_equality_operands(&mut ctx, &Calcit::Number(1.0), &x_local, "="),
      Ok(None)
    );
    assert_eq!(
      validate_wasm_equality_operands(&mut ctx, &x_local, &Calcit::Number(-2.5), "="),
      Ok(None)
    );
    assert_eq!(
      validate_wasm_equality_operands(&mut ctx, &unsafe_wrap(va_local), &unsafe_wrap(x_local.clone()), "="),
      Ok(None)
    );

    for unsafe_rhs in [
      Calcit::Number(0.0),
      Calcit::Number(-0.0),
      Calcit::Number((HEAP_BASE + 8) as f64),
      y_local,
      unsafe_wrap(vz_local),
    ] {
      let err = validate_wasm_equality_operands(&mut ctx, &x_local, &unsafe_rhs, "=").expect_err("ambiguous equality must be rejected");
      assert!(err.contains("E_WASM_NIL_TYPE_EVIDENCE"), "unexpected error: {err}");
    }
  }

  /// Specialized `recur` back edges must preserve or drop `scalar_origin` consistently.
  #[test]
  fn recur_back_edge_validates_and_invalidates_scalar_origin() {
    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::from([
        ("__rt_str_compare".to_owned(), 0),
        ("__rt_set_find_elem".to_owned(), 1),
        ("__rt_map_equal".to_owned(), 2),
        ("__rt_value_equal".to_owned(), 3),
        ("__rt_f64_value_eq".to_owned(), 4),
      ]),
      tag_index: HashMap::from([
        ("nil".to_owned(), 0),
        ("bool".to_owned(), 1),
        ("number".to_owned(), 2),
        ("string".to_owned(), 3),
        ("keyword".to_owned(), 4),
        ("list".to_owned(), 5),
        ("map".to_owned(), 6),
        ("set".to_owned(), 7),
        ("enum".to_owned(), 8),
        ("struct".to_owned(), 9),
      ]),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Default::default(),
      any_type_table: Default::default(),
    };

    let info = Arc::new(CalcitSymbolInfo {
      at_ns: Arc::from("tests.wasm"),
      at_def: Arc::from("recur-scalar-origin"),
    });
    let dyn_num = WasmTypeEvidence {
      scalar_origin: Some(Arc::new(CalcitTypeAnnotation::Number)),
      ..WasmTypeEvidence::contract(super::DYNAMIC_TYPE.clone())
    };
    let dyn_bool = WasmTypeEvidence {
      scalar_origin: Some(Arc::new(CalcitTypeAnnotation::Bool)),
      ..dyn_num.clone()
    };

    let mut ctx = WasmGenCtx::new(2, env.clone());
    ctx.locals.insert("x".to_owned(), 0);
    ctx.locals.insert("b".to_owned(), 1);
    ctx.local_types.insert("x".to_owned(), dyn_num.clone());
    ctx.local_types.insert("b".to_owned(), dyn_bool.clone());
    ctx.recur_evidence = Some(vec![dyn_num.clone()]);

    validate_recur_evidence(&mut ctx, &[Calcit::Number(2.0)]).expect("literal Number preserves Number scalar_origin");
    let b_ref = Calcit::Symbol {
      sym: Arc::from("b"),
      info: info.clone(),
      location: None,
    };
    let err = validate_recur_evidence(&mut ctx, std::slice::from_ref(&b_ref))
      .expect_err("Bool scalar_origin back edge must reject outside selected recur loop");
    assert!(err.starts_with("E_WASM_CALL_SPECIALIZATION:"), "{err}");
    ctx.in_selected_recur_loop = true;
    let retry = validate_recur_evidence(&mut ctx, &[b_ref])
      .expect_err("Bool scalar_origin back edge must request retry inside selected recur loop");
    assert!(retry.starts_with(super::RECUR_SCALAR_ORIGIN_RETRY), "{retry}");
    assert_eq!(ctx.recur_drop_scalar_origin.len(), 1);

    // Loop with Bool scalar_origin back edge drops `scalar_origin`, so `(&= x 1)` rejects open Dynamic.
    let c_sym = Calcit::Local(CalcitLocal {
      idx: 10,
      sym: Arc::from("c"),
      info: info.clone(),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Number),
    });
    let x_sym = Calcit::Local(CalcitLocal {
      idx: 11,
      sym: Arc::from("x"),
      info: info.clone(),
      location: None,
      type_info: super::DYNAMIC_TYPE.clone(),
    });
    let b_sym = Calcit::Local(CalcitLocal {
      idx: 12,
      sym: Arc::from("b"),
      info,
      location: None,
      type_info: super::DYNAMIC_TYPE.clone(),
    });
    let bad_body = [Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.wasm")),
      Calcit::from(vec![Calcit::Proc(CalcitProc::NativeEquals), c_sym.clone(), Calcit::Number(0.0)]),
      Calcit::from(vec![Calcit::Proc(CalcitProc::Recur), Calcit::Number(1.0), b_sym]),
      Calcit::from(vec![Calcit::Proc(CalcitProc::NativeEquals), x_sym.clone(), Calcit::Number(1.0)]),
    ])];
    let mut bad_ctx = WasmGenCtx::new(3, env.clone());
    bad_ctx.locals.insert("b".to_owned(), 2);
    bad_ctx.local_types.insert("b".to_owned(), dyn_bool);
    let c_param = bad_ctx.alloc_local();
    let x_param = bad_ctx.alloc_local();
    bad_ctx.locals.insert("c".to_owned(), c_param);
    bad_ctx.locals.insert("x".to_owned(), x_param);
    let err = super::emit_selected_recur_loop(
      &mut bad_ctx,
      &["c".to_owned(), "x".to_owned()],
      vec![c_param, x_param],
      vec![WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Number)), dyn_num.clone()],
      &bad_body,
    )
    .expect_err("retry with dropped scalar_origin must reject open Dynamic equality");
    assert!(err.contains("E_WASM_NIL_TYPE_EVIDENCE"), "{err}");

    // Loop whose back edge preserves Number scalar_origin compiles cleanly.
    let good_body = [Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.wasm")),
      Calcit::from(vec![Calcit::Proc(CalcitProc::NativeEquals), c_sym, Calcit::Number(0.0)]),
      Calcit::from(vec![Calcit::Proc(CalcitProc::Recur), Calcit::Number(1.0), Calcit::Number(42.0)]),
      Calcit::from(vec![Calcit::Proc(CalcitProc::NativeEquals), x_sym, Calcit::Number(1.0)]),
    ])];
    let mut good_ctx = WasmGenCtx::new(2, env);
    let c_param = good_ctx.alloc_local();
    let x_param = good_ctx.alloc_local();
    good_ctx.locals.insert("c".to_owned(), c_param);
    good_ctx.locals.insert("x".to_owned(), x_param);
    super::emit_selected_recur_loop(
      &mut good_ctx,
      &["c".to_owned(), "x".to_owned()],
      vec![c_param, x_param],
      vec![WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Number)), dyn_num],
      &good_body,
    )
    .expect("matching Number back edge must retain scalar_origin");
  }

  /// Map equality must reject open or colliding scalar value contracts while allowing disjoint/safe shapes.
  #[test]
  fn map_equality_checks_value_scalar_collision() {
    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::new(),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Default::default(),
      any_type_table: Default::default(),
    };
    let info = Arc::new(CalcitSymbolInfo {
      at_ns: Arc::from("tests.wasm"),
      at_def: Arc::from("map-eq"),
    });
    let tag_ty = Arc::new(CalcitTypeAnnotation::Tag);
    let num_map = Arc::new(CalcitTypeAnnotation::Map(tag_ty.clone(), Arc::new(CalcitTypeAnnotation::Number)));
    let bool_map = Arc::new(CalcitTypeAnnotation::Map(tag_ty.clone(), Arc::new(CalcitTypeAnnotation::Bool)));
    let nil_map = Arc::new(CalcitTypeAnnotation::Map(tag_ty.clone(), Arc::new(CalcitTypeAnnotation::Nil)));
    let str_ty = Arc::new(CalcitTypeAnnotation::String);
    let num_ty = Arc::new(CalcitTypeAnnotation::Number);
    let dyn_map = Arc::new(CalcitTypeAnnotation::Map(tag_ty.clone(), super::DYNAMIC_TYPE.clone()));
    let tag_map = Arc::new(CalcitTypeAnnotation::Map(tag_ty.clone(), tag_ty.clone()));
    let str_map = Arc::new(CalcitTypeAnnotation::Map(str_ty.clone(), str_ty.clone()));
    let list_num = Arc::new(CalcitTypeAnnotation::List(num_ty.clone()));
    let list_dyn = Arc::new(CalcitTypeAnnotation::List(super::DYNAMIC_TYPE.clone()));
    let list_str = Arc::new(CalcitTypeAnnotation::List(str_ty.clone()));
    let option_enum = super::tests_option_enum_def_for_codec();
    let opt_str = Arc::new(CalcitTypeAnnotation::Enum(option_enum.clone(), Arc::new(vec![str_ty])));
    let opt_num = Arc::new(CalcitTypeAnnotation::Enum(option_enum.clone(), Arc::new(vec![num_ty])));
    let opt_bool = Arc::new(CalcitTypeAnnotation::Enum(
      option_enum.clone(),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Bool)]),
    ));
    let opt_never = Arc::new(CalcitTypeAnnotation::Enum(
      option_enum,
      Arc::new(vec![crate::wasm::host::calcit::type_annotation::NEVER_TYPE.clone()]),
    ));
    let list_map = Arc::new(CalcitTypeAnnotation::Map(tag_ty.clone(), list_num.clone()));
    let list_str_map = Arc::new(CalcitTypeAnnotation::Map(tag_ty, list_str.clone()));

    assert!(super::map_value_types_have_scalar_collision(
      &WasmTypeEvidence::contract(num_map.clone()),
      &WasmTypeEvidence::contract(bool_map.clone()),
    ));
    assert!(super::map_value_types_have_scalar_collision(
      &WasmTypeEvidence::contract(num_map.clone()),
      &WasmTypeEvidence::contract(nil_map.clone()),
    ));
    assert!(super::map_value_types_have_scalar_collision(
      &WasmTypeEvidence::contract(dyn_map.clone()),
      &WasmTypeEvidence::contract(num_map.clone()),
    ));
    assert!(super::map_value_types_have_scalar_collision(
      &WasmTypeEvidence::contract(dyn_map.clone()),
      &WasmTypeEvidence::contract(tag_map.clone()),
    ));
    assert!(!super::map_value_types_have_scalar_collision(
      &WasmTypeEvidence::contract(dyn_map.clone()),
      &WasmTypeEvidence::contract(list_str_map),
    ));
    assert!(!super::map_value_types_have_scalar_collision(
      &WasmTypeEvidence::contract(list_map.clone()),
      &WasmTypeEvidence::contract(num_map.clone()),
    ));

    let mut ctx = WasmGenCtx::new(9, env);
    let mk_local = |idx, name, ty| {
      Calcit::Local(CalcitLocal {
        idx,
        sym: Arc::from(name),
        info: info.clone(),
        location: None,
        type_info: ty,
      })
    };
    let m_num = mk_local(0, "m-num", num_map);
    let m_bool = mk_local(1, "m-bool", bool_map);
    let m_nil = mk_local(2, "m-nil", nil_map);
    let m_list = mk_local(3, "m-list", list_map);
    let m_dyn = mk_local(4, "m-dyn", dyn_map);
    let dyn_val = mk_local(5, "dyn-val", super::DYNAMIC_TYPE.clone());
    let l_num = mk_local(6, "l-num", list_num);
    let l_dyn = mk_local(7, "l-dyn", list_dyn);
    let l_str = mk_local(8, "l-str", list_str);
    let s_str = mk_local(9, "s-str", Arc::new(CalcitTypeAnnotation::String));
    let m_tag = mk_local(10, "m-tag", tag_map);
    let m_str = mk_local(11, "m-str", str_map);
    let o_str = mk_local(12, "o-str", opt_str);
    let o_num = mk_local(13, "o-num", opt_num);
    let o_bool = mk_local(14, "o-bool", opt_bool);
    let o_never = mk_local(15, "o-never", opt_never);

    for (lhs, rhs) in [
      (&m_num, &m_bool),
      (&m_num, &m_nil),
      (&m_dyn, &m_num),
      (&l_dyn, &l_num),
      (&dyn_val, &l_num),
      (&dyn_val, &m_tag),
      (&dyn_val, &o_num),
      (&o_bool, &o_num),
    ] {
      let err =
        super::validate_wasm_equality_operands(&mut ctx, lhs, rhs, "=").expect_err("colliding container values must be rejected");
      assert!(err.contains("E_WASM_NIL_TYPE_EVIDENCE"), "{err}");
    }
    super::validate_wasm_equality_operands(&mut ctx, &m_num, &m_num, "=").expect("homogeneous Number maps are safe");
    super::validate_wasm_equality_operands(&mut ctx, &m_list, &m_num, "=").expect("heap List values do not collide with Number values");
    super::validate_wasm_equality_operands(&mut ctx, &dyn_val, &s_str, "=").expect("Dynamic vs String is safe");
    super::validate_wasm_equality_operands(&mut ctx, &dyn_val, &l_str, "=").expect("Dynamic vs List<String> is safe");
    super::validate_wasm_equality_operands(&mut ctx, &dyn_val, &m_str, "=").expect("Dynamic vs Map<String,String> is safe");
    super::validate_wasm_equality_operands(&mut ctx, &dyn_val, &o_str, "=").expect("Dynamic vs Option<String> is safe");
    super::validate_wasm_equality_operands(&mut ctx, &o_num, &o_num, "=").expect("Option<Number> vs Option<Number> is safe");
    super::validate_wasm_equality_operands(&mut ctx, &o_never, &o_num, "=").expect("Option<Never> vs Option<Number> is safe");
  }

  #[test]
  fn list_concat_equality_checks_closed_element_types() {
    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::new(),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Default::default(),
      any_type_table: Default::default(),
    };
    let info = Arc::new(CalcitSymbolInfo {
      at_ns: Arc::from("tests.wasm"),
      at_def: Arc::from("list-concat-eq"),
    });
    let num_ty = Arc::new(CalcitTypeAnnotation::Number);
    let str_ty = Arc::new(CalcitTypeAnnotation::String);
    let bool_ty = Arc::new(CalcitTypeAnnotation::Bool);
    let list_dyn_ty = Arc::new(CalcitTypeAnnotation::List(super::DYNAMIC_TYPE.clone()));
    let tuple_num_str = Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(vec![num_ty.clone(), str_ty.clone()])));
    let tuple_num_bool = Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(vec![num_ty.clone(), bool_ty.clone()])));
    let list_num_ty = Arc::new(CalcitTypeAnnotation::List(num_ty.clone()));

    let mut ctx = WasmGenCtx::new(5, env);
    let mk_local = |idx, name, ty| {
      Calcit::Local(CalcitLocal {
        idx,
        sym: Arc::from(name),
        info: info.clone(),
        location: None,
        type_info: ty,
      })
    };

    let concat_num_str = mk_local(0, "concat-num-str", list_dyn_ty.clone());
    ctx.local_types.insert(
      "concat-num-str".to_owned(),
      WasmTypeEvidence {
        list_element_types: Some(vec![num_ty.clone(), str_ty]),
        ..WasmTypeEvidence::contract(list_dyn_ty.clone())
      },
    );
    let concat_num_bool = mk_local(1, "concat-num-bool", list_dyn_ty.clone());
    ctx.local_types.insert(
      "concat-num-bool".to_owned(),
      WasmTypeEvidence {
        list_element_types: Some(vec![num_ty, bool_ty]),
        ..WasmTypeEvidence::contract(list_dyn_ty.clone())
      },
    );
    let t_num_str = mk_local(2, "t-num-str", tuple_num_str);
    let t_num_bool = mk_local(3, "t-num-bool", tuple_num_bool);
    let l_num = mk_local(4, "l-num", list_num_ty);
    let l_open_dyn = mk_local(5, "l-open-dyn", list_dyn_ty);

    super::validate_wasm_equality_operands(&mut ctx, &concat_num_str, &t_num_str, "=")
      .expect("closed heterogeneous Number+String concat list vs Tuple<Number, String> is safe");
    super::validate_wasm_equality_operands(&mut ctx, &t_num_str, &concat_num_str, "=")
      .expect("Tuple<Number, String> vs closed heterogeneous Number+String concat list is safe");

    for (lhs, rhs) in [
      (&concat_num_bool, &t_num_bool),
      (&concat_num_bool, &l_num),
      (&concat_num_str, &l_open_dyn),
    ] {
      let err =
        super::validate_wasm_equality_operands(&mut ctx, lhs, rhs, "=").expect_err("colliding or open list equality must be rejected");
      assert!(err.contains("E_WASM_NIL_TYPE_EVIDENCE"), "{err}");
    }
  }

  #[test]
  fn lower_nil_to_option_target_preserves_effects_and_nested_nullable_if() {
    std::thread::Builder::new()
      .stack_size(64 * 1024 * 1024)
      .spawn(|| {
        let _guard = crate::wasm::host::program::lock_program_test_state();
        let core = crate::wasm::host::load_core_snapshot().expect("embedded core");
        crate::wasm::host::program::PROGRAM_CODE_DATA
          .write()
          .unwrap()
          .extend(crate::wasm::host::program::extract_program_data(&core).expect("core source"));

        let env = WasmCompileEnv {
          function_value_dependencies: Default::default(),
          dyn_traits: Default::default(),
          fn_index: HashMap::new(),
          fn_arity: HashMap::new(),
          fn_has_rest: HashMap::new(),
          fn_signatures: HashMap::new(),
          has_boxed_json_scalars: true,
          runtime_fn_index: HashMap::new(),
          tag_index: HashMap::new(),
          struct_layouts: Default::default(),
          enum_layouts: Default::default(),
          string_pool: HashMap::new(),
          atom_globals: HashMap::new(),
          atom_is_scalar: HashMap::new(),
          value_imports: HashMap::new(),
          static_fn_defs: HashMap::new(),
          closures: Default::default(),
          host_imports: HashMap::new(),
          target: WasmTarget::Core,
          boundary: WasmBoundary::Native,
          world: WasiWorld::Command,
          non_coherent_nominal_defs: Default::default(),
          any_type_table: Default::default(),
        };
        let info = Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.wasm"),
          at_def: Arc::from("lower-nil-option"),
        });
        let mk_local = |idx, name, ty| {
          Calcit::Local(CalcitLocal {
            idx,
            sym: Arc::from(name),
            info: info.clone(),
            location: None,
            type_info: ty,
          })
        };

        let ctx = WasmGenCtx::new(2, env);
        let flag = mk_local(0, "flag", Arc::new(CalcitTypeAnnotation::Bool));
        let other = mk_local(1, "other", Arc::new(CalcitTypeAnnotation::Bool));
        let maybe_num = mk_local(
          2,
          "maybe-num",
          Arc::new(CalcitTypeAnnotation::Nullable(
            Arc::new(CalcitTypeAnnotation::Number),
            crate::wasm::host::calcit::NullableOrigin::Inferred,
          )),
        );

        // 1. Literal nil lowers directly to (Option :none).
        let lowered_lit = super::lower_nil_to_option_target(&ctx, &Calcit::Nil)
          .expect("literal nil lowering should not error")
          .expect("literal nil should lower");
        assert_eq!(lowered_lit, super::nominal_option_none_expression().unwrap());

        // 2. Non-literal definitely-nil expression keeps its evaluation inside (&let () expr (Option :none)).
        let println_call = Calcit::from(vec![
          Calcit::Symbol {
            sym: Arc::from("println"),
            info: info.clone(),
            location: None,
          },
          Calcit::Str(Arc::from("seen")),
        ]);
        let effectful_nil = Calcit::from(vec![
          Calcit::Syntax(CalcitSyntax::CoreLet, Arc::from("tests.wasm")),
          Calcit::from(Vec::<Calcit>::new()),
          println_call,
          Calcit::Nil,
        ]);
        let lowered_effect = super::lower_nil_to_option_target(&ctx, &effectful_nil)
          .expect("effectful nil lowering should not error")
          .expect("effectful definitely-nil expression should lower");
        let Calcit::List(items) = lowered_effect else {
          panic!("expected CoreLet wrapper around effectful nil");
        };
        assert!(matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::CoreLet, _))));
        assert_eq!(items.get(2), Some(&effectful_nil));
        assert_eq!(items.get(3), Some(&super::nominal_option_none_expression().unwrap()));

        // 3. Nested nullable `if` lowers recursively rather than wrapping the inner `if` in Some.
        let nested_if = Calcit::from(vec![
          Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.wasm")),
          flag.clone(),
          Calcit::from(vec![
            Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.wasm")),
            other,
            Calcit::Number(1.0),
            Calcit::Nil,
          ]),
          Calcit::Nil,
        ]);
        let lowered_nested = super::lower_nil_to_option_target(&ctx, &nested_if)
          .expect("nested nullable if lowering should not error")
          .expect("nested nullable if should lower recursively");
        let Calcit::List(outer_items) = lowered_nested else {
          panic!("expected outer if");
        };
        assert!(matches!(outer_items.first(), Some(Calcit::Syntax(CalcitSyntax::If, _))));
        let Calcit::List(inner_items) = outer_items.get(2).expect("lowered then branch") else {
          panic!("expected recursive inner if rather than Option :some wrapper");
        };
        assert!(matches!(inner_items.first(), Some(Calcit::Syntax(CalcitSyntax::If, _))));
        assert_eq!(
          inner_items.get(2),
          Some(&super::nominal_option_some_expression(Calcit::Number(1.0)).unwrap())
        );
        assert_eq!(inner_items.get(3), Some(&super::nominal_option_none_expression().unwrap()));
        assert_eq!(outer_items.get(3), Some(&super::nominal_option_none_expression().unwrap()));

        // 4. An `if` whose branch is a non-lowerable Nullable local refuses partial lowering.
        let unlowerable_if = Calcit::from(vec![
          Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.wasm")),
          flag,
          maybe_num,
          Calcit::Nil,
        ]);
        assert!(
          super::lower_nil_to_option_target(&ctx, &unlowerable_if)
            .expect("nullable branch check should not error")
            .is_none()
        );
      })
      .expect("spawn test thread")
      .join()
      .expect("test thread panicked");
  }

  #[test]
  fn wasm_rejects_native_ffi_resource_explicitly() {
    let direct = CalcitTypeAnnotation::FfiResource;
    assert!(super::annotation_contains_ffi_resource(
      &direct,
      &mut std::collections::HashSet::new()
    ));
    let list_of_res = CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::FfiResource));
    assert!(super::annotation_contains_ffi_resource(
      &list_of_res,
      &mut std::collections::HashSet::new()
    ));
    let string_only = CalcitTypeAnnotation::String;
    assert!(!super::annotation_contains_ffi_resource(
      &string_only,
      &mut std::collections::HashSet::new()
    ));

    let err = component_abi_type(&direct, "regex.core/use-handle", "logical_schema.parameters[0]")
      .expect_err("Component ABI must reject native FfiResource");
    assert!(err.contains("E_COMPONENT_ABI_UNSUPPORTED_TYPE"), "err: {err}");
  }

  #[test]
  fn dyn_box_respects_impl_selector_and_assigns_distinct_vtable_ids() {
    let trait_def = Arc::new(
      crate::wasm::host::calcit::CalcitTrait::new(
        EdnTag::new("ProbeShow"),
        vec![EdnTag::new("render")],
        vec![Arc::new(CalcitTypeAnnotation::from_function_parts(
          vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("Self")))],
          Arc::new(CalcitTypeAnnotation::String),
        ))],
      )
      .with_definition_ref("tests.wasm", "ProbeShow"),
    );

    let mut left_def = CalcitStructDef::from_fields(EdnTag::new("ProbeBox"), vec![]);
    left_def.definition_ref = Some(Arc::from("tests.wasm/ProbeBox"));
    left_def.impls.push(Arc::new(CalcitImpl {
      name: EdnTag::new("ProbeShow"),
      origin: Some(trait_def.clone()),
      fields: Arc::new(vec![EdnTag::new("render")]),
      values: Arc::new(vec![Calcit::Nil]),
    }));

    let mut right_def = left_def.clone();
    right_def.impls[0] = Arc::new(CalcitImpl {
      values: Arc::new(vec![Calcit::Bool(true)]),
      ..(*left_def.impls[0]).clone()
    });

    let left_ty = Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(left_def)));
    let right_ty = Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(right_def)));

    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: false,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::from([(String::from("dyn-trait"), 10)]),
      struct_layouts: Default::default(),
      enum_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      closures: Default::default(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Arc::new(std::collections::HashSet::from([String::from("tests.wasm/ProbeBox")])),
      any_type_table: Default::default(),
    };
    let mut ctx = WasmGenCtx::new(3, env.clone());
    let sym_info = Arc::new(CalcitSymbolInfo {
      at_ns: Arc::from("tests.wasm"),
      at_def: Arc::from("box-selector"),
    });
    let make_local = |name: &str, ty: Arc<CalcitTypeAnnotation>, slot: u32| -> Calcit {
      Calcit::Local(CalcitLocal {
        idx: slot as u16,
        sym: Arc::from(name),
        info: sym_info.clone(),
        location: None,
        type_info: ty,
      })
    };
    ctx.locals.insert(String::from("flag"), 0);
    ctx.local_types.insert(
      String::from("flag"),
      WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Bool)),
    );
    ctx.locals.insert(String::from("left"), 1);
    ctx.local_types.insert(
      String::from("left"),
      WasmTypeEvidence {
        exact_impls: true,
        ..WasmTypeEvidence::contract(left_ty.clone())
      },
    );
    ctx.locals.insert(String::from("right"), 2);
    ctx.local_types.insert(
      String::from("right"),
      WasmTypeEvidence {
        exact_impls: true,
        ..WasmTypeEvidence::contract(right_ty.clone())
      },
    );

    let chosen_sym = Calcit::Symbol {
      sym: Arc::from("chosen"),
      info: sym_info.clone(),
      location: None,
    };
    let joined = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.wasm")),
      make_local("flag", Arc::new(CalcitTypeAnnotation::Bool), 0),
      make_local("left", left_ty, 1),
      make_local("right", right_ty, 2),
    ]);
    let box_call = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::DynBox, Arc::from("tests.wasm")),
      make_local("chosen", super::DYNAMIC_TYPE.clone(), 0),
      Calcit::Trait(trait_def),
    ]);
    let let_expr = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::CoreLet, Arc::from("tests.wasm")),
      Calcit::from(vec![chosen_sym, joined]),
      box_call,
    ]);
    let Calcit::List(let_list) = &let_expr else {
      unreachable!("let expression must be a list");
    };
    emit_call_expr(&mut ctx, let_list).expect("dyn:box over ImplSelector must emit cleanly");

    let reg = env.dyn_traits.borrow();
    assert_eq!(reg.trait_count_for_test(), 1);
    assert_eq!(
      reg.vtable_count_for_test(0),
      2,
      "both ImplSelector branches must register distinct vtable entries"
    );
  }
}
