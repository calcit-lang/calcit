use std::ffi::{CStr, c_char};
use std::fmt;
use std::sync::Mutex;
use std::{ptr, slice};

use cirru_edn::{Edn, EdnListView};

pub const BUILD_ID_SYMBOL: &[u8] = b"calcit_ffi_build_id";

type FfiBuildId = unsafe extern "C" fn() -> *const c_char;

pub const BUFFER_PROTOCOL_VERSION: u32 = 1;
pub const BUFFER_PROTOCOL_VERSION_SYMBOL: &[u8] = b"calcit_ffi_buffer_version";
pub const BUFFER_FREE_SYMBOL: &[u8] = b"calcit_ffi_buffer_free";
const BUFFER_METHOD_SUFFIX: &str = "_calcit_ffi_v1";
const MAX_BUFFER_BYTES: usize = 256 * 1024 * 1024;

/// Version of the transport-neutral asynchronous task semantics. Native C ABI
/// adapters and future WASM adapters must preserve the same handle lifecycle,
/// even though their memory transports differ.
pub const ASYNC_PROTOCOL_VERSION: u32 = 1;
pub const ASYNC_PROTOCOL_VERSION_SYMBOL: &[u8] = b"calcit_ffi_async_version";
pub const ASYNC_METHOD_SUFFIX: &str = "_calcit_ffi_async_v1";

pub const ASYNC_TASK_FLAG_SERIAL_EVENTS: u32 = 1 << 0;
pub const ASYNC_TASK_FLAG_COALESCE_ALLOWED: u32 = 1 << 1;
pub const ASYNC_TASK_FLAG_REQUIRES_RESPONSE: u32 = 1 << 2;
pub const ASYNC_TASK_KNOWN_FLAGS: u32 =
  ASYNC_TASK_FLAG_SERIAL_EVENTS | ASYNC_TASK_FLAG_COALESCE_ALLOWED | ASYNC_TASK_FLAG_REQUIRES_RESPONSE;
pub const ASYNC_EVENT_FLAG_COALESCED: u32 = 1 << 0;
pub const ASYNC_EVENT_KNOWN_FLAGS: u32 = ASYNC_EVENT_FLAG_COALESCED;

pub type FfiAsyncHostEnqueue = unsafe extern "C" fn(
  context: u64,
  task_handle: u64,
  event_kind: u32,
  response_handle: u64,
  payload_ptr: *const u8,
  payload_len: usize,
) -> i32;
pub type FfiAsyncTaskCancel =
  unsafe extern "C" fn(task_context: u64, task_handle: u64, reason_ptr: *const u8, reason_len: usize) -> i32;
pub type FfiAsyncResponseResolve =
  unsafe extern "C" fn(response_context: u64, response_handle: u64, outcome: u32, payload_ptr: *const u8, payload_len: usize) -> i32;
pub type FfiAsyncHostConfigureTask = unsafe extern "C" fn(
  context: u64,
  task_handle: u64,
  kind: u32,
  flags: u32,
  task_context: u64,
  cancel: Option<FfiAsyncTaskCancel>,
) -> i32;
pub type FfiAsyncHostOpenResponse = unsafe extern "C" fn(
  context: u64,
  task_handle: u64,
  response_context: u64,
  timeout_ms: u64,
  resolve: Option<FfiAsyncResponseResolve>,
  out_handle: *mut u64,
) -> i32;

pub const ASYNC_RESPONSE_RESOLVE: u32 = 1;
pub const ASYNC_RESPONSE_REJECT: u32 = 2;

/// Native host functions that an async dylib may copy and call from producer
/// threads. The integer context is opaque and avoids exposing a Rust object
/// pointer or allocator across the ABI.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiAsyncHostV1 {
  pub protocol_version: u32,
  pub struct_size: u32,
  pub context: u64,
  pub enqueue: Option<FfiAsyncHostEnqueue>,
  pub configure_task: Option<FfiAsyncHostConfigureTask>,
  pub open_response: Option<FfiAsyncHostOpenResponse>,
}

impl FfiAsyncHostV1 {
  pub fn new(
    context: u64,
    enqueue: FfiAsyncHostEnqueue,
    configure_task: FfiAsyncHostConfigureTask,
    open_response: FfiAsyncHostOpenResponse,
  ) -> Self {
    Self {
      protocol_version: ASYNC_PROTOCOL_VERSION,
      struct_size: std::mem::size_of::<Self>() as u32,
      context,
      enqueue: Some(enqueue),
      configure_task: Some(configure_task),
      open_response: Some(open_response),
    }
  }
}

/// Stable status values returned by C-safe async host functions. Keep these
/// as integer constants rather than an FFI enum so foreign callers cannot
/// construct an invalid Rust discriminant.
pub mod async_status {
  pub const OK: i32 = 0;
  pub const INVALID_HANDLE: i32 = 1;
  pub const STALE_HANDLE: i32 = 2;
  pub const HANDLE_CLOSING: i32 = 3;
  pub const HANDLE_FINISHED: i32 = 4;
  pub const HANDLE_STILL_ACTIVE: i32 = 5;
  pub const HOST_CLOSING: i32 = 6;
  pub const QUEUE_FULL: i32 = 7;
  pub const INVALID_PAYLOAD: i32 = 8;
  pub const INTERNAL_ERROR: i32 = 9;
}

/// Semantic role of a host-owned handle. Values are fixed for C and future
/// WASM adapters; unknown raw values must be rejected before conversion.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiAsyncHandleKind {
  OneShot = 1,
  Stream = 2,
  Server = 3,
  Response = 4,
}

/// Stable event tags shared by native and future WASM transports. `Complete`
/// and `Fail` are terminal; `Emit` may repeat while a task is active.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiAsyncEventKind {
  Emit = 1,
  Complete = 2,
  Fail = 3,
}

impl FfiAsyncEventKind {
  pub fn is_terminal(self) -> bool {
    matches!(self, Self::Complete | Self::Fail)
  }
}

impl TryFrom<u32> for FfiAsyncEventKind {
  type Error = FfiAsyncHandleError;

  fn try_from(value: u32) -> Result<Self, Self::Error> {
    match value {
      1 => Ok(Self::Emit),
      2 => Ok(Self::Complete),
      3 => Ok(Self::Fail),
      _ => Err(FfiAsyncHandleError::InvalidEventKind(value)),
    }
  }
}

impl TryFrom<u32> for FfiAsyncHandleKind {
  type Error = FfiAsyncHandleError;

  fn try_from(value: u32) -> Result<Self, Self::Error> {
    match value {
      1 => Ok(Self::OneShot),
      2 => Ok(Self::Stream),
      3 => Ok(Self::Server),
      4 => Ok(Self::Response),
      _ => Err(FfiAsyncHandleError::InvalidKind(value)),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiAsyncLifecycle {
  Active,
  Closing,
  Finished,
}

/// C-layout task metadata. The handle remains a raw `u64` so a WASM adapter
/// may transport it as i64 or two i32 values without exposing host pointers.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiAsyncTaskDescriptor {
  pub protocol_version: u32,
  pub struct_size: u32,
  pub handle: u64,
  pub kind: u32,
  pub flags: u32,
}

impl FfiAsyncTaskDescriptor {
  pub fn new(handle: FfiAsyncHandle, kind: FfiAsyncHandleKind, flags: u32) -> Self {
    Self {
      protocol_version: ASYNC_PROTOCOL_VERSION,
      struct_size: std::mem::size_of::<Self>() as u32,
      handle: handle.raw(),
      kind: kind as u32,
      flags,
    }
  }
}

/// C-layout event metadata. Payload bytes are deliberately not represented by
/// a Rust pointer here: the native host function table and a future WASM
/// adapter provide their own copying transport around this shared descriptor.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiAsyncEventDescriptor {
  pub protocol_version: u32,
  pub struct_size: u32,
  pub kind: u32,
  pub flags: u32,
  pub task_handle: u64,
  pub response_handle: u64,
  pub sequence: u64,
  pub payload_len: u64,
}

impl FfiAsyncEventDescriptor {
  pub fn new(
    task_handle: FfiAsyncHandle,
    response_handle: Option<FfiAsyncHandle>,
    sequence: u64,
    kind: FfiAsyncEventKind,
    flags: u32,
    payload_len: usize,
  ) -> Result<Self, FfiAsyncHandleError> {
    if flags & !ASYNC_EVENT_KNOWN_FLAGS != 0 {
      return Err(FfiAsyncHandleError::InvalidEventFlags(flags));
    }
    let payload_len = u64::try_from(payload_len).map_err(|_| FfiAsyncHandleError::PayloadTooLarge)?;
    Ok(Self {
      protocol_version: ASYNC_PROTOCOL_VERSION,
      struct_size: std::mem::size_of::<Self>() as u32,
      kind: kind as u32,
      flags,
      task_handle: task_handle.raw(),
      response_handle: response_handle.unwrap_or(FfiAsyncHandle::INVALID).raw(),
      sequence,
      payload_len,
    })
  }
}

/// Opaque generation handle. Zero is permanently invalid. The low 32 bits
/// store slot index + 1 and the high 32 bits store a non-zero generation.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FfiAsyncHandle(u64);

impl FfiAsyncHandle {
  pub const INVALID: Self = Self(0);

  pub fn from_raw(raw: u64) -> Self {
    Self(raw)
  }

  pub fn raw(self) -> u64 {
    self.0
  }

  fn from_parts(index: usize, generation: u32) -> Self {
    debug_assert!(generation != 0);
    debug_assert!(index < u32::MAX as usize);
    Self(((generation as u64) << 32) | (index as u64 + 1))
  }

  fn parts(self) -> Option<(usize, u32)> {
    let slot = self.0 as u32;
    let generation = (self.0 >> 32) as u32;
    if slot == 0 || generation == 0 {
      None
    } else {
      Some(((slot - 1) as usize, generation))
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FfiAsyncHandleError {
  InvalidKind(u32),
  InvalidEventKind(u32),
  InvalidFlags(u32),
  InvalidEventFlags(u32),
  InvalidHandle,
  StaleHandle,
  HandleClosing,
  HandleFinished,
  HandleStillActive,
  TerminalAlreadyQueued,
  TaskAlreadyStarted,
  MissingResponse,
  UnexpectedResponse,
  HostClosing,
  RegistryExhausted,
  SequenceExhausted,
  PayloadTooLarge,
  RegistryPoisoned,
}

impl FfiAsyncHandleError {
  pub fn status_code(&self) -> i32 {
    match self {
      Self::InvalidKind(_)
      | Self::InvalidEventKind(_)
      | Self::InvalidFlags(_)
      | Self::InvalidEventFlags(_)
      | Self::TaskAlreadyStarted
      | Self::MissingResponse
      | Self::UnexpectedResponse
      | Self::PayloadTooLarge => async_status::INVALID_PAYLOAD,
      Self::InvalidHandle => async_status::INVALID_HANDLE,
      Self::StaleHandle => async_status::STALE_HANDLE,
      Self::HandleClosing => async_status::HANDLE_CLOSING,
      Self::HandleFinished => async_status::HANDLE_FINISHED,
      Self::HandleStillActive => async_status::HANDLE_STILL_ACTIVE,
      Self::TerminalAlreadyQueued => async_status::HANDLE_CLOSING,
      Self::HostClosing => async_status::HOST_CLOSING,
      Self::RegistryExhausted | Self::SequenceExhausted | Self::RegistryPoisoned => async_status::INTERNAL_ERROR,
    }
  }
}

impl fmt::Display for FfiAsyncHandleError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::InvalidKind(value) => write!(f, "invalid async FFI handle kind {value}"),
      Self::InvalidEventKind(value) => write!(f, "invalid async FFI event kind {value}"),
      Self::InvalidFlags(value) => write!(f, "invalid async FFI task flags 0x{value:x}"),
      Self::InvalidEventFlags(value) => write!(f, "invalid async FFI event flags 0x{value:x}"),
      Self::InvalidHandle => f.write_str("invalid async FFI handle"),
      Self::StaleHandle => f.write_str("stale async FFI handle generation"),
      Self::HandleClosing => f.write_str("async FFI handle is closing"),
      Self::HandleFinished => f.write_str("async FFI handle is already finished"),
      Self::HandleStillActive => f.write_str("async FFI handle must finish before release"),
      Self::TerminalAlreadyQueued => f.write_str("async FFI handle already has a terminal event queued"),
      Self::TaskAlreadyStarted => f.write_str("async FFI task cannot be configured after its first event"),
      Self::MissingResponse => f.write_str("async FFI server event requires a response handle"),
      Self::UnexpectedResponse => f.write_str("async FFI response handle is not valid for this event"),
      Self::HostClosing => f.write_str("async FFI host is closing"),
      Self::RegistryExhausted => f.write_str("async FFI handle registry is exhausted"),
      Self::SequenceExhausted => f.write_str("async FFI event sequence is exhausted"),
      Self::PayloadTooLarge => f.write_str("async FFI event payload length is too large"),
      Self::RegistryPoisoned => f.write_str("async FFI handle registry lock is poisoned"),
    }
  }
}

impl std::error::Error for FfiAsyncHandleError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiAsyncHandleState {
  pub kind: FfiAsyncHandleKind,
  pub flags: u32,
  pub lifecycle: FfiAsyncLifecycle,
  pub next_sequence: u64,
  pub terminal_queued: bool,
}

struct RegisteredAsyncHandle<T> {
  state: FfiAsyncHandleState,
  value: T,
}

struct AsyncHandleSlot<T> {
  generation: u32,
  task: Option<RegisteredAsyncHandle<T>>,
}

struct AsyncHandleRegistryState<T> {
  slots: Vec<AsyncHandleSlot<T>>,
  free_slots: Vec<usize>,
  closing: bool,
}

/// Thread-safe lifecycle registry shared by one-shot tasks, streams, servers,
/// and one-use response capabilities. It intentionally contains no executor
/// or Rust callback object, so native and WASM transports can share its rules.
pub struct FfiAsyncHandleRegistry<T> {
  state: Mutex<AsyncHandleRegistryState<T>>,
}

impl<T> Default for FfiAsyncHandleRegistry<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> FfiAsyncHandleRegistry<T> {
  pub fn new() -> Self {
    Self {
      state: Mutex::new(AsyncHandleRegistryState {
        slots: vec![],
        free_slots: vec![],
        closing: false,
      }),
    }
  }

  pub fn register(&self, kind: FfiAsyncHandleKind, value: T) -> Result<FfiAsyncHandle, FfiAsyncHandleError> {
    self.register_with_flags(kind, 0, value)
  }

  pub fn register_with_flags(&self, kind: FfiAsyncHandleKind, flags: u32, value: T) -> Result<FfiAsyncHandle, FfiAsyncHandleError> {
    validate_async_task_flags(kind, flags)?;
    let mut registry = self.state.lock().map_err(|_| FfiAsyncHandleError::RegistryPoisoned)?;
    if registry.closing {
      return Err(FfiAsyncHandleError::HostClosing);
    }

    let state = FfiAsyncHandleState {
      kind,
      flags,
      lifecycle: FfiAsyncLifecycle::Active,
      next_sequence: 1,
      terminal_queued: false,
    };
    while let Some(index) = registry.free_slots.pop() {
      let slot = &mut registry.slots[index];
      let Some(generation) = slot.generation.checked_add(1) else {
        // A wrapped generation could make a very old stale handle valid
        // again, so permanently retire slots that exhaust the counter.
        continue;
      };
      slot.generation = generation;
      slot.task = Some(RegisteredAsyncHandle { state, value });
      return Ok(FfiAsyncHandle::from_parts(index, slot.generation));
    }

    if registry.slots.len() >= u32::MAX as usize {
      return Err(FfiAsyncHandleError::RegistryExhausted);
    }
    let index = registry.slots.len();
    registry.slots.push(AsyncHandleSlot {
      generation: 1,
      task: Some(RegisteredAsyncHandle { state, value }),
    });
    Ok(FfiAsyncHandle::from_parts(index, 1))
  }

  pub fn state(&self, handle: FfiAsyncHandle) -> Result<FfiAsyncHandleState, FfiAsyncHandleError> {
    let registry = self.state.lock().map_err(|_| FfiAsyncHandleError::RegistryPoisoned)?;
    Ok(resolve_async_handle(&registry, handle)?.state)
  }

  /// Configure a task before its first event. Async start functions use this
  /// to specialize the host's default Stream descriptor into OneShot or
  /// Server and to declare response/coalescing policy.
  pub fn configure(&self, handle: FfiAsyncHandle, kind: FfiAsyncHandleKind, flags: u32) -> Result<(), FfiAsyncHandleError> {
    validate_async_task_flags(kind, flags)?;
    if kind == FfiAsyncHandleKind::Response {
      return Err(FfiAsyncHandleError::InvalidKind(kind as u32));
    }
    let mut registry = self.state.lock().map_err(|_| FfiAsyncHandleError::RegistryPoisoned)?;
    let task = resolve_async_handle_mut(&mut registry, handle)?;
    if task.state.lifecycle != FfiAsyncLifecycle::Active || task.state.next_sequence != 1 || task.state.terminal_queued {
      return Err(FfiAsyncHandleError::TaskAlreadyStarted);
    }
    task.state.kind = kind;
    task.state.flags = flags;
    Ok(())
  }

  /// Clone host-owned metadata/callback state without holding the registry
  /// lock while user code runs. The lifecycle is still checked separately by
  /// event reservation and terminal transitions.
  pub fn clone_value(&self, handle: FfiAsyncHandle) -> Result<T, FfiAsyncHandleError>
  where
    T: Clone,
  {
    let registry = self.state.lock().map_err(|_| FfiAsyncHandleError::RegistryPoisoned)?;
    Ok(resolve_async_handle(&registry, handle)?.value.clone())
  }

  /// Reserve the next host sequence before an event is enqueued. Events are
  /// rejected as soon as cancellation/shutdown moves a handle to Closing.
  pub fn next_event_sequence(&self, handle: FfiAsyncHandle) -> Result<u64, FfiAsyncHandleError> {
    self.reserve_event_sequence(handle, FfiAsyncEventKind::Emit)
  }

  /// Reserve an event sequence and atomically claim a terminal event when
  /// needed. Queue implementations call this only after capacity is secured,
  /// so a full queue does not leave a phantom terminal transition behind.
  pub fn reserve_event_sequence(&self, handle: FfiAsyncHandle, kind: FfiAsyncEventKind) -> Result<u64, FfiAsyncHandleError> {
    let mut registry = self.state.lock().map_err(|_| FfiAsyncHandleError::RegistryPoisoned)?;
    let task = resolve_async_handle_mut(&mut registry, handle)?;
    let sequence = task.state.next_sequence;
    let next_sequence = sequence.checked_add(1).ok_or(FfiAsyncHandleError::SequenceExhausted)?;
    if kind.is_terminal() {
      if task.state.lifecycle == FfiAsyncLifecycle::Finished {
        return Err(FfiAsyncHandleError::HandleFinished);
      }
      if task.state.terminal_queued {
        return Err(FfiAsyncHandleError::TerminalAlreadyQueued);
      }
      task.state.terminal_queued = true;
    } else {
      match task.state.lifecycle {
        FfiAsyncLifecycle::Active => {}
        FfiAsyncLifecycle::Closing => return Err(FfiAsyncHandleError::HandleClosing),
        FfiAsyncLifecycle::Finished => return Err(FfiAsyncHandleError::HandleFinished),
      }
    }

    task.state.next_sequence = next_sequence;
    Ok(sequence)
  }

  /// Begin cancellation or orderly close. Completion must still be
  /// acknowledged through `finish` before the handle can be released.
  pub fn begin_close(&self, handle: FfiAsyncHandle) -> Result<(), FfiAsyncHandleError> {
    let mut registry = self.state.lock().map_err(|_| FfiAsyncHandleError::RegistryPoisoned)?;
    let task = resolve_async_handle_mut(&mut registry, handle)?;
    match task.state.lifecycle {
      FfiAsyncLifecycle::Active => {
        task.state.lifecycle = FfiAsyncLifecycle::Closing;
        Ok(())
      }
      FfiAsyncLifecycle::Closing => Err(FfiAsyncHandleError::HandleClosing),
      FfiAsyncLifecycle::Finished => Err(FfiAsyncHandleError::HandleFinished),
    }
  }

  /// Mark completion exactly once. The tombstone remains until `release`, so
  /// duplicate completion is distinguishable from an unknown handle.
  pub fn finish(&self, handle: FfiAsyncHandle) -> Result<(), FfiAsyncHandleError> {
    let mut registry = self.state.lock().map_err(|_| FfiAsyncHandleError::RegistryPoisoned)?;
    let task = resolve_async_handle_mut(&mut registry, handle)?;
    match task.state.lifecycle {
      FfiAsyncLifecycle::Active | FfiAsyncLifecycle::Closing => {
        task.state.lifecycle = FfiAsyncLifecycle::Finished;
        Ok(())
      }
      FfiAsyncLifecycle::Finished => Err(FfiAsyncHandleError::HandleFinished),
    }
  }

  pub fn release(&self, handle: FfiAsyncHandle) -> Result<T, FfiAsyncHandleError> {
    let mut registry = self.state.lock().map_err(|_| FfiAsyncHandleError::RegistryPoisoned)?;
    let (index, generation) = handle.parts().ok_or(FfiAsyncHandleError::InvalidHandle)?;
    let task = {
      let slot = registry.slots.get_mut(index).ok_or(FfiAsyncHandleError::InvalidHandle)?;
      if slot.generation != generation || slot.task.is_none() {
        return Err(FfiAsyncHandleError::StaleHandle);
      }
      if slot
        .task
        .as_ref()
        .is_some_and(|task| task.state.lifecycle != FfiAsyncLifecycle::Finished)
      {
        return Err(FfiAsyncHandleError::HandleStillActive);
      }
      slot.task.take().ok_or(FfiAsyncHandleError::StaleHandle)?
    };
    registry.free_slots.push(index);
    Ok(task.value)
  }

  /// Stop new registrations and move every active handle into Closing. The
  /// returned handles are the tasks the host must cancel or diagnose.
  pub fn begin_shutdown(&self) -> Result<Vec<FfiAsyncHandle>, FfiAsyncHandleError> {
    let mut registry = self.state.lock().map_err(|_| FfiAsyncHandleError::RegistryPoisoned)?;
    registry.closing = true;
    let mut pending = vec![];
    for (index, slot) in registry.slots.iter_mut().enumerate() {
      if let Some(task) = &mut slot.task {
        match task.state.lifecycle {
          FfiAsyncLifecycle::Active => task.state.lifecycle = FfiAsyncLifecycle::Closing,
          FfiAsyncLifecycle::Closing => {}
          FfiAsyncLifecycle::Finished => continue,
        }
        pending.push(FfiAsyncHandle::from_parts(index, slot.generation));
      }
    }
    Ok(pending)
  }

  pub fn pending_count(&self) -> Result<usize, FfiAsyncHandleError> {
    let registry = self.state.lock().map_err(|_| FfiAsyncHandleError::RegistryPoisoned)?;
    Ok(
      registry
        .slots
        .iter()
        .filter(|slot| {
          slot
            .task
            .as_ref()
            .is_some_and(|task| task.state.lifecycle != FfiAsyncLifecycle::Finished)
        })
        .count(),
    )
  }

  /// Snapshot registered handles without holding the registry lock while the
  /// host invokes module control functions or performs timeout processing.
  pub fn snapshot(&self) -> Result<Vec<(FfiAsyncHandle, FfiAsyncHandleState, T)>, FfiAsyncHandleError>
  where
    T: Clone,
  {
    let registry = self.state.lock().map_err(|_| FfiAsyncHandleError::RegistryPoisoned)?;
    let mut values = vec![];
    for (index, slot) in registry.slots.iter().enumerate() {
      if let Some(task) = &slot.task {
        values.push((FfiAsyncHandle::from_parts(index, slot.generation), task.state, task.value.clone()));
      }
    }
    Ok(values)
  }
}

fn validate_async_task_flags(kind: FfiAsyncHandleKind, flags: u32) -> Result<(), FfiAsyncHandleError> {
  if flags & !ASYNC_TASK_KNOWN_FLAGS != 0
    || (flags & ASYNC_TASK_FLAG_COALESCE_ALLOWED != 0 && kind != FfiAsyncHandleKind::Stream)
    || (flags & ASYNC_TASK_FLAG_REQUIRES_RESPONSE != 0 && kind != FfiAsyncHandleKind::Server)
  {
    Err(FfiAsyncHandleError::InvalidFlags(flags))
  } else {
    Ok(())
  }
}

fn resolve_async_handle<T>(
  registry: &AsyncHandleRegistryState<T>,
  handle: FfiAsyncHandle,
) -> Result<&RegisteredAsyncHandle<T>, FfiAsyncHandleError> {
  let (index, generation) = handle.parts().ok_or(FfiAsyncHandleError::InvalidHandle)?;
  let slot = registry.slots.get(index).ok_or(FfiAsyncHandleError::InvalidHandle)?;
  if slot.generation != generation {
    return Err(FfiAsyncHandleError::StaleHandle);
  }
  slot.task.as_ref().ok_or(FfiAsyncHandleError::StaleHandle)
}

fn resolve_async_handle_mut<T>(
  registry: &mut AsyncHandleRegistryState<T>,
  handle: FfiAsyncHandle,
) -> Result<&mut RegisteredAsyncHandle<T>, FfiAsyncHandleError> {
  let (index, generation) = handle.parts().ok_or(FfiAsyncHandleError::InvalidHandle)?;
  let slot = registry.slots.get_mut(index).ok_or(FfiAsyncHandleError::InvalidHandle)?;
  if slot.generation != generation {
    return Err(FfiAsyncHandleError::StaleHandle);
  }
  slot.task.as_mut().ok_or(FfiAsyncHandleError::StaleHandle)
}

/// Owned bytes allocated by an FFI module. The module must release this value
/// through `calcit_ffi_buffer_free`; the host only copies from it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiBuffer {
  pub ptr: *mut u8,
  pub len: usize,
  pub cap: usize,
}

impl FfiBuffer {
  fn empty() -> Self {
    Self {
      ptr: ptr::null_mut(),
      len: 0,
      cap: 0,
    }
  }
}

type FfiBufferVersion = unsafe extern "C" fn() -> u32;
type FfiBufferFree = unsafe extern "C" fn(FfiBuffer);
type FfiBufferCall = unsafe extern "C" fn(*const u8, usize, *mut FfiBuffer) -> i32;
type FfiAsyncVersion = unsafe extern "C" fn() -> u32;
pub type FfiAsyncStart = unsafe extern "C" fn(
  request_ptr: *const u8,
  request_len: usize,
  task: *const FfiAsyncTaskDescriptor,
  host: *const FfiAsyncHostV1,
) -> i32;

#[derive(Debug, PartialEq, Eq)]
pub enum FfiBuildCompatibility {
  Exact,
  Legacy,
}

fn buffer_method_symbol(method: &str) -> String {
  format!("{method}{BUFFER_METHOD_SUFFIX}")
}

pub fn async_method_symbol(method: &str) -> String {
  format!("{method}{ASYNC_METHOD_SUFFIX}")
}

pub fn encode_buffer_request(args: Vec<Edn>) -> Result<Vec<u8>, String> {
  cirru_edn::format(&Edn::List(EdnListView(args)), true)
    .map(String::into_bytes)
    .map_err(|error| format!("failed to encode FFI buffer request: {error}"))
}

/// Probe a C-safe async method without touching the guarded Rust ABI. A
/// missing protocol or per-method symbol returns `None` for transitional
/// fallback; an advertised but incompatible version is a hard error.
pub fn lookup_async_start<'lib>(
  lib: &'lib libloading::Library,
  lib_name: &str,
  method: &str,
) -> Result<Option<libloading::Symbol<'lib, FfiAsyncStart>>, String> {
  let version: libloading::Symbol<FfiAsyncVersion> = match unsafe { lib.get(ASYNC_PROTOCOL_VERSION_SYMBOL) } {
    Ok(version) => version,
    Err(_) => return Ok(None),
  };
  let current_version = unsafe { version() };
  if current_version != ASYNC_PROTOCOL_VERSION {
    return Err(format!(
      "FFI async protocol mismatch in `{lib_name}`: dylib={current_version}, host={ASYNC_PROTOCOL_VERSION}"
    ));
  }

  let symbol = async_method_symbol(method);
  match unsafe { lib.get(symbol.as_bytes()) } {
    Ok(start) => Ok(Some(start)),
    Err(_) => Ok(None),
  }
}

fn decode_buffer_response(status: i32, output: Vec<u8>, lib_name: &str, symbol: &str) -> Result<Edn, String> {
  if status == 0 {
    let source = std::str::from_utf8(&output)
      .map_err(|error| format!("FFI buffer method `{symbol}` in `{lib_name}` returned non-UTF-8 EDN: {error}"))?;
    cirru_edn::parse(source)
      .map_err(|error| format!("FFI buffer method `{symbol}` in `{lib_name}` returned invalid Cirru EDN: {error}"))
  } else {
    let message = String::from_utf8(output)
      .map_err(|error| format!("FFI buffer method `{symbol}` in `{lib_name}` returned non-UTF-8 error output: {error}"))?;
    Err(format!(
      "FFI buffer method `{symbol}` in `{lib_name}` failed with status {status}: {message}"
    ))
  }
}

unsafe fn copy_and_free_buffer(
  buffer: FfiBuffer,
  free: &libloading::Symbol<FfiBufferFree>,
  lib_name: &str,
  symbol: &str,
) -> Result<Vec<u8>, String> {
  if buffer.len > buffer.cap {
    return Err(format!(
      "FFI buffer `{symbol}` in `{lib_name}` returned len {} larger than capacity {}",
      buffer.len, buffer.cap
    ));
  }
  if buffer.len > MAX_BUFFER_BYTES {
    return Err(format!(
      "FFI buffer `{symbol}` in `{lib_name}` returned {} bytes, exceeding the {} byte safety limit",
      buffer.len, MAX_BUFFER_BYTES
    ));
  }
  if buffer.ptr.is_null() && (buffer.len != 0 || buffer.cap != 0) {
    return Err(format!(
      "FFI buffer `{symbol}` in `{lib_name}` returned a null pointer with len {} and capacity {}",
      buffer.len, buffer.cap
    ));
  }

  let copied = if buffer.len == 0 {
    Vec::new()
  } else {
    // SAFETY: the protocol requires `ptr` to reference `len` initialized bytes
    // until the module's matching free function is called below.
    unsafe { slice::from_raw_parts(buffer.ptr.cast_const(), buffer.len) }.to_vec()
  };
  // SAFETY: ownership stays with the module that created the buffer.
  unsafe { free(buffer) };
  Ok(copied)
}

/// Try the C-safe synchronous byte-buffer protocol. `Ok(None)` means the
/// library or this particular method has not migrated and may use the guarded
/// legacy Rust ABI path.
pub fn try_call_buffer(lib: &libloading::Library, lib_name: &str, method: &str, args: Vec<Edn>) -> Result<Option<Edn>, String> {
  let version: libloading::Symbol<FfiBufferVersion> = match unsafe { lib.get(BUFFER_PROTOCOL_VERSION_SYMBOL) } {
    Ok(version) => version,
    Err(_) => return Ok(None),
  };
  let current_version = unsafe { version() };
  if current_version != BUFFER_PROTOCOL_VERSION {
    return Err(format!(
      "FFI buffer protocol mismatch in `{lib_name}`: dylib={current_version}, host={BUFFER_PROTOCOL_VERSION}"
    ));
  }

  let symbol = buffer_method_symbol(method);
  let call: libloading::Symbol<FfiBufferCall> = match unsafe { lib.get(symbol.as_bytes()) } {
    Ok(call) => call,
    Err(_) => return Ok(None),
  };
  let free: libloading::Symbol<FfiBufferFree> = unsafe { lib.get(BUFFER_FREE_SYMBOL) }
    .map_err(|error| format!("FFI buffer method `{symbol}` in `{lib_name}` is missing `calcit_ffi_buffer_free`: {error}"))?;
  let request = encode_buffer_request(args)?;
  let mut output = FfiBuffer::empty();
  let status = unsafe { call(request.as_ptr(), request.len(), &mut output) };
  let output = unsafe { copy_and_free_buffer(output, &free, lib_name, &symbol) }?;

  decode_buffer_response(status, output, lib_name, &symbol).map(Some)
}

pub fn validate_build_id(
  lib_name: &str,
  dylib_build_id: Option<&str>,
  host_build_id: &str,
  require_build_id: bool,
) -> Result<FfiBuildCompatibility, String> {
  match dylib_build_id {
    Some(dylib_build_id) if dylib_build_id == host_build_id => Ok(FfiBuildCompatibility::Exact),
    Some(dylib_build_id) => Err(format!(
      "Refusing Rust-native FFI library `{lib_name}` before invoking a Rust ABI symbol because its build identity differs from the Calcit host. dylib: `{dylib_build_id}`; host: `{host_build_id}`. Rebuild both with the same rustc, target, debug-assertion mode, and panic strategy."
    )),
    None if require_build_id => Err(format!(
      "Refusing legacy Rust-native FFI library `{lib_name}` before invoking a Rust ABI symbol: this debug Calcit host cannot prove that the dylib uses a compatible build. Export the C-safe `calcit_ffi_build_id` symbol described in the FFI guide, or run a release Calcit host built with the same toolchain as the dylib. Expected host identity: `{host_build_id}`."
    )),
    None => Ok(FfiBuildCompatibility::Legacy),
  }
}

/// Read the optional static C build identity without invoking a Rust ABI symbol.
pub fn lookup_build_id(lib: &libloading::Library, lib_name: &str) -> Result<Option<String>, String> {
  let lookup: libloading::Symbol<FfiBuildId> = match unsafe { lib.get(BUILD_ID_SYMBOL) } {
    Ok(lookup) => lookup,
    Err(_) => return Ok(None),
  };
  let ptr = unsafe { lookup() };
  if ptr.is_null() {
    return Err(format!(
      "FFI library `{lib_name}` returned a null pointer from `calcit_ffi_build_id`"
    ));
  }
  let value = unsafe { CStr::from_ptr(ptr) }
    .to_str()
    .map_err(|error| format!("FFI library `{lib_name}` returned invalid UTF-8 from `calcit_ffi_build_id`: {error}"))?;
  Ok(Some(value.to_owned()))
}

#[cfg(test)]
mod tests {
  use super::{
    ASYNC_EVENT_FLAG_COALESCED, ASYNC_PROTOCOL_VERSION, ASYNC_TASK_FLAG_COALESCE_ALLOWED, ASYNC_TASK_FLAG_REQUIRES_RESPONSE,
    ASYNC_TASK_FLAG_SERIAL_EVENTS, BUFFER_PROTOCOL_VERSION, FfiAsyncEventDescriptor, FfiAsyncEventKind, FfiAsyncHandle,
    FfiAsyncHandleError, FfiAsyncHandleKind, FfiAsyncHandleRegistry, FfiAsyncHostV1, FfiAsyncLifecycle, FfiAsyncTaskDescriptor,
    FfiBuildCompatibility, async_method_symbol, async_status, buffer_method_symbol, decode_buffer_response, encode_buffer_request,
    validate_build_id,
  };
  use cirru_edn::Edn;

  #[test]
  fn buffer_method_names_are_versioned_without_changing_source_calls() {
    assert_eq!(buffer_method_symbol("run_wat"), "run_wat_calcit_ffi_v1");
    assert_eq!(BUFFER_PROTOCOL_VERSION, 1);
  }

  unsafe extern "C" fn test_enqueue(
    _context: u64,
    _task_handle: u64,
    _event_kind: u32,
    _response_handle: u64,
    _payload_ptr: *const u8,
    _payload_len: usize,
  ) -> i32 {
    async_status::OK
  }

  unsafe extern "C" fn test_configure(
    _context: u64,
    _task_handle: u64,
    _kind: u32,
    _flags: u32,
    _task_context: u64,
    _cancel: Option<super::FfiAsyncTaskCancel>,
  ) -> i32 {
    async_status::OK
  }

  unsafe extern "C" fn test_open_response(
    _context: u64,
    _task_handle: u64,
    _response_context: u64,
    _timeout_ms: u64,
    _resolve: Option<super::FfiAsyncResponseResolve>,
    _out_handle: *mut u64,
  ) -> i32 {
    async_status::OK
  }

  #[test]
  fn async_host_table_and_method_names_are_c_stable() {
    let host = FfiAsyncHostV1::new(42, test_enqueue, test_configure, test_open_response);
    assert_eq!(async_method_symbol("watch"), "watch_calcit_ffi_async_v1");
    assert_eq!(host.protocol_version, ASYNC_PROTOCOL_VERSION);
    assert_eq!(host.struct_size as usize, std::mem::size_of::<FfiAsyncHostV1>());
    assert_eq!(host.context, 42);
    assert!(host.enqueue.is_some());
    assert!(host.configure_task.is_some());
    assert!(host.open_response.is_some());
  }

  #[test]
  fn buffer_requests_are_canonical_edn_lists() {
    let encoded = encode_buffer_request(vec![Edn::Number(1.0), Edn::str("two")]).expect("encode request");
    let source = std::str::from_utf8(&encoded).expect("UTF-8 request");
    let decoded = cirru_edn::parse(source).expect("parse request");
    assert_eq!(decoded, Edn::List(cirru_edn::EdnListView(vec![Edn::Number(1.0), Edn::str("two")])));
  }

  #[test]
  fn buffer_error_responses_require_strict_utf8() {
    let error = decode_buffer_response(1, vec![0xff], "demo", "read_calcit_ffi_v1").expect_err("invalid UTF-8 must fail");
    assert!(error.contains("non-UTF-8 error output"), "error: {error}");
  }

  #[test]
  fn exact_identity_is_accepted() {
    assert_eq!(
      validate_build_id("demo", Some("same-build"), "same-build", true).expect("exact identity should pass"),
      FfiBuildCompatibility::Exact
    );
  }

  #[test]
  fn mismatched_identity_is_rejected_before_rust_abi_calls() {
    let error = validate_build_id("demo", Some("release-build"), "debug-build", false).expect_err("different identities must fail");
    assert!(error.contains("before invoking a Rust ABI symbol"), "error: {error}");
    assert!(error.contains("release-build"), "error: {error}");
    assert!(error.contains("debug-build"), "error: {error}");
  }

  #[test]
  fn debug_hosts_reject_legacy_dylibs_before_rust_abi_calls() {
    let error = validate_build_id("demo", None, "debug-build", true).expect_err("debug host must require build identity");
    assert!(error.contains("Refusing legacy Rust-native FFI library"), "error: {error}");
    assert!(error.contains("calcit_ffi_build_id"), "error: {error}");
  }

  #[test]
  fn release_hosts_keep_a_temporary_legacy_path() {
    assert_eq!(
      validate_build_id("demo", None, "release-build", false).expect("release compatibility path should remain"),
      FfiBuildCompatibility::Legacy
    );
  }

  #[test]
  fn async_descriptor_has_stable_version_size_and_raw_tags() {
    let registry = FfiAsyncHandleRegistry::new();
    let handle = registry
      .register(FfiAsyncHandleKind::Server, "http-server")
      .expect("register server");
    let descriptor = FfiAsyncTaskDescriptor::new(
      handle,
      FfiAsyncHandleKind::Server,
      ASYNC_TASK_FLAG_SERIAL_EVENTS | ASYNC_TASK_FLAG_REQUIRES_RESPONSE,
    );

    assert_eq!(descriptor.protocol_version, ASYNC_PROTOCOL_VERSION);
    assert_eq!(descriptor.struct_size as usize, std::mem::size_of::<FfiAsyncTaskDescriptor>());
    assert_eq!(descriptor.handle, handle.raw());
    assert_eq!(descriptor.kind, FfiAsyncHandleKind::Server as u32);
    assert_eq!(FfiAsyncHandleKind::try_from(descriptor.kind), Ok(FfiAsyncHandleKind::Server));
    assert_eq!(FfiAsyncHandleKind::try_from(99), Err(FfiAsyncHandleError::InvalidKind(99)));

    let event = FfiAsyncEventDescriptor::new(handle, None, 7, FfiAsyncEventKind::Emit, ASYNC_EVENT_FLAG_COALESCED, 12)
      .expect("create event descriptor");
    assert_eq!(event.protocol_version, ASYNC_PROTOCOL_VERSION);
    assert_eq!(event.struct_size as usize, std::mem::size_of::<FfiAsyncEventDescriptor>());
    assert_eq!(event.task_handle, handle.raw());
    assert_eq!(event.response_handle, FfiAsyncHandle::INVALID.raw());
    assert_eq!(event.sequence, 7);
    assert_eq!(event.payload_len, 12);
    assert_eq!(FfiAsyncEventKind::try_from(event.kind), Ok(FfiAsyncEventKind::Emit));
    assert_eq!(FfiAsyncEventKind::try_from(99), Err(FfiAsyncHandleError::InvalidEventKind(99)));
    assert_eq!(
      FfiAsyncEventDescriptor::new(handle, None, 8, FfiAsyncEventKind::Emit, 1 << 31, 0),
      Err(FfiAsyncHandleError::InvalidEventFlags(1 << 31))
    );
    assert_eq!(
      registry.register_with_flags(FfiAsyncHandleKind::Server, ASYNC_TASK_FLAG_COALESCE_ALLOWED, "invalid-server"),
      Err(FfiAsyncHandleError::InvalidFlags(ASYNC_TASK_FLAG_COALESCE_ALLOWED))
    );
  }

  #[test]
  fn async_registry_orders_events_and_finishes_exactly_once() {
    let registry = FfiAsyncHandleRegistry::new();
    let handle = registry.register(FfiAsyncHandleKind::Stream, "watcher").expect("register watcher");

    assert_eq!(registry.next_event_sequence(handle), Ok(1));
    assert_eq!(registry.next_event_sequence(handle), Ok(2));
    assert_eq!(registry.finish(handle), Ok(()));
    assert_eq!(registry.finish(handle), Err(FfiAsyncHandleError::HandleFinished));
    assert_eq!(registry.next_event_sequence(handle), Err(FfiAsyncHandleError::HandleFinished));
    assert_eq!(registry.release(handle), Ok("watcher"));
    assert_eq!(registry.release(handle), Err(FfiAsyncHandleError::StaleHandle));
  }

  #[test]
  fn async_task_configuration_is_pre_event_and_kind_aware() {
    let registry = FfiAsyncHandleRegistry::new();
    let handle = registry
      .register_with_flags(FfiAsyncHandleKind::Stream, ASYNC_TASK_FLAG_SERIAL_EVENTS, "server")
      .expect("register provisional task");
    registry
      .configure(
        handle,
        FfiAsyncHandleKind::Server,
        ASYNC_TASK_FLAG_SERIAL_EVENTS | ASYNC_TASK_FLAG_REQUIRES_RESPONSE,
      )
      .expect("configure server");
    let state = registry.state(handle).expect("configured state");
    assert_eq!(state.kind, FfiAsyncHandleKind::Server);
    assert_eq!(state.flags, ASYNC_TASK_FLAG_SERIAL_EVENTS | ASYNC_TASK_FLAG_REQUIRES_RESPONSE);
    assert_eq!(registry.next_event_sequence(handle), Ok(1));
    assert_eq!(
      registry.configure(handle, FfiAsyncHandleKind::OneShot, ASYNC_TASK_FLAG_SERIAL_EVENTS),
      Err(FfiAsyncHandleError::TaskAlreadyStarted)
    );
    assert_eq!(
      registry.register_with_flags(
        FfiAsyncHandleKind::Stream,
        ASYNC_TASK_FLAG_SERIAL_EVENTS | ASYNC_TASK_FLAG_REQUIRES_RESPONSE,
        "invalid-stream"
      ),
      Err(FfiAsyncHandleError::InvalidFlags(
        ASYNC_TASK_FLAG_SERIAL_EVENTS | ASYNC_TASK_FLAG_REQUIRES_RESPONSE
      ))
    );
  }

  #[test]
  fn async_registry_snapshot_preserves_handle_kind_and_value() {
    let registry = FfiAsyncHandleRegistry::new();
    let task = registry.register(FfiAsyncHandleKind::Server, "server").expect("register server");
    let response = registry
      .register(FfiAsyncHandleKind::Response, "response")
      .expect("register response");
    let snapshot = registry.snapshot().expect("snapshot handles");
    assert_eq!(snapshot.len(), 2);
    assert!(snapshot.contains(&(task, registry.state(task).expect("task state"), "server")));
    assert!(snapshot.contains(&(response, registry.state(response).expect("response state"), "response")));
  }

  #[test]
  fn async_registry_rejects_events_after_cancel_but_allows_finish() {
    let registry = FfiAsyncHandleRegistry::new();
    let handle = registry.register(FfiAsyncHandleKind::OneShot, "timer").expect("register timer");

    assert_eq!(registry.begin_close(handle), Ok(()));
    assert_eq!(registry.begin_close(handle), Err(FfiAsyncHandleError::HandleClosing));
    assert_eq!(registry.next_event_sequence(handle), Err(FfiAsyncHandleError::HandleClosing));
    assert_eq!(registry.finish(handle), Ok(()));
    assert_eq!(registry.release(handle), Ok("timer"));
  }

  #[test]
  fn async_registry_generation_rejects_reused_stale_handles() {
    let registry = FfiAsyncHandleRegistry::new();
    let first = registry
      .register(FfiAsyncHandleKind::Response, "first-response")
      .expect("register first response");
    registry.finish(first).expect("finish first response");
    assert_eq!(registry.release(first), Ok("first-response"));

    let second = registry
      .register(FfiAsyncHandleKind::Response, "second-response")
      .expect("reuse response slot");
    assert_ne!(first, second);
    assert_eq!(registry.state(first), Err(FfiAsyncHandleError::StaleHandle));
    assert_eq!(
      registry.state(second).expect("current response").lifecycle,
      FfiAsyncLifecycle::Active
    );
  }

  #[test]
  fn async_registry_retires_exhausted_generation_slots() {
    let registry = FfiAsyncHandleRegistry::new();
    let first = registry
      .register(FfiAsyncHandleKind::Response, "first-response")
      .expect("register first response");
    registry.finish(first).expect("finish first response");
    assert_eq!(registry.release(first), Ok("first-response"));

    registry.state.lock().expect("registry lock").slots[0].generation = u32::MAX;

    let second = registry
      .register(FfiAsyncHandleKind::Response, "second-response")
      .expect("register after exhausted slot");
    assert_eq!(second.parts().map(|(index, _)| index), Some(1));
    assert_eq!(registry.state(first), Err(FfiAsyncHandleError::StaleHandle));
  }

  #[test]
  fn async_shutdown_closes_pending_handles_and_rejects_new_tasks() {
    let registry = FfiAsyncHandleRegistry::new();
    let timer = registry.register(FfiAsyncHandleKind::OneShot, "timer").expect("register timer");
    let server = registry.register(FfiAsyncHandleKind::Server, "server").expect("register server");
    let completed = registry
      .register(FfiAsyncHandleKind::OneShot, "completed")
      .expect("register completed task");
    registry.finish(completed).expect("finish completed task");

    let pending = registry.begin_shutdown().expect("start shutdown");
    assert_eq!(pending, vec![timer, server]);
    assert_eq!(registry.pending_count(), Ok(2));
    assert_eq!(registry.state(timer).expect("timer state").lifecycle, FfiAsyncLifecycle::Closing);
    assert_eq!(registry.state(server).expect("server state").lifecycle, FfiAsyncLifecycle::Closing);
    assert_eq!(
      registry.register(FfiAsyncHandleKind::Stream, "late watcher"),
      Err(FfiAsyncHandleError::HostClosing)
    );
  }

  #[test]
  fn async_invalid_handles_have_stable_status_codes() {
    let registry = FfiAsyncHandleRegistry::<()>::new();
    let error = registry
      .state(FfiAsyncHandle::INVALID)
      .expect_err("zero handle must remain invalid");
    assert_eq!(error, FfiAsyncHandleError::InvalidHandle);
    assert_eq!(error.status_code(), async_status::INVALID_HANDLE);
    assert_eq!(FfiAsyncHandleError::StaleHandle.status_code(), async_status::STALE_HANDLE);
  }
}
