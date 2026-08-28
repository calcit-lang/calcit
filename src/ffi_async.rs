//! Host-side scheduling primitives for asynchronous FFI events.
//!
//! Foreign producers may enqueue from any thread, but only the thread that
//! creates the queue may drain it and enter the Calcit runtime. No Rust
//! callback, executor, or allocator crosses the transport boundary.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::sync::{Condvar, Mutex};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

use crate::ffi_abi::{
  ASYNC_EVENT_FLAG_COALESCED, ASYNC_TASK_FLAG_COALESCE_ALLOWED, FfiAsyncEventDescriptor, FfiAsyncEventKind, FfiAsyncHandle,
  FfiAsyncHandleError, FfiAsyncHandleRegistry, FfiAsyncLifecycle, async_status,
};

/// Protect the host from a single event consuming unbounded memory before the
/// transport-specific EDN decoder can validate it.
pub const MAX_ASYNC_EVENT_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiAsyncQueueLimits {
  pub event_capacity: usize,
  pub byte_capacity: usize,
  pub terminal_event_reserve: usize,
  pub terminal_byte_reserve: usize,
}

impl FfiAsyncQueueLimits {
  pub const fn event_only(event_capacity: usize) -> Self {
    Self {
      event_capacity,
      byte_capacity: usize::MAX,
      terminal_event_reserve: 0,
      terminal_byte_reserve: 0,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiAsyncEnqueueDisposition {
  Enqueued,
  Coalesced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiAsyncEnqueueOutcome {
  pub sequence: u64,
  pub disposition: FfiAsyncEnqueueDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// A point-in-time snapshot of one async task's queue usage and cumulative
/// enqueue outcomes. Payload bytes are counted but never copied into metrics.
pub struct FfiAsyncTaskQueueMetrics {
  pub queued_events: usize,
  pub queued_bytes: usize,
  pub oldest_age: Option<Duration>,
  pub accepted_total: u64,
  pub coalesced_total: u64,
  pub queue_full_total: u64,
  pub dequeued_total: u64,
  pub purged_total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FfiAsyncQueueError {
  InvalidCapacity,
  InvalidByteCapacity,
  InvalidTerminalReserve,
  NullPayload,
  PayloadTooLarge { actual: usize, limit: usize },
  QueueClosed,
  QueueFull { capacity: usize },
  QueueBytesFull { capacity: usize, queued: usize, incoming: usize },
  WrongDrainThread,
  QueuePoisoned,
  Handle(FfiAsyncHandleError),
}

impl FfiAsyncQueueError {
  pub fn status_code(&self) -> i32 {
    match self {
      Self::NullPayload | Self::PayloadTooLarge { .. } => async_status::INVALID_PAYLOAD,
      Self::QueueClosed => async_status::HOST_CLOSING,
      Self::QueueFull { .. } | Self::QueueBytesFull { .. } => async_status::QUEUE_FULL,
      Self::Handle(error) => error.status_code(),
      Self::InvalidCapacity
      | Self::InvalidByteCapacity
      | Self::InvalidTerminalReserve
      | Self::WrongDrainThread
      | Self::QueuePoisoned => async_status::INTERNAL_ERROR,
    }
  }
}

impl fmt::Display for FfiAsyncQueueError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::InvalidCapacity => f.write_str("async FFI event queue capacity must be greater than zero"),
      Self::InvalidByteCapacity => f.write_str("async FFI event queue byte capacity must be greater than zero"),
      Self::InvalidTerminalReserve => f.write_str("async FFI terminal reserve must not exceed the queue capacity"),
      Self::NullPayload => f.write_str("async FFI event payload pointer is null for a non-empty payload"),
      Self::PayloadTooLarge { actual, limit } => {
        write!(f, "async FFI event payload has {actual} bytes, exceeding the {limit}-byte limit")
      }
      Self::QueueClosed => f.write_str("async FFI event queue is closed"),
      Self::QueueFull { capacity } => write!(f, "async FFI event queue is full at capacity {capacity}"),
      Self::QueueBytesFull {
        capacity,
        queued,
        incoming,
      } => write!(
        f,
        "async FFI event queue byte budget is full: queued={queued}, incoming={incoming}, capacity={capacity}"
      ),
      Self::WrongDrainThread => f.write_str("async FFI event queue must be drained by its host thread"),
      Self::QueuePoisoned => f.write_str("async FFI event queue lock is poisoned"),
      Self::Handle(error) => error.fmt(f),
    }
  }
}

impl std::error::Error for FfiAsyncQueueError {}

impl From<FfiAsyncHandleError> for FfiAsyncQueueError {
  fn from(value: FfiAsyncHandleError) -> Self {
    Self::Handle(value)
  }
}

#[derive(Debug, Clone)]
pub struct FfiAsyncQueuedEvent {
  pub descriptor: FfiAsyncEventDescriptor,
  payload: Vec<u8>,
  producer_thread: ThreadId,
  enqueued_at: Instant,
}

impl FfiAsyncQueuedEvent {
  pub fn payload(&self) -> &[u8] {
    &self.payload
  }

  pub fn task_handle(&self) -> FfiAsyncHandle {
    FfiAsyncHandle::from_raw(self.descriptor.task_handle)
  }

  pub fn kind(&self) -> Result<FfiAsyncEventKind, FfiAsyncHandleError> {
    FfiAsyncEventKind::try_from(self.descriptor.kind)
  }

  pub fn producer_thread(&self) -> ThreadId {
    self.producer_thread
  }

  pub fn queued_for(&self) -> Duration {
    self.enqueued_at.elapsed()
  }
}

#[derive(Debug, Clone)]
pub struct FfiAsyncDispatchFailure {
  pub descriptor: FfiAsyncEventDescriptor,
  pub message: String,
}

#[derive(Debug, Clone)]
pub struct FfiAsyncLifecycleFailure {
  pub descriptor: FfiAsyncEventDescriptor,
  pub error: FfiAsyncHandleError,
}

#[derive(Debug, Clone)]
pub struct FfiAsyncQueueFailure {
  pub descriptor: FfiAsyncEventDescriptor,
  pub error: FfiAsyncQueueError,
}

#[derive(Debug, Default)]
pub struct FfiAsyncDrainReport {
  pub dequeued: usize,
  pub delivered: usize,
  pub discarded: usize,
  pub purged: usize,
  pub callback_failures: Vec<FfiAsyncDispatchFailure>,
  pub lifecycle_failures: Vec<FfiAsyncLifecycleFailure>,
  pub queue_failures: Vec<FfiAsyncQueueFailure>,
  pub finished: Vec<FfiAsyncEventDescriptor>,
}

/// Validate and copy a foreign payload before it enters the host queue. A null
/// pointer is accepted only for an empty payload. As with every C ABI, a
/// non-null pointer must remain readable for the declared length during this
/// call; the host cannot validate arbitrary dangling addresses.
///
/// # Safety
///
/// For a non-zero `payload_len`, `payload_ptr` must point to at least that many
/// initialized, readable bytes for the duration of this call.
pub unsafe fn copy_async_payload(payload_ptr: *const u8, payload_len: usize) -> Result<Vec<u8>, FfiAsyncQueueError> {
  if payload_len > MAX_ASYNC_EVENT_PAYLOAD_BYTES {
    return Err(FfiAsyncQueueError::PayloadTooLarge {
      actual: payload_len,
      limit: MAX_ASYNC_EVENT_PAYLOAD_BYTES,
    });
  }
  if payload_len == 0 {
    return Ok(vec![]);
  }
  if payload_ptr.is_null() {
    return Err(FfiAsyncQueueError::NullPayload);
  }
  // SAFETY: the caller upholds readability for `payload_len`; bounds and null
  // were checked above, and the bytes are copied before returning across FFI.
  Ok(unsafe { std::slice::from_raw_parts(payload_ptr, payload_len) }.to_vec())
}

struct FfiAsyncQueueState {
  events: VecDeque<FfiAsyncQueuedEvent>,
  queued_bytes: usize,
  tasks: HashMap<FfiAsyncHandle, FfiAsyncTaskQueueState>,
  closed: bool,
}

#[derive(Debug, Clone)]
struct FfiAsyncQueuedSample {
  sequence: u64,
  bytes: usize,
  enqueued_at: Instant,
}

#[derive(Debug, Default)]
struct FfiAsyncTaskQueueState {
  queued: VecDeque<FfiAsyncQueuedSample>,
  queued_bytes: usize,
  accepted_total: u64,
  coalesced_total: u64,
  queue_full_total: u64,
  dequeued_total: u64,
  purged_total: u64,
}

impl FfiAsyncTaskQueueState {
  fn snapshot(&self, now: Instant) -> FfiAsyncTaskQueueMetrics {
    FfiAsyncTaskQueueMetrics {
      queued_events: self.queued.len(),
      queued_bytes: self.queued_bytes,
      oldest_age: self.queued.front().map(|sample| now.saturating_duration_since(sample.enqueued_at)),
      accepted_total: self.accepted_total,
      coalesced_total: self.coalesced_total,
      queue_full_total: self.queue_full_total,
      dequeued_total: self.dequeued_total,
      purged_total: self.purged_total,
    }
  }

  fn record_enqueued(&mut self, sequence: u64, bytes: usize, enqueued_at: Instant) {
    self.accepted_total = self.accepted_total.saturating_add(1);
    self.queued_bytes = self.queued_bytes.saturating_add(bytes);
    self.queued.push_back(FfiAsyncQueuedSample {
      sequence,
      bytes,
      enqueued_at,
    });
  }

  fn record_queue_full(&mut self) {
    self.queue_full_total = self.queue_full_total.saturating_add(1);
  }

  fn record_coalesced(&mut self, replaced_sequence: u64, sequence: u64, bytes: usize, enqueued_at: Instant) {
    self.accepted_total = self.accepted_total.saturating_add(1);
    self.coalesced_total = self.coalesced_total.saturating_add(1);
    let replaced = if let Some(sample) = self.queued.iter_mut().find(|sample| sample.sequence == replaced_sequence) {
      self.queued_bytes = self.queued_bytes.saturating_sub(sample.bytes).saturating_add(bytes);
      *sample = FfiAsyncQueuedSample {
        sequence,
        bytes,
        enqueued_at,
      };
      true
    } else {
      false
    };
    debug_assert!(replaced, "coalesced async event must have a matching task metric sample");
  }

  fn record_dequeued(&mut self, sequence: u64) {
    self.dequeued_total = self.dequeued_total.saturating_add(1);
    if self.queued.front().is_some_and(|sample| sample.sequence == sequence)
      && let Some(sample) = self.queued.pop_front()
    {
      self.queued_bytes = self.queued_bytes.saturating_sub(sample.bytes);
      return;
    }
    let removed = if let Some(index) = self.queued.iter().position(|sample| sample.sequence == sequence)
      && let Some(sample) = self.queued.remove(index)
    {
      self.queued_bytes = self.queued_bytes.saturating_sub(sample.bytes);
      true
    } else {
      false
    };
    debug_assert!(removed, "dequeued async event must have a matching task metric sample");
  }

  fn record_purged(&mut self, count: usize) {
    debug_assert_eq!(self.queued.len(), count, "purged async event count must match task metric samples");
    self.purged_total = self.purged_total.saturating_add(count as u64);
    self.queued.clear();
    self.queued_bytes = 0;
  }
}

/// A bounded multi-producer, single-host-thread event queue.
pub struct FfiAsyncEventQueue {
  limits: FfiAsyncQueueLimits,
  host_thread: ThreadId,
  state: Mutex<FfiAsyncQueueState>,
  ready: Condvar,
}

impl FfiAsyncEventQueue {
  pub fn new(capacity: usize) -> Result<Self, FfiAsyncQueueError> {
    Self::with_limits(FfiAsyncQueueLimits::event_only(capacity))
  }

  pub fn with_limits(limits: FfiAsyncQueueLimits) -> Result<Self, FfiAsyncQueueError> {
    if limits.event_capacity == 0 {
      return Err(FfiAsyncQueueError::InvalidCapacity);
    }
    if limits.byte_capacity == 0 {
      return Err(FfiAsyncQueueError::InvalidByteCapacity);
    }
    if limits.terminal_event_reserve > limits.event_capacity || limits.terminal_byte_reserve > limits.byte_capacity {
      return Err(FfiAsyncQueueError::InvalidTerminalReserve);
    }
    Ok(Self {
      limits,
      host_thread: thread::current().id(),
      state: Mutex::new(FfiAsyncQueueState {
        events: VecDeque::with_capacity(limits.event_capacity),
        queued_bytes: 0,
        tasks: HashMap::new(),
        closed: false,
      }),
      ready: Condvar::new(),
    })
  }

  pub fn capacity(&self) -> usize {
    self.limits.event_capacity
  }

  pub fn byte_capacity(&self) -> usize {
    self.limits.byte_capacity
  }

  pub fn queued_bytes(&self) -> Result<usize, FfiAsyncQueueError> {
    Ok(self.state.lock().map_err(|_| FfiAsyncQueueError::QueuePoisoned)?.queued_bytes)
  }

  pub fn usage(&self) -> Result<(usize, usize), FfiAsyncQueueError> {
    let queue = self.state.lock().map_err(|_| FfiAsyncQueueError::QueuePoisoned)?;
    Ok((queue.events.len(), queue.queued_bytes))
  }

  /// Snapshot metrics for one task without changing their lifecycle.
  pub fn task_metrics(&self, handle: FfiAsyncHandle) -> Result<Option<FfiAsyncTaskQueueMetrics>, FfiAsyncQueueError> {
    let queue = self.state.lock().map_err(|_| FfiAsyncQueueError::QueuePoisoned)?;
    let now = Instant::now();
    Ok(queue.tasks.get(&handle).map(|state| state.snapshot(now)))
  }

  /// Snapshot every tracked task. This is intended for diagnostics such as
  /// shutdown summaries rather than event-queue hot paths.
  pub fn task_metrics_snapshot(&self) -> Result<Vec<(FfiAsyncHandle, FfiAsyncTaskQueueMetrics)>, FfiAsyncQueueError> {
    let queue = self.state.lock().map_err(|_| FfiAsyncQueueError::QueuePoisoned)?;
    let now = Instant::now();
    Ok(queue.tasks.iter().map(|(handle, state)| (*handle, state.snapshot(now))).collect())
  }

  /// Remove and return a task's final metrics after its queued events have
  /// drained or been purged.
  pub fn take_task_metrics(&self, handle: FfiAsyncHandle) -> Result<Option<FfiAsyncTaskQueueMetrics>, FfiAsyncQueueError> {
    let mut queue = self.state.lock().map_err(|_| FfiAsyncQueueError::QueuePoisoned)?;
    let now = Instant::now();
    Ok(queue.tasks.remove(&handle).map(|state| {
      debug_assert!(
        state.queued.is_empty(),
        "task metrics must only be removed after queued events are cleared"
      );
      state.snapshot(now)
    }))
  }

  pub fn len(&self) -> Result<usize, FfiAsyncQueueError> {
    Ok(self.state.lock().map_err(|_| FfiAsyncQueueError::QueuePoisoned)?.events.len())
  }

  pub fn is_empty(&self) -> Result<bool, FfiAsyncQueueError> {
    Ok(self.len()? == 0)
  }

  pub fn close(&self) -> Result<(), FfiAsyncQueueError> {
    let mut queue = self.state.lock().map_err(|_| FfiAsyncQueueError::QueuePoisoned)?;
    queue.closed = true;
    self.ready.notify_all();
    Ok(())
  }

  /// Remove queued work for a task that failed during startup or was otherwise
  /// reclaimed before normal dispatch.
  pub fn discard_handle_events(&self, handle: FfiAsyncHandle) -> Result<usize, FfiAsyncQueueError> {
    self.purge_handle(handle)
  }

  /// Enqueue an event without blocking a foreign producer. When the queue is
  /// full, only ordinary `Emit` events from a task that explicitly allows
  /// coalescing may replace an older queued emit for the same task.
  pub fn enqueue<T>(
    &self,
    registry: &FfiAsyncHandleRegistry<T>,
    task_handle: FfiAsyncHandle,
    response_handle: Option<FfiAsyncHandle>,
    kind: FfiAsyncEventKind,
    payload: Vec<u8>,
  ) -> Result<FfiAsyncEnqueueOutcome, FfiAsyncQueueError> {
    if payload.len() > MAX_ASYNC_EVENT_PAYLOAD_BYTES {
      return Err(FfiAsyncQueueError::PayloadTooLarge {
        actual: payload.len(),
        limit: MAX_ASYNC_EVENT_PAYLOAD_BYTES,
      });
    }
    if response_handle.is_some_and(|handle| handle == FfiAsyncHandle::INVALID) {
      return Err(FfiAsyncHandleError::InvalidHandle.into());
    }

    let mut queue = self.state.lock().map_err(|_| FfiAsyncQueueError::QueuePoisoned)?;
    if queue.closed {
      return Err(FfiAsyncQueueError::QueueClosed);
    }

    let task_state = registry.state(task_handle)?;
    if kind == FfiAsyncEventKind::Emit && task_state.flags & crate::ffi_abi::ASYNC_TASK_FLAG_REQUIRES_RESPONSE != 0 {
      let response_handle = response_handle.ok_or(FfiAsyncHandleError::MissingResponse)?;
      let response_state = registry.state(response_handle)?;
      if response_state.kind != crate::ffi_abi::FfiAsyncHandleKind::Response || response_state.lifecycle != FfiAsyncLifecycle::Active {
        return Err(FfiAsyncHandleError::UnexpectedResponse.into());
      }
    } else if response_handle.is_some() {
      return Err(FfiAsyncHandleError::UnexpectedResponse.into());
    }
    let terminal = kind.is_terminal();
    let event_limit = if terminal {
      self.limits.event_capacity
    } else {
      self.limits.event_capacity - self.limits.terminal_event_reserve
    };
    let byte_limit = if terminal {
      self.limits.byte_capacity
    } else {
      self.limits.byte_capacity - self.limits.terminal_byte_reserve
    };
    let exceeds_event_limit = queue.events.len() >= event_limit;
    let exceeds_byte_limit = queue.queued_bytes.checked_add(payload.len()).is_none_or(|total| total > byte_limit);
    let coalesce_index = if (exceeds_event_limit || exceeds_byte_limit)
      && kind == FfiAsyncEventKind::Emit
      && task_state.flags & ASYNC_TASK_FLAG_COALESCE_ALLOWED != 0
    {
      queue.events.iter().rposition(|queued| {
        queued.descriptor.task_handle == task_handle.raw() && queued.descriptor.kind == FfiAsyncEventKind::Emit as u32
      })
    } else {
      None
    };

    if coalesce_index.is_none() {
      if exceeds_event_limit {
        queue.tasks.entry(task_handle).or_default().record_queue_full();
        return Err(FfiAsyncQueueError::QueueFull { capacity: event_limit });
      }
      if exceeds_byte_limit {
        queue.tasks.entry(task_handle).or_default().record_queue_full();
        return Err(FfiAsyncQueueError::QueueBytesFull {
          capacity: byte_limit,
          queued: queue.queued_bytes,
          incoming: payload.len(),
        });
      }
    } else if let Some(index) = coalesce_index {
      let replaced_bytes = queue.events[index].payload.len();
      let coalesced_bytes = queue.queued_bytes.saturating_sub(replaced_bytes).checked_add(payload.len());
      if coalesced_bytes.is_none_or(|total| total > byte_limit) {
        queue.tasks.entry(task_handle).or_default().record_queue_full();
        return Err(FfiAsyncQueueError::QueueBytesFull {
          capacity: byte_limit,
          queued: queue.queued_bytes.saturating_sub(replaced_bytes),
          incoming: payload.len(),
        });
      }
    }

    let sequence = registry.reserve_event_sequence(task_handle, kind)?;
    let descriptor_flags = if coalesce_index.is_some() { ASYNC_EVENT_FLAG_COALESCED } else { 0 };
    let descriptor = FfiAsyncEventDescriptor::new(task_handle, response_handle, sequence, kind, descriptor_flags, payload.len())?;
    let event = FfiAsyncQueuedEvent {
      descriptor,
      payload,
      producer_thread: thread::current().id(),
      enqueued_at: Instant::now(),
    };
    let enqueued_at = event.enqueued_at;

    let disposition = if let Some(index) = coalesce_index {
      let replaced_sequence = queue.events[index].descriptor.sequence;
      queue.queued_bytes = queue.queued_bytes.saturating_sub(queue.events[index].payload.len()) + event.payload.len();
      queue
        .tasks
        .entry(task_handle)
        .or_default()
        .record_coalesced(replaced_sequence, sequence, event.payload.len(), enqueued_at);
      queue.events[index] = event;
      FfiAsyncEnqueueDisposition::Coalesced
    } else {
      queue.queued_bytes += event.payload.len();
      queue
        .tasks
        .entry(task_handle)
        .or_default()
        .record_enqueued(sequence, event.payload.len(), enqueued_at);
      queue.events.push_back(event);
      FfiAsyncEnqueueDisposition::Enqueued
    };
    self.ready.notify_one();
    Ok(FfiAsyncEnqueueOutcome { sequence, disposition })
  }

  /// Wait until work arrives, the queue closes, or the timeout expires. This
  /// is a host-loop primitive and therefore follows the same thread rule as
  /// `drain`.
  pub fn wait_for_event(&self, timeout: Duration) -> Result<bool, FfiAsyncQueueError> {
    self.ensure_host_thread()?;
    let queue = self.state.lock().map_err(|_| FfiAsyncQueueError::QueuePoisoned)?;
    if !queue.events.is_empty() {
      return Ok(true);
    }
    if queue.closed {
      return Ok(false);
    }
    let (queue, _) = self
      .ready
      .wait_timeout_while(queue, timeout, |state| state.events.is_empty() && !state.closed)
      .map_err(|_| FfiAsyncQueueError::QueuePoisoned)?;
    Ok(!queue.events.is_empty())
  }

  /// Drain at most `limit` events on the queue's host thread. Dispatch errors
  /// are returned in the report, transition the task to `Finished`, and purge
  /// its remaining queued events so failures are never silently lost or
  /// followed by callbacks on a failed task.
  pub fn drain<T, F>(
    &self,
    registry: &FfiAsyncHandleRegistry<T>,
    limit: usize,
    mut dispatch: F,
  ) -> Result<FfiAsyncDrainReport, FfiAsyncQueueError>
  where
    F: FnMut(&FfiAsyncQueuedEvent) -> Result<(), String>,
  {
    self.ensure_host_thread()?;
    if limit == 0 {
      return Ok(FfiAsyncDrainReport::default());
    }

    let mut batch = Vec::with_capacity(limit.min(self.limits.event_capacity));
    {
      let mut queue = self.state.lock().map_err(|_| FfiAsyncQueueError::QueuePoisoned)?;
      for _ in 0..limit {
        let Some(event) = queue.events.pop_front() else {
          break;
        };
        queue.queued_bytes = queue.queued_bytes.saturating_sub(event.payload.len());
        if let Some(state) = queue.tasks.get_mut(&event.task_handle()) {
          state.record_dequeued(event.descriptor.sequence);
        }
        batch.push(event);
      }
    }

    let mut report = FfiAsyncDrainReport {
      dequeued: batch.len(),
      ..FfiAsyncDrainReport::default()
    };
    let mut failed_handles = HashSet::new();

    for event in batch {
      let handle = event.task_handle();
      if failed_handles.contains(&handle) {
        report.discarded += 1;
        continue;
      }

      let kind = match event.kind() {
        Ok(kind) => kind,
        Err(error) => {
          report.lifecycle_failures.push(FfiAsyncLifecycleFailure {
            descriptor: event.descriptor,
            error,
          });
          report.discarded += 1;
          continue;
        }
      };
      let state = match registry.state(handle) {
        Ok(state) => state,
        Err(error) => {
          report.lifecycle_failures.push(FfiAsyncLifecycleFailure {
            descriptor: event.descriptor,
            error,
          });
          report.discarded += 1;
          continue;
        }
      };
      if state.lifecycle == FfiAsyncLifecycle::Finished
        || (state.lifecycle == FfiAsyncLifecycle::Closing && kind == FfiAsyncEventKind::Emit)
      {
        report.lifecycle_failures.push(FfiAsyncLifecycleFailure {
          descriptor: event.descriptor,
          error: if state.lifecycle == FfiAsyncLifecycle::Finished {
            FfiAsyncHandleError::HandleFinished
          } else {
            FfiAsyncHandleError::HandleClosing
          },
        });
        report.discarded += 1;
        continue;
      }

      match dispatch(&event) {
        Ok(()) => {
          report.delivered += 1;
          if kind.is_terminal()
            && let Err(error) = registry.finish(handle)
          {
            report.lifecycle_failures.push(FfiAsyncLifecycleFailure {
              descriptor: event.descriptor,
              error,
            });
          } else if kind.is_terminal() {
            report.finished.push(event.descriptor);
          }
        }
        Err(message) => {
          report.callback_failures.push(FfiAsyncDispatchFailure {
            descriptor: event.descriptor,
            message,
          });
          report.discarded += 1;
          failed_handles.insert(handle);
          match registry.finish(handle) {
            Ok(()) => report.finished.push(event.descriptor),
            Err(error) => {
              report.lifecycle_failures.push(FfiAsyncLifecycleFailure {
                descriptor: event.descriptor,
                error,
              });
            }
          }
          match self.purge_handle(handle) {
            Ok(purged) => report.purged += purged,
            Err(error) => report.queue_failures.push(FfiAsyncQueueFailure {
              descriptor: event.descriptor,
              error,
            }),
          }
        }
      }
    }

    Ok(report)
  }

  fn purge_handle(&self, handle: FfiAsyncHandle) -> Result<usize, FfiAsyncQueueError> {
    let mut queue = self.state.lock().map_err(|_| FfiAsyncQueueError::QueuePoisoned)?;
    let before = queue.events.len();
    let mut purged_bytes = 0;
    queue.events.retain(|event| {
      if event.descriptor.task_handle == handle.raw() {
        purged_bytes += event.payload.len();
        false
      } else {
        true
      }
    });
    queue.queued_bytes = queue.queued_bytes.saturating_sub(purged_bytes);
    let purged = before - queue.events.len();
    if let Some(state) = queue.tasks.get_mut(&handle) {
      state.record_purged(purged);
    }
    Ok(purged)
  }

  fn ensure_host_thread(&self) -> Result<(), FfiAsyncQueueError> {
    if thread::current().id() == self.host_thread {
      Ok(())
    } else {
      Err(FfiAsyncQueueError::WrongDrainThread)
    }
  }
}

#[cfg(test)]
mod tests {
  use std::sync::{Arc, Barrier};
  use std::thread;

  use super::*;
  use crate::ffi_abi::{ASYNC_TASK_FLAG_COALESCE_ALLOWED, FfiAsyncHandleKind};

  #[test]
  fn bounded_queue_rejects_full_without_consuming_sequence() {
    let registry = FfiAsyncHandleRegistry::new();
    let handle = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register stream");
    let queue = FfiAsyncEventQueue::new(1).expect("create queue");

    assert_eq!(
      queue
        .enqueue(&registry, handle, None, FfiAsyncEventKind::Emit, b"first".to_vec())
        .expect("enqueue first")
        .sequence,
      1
    );
    assert_eq!(
      queue.enqueue(&registry, handle, None, FfiAsyncEventKind::Emit, b"second".to_vec()),
      Err(FfiAsyncQueueError::QueueFull { capacity: 1 })
    );
    assert_eq!(registry.state(handle).expect("stream state").next_sequence, 2);
  }

  #[test]
  fn queue_limits_validate_byte_capacity_and_terminal_reserve() {
    assert!(matches!(
      FfiAsyncEventQueue::with_limits(FfiAsyncQueueLimits {
        event_capacity: 1,
        byte_capacity: 0,
        terminal_event_reserve: 0,
        terminal_byte_reserve: 0,
      }),
      Err(FfiAsyncQueueError::InvalidByteCapacity)
    ));
    assert!(matches!(
      FfiAsyncEventQueue::with_limits(FfiAsyncQueueLimits {
        event_capacity: 1,
        byte_capacity: 8,
        terminal_event_reserve: 2,
        terminal_byte_reserve: 0,
      }),
      Err(FfiAsyncQueueError::InvalidTerminalReserve)
    ));
    assert!(matches!(
      FfiAsyncEventQueue::with_limits(FfiAsyncQueueLimits {
        event_capacity: 1,
        byte_capacity: 8,
        terminal_event_reserve: 0,
        terminal_byte_reserve: 9,
      }),
      Err(FfiAsyncQueueError::InvalidTerminalReserve)
    ));
  }

  #[test]
  fn byte_budget_rejects_emit_without_consuming_sequence_but_accepts_reserved_terminal() {
    let registry = FfiAsyncHandleRegistry::new();
    let stream = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register stream");
    let task = registry.register(FfiAsyncHandleKind::OneShot, ()).expect("register one-shot");
    let queue = FfiAsyncEventQueue::with_limits(FfiAsyncQueueLimits {
      event_capacity: 3,
      byte_capacity: 12,
      terminal_event_reserve: 1,
      terminal_byte_reserve: 5,
    })
    .expect("create byte-bounded queue");

    queue
      .enqueue(&registry, stream, None, FfiAsyncEventKind::Emit, vec![1; 7])
      .expect("fill ordinary byte budget");
    assert_eq!(
      queue.enqueue(&registry, stream, None, FfiAsyncEventKind::Emit, vec![2]),
      Err(FfiAsyncQueueError::QueueBytesFull {
        capacity: 7,
        queued: 7,
        incoming: 1,
      })
    );
    assert_eq!(registry.state(stream).expect("stream state").next_sequence, 2);

    queue
      .enqueue(&registry, task, None, FfiAsyncEventKind::Complete, b"&unit".to_vec())
      .expect("terminal event uses reserved bytes");
    assert_eq!(queue.queued_bytes(), Ok(12));
    let report = queue.drain(&registry, 3, |_| Ok(())).expect("drain byte-bounded queue");
    assert_eq!(report.delivered, 2);
    assert_eq!(queue.queued_bytes(), Ok(0));
  }

  #[test]
  fn ordinary_events_cannot_consume_terminal_event_reserve() {
    let registry = FfiAsyncHandleRegistry::new();
    let first = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register first stream");
    let second = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register second stream");
    let third = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register third stream");
    let terminal = registry.register(FfiAsyncHandleKind::OneShot, ()).expect("register terminal task");
    let queue = FfiAsyncEventQueue::with_limits(FfiAsyncQueueLimits {
      event_capacity: 3,
      byte_capacity: 128,
      terminal_event_reserve: 1,
      terminal_byte_reserve: 8,
    })
    .expect("create reserved queue");

    for handle in [first, second] {
      queue
        .enqueue(&registry, handle, None, FfiAsyncEventKind::Emit, vec![])
        .expect("fill ordinary event slots");
    }
    assert_eq!(
      queue.enqueue(&registry, third, None, FfiAsyncEventKind::Emit, vec![]),
      Err(FfiAsyncQueueError::QueueFull { capacity: 2 })
    );
    queue
      .enqueue(&registry, terminal, None, FfiAsyncEventKind::Complete, b"&unit".to_vec())
      .expect("terminal event uses reserved slot");
    assert_eq!(queue.len(), Ok(3));
  }

  #[test]
  fn byte_pressure_coalescing_replaces_payload_and_keeps_accounting_exact() {
    let registry = FfiAsyncHandleRegistry::new();
    let handle = registry
      .register_with_flags(FfiAsyncHandleKind::Stream, ASYNC_TASK_FLAG_COALESCE_ALLOWED, ())
      .expect("register coalescing stream");
    let queue = FfiAsyncEventQueue::with_limits(FfiAsyncQueueLimits {
      event_capacity: 4,
      byte_capacity: 10,
      terminal_event_reserve: 1,
      terminal_byte_reserve: 2,
    })
    .expect("create byte-bounded queue");

    queue
      .enqueue(&registry, handle, None, FfiAsyncEventKind::Emit, vec![1; 6])
      .expect("enqueue initial event");
    let outcome = queue
      .enqueue(&registry, handle, None, FfiAsyncEventKind::Emit, vec![2; 7])
      .expect("coalesce under byte pressure");
    assert_eq!(outcome.disposition, FfiAsyncEnqueueDisposition::Coalesced);
    assert_eq!(queue.queued_bytes(), Ok(7));
    assert_eq!(
      queue.enqueue(&registry, handle, None, FfiAsyncEventKind::Emit, vec![3; 9]),
      Err(FfiAsyncQueueError::QueueBytesFull {
        capacity: 8,
        queued: 0,
        incoming: 9,
      })
    );
    assert_eq!(queue.queued_bytes(), Ok(7));

    let mut payload = vec![];
    queue
      .drain(&registry, 1, |event| {
        payload = event.payload().to_vec();
        Ok(())
      })
      .expect("drain coalesced event");
    assert_eq!(payload, vec![2; 7]);
    assert_eq!(queue.queued_bytes(), Ok(0));
  }

  #[test]
  fn task_metrics_track_coalescing_rejection_drain_and_purge_without_global_scans() {
    let registry = FfiAsyncHandleRegistry::new();
    let coalescing = registry
      .register_with_flags(FfiAsyncHandleKind::Stream, ASYNC_TASK_FLAG_COALESCE_ALLOWED, ())
      .expect("register coalescing stream");
    let ordinary = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register ordinary stream");
    let queue = FfiAsyncEventQueue::new(2).expect("create queue");

    queue
      .enqueue(&registry, coalescing, None, FfiAsyncEventKind::Emit, b"old".to_vec())
      .expect("enqueue old coalescing event");
    queue
      .enqueue(&registry, ordinary, None, FfiAsyncEventKind::Emit, b"x".to_vec())
      .expect("fill queue with ordinary event");
    queue
      .enqueue(&registry, coalescing, None, FfiAsyncEventKind::Emit, b"newer".to_vec())
      .expect("coalesce latest task event");
    assert_eq!(
      queue.enqueue(&registry, ordinary, None, FfiAsyncEventKind::Emit, b"blocked".to_vec()),
      Err(FfiAsyncQueueError::QueueFull { capacity: 2 })
    );

    let coalescing_metrics = queue
      .task_metrics(coalescing)
      .expect("read coalescing metrics")
      .expect("metrics exist");
    assert_eq!(coalescing_metrics.queued_events, 1);
    assert_eq!(coalescing_metrics.queued_bytes, 5);
    assert!(coalescing_metrics.oldest_age.is_some());
    assert_eq!(coalescing_metrics.accepted_total, 2);
    assert_eq!(coalescing_metrics.coalesced_total, 1);
    assert_eq!(coalescing_metrics.queue_full_total, 0);

    let ordinary_metrics = queue.task_metrics(ordinary).expect("read ordinary metrics").expect("metrics exist");
    assert_eq!(ordinary_metrics.queued_events, 1);
    assert_eq!(ordinary_metrics.queued_bytes, 1);
    assert_eq!(ordinary_metrics.accepted_total, 1);
    assert_eq!(ordinary_metrics.queue_full_total, 1);

    queue.drain(&registry, 1, |_| Ok(())).expect("drain coalesced event");
    let coalescing_metrics = queue
      .task_metrics(coalescing)
      .expect("read drained metrics")
      .expect("metrics exist");
    assert_eq!(coalescing_metrics.queued_events, 0);
    assert_eq!(coalescing_metrics.queued_bytes, 0);
    assert_eq!(coalescing_metrics.oldest_age, None);
    assert_eq!(coalescing_metrics.dequeued_total, 1);

    assert_eq!(queue.discard_handle_events(ordinary), Ok(1));
    let ordinary_metrics = queue.task_metrics(ordinary).expect("read purged metrics").expect("metrics exist");
    assert_eq!(ordinary_metrics.queued_events, 0);
    assert_eq!(ordinary_metrics.purged_total, 1);
    assert_eq!(queue.task_metrics_snapshot().expect("snapshot metrics").len(), 2);
    assert!(queue.take_task_metrics(coalescing).expect("take metrics").is_some());
    assert_eq!(queue.task_metrics(coalescing), Ok(None));
  }

  #[test]
  fn full_queue_coalesces_only_opted_in_emit_events() {
    let registry = FfiAsyncHandleRegistry::new();
    let handle = registry
      .register_with_flags(FfiAsyncHandleKind::Stream, ASYNC_TASK_FLAG_COALESCE_ALLOWED, ())
      .expect("register coalescing stream");
    let queue = FfiAsyncEventQueue::new(1).expect("create queue");

    queue
      .enqueue(&registry, handle, None, FfiAsyncEventKind::Emit, b"old".to_vec())
      .expect("enqueue old event");
    let outcome = queue
      .enqueue(&registry, handle, None, FfiAsyncEventKind::Emit, b"new".to_vec())
      .expect("coalesce event");
    assert_eq!(outcome.sequence, 2);
    assert_eq!(outcome.disposition, FfiAsyncEnqueueDisposition::Coalesced);

    let mut payloads = vec![];
    let report = queue
      .drain(&registry, 8, |event| {
        payloads.push(event.payload().to_vec());
        Ok(())
      })
      .expect("drain queue");
    assert_eq!(payloads, vec![b"new".to_vec()]);
    assert_eq!(report.delivered, 1);
  }

  #[test]
  fn terminal_events_are_exactly_once_and_finish_during_drain() {
    let registry = FfiAsyncHandleRegistry::new();
    let handle = registry.register(FfiAsyncHandleKind::OneShot, ()).expect("register task");
    let queue = FfiAsyncEventQueue::new(2).expect("create queue");

    queue
      .enqueue(&registry, handle, None, FfiAsyncEventKind::Complete, b"&unit".to_vec())
      .expect("enqueue completion");
    assert_eq!(
      queue.enqueue(&registry, handle, None, FfiAsyncEventKind::Fail, b"late".to_vec()),
      Err(FfiAsyncQueueError::Handle(FfiAsyncHandleError::TerminalAlreadyQueued))
    );
    assert_eq!(
      registry.state(handle).expect("pending terminal").lifecycle,
      FfiAsyncLifecycle::Active
    );

    let report = queue.drain(&registry, 1, |_| Ok(())).expect("drain completion");
    assert_eq!(report.delivered, 1);
    assert_eq!(report.finished.len(), 1);
    assert_eq!(report.finished[0].task_handle, handle.raw());
    assert_eq!(
      registry.state(handle).expect("finished task").lifecycle,
      FfiAsyncLifecycle::Finished
    );
  }

  #[test]
  fn full_queue_does_not_claim_a_terminal_sequence() {
    let registry = FfiAsyncHandleRegistry::new();
    let blocker = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register blocker");
    let task = registry.register(FfiAsyncHandleKind::OneShot, ()).expect("register task");
    let queue = FfiAsyncEventQueue::new(1).expect("create queue");
    queue
      .enqueue(&registry, blocker, None, FfiAsyncEventKind::Emit, vec![])
      .expect("fill queue");

    assert_eq!(
      queue.enqueue(&registry, task, None, FfiAsyncEventKind::Complete, b"&unit".to_vec()),
      Err(FfiAsyncQueueError::QueueFull { capacity: 1 })
    );
    let state = registry.state(task).expect("unclaimed task");
    assert_eq!(state.next_sequence, 1);
    assert!(!state.terminal_queued);

    queue.drain(&registry, 1, |_| Ok(())).expect("make capacity");
    assert_eq!(
      queue
        .enqueue(&registry, task, None, FfiAsyncEventKind::Complete, b"&unit".to_vec())
        .expect("enqueue terminal")
        .sequence,
      1
    );
  }

  #[test]
  fn cancellation_discards_previously_queued_emit_but_delivers_terminal() {
    let registry = FfiAsyncHandleRegistry::new();
    let handle = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register stream");
    let queue = FfiAsyncEventQueue::new(3).expect("create queue");

    queue
      .enqueue(&registry, handle, None, FfiAsyncEventKind::Emit, b"tick".to_vec())
      .expect("enqueue tick");
    registry.begin_close(handle).expect("cancel stream");
    queue
      .enqueue(&registry, handle, None, FfiAsyncEventKind::Complete, b"&unit".to_vec())
      .expect("enqueue close acknowledgement");

    let mut kinds = vec![];
    let report = queue
      .drain(&registry, 3, |event| {
        kinds.push(event.kind().expect("known kind"));
        Ok(())
      })
      .expect("drain cancelled stream");
    assert_eq!(kinds, vec![FfiAsyncEventKind::Complete]);
    assert_eq!(report.discarded, 1);
    assert_eq!(report.delivered, 1);
    assert_eq!(report.lifecycle_failures.len(), 1);
  }

  #[test]
  fn server_request_requires_one_active_response_handle() {
    let registry = FfiAsyncHandleRegistry::new();
    let server = registry
      .register_with_flags(
        FfiAsyncHandleKind::Server,
        crate::ffi_abi::ASYNC_TASK_FLAG_SERIAL_EVENTS | crate::ffi_abi::ASYNC_TASK_FLAG_REQUIRES_RESPONSE,
        (),
      )
      .expect("register server");
    let response = registry.register(FfiAsyncHandleKind::Response, ()).expect("register response");
    let queue = FfiAsyncEventQueue::new(3).expect("create queue");

    assert_eq!(
      queue.enqueue(&registry, server, None, FfiAsyncEventKind::Emit, vec![]),
      Err(FfiAsyncQueueError::Handle(FfiAsyncHandleError::MissingResponse))
    );
    queue
      .enqueue(&registry, server, Some(response), FfiAsyncEventKind::Emit, b"request".to_vec())
      .expect("enqueue request with response");
    assert_eq!(
      queue.enqueue(&registry, server, Some(response), FfiAsyncEventKind::Complete, b"&unit".to_vec()),
      Err(FfiAsyncQueueError::Handle(FfiAsyncHandleError::UnexpectedResponse))
    );
  }

  #[test]
  fn ordinary_stream_rejects_unexpected_response_capabilities() {
    let registry = FfiAsyncHandleRegistry::new();
    let stream = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register stream");
    let response = registry.register(FfiAsyncHandleKind::Response, ()).expect("register response");
    let queue = FfiAsyncEventQueue::new(1).expect("create queue");
    assert_eq!(
      queue.enqueue(&registry, stream, Some(response), FfiAsyncEventKind::Emit, vec![]),
      Err(FfiAsyncQueueError::Handle(FfiAsyncHandleError::UnexpectedResponse))
    );
  }

  #[test]
  fn callback_failure_is_reported_finishes_task_and_purges_remaining_events() {
    let registry = FfiAsyncHandleRegistry::new();
    let handle = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register stream");
    let other = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register other stream");
    let queue = FfiAsyncEventQueue::new(4).expect("create queue");
    for (task, payload) in [(handle, b"bad".to_vec()), (handle, b"late".to_vec()), (other, b"ok".to_vec())] {
      queue
        .enqueue(&registry, task, None, FfiAsyncEventKind::Emit, payload)
        .expect("enqueue event");
    }

    let report = queue
      .drain(&registry, 1, |event| {
        if event.payload() == b"bad" {
          Err("Calcit callback failed".to_owned())
        } else {
          Ok(())
        }
      })
      .expect("drain failing callback");
    assert_eq!(report.callback_failures.len(), 1);
    assert_eq!(report.callback_failures[0].message, "Calcit callback failed");
    assert_eq!(report.discarded, 1);
    assert_eq!(report.purged, 1);
    assert_eq!(report.dequeued, report.delivered + report.discarded);
    assert_eq!(report.finished.len(), 1);
    assert_eq!(registry.state(handle).expect("failed task").lifecycle, FfiAsyncLifecycle::Finished);
    assert_eq!(queue.len(), Ok(1));
  }

  #[test]
  fn malformed_batch_event_is_reported_without_losing_drain_accounting() {
    let registry = FfiAsyncHandleRegistry::new();
    let malformed = registry
      .register(FfiAsyncHandleKind::Stream, ())
      .expect("register malformed stream");
    let valid = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register valid stream");
    let queue = FfiAsyncEventQueue::new(2).expect("create queue");
    queue
      .enqueue(&registry, malformed, None, FfiAsyncEventKind::Emit, vec![])
      .expect("enqueue malformed event placeholder");
    queue
      .enqueue(&registry, valid, None, FfiAsyncEventKind::Emit, vec![])
      .expect("enqueue valid event");
    queue.state.lock().expect("queue lock").events[0].descriptor.kind = 99;

    let report = queue.drain(&registry, 2, |_| Ok(())).expect("degraded drain report");
    assert_eq!(report.dequeued, 2);
    assert_eq!(report.delivered, 1);
    assert_eq!(report.discarded, 1);
    assert_eq!(report.lifecycle_failures.len(), 1);
    assert_eq!(report.lifecycle_failures[0].error, FfiAsyncHandleError::InvalidEventKind(99));
    assert_eq!(report.dequeued, report.delivered + report.discarded);
  }

  #[test]
  fn foreign_threads_can_enqueue_but_cannot_drain() {
    let registry = Arc::new(FfiAsyncHandleRegistry::new());
    let handle = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register stream");
    let queue = Arc::new(FfiAsyncEventQueue::new(4).expect("create queue"));
    let barrier = Arc::new(Barrier::new(2));

    let producer_registry = Arc::clone(&registry);
    let producer_queue = Arc::clone(&queue);
    let producer_barrier = Arc::clone(&barrier);
    let producer = thread::spawn(move || {
      producer_barrier.wait();
      producer_queue
        .enqueue(&producer_registry, handle, None, FfiAsyncEventKind::Emit, b"event".to_vec())
        .expect("foreign producer enqueue");
      producer_queue.drain(&producer_registry, 1, |_| Ok(()))
    });
    barrier.wait();
    assert!(matches!(
      producer.join().expect("producer thread"),
      Err(FfiAsyncQueueError::WrongDrainThread)
    ));

    let mut producer_thread = None;
    queue
      .drain(&registry, 1, |event| {
        producer_thread = Some(event.producer_thread());
        Ok(())
      })
      .expect("host drain");
    assert!(producer_thread.is_some_and(|thread_id| thread_id != thread::current().id()));
  }

  #[test]
  fn concurrent_producers_preserve_sequence_order_in_the_queue() {
    let registry = Arc::new(FfiAsyncHandleRegistry::new());
    let handle = registry.register(FfiAsyncHandleKind::Stream, ()).expect("register stream");
    let queue = Arc::new(FfiAsyncEventQueue::new(8).expect("create queue"));
    let barrier = Arc::new(Barrier::new(5));
    let mut producers = vec![];

    for index in 0..4 {
      let producer_registry = Arc::clone(&registry);
      let producer_queue = Arc::clone(&queue);
      let producer_barrier = Arc::clone(&barrier);
      producers.push(thread::spawn(move || {
        producer_barrier.wait();
        producer_queue
          .enqueue(&producer_registry, handle, None, FfiAsyncEventKind::Emit, vec![index])
          .expect("concurrent enqueue")
          .sequence
      }));
    }
    barrier.wait();
    for producer in producers {
      producer.join().expect("producer thread");
    }

    let mut sequences = vec![];
    queue
      .drain(&registry, 8, |event| {
        sequences.push(event.descriptor.sequence);
        Ok(())
      })
      .expect("host drain");
    assert_eq!(sequences, vec![1, 2, 3, 4]);
    let metrics = queue.task_metrics(handle).expect("read concurrent metrics").expect("metrics exist");
    assert_eq!(metrics.accepted_total, 4);
    assert_eq!(metrics.dequeued_total, 4);
    assert_eq!(metrics.queued_events, 0);
    assert_eq!(metrics.queued_bytes, 0);
  }

  #[test]
  fn closing_queue_wakes_host_and_rejects_new_work() {
    let registry = FfiAsyncHandleRegistry::new();
    let handle = registry.register(FfiAsyncHandleKind::OneShot, ()).expect("register task");
    let queue = FfiAsyncEventQueue::new(1).expect("create queue");
    queue.close().expect("close queue");

    assert!(!queue.wait_for_event(Duration::from_millis(1)).expect("closed queue wait"));
    assert_eq!(
      queue.enqueue(&registry, handle, None, FfiAsyncEventKind::Complete, b"&unit".to_vec()),
      Err(FfiAsyncQueueError::QueueClosed)
    );
  }

  #[test]
  fn foreign_payload_copy_checks_null_size_and_ownership() {
    assert_eq!(unsafe { copy_async_payload(std::ptr::null(), 0) }, Ok(vec![]));
    assert_eq!(
      unsafe { copy_async_payload(std::ptr::null(), 1) },
      Err(FfiAsyncQueueError::NullPayload)
    );
    assert_eq!(
      unsafe { copy_async_payload(std::ptr::null(), MAX_ASYNC_EVENT_PAYLOAD_BYTES + 1) },
      Err(FfiAsyncQueueError::PayloadTooLarge {
        actual: MAX_ASYNC_EVENT_PAYLOAD_BYTES + 1,
        limit: MAX_ASYNC_EVENT_PAYLOAD_BYTES,
      })
    );

    let source = b"([] 1 2)".to_vec();
    let copied = unsafe { copy_async_payload(source.as_ptr(), source.len()) }.expect("copy payload");
    assert_eq!(copied, source);
  }
}
