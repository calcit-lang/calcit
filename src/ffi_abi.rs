use std::collections::HashMap;
use std::fmt;
use std::slice;
use std::sync::{Arc, LazyLock, Mutex, Weak};

pub use calcit_native_ffi::{
  ASYNC_METHOD_SUFFIX, ASYNC_PROTOCOL_VERSION, ASYNC_PROTOCOL_VERSION_SYMBOL, BLOCKING_METHOD_SUFFIX, BUFFER_FREE_SYMBOL,
  BUFFER_METHOD_SUFFIX, BUFFER_PROTOCOL_VERSION, BUFFER_PROTOCOL_VERSION_SYMBOL, MAX_BUFFER_BYTES, RESOURCE_PROTOCOL_VERSION,
  RESOURCE_PROTOCOL_VERSION_SYMBOL, RESOURCE_RELEASE_SYMBOL, RESOURCE_TOKEN_BYTES, RESOURCE_TOKEN_FIELD, RESOURCE_TOKEN_STRUCT,
  async_method_symbol, blocking_method_symbol, buffer_method_symbol, status as async_status,
};
pub use calcit_native_ffi::{
  AsyncHostConfigure as FfiAsyncHostConfigureTask, AsyncHostEnqueue as FfiAsyncHostEnqueue,
  AsyncHostOpenResponse as FfiAsyncHostOpenResponse, AsyncMethodV1 as FfiAsyncStart, AsyncResponseResolve as FfiAsyncResponseResolve,
  AsyncTaskCancel as FfiAsyncTaskCancel, AsyncVersionFn as FfiAsyncVersion, BlockingHostFinish as FfiBlockingHostFinish,
  BlockingHostFreeBuffer as FfiBlockingHostFreeBuffer, BlockingHostInvoke as FfiBlockingHostInvoke,
  BlockingMethodV1 as FfiBlockingCall, BufferFreeFn as FfiBufferFree, BufferMethodV1 as FfiBufferCall,
  BufferVersionFn as FfiBufferVersion, CalcitFfiAsyncHostV1 as FfiAsyncHostV1, CalcitFfiAsyncTaskV1 as FfiAsyncTaskDescriptor,
  CalcitFfiBlockingHostV1 as FfiBlockingHostV1, CalcitFfiBuffer as FfiBuffer, ResourceReleaseV1 as FfiResourceRelease,
  ResourceVersionFn as FfiResourceVersion,
};
use cirru_edn::{Edn, EdnAnyRef, EdnEnumView, EdnListView, EdnMapView, EdnSetView, EdnStructView};

/// Version of the transport-neutral asynchronous task semantics. Native C ABI
/// adapters and future WASM adapters must preserve the same handle lifecycle,
/// even though their memory transports differ.
pub const ASYNC_TASK_FLAG_SERIAL_EVENTS: u32 = calcit_native_ffi::task_flags::SERIAL_EVENTS;
pub const ASYNC_TASK_FLAG_COALESCE_ALLOWED: u32 = calcit_native_ffi::task_flags::COALESCE_ALLOWED;
pub const ASYNC_TASK_FLAG_REQUIRES_RESPONSE: u32 = calcit_native_ffi::task_flags::REQUIRES_RESPONSE;
pub const ASYNC_TASK_KNOWN_FLAGS: u32 = calcit_native_ffi::task_flags::KNOWN;
pub const ASYNC_EVENT_FLAG_COALESCED: u32 = calcit_native_ffi::event_flags::COALESCED;
pub const ASYNC_EVENT_KNOWN_FLAGS: u32 = calcit_native_ffi::event_flags::KNOWN;
pub const ASYNC_RESPONSE_RESOLVE: u32 = calcit_native_ffi::response_outcome::RESOLVE;
pub const ASYNC_RESPONSE_REJECT: u32 = calcit_native_ffi::response_outcome::REJECT;

/// Semantic role of a host-owned handle. Values are fixed for C and future
/// WASM adapters; unknown raw values must be rejected before conversion.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiAsyncHandleKind {
  OneShot = calcit_native_ffi::task_kind::ONE_SHOT,
  Stream = calcit_native_ffi::task_kind::STREAM,
  Server = calcit_native_ffi::task_kind::SERVER,
  Response = calcit_native_ffi::task_kind::RESPONSE,
}

/// Stable event tags shared by native and future WASM transports. `Complete`
/// and `Fail` are terminal; `Emit` may repeat while a task is active.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiAsyncEventKind {
  Emit = calcit_native_ffi::event_kind::EMIT,
  Complete = calcit_native_ffi::event_kind::COMPLETE,
  Fail = calcit_native_ffi::event_kind::FAIL,
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
      calcit_native_ffi::event_kind::EMIT => Ok(Self::Emit),
      calcit_native_ffi::event_kind::COMPLETE => Ok(Self::Complete),
      calcit_native_ffi::event_kind::FAIL => Ok(Self::Fail),
      _ => Err(FfiAsyncHandleError::InvalidEventKind(value)),
    }
  }
}

impl TryFrom<u32> for FfiAsyncHandleKind {
  type Error = FfiAsyncHandleError;

  fn try_from(value: u32) -> Result<Self, Self::Error> {
    match value {
      calcit_native_ffi::task_kind::ONE_SHOT => Ok(Self::OneShot),
      calcit_native_ffi::task_kind::STREAM => Ok(Self::Stream),
      calcit_native_ffi::task_kind::SERVER => Ok(Self::Server),
      calcit_native_ffi::task_kind::RESPONSE => Ok(Self::Response),
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
    register_async_handle(&mut registry, state, value)
  }

  /// Register a child capability only while its owner is still active and has
  /// not queued a terminal event. Native response adapters hold their owner
  /// index lock around this call so owner completion cannot miss a new child.
  pub fn register_for_active_owner(
    &self,
    owner: FfiAsyncHandle,
    kind: FfiAsyncHandleKind,
    value: T,
  ) -> Result<FfiAsyncHandle, FfiAsyncHandleError> {
    validate_async_task_flags(kind, 0)?;
    let mut registry = self.state.lock().map_err(|_| FfiAsyncHandleError::RegistryPoisoned)?;
    if registry.closing {
      return Err(FfiAsyncHandleError::HostClosing);
    }
    let owner_state = resolve_async_handle(&registry, owner)?.state;
    match owner_state.lifecycle {
      FfiAsyncLifecycle::Active if !owner_state.terminal_queued => {}
      FfiAsyncLifecycle::Active | FfiAsyncLifecycle::Closing => return Err(FfiAsyncHandleError::HandleClosing),
      FfiAsyncLifecycle::Finished => return Err(FfiAsyncHandleError::HandleFinished),
    }
    let state = FfiAsyncHandleState {
      kind,
      flags: 0,
      lifecycle: FfiAsyncLifecycle::Active,
      next_sequence: 1,
      terminal_queued: false,
    };
    register_async_handle(&mut registry, state, value)
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

fn register_async_handle<T>(
  registry: &mut AsyncHandleRegistryState<T>,
  state: FfiAsyncHandleState,
  value: T,
) -> Result<FfiAsyncHandle, FfiAsyncHandleError> {
  while let Some(index) = registry.free_slots.pop() {
    let slot = &mut registry.slots[index];
    let Some(generation) = slot.generation.checked_add(1) else {
      // A wrapped generation could make a very old stale handle valid again,
      // so permanently retire slots that exhaust the counter.
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

pub type FfiResourceTrace = fn(&str, &str, u64, u64, i32);

struct NativeFfiResourceLease {
  lib_name: Arc<str>,
  handle: u64,
  generation: u64,
  release: FfiResourceRelease,
  trace: Option<FfiResourceTrace>,
  // Keep the creator loaded until the final Calcit reference has released its
  // resource. This remains necessary even if the process-wide dylib cache is
  // changed to permit unloading in the future.
  _library: Option<Arc<libloading::Library>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FfiResourceKey {
  lib_name: Arc<str>,
  handle: u64,
  generation: u64,
}

static RESOURCE_LEASES: LazyLock<Mutex<HashMap<FfiResourceKey, Weak<NativeFfiResourceLease>>>> =
  LazyLock::new(|| Mutex::new(HashMap::new()));

impl fmt::Debug for NativeFfiResourceLease {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("NativeFfiResourceLease")
      .field("lib_name", &self.lib_name)
      .field("handle", &self.handle)
      .field("generation", &self.generation)
      .finish_non_exhaustive()
  }
}

impl Drop for NativeFfiResourceLease {
  fn drop(&mut self) {
    let key = FfiResourceKey {
      lib_name: self.lib_name.clone(),
      handle: self.handle,
      generation: self.generation,
    };
    let leases = RESOURCE_LEASES.lock();
    // SAFETY: the function pointer belongs to `_library`, which is dropped
    // after this method returns. Resource v1 requires release to accept any
    // token and report stale/duplicate values through its status code.
    let status = unsafe { (self.release)(self.handle, self.generation) };
    if let Some(trace) = self.trace {
      trace("resource-release", &self.lib_name, self.handle, self.generation, status);
    }
    if let Ok(mut leases) = leases
      && leases.get(&key).is_some_and(|lease| lease.upgrade().is_none())
    {
      leases.remove(&key);
    }
    if status != 0 {
      eprintln!(
        "[Warn] FFI resource release failed: lib={} handle={} generation={} status={status}",
        self.lib_name, self.handle, self.generation
      );
    }
  }
}

#[derive(Clone, Debug)]
struct NativeFfiResource {
  lease: Arc<NativeFfiResourceLease>,
}

impl PartialEq for NativeFfiResource {
  fn eq(&self, other: &Self) -> bool {
    self.lease.lib_name == other.lease.lib_name
      && self.lease.handle == other.lease.handle
      && self.lease.generation == other.lease.generation
  }
}

struct FfiResourceAdapter {
  lib_name: Arc<str>,
  release: FfiResourceRelease,
  trace: Option<FfiResourceTrace>,
  library: Option<Arc<libloading::Library>>,
}

fn intern_resource_lease(adapter: &FfiResourceAdapter, handle: u64, generation: u64) -> Result<Arc<NativeFfiResourceLease>, String> {
  let key = FfiResourceKey {
    lib_name: adapter.lib_name.clone(),
    handle,
    generation,
  };
  let mut leases = RESOURCE_LEASES
    .lock()
    .map_err(|_| "failed to lock the FFI resource lease registry".to_owned())?;
  if let Some(lease) = leases.get(&key).and_then(Weak::upgrade) {
    return Ok(lease);
  }

  let lease = Arc::new(NativeFfiResourceLease {
    lib_name: adapter.lib_name.clone(),
    handle,
    generation,
    release: adapter.release,
    trace: adapter.trace,
    _library: adapter.library.clone(),
  });
  leases.insert(key, Arc::downgrade(&lease));
  if let Some(trace) = adapter.trace {
    trace("resource-create", &adapter.lib_name, handle, generation, 0);
  }
  Ok(lease)
}

/// Check whether one blocking method has completed the C-safe migration before
/// allocating host task state. An advertised incompatible protocol or a
/// migrated method without its matching module-side free function is a hard
/// error; an absent version/method returns `false` so the caller can report a
/// deterministic C-safe migration error before allocating host task state.
pub fn has_blocking_method(lib: &libloading::Library, lib_name: &str, method: &str) -> Result<bool, String> {
  let version: libloading::Symbol<FfiAsyncVersion> = match unsafe { lib.get(ASYNC_PROTOCOL_VERSION_SYMBOL) } {
    Ok(version) => version,
    Err(_) => return Ok(false),
  };
  let current_version = unsafe { version() };
  if current_version != ASYNC_PROTOCOL_VERSION {
    return Err(format!(
      "FFI async protocol mismatch in `{lib_name}`: dylib={current_version}, host={ASYNC_PROTOCOL_VERSION}"
    ));
  }
  let symbol = blocking_method_symbol(method);
  if unsafe { lib.get::<FfiBlockingCall>(symbol.as_bytes()) }.is_err() {
    return Ok(false);
  }
  unsafe { lib.get::<FfiBufferFree>(BUFFER_FREE_SYMBOL) }
    .map_err(|error| format!("FFI blocking method `{symbol}` in `{lib_name}` is missing `calcit_ffi_buffer_free`: {error}"))?;
  Ok(true)
}

pub fn encode_buffer_request(args: Vec<Edn>) -> Result<Vec<u8>, String> {
  cirru_edn::format(&Edn::List(EdnListView(args)), true)
    .map(String::into_bytes)
    .map_err(|error| format!("failed to encode FFI buffer request: {error}"))
}

fn encode_resource_token(handle: u64, generation: u64) -> Edn {
  let mut token = Vec::with_capacity(RESOURCE_TOKEN_BYTES);
  token.extend_from_slice(&handle.to_le_bytes());
  token.extend_from_slice(&generation.to_le_bytes());
  let mut value = EdnStructView::new(RESOURCE_TOKEN_STRUCT);
  value.insert(RESOURCE_TOKEN_FIELD, Edn::Buffer(token));
  value.into()
}

fn decode_resource_token(value: &EdnStructView) -> Result<(u64, u64), String> {
  if value.pairs.len() != 1 || !value.pairs[0].0.matches(RESOURCE_TOKEN_FIELD) {
    return Err(format!(
      "FFI resource token `{RESOURCE_TOKEN_STRUCT}` must contain exactly one `:{RESOURCE_TOKEN_FIELD}` field"
    ));
  }
  let Edn::Buffer(bytes) = &value.pairs[0].1 else {
    return Err(format!(
      "FFI resource token `{RESOURCE_TOKEN_STRUCT}` field `:{RESOURCE_TOKEN_FIELD}` must be a buffer"
    ));
  };
  if bytes.len() != RESOURCE_TOKEN_BYTES {
    return Err(format!(
      "FFI resource token `{RESOURCE_TOKEN_STRUCT}` must contain {RESOURCE_TOKEN_BYTES} bytes, got {}",
      bytes.len()
    ));
  }
  let handle = u64::from_le_bytes(bytes[..8].try_into().expect("resource handle byte width"));
  let generation = u64::from_le_bytes(bytes[8..].try_into().expect("resource generation byte width"));
  if handle == 0 || generation == 0 {
    return Err(format!(
      "FFI resource token `{RESOURCE_TOKEN_STRUCT}` requires non-zero handle and generation"
    ));
  }
  Ok((handle, generation))
}

fn transform_resource_args(value: &Edn, lib_name: &str) -> Result<Edn, String> {
  match value {
    Edn::AnyRef(reference) => {
      let value = reference
        .0
        .read()
        .map_err(|_| "failed to read FFI resource AnyRef because its lock is poisoned".to_owned())?;
      let Some(resource) = value.as_any().downcast_ref::<NativeFfiResource>() else {
        return Err(format!(
          "C-safe FFI buffer method in `{lib_name}` cannot serialize a non-resource AnyRef"
        ));
      };
      if resource.lease.lib_name.as_ref() != lib_name {
        return Err(format!(
          "FFI resource belongs to `{}`, but the attempted call targets `{lib_name}`",
          resource.lease.lib_name
        ));
      }
      Ok(encode_resource_token(resource.lease.handle, resource.lease.generation))
    }
    Edn::List(EdnListView(xs)) => Ok(Edn::List(EdnListView(
      xs.iter()
        .map(|item| transform_resource_args(item, lib_name))
        .collect::<Result<Vec<_>, _>>()?,
    ))),
    Edn::Set(xs) => {
      let mut output = EdnSetView::default();
      for item in &xs.0 {
        output.insert(transform_resource_args(item, lib_name)?);
      }
      Ok(output.into())
    }
    Edn::Map(xs) => {
      let mut output = EdnMapView::default();
      for (key, item) in &xs.0 {
        output.insert(transform_resource_args(key, lib_name)?, transform_resource_args(item, lib_name)?);
      }
      Ok(output.into())
    }
    Edn::Enum(EdnEnumView { variant, type_name, extra }) => Ok(Edn::Enum(EdnEnumView {
      variant: variant.clone(),
      type_name: type_name.clone(),
      extra: extra
        .iter()
        .map(|item| transform_resource_args(item, lib_name))
        .collect::<Result<Vec<_>, _>>()?,
    })),
    Edn::Struct(EdnStructView { name, pairs }) => {
      if name.as_ref() == RESOURCE_TOKEN_STRUCT {
        return Err(format!(
          "`{RESOURCE_TOKEN_STRUCT}` is reserved for host-managed FFI resources and cannot be supplied directly"
        ));
      }
      let mut output = EdnStructView::new(name.clone());
      for (key, item) in pairs {
        output.insert(key.clone(), transform_resource_args(item, lib_name)?);
      }
      Ok(output.into())
    }
    Edn::Atom(inner) => Ok(Edn::Atom(Box::new(transform_resource_args(inner, lib_name)?))),
    other => Ok(other.clone()),
  }
}

fn contains_resource_token(value: &Edn) -> bool {
  match value {
    Edn::Struct(value) if value.name.as_ref() == RESOURCE_TOKEN_STRUCT => true,
    Edn::List(xs) => xs.0.iter().any(contains_resource_token),
    Edn::Set(xs) => xs.0.iter().any(contains_resource_token),
    Edn::Map(xs) => xs
      .0
      .iter()
      .any(|(key, value)| contains_resource_token(key) || contains_resource_token(value)),
    Edn::Enum(value) => value.extra.iter().any(contains_resource_token),
    Edn::Struct(value) => value.pairs.iter().any(|(_, value)| contains_resource_token(value)),
    Edn::Atom(inner) => contains_resource_token(inner),
    _ => false,
  }
}

fn hydrate_resource_tokens(value: Edn, adapter: &FfiResourceAdapter) -> Result<Edn, String> {
  match value {
    Edn::Struct(resource) if resource.name.as_ref() == RESOURCE_TOKEN_STRUCT => {
      let (handle, generation) = decode_resource_token(&resource)?;
      Ok(Edn::AnyRef(EdnAnyRef::new(NativeFfiResource {
        lease: intern_resource_lease(adapter, handle, generation)?,
      })))
    }
    Edn::List(EdnListView(xs)) => Ok(Edn::List(EdnListView(
      xs.into_iter()
        .map(|item| hydrate_resource_tokens(item, adapter))
        .collect::<Result<Vec<_>, _>>()?,
    ))),
    Edn::Set(xs) => {
      let mut output = EdnSetView::default();
      for item in xs.0 {
        output.insert(hydrate_resource_tokens(item, adapter)?);
      }
      Ok(output.into())
    }
    Edn::Map(xs) => {
      let mut output = EdnMapView::default();
      for (key, item) in xs.0 {
        output.insert(hydrate_resource_tokens(key, adapter)?, hydrate_resource_tokens(item, adapter)?);
      }
      Ok(output.into())
    }
    Edn::Enum(EdnEnumView { variant, type_name, extra }) => Ok(Edn::Enum(EdnEnumView {
      variant,
      type_name,
      extra: extra
        .into_iter()
        .map(|item| hydrate_resource_tokens(item, adapter))
        .collect::<Result<Vec<_>, _>>()?,
    })),
    Edn::Struct(EdnStructView { name, pairs }) => {
      let mut output = EdnStructView::new(name);
      for (key, item) in pairs {
        output.insert(key, hydrate_resource_tokens(item, adapter)?);
      }
      Ok(output.into())
    }
    Edn::Atom(inner) => Ok(Edn::Atom(Box::new(hydrate_resource_tokens(*inner, adapter)?))),
    other => Ok(other),
  }
}

/// Probe a C-safe async method. A missing protocol or per-method symbol returns
/// `None` so the caller can report a deterministic migration error; an
/// advertised but incompatible version is a hard error.
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

/// Probe and invoke a C-safe blocking callback method. It shares the async
/// protocol version and task descriptor while using a distinct method symbol,
/// so blocking and asynchronous entry points cannot be confused. Missing
/// protocol or method symbols are reported by the caller as migration errors.
pub fn try_call_blocking(
  lib: &libloading::Library,
  lib_name: &str,
  method: &str,
  args: Vec<Edn>,
  task: &FfiAsyncTaskDescriptor,
  host: &FfiBlockingHostV1,
) -> Result<Option<Edn>, String> {
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

  let symbol = blocking_method_symbol(method);
  let call: libloading::Symbol<FfiBlockingCall> = match unsafe { lib.get(symbol.as_bytes()) } {
    Ok(call) => call,
    Err(_) => return Ok(None),
  };
  let free: libloading::Symbol<FfiBufferFree> = unsafe { lib.get(BUFFER_FREE_SYMBOL) }
    .map_err(|error| format!("FFI blocking method `{symbol}` in `{lib_name}` is missing `calcit_ffi_buffer_free`: {error}"))?;
  let request = encode_buffer_request(args)?;
  let mut output = FfiBuffer::empty();
  let status = unsafe { call(request.as_ptr(), request.len(), task, host, &mut output) };
  let output = unsafe { copy_and_free_buffer(output, &free, lib_name, &symbol) }?;
  decode_buffer_response(status, output, lib_name, &symbol).map(Some)
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
/// library or this particular method is missing required C-safe symbols; the
/// caller must report a deterministic migration error.
pub fn try_call_buffer(
  lib: &Arc<libloading::Library>,
  lib_name: &str,
  method: &str,
  args: Vec<Edn>,
  resource_trace: Option<FfiResourceTrace>,
) -> Result<Option<Edn>, String> {
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
  let args = args
    .iter()
    .map(|value| transform_resource_args(value, lib_name))
    .collect::<Result<Vec<_>, _>>()?;
  let request = encode_buffer_request(args)?;
  let mut output = FfiBuffer::empty();
  let status = unsafe { call(request.as_ptr(), request.len(), &mut output) };
  let output = unsafe { copy_and_free_buffer(output, &free, lib_name, &symbol) }?;
  let output = decode_buffer_response(status, output, lib_name, &symbol)?;
  if !contains_resource_token(&output) {
    return Ok(Some(output));
  }

  let version: libloading::Symbol<FfiResourceVersion> = unsafe { lib.get(RESOURCE_PROTOCOL_VERSION_SYMBOL) }.map_err(|error| {
    format!(
      "FFI buffer method `{symbol}` in `{lib_name}` returned a resource token but is missing `calcit_ffi_resource_version`: {error}"
    )
  })?;
  let current_version = unsafe { version() };
  if current_version != RESOURCE_PROTOCOL_VERSION {
    return Err(format!(
      "FFI resource protocol mismatch in `{lib_name}`: dylib={current_version}, host={RESOURCE_PROTOCOL_VERSION}"
    ));
  }
  let release: libloading::Symbol<FfiResourceRelease> = unsafe { lib.get(RESOURCE_RELEASE_SYMBOL) }.map_err(|error| {
    format!(
      "FFI buffer method `{symbol}` in `{lib_name}` returned a resource token but is missing `calcit_ffi_resource_release_v1`: {error}"
    )
  })?;
  let adapter = FfiResourceAdapter {
    lib_name: Arc::from(lib_name),
    release: *release,
    trace: resource_trace,
    library: Some(lib.clone()),
  };
  hydrate_resource_tokens(output, &adapter).map(Some)
}

#[cfg(test)]
mod tests {
  use super::{
    ASYNC_EVENT_FLAG_COALESCED, ASYNC_PROTOCOL_VERSION, ASYNC_TASK_FLAG_COALESCE_ALLOWED, ASYNC_TASK_FLAG_REQUIRES_RESPONSE,
    ASYNC_TASK_FLAG_SERIAL_EVENTS, BUFFER_PROTOCOL_VERSION, FfiAsyncEventDescriptor, FfiAsyncEventKind, FfiAsyncHandle,
    FfiAsyncHandleError, FfiAsyncHandleKind, FfiAsyncHandleRegistry, FfiAsyncHostV1, FfiAsyncLifecycle, FfiAsyncTaskDescriptor,
    FfiBlockingHostV1, FfiBuffer, FfiResourceAdapter, RESOURCE_PROTOCOL_VERSION, RESOURCE_TOKEN_BYTES, RESOURCE_TOKEN_STRUCT,
    async_method_symbol, async_status, blocking_method_symbol, buffer_method_symbol, decode_buffer_response, decode_resource_token,
    encode_buffer_request, encode_resource_token, hydrate_resource_tokens, transform_resource_args,
  };
  use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
  };

  use cirru_edn::{Edn, EdnAnyRef, EdnListView, EdnStructView};

  static RESOURCE_RELEASES: AtomicUsize = AtomicUsize::new(0);
  static RESOURCE_TEST_LOCK: Mutex<()> = Mutex::new(());

  unsafe extern "C" fn test_resource_release(_handle: u64, _generation: u64) -> i32 {
    RESOURCE_RELEASES.fetch_add(1, Ordering::SeqCst);
    0
  }

  fn test_resource_adapter() -> FfiResourceAdapter {
    FfiResourceAdapter {
      lib_name: Arc::from("demo"),
      release: test_resource_release,
      trace: None,
      library: None,
    }
  }

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

  unsafe extern "C" fn test_blocking_invoke(
    _context: u64,
    _task_handle: u64,
    _payload_ptr: *const u8,
    _payload_len: usize,
    _out: *mut FfiBuffer,
  ) -> i32 {
    async_status::OK
  }

  unsafe extern "C" fn test_blocking_finish(_context: u64, _task_handle: u64) -> i32 {
    async_status::OK
  }

  unsafe extern "C" fn test_blocking_free(_context: u64, _task_handle: u64, _buffer: FfiBuffer) -> i32 {
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
  fn blocking_host_table_and_method_names_are_c_stable() {
    let host = FfiBlockingHostV1::new(42, test_blocking_invoke, test_blocking_finish, test_blocking_free);
    assert_eq!(blocking_method_symbol("read_lines"), "read_lines_calcit_ffi_blocking_v1");
    assert_eq!(host.protocol_version, ASYNC_PROTOCOL_VERSION);
    assert_eq!(host.struct_size as usize, std::mem::size_of::<FfiBlockingHostV1>());
    assert_eq!(host.context, 42);
    assert!(host.invoke.is_some());
    assert!(host.finish.is_some());
    assert!(host.free_buffer.is_some());
  }

  #[test]
  fn buffer_requests_are_canonical_edn_lists() {
    let encoded = encode_buffer_request(vec![Edn::Number(1.0), Edn::str("two")]).expect("encode request");
    let source = std::str::from_utf8(&encoded).expect("UTF-8 request");
    let decoded = cirru_edn::parse(source).expect("parse request");
    assert_eq!(decoded, Edn::List(cirru_edn::EdnListView(vec![Edn::Number(1.0), Edn::str("two")])));
  }

  #[test]
  fn resource_tokens_use_fixed_non_zero_handle_and_generation() {
    let Edn::Struct(token) = encode_resource_token(7, 11) else {
      panic!("resource token should be a struct")
    };
    assert_eq!(token.name.as_ref(), RESOURCE_TOKEN_STRUCT);
    assert_eq!(decode_resource_token(&token).expect("valid resource token"), (7, 11));

    let mut malformed = EdnStructView::new(RESOURCE_TOKEN_STRUCT);
    malformed.insert("token", Edn::Buffer(vec![0; RESOURCE_TOKEN_BYTES]));
    let error = decode_resource_token(&malformed).expect_err("zero token must fail");
    assert!(error.contains("non-zero handle and generation"), "error: {error}");
  }

  #[test]
  fn nested_resources_round_trip_only_to_their_creator_module() {
    let _guard = RESOURCE_TEST_LOCK.lock().expect("resource test lock");
    RESOURCE_RELEASES.store(0, Ordering::SeqCst);
    let encoded = Edn::List(EdnListView(vec![Edn::str("prefix"), encode_resource_token(7, 11)]));
    let hydrated = hydrate_resource_tokens(encoded.clone(), &test_resource_adapter()).expect("hydrate resource");
    assert_eq!(
      transform_resource_args(&hydrated, "demo").expect("encode matching resource"),
      encoded
    );

    let error = transform_resource_args(&hydrated, "other").expect_err("wrong module must fail");
    assert!(error.contains("belongs to `demo`"), "error: {error}");
    drop(hydrated);
    assert_eq!(RESOURCE_RELEASES.load(Ordering::SeqCst), 1);
  }

  #[test]
  fn cloned_resources_release_once_after_concurrent_final_drop() {
    let _guard = RESOURCE_TEST_LOCK.lock().expect("resource test lock");
    RESOURCE_RELEASES.store(0, Ordering::SeqCst);
    let resource = hydrate_resource_tokens(encode_resource_token(17, 3), &test_resource_adapter()).expect("hydrate resource");
    let clones = (0..8).map(|_| resource.clone()).collect::<Vec<_>>();
    std::thread::scope(|scope| {
      for clone in clones {
        scope.spawn(move || drop(clone));
      }
    });
    assert_eq!(RESOURCE_RELEASES.load(Ordering::SeqCst), 0);
    drop(resource);
    assert_eq!(RESOURCE_RELEASES.load(Ordering::SeqCst), 1);
  }

  #[test]
  fn duplicate_tokens_share_one_lease_within_and_across_responses() {
    let _guard = RESOURCE_TEST_LOCK.lock().expect("resource test lock");
    RESOURCE_RELEASES.store(0, Ordering::SeqCst);
    let token = encode_resource_token(29, 5);
    let hydrated = hydrate_resource_tokens(Edn::List(EdnListView(vec![token.clone(), token.clone()])), &test_resource_adapter())
      .expect("hydrate duplicate tokens");
    let Edn::List(EdnListView(mut aliases)) = hydrated else {
      panic!("duplicate response should stay a list")
    };
    let first = aliases.pop().expect("first alias");
    let second = aliases.pop().expect("second alias");
    let across_response = hydrate_resource_tokens(token, &test_resource_adapter()).expect("hydrate across response");

    drop(first);
    drop(second);
    assert_eq!(RESOURCE_RELEASES.load(Ordering::SeqCst), 0);
    drop(across_response);
    assert_eq!(RESOURCE_RELEASES.load(Ordering::SeqCst), 1);
  }

  #[test]
  fn c_safe_buffer_rejects_unmanaged_any_refs_and_forged_tokens() {
    let unmanaged = Edn::AnyRef(EdnAnyRef::new(String::from("native")));
    let error = transform_resource_args(&unmanaged, "demo").expect_err("unmanaged AnyRef must fail");
    assert!(error.contains("non-resource AnyRef"), "error: {error}");

    let forged = encode_resource_token(1, 1);
    let error = transform_resource_args(&forged, "demo").expect_err("direct resource token must fail");
    assert!(error.contains("reserved for host-managed"), "error: {error}");
    assert_eq!(RESOURCE_PROTOCOL_VERSION, 1);
  }

  #[test]
  fn buffer_error_responses_require_strict_utf8() {
    let error = decode_buffer_response(1, vec![0xff], "demo", "read_calcit_ffi_v1").expect_err("invalid UTF-8 must fail");
    assert!(error.contains("non-UTF-8 error output"), "error: {error}");
  }

  #[test]
  fn async_descriptor_has_stable_version_size_and_raw_tags() {
    let registry = FfiAsyncHandleRegistry::new();
    let handle = registry
      .register(FfiAsyncHandleKind::Server, "http-server")
      .expect("register server");
    let descriptor = FfiAsyncTaskDescriptor::new(
      handle.raw(),
      FfiAsyncHandleKind::Server as u32,
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
  fn async_child_registration_requires_an_active_owner_without_terminal_event() {
    let registry = FfiAsyncHandleRegistry::new();
    let owner = registry.register(FfiAsyncHandleKind::Server, "server").expect("register owner");
    let response = registry
      .register_for_active_owner(owner, FfiAsyncHandleKind::Response, "response")
      .expect("register response for active owner");
    assert_eq!(registry.state(response).expect("response state").kind, FfiAsyncHandleKind::Response);
    assert_eq!(registry.reserve_event_sequence(owner, FfiAsyncEventKind::Complete), Ok(1));
    assert_eq!(
      registry.register_for_active_owner(owner, FfiAsyncHandleKind::Response, "late-response"),
      Err(FfiAsyncHandleError::HandleClosing)
    );
    registry.finish(owner).expect("finish owner");
    assert_eq!(
      registry.register_for_active_owner(owner, FfiAsyncHandleKind::Response, "finished-response"),
      Err(FfiAsyncHandleError::HandleFinished)
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
