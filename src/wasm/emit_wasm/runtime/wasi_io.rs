//! WASI Preview 1 and Component Model host/I/O runtime adapters.

use super::super::*;
use super::*;
use wasm_encoder::BlockType;

const WASI_PREOPEN_FD_LIMIT: i32 = 64;
const WASI_PREOPEN_NAME_LIMIT: i32 = 4096;
const WASI_GUEST_PATH_LIMIT: i32 = 4096;
pub(in crate::wasm::emit_wasm) const WASI_TEXT_FILE_LIMIT: i64 = 4 * 1024 * 1024;
const WASI_DIR_BUFFER_SIZE: i32 = 64 * 1024;
const WASI_DIR_ENTRY_LIMIT: i32 = 4096;
const WASI_DIR_NAME_LIMIT: i32 = 4096;
const WASI_DIR_PATH_BYTES_LIMIT: i32 = 4 * 1024 * 1024;
// Covers allocation headers, the list, read buffer, padded strings, FsPath
// wrappers, and the successful Result value at the public boundary.
const WASI_DIR_MANAGED_BYTES_LIMIT: i64 =
  56 + WASI_DIR_BUFFER_SIZE as i64 + WASI_DIR_PATH_BYTES_LIMIT as i64 + 63 * WASI_DIR_ENTRY_LIMIT as i64;

#[derive(Clone, Copy)]
pub(in crate::wasm::emit_wasm) enum WasiComponentReadSource {
  File { open: u32, descriptor_drop: u32 },
  Stdin,
}

pub(in crate::wasm::emit_wasm) struct WasiComponentReadImports {
  pub(in crate::wasm::emit_wasm) source: WasiComponentReadSource,
  pub(in crate::wasm::emit_wasm) read_via_stream: u32,
  pub(in crate::wasm::emit_wasm) stream_read: u32,
  pub(in crate::wasm::emit_wasm) stream_drop: u32,
  pub(in crate::wasm::emit_wasm) future_read: u32,
  pub(in crate::wasm::emit_wasm) future_drop: u32,
  pub(in crate::wasm::emit_wasm) free: u32,
  pub(in crate::wasm::emit_wasm) realloc: u32,
}

pub(in crate::wasm::emit_wasm) struct WasiComponentWriteImports {
  pub(in crate::wasm::emit_wasm) open: u32,
  pub(in crate::wasm::emit_wasm) stream_new: u32,
  pub(in crate::wasm::emit_wasm) stream_write: u32,
  pub(in crate::wasm::emit_wasm) stream_drop: u32,
  pub(in crate::wasm::emit_wasm) write_via_stream: u32,
  pub(in crate::wasm::emit_wasm) future_read: u32,
  pub(in crate::wasm::emit_wasm) future_drop: u32,
  pub(in crate::wasm::emit_wasm) descriptor_drop: u32,
  pub(in crate::wasm::emit_wasm) free: u32,
  pub(in crate::wasm::emit_wasm) realloc: u32,
}

pub(in crate::wasm::emit_wasm) struct WasiComponentReadDirImports {
  pub(in crate::wasm::emit_wasm) open: u32,
  pub(in crate::wasm::emit_wasm) read_directory: u32,
  pub(in crate::wasm::emit_wasm) stream_read: u32,
  pub(in crate::wasm::emit_wasm) stream_drop: u32,
  pub(in crate::wasm::emit_wasm) future_read: u32,
  pub(in crate::wasm::emit_wasm) future_drop: u32,
  pub(in crate::wasm::emit_wasm) descriptor_drop: u32,
  pub(in crate::wasm::emit_wasm) utf8_valid: u32,
  pub(in crate::wasm::emit_wasm) str_compare: u32,
  pub(in crate::wasm::emit_wasm) free: u32,
  pub(in crate::wasm::emit_wasm) realloc: u32,
  pub(in crate::wasm::emit_wasm) list_tag: i32,
  pub(in crate::wasm::emit_wasm) string_tag: i32,
}

/// Build the internal Preview 1 writer used by Calcit console operations.
/// The iovec and result word use the 16-byte scratch area below `HEAP_BASE`.
pub(in crate::wasm::emit_wasm) fn build_wasi_write_all_fn(fd_write_idx: u32) -> CompiledFn {
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
pub(in crate::wasm::emit_wasm) fn build_wasi_wait_fn(poll_oneoff_idx: u32) -> CompiledFn {
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
pub(in crate::wasm::emit_wasm) fn build_wasi_open_path_fn(
  fd_prestat_get_idx: u32,
  fd_prestat_dir_name_idx: u32,
  path_open_idx: u32,
) -> CompiledFn {
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
pub(in crate::wasm::emit_wasm) fn build_wasi_component_route_path_fn() -> CompiledFn {
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
pub(in crate::wasm::emit_wasm) fn build_wasi_component_select_preopen_fn(
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
pub(in crate::wasm::emit_wasm) fn build_wasi_component_open_at_fn(
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
pub(in crate::wasm::emit_wasm) fn build_wasi_component_read_bytes_fn(
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
pub(in crate::wasm::emit_wasm) fn build_wasi_component_write_bytes_fn(
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
pub(in crate::wasm::emit_wasm) fn build_wasi_component_read_dir_fn(
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
  b.emit(Instruction::LocalGet(string_size));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Const(8));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(managed_size));
  rt_emit_try_reserve_managed(&mut b, managed_size, reserve_status);
  b.emit(Instruction::LocalGet(reserve_status));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(valid));
  b.emit(Instruction::Else);
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
  ] {
    b.emit(instruction);
  }
  b.emit(Instruction::LocalGet(valid));
  b.emit(Instruction::If(BlockType::Empty));
  // Reserve for caller-emitted `FsPath` structs and `Result :ok` wrapper
  // against current `HEAP_PTR_GLOBAL` after any suspension during `future_read`.
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I64ExtendI32U);
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64Mul);
  b.emit(Instruction::I64Const(64));
  b.emit(Instruction::I64Add);
  b.emit(Instruction::LocalSet(managed_size));
  rt_emit_try_reserve_managed(&mut b, managed_size, reserve_status);
  b.emit(Instruction::LocalGet(reserve_status));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(list));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(list));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(list));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(0));
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

/// Suspend the current WASI 0.3 stackful command task for `nanos` nanoseconds.
/// Returns 0 on normal completion and 1 if the subtask was cancelled.
pub(in crate::wasm::emit_wasm) fn build_wasi_component_wait_fn(
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
pub(in crate::wasm::emit_wasm) fn build_wasi_component_random_get_fn(
  get_random_bytes_idx: u32,
  realloc_idx: u32,
  free_idx: u32,
) -> CompiledFn {
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
pub(in crate::wasm::emit_wasm) fn build_utf8_valid_fn() -> CompiledFn {
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
pub(in crate::wasm::emit_wasm) fn build_wasi_read_text_fn(
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
  let reserve_status = b.alloc_i32();
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
  rt_emit_try_reserve_scratch(&mut b, filestat_size, zero_size, filestat, reserve_status);
  b.emit(Instruction::LocalGet(reserve_status));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
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
  rt_emit_try_reserve_managed(&mut b, managed_size, reserve_status);
  b.emit(Instruction::LocalGet(reserve_status));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fd));
  b.emit(Instruction::Call(fd_close_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
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
pub(in crate::wasm::emit_wasm) fn build_wasi_write_text_fn(open_path_idx: u32, fd_write_idx: u32, fd_close_idx: u32) -> CompiledFn {
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

pub(in crate::wasm::emit_wasm) fn emit_wasi_dir_failure(builder: &mut RuntimeFnBuilder, fd: u32, fd_close_idx: u32) {
  builder.emit(Instruction::LocalGet(fd));
  builder.emit(Instruction::Call(fd_close_idx));
  builder.emit(Instruction::Drop);
  builder.emit(Instruction::I32Const(0));
  builder.emit(Instruction::Return);
}

/// Read a bounded Preview 1 directory into a sorted list of Calcit String pointers.
/// Returns zero on validation, authority, UTF-8, capacity, or host failures.
pub(in crate::wasm::emit_wasm) fn build_wasi_read_dir_fn(
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
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(list));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(list));
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

/// Fallibly reserve a temporary region at the top of linear memory without
/// advancing `HEAP_PTR_GLOBAL`. Sets `status_local` to 0 on success or 1 if
/// address limits or `memory.grow` fail.
pub(in crate::wasm::emit_wasm) fn rt_emit_try_reserve_scratch(
  builder: &mut RuntimeFnBuilder,
  size_local: u32,
  managed_size_local: u32,
  dst_local: u32,
  status_local: u32,
) {
  let aligned_size = builder.alloc_i64();
  let memory_bytes = builder.alloc_i64();
  let required_bytes = builder.alloc_i64();

  builder.emit(Instruction::I32Const(0));
  builder.emit(Instruction::LocalSet(status_local));
  builder.emit(Instruction::I32Const(0));
  builder.emit(Instruction::LocalSet(dst_local));
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
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::I32Const(1));
  builder.emit(Instruction::LocalSet(status_local));
  builder.emit(Instruction::Else);
  builder.emit(Instruction::MemorySize(0));
  builder.emit(Instruction::I64ExtendI32U);
  builder.emit(Instruction::I64Const(16));
  builder.emit(Instruction::I64Shl);
  builder.emit(Instruction::LocalSet(memory_bytes));
  builder.emit(Instruction::End);
  builder.emit(Instruction::End);
  builder.emit(Instruction::LocalGet(status_local));
  builder.emit(Instruction::I32Eqz);
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::LocalGet(memory_bytes));
  builder.emit(Instruction::LocalGet(aligned_size));
  builder.emit(Instruction::I64Sub);
  builder.emit(Instruction::I32WrapI64);
  builder.emit(Instruction::LocalSet(dst_local));
  builder.emit(Instruction::End);
  builder.emit(Instruction::End);
}

/// Reserve a temporary region at the top of linear memory without advancing
/// the managed bump allocator. `managed_size_local` keeps allocations made
/// while the adapter is active below the scratch region.
pub(in crate::wasm::emit_wasm) fn rt_emit_reserve_scratch(
  builder: &mut RuntimeFnBuilder,
  size_local: u32,
  managed_size_local: u32,
  dst_local: u32,
) {
  let status = builder.alloc_i32();
  rt_emit_try_reserve_scratch(builder, size_local, managed_size_local, dst_local, status);
  builder.emit(Instruction::LocalGet(status));
  rt_emit_trap_on_errno(builder);
}

/// Ensure bounded managed allocations fit in memory without trapping.
/// `status_local` is zero on success and non-zero on address overflow or a
/// failed `memory.grow`, so Result-returning adapters can fail explicitly.
pub(in crate::wasm::emit_wasm) fn rt_emit_try_reserve_managed(builder: &mut RuntimeFnBuilder, size_local: u32, status_local: u32) {
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

pub(in crate::wasm::emit_wasm) fn rt_emit_trap_on_errno(builder: &mut RuntimeFnBuilder) {
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::Unreachable);
  builder.emit(Instruction::End);
}

pub(in crate::wasm::emit_wasm) fn rt_emit_trap_if_i64_exceeds_memory32(builder: &mut RuntimeFnBuilder, value_local: u32) {
  builder.emit(Instruction::LocalGet(value_local));
  builder.emit(Instruction::I64Const(u32::MAX as i64));
  builder.emit(Instruction::I64GtU);
  builder.emit(Instruction::If(BlockType::Empty));
  builder.emit(Instruction::Unreachable);
  builder.emit(Instruction::End);
}

/// Build the Preview 1 argv adapter. The host buffers stay private while the
/// caller receives a normal tagged `List<String>` value.
pub(in crate::wasm::emit_wasm) fn build_wasi_get_args_fn(
  args_sizes_get_idx: u32,
  args_get_idx: u32,
  str_new_idx: u32,
  list_tag: i32,
) -> CompiledFn {
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
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(list));
  b.emit(Instruction::I32Const(ContainerSlotKind::ExactHeapPtr as i32));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));

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
pub(in crate::wasm::emit_wasm) fn build_wasi_get_env_fn(
  environ_sizes_get_idx: u32,
  environ_get_idx: u32,
  str_new_idx: u32,
) -> CompiledFn {
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
pub(in crate::wasm::emit_wasm) fn build_wasi_component_get_env_fn(
  environment_idx: u32,
  str_new_idx: u32,
  realloc_idx: u32,
  free_idx: u32,
) -> CompiledFn {
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
  use super::*;
  use std::collections::BTreeSet;
  use wasmtime::{Caller, Engine, Extern, Func, Instance, Module, Store, StoreLimits, StoreLimitsBuilder, Val};

  #[test]
  fn wasi_component_read_dir_rechecks_heap_capacity_after_suspension() {
    struct HostState {
      limits: StoreLimits,
      next_realloc: i32,
      allocs: BTreeSet<(i32, i32)>,
      entries_buf: i32,
      stream_wait_done: bool,
      stream_dropped: bool,
      future_dropped: bool,
      descriptor_dropped: bool,
      waitable_set_dropped: bool,
    }

    let imports = vec![
      HostImport {
        module: "test".into(),
        name: "open".into(),
        params: vec![ValType::I32; 4],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "test".into(),
        name: "read-directory".into(),
        params: vec![ValType::I32; 2],
        results: vec![],
      },
      HostImport {
        module: "test".into(),
        name: "stream-read".into(),
        params: vec![ValType::I32; 3],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "test".into(),
        name: "stream-drop".into(),
        params: vec![ValType::I32],
        results: vec![],
      },
      HostImport {
        module: "test".into(),
        name: "future-read".into(),
        params: vec![ValType::I32; 2],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "test".into(),
        name: "future-drop".into(),
        params: vec![ValType::I32],
        results: vec![],
      },
      HostImport {
        module: "test".into(),
        name: "descriptor-drop".into(),
        params: vec![ValType::I32],
        results: vec![],
      },
      HostImport {
        module: "test".into(),
        name: "free".into(),
        params: vec![ValType::I32; 2],
        results: vec![],
      },
      HostImport {
        module: "test".into(),
        name: "realloc".into(),
        params: vec![ValType::I32; 4],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "test".into(),
        name: "waitable-set-new".into(),
        params: vec![],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "test".into(),
        name: "waitable-join".into(),
        params: vec![ValType::I32; 2],
        results: vec![],
      },
      HostImport {
        module: "test".into(),
        name: "waitable-set-wait".into(),
        params: vec![ValType::I32; 2],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "test".into(),
        name: "waitable-set-drop".into(),
        params: vec![ValType::I32],
        results: vec![],
      },
    ];
    let canonical = ComponentAsyncCanonicalImports {
      waitable_set_new: 9,
      waitable_set_wait: 11,
      waitable_set_drop: 12,
      waitable_join: 10,
      subtask_drop: 0,
    };
    let utf8_valid = build_utf8_valid_fn();
    let str_compare = build_rt_str_compare();
    let mut read_dir = build_wasi_component_read_dir_fn(
      WasiComponentReadDirImports {
        open: 0,
        read_directory: 1,
        stream_read: 2,
        stream_drop: 3,
        future_read: 4,
        future_drop: 5,
        descriptor_drop: 6,
        utf8_valid: 13,
        str_compare: 14,
        free: 7,
        realloc: 8,
        list_tag: 2,
        string_tag: 1,
      },
      &canonical,
    );
    read_dir.export_name = Some("read_dir".into());

    let memory_layout = WasmMemoryLayout::compute(HEAP_BASE, 0);
    let initial_bytes = (memory_layout.initial_pages as usize) * 65_536;
    let bytes = build_wasm_module(
      &[utf8_valid, str_compare, read_dir],
      &imports,
      HEAP_BASE,
      &[],
      &[],
      1,
      ModuleFunctionLayout {
        number_format_fn_count: 0,
        memory_layout: memory_layout.clone(),
        global_layout: WasmGlobalLayout::compute(0, false),
        prune_unused_imports: false,
      },
    )
    .expect("valid component read_dir module");

    let run_case = |max_memory_bytes: usize| -> (i32, Vec<String>, bool) {
      let engine = Engine::default();
      let module = Module::new(&engine, &bytes).expect("compile component read_dir");
      let state = HostState {
        limits: StoreLimitsBuilder::new().memory_size(max_memory_bytes).build(),
        next_realloc: 2048,
        allocs: BTreeSet::new(),
        entries_buf: 0,
        stream_wait_done: false,
        stream_dropped: false,
        future_dropped: false,
        descriptor_dropped: false,
        waitable_set_dropped: false,
      };
      let mut store = Store::new(&engine, state);
      store.limiter(|s| &mut s.limits);

      let f_open = Func::wrap(
        &mut store,
        |mut c: Caller<'_, HostState>, _path: i32, _of: i32, _df: i32, out: i32| -> i32 {
          let Some(Extern::Memory(mem)) = c.get_export("memory") else {
            unreachable!()
          };
          mem.write(&mut c, out as usize, &9i32.to_le_bytes()).expect("fd");
          1
        },
      );
      let f_read_dir = Func::wrap(&mut store, |mut c: Caller<'_, HostState>, _fd: i32, pair: i32| {
        let Some(Extern::Memory(mem)) = c.get_export("memory") else {
          unreachable!()
        };
        mem.write(&mut c, pair as usize, &101i32.to_le_bytes()).expect("stream");
        mem.write(&mut c, pair as usize + 4, &202i32.to_le_bytes()).expect("future");
      });
      let f_stream_read = Func::wrap(
        &mut store,
        |mut c: Caller<'_, HostState>, _stream: i32, buf: i32, _cnt: i32| -> i32 {
          c.data_mut().entries_buf = buf;
          -1
        },
      );
      let f_stream_drop = Func::wrap(&mut store, |mut c: Caller<'_, HostState>, _stream: i32| {
        c.data_mut().stream_dropped = true;
      });
      let f_future_read = Func::wrap(&mut store, |mut c: Caller<'_, HostState>, _future: i32, res: i32| -> i32 {
        let Some(Extern::Memory(mem)) = c.get_export("memory") else {
          unreachable!()
        };
        mem.write(&mut c, res as usize, &[0u8]).expect("ok future");
        0
      });
      let f_future_drop = Func::wrap(&mut store, |mut c: Caller<'_, HostState>, _future: i32| {
        c.data_mut().future_dropped = true;
      });
      let f_desc_drop = Func::wrap(&mut store, |mut c: Caller<'_, HostState>, _fd: i32| {
        c.data_mut().descriptor_dropped = true;
      });
      let f_free = Func::wrap(&mut store, |mut c: Caller<'_, HostState>, ptr: i32, len: i32| {
        assert!(c.data_mut().allocs.remove(&(ptr, len)), "unexpected free ({ptr}, {len})");
      });
      let f_realloc = Func::wrap(
        &mut store,
        |mut c: Caller<'_, HostState>, _old: i32, _old_size: i32, align: i32, new_size: i32| -> i32 {
          let ptr = (c.data().next_realloc + align - 1) & !(align - 1);
          c.data_mut().next_realloc = ptr + new_size;
          c.data_mut().allocs.insert((ptr, new_size));
          ptr
        },
      );
      let f_ws_new = Func::wrap(&mut store, || -> i32 { 303 });
      let f_ws_join = Func::wrap(&mut store, |_w: i32, _s: i32| {});
      let f_ws_wait = Func::wrap(&mut store, |mut c: Caller<'_, HostState>, _ws: i32, event: i32| -> i32 {
        let Some(Extern::Memory(mem)) = c.get_export("memory") else {
          unreachable!()
        };
        assert!(!c.data().stream_wait_done);
        c.data_mut().stream_wait_done = true;

        // Allocate two directory entry names cleanly through tracked host storage.
        let mut alloc_name = |name: &str| -> (i32, i32) {
          let ptr = c.data().next_realloc;
          let len = name.len() as i32;
          c.data_mut().next_realloc = ptr + len;
          c.data_mut().allocs.insert((ptr, len));
          mem.write(&mut c, ptr as usize, name.as_bytes()).expect("entry name bytes");
          (ptr, len)
        };
        let (p0, l0) = alloc_name("z.txt");
        let (p1, l1) = alloc_name("a.txt");
        let buf = c.data().entries_buf as usize;
        for (idx, (p, l)) in [(p0, l0), (p1, l1)].into_iter().enumerate() {
          let base = buf + idx * 24;
          mem.write(&mut c, base, &[2u8]).expect("regular file");
          mem.write(&mut c, base + 16, &p.to_le_bytes()).expect("name ptr");
          mem.write(&mut c, base + 20, &l.to_le_bytes()).expect("name len");
        }
        mem.write(&mut c, event as usize, &101i32.to_le_bytes()).expect("event stream");
        mem
          .write(&mut c, event as usize + 4, &((2i32 << 4) | 1).to_le_bytes())
          .expect("event eof status");

        // Advance __heap_ptr during suspension so only 8 bytes remain in the initial 64 KiB page.
        let near_end = mem.data_size(&c) as i32 - 8;
        let Some(Extern::Global(heap_ptr)) = c.get_export("__heap_ptr") else {
          unreachable!()
        };
        heap_ptr.set(&mut c, Val::I32(near_end)).expect("advance heap ptr");
        2
      });
      let f_ws_drop = Func::wrap(&mut store, |mut c: Caller<'_, HostState>, _ws: i32| {
        c.data_mut().waitable_set_dropped = true;
      });

      let instance = Instance::new(
        &mut store,
        &module,
        &[
          f_open.into(),
          f_read_dir.into(),
          f_stream_read.into(),
          f_stream_drop.into(),
          f_future_read.into(),
          f_future_drop.into(),
          f_desc_drop.into(),
          f_free.into(),
          f_realloc.into(),
          f_ws_new.into(),
          f_ws_join.into(),
          f_ws_wait.into(),
          f_ws_drop.into(),
        ],
      )
      .expect("instantiate read_dir");
      let mem = instance.get_memory(&mut store, "memory").expect("memory");
      mem.write(&mut store, 1024, &9.0f64.to_le_bytes()).expect("path len");
      mem.write(&mut store, 1032, b"workspace").expect("path bytes");
      // Initial single-page capacity (65,536 bytes) vs post-suspension growth.
      let _initial_pages = mem.size(&store);
      let read_dir_fn = instance
        .get_typed_func::<i32, i32>(&mut store, "read_dir")
        .expect("read_dir export");
      let list_ptr = read_dir_fn.call(&mut store, 1024).expect("read_dir executes without trapping");

      assert!(store.data().stream_dropped);
      assert!(store.data().future_dropped);
      assert!(store.data().descriptor_dropped);
      assert!(store.data().waitable_set_dropped);
      assert!(store.data().allocs.is_empty(), "leaked allocs: {:?}", store.data().allocs);

      let mut items = Vec::new();
      if list_ptr != 0 {
        let mut count_buf = [0u8; 4];
        mem.read(&store, list_ptr as usize, &mut count_buf).expect("list count");
        let count = u32::from_le_bytes(count_buf) as usize;
        let mut buf = [0u8; 8];
        for idx in 0..count {
          mem.read(&store, list_ptr as usize + (idx + 1) * 8, &mut buf).expect("item ptr");
          let str_ptr = f64::from_le_bytes(buf) as usize;
          mem.read(&store, str_ptr, &mut buf).expect("str len");
          let len = f64::from_le_bytes(buf) as usize;
          let mut raw = vec![0u8; len];
          mem.read(&store, str_ptr + 8, &mut raw).expect("str bytes");
          items.push(String::from_utf8(raw).expect("valid utf8"));
        }
      }
      let grew = mem.size(&store) > memory_layout.initial_pages;
      (list_ptr, items, grew)
    };

    // (a) When memory can grow after suspension, re-checking capacity grows memory and returns sorted entries.
    let (ok_ptr, items, grew) = run_case(initial_bytes + 65_536);
    assert_ne!(ok_ptr, 0);
    assert!(grew, "post-suspension allocation must grow memory rather than overflowing");
    assert_eq!(items, vec!["workspace/a.txt", "workspace/z.txt"]);

    // (b) When memory growth is exhausted, post-suspension re-check returns 0 and cleans up without trapping.
    let (fail_ptr, fail_items, fail_grew) = run_case(initial_bytes);
    assert_eq!(fail_ptr, 0);
    assert!(fail_items.is_empty());
    assert!(!fail_grew);
  }

  #[test]
  fn wasi_read_text_returns_zero_and_closes_fd_when_reservation_fails() {
    struct HostState {
      limits: StoreLimits,
      file_size: i64,
      closed_fds: Vec<i32>,
      filestat_calls: usize,
      last_filestat_ptr: Option<i32>,
      fd_read_calls: usize,
    }

    let imports = vec![
      HostImport {
        module: "test".into(),
        name: "open-path".into(),
        params: vec![ValType::I32, ValType::I32, ValType::I64],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "test".into(),
        name: "fd-filestat-get".into(),
        params: vec![ValType::I32; 2],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "test".into(),
        name: "fd-read".into(),
        params: vec![ValType::I32; 4],
        results: vec![ValType::I32],
      },
      HostImport {
        module: "test".into(),
        name: "fd-close".into(),
        params: vec![ValType::I32],
        results: vec![ValType::I32],
      },
    ];
    let utf8_valid = build_utf8_valid_fn();
    let mut read_text = build_wasi_read_text_fn(0, 1, 2, 3, 4, 1);
    read_text.export_name = Some("read_text".into());

    let memory_layout = WasmMemoryLayout::compute(HEAP_BASE, 0);
    let initial_bytes = (memory_layout.initial_pages as usize) * 65_536;
    let bytes = build_wasm_module(
      &[utf8_valid, read_text],
      &imports,
      HEAP_BASE,
      &[],
      &[],
      1,
      ModuleFunctionLayout {
        number_format_fn_count: 0,
        memory_layout,
        global_layout: WasmGlobalLayout::compute(0, false),
        prune_unused_imports: false,
      },
    )
    .expect("valid read_text module");

    let run_case = |initial_heap_ptr: i32, file_size: i64| -> (i32, Vec<i32>, usize, Option<i32>, usize, Option<String>) {
      let engine = Engine::default();
      let module = Module::new(&engine, &bytes).expect("compile read_text");
      let state = HostState {
        limits: StoreLimitsBuilder::new().memory_size(initial_bytes).build(),
        file_size,
        closed_fds: Vec::new(),
        filestat_calls: 0,
        last_filestat_ptr: None,
        fd_read_calls: 0,
      };
      let mut store = Store::new(&engine, state);
      store.limiter(|s| &mut s.limits);

      let f_open = Func::wrap(&mut store, |_path: i32, _oflags: i32, _rights: i64| -> i32 { 42 });
      let f_filestat = Func::wrap(&mut store, |mut c: Caller<'_, HostState>, _fd: i32, stat_ptr: i32| -> i32 {
        c.data_mut().filestat_calls += 1;
        c.data_mut().last_filestat_ptr = Some(stat_ptr);
        let sz = c.data().file_size;
        let Some(Extern::Memory(mem)) = c.get_export("memory") else {
          unreachable!()
        };
        mem.write(&mut c, stat_ptr as usize + 32, &sz.to_le_bytes()).expect("filestat size");
        0
      });
      let f_read = Func::wrap(
        &mut store,
        |mut c: Caller<'_, HostState>, _fd: i32, iovs: i32, _cnt: i32, nread: i32| -> i32 {
          c.data_mut().fd_read_calls += 1;
          let Some(Extern::Memory(mem)) = c.get_export("memory") else {
            unreachable!()
          };
          let mut buf = [0u8; 4];
          mem.read(&c, iovs as usize, &mut buf).expect("iov base");
          let dst = u32::from_le_bytes(buf) as usize;
          mem.write(&mut c, dst, b"hello").expect("write payload");
          mem.write(&mut c, nread as usize, &5i32.to_le_bytes()).expect("nread");
          0
        },
      );
      let f_close = Func::wrap(&mut store, |mut c: Caller<'_, HostState>, fd: i32| -> i32 {
        c.data_mut().closed_fds.push(fd);
        0
      });

      let instance = Instance::new(
        &mut store,
        &module,
        &[f_open.into(), f_filestat.into(), f_read.into(), f_close.into()],
      )
      .expect("instantiate read_text");
      let mem = instance.get_memory(&mut store, "memory").expect("memory");
      mem.write(&mut store, 1024, &8.0f64.to_le_bytes()).expect("path len");
      mem.write(&mut store, 1032, b"test.txt").expect("path bytes");
      let heap_ptr = instance.get_global(&mut store, "__heap_ptr").expect("heap ptr");
      heap_ptr.set(&mut store, Val::I32(initial_heap_ptr)).expect("set initial heap ptr");

      let func = instance
        .get_typed_func::<i32, i32>(&mut store, "read_text")
        .expect("read_text export");
      let ptr = func.call(&mut store, 1024).expect("read_text must return 0 rather than trap");
      let text = if ptr != 0 {
        let mut buf = [0u8; 8];
        mem.read(&store, ptr as usize, &mut buf).expect("str len");
        let len = f64::from_le_bytes(buf) as usize;
        let mut raw = vec![0u8; len];
        mem.read(&store, ptr as usize + 8, &mut raw).expect("str bytes");
        Some(String::from_utf8(raw).expect("valid utf8"))
      } else {
        None
      };
      (
        ptr,
        store.data().closed_fds.clone(),
        store.data().filestat_calls,
        store.data().last_filestat_ptr,
        store.data().fd_read_calls,
        text,
      )
    };

    // (a) Filestat reservation exhaustion closes fd and returns 0 without trapping.
    let (ptr_a, closed_a, stat_a, stat_ptr_a, read_a, _) = run_case(initial_bytes as i32 - 16, 5);
    assert_eq!(ptr_a, 0);
    assert_eq!(closed_a, vec![42]);
    assert_eq!(stat_a, 0);
    assert_eq!(stat_ptr_a, None);
    assert_eq!(read_a, 0);

    // (b) Read-buffer reservation exhaustion closes fd and returns 0 without trapping.
    let (ptr_b, closed_b, stat_b, stat_ptr_b, read_b, _) = run_case(initial_bytes as i32 - 128, 512);
    assert_eq!(ptr_b, 0);
    assert_eq!(closed_b, vec![42]);
    assert_eq!(stat_b, 1);
    assert_eq!(stat_ptr_b, Some(initial_bytes as i32 - 64));
    assert_eq!(read_b, 0);

    // (c) Sufficient capacity succeeds and closes fd cleanly.
    let (ptr_c, closed_c, stat_c, stat_ptr_c, read_c, text_c) = run_case(HEAP_BASE, 5);
    assert_ne!(ptr_c, 0);
    assert_eq!(closed_c, vec![42]);
    assert_eq!(stat_c, 1);
    assert_eq!(stat_ptr_c, Some(initial_bytes as i32 - 64));
    assert_eq!(read_c, 1);
    assert_eq!(text_c.as_deref(), Some("hello"));
  }
}
