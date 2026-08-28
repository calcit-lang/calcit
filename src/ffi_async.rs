//! Host-side scheduling primitives for asynchronous FFI events.
//!
//! Foreign producers may enqueue from any thread, but only the thread that
//! creates the queue may drain it and enter the Calcit runtime. No Rust
//! callback, executor, or allocator crosses the transport boundary.

use std::collections::{HashSet, VecDeque};
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
  closed: bool,
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
        return Err(FfiAsyncQueueError::QueueFull { capacity: event_limit });
      }
      if exceeds_byte_limit {
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

    let disposition = if let Some(index) = coalesce_index {
      queue.queued_bytes = queue.queued_bytes.saturating_sub(queue.events[index].payload.len()) + event.payload.len();
      queue.events[index] = event;
      FfiAsyncEnqueueDisposition::Coalesced
    } else {
      queue.queued_bytes += event.payload.len();
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
    Ok(before - queue.events.len())
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
