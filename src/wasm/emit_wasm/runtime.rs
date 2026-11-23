use super::*;
use std::sync::LazyLock;
use wasm_encoder::{BlockType, Ieee64};

const WASI_PREOPEN_FD_LIMIT: i32 = 64;
const WASI_PREOPEN_NAME_LIMIT: i32 = 4096;
const WASI_GUEST_PATH_LIMIT: i32 = 4096;
pub(super) const WASI_TEXT_FILE_LIMIT: i64 = 4 * 1024 * 1024;
const WASI_DIR_BUFFER_SIZE: i32 = 64 * 1024;
const WASI_DIR_ENTRY_LIMIT: i32 = 4096;
const WASI_DIR_NAME_LIMIT: i32 = 4096;
const WASI_DIR_PATH_BYTES_LIMIT: i32 = 4 * 1024 * 1024;
// Covers allocation headers, the list, read buffer, padded strings, FsPath
// wrappers, and the successful Result value at the public boundary.
const WASI_DIR_MANAGED_BYTES_LIMIT: i64 =
  56 + WASI_DIR_BUFFER_SIZE as i64 + WASI_DIR_PATH_BYTES_LIMIT as i64 + 63 * WASI_DIR_ENTRY_LIMIT as i64;

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
}

/// `call_indirect` for a function value called with `arity` arguments: the
/// canonical type `(f64 x arity) -> f64` and the funcref table for that arity.
pub(super) fn call_indirect(arity: u32) -> Instruction<'static> {
  Instruction::CallIndirect {
    type_index: arity,
    table_index: arity,
  }
}

#[derive(Clone, Copy)]
pub(super) enum WasiComponentReadSource {
  File { open: u32, descriptor_drop: u32 },
  Stdin,
}

pub(super) struct WasiComponentReadImports {
  pub(super) source: WasiComponentReadSource,
  pub(super) read_via_stream: u32,
  pub(super) stream_read: u32,
  pub(super) stream_drop: u32,
  pub(super) future_read: u32,
  pub(super) future_drop: u32,
  pub(super) free: u32,
  pub(super) realloc: u32,
}

pub(super) struct WasiComponentWriteImports {
  pub(super) open: u32,
  pub(super) stream_new: u32,
  pub(super) stream_write: u32,
  pub(super) stream_drop: u32,
  pub(super) write_via_stream: u32,
  pub(super) future_read: u32,
  pub(super) future_drop: u32,
  pub(super) descriptor_drop: u32,
  pub(super) free: u32,
  pub(super) realloc: u32,
}

pub(super) struct WasiComponentReadDirImports {
  pub(super) open: u32,
  pub(super) read_directory: u32,
  pub(super) stream_read: u32,
  pub(super) stream_drop: u32,
  pub(super) future_read: u32,
  pub(super) future_drop: u32,
  pub(super) descriptor_drop: u32,
  pub(super) utf8_valid: u32,
  pub(super) str_compare: u32,
  pub(super) free: u32,
  pub(super) realloc: u32,
  pub(super) list_tag: i32,
  pub(super) string_tag: i32,
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

/// Build the internal Preview 1 writer used by Calcit console operations.
/// The iovec and result word use the 16-byte scratch area below `HEAP_BASE`.
pub(super) fn build_wasi_write_all_fn(fd_write_idx: u32) -> CompiledFn {
  // params: 0=fd, 1=remaining byte pointer, 2=remaining byte length
  // local: 3=bytes written by the current call
  let instructions = vec![
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(2),
    Instruction::I32Eqz,
    Instruction::BrIf(1),
    // ciovec { buf: 0, buf_len: 4 }, nwritten at 8.
    Instruction::I32Const(0),
    Instruction::LocalGet(1),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::I32Const(4),
    Instruction::LocalGet(2),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(0),
    Instruction::I32Const(0),
    Instruction::I32Const(1),
    Instruction::I32Const(8),
    Instruction::Call(fd_write_idx),
    // A host error or a zero-byte successful write cannot make progress.
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::I32Const(8),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalTee(3),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(1),
    Instruction::LocalGet(3),
    Instruction::I32Add,
    Instruction::LocalSet(1),
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32Sub,
    Instruction::LocalSet(2),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
  ];
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32; 3],
    results: vec![],
    locals: vec![ValType::I32],
    instructions,
  }
}

/// Build the Preview 1 relative monotonic-clock adapter used by `wait-ms`.
/// Returns zero on a single successful clock event and a non-zero status for
/// host errors or malformed event records.
pub(super) fn build_wasi_wait_fn(poll_oneoff_idx: u32) -> CompiledFn {
  // param: 0=relative duration in nanoseconds
  let mut b = RuntimeFnBuilder::new(1);
  let scratch_size = b.alloc_i64();
  let managed_size = b.alloc_i64();
  let scratch = b.alloc_i32();
  let event = b.alloc_i32();
  let nevents = b.alloc_i32();
  let status = b.alloc_i32();

  b.emit(Instruction::I64Const(88));
  b.emit(Instruction::LocalSet(scratch_size));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::LocalSet(managed_size));
  rt_emit_reserve_scratch(&mut b, scratch_size, managed_size, scratch);
  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::I32Const(48));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(event));
  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::I32Const(80));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(nevents));

  // __wasi_subscription_t: userdata, tagged union, then clock payload.
  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::I64Const(0x4341_4c43_4954_5741));
  b.emit(Instruction::I64Store(mem_arg_i64(0)));
  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Store8(mem_arg_byte(8)));
  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Store(mem_arg_i32(16)));
  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I64Store(mem_arg_i64(24)));
  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::I64Const(1_000_000));
  b.emit(Instruction::I64Store(mem_arg_i64(32)));
  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Store16(mem_arg_byte(40)));

  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::LocalGet(event));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalGet(nevents));
  b.emit(Instruction::Call(poll_oneoff_idx));
  b.emit(Instruction::LocalSet(status));

  b.emit(Instruction::LocalGet(status));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(nevents));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::LocalGet(event));
  b.emit(Instruction::I64Load(mem_arg_i64(0)));
  b.emit(Instruction::I64Const(0x4341_4c43_4954_5741));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(event));
  b.emit(Instruction::I32Load16U(mem_arg_byte(8)));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(event));
  b.emit(Instruction::I32Load8U(mem_arg_byte(10)));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(status));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(status));
  b.finish(vec![ValType::I64], vec![ValType::I32])
}

/// Resolve a relative Calcit path against the most specific Preview 1 preopen
/// and open it without exposing either the preopen or opened descriptor.
/// Returns -1 for validation, authority, or host failures.
pub(super) fn build_wasi_open_path_fn(fd_prestat_get_idx: u32, fd_prestat_dir_name_idx: u32, path_open_idx: u32) -> CompiledFn {
  // params: path logical pointer, oflags, rights_base
  let mut b = RuntimeFnBuilder::new(3);
  let path_data = b.alloc_i32();
  let path_len = b.alloc_i32();
  let index = b.alloc_i32();
  let segment_start = b.alloc_i32();
  let byte = b.alloc_i32();
  let scratch_size = b.alloc_i64();
  let managed_size = b.alloc_i64();
  let scratch = b.alloc_i32();
  let fd = b.alloc_i32();
  let name_len = b.alloc_i32();
  let name_start = b.alloc_i32();
  let normalized_len = b.alloc_i32();
  let compare_index = b.alloc_i32();
  let matched = b.alloc_i32();
  let best_prefix = b.alloc_i32();
  let best_fd = b.alloc_i32();
  let candidate_rel_ptr = b.alloc_i32();
  let candidate_rel_len = b.alloc_i32();
  let best_rel_ptr = b.alloc_i32();
  let best_rel_len = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(path_data));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalTee(path_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  // Absolute paths are never reinterpreted relative to a preopen.
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  // Reject NUL and every complete `..` component before consulting authority.
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(segment_start));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalTee(byte));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(segment_start));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::LocalGet(segment_start));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::LocalGet(segment_start));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(1)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(segment_start));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::I64Const(WASI_PREOPEN_NAME_LIMIT as i64));
  b.emit(Instruction::LocalSet(scratch_size));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::LocalSet(managed_size));
  rt_emit_reserve_scratch(&mut b, scratch_size, managed_size, scratch);
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(best_prefix));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(best_fd));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::LocalSet(fd));

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::I32Const(WASI_PREOPEN_FD_LIMIT));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Call(fd_prestat_get_idx));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalTee(name_len));
  b.emit(Instruction::I32Const(WASI_PREOPEN_NAME_LIMIT));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::Call(fd_prestat_dir_name_idx));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(name_start));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(name_start));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::LocalGet(name_start));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(normalized_len));

  // A guest preopen named `.` grants relative paths. Named roots require an
  // exact first-component match and the longest matching root wins.
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(matched));
  b.emit(Instruction::LocalGet(normalized_len));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::LocalGet(name_start));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(matched));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(normalized_len));
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::LocalSet(candidate_rel_ptr));
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::LocalSet(candidate_rel_len));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(matched));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(normalized_len));
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::LocalGet(normalized_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(matched));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(compare_index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(compare_index));
  b.emit(Instruction::LocalGet(normalized_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::LocalGet(compare_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(scratch));
  b.emit(Instruction::LocalGet(name_start));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(compare_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(matched));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(compare_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(compare_index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::LocalGet(normalized_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(candidate_rel_ptr));
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::LocalGet(normalized_len));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(candidate_rel_len));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(matched));
  b.emit(Instruction::LocalGet(normalized_len));
  b.emit(Instruction::LocalGet(best_prefix));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(normalized_len));
  b.emit(Instruction::LocalSet(best_prefix));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::LocalSet(best_fd));
  b.emit(Instruction::LocalGet(candidate_rel_ptr));
  b.emit(Instruction::LocalSet(best_rel_ptr));
  b.emit(Instruction::LocalGet(candidate_rel_len));
  b.emit(Instruction::LocalSet(best_rel_len));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(fd));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(best_fd));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(best_fd));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(best_rel_ptr));
  b.emit(Instruction::LocalGet(best_rel_len));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Call(path_open_idx));
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::End);
  b.finish(vec![ValType::I32, ValType::I32, ValType::I64], vec![ValType::I32])
}

/// Return the byte offset after the matching preopen prefix, or -1 for a denied path.
/// The caller passes a Calcit string pointer and a Canonical ABI preopen name.
pub(super) fn build_wasi_component_route_path_fn() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3);
  let path_data = b.alloc_i32();
  let path_len = b.alloc_i32();
  let index = b.alloc_i32();
  let segment_start = b.alloc_i32();
  let byte = b.alloc_i32();
  let name_ptr = b.alloc_i32();
  let name_len = b.alloc_i32();
  let fail = |b: &mut RuntimeFnBuilder| {
    b.emit(Instruction::I32Const(-1));
    b.emit(Instruction::Return);
  };

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(path_len));
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32Const(WASI_GUEST_PATH_LIMIT));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  fail(&mut b);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(path_data));
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  fail(&mut b);
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(segment_start));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalTee(byte));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail(&mut b);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(segment_start));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::LocalGet(segment_start));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::LocalGet(segment_start));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(1)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  fail(&mut b);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(segment_start));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalSet(name_ptr));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalSet(name_len));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Const(WASI_PREOPEN_NAME_LIMIT));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  fail(&mut b);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(name_ptr));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(name_ptr));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(name_ptr));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(name_len));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(name_ptr));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail(&mut b);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  fail(&mut b);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(path_data));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(name_ptr));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  fail(&mut b);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.finish(vec![ValType::I32; 3], vec![ValType::I32])
}

/// Select the longest matching preopen and transfer only its owned descriptor.
/// All other descriptors and Canonical ABI allocations are released here.
pub(super) fn build_wasi_component_select_preopen_fn(
  get_directories_idx: u32,
  drop_descriptor_idx: u32,
  free_idx: u32,
  route_path_idx: u32,
) -> CompiledFn {
  // params: Calcit path pointer, output tuple { descriptor, relative ptr, relative len }.
  let mut b = RuntimeFnBuilder::new(2);
  let entries = b.alloc_i32();
  let count = b.alloc_i32();
  let index = b.alloc_i32();
  let entry = b.alloc_i32();
  let candidate = b.alloc_i32();
  let best_offset = b.alloc_i32();
  let best_index = b.alloc_i32();
  let path_len = b.alloc_i32();

  // The first eight bytes are reserved scratch space below the managed heap.
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Call(get_directories_idx));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(entries));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(best_offset));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(best_index));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Store(mem_arg_i32(8)));

  // Oversized preopen tables never grant authority, but every handle still drops.
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(WASI_PREOPEN_FD_LIMIT));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(entries));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(12));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(entry));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::Call(route_path_idx));
  b.emit(Instruction::LocalTee(candidate));
  b.emit(Instruction::LocalGet(best_offset));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(candidate));
  b.emit(Instruction::LocalSet(best_offset));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalSet(best_index));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(entries));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(12));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(entry));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(best_index));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::Call(drop_descriptor_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::Call(free_idx));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(entries));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(12));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::Call(free_idx));

  b.emit(Instruction::LocalGet(best_index));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(path_len));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(best_offset));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::LocalGet(best_offset));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Store(mem_arg_i32(8)));
  b.emit(Instruction::I32Const(1));
  b.finish(vec![ValType::I32; 2], vec![ValType::I32])
}

/// Open a file relative to a selected WASI 0.3 preopen and retain only the opened descriptor.
/// The caller owns the descriptor in `out` when this returns one.
pub(super) fn build_wasi_component_open_at_fn(
  select_idx: u32,
  open_idx: u32,
  drop_descriptor_idx: u32,
  free_idx: u32,
  realloc_idx: u32,
  canonical: &ComponentAsyncCanonicalImports,
) -> CompiledFn {
  // params: Calcit path pointer, open flags, descriptor flags, 12-byte output area.
  let mut b = RuntimeFnBuilder::new(4);
  let params = b.alloc_i32();
  let result = b.alloc_i32();
  let event = b.alloc_i32();
  let status = b.alloc_i32();
  let subtask = b.alloc_i32();
  let waitable_set = b.alloc_i32();
  let completed = b.alloc_i32();
  let success = b.alloc_i32();

  for instruction in [
    Instruction::LocalGet(0),
    Instruction::LocalGet(3),
    Instruction::Call(select_idx),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
  ] {
    b.emit(instruction);
  }

  // The async import exceeds the flat-parameter limit and takes an indirect record.
  for (size, alignment, local) in [(24, 4, params), (32, 4, result), (8, 8, event)] {
    for instruction in [
      Instruction::I32Const(0),
      Instruction::I32Const(0),
      Instruction::I32Const(alignment),
      Instruction::I32Const(size),
      Instruction::Call(realloc_idx),
      Instruction::LocalSet(local),
    ] {
      b.emit(instruction);
    }
  }
  for instruction in [
    Instruction::LocalGet(params),
    Instruction::LocalGet(3),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(params),
    Instruction::I32Const(0),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::LocalGet(params),
    Instruction::LocalGet(3),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Store(mem_arg_i32(8)),
    Instruction::LocalGet(params),
    Instruction::LocalGet(3),
    Instruction::I32Load(mem_arg_i32(8)),
    Instruction::I32Store(mem_arg_i32(12)),
    Instruction::LocalGet(params),
    Instruction::LocalGet(1),
    Instruction::I32Store(mem_arg_i32(16)),
    Instruction::LocalGet(params),
    Instruction::LocalGet(2),
    Instruction::I32Store(mem_arg_i32(20)),
    Instruction::LocalGet(params),
    Instruction::LocalGet(result),
    Instruction::Call(open_idx),
    Instruction::LocalTee(status),
    Instruction::I32Const(15),
    Instruction::I32And,
    Instruction::I32Const(2),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(status),
    Instruction::I32Const(15),
    Instruction::I32And,
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(status),
    Instruction::I32Const(4),
    Instruction::I32ShrU,
    Instruction::LocalTee(subtask),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::Call(canonical.waitable_set_new),
    Instruction::LocalSet(waitable_set),
    Instruction::LocalGet(subtask),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_join),
    Instruction::I32Const(0),
    Instruction::LocalSet(completed),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(waitable_set),
    Instruction::LocalGet(event),
    Instruction::Call(canonical.waitable_set_wait),
    Instruction::I32Const(1),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalGet(subtask),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Const(2),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::LocalSet(completed),
    Instruction::Br(2),
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::BrIf(1),
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
    Instruction::Else,
    Instruction::LocalGet(status),
    Instruction::I32Const(2),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::I32Const(1),
    Instruction::LocalSet(completed),
    Instruction::End,
    // The preopen was borrowed by open-at and can now be returned to the host.
    Instruction::LocalGet(3),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::Call(drop_descriptor_idx),
    Instruction::I32Const(0),
    Instruction::LocalSet(success),
    Instruction::LocalGet(completed),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(result),
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::LocalGet(result),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::I32Const(1),
    Instruction::LocalSet(success),
    Instruction::End,
    Instruction::End,
    // error-code::other carries an optional owned string inside the Result payload.
    Instruction::LocalGet(completed),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(result),
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::I32Const(1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(result),
    Instruction::I32Load8U(mem_arg_byte(4)),
    Instruction::I32Const(36),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(result),
    Instruction::I32Load8U(mem_arg_byte(8)),
    Instruction::I32Const(1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(result),
    Instruction::I32Load(mem_arg_i32(12)),
    Instruction::LocalGet(result),
    Instruction::I32Load(mem_arg_i32(16)),
    Instruction::Call(free_idx),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(params),
    Instruction::I32Const(24),
    Instruction::Call(free_idx),
    Instruction::LocalGet(result),
    Instruction::I32Const(32),
    Instruction::Call(free_idx),
    Instruction::LocalGet(event),
    Instruction::I32Const(8),
    Instruction::Call(free_idx),
    Instruction::LocalGet(success),
  ] {
    b.emit(instruction);
  }
  b.finish(vec![ValType::I32; 4], vec![ValType::I32])
}

/// Read a UTF-8 candidate into a bounded Canonical ABI buffer.
/// On success the caller owns `out.{ptr,len}` and must free the buffer.
pub(super) fn build_wasi_component_read_bytes_fn(
  imports: WasiComponentReadImports,
  canonical: &ComponentAsyncCanonicalImports,
) -> CompiledFn {
  let WasiComponentReadImports {
    source,
    read_via_stream: read_via_stream_idx,
    stream_read: stream_read_idx,
    stream_drop: stream_drop_idx,
    future_read: future_read_idx,
    future_drop: future_drop_idx,
    free: free_idx,
    realloc: realloc_idx,
  } = imports;
  // File params: path, output pair. Stdin param: output pair only.
  let output_param = u32::from(matches!(source, WasiComponentReadSource::File { .. }));
  let mut b = RuntimeFnBuilder::new(output_param + 1);
  let opened = b.alloc_i32();
  let pair = b.alloc_i32();
  let data = b.alloc_i32();
  let future_result = b.alloc_i32();
  let event = b.alloc_i32();
  let fd = b.alloc_i32();
  let stream = b.alloc_i32();
  let future = b.alloc_i32();
  let waitable_set = b.alloc_i32();
  let count = b.alloc_i32();
  let requested = b.alloc_i32();
  let status = b.alloc_i32();
  let kind = b.alloc_i32();
  let progress = b.alloc_i32();
  let done = b.alloc_i32();
  let valid = b.alloc_i32();
  let future_status = b.alloc_i32();
  let data_capacity = WASI_TEXT_FILE_LIMIT as i32 + 1;

  for instruction in [
    Instruction::LocalGet(output_param),
    Instruction::I32Const(0),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(output_param),
    Instruction::I32Const(0),
    Instruction::I32Store(mem_arg_i32(4)),
  ] {
    b.emit(instruction);
  }
  if let WasiComponentReadSource::File { open, .. } = source {
    for instruction in [
      Instruction::I32Const(0),
      Instruction::I32Const(0),
      Instruction::I32Const(4),
      Instruction::I32Const(12),
      Instruction::Call(realloc_idx),
      Instruction::LocalSet(opened),
      Instruction::LocalGet(0),
      Instruction::I32Const(0),
      Instruction::I32Const(1),
      Instruction::LocalGet(opened),
      Instruction::Call(open),
      Instruction::I32Eqz,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(opened),
      Instruction::I32Const(12),
      Instruction::Call(free_idx),
      Instruction::I32Const(0),
      Instruction::Return,
      Instruction::End,
      Instruction::LocalGet(opened),
      Instruction::I32Load(mem_arg_i32(0)),
      Instruction::LocalSet(fd),
    ] {
      b.emit(instruction);
    }
  }

  for (size, alignment, local) in [(8, 4, pair), (data_capacity, 1, data), (32, 4, future_result), (8, 8, event)] {
    for instruction in [
      Instruction::I32Const(0),
      Instruction::I32Const(0),
      Instruction::I32Const(alignment),
      Instruction::I32Const(size),
      Instruction::Call(realloc_idx),
      Instruction::LocalSet(local),
    ] {
      b.emit(instruction);
    }
  }
  if matches!(source, WasiComponentReadSource::File { .. }) {
    b.emit(Instruction::LocalGet(fd));
    b.emit(Instruction::I64Const(0));
  }
  for instruction in [
    Instruction::LocalGet(pair),
    Instruction::Call(read_via_stream_idx),
    Instruction::LocalGet(pair),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalSet(stream),
    Instruction::LocalGet(pair),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(future),
    Instruction::Call(canonical.waitable_set_new),
    Instruction::LocalSet(waitable_set),
    Instruction::I32Const(0),
    Instruction::LocalSet(count),
    Instruction::I32Const(0),
    Instruction::LocalSet(done),
    Instruction::I32Const(1),
    Instruction::LocalSet(valid),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(done),
    Instruction::LocalGet(valid),
    Instruction::I32Eqz,
    Instruction::I32Or,
    Instruction::BrIf(1),
    Instruction::I32Const(data_capacity),
    Instruction::LocalGet(count),
    Instruction::I32Sub,
    Instruction::I32Const(64 * 1024),
    Instruction::I32LtU,
    Instruction::If(BlockType::Result(ValType::I32)),
    Instruction::I32Const(data_capacity),
    Instruction::LocalGet(count),
    Instruction::I32Sub,
    Instruction::Else,
    Instruction::I32Const(64 * 1024),
    Instruction::End,
    Instruction::LocalSet(requested),
    Instruction::LocalGet(stream),
    Instruction::LocalGet(data),
    Instruction::LocalGet(count),
    Instruction::I32Add,
    Instruction::LocalGet(requested),
    Instruction::Call(stream_read_idx),
    Instruction::LocalSet(status),
    Instruction::LocalGet(status),
    Instruction::I32Const(-1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(stream),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_join),
    Instruction::LocalGet(waitable_set),
    Instruction::LocalGet(event),
    Instruction::Call(canonical.waitable_set_wait),
    Instruction::I32Const(2),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalGet(stream),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(status),
    Instruction::LocalGet(stream),
    Instruction::I32Const(0),
    Instruction::Call(canonical.waitable_join),
    Instruction::End,
    Instruction::LocalGet(status),
    Instruction::I32Const(15),
    Instruction::I32And,
    Instruction::LocalSet(kind),
    Instruction::LocalGet(status),
    Instruction::I32Const(4),
    Instruction::I32ShrU,
    Instruction::LocalSet(progress),
    Instruction::LocalGet(progress),
    Instruction::LocalGet(requested),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(count),
    Instruction::LocalGet(progress),
    Instruction::I32Add,
    Instruction::LocalSet(count),
    Instruction::LocalGet(count),
    Instruction::I32Const(WASI_TEXT_FILE_LIMIT as i32),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(valid),
    Instruction::End,
    Instruction::LocalGet(kind),
    Instruction::I32Const(1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::LocalSet(done),
    Instruction::End,
    Instruction::LocalGet(kind),
    Instruction::I32Const(0),
    Instruction::I32Ne,
    Instruction::LocalGet(kind),
    Instruction::I32Eqz,
    Instruction::LocalGet(progress),
    Instruction::I32Eqz,
    Instruction::I32And,
    Instruction::I32Or,
    Instruction::LocalGet(kind),
    Instruction::I32Const(1),
    Instruction::I32Ne,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(valid),
    Instruction::End,
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
  ] {
    b.emit(instruction);
  }

  for instruction in [
    // Early rejection cancels the producer. At EOF, observe its completion
    // before dropping the stream so cancellation cannot hide a host error.
    Instruction::LocalGet(valid),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(stream),
    Instruction::Call(stream_drop_idx),
    Instruction::End,
  ] {
    b.emit(instruction);
  }
  // Preserve ownership independently of completion, which may change valid.
  let stream_owned = b.alloc_i32();
  b.emit(Instruction::LocalGet(valid));
  b.emit(Instruction::LocalSet(stream_owned));
  for instruction in [
    Instruction::LocalGet(future),
    Instruction::LocalGet(future_result),
    Instruction::Call(future_read_idx),
    Instruction::LocalSet(future_status),
    Instruction::LocalGet(future_status),
    Instruction::I32Const(-1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(future),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_join),
    Instruction::LocalGet(waitable_set),
    Instruction::LocalGet(event),
    Instruction::Call(canonical.waitable_set_wait),
    Instruction::I32Const(4),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalGet(future),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(future_status),
    Instruction::LocalGet(future),
    Instruction::I32Const(0),
    Instruction::Call(canonical.waitable_join),
    Instruction::End,
    Instruction::LocalGet(future_status),
    Instruction::I32Eqz,
    Instruction::LocalGet(future_result),
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::I32Eqz,
    Instruction::I32And,
    Instruction::LocalGet(valid),
    Instruction::I32And,
    Instruction::LocalGet(done),
    Instruction::I32And,
    Instruction::LocalSet(valid),
  ] {
    b.emit(instruction);
  }
  if matches!(source, WasiComponentReadSource::File { .. }) {
    for instruction in [
      // A failed future may own an error-code::other string.
      Instruction::LocalGet(future_status),
      Instruction::I32Eqz,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(future_result),
      Instruction::I32Load8U(mem_arg_byte(0)),
      Instruction::I32Const(1),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(future_result),
      Instruction::I32Load8U(mem_arg_byte(4)),
      Instruction::I32Const(36),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(future_result),
      Instruction::I32Load8U(mem_arg_byte(8)),
      Instruction::I32Const(1),
      Instruction::I32Eq,
      Instruction::If(BlockType::Empty),
      Instruction::LocalGet(future_result),
      Instruction::I32Load(mem_arg_i32(12)),
      Instruction::LocalGet(future_result),
      Instruction::I32Load(mem_arg_i32(16)),
      Instruction::Call(free_idx),
      Instruction::End,
      Instruction::End,
      Instruction::End,
      Instruction::End,
    ] {
      b.emit(instruction);
    }
  }
  for instruction in [
    Instruction::LocalGet(stream_owned),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(stream),
    Instruction::Call(stream_drop_idx),
    Instruction::End,
    Instruction::LocalGet(future),
    Instruction::Call(future_drop_idx),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_set_drop),
  ] {
    b.emit(instruction);
  }
  if let WasiComponentReadSource::File { descriptor_drop, .. } = source {
    b.emit(Instruction::LocalGet(fd));
    b.emit(Instruction::Call(descriptor_drop));
    b.emit(Instruction::LocalGet(opened));
    b.emit(Instruction::I32Const(12));
    b.emit(Instruction::Call(free_idx));
  }
  for (local, size) in [(pair, 8), (future_result, 32), (event, 8)] {
    for instruction in [
      Instruction::LocalGet(local),
      Instruction::I32Const(size),
      Instruction::Call(free_idx),
    ] {
      b.emit(instruction);
    }
  }
  for instruction in [
    Instruction::LocalGet(valid),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(output_param),
    Instruction::LocalGet(data),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(output_param),
    Instruction::LocalGet(count),
    Instruction::I32Store(mem_arg_i32(4)),
    Instruction::I32Const(1),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(data),
    Instruction::I32Const(data_capacity),
    Instruction::Call(free_idx),
    Instruction::I32Const(0),
  ] {
    b.emit(instruction);
  }
  b.finish(vec![ValType::I32; (output_param + 1) as usize], vec![ValType::I32])
}

/// Write a bounded Calcit string through a preopened WASI 0.3 descriptor.
/// Opening with create/truncate is intentionally non-atomic: failure may leave
/// a truncated or partially written file, and never reports success.
pub(super) fn build_wasi_component_write_bytes_fn(
  imports: WasiComponentWriteImports,
  canonical: &ComponentAsyncCanonicalImports,
) -> CompiledFn {
  let WasiComponentWriteImports {
    open,
    stream_new,
    stream_write,
    stream_drop,
    write_via_stream,
    future_read,
    future_drop,
    descriptor_drop,
    free,
    realloc,
  } = imports;
  // params: Calcit path pointer, Calcit string pointer.
  let mut b = RuntimeFnBuilder::new(2);
  let length = b.alloc_i32();
  let opened = b.alloc_i32();
  let fd = b.alloc_i32();
  let pair = b.alloc_i64();
  let reader = b.alloc_i32();
  let writer = b.alloc_i32();
  let future = b.alloc_i32();
  let waitable_set = b.alloc_i32();
  let event = b.alloc_i32();
  let future_result = b.alloc_i32();
  let offset = b.alloc_i32();
  let requested = b.alloc_i32();
  let status = b.alloc_i32();
  let progress = b.alloc_i32();
  let valid = b.alloc_i32();

  for instruction in [
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalTee(length),
    Instruction::I32Const(WASI_TEXT_FILE_LIMIT as i32),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(4),
    Instruction::I32Const(12),
    Instruction::Call(realloc),
    Instruction::LocalSet(opened),
    Instruction::LocalGet(0),
    Instruction::I32Const(9), // create | truncate
    Instruction::I32Const(2), // descriptor-flags::write
    Instruction::LocalGet(opened),
    Instruction::Call(open),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(opened),
    Instruction::I32Const(12),
    Instruction::Call(free),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(opened),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalSet(fd),
    Instruction::Call(stream_new),
    Instruction::LocalSet(pair),
    Instruction::LocalGet(pair),
    Instruction::I32WrapI64,
    Instruction::LocalSet(reader),
    Instruction::LocalGet(pair),
    Instruction::I64Const(32),
    Instruction::I64ShrU,
    Instruction::I32WrapI64,
    Instruction::LocalSet(writer),
    Instruction::LocalGet(fd),
    Instruction::LocalGet(reader),
    Instruction::I64Const(0),
    Instruction::Call(write_via_stream),
    Instruction::LocalSet(future),
    Instruction::Call(canonical.waitable_set_new),
    Instruction::LocalSet(waitable_set),
  ] {
    b.emit(instruction);
  }
  for (size, alignment, local) in [(8, 8, event), (32, 4, future_result)] {
    for instruction in [
      Instruction::I32Const(0),
      Instruction::I32Const(0),
      Instruction::I32Const(alignment),
      Instruction::I32Const(size),
      Instruction::Call(realloc),
      Instruction::LocalSet(local),
    ] {
      b.emit(instruction);
    }
  }
  for instruction in [
    Instruction::I32Const(0),
    Instruction::LocalSet(offset),
    Instruction::I32Const(1),
    Instruction::LocalSet(valid),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(offset),
    Instruction::LocalGet(length),
    Instruction::I32GeU,
    Instruction::LocalGet(valid),
    Instruction::I32Eqz,
    Instruction::I32Or,
    Instruction::BrIf(1),
    Instruction::LocalGet(length),
    Instruction::LocalGet(offset),
    Instruction::I32Sub,
    Instruction::I32Const(64 * 1024),
    Instruction::I32LtU,
    Instruction::If(BlockType::Result(ValType::I32)),
    Instruction::LocalGet(length),
    Instruction::LocalGet(offset),
    Instruction::I32Sub,
    Instruction::Else,
    Instruction::I32Const(64 * 1024),
    Instruction::End,
    Instruction::LocalSet(requested),
    Instruction::LocalGet(writer),
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(offset),
    Instruction::I32Add,
    Instruction::LocalGet(requested),
    Instruction::Call(stream_write),
    Instruction::LocalSet(status),
    Instruction::LocalGet(status),
    Instruction::I32Const(-1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(writer),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_join),
    Instruction::LocalGet(waitable_set),
    Instruction::LocalGet(event),
    Instruction::Call(canonical.waitable_set_wait),
    Instruction::I32Const(3),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalGet(writer),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(status),
    Instruction::LocalGet(writer),
    Instruction::I32Const(0),
    Instruction::Call(canonical.waitable_join),
    Instruction::End,
    Instruction::LocalGet(status),
    Instruction::I32Const(15),
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(valid),
    Instruction::End,
    Instruction::LocalGet(status),
    Instruction::I32Const(4),
    Instruction::I32ShrU,
    Instruction::LocalTee(progress),
    Instruction::I32Eqz,
    Instruction::LocalGet(progress),
    Instruction::LocalGet(requested),
    Instruction::I32GtU,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(valid),
    Instruction::Else,
    Instruction::LocalGet(offset),
    Instruction::LocalGet(progress),
    Instruction::I32Add,
    Instruction::LocalSet(offset),
    Instruction::End,
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(writer),
    Instruction::Call(stream_drop),
    Instruction::LocalGet(future),
    Instruction::LocalGet(future_result),
    Instruction::Call(future_read),
    Instruction::LocalSet(status),
    Instruction::LocalGet(status),
    Instruction::I32Const(-1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(future),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_join),
    Instruction::LocalGet(waitable_set),
    Instruction::LocalGet(event),
    Instruction::Call(canonical.waitable_set_wait),
    Instruction::I32Const(4),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalGet(future),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(status),
    Instruction::LocalGet(future),
    Instruction::I32Const(0),
    Instruction::Call(canonical.waitable_join),
    Instruction::End,
    Instruction::LocalGet(status),
    Instruction::I32Eqz,
    Instruction::LocalGet(future_result),
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::I32Eqz,
    Instruction::I32And,
    Instruction::LocalGet(valid),
    Instruction::I32And,
    Instruction::LocalSet(valid),
    Instruction::LocalGet(status),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(future_result),
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::I32Const(1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(future_result),
    Instruction::I32Load8U(mem_arg_byte(4)),
    Instruction::I32Const(36),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(future_result),
    Instruction::I32Load8U(mem_arg_byte(8)),
    Instruction::I32Const(1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(future_result),
    Instruction::I32Load(mem_arg_i32(12)),
    Instruction::LocalGet(future_result),
    Instruction::I32Load(mem_arg_i32(16)),
    Instruction::Call(free),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(future),
    Instruction::Call(future_drop),
    Instruction::LocalGet(fd),
    Instruction::Call(descriptor_drop),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_set_drop),
    Instruction::LocalGet(opened),
    Instruction::I32Const(12),
    Instruction::Call(free),
    Instruction::LocalGet(event),
    Instruction::I32Const(8),
    Instruction::Call(free),
    Instruction::LocalGet(future_result),
    Instruction::I32Const(32),
    Instruction::Call(free),
    Instruction::LocalGet(valid),
  ] {
    b.emit(instruction);
  }
  b.finish(vec![ValType::I32; 2], vec![ValType::I32])
}

/// Read a bounded WASI 0.3 directory into a sorted list of Calcit String pointers.
/// Returns zero on validation, authority, UTF-8, capacity, or host failures.
pub(super) fn build_wasi_component_read_dir_fn(
  imports: WasiComponentReadDirImports,
  canonical: &ComponentAsyncCanonicalImports,
) -> CompiledFn {
  let WasiComponentReadDirImports {
    open: open_idx,
    read_directory: read_directory_idx,
    stream_read: stream_read_idx,
    stream_drop: stream_drop_idx,
    future_read: future_read_idx,
    future_drop: future_drop_idx,
    descriptor_drop: descriptor_drop_idx,
    utf8_valid: utf8_valid_idx,
    str_compare: str_compare_idx,
    free: free_idx,
    realloc: realloc_idx,
    list_tag,
    string_tag,
  } = imports;
  const BATCH_ENTRIES: i32 = 16;
  const BATCH_BYTES: i32 = BATCH_ENTRIES * 24;
  let mut b = RuntimeFnBuilder::new(1);
  let path_len = b.alloc_i32();
  let separator_len = b.alloc_i32();
  let opened = b.alloc_i32();
  let fd = b.alloc_i32();
  let managed_size = b.alloc_i64();
  let reserve_status = b.alloc_i32();
  let list_size = b.alloc_i32();
  let list = b.alloc_i32();
  let pair = b.alloc_i32();
  let entries = b.alloc_i32();
  let future_result = b.alloc_i32();
  let event = b.alloc_i32();
  let stream = b.alloc_i32();
  let future = b.alloc_i32();
  let waitable_set = b.alloc_i32();
  let count = b.alloc_i32();
  let total_path_bytes = b.alloc_i32();
  let done = b.alloc_i32();
  let valid = b.alloc_i32();
  let status = b.alloc_i32();
  let kind = b.alloc_i32();
  let progress = b.alloc_i32();
  let entry_idx = b.alloc_i32();
  let entry = b.alloc_i32();
  let name = b.alloc_i32();
  let name_len = b.alloc_i32();
  let entry_valid = b.alloc_i32();
  let name_index = b.alloc_i32();
  let name_byte = b.alloc_i32();
  let skip = b.alloc_i32();
  let new_len = b.alloc_i32();
  let padded = b.alloc_i32();
  let string_size = b.alloc_i32();
  let string = b.alloc_i32();
  let string_data = b.alloc_i32();
  let insert_at = b.alloc_i32();
  let previous = b.alloc_i32();
  let stream_owned = b.alloc_i32();
  let future_status = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalTee(path_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32Const(WASI_DIR_PATH_BYTES_LIMIT));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(separator_len));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(separator_len));
  b.emit(Instruction::End);

  for instruction in [
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(4),
    Instruction::I32Const(12),
    Instruction::Call(realloc_idx),
    Instruction::LocalSet(opened),
    Instruction::LocalGet(0),
    Instruction::I32Const(2), // open-flags::directory
    Instruction::I32Const(1), // descriptor-flags::read
    Instruction::LocalGet(opened),
    Instruction::Call(open_idx),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(opened),
    Instruction::I32Const(12),
    Instruction::Call(free_idx),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(opened),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalSet(fd),
  ] {
    b.emit(instruction);
  }

  b.emit(Instruction::I64Const(WASI_DIR_MANAGED_BYTES_LIMIT));
  b.emit(Instruction::LocalSet(managed_size));
  rt_emit_try_reserve_managed(&mut b, managed_size, reserve_status);
  b.emit(Instruction::LocalGet(reserve_status));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(descriptor_drop_idx));
  b.emit(Instruction::LocalGet(opened));
  b.emit(Instruction::I32Const(12));
  b.emit(Instruction::Call(free_idx));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(8 + WASI_DIR_ENTRY_LIMIT * 8));
  b.emit(Instruction::LocalSet(list_size));
  rt_emit_alloc_dynamic(&mut b, list_size, list, list_tag);

  for (size, alignment, local) in [(8, 4, pair), (BATCH_BYTES, 4, entries), (32, 4, future_result), (8, 8, event)] {
    for instruction in [
      Instruction::I32Const(0),
      Instruction::I32Const(0),
      Instruction::I32Const(alignment),
      Instruction::I32Const(size),
      Instruction::Call(realloc_idx),
      Instruction::LocalSet(local),
    ] {
      b.emit(instruction);
    }
  }

  for instruction in [
    Instruction::LocalGet(fd),
    Instruction::LocalGet(pair),
    Instruction::Call(read_directory_idx),
    Instruction::LocalGet(pair),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalSet(stream),
    Instruction::LocalGet(pair),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(future),
    Instruction::Call(canonical.waitable_set_new),
    Instruction::LocalSet(waitable_set),
    Instruction::I32Const(0),
    Instruction::LocalSet(count),
    Instruction::I32Const(0),
    Instruction::LocalSet(total_path_bytes),
    Instruction::I32Const(0),
    Instruction::LocalSet(done),
    Instruction::I32Const(1),
    Instruction::LocalSet(valid),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(done),
    Instruction::LocalGet(valid),
    Instruction::I32Eqz,
    Instruction::I32Or,
    Instruction::BrIf(1),
    Instruction::LocalGet(stream),
    Instruction::LocalGet(entries),
    Instruction::I32Const(BATCH_ENTRIES),
    Instruction::Call(stream_read_idx),
    Instruction::LocalSet(status),
    Instruction::LocalGet(status),
    Instruction::I32Const(-1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(stream),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_join),
    Instruction::LocalGet(waitable_set),
    Instruction::LocalGet(event),
    Instruction::Call(canonical.waitable_set_wait),
    Instruction::I32Const(2),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalGet(stream),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(status),
    Instruction::LocalGet(stream),
    Instruction::I32Const(0),
    Instruction::Call(canonical.waitable_join),
    Instruction::End,
    Instruction::LocalGet(status),
    Instruction::I32Const(15),
    Instruction::I32And,
    Instruction::LocalSet(kind),
    Instruction::LocalGet(status),
    Instruction::I32Const(4),
    Instruction::I32ShrU,
    Instruction::LocalSet(progress),
    Instruction::LocalGet(progress),
    Instruction::I32Const(BATCH_ENTRIES),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::I32Const(0),
    Instruction::LocalSet(entry_idx),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(entry_idx),
    Instruction::LocalGet(progress),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(entries),
    Instruction::LocalGet(entry_idx),
    Instruction::I32Const(24),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalSet(entry),
    // Free descriptor-type::other optional string payload if present.
    Instruction::LocalGet(entry),
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::I32Const(7),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(entry),
    Instruction::I32Load8U(mem_arg_byte(4)),
    Instruction::I32Const(1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(entry),
    Instruction::I32Load(mem_arg_i32(8)),
    Instruction::LocalGet(entry),
    Instruction::I32Load(mem_arg_i32(12)),
    Instruction::Call(free_idx),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(entry),
    Instruction::I32Load(mem_arg_i32(16)),
    Instruction::LocalSet(name),
    Instruction::LocalGet(entry),
    Instruction::I32Load(mem_arg_i32(20)),
    Instruction::LocalSet(name_len),
    Instruction::LocalGet(valid),
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::LocalSet(entry_valid),
    Instruction::LocalGet(name_len),
    Instruction::I32Eqz,
    Instruction::LocalGet(name_len),
    Instruction::I32Const(WASI_DIR_NAME_LIMIT),
    Instruction::I32GtU,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(entry_valid),
    Instruction::Else,
    Instruction::LocalGet(name),
    Instruction::LocalGet(name_len),
    Instruction::Call(utf8_valid_idx),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(entry_valid),
    Instruction::Else,
    Instruction::I32Const(0),
    Instruction::LocalSet(name_index),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(name_index),
    Instruction::LocalGet(name_len),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(name),
    Instruction::LocalGet(name_index),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalTee(name_byte),
    Instruction::I32Eqz,
    Instruction::LocalGet(name_byte),
    Instruction::I32Const(b'/' as i32),
    Instruction::I32Eq,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(entry_valid),
    Instruction::Br(2),
    Instruction::End,
    Instruction::LocalGet(name_index),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(name_index),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(entry_valid),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(valid),
    Instruction::Else,
    Instruction::I32Const(0),
    Instruction::LocalSet(skip),
    Instruction::LocalGet(name_len),
    Instruction::I32Const(1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(name),
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::I32Const(b'.' as i32),
    Instruction::I32Eq,
    Instruction::LocalSet(skip),
    Instruction::End,
    Instruction::LocalGet(name_len),
    Instruction::I32Const(2),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(name),
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::I32Const(b'.' as i32),
    Instruction::I32Eq,
    Instruction::LocalGet(name),
    Instruction::I32Load8U(mem_arg_byte(1)),
    Instruction::I32Const(b'.' as i32),
    Instruction::I32Eq,
    Instruction::I32And,
    Instruction::LocalSet(skip),
    Instruction::End,
    Instruction::LocalGet(skip),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(count),
    Instruction::I32Const(WASI_DIR_ENTRY_LIMIT),
    Instruction::I32GeU,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(valid),
    Instruction::Else,
    Instruction::LocalGet(path_len),
    Instruction::LocalGet(separator_len),
    Instruction::I32Add,
    Instruction::LocalGet(name_len),
    Instruction::I32Add,
    Instruction::LocalTee(new_len),
    Instruction::LocalGet(total_path_bytes),
    Instruction::I32Add,
    Instruction::LocalTee(total_path_bytes),
    Instruction::I32Const(WASI_DIR_PATH_BYTES_LIMIT),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(valid),
    Instruction::Else,
    Instruction::LocalGet(new_len),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::I32Const(-8),
    Instruction::I32And,
    Instruction::LocalSet(padded),
    Instruction::LocalGet(padded),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(string_size),
  ] {
    b.emit(instruction);
  }
  rt_emit_alloc_dynamic(&mut b, string_size, string, string_tag);
  for instruction in [
    Instruction::LocalGet(string),
    Instruction::LocalGet(new_len),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(string),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalTee(string_data),
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(path_len),
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    Instruction::LocalGet(separator_len),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(string_data),
    Instruction::LocalGet(path_len),
    Instruction::I32Add,
    Instruction::I32Const(b'/' as i32),
    Instruction::I32Store8(mem_arg_byte(0)),
    Instruction::End,
    Instruction::LocalGet(string_data),
    Instruction::LocalGet(path_len),
    Instruction::I32Add,
    Instruction::LocalGet(separator_len),
    Instruction::I32Add,
    Instruction::LocalGet(name),
    Instruction::LocalGet(name_len),
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    Instruction::LocalGet(count),
    Instruction::LocalSet(insert_at),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(insert_at),
    Instruction::I32Eqz,
    Instruction::BrIf(1),
    Instruction::LocalGet(list),
    Instruction::LocalGet(insert_at),
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(previous),
    Instruction::LocalGet(string),
    Instruction::LocalGet(previous),
    Instruction::Call(str_compare_idx),
    Instruction::F64Const(Ieee64::from(0.0)),
    Instruction::F64Ge,
    Instruction::BrIf(1),
    Instruction::LocalGet(list),
    Instruction::LocalGet(insert_at),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalGet(previous),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(insert_at),
    Instruction::I32Const(1),
    Instruction::I32Sub,
    Instruction::LocalSet(insert_at),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(list),
    Instruction::LocalGet(insert_at),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalGet(string),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(count),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(count),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(name),
    Instruction::LocalGet(name_len),
    Instruction::Call(free_idx),
    Instruction::LocalGet(entry_idx),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(entry_idx),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(kind),
    Instruction::I32Const(1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::LocalSet(done),
    Instruction::End,
    Instruction::LocalGet(kind),
    Instruction::I32Const(0),
    Instruction::I32Ne,
    Instruction::LocalGet(kind),
    Instruction::I32Eqz,
    Instruction::LocalGet(progress),
    Instruction::I32Eqz,
    Instruction::I32And,
    Instruction::I32Or,
    Instruction::LocalGet(kind),
    Instruction::I32Const(1),
    Instruction::I32Ne,
    Instruction::I32And,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(valid),
    Instruction::End,
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(valid),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(stream),
    Instruction::Call(stream_drop_idx),
    Instruction::End,
    Instruction::LocalGet(valid),
    Instruction::LocalSet(stream_owned),
    Instruction::LocalGet(future),
    Instruction::LocalGet(future_result),
    Instruction::Call(future_read_idx),
    Instruction::LocalSet(future_status),
    Instruction::LocalGet(future_status),
    Instruction::I32Const(-1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(future),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_join),
    Instruction::LocalGet(waitable_set),
    Instruction::LocalGet(event),
    Instruction::Call(canonical.waitable_set_wait),
    Instruction::I32Const(4),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalGet(future),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(future_status),
    Instruction::LocalGet(future),
    Instruction::I32Const(0),
    Instruction::Call(canonical.waitable_join),
    Instruction::End,
    Instruction::LocalGet(future_status),
    Instruction::I32Eqz,
    Instruction::LocalGet(future_result),
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::I32Eqz,
    Instruction::I32And,
    Instruction::LocalGet(valid),
    Instruction::I32And,
    Instruction::LocalGet(done),
    Instruction::I32And,
    Instruction::LocalSet(valid),
    Instruction::LocalGet(future_status),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(future_result),
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::I32Const(1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(future_result),
    Instruction::I32Load8U(mem_arg_byte(4)),
    Instruction::I32Const(36),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(future_result),
    Instruction::I32Load8U(mem_arg_byte(8)),
    Instruction::I32Const(1),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(future_result),
    Instruction::I32Load(mem_arg_i32(12)),
    Instruction::LocalGet(future_result),
    Instruction::I32Load(mem_arg_i32(16)),
    Instruction::Call(free_idx),
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(stream_owned),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(stream),
    Instruction::Call(stream_drop_idx),
    Instruction::End,
    Instruction::LocalGet(future),
    Instruction::Call(future_drop_idx),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_set_drop),
    Instruction::LocalGet(fd),
    Instruction::Call(descriptor_drop_idx),
    Instruction::LocalGet(opened),
    Instruction::I32Const(12),
    Instruction::Call(free_idx),
    Instruction::LocalGet(pair),
    Instruction::I32Const(8),
    Instruction::Call(free_idx),
    Instruction::LocalGet(entries),
    Instruction::I32Const(BATCH_BYTES),
    Instruction::Call(free_idx),
    Instruction::LocalGet(future_result),
    Instruction::I32Const(32),
    Instruction::Call(free_idx),
    Instruction::LocalGet(event),
    Instruction::I32Const(8),
    Instruction::Call(free_idx),
    Instruction::LocalGet(valid),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(list),
    Instruction::LocalGet(count),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(list),
    Instruction::Return,
    Instruction::End,
    Instruction::I32Const(0),
  ] {
    b.emit(instruction);
  }
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

/// Suspend the current WASI 0.3 stackful command task for `nanos` nanoseconds.
/// Returns 0 on normal completion and 1 if the subtask was cancelled.
pub(super) fn build_wasi_component_wait_fn(
  wait_for_idx: u32,
  realloc_idx: u32,
  free_idx: u32,
  canonical: &ComponentAsyncCanonicalImports,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let status = b.alloc_i32();
  let subtask = b.alloc_i32();
  let waitable_set = b.alloc_i32();
  let event = b.alloc_i32();
  let completed = b.alloc_i32();

  for instruction in [
    Instruction::LocalGet(0),
    Instruction::Call(wait_for_idx),
    Instruction::LocalTee(status),
    Instruction::I32Const(15),
    Instruction::I32And,
    Instruction::I32Const(2),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(status),
    Instruction::I32Const(15),
    Instruction::I32And,
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(status),
    Instruction::I32Const(4),
    Instruction::I32ShrU,
    Instruction::LocalTee(subtask),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(8),
    Instruction::I32Const(8),
    Instruction::Call(realloc_idx),
    Instruction::LocalSet(event),
    Instruction::Call(canonical.waitable_set_new),
    Instruction::LocalSet(waitable_set),
    Instruction::LocalGet(subtask),
    Instruction::LocalGet(waitable_set),
    Instruction::Call(canonical.waitable_join),
    Instruction::I32Const(0),
    Instruction::LocalSet(completed),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(waitable_set),
    Instruction::LocalGet(event),
    Instruction::Call(canonical.waitable_set_wait),
    Instruction::I32Const(1),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalGet(subtask),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Const(2),
    Instruction::I32Eq,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(1),
    Instruction::LocalSet(completed),
    Instruction::Br(2),
    Instruction::End,
    Instruction::LocalGet(event),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::I32Const(1),
    Instruction::I32GtU,
    Instruction::BrIf(1),
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
    Instruction::LocalGet(event),
    Instruction::I32Const(8),
    Instruction::Call(free_idx),
    Instruction::Else,
    Instruction::LocalGet(status),
    Instruction::I32Const(2),
    Instruction::I32Ne,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    Instruction::I32Const(1),
    Instruction::LocalSet(completed),
    Instruction::End,
    Instruction::LocalGet(completed),
    Instruction::I32Eqz,
  ] {
    b.emit(instruction);
  }
  b.finish(vec![ValType::I64], vec![ValType::I32])
}

/// Fill `[dst_ptr, dst_ptr + size)` with cryptographically secure random bytes
/// from `wasi:random/random@0.3.1#get-random-bytes`. Returns 0 on success and 1 on error.
pub(super) fn build_wasi_component_random_get_fn(get_random_bytes_idx: u32, realloc_idx: u32, free_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let retptr = b.alloc_i32();
  let offset = b.alloc_i32();
  let remaining = b.alloc_i32();
  let chunk_ptr = b.alloc_i32();
  let chunk_len = b.alloc_i32();

  for instruction in [
    Instruction::LocalGet(1),
    Instruction::I32Eqz,
    Instruction::If(BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::Return,
    Instruction::End,
    Instruction::I32Const(0),
    Instruction::I32Const(0),
    Instruction::I32Const(4),
    Instruction::I32Const(8),
    Instruction::Call(realloc_idx),
    Instruction::LocalSet(retptr),
    Instruction::I32Const(0),
    Instruction::LocalSet(offset),
    Instruction::Block(BlockType::Empty),
    Instruction::Loop(BlockType::Empty),
    Instruction::LocalGet(offset),
    Instruction::LocalGet(1),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(1),
    Instruction::LocalGet(offset),
    Instruction::I32Sub,
    Instruction::LocalSet(remaining),
    Instruction::LocalGet(remaining),
    Instruction::I64ExtendI32U,
    Instruction::LocalGet(retptr),
    Instruction::Call(get_random_bytes_idx),
    Instruction::LocalGet(retptr),
    Instruction::I32Load(mem_arg_i32(0)),
    Instruction::LocalSet(chunk_ptr),
    Instruction::LocalGet(retptr),
    Instruction::I32Load(mem_arg_i32(4)),
    Instruction::LocalSet(chunk_len),
    Instruction::LocalGet(chunk_len),
    Instruction::I32Eqz,
    Instruction::LocalGet(chunk_len),
    Instruction::LocalGet(remaining),
    Instruction::I32GtU,
    Instruction::I32Or,
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(chunk_ptr),
    Instruction::If(BlockType::Empty),
    Instruction::LocalGet(chunk_ptr),
    Instruction::LocalGet(chunk_len),
    Instruction::Call(free_idx),
    Instruction::End,
    Instruction::LocalGet(retptr),
    Instruction::I32Const(8),
    Instruction::Call(free_idx),
    Instruction::I32Const(1),
    Instruction::Return,
    Instruction::End,
    Instruction::LocalGet(0),
    Instruction::LocalGet(offset),
    Instruction::I32Add,
    Instruction::LocalGet(chunk_ptr),
    Instruction::LocalGet(chunk_len),
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    Instruction::LocalGet(chunk_ptr),
    Instruction::LocalGet(chunk_len),
    Instruction::Call(free_idx),
    Instruction::LocalGet(offset),
    Instruction::LocalGet(chunk_len),
    Instruction::I32Add,
    Instruction::LocalSet(offset),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(retptr),
    Instruction::I32Const(8),
    Instruction::Call(free_idx),
    Instruction::I32Const(0),
  ] {
    b.emit(instruction);
  }
  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}

/// Validate an RFC 3629 UTF-8 byte sequence without constructing a Calcit value.
pub(super) fn build_utf8_valid_fn() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let index = b.alloc_i32();
  let first = b.alloc_i32();
  let width = b.alloc_i32();
  let second_min = b.alloc_i32();
  let second_max = b.alloc_i32();
  let continuation = b.alloc_i32();

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalTee(first));
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(width));
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalSet(second_min));
  b.emit(Instruction::I32Const(0xbf));
  b.emit(Instruction::LocalSet(second_max));
  // Two-byte sequences start at C2, excluding overlong C0/C1 encodings.
  b.emit(Instruction::LocalGet(first));
  b.emit(Instruction::I32Const(0xc2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(first));
  b.emit(Instruction::I32Const(0xdf));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::LocalSet(width));
  b.emit(Instruction::End);
  // Three-byte sequences constrain E0 (overlong) and ED (surrogates).
  b.emit(Instruction::LocalGet(first));
  b.emit(Instruction::I32Const(0xe0));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(first));
  b.emit(Instruction::I32Const(0xef));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::LocalSet(width));
  b.emit(Instruction::LocalGet(first));
  b.emit(Instruction::I32Const(0xe0));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0xa0));
  b.emit(Instruction::LocalSet(second_min));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(first));
  b.emit(Instruction::I32Const(0xed));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x9f));
  b.emit(Instruction::LocalSet(second_max));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  // Four-byte sequences constrain Unicode to U+10FFFF.
  b.emit(Instruction::LocalGet(first));
  b.emit(Instruction::I32Const(0xf0));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(first));
  b.emit(Instruction::I32Const(0xf4));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::LocalSet(width));
  b.emit(Instruction::LocalGet(first));
  b.emit(Instruction::I32Const(0xf0));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x90));
  b.emit(Instruction::LocalSet(second_min));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(first));
  b.emit(Instruction::I32Const(0xf4));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x8f));
  b.emit(Instruction::LocalSet(second_max));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(width));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(width));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(1)));
  b.emit(Instruction::LocalTee(continuation));
  b.emit(Instruction::LocalGet(second_min));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(continuation));
  b.emit(Instruction::LocalGet(second_max));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::LocalSet(continuation));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(continuation));
  b.emit(Instruction::LocalGet(width));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(continuation));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(0xc0));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(continuation));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(continuation));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(width));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(1));
  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}

/// Read one bounded regular file through a private Preview 1 descriptor.
/// Returns a Calcit String pointer or zero on any validation/host failure.
pub(super) fn build_wasi_read_text_fn(
  open_path_idx: u32,
  fd_filestat_get_idx: u32,
  fd_read_idx: u32,
  fd_close_idx: u32,
  utf8_valid_idx: u32,
  string_tag: i32,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let fd = b.alloc_i32();
  let file_size_i64 = b.alloc_i64();
  let file_size = b.alloc_i32();
  let padded = b.alloc_i32();
  let payload = b.alloc_i32();
  let managed_size = b.alloc_i64();
  let zero_size = b.alloc_i64();
  let unused_scratch = b.alloc_i32();
  let filestat_size = b.alloc_i64();
  let filestat = b.alloc_i32();
  let string = b.alloc_i32();
  let data = b.alloc_i32();
  let offset = b.alloc_i32();
  let read_now = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I64Const((1_i64 << 1) | (1_i64 << 21)));
  b.emit(Instruction::Call(open_path_idx));
  b.emit(Instruction::LocalTee(fd));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::I64Const(64));
  b.emit(Instruction::LocalSet(filestat_size));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::LocalSet(zero_size));
  rt_emit_reserve_scratch(&mut b, filestat_size, zero_size, filestat);
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::LocalGet(filestat));
  b.emit(Instruction::Call(fd_filestat_get_idx));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(filestat));
  b.emit(Instruction::I32Const(32));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalTee(file_size_i64));
  b.emit(Instruction::I64Const(WASI_TEXT_FILE_LIMIT));
  b.emit(Instruction::I64GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(file_size_i64));
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::LocalSet(file_size));
  b.emit(Instruction::LocalGet(file_size));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(payload));
  b.emit(Instruction::LocalGet(payload));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Const(8));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(managed_size));
  rt_emit_reserve_scratch(&mut b, zero_size, managed_size, unused_scratch);
  rt_emit_alloc_dynamic(&mut b, payload, string, string_tag);
  b.emit(Instruction::LocalGet(string));
  b.emit(Instruction::LocalGet(file_size));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(string));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(data));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(offset));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::LocalGet(file_size));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(data));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::LocalGet(file_size));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::Call(fd_read_idx));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalTee(read_now));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(read_now));
  b.emit(Instruction::LocalGet(file_size));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::LocalGet(read_now));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(offset));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(data));
  b.emit(Instruction::LocalGet(file_size));
  b.emit(Instruction::Call(utf8_valid_idx));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(string));
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

/// Write one complete UTF-8 string through a private Preview 1 descriptor.
/// Returns one on success and zero on validation, partial-write, or host failure.
pub(super) fn build_wasi_write_text_fn(open_path_idx: u32, fd_write_idx: u32, fd_close_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let fd = b.alloc_i32();
  let data = b.alloc_i32();
  let length = b.alloc_i32();
  let offset = b.alloc_i32();
  let written = b.alloc_i32();

  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(length));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(data));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(9)); // CREAT | TRUNC
  b.emit(Instruction::I64Const(1_i64 << 6)); // FD_WRITE
  b.emit(Instruction::Call(open_path_idx));
  b.emit(Instruction::LocalTee(fd));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(offset));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::LocalGet(length));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(data));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::LocalGet(length));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::Call(fd_write_idx));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalTee(written));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(written));
  b.emit(Instruction::LocalGet(length));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::LocalGet(written));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(offset));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::I32Eqz);
  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}

fn emit_wasi_dir_failure(builder: &mut RuntimeFnBuilder, fd: u32, fd_close_idx: u32) {
  builder.emit(Instruction::LocalGet(fd));
  builder.emit(Instruction::Call(fd_close_idx));
  builder.emit(Instruction::Drop);
  builder.emit(Instruction::I32Const(0));
  builder.emit(Instruction::Return);
}

/// Read a bounded Preview 1 directory into a sorted list of Calcit String pointers.
/// Returns zero on validation, authority, UTF-8, capacity, or host failures.
pub(super) fn build_wasi_read_dir_fn(
  open_path_idx: u32,
  fd_readdir_idx: u32,
  fd_close_idx: u32,
  utf8_valid_idx: u32,
  str_compare_idx: u32,
  list_tag: i32,
  string_tag: i32,
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let path_len = b.alloc_i32();
  let separator_len = b.alloc_i32();
  let fd = b.alloc_i32();
  let list_size = b.alloc_i32();
  let list = b.alloc_i32();
  let buffer_size = b.alloc_i32();
  let buffer = b.alloc_i32();
  let count = b.alloc_i32();
  let cookie = b.alloc_i64();
  let page_cookie = b.alloc_i64();
  let used = b.alloc_i32();
  let offset = b.alloc_i32();
  let remaining = b.alloc_i32();
  let next_cookie = b.alloc_i64();
  let name_len = b.alloc_i32();
  let name = b.alloc_i32();
  let complete = b.alloc_i32();
  let truncated = b.alloc_i32();
  let skip = b.alloc_i32();
  let name_index = b.alloc_i32();
  let name_byte = b.alloc_i32();
  let new_len = b.alloc_i32();
  let total_path_bytes = b.alloc_i32();
  let padded = b.alloc_i32();
  let string_size = b.alloc_i32();
  let string = b.alloc_i32();
  let string_data = b.alloc_i32();
  let insert_at = b.alloc_i32();
  let previous = b.alloc_i32();
  let managed_size = b.alloc_i64();
  let reserve_status = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalTee(path_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32Const(WASI_DIR_PATH_BYTES_LIMIT));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(separator_len));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(separator_len));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I64Const(1_i64 << 14));
  b.emit(Instruction::Call(open_path_idx));
  b.emit(Instruction::LocalTee(fd));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::I64Const(WASI_DIR_MANAGED_BYTES_LIMIT));
  b.emit(Instruction::LocalSet(managed_size));
  rt_emit_try_reserve_managed(&mut b, managed_size, reserve_status);
  b.emit(Instruction::LocalGet(reserve_status));
  b.emit(Instruction::If(BlockType::Empty));
  emit_wasi_dir_failure(&mut b, fd, fd_close_idx);
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(8 + WASI_DIR_ENTRY_LIMIT * 8));
  b.emit(Instruction::LocalSet(list_size));
  rt_emit_alloc_dynamic(&mut b, list_size, list, list_tag);
  b.emit(Instruction::I32Const(WASI_DIR_BUFFER_SIZE));
  b.emit(Instruction::LocalSet(buffer_size));
  rt_emit_alloc_dynamic(&mut b, buffer_size, buffer, 0);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(total_path_bytes));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::LocalSet(cookie));

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(cookie));
  b.emit(Instruction::LocalSet(page_cookie));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::LocalGet(buffer));
  b.emit(Instruction::I32Const(WASI_DIR_BUFFER_SIZE));
  b.emit(Instruction::LocalGet(cookie));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Call(fd_readdir_idx));
  b.emit(Instruction::If(BlockType::Empty));
  emit_wasi_dir_failure(&mut b, fd, fd_close_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalTee(used));
  b.emit(Instruction::I32Const(WASI_DIR_BUFFER_SIZE));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  emit_wasi_dir_failure(&mut b, fd, fd_close_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(used));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(offset));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(complete));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(truncated));

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::LocalGet(used));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(used));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalTee(remaining));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(truncated));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(buffer));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(next_cookie));
  b.emit(Instruction::LocalGet(buffer));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load(mem_arg_i32(16)));
  b.emit(Instruction::LocalTee(name_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  emit_wasi_dir_failure(&mut b, fd, fd_close_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Const(WASI_DIR_NAME_LIMIT));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  emit_wasi_dir_failure(&mut b, fd, fd_close_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(remaining));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(truncated));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(next_cookie));
  b.emit(Instruction::LocalGet(cookie));
  b.emit(Instruction::I64Eq);
  b.emit(Instruction::If(BlockType::Empty));
  emit_wasi_dir_failure(&mut b, fd, fd_close_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(buffer));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(name));
  b.emit(Instruction::LocalGet(name));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::Call(utf8_valid_idx));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  emit_wasi_dir_failure(&mut b, fd, fd_close_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(name_index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(name_index));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(name));
  b.emit(Instruction::LocalGet(name_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalTee(name_byte));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(name_byte));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  emit_wasi_dir_failure(&mut b, fd, fd_close_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(name_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(name_index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(skip));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(name));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalSet(skip));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(name));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(name));
  b.emit(Instruction::I32Load8U(mem_arg_byte(1)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(skip));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(skip));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(WASI_DIR_ENTRY_LIMIT));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  emit_wasi_dir_failure(&mut b, fd, fd_close_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::LocalGet(separator_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(new_len));
  b.emit(Instruction::LocalGet(total_path_bytes));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(total_path_bytes));
  b.emit(Instruction::I32Const(WASI_DIR_PATH_BYTES_LIMIT));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  emit_wasi_dir_failure(&mut b, fd, fd_close_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(new_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(string_size));
  rt_emit_alloc_dynamic(&mut b, string_size, string, string_tag);
  b.emit(Instruction::LocalGet(string));
  b.emit(Instruction::LocalGet(new_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(string));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(string_data));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.emit(Instruction::LocalGet(separator_len));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(string_data));
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(string_data));
  b.emit(Instruction::LocalGet(path_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(separator_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(name));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::LocalSet(insert_at));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(insert_at));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(list));
  b.emit(Instruction::LocalGet(insert_at));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(previous));
  b.emit(Instruction::LocalGet(string));
  b.emit(Instruction::LocalGet(previous));
  b.emit(Instruction::Call(str_compare_idx));
  b.emit(Instruction::F64Const(Ieee64::from(0.0)));
  b.emit(Instruction::F64Ge);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(list));
  b.emit(Instruction::LocalGet(insert_at));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(previous));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(insert_at));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(insert_at));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(list));
  b.emit(Instruction::LocalGet(insert_at));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(string));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(next_cookie));
  b.emit(Instruction::LocalSet(cookie));
  b.emit(Instruction::LocalGet(complete));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(complete));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(offset));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(truncated));
  b.emit(Instruction::LocalGet(used));
  b.emit(Instruction::I32Const(WASI_DIR_BUFFER_SIZE));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  emit_wasi_dir_failure(&mut b, fd, fd_close_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(used));
  b.emit(Instruction::I32Const(WASI_DIR_BUFFER_SIZE));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(complete));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(cookie));
  b.emit(Instruction::LocalGet(page_cookie));
  b.emit(Instruction::I64Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  emit_wasi_dir_failure(&mut b, fd, fd_close_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(list));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(list));
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

/// Reserve a temporary region at the top of linear memory without advancing
/// the managed bump allocator. `managed_size_local` keeps allocations made
/// while the adapter is active below the scratch region.
fn rt_emit_reserve_scratch(builder: &mut RuntimeFnBuilder, size_local: u32, managed_size_local: u32, dst_local: u32) {
  let aligned_size = builder.alloc_i64();
  let memory_bytes = builder.alloc_i64();
  let required_bytes = builder.alloc_i64();

  builder.emit(Instruction::LocalGet(size_local));
  builder.emit(Instruction::I64Const(7));
  builder.emit(Instruction::I64Add);
  builder.emit(Instruction::I64Const(-8));
  builder.emit(Instruction::I64And);
  builder.emit(Instruction::LocalSet(aligned_size));
  builder.emit(Instruction::MemorySize(0));
  builder.emit(Instruction::I64ExtendI32U);
  builder.emit(Instruction::I64Const(16));
  builder.emit(Instruction::I64Shl);
  builder.emit(Instruction::LocalSet(memory_bytes));
  builder.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  builder.emit(Instruction::I64ExtendI32U);
  builder.emit(Instruction::LocalGet(managed_size_local));
  builder.emit(Instruction::I64Add);
  builder.emit(Instruction::LocalGet(aligned_size));
  builder.emit(Instruction::I64Add);
  builder.emit(Instruction::LocalSet(required_bytes));
  builder.emit(Instruction::LocalGet(required_bytes));
  builder.emit(Instruction::I64Const(u32::MAX as i64));
  builder.emit(Instruction::I64GtU);
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::Unreachable);
  builder.emit(Instruction::End);
  builder.emit(Instruction::LocalGet(required_bytes));
  builder.emit(Instruction::LocalGet(memory_bytes));
  builder.emit(Instruction::I64GtU);
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::LocalGet(required_bytes));
  builder.emit(Instruction::LocalGet(memory_bytes));
  builder.emit(Instruction::I64Sub);
  builder.emit(Instruction::I64Const(65535));
  builder.emit(Instruction::I64Add);
  builder.emit(Instruction::I64Const(16));
  builder.emit(Instruction::I64ShrU);
  builder.emit(Instruction::I32WrapI64);
  builder.emit(Instruction::MemoryGrow(0));
  builder.emit(Instruction::I32Const(-1));
  builder.emit(Instruction::I32Eq);
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::Unreachable);
  builder.emit(Instruction::End);
  builder.emit(Instruction::MemorySize(0));
  builder.emit(Instruction::I64ExtendI32U);
  builder.emit(Instruction::I64Const(16));
  builder.emit(Instruction::I64Shl);
  builder.emit(Instruction::LocalSet(memory_bytes));
  builder.emit(Instruction::End);
  builder.emit(Instruction::LocalGet(memory_bytes));
  builder.emit(Instruction::LocalGet(aligned_size));
  builder.emit(Instruction::I64Sub);
  builder.emit(Instruction::I32WrapI64);
  builder.emit(Instruction::LocalSet(dst_local));
}

/// Ensure bounded managed allocations fit in memory without trapping.
/// `status_local` is zero on success and non-zero on address overflow or a
/// failed `memory.grow`, so Result-returning adapters can fail explicitly.
fn rt_emit_try_reserve_managed(builder: &mut RuntimeFnBuilder, size_local: u32, status_local: u32) {
  let memory_bytes = builder.alloc_i64();
  let required_bytes = builder.alloc_i64();

  builder.emit(Instruction::I32Const(0));
  builder.emit(Instruction::LocalSet(status_local));
  builder.emit(Instruction::MemorySize(0));
  builder.emit(Instruction::I64ExtendI32U);
  builder.emit(Instruction::I64Const(16));
  builder.emit(Instruction::I64Shl);
  builder.emit(Instruction::LocalSet(memory_bytes));
  builder.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  builder.emit(Instruction::I64ExtendI32U);
  builder.emit(Instruction::LocalGet(size_local));
  builder.emit(Instruction::I64Add);
  builder.emit(Instruction::LocalSet(required_bytes));
  builder.emit(Instruction::LocalGet(required_bytes));
  builder.emit(Instruction::I64Const(u32::MAX as i64));
  builder.emit(Instruction::I64GtU);
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::I32Const(1));
  builder.emit(Instruction::LocalSet(status_local));
  builder.emit(Instruction::Else);
  builder.emit(Instruction::LocalGet(required_bytes));
  builder.emit(Instruction::LocalGet(memory_bytes));
  builder.emit(Instruction::I64GtU);
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::LocalGet(required_bytes));
  builder.emit(Instruction::LocalGet(memory_bytes));
  builder.emit(Instruction::I64Sub);
  builder.emit(Instruction::I64Const(65535));
  builder.emit(Instruction::I64Add);
  builder.emit(Instruction::I64Const(16));
  builder.emit(Instruction::I64ShrU);
  builder.emit(Instruction::I32WrapI64);
  builder.emit(Instruction::MemoryGrow(0));
  builder.emit(Instruction::I32Const(-1));
  builder.emit(Instruction::I32Eq);
  builder.emit(Instruction::LocalSet(status_local));
  builder.emit(Instruction::End);
  builder.emit(Instruction::End);
}

fn rt_emit_trap_on_errno(builder: &mut RuntimeFnBuilder) {
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::Unreachable);
  builder.emit(Instruction::End);
}

fn rt_emit_trap_if_i64_exceeds_memory32(builder: &mut RuntimeFnBuilder, value_local: u32) {
  builder.emit(Instruction::LocalGet(value_local));
  builder.emit(Instruction::I64Const(u32::MAX as i64));
  builder.emit(Instruction::I64GtU);
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::Unreachable);
  builder.emit(Instruction::End);
}

/// Build the Preview 1 argv adapter. The host buffers stay private while the
/// caller receives a normal tagged `List<String>` value.
pub(super) fn build_wasi_get_args_fn(args_sizes_get_idx: u32, args_get_idx: u32, str_new_idx: u32, list_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(0);
  let count = b.alloc_i32();
  let buffer_size = b.alloc_i32();
  let raw_size = b.alloc_i64();
  let managed_size = b.alloc_i64();
  let argv = b.alloc_i32();
  let buffer = b.alloc_i32();
  let buffer_end = b.alloc_i32();
  let list_size = b.alloc_i32();
  let list = b.alloc_i32();
  let index = b.alloc_i32();
  let arg_start = b.alloc_i32();
  let arg_end = b.alloc_i32();
  let next_arg_min = b.alloc_i32();

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::Call(args_sizes_get_idx));
  rt_emit_trap_on_errno(&mut b);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(buffer_size));

  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Const(4));
  b.emit(Instruction::I64Mul);
  b.emit(Instruction::LocalGet(buffer_size));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(raw_size));
  rt_emit_trap_if_i64_exceeds_memory32(&mut b, raw_size);
  // list header/payload plus per-string headers and worst-case alignment.
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Const(31));
  b.emit(Instruction::I64Mul);
  b.emit(Instruction::LocalGet(buffer_size));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Add);
  b.emit(Instruction::I64Const(16));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(managed_size));
  rt_emit_reserve_scratch(&mut b, raw_size, managed_size, argv);
  b.emit(Instruction::LocalGet(argv));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(buffer));
  b.emit(Instruction::LocalGet(buffer));
  b.emit(Instruction::LocalGet(buffer_size));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(buffer_end));
  b.emit(Instruction::LocalGet(argv));
  b.emit(Instruction::LocalGet(buffer));
  b.emit(Instruction::Call(args_get_idx));
  rt_emit_trap_on_errno(&mut b);

  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(list_size));
  rt_emit_alloc_dynamic(&mut b, list_size, list, list_tag);
  b.emit(Instruction::LocalGet(list));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::LocalGet(buffer));
  b.emit(Instruction::LocalSet(next_arg_min));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(argv));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalTee(arg_start));
  b.emit(Instruction::LocalGet(buffer));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(arg_start));
  b.emit(Instruction::LocalGet(next_arg_min));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(arg_start));
  b.emit(Instruction::LocalSet(arg_end));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(arg_end));
  b.emit(Instruction::LocalGet(buffer_end));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(arg_end));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(arg_end));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(arg_end));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(arg_end));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(next_arg_min));
  b.emit(Instruction::LocalGet(list));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(arg_start));
  b.emit(Instruction::LocalGet(arg_end));
  b.emit(Instruction::LocalGet(arg_start));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::Call(str_new_idx));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(list));
  b.emit(Instruction::F64ConvertI32U);
  b.finish(vec![], vec![ValType::F64])
}

/// Build the Preview 1 environment adapter. Missing keys return nil; malformed
/// host buffers and non-zero WASI errno values trap instead of becoming nil.
pub(super) fn build_wasi_get_env_fn(environ_sizes_get_idx: u32, environ_get_idx: u32, str_new_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let name_len = b.alloc_i32();
  let name_bytes = b.alloc_i32();
  let count = b.alloc_i32();
  let buffer_size = b.alloc_i32();
  let raw_size = b.alloc_i64();
  let managed_size = b.alloc_i64();
  let entries = b.alloc_i32();
  let buffer = b.alloc_i32();
  let buffer_end = b.alloc_i32();
  let index = b.alloc_i32();
  let entry = b.alloc_i32();
  let byte_index = b.alloc_i32();
  let matched = b.alloc_i32();
  let value_start = b.alloc_i32();
  let value_end = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(name_len));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(name_bytes));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::Call(environ_sizes_get_idx));
  rt_emit_trap_on_errno(&mut b);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(buffer_size));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Const(4));
  b.emit(Instruction::I64Mul);
  b.emit(Instruction::LocalGet(buffer_size));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(raw_size));
  rt_emit_trap_if_i64_exceeds_memory32(&mut b, raw_size);
  // At most one returned string is allocated while the scratch data is live.
  b.emit(Instruction::LocalGet(buffer_size));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Const(23));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(managed_size));
  rt_emit_reserve_scratch(&mut b, raw_size, managed_size, entries);
  b.emit(Instruction::LocalGet(entries));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(buffer));
  b.emit(Instruction::LocalGet(buffer));
  b.emit(Instruction::LocalGet(buffer_size));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(buffer_end));
  b.emit(Instruction::LocalGet(entries));
  b.emit(Instruction::LocalGet(buffer));
  b.emit(Instruction::Call(environ_get_idx));
  rt_emit_trap_on_errno(&mut b);

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(entries));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalTee(entry));
  b.emit(Instruction::LocalGet(buffer));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(matched));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(byte_index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(byte_index));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::LocalGet(byte_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(buffer_end));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::LocalGet(byte_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(name_bytes));
  b.emit(Instruction::LocalGet(byte_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(matched));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(byte_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(byte_index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(matched));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(buffer_end));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(61));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(value_start));
  b.emit(Instruction::LocalSet(value_end));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(value_end));
  b.emit(Instruction::LocalGet(buffer_end));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(value_end));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(value_end));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(value_end));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(value_start));
  b.emit(Instruction::LocalGet(value_end));
  b.emit(Instruction::LocalGet(value_start));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::Call(str_new_idx));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(f64_const(0.0));
  b.finish(vec![ValType::I32], vec![ValType::F64])
}

/// Lift one value from the WASI 0.3 environment list without exposing its
/// Canonical ABI tuple layout to Calcit source code.
pub(super) fn build_wasi_component_get_env_fn(environment_idx: u32, str_new_idx: u32, realloc_idx: u32, free_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let result = b.alloc_i32();
  let entries = b.alloc_i32();
  let count = b.alloc_i32();
  let index = b.alloc_i32();
  let entry = b.alloc_i32();
  let name_len = b.alloc_i32();
  let byte_index = b.alloc_i32();
  let key_ptr = b.alloc_i32();
  let found = b.alloc_f64();

  b.emit(f64_const(0.0));
  b.emit(Instruction::LocalSet(found));

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(name_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::Call(realloc_idx));
  b.emit(Instruction::LocalTee(result));
  b.emit(Instruction::Call(environment_idx));
  b.emit(Instruction::LocalGet(result));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(entries));
  b.emit(Instruction::LocalGet(result));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(entries));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(entry));
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(key_ptr));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(byte_index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(byte_index));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(key_ptr));
  b.emit(Instruction::LocalGet(byte_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(byte_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(byte_index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(byte_index));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalGet(entry));
  b.emit(Instruction::I32Load(mem_arg_i32(12)));
  b.emit(Instruction::Call(str_new_idx));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  // The selected value has been copied into a Calcit-owned string. Reclaim
  // every Canonical ABI allocation, including entries we did not match.
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(entries));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(entry));
  for (ptr_offset, len_offset) in [(0, 4), (8, 12)] {
    b.emit(Instruction::LocalGet(entry));
    b.emit(Instruction::I32Load(mem_arg_i32(ptr_offset)));
    b.emit(Instruction::LocalGet(entry));
    b.emit(Instruction::I32Load(mem_arg_i32(len_offset)));
    b.emit(Instruction::Call(free_idx));
  }
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(entries));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::Call(free_idx));
  b.emit(Instruction::LocalGet(result));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::Call(free_idx));
  b.emit(Instruction::LocalGet(found));
  b.finish(vec![ValType::I32], vec![ValType::F64])
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
  // Canonical types cover every arity a call_indirect uses (not just 0..8).
  let canonical_arity = fns
    .iter()
    .flat_map(|f| f.instructions.iter())
    .filter_map(|instruction| match instruction {
      Instruction::CallIndirect { type_index, .. } => Some(*type_index + 1),
      _ => None,
    })
    .fold(MAX_CANONICAL_ARITY, u32::max);

  // Type section:
  //   0..MAX_CANONICAL_ARITY  — canonical HOF callback types: (f64×N) → f64
  //   MAX_CANONICAL_ARITY + 0..host_imports.len() — host import types
  //   MAX_CANONICAL_ARITY + host_imports.len() + 0..fns.len() — compiled fn types
  let mut types = TypeSection::new();
  for arity in 0..canonical_arity {
    types.ty().function(vec![ValType::F64; arity as usize], vec![ValType::F64]);
  }
  for imp in host_imports {
    types.ty().function(imp.params.clone(), imp.results.clone());
  }
  for f in fns {
    types.ty().function(f.params.clone(), f.results.clone());
  }
  module.section(&types);

  // Import section: host functions (type indices shifted by MAX_CANONICAL_ARITY)
  let mut imports = wasm_encoder::ImportSection::new();
  for (i, imp) in host_imports.iter().enumerate() {
    imports.import(
      imp.module.as_str(),
      imp.name.as_str(),
      wasm_encoder::EntityType::Function(canonical_arity + i as u32),
    );
  }
  module.section(&imports);

  // Function section: map each user function to its type
  let mut functions = FunctionSection::new();
  for (i, _) in fns.iter().enumerate() {
    functions.function(canonical_arity + num_imports + i as u32);
  }
  module.section(&functions);

  // Table section: funcref table N serves function-value calls with N
  // arguments. Populated from reachable table indices only so pooling hosts
  // with a 1-table limit (such as `wasmtime serve`) stay compatible.
  let used_tables = &linked.used_tables;
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
      exports.export(export_name, ExportKind::Func, num_imports + i as u32);
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
  let enum_tag = require_tag("enum");
  let set_tag = require_tag("set");
  let struct_tag = require_tag("struct");
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
      nil: nil_tag,
      bool_tag,
      number: number_tag,
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
  let set_find_instructions = vec![
    Instruction::LocalGet(0),
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
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(3),
    Instruction::I32Const(8),
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
    locals: vec![ValType::I32, ValType::I32, ValType::I32],
    instructions: set_find_instructions,
  });

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
  // Deep equality for strings, containers, and nominal payloads.
  let value_equal_idx = base_index + fns.len() as u32;
  let map_equal_idx = value_equal_idx + 1;
  let set_equal_idx = value_equal_idx + 2;
  fn_index.insert(String::from("__rt_value_equal"), value_equal_idx);
  fns.push(build_rt_value_equal(
    (
      string_tag, list_tag, enum_tag, map_tag, set_tag, struct_tag, nil_tag, bool_tag, number_tag,
    ),
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
  fns.push(build_rt_map_equal(map_linearize_idx, map_get_value_idx, value_equal_idx));

  fn_index.insert(String::from("__rt_set_equal"), set_equal_idx);
  fns.push(build_rt_set_equal(value_equal_idx));

  fns[value_hash_slot] = build_rt_value_hash(
    (
      string_tag, list_tag, enum_tag, map_tag, set_tag, struct_tag, nil_tag, bool_tag, number_tag,
    ),
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

const RT_MAP_TABLE_KIND: f64 = 0.0;
const RT_MAP_BUCKET_KIND: f64 = 1.0;
const RT_MAP_TABLE_CHILDREN: i32 = 32;
const RT_MAP_TABLE_SLOTS: i32 = 33; // kind + 32 child pointers
const RT_MAP_TABLE_BYTES: i32 = RT_MAP_TABLE_SLOTS * 8;

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

fn rt_emit_alloc_const(builder: &mut RuntimeFnBuilder, byte_size: i32, dst_local: u32, type_tag: i32) {
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

fn rt_emit_alloc_dynamic(builder: &mut RuntimeFnBuilder, size_local: u32, dst_local: u32, type_tag: i32) {
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
fn rt_emit_ensure_heap_capacity(builder: &mut RuntimeFnBuilder, needed_bytes_local: u32) {
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

fn rt_emit_ensure_heap_capacity_const(builder: &mut RuntimeFnBuilder, needed_bytes: i32, scratch_local: u32) {
  builder.emit(Instruction::I32Const(needed_bytes));
  builder.emit(Instruction::LocalSet(scratch_local));
  rt_emit_ensure_heap_capacity(builder, scratch_local);
}

fn rt_emit_table_child_addr(builder: &mut RuntimeFnBuilder, table_local: u32, child_idx_local: u32, dst_local: u32) {
  builder.emit(Instruction::LocalGet(table_local));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalGet(child_idx_local));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Mul);
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(dst_local));
}

fn rt_emit_load_table_child(builder: &mut RuntimeFnBuilder, table_local: u32, child_idx_local: u32, dst_local: u32) {
  builder.emit(Instruction::LocalGet(table_local));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalGet(child_idx_local));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Mul);
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::F64Load(mem_arg_f64(0)));
  builder.emit(Instruction::I32TruncF64U);
  builder.emit(Instruction::LocalSet(dst_local));
}

fn rt_emit_alloc_empty_table(builder: &mut RuntimeFnBuilder, dst_local: u32) {
  rt_emit_alloc_const(builder, RT_MAP_TABLE_BYTES, dst_local, 0);
  builder.emit(Instruction::LocalGet(dst_local));
  builder.emit(f64_const(RT_MAP_TABLE_KIND));
  builder.emit(Instruction::F64Store(mem_arg_f64(0)));

  let i = builder.alloc_i32();
  let addr = builder.alloc_i32();
  builder.emit(Instruction::I32Const(0));
  builder.emit(Instruction::LocalSet(i));
  builder.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  builder.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  builder.emit(Instruction::LocalGet(i));
  builder.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  builder.emit(Instruction::I32GeU);
  builder.emit(Instruction::BrIf(1));
  rt_emit_table_child_addr(builder, dst_local, i, addr);
  builder.emit(Instruction::LocalGet(addr));
  builder.emit(f64_const(0.0));
  builder.emit(Instruction::F64Store(mem_arg_f64(0)));
  builder.emit(Instruction::LocalGet(i));
  builder.emit(Instruction::I32Const(1));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(i));
  builder.emit(Instruction::Br(0));
  builder.emit(Instruction::End);
  builder.emit(Instruction::End);
}

fn rt_emit_alloc_bucket(builder: &mut RuntimeFnBuilder, count_local: u32, dst_local: u32) {
  let slots = builder.alloc_i32();
  let size = builder.alloc_i32();
  builder.emit(Instruction::LocalGet(count_local));
  builder.emit(Instruction::I32Const(2));
  builder.emit(Instruction::I32Mul);
  builder.emit(Instruction::I32Const(2));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(slots));
  builder.emit(Instruction::LocalGet(slots));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Mul);
  builder.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(builder, size, dst_local, 0);
  builder.emit(Instruction::LocalGet(dst_local));
  builder.emit(f64_const(RT_MAP_BUCKET_KIND));
  builder.emit(Instruction::F64Store(mem_arg_f64(0)));
  builder.emit(Instruction::LocalGet(dst_local));
  builder.emit(Instruction::LocalGet(count_local));
  builder.emit(Instruction::F64ConvertI32U);
  builder.emit(Instruction::F64Store(mem_arg_f64(8)));
}

fn rt_emit_copy_slots(builder: &mut RuntimeFnBuilder, copy_fn_idx: u32, dst_local: u32, src_local: u32, count_local: u32) {
  builder.emit(Instruction::LocalGet(dst_local));
  builder.emit(Instruction::LocalGet(src_local));
  builder.emit(Instruction::LocalGet(count_local));
  builder.emit(Instruction::Call(copy_fn_idx));
}

fn build_rt_f64_value_eq() -> CompiledFn {
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

fn build_rt_hash_f64() -> CompiledFn {
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
fn build_rt_struct_load_field() -> CompiledFn {
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
fn build_rt_struct_store_field(is_heap_ptr_idx: u32, bool_tag: i32) -> CompiledFn {
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

fn build_rt_value_hash(
  (string_tag, list_tag, enum_tag, map_tag, set_tag, struct_tag, nil_tag, bool_tag, number_tag): (
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
  ),
  (scalar_hash_idx, self_idx, map_linearize_idx, is_heap_ptr_idx, struct_load_field_idx): (u32, u32, u32, u32, u32),
  struct_layouts: &StructLayouts,
) -> CompiledFn {
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
  for (index, kind) in [string_tag, list_tag, enum_tag, struct_tag, set_tag, map_tag]
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
  // Strings use byte content, not allocation identity.
  b.emit(Instruction::LocalGet(tag));
  b.emit(Instruction::I32Const(string_tag));
  b.emit(Instruction::I32Eq);
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
  b.emit(Instruction::LocalGet(hash));
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

fn build_rt_map_key_equal(value_equal_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(value_equal_idx));
  b.finish(vec![ValType::F64, ValType::F64], vec![ValType::I32])
}

fn build_rt_map_make(map_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let dst = b.alloc_i32();
  rt_emit_alloc_const(&mut b, 16, dst, map_tag);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(8)));
  b.emit(Instruction::LocalGet(dst));
  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}

fn build_rt_map_root_assoc(copy_fn_idx: u32, key_equal_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(4); // root, key, value, hash
  let idx0 = b.alloc_i32();
  let idx1 = b.alloc_i32();
  let table1 = b.alloc_i32();
  let bucket = b.alloc_i32();
  let new_root = b.alloc_i32();
  let new_table1 = b.alloc_i32();
  let new_bucket = b.alloc_i32();
  let bucket_count = b.alloc_i32();
  let found_idx = b.alloc_i32();
  let i = b.alloc_i32();
  let slots = b.alloc_i32();
  let addr = b.alloc_i32();
  let added = b.alloc_i32();
  let key_addr = b.alloc_i32();

  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(31));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(idx0));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(5));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(31));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(idx1));

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  rt_emit_alloc_empty_table(&mut b, new_root);
  rt_emit_alloc_empty_table(&mut b, new_table1);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(bucket_count));
  rt_emit_alloc_bucket(&mut b, bucket_count, new_bucket);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Store(mem_arg_f64(16)));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(24)));
  rt_emit_table_child_addr(&mut b, new_table1, idx1, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  rt_emit_table_child_addr(&mut b, new_root, idx0, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_table1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::Else);

  rt_emit_load_table_child(&mut b, 0, idx0, table1);
  b.emit(Instruction::LocalGet(table1));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_root, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_root, 0, slots);
  rt_emit_alloc_empty_table(&mut b, new_table1);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(bucket_count));
  rt_emit_alloc_bucket(&mut b, bucket_count, new_bucket);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Store(mem_arg_f64(16)));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(24)));
  rt_emit_table_child_addr(&mut b, new_table1, idx1, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  rt_emit_table_child_addr(&mut b, new_root, idx0, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_table1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::Else);

  rt_emit_load_table_child(&mut b, table1, idx1, bucket);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_root, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_root, 0, slots);
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_table1, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_table1, table1, slots);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(bucket_count));
  rt_emit_alloc_bucket(&mut b, bucket_count, new_bucket);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Store(mem_arg_f64(16)));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(24)));
  rt_emit_table_child_addr(&mut b, new_table1, idx1, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  rt_emit_table_child_addr(&mut b, new_root, idx0, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_table1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::Else);

  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(bucket_count));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(found_idx));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(key_addr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(key_equal_idx));
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalSet(found_idx));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  rt_emit_alloc_bucket(&mut b, slots, new_bucket);
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_bucket, bucket, slots);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(8)));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(addr));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(8)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::Else);
  rt_emit_alloc_bucket(&mut b, bucket_count, new_bucket);
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_bucket, bucket, slots);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::End);

  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_root, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_root, 0, slots);
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_table1, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_table1, table1, slots);
  rt_emit_table_child_addr(&mut b, new_table1, idx1, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  rt_emit_table_child_addr(&mut b, new_root, idx0, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_table1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(new_root));
  b.emit(Instruction::LocalGet(added));

  b.finish(
    vec![ValType::I32, ValType::F64, ValType::F64, ValType::I32],
    vec![ValType::I32, ValType::I32],
  )
}

fn build_rt_map_root_lookup(key_equal_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3); // root, key, hash
  let idx0 = b.alloc_i32();
  let idx1 = b.alloc_i32();
  let table1 = b.alloc_i32();
  let bucket = b.alloc_i32();
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let found = b.alloc_i32();
  let value = b.alloc_f64();

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(found));
  b.emit(f64_const(0.0));
  b.emit(Instruction::LocalSet(value));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(31));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(idx0));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(5));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(31));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(idx1));
  rt_emit_load_table_child(&mut b, 0, idx0, table1);
  b.emit(Instruction::LocalGet(table1));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  rt_emit_load_table_child(&mut b, table1, idx1, bucket);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(key_equal_idx));
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(value));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(found));
  b.emit(Instruction::LocalGet(value));
  b.finish(vec![ValType::I32, ValType::F64, ValType::I32], vec![ValType::I32, ValType::F64])
}

fn build_rt_map_root_contains_value() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2); // root, target
  let i0 = b.alloc_i32();
  let i1 = b.alloc_i32();
  let table1 = b.alloc_i32();
  let bucket = b.alloc_i32();
  let count = b.alloc_i32();
  let bi = b.alloc_i32();
  let found = b.alloc_i32();
  let entry_value = b.alloc_f64();

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i0));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i0));
  b.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  rt_emit_load_table_child(&mut b, 0, i0, table1);
  b.emit(Instruction::LocalGet(table1));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i1));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i1));
  b.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  rt_emit_load_table_child(&mut b, table1, i1, bucket);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(bi));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalTee(entry_value));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Eq);
  // NaN equals NaN as a value
  b.emit(Instruction::LocalGet(entry_value));
  b.emit(Instruction::LocalGet(entry_value));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::Br(6));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(bi));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i1));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i1));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i0));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i0));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(found));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

fn build_rt_map_root_write_pairs() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3); // root, dst_base, write_idx
  let i0 = b.alloc_i32();
  let i1 = b.alloc_i32();
  let table1 = b.alloc_i32();
  let bucket = b.alloc_i32();
  let count = b.alloc_i32();
  let bi = b.alloc_i32();
  let out = b.alloc_i32();
  let addr = b.alloc_i32();
  let tmp = b.alloc_f64();

  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalSet(out));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i0));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i0));
  b.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  rt_emit_load_table_child(&mut b, 0, i0, table1);
  b.emit(Instruction::LocalGet(table1));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i1));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i1));
  b.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  rt_emit_load_table_child(&mut b, table1, i1, bucket);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(bi));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));

  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(addr));

  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(tmp));
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(tmp));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));

  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(tmp));
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(tmp));
  b.emit(Instruction::F64Store(mem_arg_f64(8)));

  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out));
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(bi));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i1));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i1));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i0));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i0));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(out));
  b.finish(vec![ValType::I32, ValType::I32, ValType::I32], vec![ValType::I32])
}

fn build_rt_map_from_flat(hash_idx: u32, root_assoc_idx: u32, map_make_idx: u32) -> CompiledFn {
  build_rt_map_from_flat_impl(hash_idx, root_assoc_idx, map_make_idx, false)
}

fn build_rt_map_from_flat_checked(hash_idx: u32, root_assoc_idx: u32, map_make_idx: u32) -> CompiledFn {
  build_rt_map_from_flat_impl(hash_idx, root_assoc_idx, map_make_idx, true)
}

fn build_rt_map_from_flat_impl(hash_idx: u32, root_assoc_idx: u32, map_make_idx: u32, checked: bool) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let root = b.alloc_i32();
  let actual = b.alloc_i32();
  let key = b.alloc_f64();
  let val = b.alloc_f64();
  let hash = b.alloc_i32();
  let added = b.alloc_i32();
  let req_bytes = if checked { Some(b.alloc_i32()) } else { None };

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(actual));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(key));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(val));
  if let Some(req) = req_bytes {
    // Each assoc allocates at most two 272-byte HAMT tables plus one bucket of up to `actual + 1` entries (`16 * (actual + 1) + 24`).
    b.emit(Instruction::LocalGet(actual));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::I32Const(16));
    b.emit(Instruction::I32Mul);
    b.emit(Instruction::I32Const(568));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(req));
    rt_emit_ensure_heap_capacity(&mut b, req);
    b.emit(Instruction::I32Eqz);
    b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::Return);
    b.emit(Instruction::End);
  }
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::Call(hash_idx));
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Call(root_assoc_idx));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(actual));
  b.emit(Instruction::LocalGet(added));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(actual));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  if let Some(req) = req_bytes {
    rt_emit_ensure_heap_capacity_const(&mut b, 24, req);
    b.emit(Instruction::I32Eqz);
    b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::Return);
    b.emit(Instruction::End);
  }
  b.emit(Instruction::LocalGet(actual));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::Call(map_make_idx));
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

fn build_rt_map_root_from_flat(hash_idx: u32, root_assoc_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let root = b.alloc_i32();
  let actual = b.alloc_i32();
  let key = b.alloc_f64();
  let val = b.alloc_f64();
  let hash = b.alloc_i32();
  let added = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(actual));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(key));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(val));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::Call(hash_idx));
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Call(root_assoc_idx));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(actual));
  b.emit(Instruction::LocalGet(added));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(actual));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(actual));
  b.emit(Instruction::LocalGet(root));
  b.finish(vec![ValType::I32], vec![ValType::I32, ValType::I32])
}

fn build_rt_map_linearize(root_write_pairs_idx: u32, list_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let count = b.alloc_i32();
  let root = b.alloc_i32();
  let slots = b.alloc_i32();
  let size = b.alloc_i32();
  let dst = b.alloc_i32();
  let base = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  b.emit(Instruction::LocalGet(slots));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(&mut b, size, dst, list_tag);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(base));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(base));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Call(root_write_pairs_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::LocalGet(dst));
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

fn build_rt_map_assoc(hash_idx: u32, root_assoc_idx: u32, map_make_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3);
  let count = b.alloc_i32();
  let root = b.alloc_i32();
  let hash = b.alloc_i32();
  let added = b.alloc_i32();
  let new_count = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(hash_idx));
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Call(root_assoc_idx));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::LocalGet(added));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(new_count));
  b.emit(Instruction::LocalGet(new_count));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::Call(map_make_idx));
  b.finish(vec![ValType::I32, ValType::F64, ValType::F64], vec![ValType::I32])
}

fn build_rt_map_get_value(hash_idx: u32, root_lookup_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let root = b.alloc_i32();
  let hash = b.alloc_i32();
  let found = b.alloc_i32();
  let value = b.alloc_f64();
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(hash_idx));
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Call(root_lookup_idx));
  b.emit(Instruction::LocalSet(value));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(value));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::F64])
}

fn build_rt_map_contains_key(hash_idx: u32, root_lookup_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let root = b.alloc_i32();
  let hash = b.alloc_i32();
  let found = b.alloc_i32();
  let value = b.alloc_f64();
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(hash_idx));
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Call(root_lookup_idx));
  b.emit(Instruction::LocalSet(value));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(found));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

fn build_rt_map_contains_value(root_contains_value_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let root = b.alloc_i32();
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(root_contains_value_idx));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

fn build_rt_map_dissoc(copy_fn_idx: u32, map_linearize_idx: u32, map_from_flat_idx: u32, key_equal_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let count = b.alloc_i32();
  let flat = b.alloc_i32();
  let found = b.alloc_i32();
  let found_idx = b.alloc_i32();
  let i = b.alloc_i32();
  let new_count = b.alloc_i32();
  let slots = b.alloc_i32();
  let size = b.alloc_i32();
  let dst = b.alloc_i32();
  let before_slots = b.alloc_i32();
  let after_slots = b.alloc_i32();
  let dst_base = b.alloc_i32();
  let src_base = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(map_linearize_idx));
  b.emit(Instruction::LocalSet(flat));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(found_idx));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(key_equal_idx));
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalSet(found_idx));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(found));
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(new_count));
  b.emit(Instruction::LocalGet(new_count));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  b.emit(Instruction::LocalGet(slots));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(&mut b, size, dst, 0);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(new_count));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(before_slots));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst_base));
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(src_base));
  rt_emit_copy_slots(&mut b, copy_fn_idx, dst_base, src_base, before_slots);
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(after_slots));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst_base));
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(src_base));
  rt_emit_copy_slots(&mut b, copy_fn_idx, dst_base, src_base, after_slots);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::Call(map_from_flat_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

/// `__rt_str_compare(ptr_a: i32, ptr_b: i32) → f64`
///
/// Lexicographic byte comparison. Returns -1.0, 0.0, or 1.0.
/// Strings are UTF-8 in linear memory at logical_ptr+8, length at logical_ptr+0 (f64).
fn build_rt_str_compare() -> CompiledFn {
  // params: 0 = ptr_a (i32), 1 = ptr_b (i32)
  // locals: 2 = len_a, 3 = len_b, 4 = min_len, 5 = i, 6 = byte_a, 7 = byte_b
  let instructions = vec![
    // len_a = i32.trunc(f64.load ptr_a+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    // len_b = i32.trunc(f64.load ptr_b+0)
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // min_len = min(len_a, len_b)
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)),
    Instruction::LocalGet(2),
    Instruction::Else,
    Instruction::LocalGet(3),
    Instruction::End,
    Instruction::LocalSet(4),
    // i = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(5),
    // block $outer (result f64) — used for early return via Br(2)
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // block $break_loop — exit loop via BrIf(1) or Br(1)
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // loop $loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if i >= min_len: exit loop
    Instruction::LocalGet(5),
    Instruction::LocalGet(4),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    // byte_a = i32.load8u(ptr_a + 8 + i)
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(5),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(6),
    // byte_b = i32.load8u(ptr_b + 8 + i)
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(5),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(7),
    // if byte_a < byte_b: push -1.0, br $outer (depth 3 from inside if)
    Instruction::LocalGet(6),
    Instruction::LocalGet(7),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(-1.0f64)),
    Instruction::Br(3),
    Instruction::End,
    // if byte_a > byte_b: push 1.0, br $outer
    Instruction::LocalGet(6),
    Instruction::LocalGet(7),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    Instruction::Br(3),
    Instruction::End,
    // i += 1; continue loop
    Instruction::LocalGet(5),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(5),
    Instruction::Br(0),
    Instruction::End, // end loop
    Instruction::End, // end $break_loop
    // After loop: compare lengths to determine result
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)),
    Instruction::F64Const(Ieee64::from(-1.0f64)),
    Instruction::Else,
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    Instruction::Else,
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::End,
    Instruction::End,
    Instruction::End, // end $outer
  ];

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // len_a
      ValType::I32, // len_b
      ValType::I32, // min_len
      ValType::I32, // i
      ValType::I32, // byte_a
      ValType::I32, // byte_b
    ],
    instructions,
  }
}

/// `__rt_str_find_index(h_ptr: i32, n_ptr: i32) → f64`
///
/// Naive byte-level substring search. Returns the Unicode scalar index of the first
/// occurrence of the needle string in the haystack, or -1.0 if not found.
/// An empty needle always returns 0.0.
fn build_rt_str_find_index() -> CompiledFn {
  // params: 0=h_ptr (i32), 1=n_ptr (i32)
  // locals: 2=h_len(i32), 3=n_len(i32), 4=h_base(i32), 5=n_base(i32),
  //         6=i(i32), 7=j(i32), 8=limit(i32), 9=byte_h(i32), 10=byte_n(i32), 11=scalar(i32)
  let instructions = vec![
    // h_len = i32(f64.load h_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    // n_len = i32(f64.load n_ptr+0)
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // h_base = h_ptr + 8
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(4),
    // n_base = n_ptr + 8
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(5),
    // Block $outer (result f64) — used for early-exit via Br
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if n_len == 0: push 0.0, br $outer
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // if n_len > h_len: push -1.0, br $outer
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(-1.0f64)),
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // limit = h_len - n_len  (last valid start position)
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32Sub,
    Instruction::LocalSet(8),
    // i = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(6),
    // Block $exit_outer (empty) — outer loop exit
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // Loop $outer_loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if i > limit: br $exit_outer
    Instruction::LocalGet(6),
    Instruction::LocalGet(8),
    Instruction::I32GtU,
    Instruction::BrIf(1), // 0=$outer_loop(continue), 1=$exit_outer(break)
    // j = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(7),
    // Block $mismatch (empty) — inner loop exit on mismatch
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // Loop $inner_loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if j >= n_len: found at byte i. Count scalars in [0, i) only on success.
    Instruction::LocalGet(7),
    Instruction::LocalGet(3),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(7), // reuse j as prefix cursor
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(7),
    Instruction::LocalGet(6),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    // A valid UTF-8 scalar starts at every non-continuation byte.
    // j < i <= h_len - n_len keeps every load inside the haystack.
    Instruction::LocalGet(11),
    Instruction::LocalGet(4),
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::I32Const(0xc0),
    Instruction::I32And,
    Instruction::I32Const(0x80),
    Instruction::I32Ne,
    Instruction::I32Add,
    Instruction::LocalSet(11),
    Instruction::LocalGet(7),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(7),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(11),
    Instruction::F64ConvertI32U,
    // depths inside If: 0=If,1=$inner_loop,2=$mismatch,3=$outer_loop,4=$exit_outer,5=$outer
    Instruction::Br(5),
    Instruction::End,
    // byte_h = h_base[i + j]
    Instruction::LocalGet(4),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(9),
    // byte_n = n_base[j]
    Instruction::LocalGet(5),
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(10),
    // if byte_h != byte_n: br $mismatch
    Instruction::LocalGet(9),
    Instruction::LocalGet(10),
    Instruction::I32Ne,
    Instruction::BrIf(1), // 0=$inner_loop, 1=$mismatch
    // j++
    Instruction::LocalGet(7),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(7),
    Instruction::Br(0), // continue $inner_loop
    Instruction::End,   // end $inner_loop
    Instruction::End,   // end $mismatch
    // i++
    Instruction::LocalGet(6),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(6),
    Instruction::Br(0), // continue $outer_loop
    Instruction::End,   // end $outer_loop
    Instruction::End,   // end $exit_outer
    // Not found: push -1.0 (result of $outer)
    Instruction::F64Const(Ieee64::from(-1.0f64)),
    Instruction::End, // end $outer (result f64)
  ];

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // h_len (2)
      ValType::I32, // n_len (3)
      ValType::I32, // h_base (4)
      ValType::I32, // n_base (5)
      ValType::I32, // i (6)
      ValType::I32, // j (7)
      ValType::I32, // limit (8)
      ValType::I32, // byte_h (9)
      ValType::I32, // byte_n (10)
      ValType::I32, // scalar (11), zero-initialized on entry
    ],
    instructions,
  }
}

/// `__rt_str_starts_with(s_ptr: i32, p_ptr: i32) → f64`
///
/// Returns 1.0 if the string at s_ptr starts with the prefix at p_ptr, else 0.0.
/// An empty prefix always returns 1.0.
fn build_rt_str_starts_with() -> CompiledFn {
  // params: 0=s_ptr(i32), 1=p_ptr(i32)
  // locals: 2=s_len(i32), 3=p_len(i32), 4=s_base(i32), 5=p_base(i32),
  //         6=i(i32), 7=byte_s(i32), 8=byte_p(i32)
  let instructions = vec![
    // s_len = i32(f64.load s_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    // p_len = i32(f64.load p_ptr+0)
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // s_base = s_ptr + 8
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(4),
    // p_base = p_ptr + 8
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(5),
    // Block $outer (result f64) — early-exit via Br
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if p_len == 0: push 1.0, br $outer (empty prefix always matches)
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // if p_len > s_len: push 0.0, br $outer
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // i = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(6),
    // Block $fail (empty) — break here on first mismatch
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // Loop $loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if i >= p_len: all bytes matched → push 1.0, br $outer
    Instruction::LocalGet(6),
    Instruction::LocalGet(3),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    // depths: 0=If, 1=$loop, 2=$fail, 3=$outer
    Instruction::Br(3),
    Instruction::End,
    // byte_s = s_base[i]
    Instruction::LocalGet(4),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(7),
    // byte_p = p_base[i]
    Instruction::LocalGet(5),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(8),
    // if byte_s != byte_p: br $fail
    Instruction::LocalGet(7),
    Instruction::LocalGet(8),
    Instruction::I32Ne,
    Instruction::BrIf(1), // 0=$loop, 1=$fail
    // i++
    Instruction::LocalGet(6),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(6),
    Instruction::Br(0), // continue $loop
    Instruction::End,   // end $loop
    Instruction::End,   // end $fail
    // Mismatch: push 0.0 (result of $outer)
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::End, // end $outer
  ];

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // s_len (2)
      ValType::I32, // p_len (3)
      ValType::I32, // s_base (4)
      ValType::I32, // p_base (5)
      ValType::I32, // i (6)
      ValType::I32, // byte_s (7)
      ValType::I32, // byte_p (8)
    ],
    instructions,
  }
}

/// `__rt_str_ends_with(s_ptr: i32, suf_ptr: i32) → f64`
///
/// Returns 1.0 if the string at s_ptr ends with the suffix at suf_ptr, else 0.0.
/// An empty suffix always returns 1.0.
fn build_rt_str_ends_with() -> CompiledFn {
  // params: 0=s_ptr(i32), 1=suf_ptr(i32)
  // locals: 2=s_len(i32), 3=suf_len(i32), 4=s_base(i32), 5=suf_base(i32),
  //         6=offset(i32), 7=i(i32), 8=byte_s(i32), 9=byte_suf(i32)
  let instructions = vec![
    // s_len = i32(f64.load s_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    // suf_len = i32(f64.load suf_ptr+0)
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // s_base = s_ptr + 8
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(4),
    // suf_base = suf_ptr + 8
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(5),
    // Block $outer (result f64)
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if suf_len == 0: push 1.0, br $outer
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    Instruction::Br(1),
    Instruction::End,
    // if suf_len > s_len: push 0.0, br $outer
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::Br(1),
    Instruction::End,
    // offset = s_len - suf_len  (start byte position in s for comparison)
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32Sub,
    Instruction::LocalSet(6),
    // i = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(7),
    // Block $fail (empty)
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // Loop $loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if i >= suf_len: push 1.0, br $outer
    Instruction::LocalGet(7),
    Instruction::LocalGet(3),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    // depths: 0=If, 1=$loop, 2=$fail, 3=$outer
    Instruction::Br(3),
    Instruction::End,
    // byte_s = s_base[offset + i]
    Instruction::LocalGet(4),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(8),
    // byte_suf = suf_base[i]
    Instruction::LocalGet(5),
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(9),
    // if byte_s != byte_suf: br $fail
    Instruction::LocalGet(8),
    Instruction::LocalGet(9),
    Instruction::I32Ne,
    Instruction::BrIf(1),
    // i++
    Instruction::LocalGet(7),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(7),
    Instruction::Br(0), // continue $loop
    Instruction::End,   // end $loop
    Instruction::End,   // end $fail
    // Mismatch: push 0.0
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::End, // end $outer
  ];

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // s_len (2)
      ValType::I32, // suf_len (3)
      ValType::I32, // s_base (4)
      ValType::I32, // suf_base (5)
      ValType::I32, // offset (6)
      ValType::I32, // i (7)
      ValType::I32, // byte_s (8)
      ValType::I32, // byte_suf (9)
    ],
    instructions,
  }
}

/// `__rt_f64_to_str(value: f64) → f64` (logical string pointer)
/// Converts a number to a heap-allocated string.
/// Handles every finite f64 and the language's special values.
fn emit_f64_const_text(b: &mut Vec<Instruction<'static>>, text: &str, string_tag: i32) {
  let padded_len = (text.len() + 7) & !7;
  b.extend([
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::LocalTee(11),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(11),
    Instruction::I32Const(4),
    Instruction::I32Add,
    Instruction::I32Const(string_tag),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(11),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalTee(7),
    Instruction::F64Const(Ieee64::from(text.len() as f64)),
    Instruction::F64Store(mem_arg_f64(0)),
  ]);
  for (offset, byte) in text.bytes().enumerate() {
    b.extend([
      Instruction::LocalGet(7),
      Instruction::I32Const(8 + offset as i32),
      Instruction::I32Add,
      Instruction::I32Const(byte as i32),
      Instruction::I32Store8(mem_arg_byte(0)),
    ]);
  }
  b.extend([
    Instruction::LocalGet(11),
    Instruction::I32Const((16 + padded_len) as i32),
    Instruction::I32Add,
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::LocalGet(7),
    Instruction::F64ConvertI32U,
    Instruction::Return,
  ]);
}

fn build_rt_f64_to_str(string_tag: i32, ryu_format_idx: u32) -> CompiledFn {
  // param 0: value (f64)
  // locals: 1=raw_i64(i64), 2=neg(i32), 3=abs_i64(i64), 4=ndigits(i32),
  //          5=tmp_i64(i64), 6=payload(i32), 7=str_ptr(i32), 8=content(i32),
  //          9=pos(i32), 10=digit(i32), 11=raw_base(i32)
  let mut b = Vec::new();

  // Preserve native spelling for non-finite values and signed zero.
  b.extend([
    Instruction::LocalGet(0),
    Instruction::LocalGet(0),
    Instruction::F64Ne,
    Instruction::If(BlockType::Empty),
  ]);
  emit_f64_const_text(&mut b, "NaN", string_tag);
  b.push(Instruction::End);
  for (value, text) in [(f64::INFINITY, "inf"), (f64::NEG_INFINITY, "-inf")] {
    b.extend([
      Instruction::LocalGet(0),
      Instruction::F64Const(Ieee64::from(value)),
      Instruction::F64Eq,
      Instruction::If(BlockType::Empty),
    ]);
    emit_f64_const_text(&mut b, text, string_tag);
    b.push(Instruction::End);
  }
  b.extend([
    Instruction::LocalGet(0),
    Instruction::I64ReinterpretF64,
    Instruction::I64Const(i64::MIN),
    Instruction::I64Eq,
    Instruction::If(BlockType::Empty),
  ]);
  emit_f64_const_text(&mut b, "-0", string_tag);
  b.extend([Instruction::End, Instruction::LocalGet(0)]);

  // Preserve the cheap integer path; all other finite values use the pinned
  // shortest-decimal formatter and the same canonical expansion as native.
  b.push(Instruction::F64Floor);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Eq);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(-9007199254740992.0f64)));
  b.push(Instruction::F64Ge);
  b.push(Instruction::I32And);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(9007199254740992.0f64)));
  b.push(Instruction::F64Le);
  b.push(Instruction::I32And);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I64ReinterpretF64);
  b.push(Instruction::I64Const(i64::MIN));
  b.push(Instruction::I64Ne);
  b.push(Instruction::I32And);
  b.push(Instruction::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::F64)));
  // --- integer branch ---

  // neg = value < 0.0 ? 1 : 0
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::F64Lt);
  b.push(Instruction::LocalSet(2)); // neg

  // raw_i64 = i64.trunc_f64_s(value)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I64TruncF64S);
  b.push(Instruction::LocalSet(1)); // raw_i64

  // abs_i64 = neg ? -raw_i64 : raw_i64
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::I64Const(0));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I64Sub);
  b.push(Instruction::LocalSet(3));
  b.push(Instruction::Else);
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::LocalSet(3));
  b.push(Instruction::End);

  // Count digits: ndigits = 0; tmp = abs_i64; do { ndigits++; tmp /= 10; } while tmp != 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(4)); // ndigits = 0
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalSet(5)); // tmp = abs_i64

  b.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  // ndigits++
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  // tmp /= 10
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Const(10));
  b.push(Instruction::I64DivU);
  b.push(Instruction::LocalSet(5));
  // if tmp == 0: break
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // total_len = ndigits + (neg ? 1 : 0)
  // payload = 8 (byte_len f64) + total_len rounded up to 8 bytes
  // Compute total_len
  b.push(Instruction::LocalGet(4)); // ndigits
  b.push(Instruction::LocalGet(2)); // neg flag (0 or 1)
  b.push(Instruction::I32Add); // total_len

  // payload = total_len (round up to 8): (total_len + 7) & ~7
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add); // payload = 8 + padded_total_len
  b.push(Instruction::LocalSet(6)); // payload

  // Allocate string: raw_base = HEAP_PTR_GLOBAL
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(11)); // raw_base
  // Write HEAP_MAGIC at raw_base
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // Write string_tag at raw_base+4
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(string_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // str_ptr = raw_base + 8
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(7)); // str_ptr (logical)
  // content = str_ptr + 8 (after byte_len slot)
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8)); // content base
  // Advance heap_ptr by payload
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // Write byte_len = total_len (ndigits + neg) at str_ptr+0
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::LocalGet(4)); // ndigits
  b.push(Instruction::LocalGet(2)); // neg
  b.push(Instruction::I32Add); // total_len
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));

  // Write '-' at content[0] if neg
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(8)); // content addr
  b.push(Instruction::I32Const(b'-' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::End);

  // Write digits right-to-left, pos starts at (neg + ndigits - 1)
  // pos = neg + ndigits - 1
  b.push(Instruction::LocalGet(2)); // neg
  b.push(Instruction::LocalGet(4)); // ndigits
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(9)); // pos

  // tmp = abs_i64 again
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalSet(5));

  // Loop: write digits from least significant to most significant (right to left)
  b.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  // digit = tmp % 10
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Const(10));
  b.push(Instruction::I64RemU);
  b.push(Instruction::I32WrapI64);
  b.push(Instruction::LocalSet(10)); // digit
  // content[pos] = '0' + digit
  b.push(Instruction::LocalGet(8)); // content base
  b.push(Instruction::LocalGet(9)); // pos
  b.push(Instruction::I32Add); // content + pos
  b.push(Instruction::LocalGet(10)); // digit
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  // pos--
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(9));
  // tmp /= 10
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Const(10));
  b.push(Instruction::I64DivU);
  b.push(Instruction::LocalSet(5));
  // if tmp == 0: break
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // return str_ptr as f64 — leave on stack
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::Else);
  // The helper writes into reserved scratch memory, outside the heap. The
  // longest expanded f64 uses 327 bytes; 400 is the helper's ABI bound.
  b.extend([
    Instruction::LocalGet(0),
    Instruction::I32Const(number_format::NUMBER_SCRATCH),
    Instruction::Call(ryu_format_idx),
    Instruction::LocalTee(12),
    Instruction::I32Const(400),
    Instruction::I32GtU,
    Instruction::If(BlockType::Empty),
    Instruction::Unreachable,
    Instruction::End,
    // payload = 8-byte length slot + padded UTF-8 bytes.
    Instruction::LocalGet(12),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::I32Const(-8),
    Instruction::I32And,
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(13),
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::LocalTee(11),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(11),
    Instruction::I32Const(4),
    Instruction::I32Add,
    Instruction::I32Const(string_tag),
    Instruction::I32Store(mem_arg_i32(0)),
    Instruction::LocalGet(11),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalTee(7),
    Instruction::LocalGet(12),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(7),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::I32Const(number_format::NUMBER_SCRATCH),
    Instruction::LocalGet(12),
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    Instruction::LocalGet(11),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(13),
    Instruction::I32Add,
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    Instruction::LocalGet(7),
    Instruction::F64ConvertI32U,
  ]);

  b.push(Instruction::End);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I64, // raw_i64 (1)
      ValType::I32, // neg (2)
      ValType::I64, // abs_i64 (3)
      ValType::I32, // ndigits (4)
      ValType::I64, // tmp_i64 (5)
      ValType::I32, // payload (6)
      ValType::I32, // str_ptr (7)
      ValType::I32, // content (8)
      ValType::I32, // pos (9)
      ValType::I32, // digit (10)
      ValType::I32, // raw_base (11)
      ValType::I32, // canonical_len (12)
      ValType::I32, // ryu_payload (13)
    ],
    instructions: b,
  }
}

/// `__rt_display_by(value: f64, radix: f64) -> f64`
///
/// Converts `value` (a non-negative safe integer) to a string in radix 2, 8 or 16,
/// prefixed with "0b", "0o" or "0x". Other values or radixes trap.
#[allow(clippy::vec_init_then_push)]
fn build_rt_display_by(string_tag: i32) -> CompiledFn {
  // params: 0=value(f64), 1=radix(f64)
  // locals (allocated manually to match flat vec):
  //   2=radix_i64(i64), 3=is_neg(i32), 4=abs_i64(i64), 5=tmp_i64(i64),
  //   6=ndigits(i32), 7=prefix_len(i32), 8=total_len(i32), 9=padded(i32),
  //   10=str_ptr(i32), 11=content(i32), 12=digit_pos(i32), 13=digit(i32),
  //   14=raw_base(i32), 15=ch(i32), 16=payload(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // Domain (docs/data/number.md): value is a non-negative safe integer and radix is 2, 8 or 16;
  // anything else traps where native and JS raise an error.
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Trunc);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Ne); // fractional or NaN
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::F64Lt);
  b.push(Instruction::I32Or);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(9_007_199_254_740_991.0f64)));
  b.push(Instruction::F64Gt);
  b.push(Instruction::I32Or);
  for radix in [2.0f64, 8.0, 16.0] {
    b.push(Instruction::LocalGet(1));
    b.push(Instruction::F64Const(Ieee64::from(radix)));
    b.push(Instruction::F64Ne);
  }
  b.push(Instruction::I32And);
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Unreachable);
  b.push(Instruction::End);

  // radix_i64 = i64.trunc_f64_s(radix)
  b.push(Instruction::LocalGet(1)); // radix f64
  b.push(Instruction::I64TruncF64S);
  b.push(Instruction::LocalSet(2)); // radix_i64

  // is_neg = value < 0.0 ? 1 : 0
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::F64Lt);
  b.push(Instruction::LocalSet(3)); // is_neg

  // abs_i64 = trunc(value) in i64, then abs
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I64TruncF64S);
  b.push(Instruction::LocalSet(5)); // tmp = signed i64(value)
  // abs: if is_neg, negate
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::I64Const(0));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Sub);
  b.push(Instruction::LocalSet(4)); // abs_i64
  b.push(Instruction::Else);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);

  // Count ndigits: loop { ndigits++; tmp /= radix; } while tmp != 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(6)); // ndigits = 0
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalSet(5)); // tmp = abs_i64

  b.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(6)); // ndigits++
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(2)); // radix_i64
  b.push(Instruction::I64DivU);
  b.push(Instruction::LocalSet(5)); // tmp /= radix
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Eqz);
  b.push(Instruction::BrIf(1)); // break if tmp == 0
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // prefix_len: 2 if radix ∈ {2,8,16}, else 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(7)); // prefix_len = 0
  // if radix == 2 || radix == 8 || radix == 16
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(2.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(8.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(16.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::LocalSet(7)); // prefix_len = 2
  b.push(Instruction::End);

  // total_len = is_neg + prefix_len + ndigits
  b.push(Instruction::LocalGet(3)); // is_neg
  b.push(Instruction::LocalGet(7)); // prefix_len
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(6)); // ndigits
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8)); // total_len

  // padded = (total_len + 7) & ~7
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(9)); // padded

  // payload = 8 (byte_len f64) + padded
  b.push(Instruction::I32Const(8));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(16)); // payload

  // Allocate string: raw_base = HEAP_PTR_GLOBAL
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(14)); // raw_base
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(string_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // str_ptr = raw_base + 8
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(10)); // str_ptr (logical)
  // content = str_ptr + 8
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(11)); // content
  // Advance heap_ptr: raw_base + 8 + payload
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // Write byte_len at str_ptr+0
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::LocalGet(8)); // total_len
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));

  // Write '-' if is_neg (content[0] = '-')
  b.push(Instruction::LocalGet(3)); // is_neg
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(b'-' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::End);

  // Write prefix bytes: current byte offset = is_neg (0 or 1)
  // pos = is_neg  (i32 local 12 reused as write-cursor for prefix/digit region)
  b.push(Instruction::LocalGet(3)); // is_neg as starting byte offset
  b.push(Instruction::LocalSet(12)); // digit_pos = is_neg (cursor)

  // if radix == 2: content[cursor]='0', content[cursor+1]='b'; cursor+=2
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(2.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'b' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::End);

  // if radix == 8: prefix "0o"
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(8.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'o' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::End);

  // if radix == 16: prefix "0x"
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(16.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'x' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::End);

  // Now write digits right-to-left starting at content[cursor + ndigits - 1]
  // digit_pos = cursor + ndigits - 1
  b.push(Instruction::LocalGet(12)); // cursor (after prefix)
  b.push(Instruction::LocalGet(6)); // ndigits
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(12)); // digit_pos (rightmost digit byte offset)

  // tmp = abs_i64
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalSet(5));

  // Loop: write digits right to left
  b.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  // digit = i32(tmp % radix_i64)
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(2)); // radix_i64
  b.push(Instruction::I64RemU);
  b.push(Instruction::I32WrapI64);
  b.push(Instruction::LocalSet(13)); // digit
  // ch = digit < 10 ? '0' + digit : 'a' + digit - 10
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::I32Const(10));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(15)); // ch = '0' + digit
  b.push(Instruction::Else);
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::I32Const(b'a' as i32 - 10));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(15)); // ch = 'a' + digit - 10
  b.push(Instruction::End);
  // content[digit_pos] = ch
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  // digit_pos--
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(12));
  // tmp /= radix
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I64DivU);
  b.push(Instruction::LocalSet(5));
  // break if tmp == 0
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // return str_ptr as f64
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I64, // radix_i64 (2)
      ValType::I32, // is_neg (3)
      ValType::I64, // abs_i64 (4)
      ValType::I64, // tmp_i64 (5)
      ValType::I32, // ndigits (6)
      ValType::I32, // prefix_len (7)
      ValType::I32, // total_len (8)
      ValType::I32, // padded (9)
      ValType::I32, // str_ptr (10)
      ValType::I32, // content (11)
      ValType::I32, // digit_pos / cursor (12)
      ValType::I32, // digit (13)
      ValType::I32, // raw_base (14)
      ValType::I32, // ch (15)
      ValType::I32, // payload (16)
    ],
    instructions: b,
  }
}

// ---------------------------------------------------------------------------
// Helper: emit "is ASCII whitespace" check for local `loc` → leaves i32 on stack.
// Whitespace: 0x09 (tab), 0x0A (LF), 0x0D (CR), 0x20 (space).
fn emit_is_ws_check(b: &mut Vec<Instruction<'static>>, loc: u32) {
  b.push(Instruction::LocalGet(loc));
  b.push(Instruction::I32Const(0x09));
  b.push(Instruction::I32Eq);
  b.push(Instruction::LocalGet(loc));
  b.push(Instruction::I32Const(0x0A));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::LocalGet(loc));
  b.push(Instruction::I32Const(0x0D));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::LocalGet(loc));
  b.push(Instruction::I32Const(0x20));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
}

/// `__rt_trim_ws(s: f64) → f64`
///
/// Strips leading and trailing ASCII whitespace from a heap string.
/// Returns a new heap-allocated string.
#[allow(clippy::vec_init_then_push)]
fn build_rt_trim_ws(str_tag: i32) -> CompiledFn {
  // params: 0=s(f64)
  // locals: 1=ptr(i32), 2=byte_len(i32), 3=content(i32), 4=start(i32),
  //         5=end(i32), 6=new_len(i32), 7=raw_base(i32), 8=new_ptr(i32),
  //         9=padded(i32), 10=b(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // ptr = i32(s)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(1));

  // byte_len = i32(f64load(ptr+0))
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(2));

  // content = ptr + 8
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(3));

  // start = 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(4));

  // Forward loop: while start < byte_len && is_ws(content[start]): start++
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(10));
  emit_is_ws_check(&mut b, 10);
  b.push(Instruction::I32Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // end = byte_len
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::LocalSet(5));

  // Backward loop: while end > start && is_ws(content[end-1]): end--
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32LeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-1i32));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(10));
  emit_is_ws_check(&mut b, 10);
  b.push(Instruction::I32Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(5));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // new_len = end - start
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(6));

  // padded = (new_len + 7) & -8
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(9));

  // Allocate heap string
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(7)); // raw_base
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(str_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // new_ptr = raw_base + 8
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8));
  // byte_len at new_ptr+0
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));
  // advance heap: raw_base + 16 + padded
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  // memory.copy(new_ptr+8, content+start, new_len)
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  // return f64(new_ptr)
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // ptr (1)
      ValType::I32, // byte_len (2)
      ValType::I32, // content (3)
      ValType::I32, // start (4)
      ValType::I32, // end (5)
      ValType::I32, // new_len (6)
      ValType::I32, // raw_base (7)
      ValType::I32, // new_ptr (8)
      ValType::I32, // padded (9)
      ValType::I32, // b (10)
    ],
    instructions: b,
  }
}

/// `__rt_trim_char(s: f64, c: f64) → f64`
///
/// Strips the first byte of the `c` string from both ends of `s`.
#[allow(clippy::vec_init_then_push)]
fn build_rt_trim_char(str_tag: i32) -> CompiledFn {
  // params: 0=s(f64), 1=c(f64) (char string ptr)
  // locals: 2=ptr(i32), 3=byte_len(i32), 4=content(i32), 5=start(i32),
  //         6=end(i32), 7=new_len(i32), 8=raw_base(i32), 9=new_ptr(i32),
  //         10=padded(i32), 11=ch(i32), 12=b(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // ch = first byte of char string c
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(11));

  // ptr = i32(s)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(2));

  // byte_len = i32(f64load(ptr))
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(3));

  // content = ptr + 8
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));

  // start = 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(5));

  // Forward loop: while start < byte_len && content[start] == ch
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Ne);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(5));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // end = byte_len
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalSet(6));

  // Backward loop: while end > start && content[end-1] == ch
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32LeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-1i32));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Ne);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(6));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // new_len = end - start
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(7));

  // padded = (new_len + 7) & -8
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(10));

  // Allocate heap string
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(8));
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(str_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(9));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // ptr (2)
      ValType::I32, // byte_len (3)
      ValType::I32, // content (4)
      ValType::I32, // start (5)
      ValType::I32, // end (6)
      ValType::I32, // new_len (7)
      ValType::I32, // raw_base (8)
      ValType::I32, // new_ptr (9)
      ValType::I32, // padded (10)
      ValType::I32, // ch (11)
      ValType::I32, // b (12)
    ],
    instructions: b,
  }
}

/// `__rt_blank(s: f64) → f64`
///
/// Returns 1.0 if the string contains only ASCII whitespace (or is empty), 0.0 otherwise.
#[allow(clippy::vec_init_then_push)]
fn build_rt_blank() -> CompiledFn {
  // params: 0=s(f64)
  // locals: 1=ptr(i32), 2=byte_len(i32), 3=content(i32), 4=i(i32), 5=b(i32)
  let mut b: Vec<Instruction> = Vec::new();

  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(1));

  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(2));

  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(3));

  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(4));

  // Loop: if any byte is non-whitespace, return 0.0
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  // if i >= byte_len: break (all whitespace)
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  // b = load8(content + i)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(5));
  // if NOT is_ws: return 0.0
  emit_is_ws_check(&mut b, 5);
  b.push(Instruction::I32Eqz);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::Return);
  b.push(Instruction::End);
  // i++
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  b.push(Instruction::F64Const(Ieee64::from(1.0f64)));

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // ptr (1)
      ValType::I32, // byte_len (2)
      ValType::I32, // content (3)
      ValType::I32, // i (4)
      ValType::I32, // b (5)
    ],
    instructions: b,
  }
}

#[derive(Clone, Copy)]
struct ParseFloatLocals {
  hi: u32,
  lo: u32,
  tz_count: u32,
  sig_digits: u32,
  sticky: u32,
  exp10: u32,
  exp2: u32,
  digit: u32,
  t_lo: u32,
  t_hi: u32,
  t2_lo: u32,
  t2_hi: u32,
  rem: u32,
  shift: u32,
}

fn rt_emit_u128_mul10_add(b: &mut RuntimeFnBuilder, locs: &ParseFloatLocals, add_digit: Option<u32>) {
  // Compute `(hi, lo) * 8` and `(hi, lo) * 2` without overflow (`sig_digits < 38`).
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(3));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalSet(locs.t_lo));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(3));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(61));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.t_hi));

  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalSet(locs.t2_lo));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(63));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.t2_hi));

  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::LocalGet(locs.t2_lo));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(locs.lo));

  // Add optional digit `add_digit` and accumulate both 64-bit carries into `hi`.
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::LocalGet(locs.t2_hi));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::I64LtU);
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Add);
  if let Some(digit) = add_digit {
    b.emit(Instruction::LocalSet(locs.hi));
    b.emit(Instruction::LocalGet(locs.lo));
    b.emit(Instruction::LocalSet(locs.t_lo));
    b.emit(Instruction::LocalGet(locs.t_lo));
    b.emit(Instruction::LocalGet(digit));
    b.emit(Instruction::I64ExtendI32U);
    b.emit(Instruction::I64Add);
    b.emit(Instruction::LocalSet(locs.lo));
    b.emit(Instruction::LocalGet(locs.hi));
    b.emit(Instruction::LocalGet(locs.lo));
    b.emit(Instruction::LocalGet(locs.t_lo));
    b.emit(Instruction::I64LtU);
    b.emit(Instruction::I64ExtendI32U);
    b.emit(Instruction::I64Add);
  }
  b.emit(Instruction::LocalSet(locs.hi));
}

fn rt_emit_u128_decimal_digit(b: &mut RuntimeFnBuilder, locs: &ParseFloatLocals, in_frac: bool) {
  b.emit(Instruction::LocalGet(locs.sig_digits));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(locs.digit));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  if in_frac {
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(-10_000_000));
    b.emit(Instruction::I32GtS);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::LocalSet(locs.exp10));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(locs.digit));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.tz_count));
  b.emit(Instruction::I32Const(10_000_000));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.tz_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.tz_count));
  b.emit(Instruction::End);
  if !in_frac {
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(10_000_000));
    b.emit(Instruction::I32LtS);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(locs.exp10));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::Else);
  // Flush pending zeros into the 128-bit significand before appending this non-zero digit.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.tz_count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LeS);
  b.emit(Instruction::LocalGet(locs.sig_digits));
  b.emit(Instruction::I32Const(38));
  b.emit(Instruction::I32GeS);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  rt_emit_u128_mul10_add(b, locs, None);
  b.emit(Instruction::LocalGet(locs.sig_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.sig_digits));
  b.emit(Instruction::LocalGet(locs.tz_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.tz_count));
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Const(-10_000_000));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.exp10));
  b.emit(Instruction::End);
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.tz_count));

  b.emit(Instruction::LocalGet(locs.sig_digits));
  b.emit(Instruction::I32Const(38));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  rt_emit_u128_mul10_add(b, locs, Some(locs.digit));
  b.emit(Instruction::LocalGet(locs.sig_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.sig_digits));
  if in_frac {
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(-10_000_000));
    b.emit(Instruction::I32GtS);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::LocalSet(locs.exp10));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(locs.sticky));
  if !in_frac {
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(10_000_000));
    b.emit(Instruction::I32LtS);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(locs.exp10));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(locs.exp10));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
}

fn rt_emit_u128_normalize(b: &mut RuntimeFnBuilder, locs: &ParseFloatLocals) {
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::I32Const(64));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.exp2));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Clz);
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::LocalTee(locs.shift));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.shift));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::LocalSet(locs.t_hi)); // shift as u64 (1..=63)
  b.emit(Instruction::I64Const(64));
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::I64Sub);
  b.emit(Instruction::LocalSet(locs.t_lo)); // 64 - shift (1..=63)
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::LocalGet(locs.shift));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.exp2));
  b.emit(Instruction::End);
}

fn rt_emit_u128_mul_by_5_step(b: &mut RuntimeFnBuilder, locs: &ParseFloatLocals) {
  // Ensure `(hi, lo) * 5 < 2^128` (`u128::MAX / 5 = 0x3333...`).
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(0x3333_3333_3333_3333u64 as i64));
  b.emit(Instruction::I64LtU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(locs.sticky));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(63));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.exp2));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Multiply `(hi, lo)` by 5 (`((hi, lo) << 2) + (hi, lo)`).
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(2));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalSet(locs.t_lo));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(2));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(62));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.t_hi));

  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(locs.t2_lo));
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalGet(locs.t2_lo));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64LtU);
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t2_lo));
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.exp2));
}

fn rt_emit_u128_div_by_5_step(b: &mut RuntimeFnBuilder, locs: &ParseFloatLocals) {
  // Normalize `(hi, lo)` first so division retains full 128-bit precision.
  rt_emit_u128_normalize(b, locs);

  // Divide 128-bit `(hi, lo)` by 5 via 64/32-bit chunks: `(q_hi, q_lo)` and `rem` (0..=4).
  // Step 1: `hi / 5`.
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64RemU);
  b.emit(Instruction::LocalSet(locs.t2_hi)); // r0 in 0..=4
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64DivU);
  b.emit(Instruction::LocalSet(locs.hi)); // q_hi

  // Step 2: `((r0 << 32) | (lo >> 32)) / 5`.
  b.emit(Instruction::LocalGet(locs.t2_hi));
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalTee(locs.t_hi)); // num1
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64RemU);
  b.emit(Instruction::LocalSet(locs.t2_lo)); // r1 in 0..=4
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64DivU);
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalSet(locs.t_hi)); // q_mid << 32

  // Step 3: `((r1 << 32) | (lo & 0xFFFF_FFFF)) / 5`.
  b.emit(Instruction::LocalGet(locs.t2_lo));
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(0xFFFF_FFFFi64));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalTee(locs.t_lo)); // num2
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64RemU);
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::LocalSet(locs.rem));
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64DivU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.lo));

  // Factor of 1/2 from dividing by 10 = 5 * 2.
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.exp2));

  // Shift exact fractional bits from `rem / 5` while restoring bit 63 of `hi`.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64LtS);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(locs.rem));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::LocalSet(locs.rem));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.digit));
  b.emit(Instruction::LocalGet(locs.rem));
  b.emit(Instruction::I32Const(5));
  b.emit(Instruction::I32GeS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.rem));
  b.emit(Instruction::I32Const(5));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.rem));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(locs.digit));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(63));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(locs.digit));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.exp2));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(locs.rem));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(locs.sticky));
  b.emit(Instruction::End);
}

fn rt_emit_u128_round_to_f64(b: &mut RuntimeFnBuilder, locs: &ParseFloatLocals, result: u32) {
  let mant = locs.t_hi;
  let extra_nonzero = locs.digit;
  let round_up = locs.rem;
  let exp2_cur = b.alloc_i32();

  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::LocalSet(exp2_cur));

  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(1023));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(f64::INFINITY)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(-1022));
  b.emit(Instruction::I32GeS);
  b.emit(Instruction::If(BlockType::Empty));
  // Normal IEEE 754 binary64: round-to-nearest-even on top 53 bits of normalized `hi`.
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(11));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalSet(mant));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::LocalGet(locs.sticky));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalSet(extra_nonzero));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(0x7FF));
  b.emit(Instruction::I64And);
  b.emit(Instruction::LocalTee(locs.t_lo));
  b.emit(Instruction::I64Const(0x400));
  b.emit(Instruction::I64GtU);
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::I64Const(0x400));
  b.emit(Instruction::I64Eq);
  b.emit(Instruction::LocalGet(extra_nonzero));
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(mant));
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(53));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalSet(mant));
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(exp2_cur));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(1023));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(f64::INFINITY)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(1023));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Const(52));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(0x000F_FFFF_FFFF_FFFFi64));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Or);
  b.emit(Instruction::F64ReinterpretI64);
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(-1074));
  b.emit(Instruction::I32GeS);
  b.emit(Instruction::If(BlockType::Empty));
  // Subnormal IEEE 754 binary64: `total_shift` is strictly in 12..=63.
  b.emit(Instruction::I32Const(-1011));
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::LocalTee(locs.t2_hi)); // total_shift in 12..=63
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Sub);
  b.emit(Instruction::LocalSet(locs.t2_lo)); // half_pos in 11..=62

  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t2_hi));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalSet(mant));

  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::LocalGet(locs.t2_lo));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Sub);
  b.emit(Instruction::LocalSet(locs.t_lo)); // mask for bits below half

  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(locs.sticky));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalSet(extra_nonzero));

  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t2_lo));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::LocalGet(extra_nonzero));
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(round_up));
  b.emit(Instruction::LocalGet(round_up));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(mant));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(mant));
  b.emit(Instruction::F64ReinterpretI64);
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(exp2_cur));
  b.emit(Instruction::I32Const(-1075));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  // Halfway to smallest subnormal `2^-1074`: round to even (0) only on exact tie.
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Const(0x7FFF_FFFF_FFFF_FFFFi64));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(locs.sticky));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::F64ReinterpretI64);
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
}

/// `__rt_bigint_mul_add(base_ptr: i32, len: i32, mul: i64, add: i64) -> i32`
///
/// Multiplies a little-endian `u32` limb slice in place by `mul` and adds `add`,
/// returning the updated limb length. Capacity is 128 limbs (4096 bits).
pub(super) fn build_rt_bigint_mul_add() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(4);
  let base_ptr = 0;
  let len = 1;
  let mul = 2;
  let add = 3;
  let carry = b.alloc_i64();
  let idx = b.alloc_i32();
  let addr = b.alloc_i32();
  let prod = b.alloc_i64();

  b.emit(Instruction::LocalGet(add));
  b.emit(Instruction::LocalSet(carry));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(base_ptr));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(addr));
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::LocalGet(mul));
  b.emit(Instruction::I64Mul);
  b.emit(Instruction::LocalGet(carry));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalTee(prod));
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(prod));
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalSet(carry));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(carry));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32Const(128));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(base_ptr));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(carry));
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(len));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(len));
  b.finish(vec![ValType::I32, ValType::I32, ValType::I64, ValType::I64], vec![ValType::I32])
}

fn rt_emit_scale_bigint_pow5(b: &mut RuntimeFnBuilder, bigint_mul_add_idx: u32, buf_ptr: u32, buf_len: u32, count: u32) {
  let rem = b.alloc_i32();
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::LocalSet(rem));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(13));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(buf_ptr));
  b.emit(Instruction::LocalGet(buf_len));
  b.emit(Instruction::I64Const(1_220_703_125i64));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(buf_len));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(13));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(rem));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LeS);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(buf_ptr));
  b.emit(Instruction::LocalGet(buf_len));
  b.emit(Instruction::I64Const(5));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(buf_len));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(rem));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
}

fn rt_emit_scale_bigint_pow2(b: &mut RuntimeFnBuilder, bigint_mul_add_idx: u32, buf_ptr: u32, buf_len: u32, count: u32) {
  let rem = b.alloc_i32();
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::LocalSet(rem));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(buf_ptr));
  b.emit(Instruction::LocalGet(buf_len));
  b.emit(Instruction::I64Const(1i64 << 24));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(buf_len));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(rem));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(buf_ptr));
  b.emit(Instruction::LocalGet(buf_len));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::LocalGet(rem));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(buf_len));
  b.emit(Instruction::End);
}

/// `__rt_parse_float_exact_midpoint(content: i32, byte_len: i32, start_i: i32, explicit_exp: i32, bits_low: i64) -> f64`
///
/// Compares the full decimal mantissa (up to 768 significant digits plus sticky)
/// against the exact binary64 midpoint between `bits_low` and `bits_low + 1`.
pub(super) fn build_rt_parse_float_exact_midpoint(bigint_mul_add_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(5);
  let content = 0;
  let byte_len = 1;
  let start_i = 2;
  let explicit_exp = 3;
  let bits_low = 4;

  let scratch_size = b.alloc_i64();
  let managed_zero = b.alloc_i64();
  let lhs_ptr = b.alloc_i32();
  let rhs_ptr = b.alloc_i32();
  let lhs_len = b.alloc_i32();
  let rhs_len = b.alloc_i32();
  let i = b.alloc_i32();
  let ch = b.alloc_i32();
  let in_frac = b.alloc_i32();
  let sig_digits = b.alloc_i32();
  let tz_count = b.alloc_i32();
  let e_d = b.alloc_i32();
  let extra_nonzero = b.alloc_i32();
  let biased_exp = b.alloc_i32();
  let p2 = b.alloc_i32();
  let frac = b.alloc_i64();
  let m2 = b.alloc_i64();
  let m2_hi = b.alloc_i32();
  let scale_steps = b.alloc_i32();
  let shift2 = b.alloc_i32();
  let cmp = b.alloc_i32();
  let idx = b.alloc_i32();
  let off = b.alloc_i32();
  let lv = b.alloc_i32();
  let rv = b.alloc_i32();

  b.emit(Instruction::I64Const(1024));
  b.emit(Instruction::LocalSet(scratch_size));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::LocalSet(managed_zero));
  rt_emit_reserve_scratch(&mut b, scratch_size, managed_zero, lhs_ptr);
  b.emit(Instruction::LocalGet(lhs_ptr));
  b.emit(Instruction::I32Const(512));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(rhs_ptr));

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(lhs_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(rhs_len));
  b.emit(Instruction::LocalGet(start_i));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(in_frac));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(sig_digits));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(tz_count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(extra_nonzero));

  // Scan up to 768 significant digits of the decimal mantissa into `lhs`.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalTee(ch));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(in_frac));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::BrIf(2));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(in_frac));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(2));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(ch)); // digit 0..=9
  b.emit(Instruction::LocalGet(sig_digits));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(in_frac));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(-10_000_000));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(tz_count));
  b.emit(Instruction::I32Const(10_000_000));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(tz_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(tz_count));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(in_frac));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(10_000_000));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  // Flush pending zeros while within 768 significant digits.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(tz_count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LeS);
  b.emit(Instruction::LocalGet(sig_digits));
  b.emit(Instruction::I32Const(768));
  b.emit(Instruction::I32GeS);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(lhs_ptr));
  b.emit(Instruction::LocalGet(lhs_len));
  b.emit(Instruction::I64Const(10));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(lhs_len));
  b.emit(Instruction::LocalGet(sig_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(sig_digits));
  b.emit(Instruction::LocalGet(tz_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(tz_count));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(-10_000_000));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::End);
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(tz_count));

  b.emit(Instruction::LocalGet(sig_digits));
  b.emit(Instruction::I32Const(768));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(lhs_ptr));
  b.emit(Instruction::LocalGet(lhs_len));
  b.emit(Instruction::I64Const(10));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(lhs_len));
  b.emit(Instruction::LocalGet(sig_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(sig_digits));
  b.emit(Instruction::LocalGet(in_frac));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(-10_000_000));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(extra_nonzero));
  b.emit(Instruction::LocalGet(in_frac));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(10_000_000));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(extra_nonzero));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(lhs_ptr));
  b.emit(Instruction::LocalGet(lhs_len));
  b.emit(Instruction::I64Const(10));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::Call(bigint_mul_add_idx));
  b.emit(Instruction::LocalSet(lhs_len));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(e_d));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::LocalGet(explicit_exp));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(e_d));

  // Build exact binary64 midpoint `m2 * 2^p2` for `(bits_low, bits_low + 1)` into `rhs`.
  b.emit(Instruction::LocalGet(bits_low));
  b.emit(Instruction::I64Const(52));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I64Const(0x7FF));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::LocalSet(biased_exp));

  b.emit(Instruction::LocalGet(bits_low));
  b.emit(Instruction::I64Const(0x000F_FFFF_FFFF_FFFFi64));
  b.emit(Instruction::I64And);
  b.emit(Instruction::LocalSet(frac));

  b.emit(Instruction::LocalGet(biased_exp));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(frac));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(m2));
  b.emit(Instruction::I32Const(-1075));
  b.emit(Instruction::LocalSet(p2));
  b.emit(Instruction::Else);
  b.emit(Instruction::I64Const(1i64 << 52));
  b.emit(Instruction::LocalGet(frac));
  b.emit(Instruction::I64Or);
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Shl);
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Or);
  b.emit(Instruction::LocalSet(m2));
  b.emit(Instruction::LocalGet(biased_exp));
  b.emit(Instruction::I32Const(1076));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(p2));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(rhs_ptr));
  b.emit(Instruction::LocalGet(m2));
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(m2));
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::LocalTee(m2_hi));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(rhs_ptr));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(m2_hi));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::LocalSet(rhs_len));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(rhs_len));
  b.emit(Instruction::End);

  // Align powers of 5: multiply `lhs` by 5^e_d when `e_d > 0`, or `rhs` by 5^(-e_d) when `e_d < 0`.
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::LocalSet(scale_steps));
  rt_emit_scale_bigint_pow5(&mut b, bigint_mul_add_idx, lhs_ptr, lhs_len, scale_steps);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(scale_steps));
  rt_emit_scale_bigint_pow5(&mut b, bigint_mul_add_idx, rhs_ptr, rhs_len, scale_steps);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Align powers of 2: `shift2 = e_d - p2`.
  b.emit(Instruction::LocalGet(e_d));
  b.emit(Instruction::LocalGet(p2));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalTee(shift2));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(shift2));
  b.emit(Instruction::LocalSet(scale_steps));
  rt_emit_scale_bigint_pow2(&mut b, bigint_mul_add_idx, lhs_ptr, lhs_len, scale_steps);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(shift2));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(shift2));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(scale_steps));
  rt_emit_scale_bigint_pow2(&mut b, bigint_mul_add_idx, rhs_ptr, rhs_len, scale_steps);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Compare unsigned big integers `lhs` and `rhs`.
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(cmp));
  b.emit(Instruction::LocalGet(lhs_len));
  b.emit(Instruction::LocalGet(rhs_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(cmp));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(lhs_len));
  b.emit(Instruction::LocalGet(rhs_len));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(cmp));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(lhs_len));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::LocalSet(off));
  b.emit(Instruction::LocalGet(lhs_ptr));
  b.emit(Instruction::LocalGet(off));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(lv));
  b.emit(Instruction::LocalGet(rhs_ptr));
  b.emit(Instruction::LocalGet(off));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(rv));
  b.emit(Instruction::LocalGet(lv));
  b.emit(Instruction::LocalGet(rv));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalGet(lv));
  b.emit(Instruction::LocalGet(rv));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::Select);
  b.emit(Instruction::LocalSet(cmp));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::End);
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Round up iff `lhs > mid` or exact tie with odd `bits_low`.
  b.emit(Instruction::LocalGet(cmp));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::LocalGet(cmp));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(bits_low));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64And);
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Result(ValType::F64)));
  b.emit(Instruction::LocalGet(bits_low));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::F64ReinterpretI64);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(bits_low));
  b.emit(Instruction::F64ReinterpretI64);
  b.emit(Instruction::End);
  b.finish(
    vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I64],
    vec![ValType::F64],
  )
}

/// `__rt_parse_float(s: f64) → f64`
///
/// Parses decimal strings to correctly rounded f64 values using 128-bit integer
/// decimal-to-binary scaling with an exact big-integer midpoint fallback.
pub(super) fn build_rt_parse_float(parse_float_exact_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1); // 0=s(f64)
  let ptr = b.alloc_i32();
  let byte_len = b.alloc_i32();
  let content = b.alloc_i32();
  let i = b.alloc_i32();
  let num_start_i = b.alloc_i32();
  let is_neg = b.alloc_i32();
  let ch = b.alloc_i32();
  let result = b.alloc_f64();
  let result_hi = b.alloc_f64();
  let exp_val = b.alloc_i32();
  let exp_neg = b.alloc_i32();
  let explicit_exp = b.alloc_i32();
  let is_special = b.alloc_i32();
  let scale_steps = b.alloc_i32();
  let locs = ParseFloatLocals {
    hi: b.alloc_i64(),
    lo: b.alloc_i64(),
    tz_count: b.alloc_i32(),
    sig_digits: b.alloc_i32(),
    sticky: b.alloc_i32(),
    exp10: b.alloc_i32(),
    exp2: b.alloc_i32(),
    digit: b.alloc_i32(),
    t_lo: b.alloc_i64(),
    t_hi: b.alloc_i64(),
    t2_lo: b.alloc_i64(),
    t2_hi: b.alloc_i64(),
    rem: b.alloc_i32(),
    shift: b.alloc_i32(),
  };

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(ptr));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(byte_len));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(content));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));

  // Skip leading whitespace.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(ch));
  emit_is_ws_check(&mut b.instructions, ch);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Optional sign.
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(is_neg));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(ch));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'-' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(is_neg));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'+' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalSet(num_start_i));
  b.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.tz_count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.sig_digits));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.sticky));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.exp10));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(explicit_exp));
  b.emit(Instruction::I32Const(127));
  b.emit(Instruction::LocalSet(locs.exp2));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(is_special));

  // `inf` / `infinity` / `nan` spellings (validated by __rt_parse_float_valid).
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(0x20));
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalSet(ch));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'i' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(f64::INFINITY)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(is_special));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'n' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(f64::NAN)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(is_special));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Integer digits accumulated in unsigned 128-bit significand (up to 38 significant digits).
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(ch));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.digit));
  rt_emit_u128_decimal_digit(&mut b, &locs, false);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Optional fractional part.
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(ch));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(locs.digit));
  rt_emit_u128_decimal_digit(&mut b, &locs, true);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Optional decimal exponent: e|E [+-]? digits.
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(0x20));
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Const(b'e' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(exp_neg));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(exp_val));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(ch));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'-' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(exp_neg));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'+' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(ch));
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(exp_val));
  b.emit(Instruction::I32Const(10_000_000));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(exp_val));
  b.emit(Instruction::I32Const(10));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalGet(ch));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(exp_val));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(exp_neg));
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(exp_val));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(exp_val));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalTee(explicit_exp));
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.exp10));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Convert accumulated 128-bit significand and scale by 10^exp10, then round once to f64.
  b.emit(Instruction::LocalGet(is_special));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64Eqz);
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Eqz);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Const(315));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(f64::INFINITY)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Const(-370));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::LocalSet(scale_steps));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(scale_steps));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));
  rt_emit_u128_mul_by_5_step(&mut b, &locs);
  b.emit(Instruction::LocalGet(scale_steps));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(scale_steps));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32LtS);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(locs.exp10));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(scale_steps));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(scale_steps));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));
  rt_emit_u128_div_by_5_step(&mut b, &locs);
  b.emit(Instruction::LocalGet(scale_steps));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(scale_steps));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  rt_emit_u128_normalize(&mut b, &locs);
  rt_emit_u128_round_to_f64(&mut b, &locs, result);
  b.emit(Instruction::LocalGet(locs.sticky));
  b.emit(Instruction::If(BlockType::Empty));
  // Bound accumulated truncation/scaling error: check whether `V_computed + 65536 ULP_lo`
  // could cross an f64 rounding midpoint, and fall back to exact big-integer comparison if so.
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64Const(65_536));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(locs.t_lo));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::LocalGet(locs.lo));
  b.emit(Instruction::I64LtU);
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(locs.t_hi));
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::LocalGet(locs.hi));
  b.emit(Instruction::I64LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I64Const(0x8000_0000_0000_0000u64 as i64));
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::I64Const(1));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::LocalGet(locs.exp2));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(locs.exp2));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(locs.t_hi));
  b.emit(Instruction::LocalSet(locs.hi));
  b.emit(Instruction::LocalGet(locs.t_lo));
  b.emit(Instruction::LocalSet(locs.lo));
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(locs.sticky));
  rt_emit_u128_round_to_f64(&mut b, &locs, result_hi);
  b.emit(Instruction::LocalGet(result));
  b.emit(Instruction::I64ReinterpretF64);
  b.emit(Instruction::LocalGet(result_hi));
  b.emit(Instruction::I64ReinterpretF64);
  b.emit(Instruction::I64Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(content));
  b.emit(Instruction::LocalGet(byte_len));
  b.emit(Instruction::LocalGet(num_start_i));
  b.emit(Instruction::LocalGet(explicit_exp));
  b.emit(Instruction::LocalGet(result));
  b.emit(Instruction::I64ReinterpretF64);
  b.emit(Instruction::Call(parse_float_exact_idx));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Apply sign.
  b.emit(Instruction::LocalGet(is_neg));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(result));
  b.emit(Instruction::F64Neg);
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(result));
  b.finish(vec![ValType::F64], vec![ValType::F64])
}

/// `__rt_parse_float_valid(s: f64) → f64`
///
/// Returns 1.0 when the whole text is a float literal accepted by native
/// `str::parse::<f64>`: optional sign, then `inf` / `infinity` / `nan` in any
/// case, or digits with an optional fraction (at least one digit overall) and
/// an optional exponent with at least one digit. No surrounding whitespace.
#[allow(clippy::vec_init_then_push)]
fn build_rt_parse_float_valid() -> CompiledFn {
  // params: 0=s(f64)
  // locals: 1=ptr, 2=len, 3=content, 4=i, 5=b, 6=digits, 7=ok, 8=edigits (all i32)
  let mut b: Vec<Instruction> = Vec::new();
  let byte_at_i = |b: &mut Vec<Instruction<'static>>| {
    b.push(Instruction::LocalGet(3));
    b.push(Instruction::LocalGet(4));
    b.push(Instruction::I32Add);
    b.push(Instruction::I32Load8U(mem_arg_byte(0)));
    b.push(Instruction::LocalSet(5));
  };
  let inc_i = |b: &mut Vec<Instruction<'static>>| {
    b.push(Instruction::LocalGet(4));
    b.push(Instruction::I32Const(1));
    b.push(Instruction::I32Add);
    b.push(Instruction::LocalSet(4));
  };
  // Consume ASCII digits, counting them in `counter`.
  let digit_loop = |b: &mut Vec<Instruction<'static>>, counter: u32| {
    b.push(Instruction::Block(BlockType::Empty));
    b.push(Instruction::Loop(BlockType::Empty));
    b.push(Instruction::LocalGet(4));
    b.push(Instruction::LocalGet(2));
    b.push(Instruction::I32GeU);
    b.push(Instruction::BrIf(1));
    b.push(Instruction::LocalGet(3));
    b.push(Instruction::LocalGet(4));
    b.push(Instruction::I32Add);
    b.push(Instruction::I32Load8U(mem_arg_byte(0)));
    b.push(Instruction::LocalSet(5));
    b.push(Instruction::LocalGet(5));
    b.push(Instruction::I32Const(b'0' as i32));
    b.push(Instruction::I32LtU);
    b.push(Instruction::BrIf(1));
    b.push(Instruction::LocalGet(5));
    b.push(Instruction::I32Const(b'9' as i32));
    b.push(Instruction::I32GtU);
    b.push(Instruction::BrIf(1));
    b.push(Instruction::LocalGet(counter));
    b.push(Instruction::I32Const(1));
    b.push(Instruction::I32Add);
    b.push(Instruction::LocalSet(counter));
    b.push(Instruction::LocalGet(4));
    b.push(Instruction::I32Const(1));
    b.push(Instruction::I32Add);
    b.push(Instruction::LocalSet(4));
    b.push(Instruction::Br(0));
    b.push(Instruction::End);
    b.push(Instruction::End);
  };

  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(1));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(2));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(3));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(6));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(8));

  // optional sign
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  byte_at_i(&mut b);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(b'-' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(b'+' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::If(BlockType::Empty));
  inc_i(&mut b);
  b.push(Instruction::End);
  b.push(Instruction::End);

  // Case-insensitive spellings: the rest must equal one word exactly.
  // Local 6 doubles as the "special matched" flag until the numeric path starts.
  for word in ["inf", "infinity", "nan"] {
    // matched = (len - i == word.len())
    b.push(Instruction::LocalGet(2));
    b.push(Instruction::LocalGet(4));
    b.push(Instruction::I32Sub);
    b.push(Instruction::I32Const(word.len() as i32));
    b.push(Instruction::I32Eq);
    b.push(Instruction::If(BlockType::Empty));
    for (k, ch) in word.bytes().enumerate() {
      // if (content[i + k] | 0x20) != ch then mismatch
      b.push(Instruction::LocalGet(3));
      b.push(Instruction::LocalGet(4));
      b.push(Instruction::I32Add);
      b.push(Instruction::I32Load8U(mem_arg_byte(k as u64)));
      b.push(Instruction::I32Const(0x20));
      b.push(Instruction::I32Or);
      b.push(Instruction::I32Const(ch as i32));
      b.push(Instruction::I32Ne);
      b.push(Instruction::If(BlockType::Empty));
      b.push(Instruction::I32Const(-1));
      b.push(Instruction::LocalSet(8));
      b.push(Instruction::End);
    }
    // local 8 stays 0 when every byte matched
    b.push(Instruction::LocalGet(8));
    b.push(Instruction::I32Eqz);
    b.push(Instruction::If(BlockType::Empty));
    b.push(Instruction::I32Const(1));
    b.push(Instruction::LocalSet(7));
    b.push(Instruction::End);
    b.push(Instruction::I32Const(0));
    b.push(Instruction::LocalSet(8));
    b.push(Instruction::End);
  }

  // Numeric path only when no spelling matched.
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Eqz);
  b.push(Instruction::If(BlockType::Empty));
  digit_loop(&mut b, 6);
  // optional fraction
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  byte_at_i(&mut b);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(b'.' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  inc_i(&mut b);
  digit_loop(&mut b, 6);
  b.push(Instruction::End);
  b.push(Instruction::End);
  // at least one mantissa digit
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::If(BlockType::Empty));
  // ok = 1 unless a malformed exponent follows
  b.push(Instruction::I32Const(1));
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  byte_at_i(&mut b);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(0x20));
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Const(b'e' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  inc_i(&mut b);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  byte_at_i(&mut b);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(b'-' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(b'+' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::If(BlockType::Empty));
  inc_i(&mut b);
  b.push(Instruction::End);
  b.push(Instruction::End);
  digit_loop(&mut b, 8);
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Eqz);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::End);
  b.push(Instruction::End);
  b.push(Instruction::End);
  // the whole text must be consumed
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Ne);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::End);
  b.push(Instruction::End); // end if digits
  b.push(Instruction::End); // end numeric path

  b.push(Instruction::LocalGet(7));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![ValType::I32; 8],
    instructions: b,
  }
}

/// `__rt_char_from_code(cp: f64) → f64`
///
/// Encodes a Unicode codepoint (u32) as a UTF-8 string and allocates it on the heap.
/// Returns the logical string pointer as f64.
#[allow(clippy::vec_init_then_push)]
fn build_rt_char_from_code(str_tag: i32) -> CompiledFn {
  // params: 0=cp(f64)
  // locals: 1=code(i32), 2=raw_base(i32), 3=new_ptr(i32), 4=byte_len(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // Reject non-integers, out-of-range values, and surrogate code points.
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Trunc);
  b.push(Instruction::F64Ne);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::Unreachable);
  b.push(Instruction::End);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(0.0.into()));
  b.push(Instruction::F64Lt);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::Unreachable);
  b.push(Instruction::End);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const((0x10ffff as f64).into()));
  b.push(Instruction::F64Gt);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::Unreachable);
  b.push(Instruction::End);

  // code = i32(cp)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(1));

  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0xd800));
  b.push(Instruction::I32GeU);
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0xdfff));
  b.push(Instruction::I32LeU);
  b.push(Instruction::I32And);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::Unreachable);
  b.push(Instruction::End);

  // Determine byte_len based on codepoint range
  // Default: 1 byte (ASCII)
  b.push(Instruction::I32Const(1));
  b.push(Instruction::LocalSet(4));

  // if code >= 0x80: byte_len = 2
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::I32GeU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);

  // if code >= 0x800: byte_len = 3
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x800));
  b.push(Instruction::I32GeU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(3));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);

  // if code >= 0x10000: byte_len = 4
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x10000));
  b.push(Instruction::I32GeU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);

  // Allocate: raw_base = HEAP_PTR; write magic+tag; new_ptr = raw_base+8
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(2)); // raw_base
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(str_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // new_ptr = raw_base + 8
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(3));
  // store byte_len as f64 at new_ptr+0
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));
  // advance heap: raw_base + 16 + ((byte_len+7)&-8)
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // Write UTF-8 bytes at new_ptr+8 based on byte_len
  // content_base = new_ptr + 8
  // byte_len == 1: store byte code (0..7F)
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Store8(mem_arg_byte(8)));
  b.push(Instruction::End);

  // byte_len == 2: 110xxxxx 10xxxxxx
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  // b0 = 0xC0 | (code >> 6)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0xC0));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(6));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(8)));
  // b1 = 0x80 | (code & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(9)));
  b.push(Instruction::End);

  // byte_len == 3: 1110xxxx 10xxxxxx 10xxxxxx
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(3));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  // b0 = 0xE0 | (code >> 12)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0xE0));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(12));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(8)));
  // b1 = 0x80 | ((code >> 6) & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(6));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(9)));
  // b2 = 0x80 | (code & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(10)));
  b.push(Instruction::End);

  // byte_len == 4: 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  // b0 = 0xF0 | (code >> 18)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0xF0));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(18));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(8)));
  // b1 = 0x80 | ((code >> 12) & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(12));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(9)));
  // b2 = 0x80 | ((code >> 6) & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(6));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(10)));
  // b3 = 0x80 | (code & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(11)));
  b.push(Instruction::End);

  // return f64(new_ptr)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // code (1)
      ValType::I32, // raw_base (2)
      ValType::I32, // new_ptr (3)
      ValType::I32, // byte_len (4)
    ],
    instructions: b,
  }
}

/// `__rt_str_replace(s: f64, pat: f64, rep: f64) → f64`
///
/// Replaces ALL non-overlapping occurrences of `pat` in `s` with `rep`.
/// Writes output directly to heap — no separate scratch buffer needed.
///
/// Layout of a string logical ptr P: [byte_len:f64][utf8_bytes...]
#[allow(clippy::vec_init_then_push)]
fn build_rt_str_replace(str_tag: i32) -> CompiledFn {
  // params: 0=s(f64), 1=pat(f64), 2=rep(f64)
  // locals: 3=s_ptr(i32), 4=s_len(i32), 5=s_cont(i32)
  //         6=pat_ptr(i32), 7=pat_len(i32), 8=pat_cont(i32)
  //         9=rep_ptr(i32), 10=rep_len(i32), 11=rep_cont(i32)
  //         12=raw_base(i32), 13=content_base(i32)
  //         14=out_pos(i32), 15=si(i32)
  //         16=pi(i32), 17=matched(i32)
  //         18=new_ptr(i32), 19=padded(i32)
  //         20=max_out(i32), 21=tmp(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // unpack s
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(3));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(5));

  // unpack pat
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(6));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8));

  // if pat_len == 0: return s unchanged
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Eqz);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::Return);
  b.push(Instruction::End);

  // unpack rep
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(9));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(10));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(11));

  // max_out = s_len + (s_len + 1) * rep_len (worst case: all chars replaced)
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::I32Mul);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(20));

  // raw_base = HEAP_PTR (save before allocating)
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalSet(12));

  // content_base = raw_base + 16 (after 8-byte header + 8-byte byte_len)
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(13));

  // padded = (max_out + 7) & -8
  b.push(Instruction::LocalGet(20));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(19));

  // Pessimistic bump: HEAP_PTR = raw_base + 16 + padded
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(19));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // out_pos = 0, si = 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(15));

  // Main scan loop: while si + pat_len <= s_len
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  // break if si + pat_len > s_len
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32GtU);
  b.push(Instruction::BrIf(1));

  // Try to match: pi = 0, matched = 1
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(16));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::LocalSet(17));

  // Inner compare loop
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  // if pi >= pat_len: done (matched)
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  // if s_cont[si+pi] != pat_cont[pi]: mismatch
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::I32Ne);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(17)); // matched = 0
  b.push(Instruction::Br(2)); // break out of inner block
  b.push(Instruction::End);
  // pi++; continue
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(16));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // if matched: copy rep → content_base+out_pos, si += pat_len, out_pos += rep_len
  b.push(Instruction::LocalGet(17));
  b.push(Instruction::If(BlockType::Empty));
  // memory.copy(content_base + out_pos, rep_cont, rep_len)
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(14)); // out_pos += rep_len
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(15)); // si += pat_len
  b.push(Instruction::Else);
  // not matched: copy 1 byte; si++; out_pos++
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(15));
  b.push(Instruction::End);
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // Copy tail: s_cont[si..s_len] → content_base + out_pos
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(21)); // tmp = s_len - si
  b.push(Instruction::LocalGet(21));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::I32GtU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(21));
  b.push(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::LocalGet(21));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::End);

  // Write header: magic, str_tag, byte_len at raw_base
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(str_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // new_ptr = raw_base + 8
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(18));
  // byte_len = out_pos
  b.push(Instruction::LocalGet(18));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));

  // Correct HEAP_PTR: raw_base + 16 + ((out_pos + 7) & -8)
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(19));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(19));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // return f64(new_ptr)
  b.push(Instruction::LocalGet(18));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::F64, ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // s_ptr (3)
      ValType::I32, // s_len (4)
      ValType::I32, // s_cont (5)
      ValType::I32, // pat_ptr (6)
      ValType::I32, // pat_len (7)
      ValType::I32, // pat_cont (8)
      ValType::I32, // rep_ptr (9)
      ValType::I32, // rep_len (10)
      ValType::I32, // rep_cont (11)
      ValType::I32, // raw_base (12)
      ValType::I32, // content_base (13)
      ValType::I32, // out_pos (14)
      ValType::I32, // si (15)
      ValType::I32, // pi (16)
      ValType::I32, // matched (17)
      ValType::I32, // new_ptr (18)
      ValType::I32, // padded (19)
      ValType::I32, // max_out (20)
      ValType::I32, // tmp (21)
    ],
    instructions: b,
  }
}

/// `__rt_str_escape(s: f64) → f64`
///
/// Escapes special characters in a string and wraps in double quotes.
fn build_rt_str_escape(string_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let src = b.alloc_i32();
  let len = b.alloc_i32();
  let index = b.alloc_i32();
  let byte = b.alloc_i32();
  let extra = b.alloc_i32();
  let out_len = b.alloc_i32();
  let padded = b.alloc_i32();
  let payload = b.alloc_i32();
  let out = b.alloc_i32();
  let dst = b.alloc_i32();
  let out_index = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(src));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(extra));

  // Pass 1: count escape bytes for \n, \r, \t, ", ', \
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(byte));

  for escaped in b"\n\r\t\"'\\" {
    b.emit(Instruction::LocalGet(byte));
    b.emit(Instruction::I32Const(*escaped as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(extra));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(extra));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Allocate: 2 quotes + len + extra
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::LocalGet(extra));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_len));
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(payload));
  rt_emit_alloc_dynamic(&mut b, payload, out, string_tag);
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst));

  // Leading '"'
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(out_index));

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(byte));

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\n' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\r' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\t' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\'' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\n' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(b'n' as i32));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\r' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(b'r' as i32));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\t' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(b't' as i32));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Trailing '"'
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));

  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::F64ConvertI32U);
  b.finish(vec![ValType::F64], vec![ValType::F64])
}

/// `__rt_edn_string(s: f64) → f64`
///
/// Formats a heap string as a Cirru EDN string leaf. Simple ASCII tokens use
/// the `|text` form; other values use a quoted `"|text"` leaf and escape the
/// four characters handled specially by the Cirru writer.
fn build_rt_edn_string(string_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let src = b.alloc_i32();
  let len = b.alloc_i32();
  let index = b.alloc_i32();
  let byte = b.alloc_i32();
  let simple = b.alloc_i32();
  let extra = b.alloc_i32();
  let out_len = b.alloc_i32();
  let padded = b.alloc_i32();
  let payload = b.alloc_i32();
  let out = b.alloc_i32();
  let dst = b.alloc_i32();
  let out_index = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(src));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(len));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32Const(edn::MAX_EDN_OUTPUT_BYTES));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(simple));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(extra));

  // First pass: decide whether the leaf can stay bare and count escapes.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(byte));

  // Cirru bare-leaf characters: ASCII alphanumeric plus writer punctuation.
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'A' as i32));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'Z' as i32));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'a' as i32));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'z' as i32));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  for allowed in b"$-:<>[]{}*=+.,\\/!?~_@#&%^|;'" {
    b.emit(Instruction::LocalGet(byte));
    b.emit(Instruction::I32Const(*allowed as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::I32Or);
  }
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(simple));
  b.emit(Instruction::End);

  for escaped in b"\n\t\"\\" {
    b.emit(Instruction::LocalGet(byte));
    b.emit(Instruction::I32Const(*escaped as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(extra));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(extra));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // bare: `|` + bytes; quoted: `"|` + escaped bytes + `"`.
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::LocalGet(simple));
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::LocalGet(extra));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_len));
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::I32Const(edn::MAX_EDN_OUTPUT_BYTES));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(payload));
  rt_emit_alloc_dynamic(&mut b, payload, out, string_tag);
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst));

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::LocalGet(simple));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'|' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(byte));

  b.emit(Instruction::LocalGet(simple));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\n' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\t' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\n' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(b'n' as i32));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\t' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(b't' as i32));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(simple));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::F64ConvertI32U);
  b.finish(vec![ValType::F64], vec![ValType::F64])
}

/// `__rt_json_string(s: f64) → f64`
///
/// Formats a heap string as an RFC 8259 JSON string literal (`"..."`),
/// escaping `"`, `\`, `\b`, `\f`, `\n`, `\r`, `\t`, and any control byte `< 0x20`
/// as `\u00xx`.
fn build_rt_json_string(string_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let src = b.alloc_i32();
  let len = b.alloc_i32();
  let index = b.alloc_i32();
  let byte = b.alloc_i32();
  let extra = b.alloc_i32();
  let out_len = b.alloc_i32();
  let padded = b.alloc_i32();
  let payload = b.alloc_i32();
  let out = b.alloc_i32();
  let dst = b.alloc_i32();
  let out_index = b.alloc_i32();
  let hex_digit = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(src));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(len));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32Const(edn::MAX_EDN_OUTPUT_BYTES));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(extra));

  // Pass 1: count extra escape bytes.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(byte));

  // '"', '\\', '\b' (8), '\t' (9), '\n' (10), '\f' (12), '\r' (13) add 1 byte.
  // Any other control byte (< 0x20) adds 5 bytes ("\u00xx").
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x08));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x09));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0a));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0c));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0d));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(extra));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(extra));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x20));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(extra));
  b.emit(Instruction::I32Const(5));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(extra));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::LocalGet(extra));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_len));
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::I32Const(edn::MAX_EDN_OUTPUT_BYTES));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Unreachable);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(payload));
  rt_emit_alloc_dynamic(&mut b, payload, out, string_tag);
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::LocalGet(out_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst));

  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(index));

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(byte));

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x08));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x09));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0a));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0c));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0d));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  for (raw_byte, esc_char) in [(0x08, b'b'), (0x09, b't'), (0x0a, b'n'), (0x0c, b'f'), (0x0d, b'r')] {
    b.emit(Instruction::LocalGet(byte));
    b.emit(Instruction::I32Const(raw_byte));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::I32Const(esc_char as i32));
    b.emit(Instruction::LocalSet(byte));
    b.emit(Instruction::End);
  }
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x20));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  for (offset, ch) in [(0, b'\\'), (1, b'u'), (2, b'0'), (3, b'0')] {
    b.emit(Instruction::LocalGet(dst));
    b.emit(Instruction::LocalGet(out_index));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::I32Const(ch as i32));
    b.emit(Instruction::I32Store8(mem_arg_byte(offset)));
  }
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::LocalSet(hex_digit));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(hex_digit));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Store8(mem_arg_byte(4)));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x0f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(hex_digit));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(hex_digit));
  b.emit(Instruction::I32Const(10));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(hex_digit));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(hex_digit));
  b.emit(Instruction::I32Const((b'a' - 10) as i32));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Store8(mem_arg_byte(5)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(6));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_index));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(index));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(index));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_index));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::F64ConvertI32U);
  b.finish(vec![ValType::F64], vec![ValType::F64])
}

/// `__rt_tag_from_str(str_ptr: i32) → f64`
///
/// Looks up a heap string's bytes in the static tag table and returns the tag id as f64,
/// or 0.0 if the tag is not registered in the compiled module.
fn build_rt_tag_from_str(tag_index: &HashMap<String, u32>) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let len = b.alloc_i32();
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(len));

  let mut by_len: BTreeMap<usize, Vec<(&str, u32)>> = BTreeMap::new();
  for (name, &id) in tag_index {
    by_len.entry(name.len()).or_default().push((name.as_str(), id));
  }
  for entries in by_len.values_mut() {
    entries.sort_by_key(|(_, id)| *id);
  }

  for (tag_len, entries) in by_len {
    b.emit(Instruction::LocalGet(len));
    b.emit(Instruction::I32Const(tag_len as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::If(BlockType::Empty));
    for (name, id) in entries {
      if tag_len == 0 {
        b.emit(f64_const(id as f64));
        b.emit(Instruction::Return);
      } else {
        for (idx, ch) in name.bytes().enumerate() {
          b.emit(Instruction::LocalGet(0));
          b.emit(Instruction::I32Load8U(mem_arg_byte(8 + idx as u64)));
          b.emit(Instruction::I32Const(ch as i32));
          b.emit(Instruction::I32Eq);
          if idx > 0 {
            b.emit(Instruction::I32And);
          }
        }
        b.emit(Instruction::If(BlockType::Empty));
        b.emit(f64_const(id as f64));
        b.emit(Instruction::Return);
        b.emit(Instruction::End);
      }
    }
    b.emit(Instruction::End);
  }

  b.emit(f64_const(0.0));
  b.finish(vec![ValType::I32], vec![ValType::F64])
}

/// `__rt_json_parse_str_at(src_ptr: i32, pos: i32, end: i32) → (str_ptr: i32, next_pos: i32, ok: i32)`
///
/// Parses an RFC 8259 JSON string starting at `pos` (where `src[pos] == '"'`).
/// Supports `\"`, `\\`, `\/`, `\b`, `\f`, `\n`, `\r`, `\t`, and `\uXXXX` (including surrogate pairs).
fn build_rt_json_parse_str_at(string_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3); // 0=src_ptr, 1=pos, 2=end
  let cur = b.alloc_i32();
  let max_len = b.alloc_i32();
  let padded = b.alloc_i32();
  let payload = b.alloc_i32();
  let req_bytes = b.alloc_i32();
  let heap_mark = b.alloc_i32();
  let out = b.alloc_i32();
  let dst = b.alloc_i32();
  let out_idx = b.alloc_i32();
  let byte = b.alloc_i32();
  let cp = b.alloc_i32();
  let cp2 = b.alloc_i32();
  let hi = b.alloc_i32();
  let nib = b.alloc_i32();

  let fail_after_alloc = |b: &mut RuntimeFnBuilder, cur_local: u32| {
    b.emit(Instruction::LocalGet(out));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::I32Store(mem_arg_i32(0)));
    b.emit(Instruction::LocalGet(heap_mark));
    b.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::Return);
  };

  // Must start with '"'
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  // Allocate pessimistic output string of length (end - pos), and shrink/rewind on exit.
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(max_len));
  b.emit(Instruction::LocalGet(max_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(payload));
  b.emit(Instruction::LocalGet(payload));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(req_bytes));
  rt_emit_ensure_heap_capacity(&mut b, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.emit(Instruction::LocalSet(heap_mark));
  rt_emit_alloc_dynamic(&mut b, payload, out, string_tag);
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst));

  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(out_idx));

  let emit_hex_nibble = |b: &mut RuntimeFnBuilder, src_local: u32, cur_local: u32, end_local: u32, nib_local: u32, acc_local: u32| {
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::LocalGet(end_local));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::If(BlockType::Empty));
    fail_after_alloc(b, cur_local);
    b.emit(Instruction::End);
    b.emit(Instruction::LocalGet(src_local));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
    b.emit(Instruction::LocalSet(nib_local));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(cur_local));

    b.emit(Instruction::LocalGet(nib_local));
    b.emit(Instruction::I32Const(b'0' as i32));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::LocalGet(nib_local));
    b.emit(Instruction::I32Const(b'9' as i32));
    b.emit(Instruction::I32LeU);
    b.emit(Instruction::I32And);
    b.emit(Instruction::If(BlockType::Result(ValType::I32)));
    b.emit(Instruction::LocalGet(nib_local));
    b.emit(Instruction::I32Const(b'0' as i32));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::Else);
    b.emit(Instruction::LocalGet(nib_local));
    b.emit(Instruction::I32Const(0x20));
    b.emit(Instruction::I32Or);
    b.emit(Instruction::LocalTee(nib_local));
    b.emit(Instruction::I32Const(b'a' as i32));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::LocalGet(nib_local));
    b.emit(Instruction::I32Const(b'f' as i32));
    b.emit(Instruction::I32LeU);
    b.emit(Instruction::I32And);
    b.emit(Instruction::If(BlockType::Result(ValType::I32)));
    b.emit(Instruction::LocalGet(nib_local));
    b.emit(Instruction::I32Const((b'a' - 10) as i32));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::Else);
    fail_after_alloc(b, cur_local);
    b.emit(Instruction::Unreachable);
    b.emit(Instruction::End);
    b.emit(Instruction::End);
    b.emit(Instruction::LocalGet(acc_local));
    b.emit(Instruction::I32Const(4));
    b.emit(Instruction::I32Shl);
    b.emit(Instruction::I32Or);
    b.emit(Instruction::LocalSet(acc_local));
  };

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));

  // Closing '"'
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  // Reclaim unused trailing padding beyond the decoded string byte length.
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  // Unescaped control characters (< 0x20) are invalid in JSON strings.
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(0x20));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);

  // Escape sequence
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'u' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(cp));
  for _ in 0..4 {
    emit_hex_nibble(&mut b, 0, cur, 2, nib, cp);
  }
  // High surrogate 0xD800..0xDBFF must be followed by `\uDC00`..`\uDFFF`
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0xd800));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0xdbff));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(6));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(9)));
  b.emit(Instruction::I32Const(b'u' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::LocalSet(hi));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(cp2));
  for _ in 0..4 {
    emit_hex_nibble(&mut b, 0, cur, 2, nib, cp2);
  }
  b.emit(Instruction::LocalGet(cp2));
  b.emit(Instruction::I32Const(0xdc00));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(cp2));
  b.emit(Instruction::I32Const(0xdfff));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(hi));
  b.emit(Instruction::I32Const(0xd800));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Const(10));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::LocalGet(cp2));
  b.emit(Instruction::I32Const(0xdc00));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x10000));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cp));
  b.emit(Instruction::Else);
  // Reject lone low surrogate 0xDC00..0xDFFF
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0xdc00));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0xdfff));
  b.emit(Instruction::I32LeU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Encode Unicode scalar `cp` as UTF-8 into `dst + out_idx`
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0x800));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0xc0));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(6));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0x3f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(1)));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0x10000));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0xe0));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(12));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(6));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(0x3f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(1)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0x3f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(2)));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0xf0));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(18));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(12));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(0x3f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(1)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(6));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(0x3f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(2)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::LocalGet(cp));
  b.emit(Instruction::I32Const(0x3f));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Store8(mem_arg_byte(3)));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  // Single-character JSON escapes: \", \\, \/, \b, \f, \n, \r, \t
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'\\' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'/' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'b' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x08));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'f' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x0c));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'n' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x0a));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'r' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x0d));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b't' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0x09));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::Else);
  fail_after_alloc(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_idx));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(out_idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_idx));
  b.emit(Instruction::End);

  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  fail_after_alloc(&mut b, cur);
  b.finish(vec![ValType::I32; 3], vec![ValType::I32; 3])
}

/// `__rt_json_parse_at(src_ptr: i32, pos: i32, end: i32, depth: i32) → (val: f64, next_pos: i32, ok: i32)`
fn build_rt_json_parse_at(
  (string_tag, list_tag, map_tag, nil_tag, bool_tag, number_tag): (i32, i32, i32, i32, i32, i32),
  (self_idx, parse_str_idx, tag_from_str_idx, map_from_flat_idx, parse_float_valid_idx, parse_float_idx): (
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
  ),
) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(4); // 0=src_ptr, 1=pos, 2=end, 3=depth
  let cur = b.alloc_i32();
  let byte = b.alloc_i32();
  let str_ptr = b.alloc_i32();
  let boxed_ptr = b.alloc_i32();
  let ok = b.alloc_i32();
  let val = b.alloc_f64();
  let list_buf = b.alloc_i32();
  let list_cap = b.alloc_i32();
  let list_cnt = b.alloc_i32();
  let list_alloc_size = b.alloc_i32();
  let obj_buf = b.alloc_i32();
  let obj_cap = b.alloc_i32();
  let pair_cnt = b.alloc_i32();
  let member_cnt = b.alloc_i32();
  let obj_alloc_size = b.alloc_i32();
  let key_heap_mark = b.alloc_i32();
  let j = b.alloc_i32();
  let found_key = b.alloc_i32();
  let req_bytes = b.alloc_i32();
  let map_ptr = b.alloc_i32();
  let key_tag = b.alloc_f64();
  let num_start = b.alloc_i32();
  let num_len = b.alloc_i32();
  let num_padded = b.alloc_i32();
  let num_payload = b.alloc_i32();
  let num_heap_mark = b.alloc_i32();
  let num_str = b.alloc_i32();
  let int_digits = b.alloc_i32();
  let first_digit = b.alloc_i32();

  let skip_ws = |b: &mut RuntimeFnBuilder, src_local: u32, cur_local: u32, end_local: u32, byte_local: u32| {
    b.emit(Instruction::Block(BlockType::Empty));
    b.emit(Instruction::Loop(BlockType::Empty));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::LocalGet(end_local));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::BrIf(1));
    b.emit(Instruction::LocalGet(src_local));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
    b.emit(Instruction::LocalSet(byte_local));
    emit_is_ws_check(&mut b.instructions, byte_local);
    b.emit(Instruction::I32Eqz);
    b.emit(Instruction::BrIf(1));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(cur_local));
    b.emit(Instruction::Br(0));
    b.emit(Instruction::End);
    b.emit(Instruction::End);
  };

  let fail_return = |b: &mut RuntimeFnBuilder, cur_local: u32| {
    b.emit(f64_const(0.0));
    b.emit(Instruction::LocalGet(cur_local));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::Return);
  };

  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, 1);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalSet(cur));
  skip_ws(&mut b, 0, cur, 2, byte);

  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));

  // Literal words: box `"null"` with `nil_tag` and `"true"`/`"false"` with `bool_tag` so
  // dynamic JSON values remain distinct from numeric `0.0` / `1.0`.
  for (word, kind_tag, result_val) in [("null", nil_tag, 0.0), ("true", bool_tag, 1.0), ("false", bool_tag, 0.0)] {
    b.emit(Instruction::LocalGet(byte));
    b.emit(Instruction::I32Const(word.as_bytes()[0] as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::LocalGet(cur));
    b.emit(Instruction::I32Const(word.len() as i32));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalGet(2));
    b.emit(Instruction::I32GtU);
    b.emit(Instruction::If(BlockType::Empty));
    fail_return(&mut b, cur);
    b.emit(Instruction::End);
    for (idx, ch) in word.bytes().enumerate().skip(1) {
      b.emit(Instruction::LocalGet(0));
      b.emit(Instruction::LocalGet(cur));
      b.emit(Instruction::I32Add);
      b.emit(Instruction::I32Load8U(mem_arg_byte(8 + idx as u64)));
      b.emit(Instruction::I32Const(ch as i32));
      b.emit(Instruction::I32Ne);
      b.emit(Instruction::If(BlockType::Empty));
      fail_return(&mut b, cur);
      b.emit(Instruction::End);
    }
    rt_emit_ensure_heap_capacity_const(&mut b, 16, req_bytes);
    b.emit(Instruction::I32Eqz);
    b.emit(Instruction::If(BlockType::Empty));
    fail_return(&mut b, cur);
    b.emit(Instruction::End);
    rt_emit_alloc_const(&mut b, 8, boxed_ptr, kind_tag);
    b.emit(Instruction::LocalGet(boxed_ptr));
    b.emit(f64_const(result_val));
    b.emit(Instruction::F64Store(mem_arg_f64(0)));
    b.emit(Instruction::LocalGet(boxed_ptr));
    b.emit(Instruction::F64ConvertI32U);
    b.emit(Instruction::LocalGet(cur));
    b.emit(Instruction::I32Const(word.len() as i32));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::Return);
    b.emit(Instruction::End);
  }

  // String literal: "..."
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'"' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::Call(parse_str_idx));
  b.emit(Instruction::LocalSet(ok));
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalSet(str_ptr));
  b.emit(Instruction::LocalGet(ok));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  // Array literal: [...]
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'[' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  skip_ws(&mut b, 0, cur, 2, byte);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Empty array `[]`
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(b']' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  rt_emit_ensure_heap_capacity_const(&mut b, 16, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  rt_emit_alloc_const(&mut b, 8, list_buf, list_tag);
  b.emit(Instruction::LocalGet(list_buf));
  b.emit(f64_const(0.0));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(list_buf));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::LocalSet(list_cap));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(list_cnt));
  b.emit(Instruction::I32Const(8 + 8 * 8));
  b.emit(Instruction::LocalSet(list_alloc_size));
  rt_emit_ensure_heap_capacity_const(&mut b, 8 + 8 + 8 * 8, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  rt_emit_alloc_dynamic(&mut b, list_alloc_size, list_buf, list_tag);

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(list_cnt));
  b.emit(Instruction::I32Const(4096));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::Call(self_idx));
  b.emit(Instruction::LocalSet(ok));
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalSet(val));
  b.emit(Instruction::LocalGet(ok));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Grow list buffer if list_cnt == list_cap
  b.emit(Instruction::LocalGet(list_cnt));
  b.emit(Instruction::LocalGet(list_cap));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(list_cap));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::LocalSet(list_cap));
  b.emit(Instruction::LocalGet(list_cap));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(list_alloc_size));
  b.emit(Instruction::LocalGet(list_alloc_size));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(req_bytes));
  rt_emit_ensure_heap_capacity(&mut b, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  let new_list_buf = b.alloc_i32();
  rt_emit_alloc_dynamic(&mut b, list_alloc_size, new_list_buf, list_tag);
  b.emit(Instruction::LocalGet(new_list_buf));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(list_buf));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(list_cnt));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.emit(Instruction::LocalGet(new_list_buf));
  b.emit(Instruction::LocalSet(list_buf));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(list_buf));
  b.emit(Instruction::LocalGet(list_cnt));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(list_cnt));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(list_cnt));

  skip_ws(&mut b, 0, cur, 2, byte);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b']' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(list_buf));
  b.emit(Instruction::LocalGet(list_cnt));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(list_buf));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b',' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Object literal: {...}
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'{' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  skip_ws(&mut b, 0, cur, 2, byte);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Empty object `{}`
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(b'}' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  rt_emit_ensure_heap_capacity_const(&mut b, 24, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  rt_emit_alloc_const(&mut b, 16, map_ptr, map_tag);
  b.emit(Instruction::LocalGet(map_ptr));
  b.emit(f64_const(0.0));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(map_ptr));
  b.emit(f64_const(0.0));
  b.emit(Instruction::F64Store(mem_arg_f64(8)));
  b.emit(Instruction::LocalGet(map_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::LocalSet(obj_cap));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(pair_cnt));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(member_cnt));
  b.emit(Instruction::I32Const(8 + 8 * 16));
  b.emit(Instruction::LocalSet(obj_alloc_size));
  rt_emit_ensure_heap_capacity_const(&mut b, 8 + 8 + 8 * 16, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  rt_emit_alloc_dynamic(&mut b, obj_alloc_size, obj_buf, map_tag);

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(member_cnt));
  b.emit(Instruction::I32Const(2048));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  skip_ws(&mut b, 0, cur, 2, byte);
  b.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.emit(Instruction::LocalSet(key_heap_mark));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::Call(parse_str_idx));
  b.emit(Instruction::LocalSet(ok));
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalSet(str_ptr));
  b.emit(Instruction::LocalGet(ok));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::Call(tag_from_str_idx));
  b.emit(Instruction::LocalSet(key_tag));
  // Reclaim temporary key string now that tag_from_str has resolved it to a tag ID.
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(key_heap_mark));
  b.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  b.emit(Instruction::LocalGet(key_tag));
  b.emit(f64_const(0.0));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  skip_ws(&mut b, 0, cur, 2, byte);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(b':' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::Call(self_idx));
  b.emit(Instruction::LocalSet(ok));
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalSet(val));
  b.emit(Instruction::LocalGet(ok));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Update duplicate key in place inside the parser-owned flat buffer; append only new keys.
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(found_key));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(key_tag));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(found_key));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(found_key));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::LocalGet(obj_cap));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(obj_cap));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::LocalSet(obj_cap));
  b.emit(Instruction::LocalGet(obj_cap));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(obj_alloc_size));
  b.emit(Instruction::LocalGet(obj_alloc_size));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(req_bytes));
  rt_emit_ensure_heap_capacity(&mut b, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  let new_obj_buf = b.alloc_i32();
  rt_emit_alloc_dynamic(&mut b, obj_alloc_size, new_obj_buf, map_tag);
  b.emit(Instruction::LocalGet(new_obj_buf));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.emit(Instruction::LocalGet(new_obj_buf));
  b.emit(Instruction::LocalSet(obj_buf));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(key_tag));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(pair_cnt));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(member_cnt));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(member_cnt));

  skip_ws(&mut b, 0, cur, 2, byte);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'}' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::LocalGet(pair_cnt));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(obj_buf));
  b.emit(Instruction::Call(map_from_flat_idx));
  b.emit(Instruction::LocalSet(map_ptr));
  b.emit(Instruction::LocalGet(map_ptr));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(map_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b',' as i32));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Number literal: optional '-', integer digits (no leading zero except single '0'),
  // optional '.' + digits, optional ('e'|'E') + ['+'|'-'] + digits.
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalSet(num_start));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'-' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(first_digit));
  b.emit(Instruction::LocalGet(first_digit));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(first_digit));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(int_digits));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(int_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(int_digits));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Reject leading zero followed by more integer digits (e.g. `01`)
  b.emit(Instruction::LocalGet(first_digit));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(int_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Optional fractional part: '.' followed by at least one digit
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(b'.' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(int_digits));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(int_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(int_digits));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(int_digits));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Optional exponent part: ('e'|'E') ['+'|'-'] digits+
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::I32Const(0x20));
  b.emit(Instruction::I32Or);
  b.emit(Instruction::I32Const(b'e' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'+' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'-' as i32));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(int_digits));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'0' as i32));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::LocalGet(byte));
  b.emit(Instruction::I32Const(b'9' as i32));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(int_digits));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(int_digits));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(int_digits));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Slice the number token into a heap string and parse via __rt_parse_float_valid / __rt_parse_float.
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(num_start));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(num_len));
  b.emit(Instruction::LocalGet(num_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(num_padded));
  b.emit(Instruction::LocalGet(num_padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(num_payload));
  b.emit(Instruction::LocalGet(num_payload));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(req_bytes));
  rt_emit_ensure_heap_capacity(&mut b, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  b.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.emit(Instruction::LocalSet(num_heap_mark));
  rt_emit_alloc_dynamic(&mut b, num_payload, num_str, string_tag);
  b.emit(Instruction::LocalGet(num_str));
  b.emit(Instruction::LocalGet(num_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(num_str));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(num_start));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(num_len));
  b.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  let reclaim_num_str = |b: &mut RuntimeFnBuilder| {
    b.emit(Instruction::LocalGet(num_str));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Sub);
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::I32Store(mem_arg_i32(0)));
    b.emit(Instruction::LocalGet(num_heap_mark));
    b.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  };

  b.emit(Instruction::LocalGet(num_str));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::Call(parse_float_valid_idx));
  b.emit(f64_const(0.0));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::If(BlockType::Empty));
  reclaim_num_str(&mut b);
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Guarantee `__rt_parse_float` can reserve its 1024-byte midpoint scratch
  // buffer without trapping when memory cannot grow.
  rt_emit_ensure_heap_capacity_const(&mut b, 1024, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  reclaim_num_str(&mut b);
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(num_str));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::Call(parse_float_idx));
  b.emit(Instruction::LocalSet(val));
  reclaim_num_str(&mut b);

  // Reject non-finite overflow (Infinity / NaN)
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::F64Sub);
  b.emit(f64_const(0.0));
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);

  // Box parsed JSON numbers with `number_tag` so integer values in the heap
  // range remain distinguishable from heap pointers under `type-of` and decoding.
  rt_emit_ensure_heap_capacity_const(&mut b, 16, req_bytes);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  fail_return(&mut b, cur);
  b.emit(Instruction::End);
  rt_emit_alloc_const(&mut b, 8, boxed_ptr, number_tag);
  b.emit(Instruction::LocalGet(boxed_ptr));
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(boxed_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.finish(vec![ValType::I32; 4], vec![ValType::F64, ValType::I32, ValType::I32])
}

/// `__rt_json_parse_root(s: f64) → (val: f64, ok: i32)`
fn build_rt_json_parse_root(parse_at_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let src = b.alloc_i32();
  let len = b.alloc_i32();
  let heap_mark = b.alloc_i32();
  let cur = b.alloc_i32();
  let ok = b.alloc_i32();
  let byte = b.alloc_i32();
  let val = b.alloc_f64();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(src));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(len));

  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32Const(edn::MAX_EDN_OUTPUT_BYTES));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(f64_const(0.0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.emit(Instruction::LocalSet(heap_mark));

  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Call(parse_at_idx));
  b.emit(Instruction::LocalSet(ok));
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::LocalSet(val));

  b.emit(Instruction::LocalGet(ok));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(heap_mark));
  b.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  b.emit(f64_const(0.0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  // Skip trailing whitespace and ensure we reached `len`.
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::LocalGet(len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(src));
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(8)));
  b.emit(Instruction::LocalSet(byte));
  emit_is_ws_check(&mut b.instructions, byte);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(heap_mark));
  b.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  b.emit(f64_const(0.0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(cur));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(cur));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::I32Const(1));
  b.finish(vec![ValType::F64], vec![ValType::F64, ValType::I32])
}

/// `__rt_map_equal(a: i32, b: i32) → i32`
/// Returns 1 if maps a and b have the same key-value pairs, else 0.
fn build_rt_map_equal(map_linearize_idx: u32, map_get_value_idx: u32, value_equal_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2); // a=0, b=1
  let count_a = b.alloc_i32();
  let count_b = b.alloc_i32();
  let flat = b.alloc_i32();
  let i = b.alloc_i32();
  let key = b.alloc_f64();
  let val_a = b.alloc_f64();
  let val_b = b.alloc_f64();
  let all_eq = b.alloc_i32();

  // count_a = a[0] as i32
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count_a));

  // count_b = b[0] as i32
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count_b));

  // if count_a != count_b → return 0
  b.emit(Instruction::LocalGet(count_a));
  b.emit(Instruction::LocalGet(count_b));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Else);

  // if count_a == 0 → return 1
  b.emit(Instruction::LocalGet(count_a));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Else);

  // linearize a → flat
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(map_linearize_idx));
  b.emit(Instruction::LocalSet(flat));

  // all_eq = 1, i = 0
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(all_eq));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));

  // loop over pairs
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  // if i >= count_a, exit
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count_a));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));

  // key = flat + 8 + i*16
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(key));

  // val_a = flat + 8 + i*16 + 8
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::LocalSet(val_a));

  // val_b = __rt_map_get_value(b_ptr, key)
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::Call(map_get_value_idx));
  b.emit(Instruction::LocalSet(val_b));

  // if val_a != val_b → all_eq = 0, break
  b.emit(Instruction::LocalGet(val_a));
  b.emit(Instruction::LocalGet(val_b));
  b.emit(Instruction::Call(value_equal_idx));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(all_eq));
  b.emit(Instruction::Br(2)); // break out of block
  b.emit(Instruction::End);

  // i++
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End); // loop
  b.emit(Instruction::End); // block

  b.emit(Instruction::LocalGet(all_eq));
  b.emit(Instruction::End); // end count_a == 0 else
  b.emit(Instruction::End); // end count_a != count_b if

  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}

/// `__rt_str_find_from(h_ptr: i32, h_start: i32, pat_ptr: i32) -> i32`
/// Searches for pat in h starting at byte offset h_start.
/// Returns the byte offset of the first match, or -1 if not found.
fn build_rt_str_find_from() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3); // h_ptr=0, h_start=1, pat_ptr=2
  let h_len = b.alloc_i32();
  let p_len = b.alloc_i32();
  let h_base = b.alloc_i32();
  let p_base = b.alloc_i32();
  let i = b.alloc_i32();
  let j = b.alloc_i32();
  let limit = b.alloc_i32();
  let bh = b.alloc_i32();
  let bp = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(h_len));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(p_len));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(h_base));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(p_base));

  // Block (result i32) for early-exit
  b.emit(Instruction::Block(wasm_encoder::BlockType::Result(ValType::I32)));

  // if p_len == 0: return h_start
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);

  // if h_start + p_len > h_len: return -1
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(h_len));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);

  // limit = h_len - p_len
  b.emit(Instruction::LocalGet(h_len));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(limit));

  // i = h_start
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalSet(i));

  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty)); // $exit
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty)); // $outer
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(limit));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::BrIf(1)); // $exit
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty)); // $mismatch
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty)); // $inner
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  // nesting: 0=If, 1=$inner, 2=$mismatch, 3=$outer, 4=$exit, 5=Block(i32)
  b.emit(Instruction::Br(5));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(h_base));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(bh));
  b.emit(Instruction::LocalGet(p_base));
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(bp));
  b.emit(Instruction::LocalGet(bh));
  b.emit(Instruction::LocalGet(bp));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::BrIf(1)); // $mismatch
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Br(0)); // $inner
  b.emit(Instruction::End); // end $inner
  b.emit(Instruction::End); // end $mismatch
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0)); // $outer
  b.emit(Instruction::End); // end $outer
  b.emit(Instruction::End); // end $exit
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::End); // end Block(i32)

  b.finish(vec![ValType::I32, ValType::I32, ValType::I32], vec![ValType::I32])
}

/// `__rt_utf8_char_len(b: i32) -> i32`
/// Given the first byte of a UTF-8 sequence, returns its byte width (1-4).
fn build_rt_utf8_char_len() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  b.emit(Instruction::Block(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(0xE0));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(0xF0));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::End);
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

/// `__rt_str_split(s_ptr: i32, pat_ptr: i32) -> i32` (list logical ptr)
/// Splits string s by pat. Empty pieces are dropped. Empty pat = char-split.
fn build_rt_str_split(string_tag: i32, list_tag: i32, find_from_idx: u32, utf8_char_len_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2); // s_ptr=0, pat_ptr=1
  let s_len = b.alloc_i32();
  let p_len = b.alloc_i32();
  let s_cont = b.alloc_i32();
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let prev = b.alloc_i32();
  let list_ptr = b.alloc_i32();
  let li = b.alloc_i32();
  let idx = b.alloc_i32();
  let piece_len = b.alloc_i32();
  let str_ptr = b.alloc_i32();
  let str_cont = b.alloc_i32();
  let char_len = b.alloc_i32();
  let padded = b.alloc_i32();
  let size = b.alloc_i32();
  let ki = b.alloc_i32();
  let list_size = b.alloc_i32();

  // load s_len, p_len, s_cont
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(s_len));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(p_len));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(s_cont));

  // --- PASS 1: COUNT ---
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // empty-pat: count UTF-8 chars
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(s_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(s_cont));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::Call(utf8_char_len_idx));
  b.emit(Instruction::LocalSet(char_len));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  // non-empty pat: count delimited pieces
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(prev));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(find_from_idx));
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // last segment
  b.emit(Instruction::LocalGet(s_len));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(piece_len));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::End);
  b.emit(Instruction::Br(2)); // break loop
  b.emit(Instruction::End);
  // mid segment
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(piece_len));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(prev));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End); // if p_len == 0

  // --- ALLOCATE LIST ---
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(list_size));
  rt_emit_alloc_dynamic(&mut b, list_size, list_ptr, list_tag);
  b.emit(Instruction::LocalGet(list_ptr));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));

  // --- PASS 2: FILL ---
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(li));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(prev));

  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // empty-pat: emit each UTF-8 char as a string
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(s_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(s_cont));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::Call(utf8_char_len_idx));
  b.emit(Instruction::LocalSet(char_len));
  // padded = (char_len + 7) & -8
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(&mut b, size, str_ptr, string_tag);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(str_cont));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(ki));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(str_cont));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(s_cont));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(ki));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  // store str_ptr in list
  b.emit(Instruction::LocalGet(list_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(li));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(li));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(li));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  // non-empty pat: emit each delimited piece
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(find_from_idx));
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(s_len));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(piece_len));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(piece_len));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(&mut b, size, str_ptr, string_tag);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(str_cont));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(ki));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(str_cont));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(s_cont));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(ki));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(list_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(li));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(li));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(li));
  b.emit(Instruction::End); // if piece_len > 0
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::BrIf(1)); // exit
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(prev));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);

  b.emit(Instruction::End);
  b.emit(Instruction::End); // if p_len == 0

  b.emit(Instruction::LocalGet(list_ptr));
  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}

/// `__rt_set_equal(a: i32, b: i32) → i32`
/// Compare unordered set elements using recursive value equality.
fn build_rt_set_equal(value_equal_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let j = b.alloc_i32();
  let found = b.alloc_i32();
  let result = b.alloc_i32();
  let elem = b.alloc_f64();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(result));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(elem));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(elem));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::Call(value_equal_idx));
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End); // inner loop
  b.emit(Instruction::End); // inner block
  b.emit(Instruction::LocalGet(found));
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
  b.emit(Instruction::End); // outer loop
  b.emit(Instruction::End); // outer block
  b.emit(Instruction::LocalGet(result));
  b.emit(Instruction::End); // count comparison
  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}

/// `__rt_value_equal(a: f64, b: f64) → i32`
///
/// Deep structural equality:
/// - Numbers/booleans/nil: f64 equality.
/// - Strings: byte-by-byte comparison via __rt_str_compare.
/// - Lists: element-wise recursive comparison.
/// - Enums (including Option/Result): variant and payload comparison.
/// - Structs: definition/layout identity and recursive field comparison.
/// - Maps and Sets: recursive structural comparison.
/// - Other heap objects: pointer (f64) equality.
fn build_rt_value_equal(
  (string_tag, list_tag, enum_tag, map_tag, set_tag, struct_tag, nil_tag, bool_tag, number_tag): (
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
  ),
  (str_compare_idx, self_idx, map_equal_idx, set_equal_idx, is_heap_ptr_idx, struct_load_field_idx): (u32, u32, u32, u32, u32, u32),
  struct_layouts: &StructLayouts,
) -> CompiledFn {
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

  // String comparison
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(string_tag));
  b.emit(Instruction::I32Eq);
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
  b.emit(Instruction::I32Const(0));
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
