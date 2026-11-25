//! Core WASM runtime helper orchestration, module assembly, and scalar/hash/equality primitives.

use super::*;
use std::sync::LazyLock;
use wasm_encoder::{BlockType, Ieee64};

mod collections;
mod json_parse;
mod numfmt;
mod strings;
mod wasi_io;

use collections::*;
use json_parse::*;
use numfmt::*;
pub(super) use numfmt::{build_rt_bigint_mul_add, build_rt_parse_float, build_rt_parse_float_exact_midpoint};
use strings::*;
pub(super) use wasi_io::*;

#[derive(Clone, Debug)]
pub(super) struct HostImport {
  pub(super) module: String,
  pub(super) name: String,
  pub(super) params: Vec<ValType>,
  pub(super) results: Vec<ValType>,
}

pub(super) struct RuntimeFns {
  pub(super) fns: Vec<CompiledFn>,
  pub(super) fn_index: HashMap<String, u32>,
  pub(super) number_format_fn_count: u32,
}

pub(super) struct ModuleFunctionLayout {
  pub(super) runtime_fn_count: u32,
  pub(super) number_format_fn_count: u32,
  pub(super) table_fn_count: u32,
  pub(super) memory_layout: WasmMemoryLayout,
  pub(super) global_layout: WasmGlobalLayout,
  /// `(call arity, table slot) -> adapter function index`. A function value
  /// is its table slot; a call with N arguments uses funcref table N (see
  /// `call_indirect`). In table N the slot of a `&` rest-parameter function
  /// points at an adapter typed `(f64 x N) -> f64` that packs the trailing
  /// arguments into the rest list (#1972); every other slot points at the
  /// function itself, so a call with the wrong arity fails the signature check.
  pub(super) rest_adapters: HashMap<(u32, u32), u32>,
  pub(super) prune_unused_imports: bool,
}

/// `call_indirect` for a function value called with `arity` arguments: the
/// canonical type `(f64 x arity) -> f64` and the funcref table for that arity.
pub(super) fn call_indirect(arity: u32) -> Instruction<'static> {
  Instruction::CallIndirect {
    type_index: arity,
    table_index: arity,
  }
}

/// List of host-imported functions.
/// These are provided by the selected host and indexed before user functions.
static CORE_HOST_IMPORTS: LazyLock<Vec<HostImport>> = LazyLock::new(|| {
  vec![
    HostImport {
      module: "math".into(),
      name: "pow".into(),
      params: vec![ValType::F64; 2],
      results: vec![ValType::F64],
    },
    HostImport {
      module: "math".into(),
      name: "sin".into(),
      params: vec![ValType::F64],
      results: vec![ValType::F64],
    },
    HostImport {
      module: "math".into(),
      name: "cos".into(),
      params: vec![ValType::F64],
      results: vec![ValType::F64],
    },
    // IO: log a single value (f64) — host reads memory to decode type
    HostImport {
      module: "io".into(),
      name: "log_value".into(),
      params: vec![ValType::F64],
      results: vec![ValType::F64],
    },
    // IO: log a string directly (ptr to heap string) — more efficient than log_value for strings
    HostImport {
      module: "io".into(),
      name: "log_str".into(),
      params: vec![ValType::F64],
      results: vec![ValType::F64],
    },
    // IO: log one formatted line; channel is 1 for stdout or 2 for stderr,
    // and the second parameter is a pointer to a tagged heap string.
    HostImport {
      module: "io".into(),
      name: "log_line".into(),
      params: vec![ValType::F64; 2],
      results: vec![],
    },
    // IO: read file contents as string (ptr to path in heap)
    HostImport {
      module: "io".into(),
      name: "read_file_str".into(),
      params: vec![ValType::F64],
      results: vec![ValType::F64],
    },
    // IO: check if file exists (ptr to path in heap) — returns 1.0 if exists, 0.0 otherwise
    HostImport {
      module: "io".into(),
      name: "file_exists".into(),
      params: vec![ValType::F64],
      results: vec![ValType::F64],
    },
    // IO: parse JSON string (ptr to JSON string in heap) — returns parsed value or nil on error
    HostImport {
      module: "io".into(),
      name: "parse_json".into(),
      params: vec![ValType::F64],
      results: vec![ValType::F64],
    },
    // IO: get current time in milliseconds
    HostImport {
      module: "io".into(),
      name: "current_time".into(),
      params: vec![],
      results: vec![ValType::F64],
    },
    // IO: get environment variable (ptr to key in heap) — returns value string or nil
    HostImport {
      module: "io".into(),
      name: "get_env".into(),
      params: vec![ValType::F64],
      results: vec![ValType::F64],
    },
  ]
});

pub(super) fn host_imports_for_target(target: WasmTarget) -> Vec<HostImport> {
  match target {
    WasmTarget::Core => CORE_HOST_IMPORTS.to_vec(),
    WasmTarget::Wasi => vec![
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "fd_write".into(),
        params: vec![ValType::I32; 4],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "args_sizes_get".into(),
        params: vec![ValType::I32; 2],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "args_get".into(),
        params: vec![ValType::I32; 2],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "environ_sizes_get".into(),
        params: vec![ValType::I32; 2],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "environ_get".into(),
        params: vec![ValType::I32; 2],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "proc_exit".into(),
        params: vec![ValType::I32],
        results: vec![],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "clock_time_get".into(),
        params: vec![ValType::I32, ValType::I64, ValType::I32],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "poll_oneoff".into(),
        params: vec![ValType::I32; 4],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "random_get".into(),
        params: vec![ValType::I32, ValType::I32],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "fd_prestat_get".into(),
        params: vec![ValType::I32, ValType::I32],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "fd_prestat_dir_name".into(),
        params: vec![ValType::I32, ValType::I32, ValType::I32],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "path_open".into(),
        params: vec![
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
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "fd_filestat_get".into(),
        params: vec![ValType::I32, ValType::I32],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "fd_read".into(),
        params: vec![ValType::I32; 4],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "fd_readdir".into(),
        params: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I64, ValType::I32],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "wasi_snapshot_preview1".into(),
        name: "fd_close".into(),
        params: vec![ValType::I32],
        results: vec![ValType::I32],
      },
    ],
  }
}

pub(super) fn core_host_import(name: &str) -> Option<&'static HostImport> {
  CORE_HOST_IMPORTS.iter().find(|import| import.name == name)
}

/// Maximum arity covered by canonical call_indirect type entries.
/// Types 0..MAX_CANONICAL_ARITY are always reserved: type N = (f64 × N) → f64.
/// A module whose `call_indirect` uses a larger arity reserves more entries.
pub(super) const MAX_CANONICAL_ARITY: u32 = 8;

/// Build a binary WASM module from compiled functions.
/// Host imports occupy the first function indices, then generated functions follow.
///
/// `layout.runtime_fn_count`: how many leading entries in `fns` are runtime helper
/// functions (not user-defined calcit functions). User functions immediately
/// after that prefix are registered in the funcref table.
/// `layout.table_fn_count` excludes target-specific entry adapters such as `_start`.
pub(super) fn build_wasm_module(
  fns: &[CompiledFn],
  host_imports: &[HostImport],
  heap_start: i32,
  string_data: &[u8],
  atom_initial_values: &[f64],
  string_tag_id: i32,
  layout: ModuleFunctionLayout,
) -> Result<Vec<u8>, String> {
  let mut module = Module::new();
  let num_imports = host_imports.len() as u32;
  let linked = link::link_module(fns, num_imports, &layout);
  let fns = &linked.fns;
  let kept_imports: Vec<&HostImport> = linked.kept_import_indices.iter().map(|&i| &host_imports[i]).collect();
  let new_num_imports = kept_imports.len() as u32;
  let used_tables = &linked.used_tables;

  // Type section (deduplicated; canonical HOF types 0..canonical_arity are only
  // emitted when at least one indirect funcref table is materialized).
  let mut types = TypeSection::new();
  let mut type_map: HashMap<(Vec<ValType>, Vec<ValType>), u32> = HashMap::new();
  let mut type_count = 0u32;
  if !used_tables.is_empty() {
    let canonical_arity = fns
      .iter()
      .flat_map(|f| f.instructions.iter())
      .filter_map(|instruction| match instruction {
        Instruction::CallIndirect { type_index, .. } | Instruction::ReturnCallIndirect { type_index, .. } => Some(*type_index + 1),
        _ => None,
      })
      .fold(MAX_CANONICAL_ARITY, u32::max);
    for arity in 0..canonical_arity {
      let sig = (vec![ValType::F64; arity as usize], vec![ValType::F64]);
      types.ty().function(sig.0.iter().copied(), sig.1.iter().copied());
      type_map.insert(sig, type_count);
      type_count += 1;
    }
  }
  let mut intern_type = |params: Vec<ValType>, results: Vec<ValType>| -> u32 {
    if let Some(&idx) = type_map.get(&(params.clone(), results.clone())) {
      return idx;
    }
    types.ty().function(params.iter().copied(), results.iter().copied());
    let idx = type_count;
    type_map.insert((params, results), idx);
    type_count += 1;
    idx
  };
  let import_type_indices: Vec<u32> = kept_imports
    .iter()
    .map(|imp| intern_type(imp.params.clone(), imp.results.clone()))
    .collect();
  let fn_type_indices: Vec<u32> = fns.iter().map(|f| intern_type(f.params.clone(), f.results.clone())).collect();
  module.section(&types);

  // Import section: kept host functions only.
  if !kept_imports.is_empty() {
    let mut imports = wasm_encoder::ImportSection::new();
    for (imp, &ty_idx) in kept_imports.iter().zip(&import_type_indices) {
      imports.import(imp.module.as_str(), imp.name.as_str(), wasm_encoder::EntityType::Function(ty_idx));
    }
    module.section(&imports);
  }

  // Function section: map each kept function to its deduplicated type index.
  let mut functions = FunctionSection::new();
  for &ty_idx in &fn_type_indices {
    functions.function(ty_idx);
  }
  module.section(&functions);

  // Table section: funcref table N serves function-value calls with N
  // arguments. Populated from reachable table indices only so pooling hosts
  // with a 1-table limit (such as `wasmtime serve`) stay compatible.
  if !used_tables.is_empty() {
    let table_count = used_tables.last().expect("non-empty used_tables") + 1;
    let mut tables = TableSection::new();
    for table in 0..table_count {
      let size = if used_tables.contains(&table) { layout.table_fn_count } else { 0 } as u64;
      tables.table(TableType {
        element_type: RefType::FUNCREF,
        minimum: size,
        maximum: Some(size),
        table64: false,
        shared: false,
      });
    }
    module.section(&tables);
  }

  // Memory section: baseline pages covering static data, optional persistent semi-spaces, and Scratch Arena.
  let mut memories = MemorySection::new();
  memories.memory(MemoryType {
    minimum: layout.memory_layout.initial_pages,
    maximum: None,
    memory64: false,
    shared: false,
    page_size_log2: None,
  });
  module.section(&memories);

  // Global section: heap pointer, atom globals, runtime metadata, optional persistent/component state, and number stack.
  debug_assert_eq!(atom_initial_values.len() as u32, layout.global_layout.atom_count);
  let mut globals = GlobalSection::new();
  globals.global(
    GlobalType {
      val_type: ValType::I32,
      mutable: true,
      shared: false,
    },
    &ConstExpr::i32_const(layout.memory_layout.arena_base),
  );
  let _ = heap_start;
  for &init_val in atom_initial_values {
    globals.global(
      GlobalType {
        val_type: ValType::F64,
        mutable: true,
        shared: false,
      },
      &ConstExpr::f64_const(init_val.into()),
    );
  }
  // string_tag
  globals.global(
    GlobalType {
      val_type: ValType::I32,
      mutable: false,
      shared: false,
    },
    &ConstExpr::i32_const(string_tag_id),
  );
  // arena_base
  globals.global(
    GlobalType {
      val_type: ValType::I32,
      mutable: false,
      shared: false,
    },
    &ConstExpr::i32_const(layout.memory_layout.arena_base),
  );
  // active_turns
  globals.global(
    GlobalType {
      val_type: ValType::I32,
      mutable: true,
      shared: false,
    },
    &ConstExpr::i32_const(0),
  );
  if layout.global_layout.native.is_some() {
    for init in [0, layout.memory_layout.arena_base, 0] {
      globals.global(
        GlobalType {
          val_type: ValType::I32,
          mutable: true,
          shared: false,
        },
        &ConstExpr::i32_const(init),
      );
    }
  }
  if layout.global_layout.persistent.is_some() {
    for init in [
      0,
      layout.memory_layout.space_a_base,
      layout.memory_layout.space_a_end,
      0,
      layout.memory_layout.space_a_base + PERSISTENT_COMPACT_THRESHOLD,
      0,
      0,
      0,
      0,
      0,
      0,
    ] {
      globals.global(
        GlobalType {
          val_type: ValType::I32,
          mutable: true,
          shared: false,
        },
        &ConstExpr::i32_const(init),
      );
    }
  }
  if layout.global_layout.component.is_some() {
    for _ in 0..4 {
      globals.global(
        GlobalType {
          val_type: ValType::I32,
          mutable: true,
          shared: false,
        },
        &ConstExpr::i32_const(0),
      );
    }
  }
  if linked.needs_number_format {
    debug_assert_eq!(globals.len(), layout.global_layout.number_stack);
    globals.global(
      GlobalType {
        val_type: ValType::I32,
        mutable: true,
        shared: false,
      },
      &ConstExpr::i32_const(number_format::NUMBER_STACK_TOP),
    );
  }
  module.section(&globals);

  // Export section: memory, heap metadata globals, and named functions
  let mut exports = ExportSection::new();
  exports.export("memory", ExportKind::Memory, 0);
  exports.export("__heap_ptr", ExportKind::Global, HEAP_PTR_GLOBAL);
  exports.export("__string_tag", ExportKind::Global, layout.global_layout.string_tag);
  exports.export("__arena_base", ExportKind::Global, layout.global_layout.arena_base);
  if let Some(pers) = &layout.global_layout.persistent {
    exports.export("__persistent_ptr", ExportKind::Global, pers.ptr);
  }
  for (i, f) in fns.iter().enumerate() {
    if let Some(export_name) = &f.export_name {
      exports.export(export_name, ExportKind::Func, new_num_imports + i as u32);
    }
  }
  module.section(&exports);

  // Element section: populate each used funcref table with linked function indices.
  if !linked.table_elements.is_empty() {
    let mut elements = ElementSection::new();
    for (table, fn_indices) in &linked.table_elements {
      elements.active(
        Some(*table),
        &ConstExpr::i32_const(0),
        Elements::Functions(fn_indices.as_slice().into()),
      );
    }
    module.section(&elements);
  }

  // Code section
  let mut codes = CodeSection::new();
  for f in fns {
    let locals: Vec<(u32, ValType)> = if f.locals.is_empty() {
      vec![]
    } else {
      // Group consecutive identical types
      let mut groups = Vec::new();
      let mut count = 1u32;
      let mut prev = f.locals[0];
      for &t in &f.locals[1..] {
        if t == prev {
          count += 1;
        } else {
          groups.push((count, prev));
          prev = t;
          count = 1;
        }
      }
      groups.push((count, prev));
      groups
    };

    let mut func = Function::new(locals);
    for instr in &f.instructions {
      func.instruction(instr);
    }
    func.instruction(&Instruction::End);
    codes.function(&func);
  }
  module.section(&codes);

  // The Number lookup table occupies reserved low memory, disjoint from the
  // Calcit string pool and bump allocator; omit it when formatting is unused.
  if linked.needs_number_format || !string_data.is_empty() {
    let mut data = wasm_encoder::DataSection::new();
    if linked.needs_number_format {
      data.active(
        0,
        &ConstExpr::i32_const(number_format::NUMBER_TABLE_BASE),
        number_format::number_table_data()?.iter().copied(),
      );
    }
    if !string_data.is_empty() {
      data.active(0, &ConstExpr::i32_const(HEAP_BASE), string_data.iter().copied());
    }
    module.section(&data);
  }

  Ok(module.finish())
}

pub(super) fn build_runtime_fns(
  base_index: u32,
  memory_layout: &WasmMemoryLayout,
  global_layout: &WasmGlobalLayout,
  boundary: WasmBoundary,
  tag_index: &HashMap<String, u32>,
  atom_is_scalar: &[bool],
  struct_layouts: &StructLayouts,
) -> Result<RuntimeFns, String> {
  let require_tag = |name: &str| *tag_index.get(name).unwrap_or_else(|| panic!("{name} tag must exist")) as i32;
  let map_tag = require_tag("map");
  let list_tag = require_tag("list");
  let string_tag = require_tag("string");
  let buffer_tag = require_tag("buffer");
  let enum_tag = require_tag("enum");
  let set_tag = require_tag("set");
  let struct_tag = require_tag("struct");
  let dyn_trait_tag = require_tag("dyn-trait");
  let nil_tag = require_tag("nil");
  let bool_tag = require_tag("bool");
  let number_tag = require_tag("number");
  let mut fn_index = HashMap::new();
  let (mut fns, _) = number_format::load_number_functions(base_index, global_layout.number_stack)?;
  let number_format_fn_count = fns.len() as u32;
  fn_index.insert(String::from("__rt_ryu_format"), base_index);

  let copy_name = String::from("__rt_copy_f64_slots");
  fn_index.insert(copy_name, base_index + fns.len() as u32);

  let i_local = 3u32;
  let copy_instructions = vec![
    Instruction::I32Const(0),
    Instruction::LocalSet(i_local),
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(i_local),
    Instruction::LocalGet(2),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(0),
    Instruction::LocalGet(i_local),
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalGet(1),
    Instruction::LocalGet(i_local),
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(i_local),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(i_local),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
  ];
  fns.push(CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32, ValType::I32],
    results: vec![],
    locals: vec![ValType::I32],
    instructions: copy_instructions,
  });

  let is_heap_ptr_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_is_heap_ptr"), is_heap_ptr_idx);
  fns.push(build_rt_is_heap_ptr_fn(memory_layout, global_layout));

  let arena_alloc_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_arena_alloc"), arena_alloc_idx);
  fns.push(build_rt_arena_alloc_fn());

  let compact_persistent_idx = if let Some(pers) = &global_layout.persistent {
    let promote_tags = PromoteTypeTags {
      string: string_tag,
      buffer: *tag_index.get("buffer").expect("buffer tag must exist") as i32,
      list: list_tag,
      buf_list: *tag_index.get("buf-list").expect("buf-list tag must exist") as i32,
      map: map_tag,
      set: set_tag,
      struct_tag,
      enum_tag,
      dyn_trait: dyn_trait_tag,
      nil: nil_tag,
      bool_tag,
      number: number_tag,
      any_box: *tag_index.get("any-box").expect("any-box tag must exist") as i32,
    };
    let fwd_grow_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__rt_fwd_grow"), fwd_grow_idx);
    fns.push(build_rt_fwd_grow_fn(pers, arena_alloc_idx));

    let fwd_get_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__rt_fwd_get"), fwd_get_idx);
    fns.push(build_rt_fwd_get_fn(pers));

    let fwd_put_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__rt_fwd_put"), fwd_put_idx);
    fns.push(build_rt_fwd_put_fn(pers, fwd_grow_idx));

    let fwd_mark_flag_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__rt_fwd_mark_flag"), fwd_mark_flag_idx);
    fns.push(build_rt_fwd_mark_flag_fn(pers, fwd_grow_idx));

    let promote_value_slot = fns.len();
    let promote_value_idx = base_index + promote_value_slot as u32;
    fn_index.insert(String::from("__rt_promote_value"), promote_value_idx);
    fns.push(build_rt_hash_f64());

    let promote_map_node_slot = fns.len();
    let promote_map_node_idx = base_index + promote_map_node_slot as u32;
    fn_index.insert(String::from("__rt_promote_map_node"), promote_map_node_idx);
    fns.push(build_rt_hash_f64());

    let relocate_refs_slot = fns.len();
    let relocate_refs_idx = base_index + relocate_refs_slot as u32;
    fn_index.insert(String::from("__rt_relocate_scratch_refs"), relocate_refs_idx);
    fns.push(build_rt_hash_f64());

    let relocate_map_node_slot = fns.len();
    let relocate_map_node_idx = base_index + relocate_map_node_slot as u32;
    fn_index.insert(String::from("__rt_relocate_scratch_map_node"), relocate_map_node_idx);
    fns.push(build_rt_hash_f64());

    let persistent_alloc_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__rt_persistent_alloc"), persistent_alloc_idx);
    fns.push(build_rt_persistent_alloc_fn(pers));

    let promote_fns = PersistentPromoteFns {
      persistent_alloc: persistent_alloc_idx,
      promote_value: promote_value_idx,
      promote_map_node: promote_map_node_idx,
      fwd_get: fwd_get_idx,
      fwd_put: fwd_put_idx,
    };
    let relocate_fns = RelocateScratchFns {
      promote_value: promote_value_idx,
      promote_map_node: promote_map_node_idx,
      fwd_get: fwd_get_idx,
      fwd_mark_flag: fwd_mark_flag_idx,
      relocate_refs: relocate_refs_idx,
      relocate_map_node: relocate_map_node_idx,
    };

    fns[promote_map_node_slot] = build_rt_promote_map_node_fn(memory_layout, pers, &promote_fns);
    fns[promote_value_slot] = build_rt_promote_value_fn(memory_layout, pers, &promote_fns, &promote_tags, struct_layouts);
    fns[relocate_map_node_slot] = build_rt_relocate_scratch_map_node_fn(memory_layout, pers, &relocate_fns);
    fns[relocate_refs_slot] = build_rt_relocate_scratch_refs_fn(memory_layout, pers, &relocate_fns, &promote_tags, struct_layouts);

    let commit_defref_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__rt_commit_defref"), commit_defref_idx);
    fns.push(build_rt_commit_defref_fn(memory_layout, pers, promote_value_idx));

    let compact_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__rt_compact_persistent"), compact_idx);
    fns.push(build_rt_compact_persistent_fn(
      memory_layout,
      global_layout,
      pers,
      promote_value_idx,
      relocate_refs_idx,
      atom_is_scalar,
    ));
    Some(compact_idx)
  } else {
    None
  };

  if boundary == WasmBoundary::Native {
    let native = global_layout.native.as_ref().expect("native turn globals must exist");
    let turn_enter_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__rt_turn_enter"), turn_enter_idx);
    fns.push(build_rt_turn_enter_fn(memory_layout, global_layout, native, compact_persistent_idx));

    let turn_end_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__rt_turn_end"), turn_end_idx);
    fns.push(build_rt_turn_end_fn(
      memory_layout,
      global_layout,
      compact_persistent_idx,
      arena_alloc_idx,
    ));

    let arena_reset_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__arena_reset"), arena_reset_idx);
    fns.push(build_arena_reset_fn(memory_layout, global_layout, native, compact_persistent_idx));
  } else if let Some(comp) = &global_layout.component {
    let turn_enter_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__rt_component_turn_enter"), turn_enter_idx);
    fns.push(build_rt_component_turn_enter_fn(memory_layout, global_layout, comp));

    let rewind_to_mark_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__rt_component_rewind_to_mark"), rewind_to_mark_idx);
    fns.push(build_rt_component_rewind_to_mark_fn(comp));

    let turn_exit_idx = base_index + fns.len() as u32;
    fn_index.insert(String::from("__rt_component_turn_exit"), turn_exit_idx);
    fns.push(build_rt_component_turn_exit_fn(
      memory_layout,
      global_layout,
      comp,
      rewind_to_mark_idx,
      compact_persistent_idx,
    ));
  }

  let map_flat_pairs_name = String::from("__rt_map_flat_pairs");
  fn_index.insert(map_flat_pairs_name, base_index + fns.len() as u32);
  let map_flat_pairs_instructions = vec![
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::I32TruncF64U,
  ];
  fns.push(CompiledFn {
    export_name: None,
    params: vec![ValType::I32],
    results: vec![ValType::I32],
    locals: vec![],
    instructions: map_flat_pairs_instructions,
  });

  // value-eq: __rt_f64_value_eq(a: f64, b: f64) → i32
  // Scalar value equality shared by every lookup: NaN equals NaN, `-0` equals `0`.
  let f64_value_eq_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_f64_value_eq"), f64_value_eq_idx);
  fns.push(build_rt_f64_value_eq());

  let map_find_key_name = String::from("__rt_map_find_key");
  fn_index.insert(map_find_key_name, base_index + fns.len() as u32);
  let map_find_key_instructions = vec![
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(5),
    Instruction::LocalGet(5),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    Instruction::I32Const(-1),
    Instruction::LocalSet(4),
    Instruction::I32Const(0),
    Instruction::LocalSet(3),
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(5),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(3),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalGet(1),
    Instruction::Call(f64_value_eq_idx),
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::LocalSet(4),
    Instruction::Br(2),
    Instruction::End,
    Instruction::LocalGet(3),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(3),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(4),
  ];
  fns.push(CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::F64],
    results: vec![ValType::I32],
    locals: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
    instructions: map_find_key_instructions,
  });

  let map_find_value_name = String::from("__rt_map_find_value");
  fn_index.insert(map_find_value_name, base_index + fns.len() as u32);
  let map_find_value_instructions = vec![
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(5),
    Instruction::LocalGet(5),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    Instruction::I32Const(-1),
    Instruction::LocalSet(4),
    Instruction::I32Const(0),
    Instruction::LocalSet(3),
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(5),
    Instruction::I32Const(16),
    Instruction::I32Add,
    Instruction::LocalGet(3),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalGet(1),
    Instruction::Call(f64_value_eq_idx),
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::LocalSet(4),
    Instruction::Br(2),
    Instruction::End,
    Instruction::LocalGet(3),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(3),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(4),
  ];
  fns.push(CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::F64],
    results: vec![ValType::I32],
    locals: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
    instructions: map_find_value_instructions,
  });

  let set_find_name = String::from("__rt_set_find_elem");
  fn_index.insert(set_find_name, base_index + fns.len() as u32);
  let set_find_slot = fns.len();
  // Filled once `__rt_value_equal` has its function index so set membership uses deep structural equality.
  fns.push(build_rt_hash_f64());

  let hash_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_hash_f64"), hash_idx);
  fns.push(build_rt_hash_f64());

  // Register string comparison before map helpers so HAMT buckets can compare
  // runtime-created string keys by content instead of allocation identity.
  let str_compare_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_compare"), str_compare_idx);
  fns.push(build_rt_str_compare());

  let map_key_hash_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_key_hash"), map_key_hash_idx);
  fn_index.insert(String::from("__rt_value_hash"), map_key_hash_idx);
  let value_hash_slot = fns.len();
  // Fill these mutually dependent helpers once the map and equality indices exist.
  fns.push(build_rt_hash_f64());

  let map_key_equal_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_key_equal"), map_key_equal_idx);
  let key_equal_slot = fns.len();
  fns.push(build_rt_hash_f64());

  let map_root_assoc_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_assoc"), map_root_assoc_idx);
  fns.push(build_rt_map_root_assoc(
    *fn_index.get("__rt_copy_f64_slots").expect("copy helper"),
    map_key_equal_idx,
  ));

  let map_root_lookup_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_lookup"), map_root_lookup_idx);
  fns.push(build_rt_map_root_lookup(map_key_equal_idx));

  let map_root_contains_value_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_contains_value"), map_root_contains_value_idx);
  fns.push(build_rt_map_root_contains_value());

  let map_root_write_pairs_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_write_pairs"), map_root_write_pairs_idx);
  fns.push(build_rt_map_root_write_pairs());

  let map_make_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_make"), map_make_idx);
  fns.push(build_rt_map_make(map_tag));

  let map_from_flat_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_from_flat"), map_from_flat_idx);
  fns.push(build_rt_map_from_flat(map_key_hash_idx, map_root_assoc_idx, map_make_idx));

  let map_from_flat_checked_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_from_flat_checked"), map_from_flat_checked_idx);
  fns.push(build_rt_map_from_flat_checked(map_key_hash_idx, map_root_assoc_idx, map_make_idx));

  let map_root_from_flat_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_from_flat"), map_root_from_flat_idx);
  fns.push(build_rt_map_root_from_flat(map_key_hash_idx, map_root_assoc_idx));

  let map_linearize_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_linearize"), map_linearize_idx);
  fns.push(build_rt_map_linearize(map_root_write_pairs_idx, list_tag));

  let map_assoc_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_assoc"), map_assoc_idx);
  fns.push(build_rt_map_assoc(map_key_hash_idx, map_root_assoc_idx, map_make_idx));

  let map_get_value_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_get_value"), map_get_value_idx);
  fns.push(build_rt_map_get_value(map_key_hash_idx, map_root_lookup_idx));

  let map_contains_key_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_contains_key"), map_contains_key_idx);
  fns.push(build_rt_map_contains_key(map_key_hash_idx, map_root_lookup_idx));

  let map_contains_value_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_contains_value"), map_contains_value_idx);
  fns.push(build_rt_map_contains_value(map_root_contains_value_idx));

  let map_dissoc_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_dissoc"), map_dissoc_idx);
  fns.push(build_rt_map_dissoc(
    *fn_index.get("__rt_copy_f64_slots").expect("copy helper"),
    map_linearize_idx,
    map_from_flat_idx,
    map_key_equal_idx,
  ));

  // Substring search helper: __rt_str_find_index(h_ptr: i32, n_ptr: i32) → f64
  // Returns Unicode scalar index of first occurrence, or -1.0 if not found.
  let str_find_index_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_find_index"), str_find_index_idx);
  fns.push(build_rt_str_find_index());

  // Prefix check: __rt_str_starts_with(s_ptr: i32, p_ptr: i32) → f64
  // Returns 1.0 if s starts with p, else 0.0.
  let str_starts_with_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_starts_with"), str_starts_with_idx);
  fns.push(build_rt_str_starts_with());

  // Suffix check: __rt_str_ends_with(s_ptr: i32, suf_ptr: i32) → f64
  // Returns 1.0 if s ends with suf, else 0.0.
  let str_ends_with_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_ends_with"), str_ends_with_idx);
  fns.push(build_rt_str_ends_with());

  // Number-to-string: __rt_f64_to_str(value: f64) → i32 (string logical ptr)
  let f64_to_str_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_f64_to_str"), f64_to_str_idx);
  fns.push(build_rt_f64_to_str(string_tag, fn_index["__rt_ryu_format"]));

  // Radix display: __rt_display_by(value: f64, radix: f64) → f64 (string logical ptr as f64)
  let display_by_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_display_by"), display_by_idx);
  fns.push(build_rt_display_by(string_tag));

  // Trim whitespace: __rt_trim_ws(s: f64) → f64 (trimmed string ptr)
  let trim_ws_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_trim_ws"), trim_ws_idx);
  fns.push(build_rt_trim_ws(string_tag));

  // Trim char: __rt_trim_char(s: f64, c: f64) → f64 (trimmed string ptr)
  let trim_char_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_trim_char"), trim_char_idx);
  fns.push(build_rt_trim_char(string_tag));

  // blank?: __rt_blank(s: f64) → f64 (1.0 if blank, 0.0 otherwise)
  let blank_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_blank"), blank_idx);
  fns.push(build_rt_blank());

  // Bigint helper and exact midpoint comparator for parse-float fallback.
  let bigint_mul_add_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_bigint_mul_add"), bigint_mul_add_idx);
  fns.push(build_rt_bigint_mul_add());

  let parse_float_exact_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_parse_float_exact_midpoint"), parse_float_exact_idx);
  fns.push(build_rt_parse_float_exact_midpoint(bigint_mul_add_idx));

  // parse-float: __rt_parse_float(s: f64) → f64 (parsed number)
  let parse_float_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_parse_float"), parse_float_idx);
  fns.push(build_rt_parse_float(parse_float_exact_idx));

  // &parse-float validity: __rt_parse_float_valid(s: f64) → f64 (1.0 when the text is a float literal)
  let parse_float_valid_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_parse_float_valid"), parse_float_valid_idx);
  fns.push(build_rt_parse_float_valid());

  // char-from-code: __rt_char_from_code(cp: f64) → f64 (single-char string ptr)
  let char_from_code_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_char_from_code"), char_from_code_idx);
  fns.push(build_rt_char_from_code(string_tag));

  // str-replace: __rt_str_replace(s:f64, pat:f64, rep:f64) → f64 (new string ptr)
  let str_replace_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_replace"), str_replace_idx);
  fns.push(build_rt_str_replace(string_tag));

  // str-escape: __rt_str_escape(s:f64) → f64 (new string ptr with escaped chars)
  let str_escape_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_escape"), str_escape_idx);
  fns.push(build_rt_str_escape(string_tag));

  // Cirru EDN string token: __rt_edn_string(s:f64) → f64.
  let edn_string_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_edn_string"), edn_string_idx);
  fns.push(build_rt_edn_string(string_tag));

  // JSON string token: __rt_json_string(s:f64) → f64.
  let json_string_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_json_string"), json_string_idx);
  fns.push(build_rt_json_string(string_tag));

  // Tag lookup from string pointer: __rt_tag_from_str(str_ptr:i32) → f64.
  let tag_from_str_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_tag_from_str"), tag_from_str_idx);
  fns.push(build_rt_tag_from_str(tag_index));

  // JSON string parser: __rt_json_parse_str_at(src_ptr:i32, pos:i32, end:i32) → (str_ptr:i32, next_pos:i32, ok:i32).
  let json_parse_str_at_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_json_parse_str_at"), json_parse_str_at_idx);
  fns.push(build_rt_json_parse_str_at(string_tag));

  // JSON recursive value parser: __rt_json_parse_at(src_ptr:i32, pos:i32, end:i32, depth:i32) → (val:f64, next_pos:i32, ok:i32).
  let json_parse_at_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_json_parse_at"), json_parse_at_idx);
  fns.push(build_rt_json_parse_at(
    (string_tag, list_tag, map_tag, nil_tag, bool_tag, number_tag),
    (
      json_parse_at_idx,
      json_parse_str_at_idx,
      tag_from_str_idx,
      map_from_flat_checked_idx,
      parse_float_valid_idx,
      parse_float_idx,
    ),
  ));

  // JSON root parser: __rt_json_parse_root(s:f64) → (val:f64, ok:i32).
  let json_parse_root_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_json_parse_root"), json_parse_root_idx);
  fns.push(build_rt_json_parse_root(json_parse_at_idx));

  // Closed JsonValue recursive parser: __rt_json_parse_value_at(src_ptr:i32, pos:i32, end:i32, depth:i32) → (val:f64, next_pos:i32, ok:i32).
  let json_parse_value_at_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_json_parse_value_at"), json_parse_value_at_idx);
  let json_value_tags = JsonValueEnumTags {
    enum_tag,
    null_variant: require_tag("null") as u32,
    bool_variant: require_tag("bool") as u32,
    number_variant: require_tag("number") as u32,
    string_variant: require_tag("string") as u32,
    array_variant: require_tag("array") as u32,
    object_variant: require_tag("object") as u32,
    map_key_equal_idx,
  };
  fns.push(build_rt_json_parse_value_at(
    (string_tag, list_tag, map_tag, nil_tag, bool_tag, number_tag),
    (
      json_parse_value_at_idx,
      json_parse_str_at_idx,
      tag_from_str_idx,
      map_from_flat_checked_idx,
      parse_float_valid_idx,
      parse_float_idx,
    ),
    json_value_tags,
  ));

  // Closed JsonValue root parser: __rt_json_parse_value_root(s:f64) → (val:f64, ok:i32).
  let json_parse_value_root_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_json_parse_value_root"), json_parse_value_root_idx);
  fns.push(build_rt_json_parse_root(json_parse_value_at_idx));

  // str-find-from: __rt_str_find_from(h_ptr: i32, h_start: i32, pat_ptr: i32) → i32
  // Returns byte offset of first occurrence at or after h_start, or -1.
  let str_find_from_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_find_from"), str_find_from_idx);
  fns.push(build_rt_str_find_from());

  // utf8-char-len: __rt_utf8_char_len(b: i32) → i32
  // Returns byte width of a UTF-8 character given its first byte.
  let utf8_char_len_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_utf8_char_len"), utf8_char_len_idx);
  fns.push(build_rt_utf8_char_len());

  // str-split: __rt_str_split(s_ptr: i32, pat_ptr: i32) → i32 (list ptr)
  let str_split_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_split"), str_split_idx);
  fns.push(build_rt_str_split(string_tag, list_tag, str_find_from_idx, utf8_char_len_idx));

  // struct field codecs: __rt_struct_load_field(ptr, offset, kind) -> f64,
  // __rt_struct_store_field(ptr, offset, kind, val) -> ()
  let struct_load_field_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_struct_load_field"), struct_load_field_idx);
  fns.push(build_rt_struct_load_field());

  let struct_store_field_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_struct_store_field"), struct_store_field_idx);
  fns.push(build_rt_struct_store_field(is_heap_ptr_idx, bool_tag));

  // value-equal: __rt_value_equal(a: f64, b: f64) → i32 (1=equal, 0=not)
  // Deep equality for strings, buffers, containers, and nominal payloads.
  let value_tags = ValueRuntimeTags {
    string: string_tag,
    buffer: buffer_tag,
    list: list_tag,
    enum_tag,
    map: map_tag,
    set: set_tag,
    struct_tag,
    dyn_trait: dyn_trait_tag,
    nil: nil_tag,
    bool_tag,
    number: number_tag,
    any_box: *tag_index.get("any-box").expect("any-box tag must exist") as i32,
  };
  let value_equal_idx = base_index + fns.len() as u32;
  let map_equal_idx = value_equal_idx + 1;
  let set_equal_idx = value_equal_idx + 2;
  fn_index.insert(String::from("__rt_value_equal"), value_equal_idx);
  fns.push(build_rt_value_equal(
    value_tags,
    (
      str_compare_idx,
      value_equal_idx,
      map_equal_idx,
      set_equal_idx,
      is_heap_ptr_idx,
      struct_load_field_idx,
    ),
    struct_layouts,
  ));

  // Map values can contain nested Options and Enums, so use the same deep comparator.
  fn_index.insert(String::from("__rt_map_equal"), map_equal_idx);
  fns.push(build_rt_map_equal(
    map_linearize_idx,
    map_key_hash_idx,
    map_root_lookup_idx,
    value_equal_idx,
  ));

  fn_index.insert(String::from("__rt_set_equal"), set_equal_idx);
  fns.push(build_rt_set_equal(value_equal_idx));

  fns[set_find_slot] = build_rt_set_find_elem(value_equal_idx);
  fns[value_hash_slot] = build_rt_value_hash(
    value_tags,
    (
      hash_idx,
      map_key_hash_idx,
      map_linearize_idx,
      is_heap_ptr_idx,
      struct_load_field_idx,
    ),
    struct_layouts,
  );
  fns[key_equal_slot] = build_rt_map_key_equal(value_equal_idx);

  Ok(RuntimeFns {
    fns,
    fn_index,
    number_format_fn_count,
  })
}

pub(super) struct RuntimeFnBuilder {
  locals: Vec<ValType>,
  next_local: u32,
  pub(super) instructions: Vec<Instruction<'static>>,
}

impl RuntimeFnBuilder {
  pub(super) fn new(param_count: u32) -> Self {
    Self {
      locals: Vec::new(),
      next_local: param_count,
      instructions: Vec::new(),
    }
  }

  pub(super) fn alloc_i32(&mut self) -> u32 {
    let idx = self.next_local;
    self.next_local += 1;
    self.locals.push(ValType::I32);
    idx
  }

  pub(super) fn alloc_i64(&mut self) -> u32 {
    let idx = self.next_local;
    self.next_local += 1;
    self.locals.push(ValType::I64);
    idx
  }

  pub(super) fn alloc_f64(&mut self) -> u32 {
    let idx = self.next_local;
    self.next_local += 1;
    self.locals.push(ValType::F64);
    idx
  }

  pub(super) fn emit(&mut self, instr: Instruction<'static>) {
    self.instructions.push(instr);
  }

  pub(super) fn finish(self, params: Vec<ValType>, results: Vec<ValType>) -> CompiledFn {
    self.finish_exported(None, params, results)
  }

  pub(super) fn finish_exported(self, export_name: Option<String>, params: Vec<ValType>, results: Vec<ValType>) -> CompiledFn {
    CompiledFn {
      export_name,
      params,
      results,
      locals: self.locals,
      instructions: self.instructions,
    }
  }
}

pub(super) fn rt_emit_alloc_const(builder: &mut RuntimeFnBuilder, byte_size: i32, dst_local: u32, type_tag: i32) {
  let raw = builder.alloc_i32();
  builder.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  builder.emit(Instruction::LocalTee(raw));
  builder.emit(Instruction::I32Const(HEAP_MAGIC));
  builder.emit(Instruction::I32Store(mem_arg_i32(0)));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(4));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::I32Const(type_tag));
  builder.emit(Instruction::I32Store(mem_arg_i32(0)));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(dst_local));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(byte_size + 8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
}

pub(super) fn rt_emit_alloc_dynamic(builder: &mut RuntimeFnBuilder, size_local: u32, dst_local: u32, type_tag: i32) {
  let raw = builder.alloc_i32();
  builder.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  builder.emit(Instruction::LocalTee(raw));
  builder.emit(Instruction::I32Const(HEAP_MAGIC));
  builder.emit(Instruction::I32Store(mem_arg_i32(0)));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(4));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::I32Const(type_tag));
  builder.emit(Instruction::I32Store(mem_arg_i32(0)));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(dst_local));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalGet(size_local));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
}

/// Ensures linear memory covers `HEAP_PTR_GLOBAL + needed_bytes`, growing memory
/// if needed, and pushes `1` (`i32`) on success or `0` if capacity is exhausted.
pub(super) fn rt_emit_ensure_heap_capacity(builder: &mut RuntimeFnBuilder, needed_bytes_local: u32) {
  let end = builder.alloc_i64();
  let desired_pages = builder.alloc_i32();
  let current_pages = builder.alloc_i32();
  let ok = builder.alloc_i32();

  builder.emit(Instruction::I32Const(0));
  builder.emit(Instruction::LocalSet(ok));
  builder.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  builder.emit(Instruction::I64ExtendI32U);
  builder.emit(Instruction::LocalGet(needed_bytes_local));
  builder.emit(Instruction::I64ExtendI32U);
  builder.emit(Instruction::I64Add);
  builder.emit(Instruction::LocalTee(end));
  builder.emit(Instruction::I64Const(i64::from(u32::MAX)));
  builder.emit(Instruction::I64LeU);
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::LocalGet(end));
  builder.emit(Instruction::I64Const(65_535));
  builder.emit(Instruction::I64Add);
  builder.emit(Instruction::I64Const(16));
  builder.emit(Instruction::I64ShrU);
  builder.emit(Instruction::I32WrapI64);
  builder.emit(Instruction::LocalSet(desired_pages));
  builder.emit(Instruction::MemorySize(0));
  builder.emit(Instruction::LocalTee(current_pages));
  builder.emit(Instruction::LocalGet(desired_pages));
  builder.emit(Instruction::I32LtU);
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::LocalGet(desired_pages));
  builder.emit(Instruction::LocalGet(current_pages));
  builder.emit(Instruction::I32Sub);
  builder.emit(Instruction::MemoryGrow(0));
  builder.emit(Instruction::I32Const(-1));
  builder.emit(Instruction::I32Ne);
  builder.emit(Instruction::LocalSet(ok));
  builder.emit(Instruction::Else);
  builder.emit(Instruction::I32Const(1));
  builder.emit(Instruction::LocalSet(ok));
  builder.emit(Instruction::End);
  builder.emit(Instruction::End);
  builder.emit(Instruction::LocalGet(ok));
}

pub(super) fn rt_emit_ensure_heap_capacity_const(builder: &mut RuntimeFnBuilder, needed_bytes: i32, scratch_local: u32) {
  builder.emit(Instruction::I32Const(needed_bytes));
  builder.emit(Instruction::LocalSet(scratch_local));
  rt_emit_ensure_heap_capacity(builder, scratch_local);
}

pub(super) fn build_rt_f64_value_eq() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.finish(vec![ValType::F64, ValType::F64], vec![ValType::I32])
}

pub(super) fn build_rt_hash_f64() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  // Equality identifies -0 with +0, so their hashes must also agree.
  b.emit(Instruction::LocalGet(0));
  b.emit(f64_const(0.0));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  // Every NaN equals every other NaN, so all NaN payloads share one hash.
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x7ff8_0000u32 as i32));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I64ReinterpretF64);
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::I32Const(0x9e37_79b9u32 as i32));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Rotl);
  b.finish(vec![ValType::F64], vec![ValType::I32])
}

/// Hash the same recursive value structure that participates in equality.
/// The numeric hash is backend-local; equal values must share a hash.
/// Runtime helper: load a compact Struct field from `(ptr, offset, kind)` and return as `f64`.
pub(super) fn build_rt_struct_load_field() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3);
  let addr = b.alloc_i32();
  // params: 0 = ptr (i32), 1 = offset (i32), 2 = kind (i32)
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(StructFieldKind::UnitZero as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(f64_const(0.0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(addr));

  let emit_case = |b: &mut RuntimeFnBuilder, kinds: &[i32], instrs: &[Instruction<'static>]| {
    for (idx, &k) in kinds.iter().enumerate() {
      b.emit(Instruction::LocalGet(2));
      b.emit(Instruction::I32Const(k));
      b.emit(Instruction::I32Eq);
      if idx > 0 {
        b.emit(Instruction::I32Or);
      }
    }
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(addr));
    for instr in instrs {
      b.emit(instr.clone());
    }
    b.emit(Instruction::Return);
    b.emit(Instruction::End);
  };

  emit_case(&mut b, &[StructFieldKind::BoxedF64 as i32], &[Instruction::F64Load(mem_arg_f64(0))]);
  emit_case(
    &mut b,
    &[StructFieldKind::BoolU8 as i32, StructFieldKind::UInt8 as i32],
    &[Instruction::I32Load8U(mem_arg_byte(0)), Instruction::F64ConvertI32U],
  );
  emit_case(
    &mut b,
    &[
      StructFieldKind::TagU32 as i32,
      StructFieldKind::UInt32 as i32,
      StructFieldKind::HeapPtrI32 as i32,
    ],
    &[Instruction::I32Load(mem_arg_i32(0)), Instruction::F64ConvertI32U],
  );
  emit_case(
    &mut b,
    &[StructFieldKind::Int8 as i32],
    &[Instruction::I32Load8S(mem_arg_byte(0)), Instruction::F64ConvertI32S],
  );
  emit_case(
    &mut b,
    &[StructFieldKind::Int16 as i32],
    &[Instruction::I32Load16S(mem_arg_i16(0)), Instruction::F64ConvertI32S],
  );
  emit_case(
    &mut b,
    &[StructFieldKind::UInt16 as i32],
    &[Instruction::I32Load16U(mem_arg_i16(0)), Instruction::F64ConvertI32U],
  );
  emit_case(
    &mut b,
    &[StructFieldKind::Int32 as i32],
    &[Instruction::I32Load(mem_arg_i32(0)), Instruction::F64ConvertI32S],
  );
  emit_case(
    &mut b,
    &[StructFieldKind::Int64 as i32],
    &[Instruction::I64Load(mem_arg_i64(0)), Instruction::F64ConvertI64S],
  );
  emit_case(
    &mut b,
    &[StructFieldKind::UInt64 as i32],
    &[Instruction::I64Load(mem_arg_i64(0)), Instruction::F64ConvertI64U],
  );
  b.emit(Instruction::Unreachable);
  b.finish(vec![ValType::I32, ValType::I32, ValType::I32], vec![ValType::F64])
}

/// Runtime helper: store `val: f64` into a compact Struct field at `(ptr, offset, kind)`.
pub(super) fn build_rt_struct_store_field(is_heap_ptr_idx: u32, bool_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(4);
  let addr = b.alloc_i32();
  let boxed_ptr = b.alloc_i32();
  // params: 0 = ptr (i32), 1 = offset (i32), 2 = kind (i32), 3 = val (f64)
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(StructFieldKind::UnitZero as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(addr));

  let emit_case = |b: &mut RuntimeFnBuilder, kinds: &[i32], val_conv: &[Instruction<'static>], store: Instruction<'static>| {
    for (idx, &k) in kinds.iter().enumerate() {
      b.emit(Instruction::LocalGet(2));
      b.emit(Instruction::I32Const(k));
      b.emit(Instruction::I32Eq);
      if idx > 0 {
        b.emit(Instruction::I32Or);
      }
    }
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(addr));
    b.emit(Instruction::LocalGet(3));
    for instr in val_conv {
      b.emit(instr.clone());
    }
    b.emit(store);
    b.emit(Instruction::Return);
    b.emit(Instruction::End);
  };

  emit_case(
    &mut b,
    &[StructFieldKind::BoxedF64 as i32],
    &[],
    Instruction::F64Store(mem_arg_f64(0)),
  );
  // Unbox heap-tagged JSON Bool scalars so boxed `false` stores `0` instead of non-zero ptr.
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(StructFieldKind::BoolU8 as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::Call(is_heap_ptr_idx));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalTee(boxed_ptr));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::I32Const(bool_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(boxed_ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(3));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(3));
  b.emit(f64_const(0.0));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  emit_case(
    &mut b,
    &[
      StructFieldKind::TagU32 as i32,
      StructFieldKind::UInt32 as i32,
      StructFieldKind::HeapPtrI32 as i32,
    ],
    &[Instruction::I32TruncF64U],
    Instruction::I32Store(mem_arg_i32(0)),
  );
  emit_case(
    &mut b,
    &[StructFieldKind::Int8 as i32],
    &[Instruction::I32TruncF64S],
    Instruction::I32Store8(mem_arg_byte(0)),
  );
  emit_case(
    &mut b,
    &[StructFieldKind::UInt8 as i32],
    &[Instruction::I32TruncF64U],
    Instruction::I32Store8(mem_arg_byte(0)),
  );
  emit_case(
    &mut b,
    &[StructFieldKind::Int16 as i32],
    &[Instruction::I32TruncF64S],
    Instruction::I32Store16(mem_arg_i16(0)),
  );
  emit_case(
    &mut b,
    &[StructFieldKind::UInt16 as i32],
    &[Instruction::I32TruncF64U],
    Instruction::I32Store16(mem_arg_i16(0)),
  );
  emit_case(
    &mut b,
    &[StructFieldKind::Int32 as i32],
    &[Instruction::I32TruncF64S],
    Instruction::I32Store(mem_arg_i32(0)),
  );
  emit_case(
    &mut b,
    &[StructFieldKind::Int64 as i32],
    &[Instruction::I64TruncF64S],
    Instruction::I64Store(mem_arg_i64(0)),
  );
  emit_case(
    &mut b,
    &[StructFieldKind::UInt64 as i32],
    &[Instruction::I64TruncF64U],
    Instruction::I64Store(mem_arg_i64(0)),
  );
  b.emit(Instruction::Unreachable);
  b.finish(vec![ValType::I32, ValType::I32, ValType::I32, ValType::F64], vec![])
}

#[derive(Clone, Copy)]
pub(super) struct ValueRuntimeTags {
  pub(super) string: i32,
  pub(super) buffer: i32,
  pub(super) list: i32,
  pub(super) enum_tag: i32,
  pub(super) map: i32,
  pub(super) set: i32,
  pub(super) struct_tag: i32,
  pub(super) dyn_trait: i32,
  pub(super) nil: i32,
  pub(super) bool_tag: i32,
  pub(super) number: i32,
  pub(super) any_box: i32,
}

pub(super) fn build_rt_value_hash(
  tags: ValueRuntimeTags,
  (scalar_hash_idx, self_idx, map_linearize_idx, is_heap_ptr_idx, struct_load_field_idx): (u32, u32, u32, u32, u32),
  struct_layouts: &StructLayouts,
) -> CompiledFn {
  let ValueRuntimeTags {
    string: string_tag,
    buffer: buffer_tag,
    list: list_tag,
    enum_tag,
    map: map_tag,
    set: set_tag,
    struct_tag,
    dyn_trait: dyn_trait_tag,
    nil: nil_tag,
    bool_tag,
    number: number_tag,
    any_box: any_box_tag,
  } = tags;
  let mut b = RuntimeFnBuilder::new(1);
  let ptr = b.alloc_i32();
  let tag = b.alloc_i32();
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let hash = b.alloc_i32();
  let item_hash = b.alloc_i32();
  let layout_id = b.alloc_i32();
  let desc_ptr = b.alloc_i32();
  let meta_ptr = b.alloc_i32();
  let packed = b.alloc_i32();
  let field_kind = b.alloc_i32();
  let field_val = b.alloc_f64();

  // Check range and active regions before converting: negative numbers, NaN, infinities,
  // and numbers colliding with inactive semi-space addresses are treated as scalars.
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(is_heap_ptr_idx));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(ptr));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(tag));
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(nil_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const((nil_tag as u32).wrapping_mul(0x0100_0193) as i32));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(bool_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::Call(scalar_hash_idx));
  b.emit(Instruction::I32Const((bool_tag as u32).wrapping_mul(0x0100_0193) as i32));
  b.emit(Instruction::I32Xor);
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(number_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::Call(scalar_hash_idx));
  b.emit(Instruction::I32Const((number_tag as u32).wrapping_mul(0x0100_0193) as i32));
  b.emit(Instruction::I32Xor);
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(dyn_trait_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::I32Const(super::dyn_trait::DYN_TRAIT_ID_MASK as i32));
  b.emit(Instruction::I32And);
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::Call(scalar_hash_idx));
  b.emit(Instruction::I32Const(0x0100_0193));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::Call(scalar_hash_idx));
  b.emit(Instruction::I32Xor);
  b.emit(Instruction::I32Const(0x0100_0193));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::I32Const(super::dyn_trait::DYN_TRAIT_HEAP_FLAG as i32));
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::Call(self_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::Call(scalar_hash_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::I32Xor);
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  for (index, kind) in [
    string_tag,
    buffer_tag,
    list_tag,
    enum_tag,
    struct_tag,
    set_tag,
    map_tag,
    any_box_tag,
  ]
  .into_iter()
  .enumerate()
  {
    b.emit(Instruction::LocalGet(tag));
    b.emit(Instruction::I32Const(kind));
    b.emit(Instruction::I32Eq);
    if index > 0 {
      b.emit(Instruction::I32Or);
    }
  }
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(scalar_hash_idx));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  // Strings and Buffers share `[byte_len: f64 @ 0][bytes: u8 @ 8..]` and hash by byte content,
  // not allocation identity. Mix `buffer_tag` into Buffer hashes so identical bytes don't collide with Strings.
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(string_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(buffer_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0x811c_9dc5u32 as i32));
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Xor);
  b.emit(Instruction::I32Const(0x0100_0193));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(buffer_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::I32Const((buffer_tag as u32).wrapping_mul(0x0100_0193) as i32));
  b.emit(Instruction::I32Xor);
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  // AnyBox: `[type_id:u32 @ 0][flags:u32 @ 4][payload:f64 @ 8]`
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(any_box_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const((any_box_tag as u32).wrapping_mul(0x0100_0193) as i32));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::Call(scalar_hash_idx));
  b.emit(Instruction::I32Xor);
  b.emit(Instruction::I32Const(0x0100_0193));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::Call(self_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::Call(scalar_hash_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::I32Xor);
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  // Compact Struct: `[count:u32 @ 0][layout_id:u32 @ 4]` + descriptor table fields
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(struct_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::LocalSet(hash));
  if struct_layouts.max_layout_id > 0 {
    b.emit(Instruction::LocalGet(ptr));
    b.emit(Instruction::I32Load(mem_arg_i32(4)));
    b.emit(Instruction::LocalTee(layout_id));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32LtU);
    b.emit(Instruction::LocalGet(layout_id));
    b.emit(Instruction::I32Const(struct_layouts.max_layout_id as i32));
    b.emit(Instruction::I32GtU);
    b.emit(Instruction::I32Or);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::Unreachable);
    b.emit(Instruction::End);

    b.emit(Instruction::LocalGet(hash));
    b.emit(Instruction::LocalGet(layout_id));
    b.emit(Instruction::F64ConvertI32U);
    b.emit(Instruction::Call(scalar_hash_idx));
    b.emit(Instruction::I32Xor);
    b.emit(Instruction::LocalSet(hash));

    b.emit(Instruction::I32Const(struct_layouts.desc_table_base));
    b.emit(Instruction::LocalGet(layout_id));
    b.emit(Instruction::I32Const(12));
    b.emit(Instruction::I32Mul);
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalTee(desc_ptr));
    b.emit(Instruction::I32Load(mem_arg_i32(4)));
    b.emit(Instruction::LocalSet(count));
    b.emit(Instruction::LocalGet(desc_ptr));
    b.emit(Instruction::I32Load(mem_arg_i32(8)));
    b.emit(Instruction::LocalSet(meta_ptr));

    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::LocalSet(i));
    b.emit(Instruction::Block(BlockType::Empty));
    b.emit(Instruction::Loop(BlockType::Empty));
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::LocalGet(count));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::BrIf(1));
    structs::push_load_struct_field_meta(&mut b.instructions, meta_ptr, i);
    b.emit(Instruction::LocalSet(packed));
    structs::push_extract_struct_field_kind(&mut b.instructions, packed);
    b.emit(Instruction::LocalSet(field_kind));
    b.emit(Instruction::LocalGet(ptr));
    structs::push_extract_struct_field_offset(&mut b.instructions, packed);
    b.emit(Instruction::LocalGet(field_kind));
    b.emit(Instruction::Call(struct_load_field_idx));
    b.emit(Instruction::LocalSet(field_val));
    // Compact scalar fields hold non-heap values; hash via scalar_hash without probing the heap table.
    b.emit(Instruction::LocalGet(field_kind));
    b.emit(Instruction::I32Const(StructFieldKind::BoxedF64 as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::LocalGet(field_kind));
    b.emit(Instruction::I32Const(StructFieldKind::HeapPtrI32 as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::I32Or);
    b.emit(Instruction::If(BlockType::Result(ValType::I32)));
    b.emit(Instruction::LocalGet(field_val));
    b.emit(Instruction::Call(self_idx));
    b.emit(Instruction::Else);
    b.emit(Instruction::LocalGet(field_val));
    b.emit(Instruction::Call(scalar_hash_idx));
    b.emit(Instruction::End);
    b.emit(Instruction::LocalSet(item_hash));
    b.emit(Instruction::LocalGet(hash));
    b.emit(Instruction::I32Const(0x0100_0193));
    b.emit(Instruction::I32Mul);
    b.emit(Instruction::LocalGet(item_hash));
    b.emit(Instruction::I32Xor);
    b.emit(Instruction::LocalSet(hash));
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(i));
    b.emit(Instruction::Br(0));
    b.emit(Instruction::End);
    b.emit(Instruction::End);
  }
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  for (index, kind) in [list_tag, enum_tag, set_tag, map_tag].into_iter().enumerate() {
    b.emit(Instruction::LocalGet(tag));
    b.emit(Instruction::I32Const(kind));
    b.emit(Instruction::I32Eq);
    if index > 0 {
      b.emit(Instruction::I32Or);
    }
  }
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(enum_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::Call(scalar_hash_idx));
  b.emit(Instruction::I32Xor);
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(map_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::Call(map_linearize_idx));
  b.emit(Instruction::LocalSet(ptr));
  b.emit(Instruction::End);

  // Lists and nominal Enum payloads are ordered. Sets and map entries are unordered.
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::Call(self_idx));
  b.emit(Instruction::LocalSet(item_hash));
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(map_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(item_hash));
  b.emit(Instruction::I32Const(0x0100_0193));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(16)));
  b.emit(Instruction::Call(self_idx));
  b.emit(Instruction::I32Xor);
  b.emit(Instruction::LocalSet(item_hash));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(ptr));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(list_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(enum_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::I32Const(0x0100_0193));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::LocalGet(item_hash));
  b.emit(Instruction::I32Xor);
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(ptr));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(scalar_hash_idx));
  b.finish(vec![ValType::F64], vec![ValType::I32])
}

/// `__rt_value_equal(a: f64, b: f64) → i32`
///
/// Deep structural equality:
/// - Numbers/booleans/nil: f64 equality.
/// - Strings and Buffers: byte-by-byte comparison via __rt_str_compare (tags already matched).
/// - Lists: element-wise recursive comparison.
/// - Enums (including Option/Result): variant and payload comparison.
/// - Structs: definition/layout identity and recursive field comparison.
/// - Maps and Sets: recursive structural comparison.
/// - Other heap objects: pointer (f64) equality.
fn build_rt_value_equal(
  tags: ValueRuntimeTags,
  (str_compare_idx, self_idx, map_equal_idx, set_equal_idx, is_heap_ptr_idx, struct_load_field_idx): (u32, u32, u32, u32, u32, u32),
  struct_layouts: &StructLayouts,
) -> CompiledFn {
  let ValueRuntimeTags {
    string: string_tag,
    buffer: buffer_tag,
    list: list_tag,
    enum_tag,
    map: map_tag,
    set: set_tag,
    struct_tag,
    dyn_trait: dyn_trait_tag,
    nil: nil_tag,
    bool_tag,
    number: number_tag,
    any_box: any_box_tag,
  } = tags;
  // params: 0 = a (f64), 1 = b (f64)
  let mut b = RuntimeFnBuilder::new(2);
  let result = b.alloc_i32(); // 0=not-equal, 1=equal
  let ptr_a = b.alloc_i32();
  let ptr_b = b.alloc_i32();
  let tag_a = b.alloc_i32();
  let tag_b = b.alloc_i32();
  let cnt = b.alloc_i32();
  let i = b.alloc_i32();
  let elem_a = b.alloc_f64();
  let elem_b = b.alloc_f64();
  let layout_id = b.alloc_i32();
  let desc_ptr = b.alloc_i32();
  let meta_ptr = b.alloc_i32();
  let packed = b.alloc_i32();
  let field_kind = b.alloc_i32();

  // Fast path: scalar value equality (NaN equals NaN, `-0` equals `0`)
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Else);

  // Check both are valid live heap pointers across active regions
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(is_heap_ptr_idx));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(is_heap_ptr_idx));
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));

  // Convert to i32 pointers
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(ptr_a));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(ptr_b));

  // Read type tags (ptr - 4 = raw_base + 4)
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(tag_a));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(tag_b));

  // If different types → not equal
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::LocalGet(tag_b));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Else);

  // Same type: dispatch on tag

  // String and Buffer comparison (both share `[byte_len: f64 @ 0][bytes: u8 @ 8..]`; tag_a == tag_b already holds)
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(string_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(buffer_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  // str_compare returns f64: 0.0 = equal
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::Call(str_compare_idx));
  b.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::Else);

  // List deep comparison
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(list_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  {
    // count_a == count_b?
    b.emit(Instruction::LocalGet(ptr_a));
    b.emit(Instruction::F64Load(mem_arg_f64(0)));
    b.emit(Instruction::I32TruncF64U);
    b.emit(Instruction::LocalSet(cnt));
    b.emit(Instruction::LocalGet(ptr_b));
    b.emit(Instruction::F64Load(mem_arg_f64(0)));
    b.emit(Instruction::I32TruncF64U);
    b.emit(Instruction::LocalGet(cnt));
    b.emit(Instruction::I32Ne);
    b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::Else);
    // Compare each element recursively
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::LocalSet(result));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::LocalSet(i));
    // block (break-out-on-mismatch)
    b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
    // exit when i >= cnt
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::LocalGet(cnt));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::BrIf(1)); // break out of block
    // elem_a = ptr_a[8 + i*8]
    b.emit(Instruction::LocalGet(ptr_a));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Mul);
    b.emit(Instruction::I32Add);
    b.emit(Instruction::F64Load(mem_arg_f64(0)));
    b.emit(Instruction::LocalSet(elem_a));
    // elem_b = ptr_b[8 + i*8]
    b.emit(Instruction::LocalGet(ptr_b));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Mul);
    b.emit(Instruction::I32Add);
    b.emit(Instruction::F64Load(mem_arg_f64(0)));
    b.emit(Instruction::LocalSet(elem_b));
    // if !value_equal(elem_a, elem_b) → result=0, break
    b.emit(Instruction::LocalGet(elem_a));
    b.emit(Instruction::LocalGet(elem_b));
    b.emit(Instruction::Call(self_idx));
    b.emit(Instruction::I32Eqz);
    b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::LocalSet(result));
    b.emit(Instruction::Br(2)); // break out of block
    b.emit(Instruction::End);
    // i++
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(i));
    b.emit(Instruction::Br(0)); // continue loop
    b.emit(Instruction::End); // loop
    b.emit(Instruction::End); // block
    b.emit(Instruction::LocalGet(result));
    b.emit(Instruction::End); // count_ne if
  }

  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(enum_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  // Compact Enum layout: `[count:u32 @ 0][tag_id:u32 @ 4][payload0:f64 @ 8]...`
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(cnt));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(cnt));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(cnt));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(elem_a));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(elem_b));
  b.emit(Instruction::LocalGet(elem_a));
  b.emit(Instruction::LocalGet(elem_b));
  b.emit(Instruction::Call(self_idx));
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
  b.emit(Instruction::End); // loop
  b.emit(Instruction::End); // block
  b.emit(Instruction::LocalGet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::End); // enum count and variant match
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(struct_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  if struct_layouts.max_layout_id == 0 {
    b.emit(Instruction::I32Const(0));
  } else {
    b.emit(Instruction::LocalGet(ptr_a));
    b.emit(Instruction::I32Load(mem_arg_i32(4)));
    b.emit(Instruction::LocalTee(layout_id));
    b.emit(Instruction::LocalGet(ptr_b));
    b.emit(Instruction::I32Load(mem_arg_i32(4)));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::LocalGet(layout_id));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::I32And);
    b.emit(Instruction::LocalGet(layout_id));
    b.emit(Instruction::I32Const(struct_layouts.max_layout_id as i32));
    b.emit(Instruction::I32LeU);
    b.emit(Instruction::I32And);
    b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
    b.emit(Instruction::I32Const(struct_layouts.desc_table_base));
    b.emit(Instruction::LocalGet(layout_id));
    b.emit(Instruction::I32Const(12));
    b.emit(Instruction::I32Mul);
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalTee(desc_ptr));
    b.emit(Instruction::I32Load(mem_arg_i32(4)));
    b.emit(Instruction::LocalSet(cnt));
    b.emit(Instruction::LocalGet(desc_ptr));
    b.emit(Instruction::I32Load(mem_arg_i32(8)));
    b.emit(Instruction::LocalSet(meta_ptr));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::LocalSet(result));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::LocalSet(i));
    b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::LocalGet(cnt));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::BrIf(1));
    structs::push_load_struct_field_meta(&mut b.instructions, meta_ptr, i);
    b.emit(Instruction::LocalSet(packed));
    structs::push_extract_struct_field_kind(&mut b.instructions, packed);
    b.emit(Instruction::LocalSet(field_kind));
    b.emit(Instruction::LocalGet(ptr_a));
    structs::push_extract_struct_field_offset(&mut b.instructions, packed);
    b.emit(Instruction::LocalGet(field_kind));
    b.emit(Instruction::Call(struct_load_field_idx));
    b.emit(Instruction::LocalSet(elem_a));
    b.emit(Instruction::LocalGet(ptr_b));
    structs::push_extract_struct_field_offset(&mut b.instructions, packed);
    b.emit(Instruction::LocalGet(field_kind));
    b.emit(Instruction::Call(struct_load_field_idx));
    b.emit(Instruction::LocalSet(elem_b));
    b.emit(Instruction::LocalGet(field_kind));
    b.emit(Instruction::I32Const(StructFieldKind::BoxedF64 as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::LocalGet(field_kind));
    b.emit(Instruction::I32Const(StructFieldKind::HeapPtrI32 as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::I32Or);
    b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
    b.emit(Instruction::LocalGet(elem_a));
    b.emit(Instruction::LocalGet(elem_b));
    b.emit(Instruction::Call(self_idx));
    b.emit(Instruction::Else);
    // Packed scalar fields hold non-heap values; compare without probing the heap table.
    b.emit(Instruction::LocalGet(elem_a));
    b.emit(Instruction::LocalGet(elem_b));
    b.emit(Instruction::F64Eq);
    b.emit(Instruction::LocalGet(elem_a));
    b.emit(Instruction::LocalGet(elem_a));
    b.emit(Instruction::F64Ne);
    b.emit(Instruction::LocalGet(elem_b));
    b.emit(Instruction::LocalGet(elem_b));
    b.emit(Instruction::F64Ne);
    b.emit(Instruction::I32And);
    b.emit(Instruction::I32Or);
    b.emit(Instruction::End);
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
    b.emit(Instruction::End); // loop
    b.emit(Instruction::End); // block
    b.emit(Instruction::LocalGet(result));
    b.emit(Instruction::Else);
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(map_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::Call(map_equal_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(set_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::Call(set_equal_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(nil_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(bool_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(number_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(any_box_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  // Compare canonical TypeId first (`Int32` vs `Number`, `false` vs `0`)
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::Call(self_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::LocalSet(elem_a));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::LocalSet(elem_b));
  b.emit(Instruction::LocalGet(elem_a));
  b.emit(Instruction::LocalGet(elem_b));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::LocalGet(elem_a));
  b.emit(Instruction::LocalGet(elem_a));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::LocalGet(elem_b));
  b.emit(Instruction::LocalGet(elem_b));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(dyn_trait_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::LocalSet(elem_a));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::LocalSet(elem_b));
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::I32Const(super::dyn_trait::DYN_TRAIT_HEAP_FLAG as i32));
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(elem_a));
  b.emit(Instruction::LocalGet(elem_b));
  b.emit(Instruction::Call(self_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(elem_a));
  b.emit(Instruction::LocalGet(elem_b));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::End); // dyn_trait if
  b.emit(Instruction::End); // any_box if
  b.emit(Instruction::End); // number if
  b.emit(Instruction::End); // bool if
  b.emit(Instruction::End); // nil if
  b.emit(Instruction::End); // set if
  b.emit(Instruction::End); // map if
  b.emit(Instruction::End); // struct if
  b.emit(Instruction::End); // enum if
  b.emit(Instruction::End); // list if

  b.emit(Instruction::End); // string if
  b.emit(Instruction::End); // tag_ne if
  b.emit(Instruction::Else);
  // Not both heap pointers → not equal
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::End); // both_heap if

  b.emit(Instruction::End); // fast-path if

  b.finish(vec![ValType::F64, ValType::F64], vec![ValType::I32])
}
