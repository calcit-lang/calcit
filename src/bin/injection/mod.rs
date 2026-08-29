use crate::runner;
use cirru_edn::{Edn, EdnAnyRef};
use colored::Colorize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use calcit::{
  builtins,
  builtins::{RegisteredProcDescriptor, RegisteredProcPlatform, RegisteredProcStability, proc_tags},
  calcit::{Calcit, CalcitErr, CalcitErrKind},
  call_stack::{CallStackList, display_stack},
  data::edn::{calcit_to_edn, edn_to_calcit, sanitize_edn_for_format},
  ffi_abi::{
    ASYNC_RESPONSE_REJECT, ASYNC_RESPONSE_RESOLVE, ASYNC_TASK_FLAG_REQUIRES_RESPONSE, ASYNC_TASK_FLAG_SERIAL_EVENTS, FfiAsyncEventKind,
    FfiAsyncHandle, FfiAsyncHandleKind, FfiAsyncHandleRegistry, FfiAsyncHostV1, FfiAsyncResponseResolve, FfiAsyncTaskCancel,
    FfiAsyncTaskDescriptor, FfiBlockingHostV1, FfiBuffer, async_status,
  },
  ffi_async::{FfiAsyncDrainReport, FfiAsyncEventQueue, FfiAsyncTaskQueueMetrics, copy_async_payload},
  runner::track,
};

/// lazily cache dylibs, in case Linux drops memory of libraries
static DYLIBS: LazyLock<Mutex<HashMap<String, Arc<libloading::Library>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static TRACE_FFI: AtomicBool = AtomicBool::new(false);
static FFI_METRICS_ENABLED: AtomicBool = AtomicBool::new(false);
static STDOUT_TO_STDERR: AtomicBool = AtomicBool::new(false);
static SILENCE_PROGRAM_OUTPUT: AtomicBool = AtomicBool::new(false);
static TRACE_FFI_EVENT_ID: AtomicUsize = AtomicUsize::new(1);
static TRACE_FFI_STARTED: LazyLock<Instant> = LazyLock::new(Instant::now);
const ASYNC_EVENT_QUEUE_CAPACITY: usize = 1024;
const ASYNC_EVENT_QUEUE_BYTE_CAPACITY: usize = 64 * 1024 * 1024;
const ASYNC_TERMINAL_EVENT_RESERVE: usize = 16;
const ASYNC_TERMINAL_BYTE_RESERVE: usize = 64 * 1024;
const MAX_ASYNC_RESPONSES_PER_TASK: usize = ASYNC_EVENT_QUEUE_CAPACITY;
const MAX_ASYNC_RESPONSE_TIMEOUT_MS: u64 = 24 * 60 * 60 * 1000;
const ASYNC_SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

static CTRL_C_CALLBACK_PENDING: AtomicBool = AtomicBool::new(false);
type CtrlCCallback = (Arc<Calcit>, Arc<CallStackList>);
static CTRL_C_CALLBACK: LazyLock<Mutex<Option<CtrlCCallback>>> = LazyLock::new(|| Mutex::new(None));

#[derive(Clone)]
struct NativeAsyncTask {
  callback: Calcit,
  stack: Arc<CallStackList>,
  lib_name: String,
  method: String,
  control: Arc<Mutex<NativeAsyncTaskState>>,
  blocking: Option<NativeBlockingTask>,
  started_at: Instant,
}

#[derive(Clone)]
struct NativeBlockingTask {
  owner_thread: thread::ThreadId,
  buffers: Arc<Mutex<HashMap<usize, Box<[u8]>>>>,
  failures: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone, Copy)]
struct NativeAsyncTaskControl {
  context: u64,
  cancel: FfiAsyncTaskCancel,
}

#[derive(Default)]
struct NativeAsyncTaskState {
  control: Option<NativeAsyncTaskControl>,
  outcomes: NativeAsyncTaskOutcomes,
}

#[derive(Clone, Copy)]
struct NativeAsyncResponse {
  owner_task: FfiAsyncHandle,
  context: u64,
  deadline: Instant,
  resolve: FfiAsyncResponseResolve,
}

#[derive(Clone)]
enum NativeAsyncResource {
  Task(NativeAsyncTask),
  Response(NativeAsyncResponse),
}

#[derive(Default)]
struct NativeAsyncResponseIndex {
  deadlines: BTreeMap<Instant, HashSet<FfiAsyncHandle>>,
  responses: HashMap<FfiAsyncHandle, (FfiAsyncHandle, Instant)>,
  by_owner: HashMap<FfiAsyncHandle, HashSet<FfiAsyncHandle>>,
}

impl NativeAsyncResponseIndex {
  fn insert(&mut self, handle: FfiAsyncHandle, response: NativeAsyncResponse) -> Result<(), ()> {
    let owner_responses = self.by_owner.entry(response.owner_task).or_default();
    if owner_responses.len() >= MAX_ASYNC_RESPONSES_PER_TASK {
      return Err(());
    }
    owner_responses.insert(handle);
    self.responses.insert(handle, (response.owner_task, response.deadline));
    self.deadlines.entry(response.deadline).or_default().insert(handle);
    Ok(())
  }

  fn remove(&mut self, handle: FfiAsyncHandle) -> bool {
    let Some((owner, deadline)) = self.responses.remove(&handle) else {
      return false;
    };
    if let Some(deadline_responses) = self.deadlines.get_mut(&deadline) {
      deadline_responses.remove(&handle);
      if deadline_responses.is_empty() {
        self.deadlines.remove(&deadline);
      }
    }
    if let Some(owner_responses) = self.by_owner.get_mut(&owner) {
      owner_responses.remove(&handle);
      if owner_responses.is_empty() {
        self.by_owner.remove(&owner);
      }
    }
    true
  }

  fn take_due(&mut self, now: Instant) -> Vec<FfiAsyncHandle> {
    let mut due = vec![];
    while let Some((&deadline, _handles)) = self.deadlines.first_key_value() {
      if deadline > now {
        break;
      }
      let (_deadline, handles) = self.deadlines.pop_first().expect("first deadline exists");
      for handle in handles {
        let Some((owner, current)) = self.responses.remove(&handle) else {
          continue;
        };
        debug_assert_eq!(current, deadline);
        if let Some(owner_responses) = self.by_owner.get_mut(&owner) {
          owner_responses.remove(&handle);
          if owner_responses.is_empty() {
            self.by_owner.remove(&owner);
          }
        }
        due.push(handle);
      }
    }
    due
  }

  fn take_owner(&mut self, owner: FfiAsyncHandle) -> Vec<FfiAsyncHandle> {
    let Some(handles) = self.by_owner.get(&owner).cloned() else {
      return vec![];
    };
    for handle in &handles {
      self.remove(*handle);
    }
    handles.into_iter().collect()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeAsyncCapability {
  handle: FfiAsyncHandle,
  kind: FfiAsyncHandleKind,
}

struct NativeAsyncRuntime {
  registry: FfiAsyncHandleRegistry<NativeAsyncResource>,
  queue: FfiAsyncEventQueue,
  responses: Mutex<NativeAsyncResponseIndex>,
  completed_metrics: Mutex<BTreeMap<(String, String), NativeAsyncModuleQueueMetrics>>,
}

#[derive(Debug, Clone, Copy, Default)]
struct NativeAsyncTaskOutcomes {
  cancel_requested_total: u64,
  cancel_succeeded_total: u64,
  cancel_failed_total: u64,
  deadline_timeout_total: u64,
}

#[derive(Debug, Clone, Default)]
struct NativeAsyncModuleQueueMetrics {
  active_tasks: usize,
  closing_tasks: usize,
  completed_tasks: u64,
  queued_events: usize,
  queued_bytes: usize,
  oldest_age: Option<Duration>,
  accepted_total: u64,
  coalesced_total: u64,
  queue_full_total: u64,
  dequeued_total: u64,
  purged_total: u64,
  cancel_requested_total: u64,
  cancel_succeeded_total: u64,
  cancel_failed_total: u64,
  deadline_timeout_total: u64,
}

impl NativeAsyncModuleQueueMetrics {
  fn add_task(&mut self, lifecycle: calcit::ffi_abi::FfiAsyncLifecycle, metrics: FfiAsyncTaskQueueMetrics) {
    match lifecycle {
      calcit::ffi_abi::FfiAsyncLifecycle::Active => self.active_tasks += 1,
      calcit::ffi_abi::FfiAsyncLifecycle::Closing => self.closing_tasks += 1,
      calcit::ffi_abi::FfiAsyncLifecycle::Finished => {}
    }
    self.queued_events = self.queued_events.saturating_add(metrics.queued_events);
    self.queued_bytes = self.queued_bytes.saturating_add(metrics.queued_bytes);
    self.oldest_age = match (self.oldest_age, metrics.oldest_age) {
      (Some(current), Some(candidate)) => Some(current.max(candidate)),
      (None, Some(candidate)) => Some(candidate),
      (current, None) => current,
    };
    self.accepted_total = self.accepted_total.saturating_add(metrics.accepted_total);
    self.coalesced_total = self.coalesced_total.saturating_add(metrics.coalesced_total);
    self.queue_full_total = self.queue_full_total.saturating_add(metrics.queue_full_total);
    self.dequeued_total = self.dequeued_total.saturating_add(metrics.dequeued_total);
    self.purged_total = self.purged_total.saturating_add(metrics.purged_total);
  }

  fn add_completed(&mut self, metrics: FfiAsyncTaskQueueMetrics, outcomes: NativeAsyncTaskOutcomes) {
    self.completed_tasks = self.completed_tasks.saturating_add(1);
    self.add_task(calcit::ffi_abi::FfiAsyncLifecycle::Finished, metrics);
    self.add_outcomes(outcomes);
  }

  fn add_outcomes(&mut self, outcomes: NativeAsyncTaskOutcomes) {
    self.cancel_requested_total = self.cancel_requested_total.saturating_add(outcomes.cancel_requested_total);
    self.cancel_succeeded_total = self.cancel_succeeded_total.saturating_add(outcomes.cancel_succeeded_total);
    self.cancel_failed_total = self.cancel_failed_total.saturating_add(outcomes.cancel_failed_total);
    self.deadline_timeout_total = self.deadline_timeout_total.saturating_add(outcomes.deadline_timeout_total);
  }

  fn merge(&mut self, other: &Self) {
    self.active_tasks = self.active_tasks.saturating_add(other.active_tasks);
    self.closing_tasks = self.closing_tasks.saturating_add(other.closing_tasks);
    self.completed_tasks = self.completed_tasks.saturating_add(other.completed_tasks);
    self.queued_events = self.queued_events.saturating_add(other.queued_events);
    self.queued_bytes = self.queued_bytes.saturating_add(other.queued_bytes);
    self.oldest_age = match (self.oldest_age, other.oldest_age) {
      (Some(current), Some(candidate)) => Some(current.max(candidate)),
      (None, Some(candidate)) => Some(candidate),
      (current, None) => current,
    };
    self.accepted_total = self.accepted_total.saturating_add(other.accepted_total);
    self.coalesced_total = self.coalesced_total.saturating_add(other.coalesced_total);
    self.queue_full_total = self.queue_full_total.saturating_add(other.queue_full_total);
    self.dequeued_total = self.dequeued_total.saturating_add(other.dequeued_total);
    self.purged_total = self.purged_total.saturating_add(other.purged_total);
    self.cancel_requested_total = self.cancel_requested_total.saturating_add(other.cancel_requested_total);
    self.cancel_succeeded_total = self.cancel_succeeded_total.saturating_add(other.cancel_succeeded_total);
    self.cancel_failed_total = self.cancel_failed_total.saturating_add(other.cancel_failed_total);
    self.deadline_timeout_total = self.deadline_timeout_total.saturating_add(other.deadline_timeout_total);
  }
}

fn format_oldest_age(age: Option<Duration>) -> String {
  age
    .map(|value| format!("{:.3}", value.as_secs_f64() * 1000.0))
    .unwrap_or_else(|| "none".to_owned())
}

fn format_task_queue_metrics(metrics: FfiAsyncTaskQueueMetrics) -> String {
  format!(
    "queued_events={} queued_bytes={} oldest_ms={} accepted={} coalesced={} queue_full={} dequeued={} purged={}",
    metrics.queued_events,
    metrics.queued_bytes,
    format_oldest_age(metrics.oldest_age),
    metrics.accepted_total,
    metrics.coalesced_total,
    metrics.queue_full_total,
    metrics.dequeued_total,
    metrics.purged_total
  )
}

fn task_outcomes(task: &NativeAsyncTask) -> Result<NativeAsyncTaskOutcomes, String> {
  task
    .control
    .lock()
    .map(|state| state.outcomes)
    .map_err(|_| "async FFI task outcome metrics lock is poisoned".to_owned())
}

fn ffi_metrics_enabled() -> bool {
  FFI_METRICS_ENABLED.load(Ordering::Relaxed) || cfg!(test)
}

fn update_task_outcomes(task: &NativeAsyncTask, f: impl FnOnce(&mut NativeAsyncTaskOutcomes)) -> Result<(), String> {
  if !ffi_metrics_enabled() {
    return Ok(());
  }
  let mut outcomes = task
    .control
    .lock()
    .map_err(|_| "async FFI task outcome metrics lock is poisoned".to_owned())?;
  f(&mut outcomes.outcomes);
  Ok(())
}

fn archive_task_metrics(
  runtime: &NativeAsyncRuntime,
  task: &NativeAsyncTask,
  queue_metrics: FfiAsyncTaskQueueMetrics,
) -> Result<(), String> {
  if !ffi_metrics_enabled() || task.blocking.is_some() {
    return Ok(());
  }
  let outcomes = task_outcomes(task)?;
  runtime
    .completed_metrics
    .lock()
    .map_err(|_| "async FFI completed metrics lock is poisoned".to_owned())?
    .entry((task.lib_name.clone(), task.method.clone()))
    .or_default()
    .add_completed(queue_metrics, outcomes);
  Ok(())
}

fn collect_native_async_module_metrics(
  runtime: &NativeAsyncRuntime,
) -> Result<BTreeMap<(String, String), NativeAsyncModuleQueueMetrics>, String> {
  let mut modules = runtime
    .completed_metrics
    .lock()
    .map_err(|_| "async FFI completed metrics lock is poisoned".to_owned())?
    .clone();
  let queue_metrics: HashMap<FfiAsyncHandle, FfiAsyncTaskQueueMetrics> = runtime
    .queue
    .task_metrics_snapshot()
    .map_err(|error| error.to_string())?
    .into_iter()
    .collect();
  for (handle, state, resource) in runtime.registry.snapshot().map_err(|error| error.to_string())? {
    let NativeAsyncResource::Task(task) = resource else {
      continue;
    };
    if task.blocking.is_some() {
      continue;
    }
    let aggregate = modules.entry((task.lib_name.clone(), task.method.clone())).or_default();
    aggregate.add_task(state.lifecycle, queue_metrics.get(&handle).copied().unwrap_or_default());
    aggregate.add_outcomes(task_outcomes(&task)?);
  }
  Ok(modules)
}

fn module_metrics_json(metrics: &NativeAsyncModuleQueueMetrics) -> serde_json::Value {
  serde_json::json!({
    "activeTasks": metrics.active_tasks,
    "closingTasks": metrics.closing_tasks,
    "completedTasks": metrics.completed_tasks,
    "queuedEvents": metrics.queued_events,
    "queuedBytes": metrics.queued_bytes,
    "oldestQueuedAgeMs": metrics.oldest_age.map(|age| age.as_secs_f64() * 1000.0),
    "acceptedTotal": metrics.accepted_total,
    "coalescedTotal": metrics.coalesced_total,
    "queueFullTotal": metrics.queue_full_total,
    "dequeuedTotal": metrics.dequeued_total,
    "purgedTotal": metrics.purged_total,
    "cancelRequestedTotal": metrics.cancel_requested_total,
    "cancelSucceededTotal": metrics.cancel_succeeded_total,
    "cancelFailedTotal": metrics.cancel_failed_total,
    "deadlineTimeoutTotal": metrics.deadline_timeout_total,
  })
}

fn native_async_metrics_json(runtime: &NativeAsyncRuntime) -> Result<String, String> {
  let modules = collect_native_async_module_metrics(runtime)?;
  let mut totals = NativeAsyncModuleQueueMetrics::default();
  let rows = modules
    .iter()
    .map(|((lib_name, method), metrics)| {
      totals.merge(metrics);
      let mut row = module_metrics_json(metrics);
      let object = row.as_object_mut().expect("module metrics JSON must be an object");
      object.insert("module".to_owned(), serde_json::Value::String(lib_name.clone()));
      object.insert("method".to_owned(), serde_json::Value::String(method.clone()));
      row
    })
    .collect::<Vec<_>>();
  serde_json::to_string(&serde_json::json!({
    "schemaVersion": 1,
    "units": { "age": "milliseconds", "bytes": "bytes" },
    "totals": module_metrics_json(&totals),
    "modules": rows,
  }))
  .map_err(|error| format!("failed to serialize native async FFI metrics: {error}"))
}

pub struct FfiMetricsReportOnDrop {
  enabled: bool,
}

impl FfiMetricsReportOnDrop {
  pub fn new(enabled: bool) -> Self {
    FFI_METRICS_ENABLED.store(enabled, Ordering::Relaxed);
    Self { enabled }
  }
}

impl Drop for FfiMetricsReportOnDrop {
  fn drop(&mut self) {
    if !self.enabled {
      return;
    }
    let Some(runtime) = NATIVE_ASYNC_RUNTIME.get() else {
      return;
    };
    match native_async_metrics_json(runtime) {
      Ok(report) => eprintln!("ffi-async-metrics: {report}"),
      Err(error) => eprintln!("[Warn] {error}"),
    }
    FFI_METRICS_ENABLED.store(false, Ordering::Relaxed);
  }
}

static NATIVE_ASYNC_RUNTIME: OnceLock<NativeAsyncRuntime> = OnceLock::new();
#[allow(dead_code)]
pub fn set_trace_ffi(v: bool) {
  TRACE_FFI.store(v, Ordering::Relaxed);
  if v {
    let cwd = std::env::current_dir()
      .map(|p| p.display().to_string())
      .unwrap_or_else(|_| "<unknown-cwd>".to_string());
    let exe = std::env::current_exe()
      .map(|p| p.display().to_string())
      .unwrap_or_else(|_| "<unknown-exe>".to_string());
    trace_ffi_event(
      "enable",
      format!(
        "cwd={cwd} exe={exe} buffer={} async={} resource={} host={}",
        calcit::ffi_abi::BUFFER_PROTOCOL_VERSION,
        calcit::ffi_abi::ASYNC_PROTOCOL_VERSION,
        calcit::ffi_abi::RESOURCE_PROTOCOL_VERSION,
        std::env::consts::OS,
      ),
    );
  }
}

#[allow(dead_code)]
pub fn set_stdout_to_stderr(v: bool) {
  STDOUT_TO_STDERR.store(v, Ordering::Relaxed);
}

#[allow(dead_code)]
pub fn set_program_output_silenced(v: bool) {
  SILENCE_PROGRAM_OUTPUT.store(v, Ordering::Relaxed);
}

fn should_trace_ffi() -> bool {
  TRACE_FFI.load(Ordering::Relaxed)
}

fn format_edn_args_for_trace(args: &[Edn]) -> String {
  let sanitized: Vec<Edn> = args.iter().map(sanitize_edn_for_format).collect();
  match cirru_edn::format(&Edn::List(cirru_edn::EdnListView(sanitized)), true) {
    Ok(s) => s.trim().to_owned(),
    Err(e) => format!("<failed to format ffi args: {e}>"),
  }
}

fn resolve_trace_path(lib_name: &str) -> String {
  let path = Path::new(lib_name);
  let resolved: PathBuf = if path.is_absolute() {
    path.to_path_buf()
  } else {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
  };

  match resolved.canonicalize() {
    Ok(p) => p.display().to_string(),
    Err(_) => resolved.display().to_string(),
  }
}

fn trace_ffi_event(label: &str, message: impl AsRef<str>) {
  if should_trace_ffi() {
    let event_id = TRACE_FFI_EVENT_ID.fetch_add(1, Ordering::Relaxed);
    let elapsed_ms = TRACE_FFI_STARTED.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
      "[ffi #{event_id} +{elapsed_ms:.3}ms pid={} tid={:?}] {label} {}",
      process::id(),
      thread::current().id(),
      message.as_ref()
    );
  }
}

fn trace_ffi_resource_event(label: &str, lib_name: &str, handle: u64, generation: u64, status: i32) {
  trace_ffi_event(
    label,
    format!("lib={lib_name} handle={handle} generation={generation} status={status}"),
  );
}

/// Bind the async queue to the CLI worker thread before any dylib can publish
/// work. Tests that only inject proc metadata do not initialize global runtime
/// state and therefore cannot accidentally claim host-thread ownership.
pub fn init_async_runtime() -> Result<(), String> {
  if NATIVE_ASYNC_RUNTIME.get().is_some() {
    return Ok(());
  }
  let runtime = NativeAsyncRuntime {
    registry: FfiAsyncHandleRegistry::new(),
    queue: FfiAsyncEventQueue::with_limits(calcit::ffi_async::FfiAsyncQueueLimits {
      event_capacity: ASYNC_EVENT_QUEUE_CAPACITY,
      byte_capacity: ASYNC_EVENT_QUEUE_BYTE_CAPACITY,
      terminal_event_reserve: ASYNC_TERMINAL_EVENT_RESERVE,
      terminal_byte_reserve: ASYNC_TERMINAL_BYTE_RESERVE,
    })
    .map_err(|error| error.to_string())?,
    responses: Mutex::new(NativeAsyncResponseIndex::default()),
    completed_metrics: Mutex::new(BTreeMap::new()),
  };
  NATIVE_ASYNC_RUNTIME
    .set(runtime)
    .map_err(|_| "async FFI runtime was initialized concurrently".to_owned())?;
  track::reset_shutdown();
  ctrlc::set_handler(|| {
    CTRL_C_CALLBACK_PENDING.store(true, Ordering::Release);
    track::request_shutdown();
  })
  .map_err(|error| format!("failed to install Calcit Ctrl-C handler: {error}"))
}

pub fn shutdown_requested() -> bool {
  track::shutdown_requested()
}

fn native_async_runtime() -> Result<&'static NativeAsyncRuntime, String> {
  NATIVE_ASYNC_RUNTIME
    .get()
    .ok_or_else(|| "async FFI runtime is not initialized on the CLI worker thread".to_owned())
}

fn async_host_table(task_handle: FfiAsyncHandle) -> FfiAsyncHostV1 {
  FfiAsyncHostV1::new(
    task_handle.raw(),
    native_async_enqueue,
    native_async_configure_task,
    native_async_open_response,
  )
}

fn blocking_host_table(task_handle: FfiAsyncHandle) -> FfiBlockingHostV1 {
  FfiBlockingHostV1::new(
    task_handle.raw(),
    native_blocking_invoke,
    native_blocking_finish,
    native_blocking_free_buffer,
  )
}

fn resolve_native_blocking_task(
  runtime: &NativeAsyncRuntime,
  context: u64,
  task_handle: u64,
) -> Result<(FfiAsyncHandle, NativeAsyncTask, NativeBlockingTask), i32> {
  let handle = FfiAsyncHandle::from_raw(task_handle);
  let task = match runtime.registry.clone_value(handle) {
    Ok(NativeAsyncResource::Task(task)) => task,
    Ok(NativeAsyncResource::Response(_)) => return Err(async_status::INVALID_HANDLE),
    Err(error) => return Err(error.status_code()),
  };
  let Some(blocking) = task.blocking.clone() else {
    return Err(async_status::INVALID_HANDLE);
  };
  if context != task_handle {
    record_blocking_failure(&blocking, "blocking FFI host context does not own this task");
    return Err(async_status::INVALID_HANDLE);
  }
  if blocking.owner_thread != thread::current().id() {
    record_blocking_failure(&blocking, "blocking FFI callback was invoked from a foreign thread");
    return Err(async_status::WRONG_THREAD);
  }
  Ok((handle, task, blocking))
}

fn record_blocking_failure(blocking: &NativeBlockingTask, message: impl Into<String>) {
  if let Ok(mut failures) = blocking.failures.lock() {
    failures.push(message.into());
  }
}

fn store_blocking_host_buffer(blocking: &NativeBlockingTask, bytes: Vec<u8>, out: *mut FfiBuffer) -> Result<(), i32> {
  if out.is_null() || bytes.is_empty() {
    return Err(async_status::INVALID_PAYLOAD);
  }
  let mut bytes = bytes.into_boxed_slice();
  let ptr = bytes.as_mut_ptr();
  let len = bytes.len();
  let key = ptr as usize;
  let mut buffers = blocking.buffers.lock().map_err(|_| async_status::INTERNAL_ERROR)?;
  if buffers.insert(key, bytes).is_some() {
    return Err(async_status::INTERNAL_ERROR);
  }
  // SAFETY: the C caller provides a writable output descriptor for this
  // synchronous host call. Ownership of the bytes remains in `buffers`.
  unsafe { out.write(FfiBuffer { ptr, len, cap: len }) };
  Ok(())
}

fn free_blocking_host_buffer(blocking: &NativeBlockingTask, buffer: FfiBuffer) -> Result<(), i32> {
  if buffer.ptr.is_null() || buffer.len == 0 || buffer.cap != buffer.len {
    return Err(async_status::INVALID_PAYLOAD);
  }
  let key = buffer.ptr as usize;
  let mut buffers = blocking.buffers.lock().map_err(|_| async_status::INTERNAL_ERROR)?;
  let Some(bytes) = buffers.remove(&key) else {
    return Err(async_status::INVALID_PAYLOAD);
  };
  if bytes.len() != buffer.len {
    buffers.insert(key, bytes);
    return Err(async_status::INVALID_PAYLOAD);
  }
  Ok(())
}

unsafe extern "C" fn native_blocking_invoke(
  context: u64,
  task_handle: u64,
  payload_ptr: *const u8,
  payload_len: usize,
  out: *mut FfiBuffer,
) -> i32 {
  std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let runtime = match native_async_runtime() {
      Ok(runtime) => runtime,
      Err(_) => return async_status::HOST_CLOSING,
    };
    // SAFETY: forwarded from the C entry point's documented pointer contract.
    unsafe { invoke_native_blocking(runtime, context, task_handle, payload_ptr, payload_len, out) }
  }))
  .unwrap_or(async_status::INTERNAL_ERROR)
}

unsafe fn invoke_native_blocking(
  runtime: &NativeAsyncRuntime,
  context: u64,
  task_handle: u64,
  payload_ptr: *const u8,
  payload_len: usize,
  out: *mut FfiBuffer,
) -> i32 {
  if out.is_null() {
    return async_status::INVALID_PAYLOAD;
  }
  // SAFETY: validated non-null above; initialize before any fallible work so
  // a rejected call never exposes uninitialized host memory.
  unsafe { out.write(FfiBuffer::empty()) };
  let (handle, task, blocking) = match resolve_native_blocking_task(runtime, context, task_handle) {
    Ok(task) => task,
    Err(status) => return status,
  };
  // SAFETY: the caller promises a readable payload for this synchronous call.
  let payload = match unsafe { copy_async_payload(payload_ptr, payload_len) } {
    Ok(payload) => payload,
    Err(error) => {
      record_blocking_failure(&blocking, error.to_string());
      return error.status_code();
    }
  };
  let args = match decode_async_emit(&payload) {
    Ok(args) => args,
    Err(error) => {
      record_blocking_failure(&blocking, error);
      return async_status::INVALID_PAYLOAD;
    }
  };
  let sequence = match runtime.registry.next_event_sequence(handle) {
    Ok(sequence) => sequence,
    Err(error) => {
      record_blocking_failure(&blocking, error.to_string());
      return error.status_code();
    }
  };
  let Calcit::Fn { info, .. } = &task.callback else {
    return async_status::INVALID_HANDLE;
  };
  trace_ffi_event(
    "blocking-callback-in-v1",
    format!(
      "lib={} symbol={} task={} sequence={} argc={}",
      task.lib_name,
      task.method,
      handle.raw(),
      sequence,
      args.len()
    ),
  );
  match runner::run_fn(&args, info, &task.stack) {
    Ok(ret) => match encode_async_value(&ret) {
      Ok(output) => match store_blocking_host_buffer(&blocking, output, out) {
        Ok(()) => async_status::OK,
        Err(status) => status,
      },
      Err(error) => {
        let message = format!("failed to encode blocking callback result: {}", error.msg);
        record_blocking_failure(&blocking, message.clone());
        match store_blocking_host_buffer(&blocking, message.into_bytes(), out) {
          Ok(()) => async_status::CALLBACK_ERROR,
          Err(status) => status,
        }
      }
    },
    Err(error) => {
      let _ = display_stack(
        &format!("[Error] blocking FFI callback failed: {}", error.msg),
        &error.stack,
        error.location.as_ref(),
      );
      let message = format!("Calcit callback failed: {}", error.msg);
      record_blocking_failure(&blocking, message.clone());
      match store_blocking_host_buffer(&blocking, message.into_bytes(), out) {
        Ok(()) => async_status::CALLBACK_ERROR,
        Err(status) => status,
      }
    }
  }
}

unsafe extern "C" fn native_blocking_finish(context: u64, task_handle: u64) -> i32 {
  std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let runtime = match native_async_runtime() {
      Ok(runtime) => runtime,
      Err(_) => return async_status::HOST_CLOSING,
    };
    let (handle, _, blocking) = match resolve_native_blocking_task(runtime, context, task_handle) {
      Ok(task) => task,
      Err(status) => return status,
    };
    match runtime.registry.finish(handle) {
      Ok(()) => async_status::OK,
      Err(error) => {
        record_blocking_failure(&blocking, error.to_string());
        error.status_code()
      }
    }
  }))
  .unwrap_or(async_status::INTERNAL_ERROR)
}

unsafe extern "C" fn native_blocking_free_buffer(context: u64, task_handle: u64, buffer: FfiBuffer) -> i32 {
  std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let runtime = match native_async_runtime() {
      Ok(runtime) => runtime,
      Err(_) => return async_status::HOST_CLOSING,
    };
    let (_, _, blocking) = match resolve_native_blocking_task(runtime, context, task_handle) {
      Ok(task) => task,
      Err(status) => return status,
    };
    match free_blocking_host_buffer(&blocking, buffer) {
      Ok(()) => async_status::OK,
      Err(status) => {
        record_blocking_failure(&blocking, format!("blocking FFI rejected host buffer free with status {status}"));
        status
      }
    }
  }))
  .unwrap_or(async_status::INTERNAL_ERROR)
}

unsafe extern "C" fn native_async_enqueue(
  context: u64,
  task_handle: u64,
  event_kind: u32,
  response_handle: u64,
  payload_ptr: *const u8,
  payload_len: usize,
) -> i32 {
  std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let runtime = match native_async_runtime() {
      Ok(runtime) => runtime,
      Err(_) => return async_status::HOST_CLOSING,
    };
    // SAFETY: the C caller promises that a non-empty payload remains readable
    // for this call. The helper validates length/null and copies immediately.
    unsafe { enqueue_native_async_event(runtime, context, task_handle, event_kind, response_handle, payload_ptr, payload_len) }
  }))
  .unwrap_or(async_status::INTERNAL_ERROR)
}

unsafe fn enqueue_native_async_event(
  runtime: &NativeAsyncRuntime,
  context: u64,
  task_handle: u64,
  event_kind: u32,
  response_handle: u64,
  payload_ptr: *const u8,
  payload_len: usize,
) -> i32 {
  let kind = match FfiAsyncEventKind::try_from(event_kind) {
    Ok(kind) => kind,
    Err(error) => return error.status_code(),
  };
  // SAFETY: forwarded from the C entry point's documented pointer contract.
  let payload = match unsafe { copy_async_payload(payload_ptr, payload_len) } {
    Ok(payload) => payload,
    Err(error) => return error.status_code(),
  };
  let task_handle = FfiAsyncHandle::from_raw(task_handle);
  if context != task_handle.raw() {
    return async_status::INVALID_HANDLE;
  }
  if !matches!(runtime.registry.clone_value(task_handle), Ok(NativeAsyncResource::Task(_))) {
    return async_status::INVALID_HANDLE;
  }
  let response_handle = if response_handle == FfiAsyncHandle::INVALID.raw() {
    None
  } else {
    Some(FfiAsyncHandle::from_raw(response_handle))
  };
  if let Some(response_handle) = response_handle {
    let response = match runtime.registry.clone_value(response_handle) {
      Ok(NativeAsyncResource::Response(response)) if response.owner_task == task_handle => response,
      Ok(_) => return async_status::INVALID_HANDLE,
      Err(error) => return error.status_code(),
    };
    if response.deadline <= Instant::now() {
      return async_status::HANDLE_FINISHED;
    }
  }

  match runtime
    .queue
    .enqueue(&runtime.registry, task_handle, response_handle, kind, payload)
  {
    Ok(outcome) => {
      let (queued_events, queued_bytes) = runtime.queue.usage().unwrap_or((0, 0));
      let task_metrics = runtime.queue.task_metrics(task_handle).ok().flatten().unwrap_or_default();
      trace_ffi_event(
        "async-enqueue",
        format!(
          "task={} kind={kind:?} sequence={} disposition={:?} global_queued_events={queued_events} global_queued_bytes={queued_bytes} {} producer={:?}",
          task_handle.raw(),
          outcome.sequence,
          outcome.disposition,
          format_task_queue_metrics(task_metrics),
          thread::current().id()
        ),
      );
      async_status::OK
    }
    Err(error) => {
      let (queued_events, queued_bytes) = runtime.queue.usage().unwrap_or((0, 0));
      let task_metrics = runtime.queue.task_metrics(task_handle).ok().flatten().unwrap_or_default();
      trace_ffi_event(
        "async-enqueue-rejected",
        format!(
          "task={} kind={kind:?} status={} global_queued_events={queued_events} global_queued_bytes={queued_bytes} {} error={error}",
          task_handle.raw(),
          error.status_code(),
          format_task_queue_metrics(task_metrics)
        ),
      );
      error.status_code()
    }
  }
}

unsafe extern "C" fn native_async_configure_task(
  context: u64,
  task_handle: u64,
  kind: u32,
  flags: u32,
  task_context: u64,
  cancel: Option<FfiAsyncTaskCancel>,
) -> i32 {
  std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    if context != task_handle {
      return async_status::INVALID_HANDLE;
    }
    let runtime = match native_async_runtime() {
      Ok(runtime) => runtime,
      Err(_) => return async_status::HOST_CLOSING,
    };
    let kind = match FfiAsyncHandleKind::try_from(kind) {
      Ok(kind) if kind != FfiAsyncHandleKind::Response => kind,
      Ok(_) => return async_status::INVALID_PAYLOAD,
      Err(error) => return error.status_code(),
    };
    if kind == FfiAsyncHandleKind::Server && cancel.is_none() {
      return async_status::INVALID_PAYLOAD;
    }
    let resource = match runtime.registry.clone_value(FfiAsyncHandle::from_raw(task_handle)) {
      Ok(NativeAsyncResource::Task(task)) => task,
      Ok(_) => return async_status::INVALID_HANDLE,
      Err(error) => return error.status_code(),
    };
    if let Err(error) = runtime.registry.configure(FfiAsyncHandle::from_raw(task_handle), kind, flags) {
      return error.status_code();
    }
    let mut control = match resource.control.lock() {
      Ok(control) => control,
      Err(_) => return async_status::INTERNAL_ERROR,
    };
    control.control = cancel.map(|cancel| NativeAsyncTaskControl {
      context: task_context,
      cancel,
    });
    trace_ffi_event(
      "async-configure",
      format!("task={task_handle} kind={kind:?} flags=0x{flags:x} cancel={}", cancel.is_some()),
    );
    async_status::OK
  }))
  .unwrap_or(async_status::INTERNAL_ERROR)
}

unsafe extern "C" fn native_async_open_response(
  context: u64,
  task_handle: u64,
  response_context: u64,
  timeout_ms: u64,
  resolve: Option<FfiAsyncResponseResolve>,
  out_handle: *mut u64,
) -> i32 {
  std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    if context != task_handle {
      return async_status::INVALID_HANDLE;
    }
    if out_handle.is_null() || timeout_ms == 0 || timeout_ms > MAX_ASYNC_RESPONSE_TIMEOUT_MS {
      return async_status::INVALID_PAYLOAD;
    }
    let Some(resolve) = resolve else {
      return async_status::INVALID_PAYLOAD;
    };
    let runtime = match native_async_runtime() {
      Ok(runtime) => runtime,
      Err(_) => return async_status::HOST_CLOSING,
    };
    let task_handle = FfiAsyncHandle::from_raw(task_handle);
    let task_state = match runtime.registry.state(task_handle) {
      Ok(state) => state,
      Err(error) => return error.status_code(),
    };
    if task_state.kind != FfiAsyncHandleKind::Server
      || task_state.flags & ASYNC_TASK_FLAG_REQUIRES_RESPONSE == 0
      || task_state.lifecycle != calcit::ffi_abi::FfiAsyncLifecycle::Active
    {
      return async_status::INVALID_HANDLE;
    }
    let Some(deadline) = Instant::now().checked_add(Duration::from_millis(timeout_ms)) else {
      return async_status::INVALID_PAYLOAD;
    };
    let response = NativeAsyncResponse {
      owner_task: task_handle,
      context: response_context,
      deadline,
      resolve,
    };
    let mut responses = match runtime.responses.lock() {
      Ok(responses) => responses,
      Err(_) => return async_status::INTERNAL_ERROR,
    };
    let response_handle = match runtime.registry.register_for_active_owner(
      task_handle,
      FfiAsyncHandleKind::Response,
      NativeAsyncResource::Response(response),
    ) {
      Ok(handle) => handle,
      Err(error) => return error.status_code(),
    };
    if responses.insert(response_handle, response).is_err() {
      let _ = runtime.registry.finish(response_handle);
      let _ = runtime.registry.release(response_handle);
      return async_status::QUEUE_FULL;
    }
    drop(responses);
    // SAFETY: the C caller provided a non-null writable out pointer for this
    // synchronous call; only one fixed-width value is written.
    unsafe { out_handle.write(response_handle.raw()) };
    trace_ffi_event(
      "async-response-open",
      format!(
        "task={} response={} timeout_ms={timeout_ms}",
        task_handle.raw(),
        response_handle.raw()
      ),
    );
    async_status::OK
  }))
  .unwrap_or(async_status::INTERNAL_ERROR)
}

fn async_capability(handle: FfiAsyncHandle, kind: FfiAsyncHandleKind) -> Calcit {
  Calcit::AnyRef(EdnAnyRef::new(NativeAsyncCapability { handle, kind }))
}

fn read_native_async_capability(value: &Calcit) -> Result<NativeAsyncCapability, CalcitErr> {
  let Calcit::AnyRef(reference) = value else {
    return Err(CalcitErr::use_str(
      CalcitErrKind::Type,
      format!("expected async FFI capability, got: {value}"),
    ));
  };
  let guard = reference
    .0
    .read()
    .map_err(|_| CalcitErr::use_str(CalcitErrKind::Unexpected, "async FFI capability lock is poisoned"))?;
  let Some(capability) = guard.as_any().downcast_ref::<NativeAsyncCapability>().copied() else {
    return Err(CalcitErr::use_str(CalcitErrKind::Type, "expected async FFI capability"));
  };
  Ok(capability)
}

fn read_async_capability(value: &Calcit, expected: FfiAsyncHandleKind) -> Result<NativeAsyncCapability, CalcitErr> {
  let capability = read_native_async_capability(value)?;
  if capability.kind != expected {
    return Err(CalcitErr::use_str(
      CalcitErrKind::Type,
      format!("expected async FFI {expected:?} capability, got {:?}", capability.kind),
    ));
  }
  Ok(capability)
}

fn read_async_task_capability(value: &Calcit) -> Result<NativeAsyncCapability, CalcitErr> {
  let capability = read_native_async_capability(value)?;
  if capability.kind == FfiAsyncHandleKind::Response {
    return Err(CalcitErr::use_str(
      CalcitErrKind::Type,
      "expected async FFI task capability, got response capability",
    ));
  }
  Ok(capability)
}

fn encode_async_value(value: &Calcit) -> Result<Vec<u8>, CalcitErr> {
  if matches!(value, Calcit::Unit) {
    return Ok(b"&unit".to_vec());
  }
  let value = calcit_to_edn(value)?;
  cirru_edn::format(&value, true)
    .map(String::into_bytes)
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, format!("failed to encode async FFI value: {error}")))
}

fn resolve_async_response_with(
  runtime: &NativeAsyncRuntime,
  capability: NativeAsyncCapability,
  outcome: u32,
  payload: &[u8],
) -> Result<(), String> {
  let response = claim_async_response(runtime, capability.handle)?;
  if response.deadline <= Instant::now() {
    let reject_error =
      finish_claimed_response(runtime, capability.handle, response, ASYNC_RESPONSE_REJECT, b"{} (:code :timeout)").err();
    return Err(match reject_error {
      Some(error) => format!("async FFI response capability expired; timeout rejection failed: {error}"),
      None => "async FFI response capability has expired".to_owned(),
    });
  }
  finish_claimed_response(runtime, capability.handle, response, outcome, payload)
}

fn claim_async_response(runtime: &NativeAsyncRuntime, handle: FfiAsyncHandle) -> Result<NativeAsyncResponse, String> {
  let state = runtime.registry.state(handle).map_err(|error| error.to_string())?;
  if state.kind != FfiAsyncHandleKind::Response {
    return Err("async FFI capability does not reference a response".to_owned());
  }
  runtime.registry.begin_close(handle).map_err(|error| error.to_string())?;
  let NativeAsyncResource::Response(response) = runtime.registry.clone_value(handle).map_err(|error| error.to_string())? else {
    return Err("async FFI capability does not reference a response".to_owned());
  };
  runtime
    .responses
    .lock()
    .map_err(|_| "async FFI response index lock is poisoned".to_owned())?
    .remove(handle);
  Ok(response)
}

fn finish_claimed_response(
  runtime: &NativeAsyncRuntime,
  handle: FfiAsyncHandle,
  response: NativeAsyncResponse,
  outcome: u32,
  payload: &[u8],
) -> Result<(), String> {
  let status = unsafe { (response.resolve)(response.context, handle.raw(), outcome, payload.as_ptr(), payload.len()) };
  runtime.registry.finish(handle).map_err(|error| error.to_string())?;
  match runtime.registry.release(handle) {
    Ok(NativeAsyncResource::Response(_)) => {}
    Ok(NativeAsyncResource::Task(_)) => {
      return Err("async FFI response released a task resource".to_owned());
    }
    Err(error) => return Err(error.to_string()),
  }
  trace_ffi_event(
    "async-response-finish",
    format!("task={} response={} outcome={outcome}", response.owner_task.raw(), handle.raw()),
  );
  if status == async_status::OK {
    Ok(())
  } else {
    Err(format!(
      "async FFI response {} was rejected by the module with status {status}",
      handle.raw()
    ))
  }
}

fn resolve_async_response(capability: NativeAsyncCapability, outcome: u32, payload: &[u8]) -> Result<Calcit, CalcitErr> {
  let runtime = native_async_runtime().map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?;
  resolve_async_response_with(runtime, capability, outcome, payload)
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?;
  Ok(Calcit::Unit)
}

fn ffi_response_resolve(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&ffi-response-resolve expected 2 arguments, got: {xs:?}"),
    );
  }
  let capability = read_async_capability(&xs[0], FfiAsyncHandleKind::Response)?;
  let payload = encode_async_value(&xs[1])?;
  resolve_async_response(capability, ASYNC_RESPONSE_RESOLVE, &payload)
}

fn ffi_response_reject(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&ffi-response-reject expected 2 arguments, got: {xs:?}"),
    );
  }
  let capability = read_async_capability(&xs[0], FfiAsyncHandleKind::Response)?;
  let payload = encode_async_value(&xs[1])?;
  resolve_async_response(capability, ASYNC_RESPONSE_REJECT, &payload)
}

fn reject_response_for_host(runtime: &NativeAsyncRuntime, handle: FfiAsyncHandle, reason: &[u8]) -> Option<String> {
  let response = match claim_async_response(runtime, handle) {
    Ok(response) => response,
    Err(error) => return Some(format!("response {} claim failed: {error}", handle.raw())),
  };
  finish_claimed_response(runtime, handle, response, ASYNC_RESPONSE_REJECT, reason).err()
}

fn expire_async_responses(runtime: &NativeAsyncRuntime) -> Result<(), String> {
  let now = Instant::now();
  let handles = runtime
    .responses
    .lock()
    .map_err(|_| "async FFI response index lock is poisoned".to_owned())?
    .take_due(now);
  for handle in handles {
    if let Ok(NativeAsyncResource::Response(response)) = runtime.registry.clone_value(handle)
      && let Ok(NativeAsyncResource::Task(task)) = runtime.registry.clone_value(response.owner_task)
    {
      update_task_outcomes(&task, |outcomes| {
        outcomes.deadline_timeout_total = outcomes.deadline_timeout_total.saturating_add(1);
      })?;
    }
    if let Some(error) = reject_response_for_host(runtime, handle, b"{} (:code :timeout)") {
      eprintln!("[Error] async FFI response timeout: {error}");
    } else {
      eprintln!("[Error] async FFI response {} timed out", handle.raw());
    }
  }
  Ok(())
}

fn reject_owned_responses(runtime: &NativeAsyncRuntime, owner: FfiAsyncHandle, reason: &[u8]) -> Result<(), String> {
  let handles = runtime
    .responses
    .lock()
    .map_err(|_| "async FFI response index lock is poisoned".to_owned())?
    .take_owner(owner);
  for handle in handles {
    if let Some(error) = reject_response_for_host(runtime, handle, reason) {
      eprintln!("[Error] async FFI response cleanup: {error}");
    }
  }
  Ok(())
}

fn discard_owned_responses(runtime: &NativeAsyncRuntime, owner: FfiAsyncHandle) -> Result<usize, String> {
  let mut discarded = 0;
  let handles = runtime
    .responses
    .lock()
    .map_err(|_| "async FFI response index lock is poisoned".to_owned())?
    .take_owner(owner);
  for handle in handles {
    let _ = runtime.registry.finish(handle);
    if runtime.registry.release(handle).is_ok() {
      discarded += 1;
    }
  }
  Ok(discarded)
}

fn begin_async_task_cancel(runtime: &NativeAsyncRuntime, handle: FfiAsyncHandle) -> Result<usize, String> {
  runtime.registry.begin_close(handle).map_err(|error| error.to_string())?;
  runtime.queue.discard_handle_events(handle).map_err(|error| error.to_string())
}

fn ffi_task_cancel(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() || xs.len() > 2 {
    return CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&ffi-task-cancel expected 1 or 2 arguments, got: {xs:?}"),
    );
  }
  let capability = read_async_task_capability(&xs[0])?;
  let reason = if xs.len() == 2 {
    encode_async_value(&xs[1])?
  } else {
    b"{} (:code :cancelled)".to_vec()
  };
  let runtime = native_async_runtime().map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?;
  let state = runtime
    .registry
    .state(capability.handle)
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error.to_string()))?;
  if state.lifecycle == calcit::ffi_abi::FfiAsyncLifecycle::Closing {
    return Ok(Calcit::Unit);
  }
  let NativeAsyncResource::Task(task) = runtime
    .registry
    .clone_value(capability.handle)
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error.to_string()))?
  else {
    return CalcitErr::err_str(CalcitErrKind::Unexpected, "async FFI capability does not reference a task");
  };
  let control = task
    .control
    .lock()
    .map_err(|_| CalcitErr::use_str(CalcitErrKind::Unexpected, "async FFI task control lock is poisoned"))?
    .control
    .ok_or_else(|| CalcitErr::use_str(CalcitErrKind::Unexpected, "async FFI task does not provide cancellation"))?;
  let purged =
    begin_async_task_cancel(runtime, capability.handle).map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?;
  update_task_outcomes(&task, |outcomes| {
    outcomes.cancel_requested_total = outcomes.cancel_requested_total.saturating_add(1);
  })
  .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?;
  let status = unsafe { (control.cancel)(control.context, capability.handle.raw(), reason.as_ptr(), reason.len()) };
  trace_ffi_event(
    "async-task-cancel",
    format!("task={} status={status} purged={purged}", capability.handle.raw()),
  );
  if status == async_status::OK {
    update_task_outcomes(&task, |outcomes| {
      outcomes.cancel_succeeded_total = outcomes.cancel_succeeded_total.saturating_add(1);
    })
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?;
    Ok(Calcit::Unit)
  } else {
    update_task_outcomes(&task, |outcomes| {
      outcomes.cancel_failed_total = outcomes.cancel_failed_total.saturating_add(1);
    })
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?;
    let _ = runtime.queue.discard_handle_events(capability.handle);
    let _ = reject_owned_responses(runtime, capability.handle, b"{} (:code :cancel-failed)");
    let _ = runtime.registry.finish(capability.handle);
    let metrics = runtime
      .queue
      .take_task_metrics(capability.handle)
      .ok()
      .flatten()
      .unwrap_or_default();
    if let Ok(NativeAsyncResource::Task(released_task)) = runtime.registry.release(capability.handle) {
      if let Err(error) = archive_task_metrics(runtime, &released_task, metrics) {
        eprintln!("[Warn] {error}");
      }
      track::track_task_release();
    }
    trace_ffi_event(
      "async-task-cancel-failed",
      format!(
        "task={} status={status} {}",
        capability.handle.raw(),
        format_task_queue_metrics(metrics)
      ),
    );
    CalcitErr::err_str(
      CalcitErrKind::Unexpected,
      format!(
        "async FFI task {} cancellation failed with status {status}",
        capability.handle.raw()
      ),
    )
  }
}

fn decode_async_emit(payload: &[u8]) -> Result<Vec<Calcit>, String> {
  let source = std::str::from_utf8(payload).map_err(|error| format!("async FFI event payload is not UTF-8: {error}"))?;
  let value = cirru_edn::parse(source).map_err(|error| format!("async FFI event payload is not valid Cirru EDN: {error}"))?;
  let Edn::List(cirru_edn::EdnListView(args)) = value else {
    return Err(format!("async FFI Emit payload must be a Cirru EDN list, got: {value}"));
  };
  Ok(args.iter().map(|arg| edn_to_calcit(arg, &Calcit::Nil)).collect())
}

fn validate_async_completion(payload: &[u8]) -> Result<(), String> {
  let source = std::str::from_utf8(payload).map_err(|error| format!("async FFI completion payload is not UTF-8: {error}"))?;
  if source.trim() == "&unit" {
    Ok(())
  } else {
    Err(format!(
      "async FFI Complete payload must be explicit `&unit`, got: {}",
      source.trim()
    ))
  }
}

fn decode_async_failure(payload: &[u8]) -> Result<String, String> {
  let source = std::str::from_utf8(payload).map_err(|error| format!("async FFI failure payload is not UTF-8: {error}"))?;
  let value = cirru_edn::parse(source).map_err(|error| format!("async FFI failure payload is not valid Cirru EDN: {error}"))?;
  cirru_edn::format(&sanitize_edn_for_format(&value), true)
    .map(|message| message.trim().to_owned())
    .map_err(|error| format!("failed to format async FFI failure payload: {error}"))
}

fn dispatch_native_async_event(runtime: &NativeAsyncRuntime, event: &calcit::ffi_async::FfiAsyncQueuedEvent) -> Result<(), String> {
  let handle = event.task_handle();
  let NativeAsyncResource::Task(task) = runtime.registry.clone_value(handle).map_err(|error| error.to_string())? else {
    return Err("async FFI event targeted a non-task handle".to_owned());
  };
  let kind = event.kind().map_err(|error| error.to_string())?;
  trace_ffi_event(
    "async-drain",
    format!(
      "lib={} symbol={} task={} kind={kind:?} sequence={} producer={:?} queued_ms={:.3}",
      task.lib_name,
      task.method,
      handle.raw(),
      event.descriptor.sequence,
      event.producer_thread(),
      event.queued_for().as_secs_f64() * 1000.0
    ),
  );

  match kind {
    FfiAsyncEventKind::Emit => {
      let mut args = decode_async_emit(event.payload())?;
      if event.descriptor.response_handle != FfiAsyncHandle::INVALID.raw() {
        let response_handle = FfiAsyncHandle::from_raw(event.descriptor.response_handle);
        let response = match runtime.registry.clone_value(response_handle) {
          Ok(NativeAsyncResource::Response(response)) => response,
          Ok(NativeAsyncResource::Task(_)) => return Err("async FFI event carried a non-response handle".to_owned()),
          Err(calcit::ffi_abi::FfiAsyncHandleError::StaleHandle | calcit::ffi_abi::FfiAsyncHandleError::HandleFinished) => {
            trace_ffi_event(
              "async-response-skip",
              format!(
                "task={} response={} sequence={} reason=expired-or-finished",
                handle.raw(),
                response_handle.raw(),
                event.descriptor.sequence
              ),
            );
            return Ok(());
          }
          Err(error) => return Err(error.to_string()),
        };
        if response.owner_task != handle {
          return Err("async FFI response handle belongs to another task".to_owned());
        }
        if response.deadline <= Instant::now() {
          if let Some(error) = reject_response_for_host(runtime, response_handle, b"{} (:code :timeout)") {
            eprintln!("[Error] async FFI response expiry at dispatch: {error}");
          }
          return Ok(());
        }
        args.push(async_capability(response_handle, FfiAsyncHandleKind::Response));
      }
      let Calcit::Fn { info, .. } = &task.callback else {
        return Err("async FFI task lost its Calcit callback".to_owned());
      };
      match runner::run_fn(&args, info, &task.stack) {
        Ok(ret) => {
          trace_ffi_event(
            "async-callback-out",
            format!(
              "lib={} symbol={} task={} sequence={} ret={}",
              task.lib_name,
              task.method,
              handle.raw(),
              event.descriptor.sequence,
              ret.turn_string()
            ),
          );
          Ok(())
        }
        Err(error) => {
          let _ = display_stack(
            &format!("[Error] async FFI callback failed: {}", error.msg),
            &error.stack,
            error.location.as_ref(),
          );
          Err(format!("Calcit callback failed: {}", error.msg))
        }
      }
    }
    FfiAsyncEventKind::Complete => validate_async_completion(event.payload()),
    FfiAsyncEventKind::Fail => Err(format!("async FFI task failed: {}", decode_async_failure(event.payload())?)),
  }
}

fn drain_async_events_from(runtime: &NativeAsyncRuntime, limit: usize) -> Result<FfiAsyncDrainReport, String> {
  expire_async_responses(runtime)?;
  let mut report = runtime
    .queue
    .drain(&runtime.registry, limit, |event| dispatch_native_async_event(runtime, event))
    .map_err(|error| error.to_string())?;

  for failure in &report.callback_failures {
    eprintln!(
      "[Error] async FFI task {} sequence {}: {}",
      failure.descriptor.task_handle, failure.descriptor.sequence, failure.message
    );
  }
  for failure in &report.lifecycle_failures {
    eprintln!(
      "[Error] async FFI lifecycle task {} sequence {}: {}",
      failure.descriptor.task_handle, failure.descriptor.sequence, failure.error
    );
  }
  for failure in &report.queue_failures {
    eprintln!(
      "[Error] async FFI queue task {} sequence {}: {}",
      failure.descriptor.task_handle, failure.descriptor.sequence, failure.error
    );
  }

  for descriptor in report.finished.clone() {
    let handle = FfiAsyncHandle::from_raw(descriptor.task_handle);
    reject_owned_responses(runtime, handle, b"{} (:code :owner-finished)")?;
    let task = match runtime.registry.release(handle) {
      Ok(NativeAsyncResource::Task(task)) => task,
      Ok(NativeAsyncResource::Response(_)) => {
        return Err(format!("async FFI task {} released a response resource", handle.raw()));
      }
      Err(error) => {
        report
          .lifecycle_failures
          .push(calcit::ffi_async::FfiAsyncLifecycleFailure { descriptor, error });
        continue;
      }
    };
    let metrics = runtime
      .queue
      .take_task_metrics(handle)
      .map_err(|error| error.to_string())?
      .unwrap_or_default();
    archive_task_metrics(runtime, &task, metrics)?;
    track::track_task_release();
    trace_ffi_event(
      "async-task-release",
      format!(
        "lib={} symbol={} task={} sequence={} {} pending={}",
        task.lib_name,
        task.method,
        handle.raw(),
        descriptor.sequence,
        format_task_queue_metrics(metrics),
        track::count_pending_tasks()
      ),
    );
  }

  Ok(report)
}

pub fn drain_async_events(limit: usize) -> Result<FfiAsyncDrainReport, String> {
  drain_async_events_from(native_async_runtime()?, limit)
}

fn run_pending_ctrl_c_callback() -> Result<(), String> {
  if !CTRL_C_CALLBACK_PENDING.swap(false, Ordering::AcqRel) {
    return Ok(());
  }
  let callback = CTRL_C_CALLBACK
    .lock()
    .map_err(|_| "Ctrl-C callback lock is poisoned".to_owned())?
    .clone();
  let Some((callback, stack)) = callback else {
    return Ok(());
  };
  let Calcit::Fn { info, .. } = callback.as_ref() else {
    return Err("registered Ctrl-C callback is not a function".to_owned());
  };
  runner::run_fn_during_shutdown(&[], info, &stack)
    .map(|_| ())
    .map_err(|error| format!("Ctrl-C callback failed: {}", error.msg))
}

fn reject_response_during_shutdown(
  runtime: &NativeAsyncRuntime,
  handle: FfiAsyncHandle,
  response: NativeAsyncResponse,
) -> Result<(), String> {
  runtime
    .responses
    .lock()
    .map_err(|_| "async FFI response index lock is poisoned".to_owned())?
    .remove(handle);
  let reason = b"{} (:code :host-shutdown)";
  let status = unsafe { (response.resolve)(response.context, handle.raw(), ASYNC_RESPONSE_REJECT, reason.as_ptr(), reason.len()) };
  runtime.registry.finish(handle).map_err(|error| error.to_string())?;
  match runtime.registry.release(handle).map_err(|error| error.to_string())? {
    NativeAsyncResource::Response(_) => {}
    NativeAsyncResource::Task(_) => return Err(format!("async FFI response {} released a task resource", handle.raw())),
  }
  trace_ffi_event(
    "async-response-shutdown",
    format!("task={} response={} status={status}", response.owner_task.raw(), handle.raw()),
  );
  if status == async_status::OK {
    Ok(())
  } else {
    Err(format!(
      "async FFI response {} shutdown rejection failed with status {status}",
      handle.raw()
    ))
  }
}

fn begin_native_async_shutdown(runtime: &NativeAsyncRuntime) -> Result<usize, String> {
  let snapshot = runtime.registry.snapshot().map_err(|error| error.to_string())?;
  let queue_metrics: HashMap<FfiAsyncHandle, FfiAsyncTaskQueueMetrics> = runtime
    .queue
    .task_metrics_snapshot()
    .map_err(|error| error.to_string())?
    .into_iter()
    .collect();
  let mut module_metrics: BTreeMap<(String, String), NativeAsyncModuleQueueMetrics> = BTreeMap::new();
  for (handle, state, resource) in &snapshot {
    if let NativeAsyncResource::Task(task) = resource {
      module_metrics
        .entry((task.lib_name.clone(), task.method.clone()))
        .or_default()
        .add_task(state.lifecycle, queue_metrics.get(handle).copied().unwrap_or_default());
    }
  }
  for ((lib_name, method), metrics) in module_metrics {
    trace_ffi_event(
      "async-shutdown-summary",
      format!(
        "lib={lib_name} symbol={method} active={} closing={} queued_events={} queued_bytes={} oldest_ms={} accepted={} coalesced={} queue_full={} dequeued={} purged={}",
        metrics.active_tasks,
        metrics.closing_tasks,
        metrics.queued_events,
        metrics.queued_bytes,
        format_oldest_age(metrics.oldest_age),
        metrics.accepted_total,
        metrics.coalesced_total,
        metrics.queue_full_total,
        metrics.dequeued_total,
        metrics.purged_total
      ),
    );
  }
  let pending = runtime.registry.begin_shutdown().map_err(|error| error.to_string())?;

  for (handle, state, resource) in &snapshot {
    if state.kind == FfiAsyncHandleKind::Response
      && let NativeAsyncResource::Response(response) = resource
      && let Err(error) = reject_response_during_shutdown(runtime, *handle, *response)
    {
      eprintln!("[Error] {error}");
    }
  }

  let reason = b"{} (:code :host-shutdown)";
  for (handle, state, resource) in snapshot {
    if state.kind == FfiAsyncHandleKind::Response || state.lifecycle != calcit::ffi_abi::FfiAsyncLifecycle::Active {
      continue;
    }
    let NativeAsyncResource::Task(task) = resource else {
      continue;
    };
    let control = task
      .control
      .lock()
      .map_err(|_| format!("async FFI task {} control lock is poisoned", handle.raw()))?
      .control;
    let Some(control) = control else {
      trace_ffi_event(
        "async-shutdown-wait",
        format!(
          "lib={} symbol={} task={} kind={:?} cancellable=false {}",
          task.lib_name,
          task.method,
          handle.raw(),
          state.kind,
          format_task_queue_metrics(queue_metrics.get(&handle).copied().unwrap_or_default())
        ),
      );
      continue;
    };
    update_task_outcomes(&task, |outcomes| {
      outcomes.cancel_requested_total = outcomes.cancel_requested_total.saturating_add(1);
    })?;
    let status = unsafe { (control.cancel)(control.context, handle.raw(), reason.as_ptr(), reason.len()) };
    update_task_outcomes(&task, |outcomes| {
      if status == async_status::OK || status == async_status::HANDLE_FINISHED {
        outcomes.cancel_succeeded_total = outcomes.cancel_succeeded_total.saturating_add(1);
      } else {
        outcomes.cancel_failed_total = outcomes.cancel_failed_total.saturating_add(1);
      }
    })?;
    trace_ffi_event(
      "async-shutdown-cancel",
      format!(
        "lib={} symbol={} task={} kind={:?} status={status} {}",
        task.lib_name,
        task.method,
        handle.raw(),
        state.kind,
        format_task_queue_metrics(queue_metrics.get(&handle).copied().unwrap_or_default())
      ),
    );
    if status != async_status::OK && status != async_status::HANDLE_FINISHED {
      eprintln!(
        "[Error] async FFI shutdown cancellation failed: lib={} symbol={} task={} status={status}",
        task.lib_name,
        task.method,
        handle.raw()
      );
    }
  }
  trace_ffi_event("async-shutdown-begin", format!("pending={}", pending.len()));
  Ok(pending.len())
}

fn force_cleanup_native_async(runtime: &NativeAsyncRuntime, release_tracked_tasks: bool) -> Result<usize, String> {
  let snapshot = runtime.registry.snapshot().map_err(|error| error.to_string())?;
  let mut forced = 0;

  for (handle, state, resource) in &snapshot {
    if state.kind == FfiAsyncHandleKind::Response
      && let NativeAsyncResource::Response(response) = resource
      && let Err(error) = reject_response_during_shutdown(runtime, *handle, *response)
    {
      eprintln!("[Error] {error}");
    }
  }

  for (handle, state, resource) in snapshot {
    if state.kind == FfiAsyncHandleKind::Response {
      continue;
    }
    let NativeAsyncResource::Task(task) = resource else {
      continue;
    };
    let unfinished = state.lifecycle != calcit::ffi_abi::FfiAsyncLifecycle::Finished;
    let purged = runtime.queue.discard_handle_events(handle).map_err(|error| error.to_string())?;
    let discarded_responses = discard_owned_responses(runtime, handle)?;
    if state.lifecycle != calcit::ffi_abi::FfiAsyncLifecycle::Finished {
      runtime.registry.finish(handle).map_err(|error| error.to_string())?;
    }
    match runtime.registry.release(handle).map_err(|error| error.to_string())? {
      NativeAsyncResource::Task(_) if release_tracked_tasks => track::track_task_release(),
      NativeAsyncResource::Task(_) => {}
      NativeAsyncResource::Response(_) => return Err(format!("async FFI task {} released a response resource", handle.raw())),
    }
    let metrics = runtime
      .queue
      .take_task_metrics(handle)
      .map_err(|error| error.to_string())?
      .unwrap_or_default();
    archive_task_metrics(runtime, &task, metrics)?;
    if unfinished {
      forced += 1;
      eprintln!(
        "[Warn] force-cleaned unfinished async FFI task: lib={} symbol={} task={} kind={:?} age_ms={:.3} purged={} responses={} {}",
        task.lib_name,
        task.method,
        handle.raw(),
        state.kind,
        task.started_at.elapsed().as_secs_f64() * 1000.0,
        purged,
        discarded_responses,
        format_task_queue_metrics(metrics)
      );
    }
  }
  Ok(forced)
}

fn shutdown_native_async_runtime(runtime: &NativeAsyncRuntime, grace: Duration, release_tracked_tasks: bool) -> Result<usize, String> {
  let requested = begin_native_async_shutdown(runtime)?;
  let deadline = Instant::now() + grace;
  while runtime.registry.pending_count().map_err(|error| error.to_string())? > 0 && Instant::now() < deadline {
    drain_async_events_from(runtime, 256)?;
    if runtime.registry.pending_count().map_err(|error| error.to_string())? == 0 {
      break;
    }
    let remaining = deadline.saturating_duration_since(Instant::now()).min(Duration::from_millis(40));
    if !remaining.is_zero() {
      runtime.queue.wait_for_event(remaining).map_err(|error| error.to_string())?;
    }
  }
  drain_async_events_from(runtime, 256)?;
  runtime.queue.close().map_err(|error| error.to_string())?;
  let forced = force_cleanup_native_async(runtime, release_tracked_tasks)?;
  trace_ffi_event("async-shutdown-complete", format!("requested={requested} forced={forced}"));
  Ok(forced)
}

pub fn exit_when_async_cleared() -> Result<(), String> {
  let runtime = native_async_runtime()?;
  let mut shutdown_complete = false;
  loop {
    if shutdown_requested() && !shutdown_complete {
      if let Err(error) = run_pending_ctrl_c_callback() {
        eprintln!("[Error] {error}");
      }
      shutdown_native_async_runtime(runtime, ASYNC_SHUTDOWN_GRACE, true)?;
      shutdown_complete = true;
    }
    drain_async_events(256)?;
    if track::count_pending_tasks() == 0 && runtime.queue.is_empty().map_err(|error| error.to_string())? {
      return Ok(());
    }
    if shutdown_complete {
      thread::sleep(Duration::from_millis(10));
      continue;
    }
    if runtime.queue.is_empty().map_err(|error| error.to_string())? {
      runtime
        .queue
        .wait_for_event(Duration::from_millis(40))
        .map_err(|error| error.to_string())?;
    }
  }
}

/// load dylib, cache it
fn load_dylib(lib_name: &str) -> Result<Arc<libloading::Library>, CalcitErr> {
  let resolved_path = resolve_trace_path(lib_name);
  let mut dylibs = DYLIBS
    .lock()
    .map_err(|_| CalcitErr::use_str(CalcitErrKind::Unexpected, "failed to lock dylib cache"))?;
  if let Some(lib) = dylibs.get(lib_name) {
    trace_ffi_event("reuse-dylib", format!("lib={lib_name} resolved={resolved_path}"));
    return Ok(lib.to_owned());
  }
  trace_ffi_event("load-dylib", format!("lib={lib_name} resolved={resolved_path}"));
  let lib = unsafe { libloading::Library::new(lib_name) }
    .map_err(|e| CalcitErr::use_str(CalcitErrKind::Unexpected, format!("failed to load dylib `{lib_name}`: {e}")))?;
  let lib = Arc::new(lib);
  dylibs.insert(lib_name.to_owned(), lib.to_owned());
  Ok(lib)
}

fn c_safe_migration_error(kind: &str, protocol: &str, lib_name: &str, method: &str, expected: &str) -> CalcitErr {
  CalcitErr::use_str(
    CalcitErrKind::Unexpected,
    format!(
      "FFI {kind} method `{method}` in `{lib_name}` does not provide C-safe {protocol}. Expected {expected}; the legacy Rust-ABI fallback has been removed. Upgrade and rebuild the native module."
    ),
  )
}

static PLATFORM_APIS_INJECTED: AtomicBool = AtomicBool::new(false);

#[allow(dead_code)]
pub fn inject_platform_apis() {
  if PLATFORM_APIS_INJECTED.swap(true, Ordering::Relaxed) {
    return;
  }
  builtins::register_import_proc_with_descriptor(
    "&call-dylib-edn",
    call_dylib_edn,
    RegisteredProcDescriptor {
      arity_min: 2,
      arity_max: None,
      platforms: vec![RegisteredProcPlatform::Native],
      stability: RegisteredProcStability::Public,
      docs_hint: Some(Arc::from("Fix: use native runtime and pass (lib-name method ...args).")),
      callback_last: false,
      tags: proc_tags(["interop", "io"]),
    },
  );
  let log_io = proc_tags(["log", "io"]);
  builtins::register_import_proc_with_descriptor(
    "echo",
    stdout_println,
    RegisteredProcDescriptor {
      tags: log_io.clone(),
      ..Default::default()
    },
  );
  builtins::register_import_proc_with_descriptor(
    "println",
    stdout_println,
    RegisteredProcDescriptor {
      tags: log_io.clone(),
      ..Default::default()
    },
  );
  builtins::register_import_proc_with_descriptor(
    "eprintln",
    stderr_println,
    RegisteredProcDescriptor {
      tags: log_io,
      ..Default::default()
    },
  );
  builtins::register_import_proc_with_descriptor(
    "&call-dylib-edn-fn",
    call_dylib_edn_fn,
    RegisteredProcDescriptor {
      arity_min: 3,
      arity_max: None,
      platforms: vec![RegisteredProcPlatform::Native],
      stability: RegisteredProcStability::Public,
      docs_hint: Some(Arc::from("Fix: use native runtime and put callback fn as last argument.")),
      callback_last: true,
      tags: proc_tags(["interop", "io"]),
    },
  );
  builtins::register_import_proc_with_descriptor(
    "&blocking-dylib-edn-fn",
    blocking_dylib_edn_fn,
    RegisteredProcDescriptor {
      arity_min: 3,
      arity_max: None,
      platforms: vec![RegisteredProcPlatform::Native],
      stability: RegisteredProcStability::Public,
      docs_hint: Some(Arc::from("Fix: use native runtime and put callback fn as last argument.")),
      callback_last: true,
      tags: proc_tags(["interop", "io"]),
    },
  );
  for (name, proc, arity_min, arity_max) in [
    (
      "&ffi-response-resolve",
      ffi_response_resolve as calcit::builtins::FnType,
      2,
      Some(2),
    ),
    ("&ffi-response-reject", ffi_response_reject as calcit::builtins::FnType, 2, Some(2)),
    ("&ffi-task-cancel", ffi_task_cancel as calcit::builtins::FnType, 1, Some(2)),
  ] {
    builtins::register_import_proc_with_descriptor(
      name,
      proc,
      RegisteredProcDescriptor {
        arity_min,
        arity_max,
        platforms: vec![RegisteredProcPlatform::Native],
        stability: RegisteredProcStability::Public,
        docs_hint: Some(Arc::from("Fix: use a capability received from native async FFI.")),
        callback_last: false,
        tags: proc_tags(["interop", "io"]),
      },
    );
  }
  builtins::register_import_proc_with_descriptor(
    "async-sleep",
    builtins::meta::async_sleep,
    RegisteredProcDescriptor {
      tags: proc_tags(["io"]),
      ..Default::default()
    },
  );
  builtins::register_import_proc_with_descriptor(
    "on-control-c",
    on_ctrl_c,
    RegisteredProcDescriptor {
      tags: proc_tags(["control", "io"]),
      ..Default::default()
    },
  );

  if !calcit::quiet_tool_output() {
    eprintln!("{}", "registered platform APIs".dimmed());
  }
}

// &call-dylib-edn
pub fn call_dylib_edn(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    return CalcitErr::err_str(CalcitErrKind::Arity, format!("&call-dylib-edn expected >2 arguments, got: {xs:?}"));
  }
  let lib_name: String = if let Calcit::Str(s) = &xs[0] {
    (**s).to_owned()
  } else {
    return CalcitErr::err_str(CalcitErrKind::Type, format!("&call-dylib-edn expected a lib_name, got: {}", xs[0]));
  };

  let method: String = if let Calcit::Str(s) = &xs[1] {
    (**s).to_owned()
  } else {
    return CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&call-dylib-edn expected a method name, got: {}", xs[1]),
    );
  };
  let mut ys: Vec<Edn> = Vec::with_capacity(xs.len());
  for v in xs.into_iter().skip(2) {
    ys.push(calcit_to_edn(&v)?);
  }

  trace_ffi_event(
    "call",
    format!(
      "lib={lib_name} resolved={} symbol={method} argc={} args={}",
      resolve_trace_path(&lib_name),
      ys.len(),
      format_edn_args_for_trace(&ys)
    ),
  );

  let lib = load_dylib(&lib_name)?;
  match calcit::ffi_abi::try_call_buffer(&lib, &lib_name, &method, ys.clone(), Some(trace_ffi_resource_event))
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?
  {
    Some(ret) => {
      trace_ffi_event(
        "return-buffer",
        format!(
          "lib={lib_name} symbol={method}_calcit_ffi_v1 ret={}",
          format_edn_args_for_trace(std::slice::from_ref(&ret))
        ),
      );
      Ok(edn_to_calcit(&ret, &Calcit::Nil))
    }
    None => {
      let symbol = calcit::ffi_abi::buffer_method_symbol(&method);
      trace_ffi_event("buffer-required", format!("lib={lib_name} symbol={symbol} missing=true"));
      Err(c_safe_migration_error(
        "synchronous",
        "buffer protocol v1",
        &lib_name,
        &method,
        &format!("`calcit_ffi_buffer_version`, `{symbol}`, and `calcit_ffi_buffer_free`"),
      ))
    }
  }
}

pub fn stdout_println(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let mut s = String::from("");
  for (idx, x) in xs.into_iter().enumerate() {
    if idx > 0 {
      s.push(' ');
    }
    s.push_str(&x.turn_string());
  }
  if SILENCE_PROGRAM_OUTPUT.load(Ordering::Relaxed) {
    return Ok(Calcit::Nil);
  }
  if STDOUT_TO_STDERR.load(Ordering::Relaxed) {
    eprintln!("{s}");
  } else {
    println!("{s}");
  }
  Ok(Calcit::Nil)
}

pub fn stderr_println(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let mut s = String::from("");
  for (idx, x) in xs.into_iter().enumerate() {
    if idx > 0 {
      s.push(' ');
    }
    s.push_str(&x.turn_string());
  }
  if !SILENCE_PROGRAM_OUTPUT.load(Ordering::Relaxed) {
    eprintln!("{s}");
  }
  Ok(Calcit::Nil)
}

fn try_start_async_callback_v1(
  lib: &libloading::Library,
  lib_name: &str,
  method: &str,
  args: Vec<Edn>,
  callback: Calcit,
  call_stack: &CallStackList,
) -> Result<Option<Calcit>, CalcitErr> {
  let Some(start) =
    calcit::ffi_abi::lookup_async_start(lib, lib_name, method).map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?
  else {
    return Ok(None);
  };
  let runtime = native_async_runtime().map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?;
  let request = calcit::ffi_abi::encode_buffer_request(args).map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?;
  let task = NativeAsyncTask {
    callback,
    stack: Arc::new(call_stack.to_owned()),
    lib_name: lib_name.to_owned(),
    method: method.to_owned(),
    control: Arc::new(Mutex::new(NativeAsyncTaskState::default())),
    blocking: None,
    started_at: Instant::now(),
  };
  let handle = runtime
    .registry
    .register_with_flags(
      FfiAsyncHandleKind::Stream,
      ASYNC_TASK_FLAG_SERIAL_EVENTS,
      NativeAsyncResource::Task(task),
    )
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error.to_string()))?;
  let descriptor = FfiAsyncTaskDescriptor::new(handle.raw(), FfiAsyncHandleKind::Stream as u32, ASYNC_TASK_FLAG_SERIAL_EVENTS);
  let host = async_host_table(handle);

  track::track_task_add();
  trace_ffi_event(
    "async-start",
    format!(
      "lib={lib_name} symbol={} task={} request_bytes={} pending={}",
      calcit::ffi_abi::async_method_symbol(method),
      handle.raw(),
      request.len(),
      track::count_pending_tasks()
    ),
  );
  let status = unsafe { start(request.as_ptr(), request.len(), &descriptor, &host) };
  if status == async_status::OK {
    let state = runtime
      .registry
      .state(handle)
      .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error.to_string()))?;
    let NativeAsyncResource::Task(task) = runtime
      .registry
      .clone_value(handle)
      .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error.to_string()))?
    else {
      return CalcitErr::err_str(CalcitErrKind::Unexpected, "async FFI start replaced its task resource").map(Some);
    };
    let cancellable = task
      .control
      .lock()
      .map_err(|_| CalcitErr::use_str(CalcitErrKind::Unexpected, "async FFI task control lock is poisoned"))?
      .control
      .is_some();
    return Ok(Some(if cancellable {
      async_capability(handle, state.kind)
    } else {
      Calcit::Unit
    }));
  }

  let _ = runtime.registry.begin_close(handle);
  let purged = runtime.queue.discard_handle_events(handle).unwrap_or(0);
  let discarded_responses = discard_owned_responses(runtime, handle).unwrap_or(0);
  let _ = runtime.registry.finish(handle);
  let metrics = runtime.queue.take_task_metrics(handle).ok().flatten().unwrap_or_default();
  if let Ok(NativeAsyncResource::Task(released_task)) = runtime.registry.release(handle)
    && let Err(error) = archive_task_metrics(runtime, &released_task, metrics)
  {
    eprintln!("[Warn] {error}");
  }
  track::track_task_release();
  trace_ffi_event(
    "async-start-failed",
    format!(
      "lib={lib_name} symbol={} task={} status={status} purged={purged} discarded_responses={discarded_responses} {} pending={}",
      calcit::ffi_abi::async_method_symbol(method),
      handle.raw(),
      format_task_queue_metrics(metrics),
      track::count_pending_tasks()
    ),
  );
  CalcitErr::err_str(
    CalcitErrKind::Unexpected,
    format!(
      "FFI async method `{}` in `{lib_name}` failed to start with status {status}",
      calcit::ffi_abi::async_method_symbol(method)
    ),
  )
  .map(Some)
}

/// pass callback function to FFI function, so it can call multiple times
/// currently for HTTP servers
pub fn call_dylib_edn_fn(xs: Vec<Calcit>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() < 3 {
    return CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&call-dylib-edn-fn expected >3 arguments, got: {xs:?}"),
    );
  }

  let lib_name: String = if let Calcit::Str(s) = &xs[0] {
    (**s).to_owned()
  } else {
    return CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&call-dylib-edn-fn expected a lib_name, got: {}", xs[0]),
    );
  };

  let method: String = if let Calcit::Str(s) = &xs[1] {
    (**s).to_owned()
  } else {
    return CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&call-dylib-edn-fn expected a method name, got: {}", xs[1]),
    );
  };
  let mut ys: Vec<Edn> = Vec::with_capacity(xs.len() - 2);
  let callback = xs[xs.len() - 1].to_owned();
  let size = xs.len();
  for (idx, v) in xs.iter().enumerate() {
    if idx > 1 && idx < size - 1 {
      ys.push(calcit_to_edn(v)?);
    }
  }
  if let Calcit::Fn { .. } = callback {
  } else {
    return CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("expected last argument to be callback fn, got: {callback}"),
    );
  }

  trace_ffi_event(
    "spawn-callback",
    format!(
      "lib={lib_name} resolved={} symbol={method} argc={} args={}",
      resolve_trace_path(&lib_name),
      ys.len(),
      format_edn_args_for_trace(&ys)
    ),
  );

  let lib = load_dylib(&lib_name)?;
  if let Some(result) = try_start_async_callback_v1(&lib, &lib_name, &method, ys.clone(), callback.clone(), call_stack)? {
    return Ok(result);
  }
  let symbol = calcit::ffi_abi::async_method_symbol(&method);
  trace_ffi_event("async-required", format!("lib={lib_name} symbol={symbol} missing=true"));
  Err(c_safe_migration_error(
    "callback",
    "async protocol v1",
    &lib_name,
    &method,
    &format!("`calcit_ffi_async_version` and `{symbol}`"),
  ))
}

fn release_blocking_task(runtime: &NativeAsyncRuntime, handle: FfiAsyncHandle) -> Result<(usize, Vec<String>), String> {
  match runtime.registry.state(handle).map_err(|error| error.to_string())?.lifecycle {
    calcit::ffi_abi::FfiAsyncLifecycle::Active | calcit::ffi_abi::FfiAsyncLifecycle::Closing => {
      runtime.registry.finish(handle).map_err(|error| error.to_string())?;
    }
    calcit::ffi_abi::FfiAsyncLifecycle::Finished => {}
  }
  let NativeAsyncResource::Task(task) = runtime.registry.release(handle).map_err(|error| error.to_string())? else {
    return Err("blocking FFI handle released a response resource".to_owned());
  };
  archive_task_metrics(runtime, &task, FfiAsyncTaskQueueMetrics::default())?;
  let (leaked, failures) = match task.blocking {
    Some(blocking) => {
      let leaked = blocking
        .buffers
        .lock()
        .map_err(|_| "blocking FFI host buffer lock is poisoned".to_owned())?
        .len();
      let failures = blocking
        .failures
        .lock()
        .map_err(|_| "blocking FFI failure log lock is poisoned".to_owned())?
        .clone();
      (leaked, failures)
    }
    None => return Err("blocking FFI handle lost its blocking state".to_owned()),
  };
  track::track_task_release();
  trace_ffi_event(
    "blocking-task-release-v1",
    format!(
      "task={} leaked_buffers={leaked} pending={}",
      handle.raw(),
      track::count_pending_tasks()
    ),
  );
  Ok((leaked, failures))
}

fn try_call_blocking_v1(
  lib: &libloading::Library,
  lib_name: &str,
  method: &str,
  args: Vec<Edn>,
  callback: Calcit,
  call_stack: &CallStackList,
) -> Result<Option<Calcit>, CalcitErr> {
  let runtime = native_async_runtime().map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?;
  let task = NativeAsyncTask {
    callback,
    stack: Arc::new(call_stack.to_owned()),
    lib_name: lib_name.to_owned(),
    method: method.to_owned(),
    control: Arc::new(Mutex::new(NativeAsyncTaskState::default())),
    blocking: Some(NativeBlockingTask {
      owner_thread: thread::current().id(),
      buffers: Arc::new(Mutex::new(HashMap::new())),
      failures: Arc::new(Mutex::new(vec![])),
    }),
    started_at: Instant::now(),
  };
  let handle = runtime
    .registry
    .register_with_flags(
      FfiAsyncHandleKind::OneShot,
      ASYNC_TASK_FLAG_SERIAL_EVENTS,
      NativeAsyncResource::Task(task),
    )
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error.to_string()))?;
  let descriptor = FfiAsyncTaskDescriptor::new(handle.raw(), FfiAsyncHandleKind::OneShot as u32, ASYNC_TASK_FLAG_SERIAL_EVENTS);
  let host = blocking_host_table(handle);
  track::track_task_add();
  trace_ffi_event(
    "blocking-call-v1",
    format!(
      "lib={lib_name} symbol={} task={} argc={} pending={}",
      calcit::ffi_abi::blocking_method_symbol(method),
      handle.raw(),
      args.len(),
      track::count_pending_tasks()
    ),
  );

  let outcome = calcit::ffi_abi::try_call_blocking(lib, lib_name, method, args, &descriptor, &host);
  let (leaked, mut failures) = release_blocking_task(runtime, handle)
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, format!("blocking task cleanup failed: {error}")))?;
  let value = match outcome {
    Ok(value) => value,
    Err(error) => {
      failures.insert(0, error);
      None
    }
  };
  if leaked > 0 {
    failures.push(format!("leaked {leaked} host callback buffer(s)"));
  }
  if !failures.is_empty() {
    return Err(CalcitErr::use_str(
      CalcitErrKind::Unexpected,
      format!("FFI blocking method `{method}` in `{lib_name}` failed: {}", failures.join("; ")),
    ));
  }
  match value {
    None => Ok(None),
    Some(ret) => {
      trace_ffi_event(
        "blocking-return-v1",
        format!(
          "lib={lib_name} symbol={} ret={}",
          calcit::ffi_abi::blocking_method_symbol(method),
          format_edn_args_for_trace(std::slice::from_ref(&ret))
        ),
      );
      Ok(Some(Calcit::Nil))
    }
  }
}

/// Pass a callback to a C-safe FFI method while the dylib owns the host
/// thread. Missing C-safe protocol or method symbols are migration errors.
pub fn blocking_dylib_edn_fn(xs: Vec<Calcit>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() < 3 {
    return CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&blocking-dylib-edn-fn expected >3 arguments, got: {xs:?}"),
    );
  }

  let lib_name: String = if let Calcit::Str(s) = &xs[0] {
    (**s).to_owned()
  } else {
    return CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&blocking-dylib-edn-fn expected a lib_name, got: {}", xs[0]),
    );
  };

  let method: String = if let Calcit::Str(s) = &xs[1] {
    (**s).to_owned()
  } else {
    return CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&blocking-dylib-edn-fn expected a method name, got: {}", xs[1]),
    );
  };
  let mut ys: Vec<Edn> = Vec::with_capacity(xs.len() - 2);
  let callback = xs[xs.len() - 1].to_owned();
  let size = xs.len();
  for (idx, v) in xs.iter().enumerate() {
    if idx > 1 && idx < size - 1 {
      ys.push(calcit_to_edn(v)?);
    }
  }
  if let Calcit::Fn { .. } = callback {
  } else {
    return CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("expected last argument to be callback fn, got: {callback}"),
    );
  }

  trace_ffi_event(
    "blocking-call",
    format!(
      "lib={lib_name} resolved={} symbol={method} argc={} args={}",
      resolve_trace_path(&lib_name),
      ys.len(),
      format_edn_args_for_trace(&ys)
    ),
  );

  let lib = load_dylib(&lib_name)?;
  let has_blocking_v1 = calcit::ffi_abi::has_blocking_method(&lib, &lib_name, &method)
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?;
  if has_blocking_v1 {
    return try_call_blocking_v1(&lib, &lib_name, &method, ys.clone(), callback.clone(), call_stack)?.ok_or_else(|| {
      CalcitErr::use_str(
        CalcitErrKind::Unexpected,
        format!("FFI blocking method `{method}` in `{lib_name}` disappeared after protocol lookup"),
      )
    });
  }
  let symbol = calcit::ffi_abi::blocking_method_symbol(&method);
  trace_ffi_event("blocking-required", format!("lib={lib_name} symbol={symbol} missing=true"));
  Err(c_safe_migration_error(
    "blocking",
    "blocking protocol v1",
    &lib_name,
    &method,
    &format!("`calcit_ffi_async_version`, `{symbol}`, and `calcit_ffi_buffer_free`"),
  ))
}

/// need to put it here since the crate does not compile for dylib
#[unsafe(no_mangle)]
pub fn on_ctrl_c(xs: Vec<Calcit>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() == 1 {
    if !matches!(&xs[0], Calcit::Fn { .. }) {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("on-control-c expected a callback function, got: {}", xs[0]),
      );
    }
    *CTRL_C_CALLBACK
      .lock()
      .map_err(|_| CalcitErr::use_str(CalcitErrKind::Unexpected, "Ctrl-C callback lock is poisoned"))? =
      Some((Arc::new(xs[0].to_owned()), Arc::new(call_stack.to_owned())));
    Ok(Calcit::Nil)
  } else {
    CalcitErr::err_str(CalcitErrKind::Arity, format!("on-control-c expected a callback function {xs:?}"))
  }
}

#[cfg(test)]
mod async_callback_tests {
  use super::*;

  type RecordedResponse = (u64, u64, u32, Vec<u8>);
  static RESPONSE_EVENTS: LazyLock<Mutex<Vec<RecordedResponse>>> = LazyLock::new(|| Mutex::new(vec![]));

  #[test]
  fn missing_c_safe_symbols_report_migration_without_rust_symbol_probes() {
    for (kind, protocol, expected) in [
      ("synchronous", "buffer protocol v1", "`read_calcit_ffi_v1`"),
      ("callback", "async protocol v1", "`serve_calcit_ffi_async_v1`"),
      ("blocking", "blocking protocol v1", "`paint_calcit_ffi_blocking_v1`"),
    ] {
      let error = c_safe_migration_error(kind, protocol, "fixture.so", "demo", expected);
      assert!(error.msg.contains(protocol), "error: {}", error.msg);
      assert!(error.msg.contains(expected), "error: {}", error.msg);
      assert!(
        error.msg.contains("legacy Rust-ABI fallback has been removed"),
        "error: {}",
        error.msg
      );
      assert!(!error.msg.contains("abi_version"), "error: {}", error.msg);
      assert!(!error.msg.contains("edn_version"), "error: {}", error.msg);
    }
  }

  unsafe extern "C" fn record_response(
    context: u64,
    response_handle: u64,
    outcome: u32,
    payload_ptr: *const u8,
    payload_len: usize,
  ) -> i32 {
    let payload = if payload_len == 0 {
      vec![]
    } else {
      // SAFETY: test callers pass a readable payload for this synchronous call.
      unsafe { std::slice::from_raw_parts(payload_ptr, payload_len) }.to_vec()
    };
    RESPONSE_EVENTS
      .lock()
      .expect("response event lock")
      .push((context, response_handle, outcome, payload));
    async_status::OK
  }

  unsafe extern "C" fn record_shutdown_cancel(context: u64, _task_handle: u64, reason_ptr: *const u8, reason_len: usize) -> i32 {
    if context == 0 || reason_ptr.is_null() {
      return async_status::INVALID_PAYLOAD;
    }
    let reason = unsafe { std::slice::from_raw_parts(reason_ptr, reason_len) };
    if reason != b"{} (:code :host-shutdown)" {
      return async_status::INVALID_PAYLOAD;
    }
    // SAFETY: shutdown tests pass a live AtomicUsize address and invoke the
    // cancellation callback synchronously before the counter is dropped.
    unsafe { &*(context as *const AtomicUsize) }.fetch_add(1, Ordering::Relaxed);
    async_status::OK
  }

  unsafe extern "C" fn complete_shutdown_cancel(context: u64, task_handle: u64, _reason_ptr: *const u8, _reason_len: usize) -> i32 {
    // SAFETY: this test passes a live host-thread runtime address as context
    // and invokes cancellation synchronously before that runtime is dropped.
    let runtime = unsafe { &*(context as *const NativeAsyncRuntime) };
    match runtime.queue.enqueue(
      &runtime.registry,
      FfiAsyncHandle::from_raw(task_handle),
      None,
      FfiAsyncEventKind::Complete,
      b"&unit".to_vec(),
    ) {
      Ok(_) => async_status::OK,
      Err(error) => error.status_code(),
    }
  }

  fn test_task() -> NativeAsyncResource {
    NativeAsyncResource::Task(NativeAsyncTask {
      callback: Calcit::Nil,
      stack: Arc::new(CallStackList::default()),
      lib_name: "fixture".to_owned(),
      method: "server".to_owned(),
      control: Arc::new(Mutex::new(NativeAsyncTaskState::default())),
      blocking: None,
      started_at: Instant::now(),
    })
  }

  fn test_runtime(capacity: usize) -> NativeAsyncRuntime {
    NativeAsyncRuntime {
      registry: FfiAsyncHandleRegistry::new(),
      queue: FfiAsyncEventQueue::new(capacity).expect("create host queue"),
      responses: Mutex::new(NativeAsyncResponseIndex::default()),
      completed_metrics: Mutex::new(BTreeMap::new()),
    }
  }

  #[test]
  fn task_cancel_purges_queued_emits_but_still_accepts_terminal() {
    let runtime = test_runtime(4);
    let handle = runtime
      .registry
      .register_with_flags(FfiAsyncHandleKind::Stream, ASYNC_TASK_FLAG_SERIAL_EVENTS, test_task())
      .expect("register cancellable task");
    let emit = b"([] |queued-before-cancel)";
    assert_eq!(
      unsafe {
        enqueue_native_async_event(
          &runtime,
          handle.raw(),
          handle.raw(),
          FfiAsyncEventKind::Emit as u32,
          FfiAsyncHandle::INVALID.raw(),
          emit.as_ptr(),
          emit.len(),
        )
      },
      async_status::OK
    );
    assert_eq!(runtime.queue.len(), Ok(1));

    assert_eq!(begin_async_task_cancel(&runtime, handle), Ok(1));
    assert_eq!(runtime.queue.len(), Ok(0));
    assert_eq!(
      unsafe {
        enqueue_native_async_event(
          &runtime,
          handle.raw(),
          handle.raw(),
          FfiAsyncEventKind::Emit as u32,
          FfiAsyncHandle::INVALID.raw(),
          emit.as_ptr(),
          emit.len(),
        )
      },
      async_status::HANDLE_CLOSING
    );

    let terminal = b"&unit";
    assert_eq!(
      unsafe {
        enqueue_native_async_event(
          &runtime,
          handle.raw(),
          handle.raw(),
          FfiAsyncEventKind::Complete as u32,
          FfiAsyncHandle::INVALID.raw(),
          terminal.as_ptr(),
          terminal.len(),
        )
      },
      async_status::OK
    );
    let report = runtime
      .queue
      .drain(&runtime.registry, 4, |event| dispatch_native_async_event(&runtime, event))
      .expect("drain terminal after cancel");
    assert_eq!(report.delivered, 1);
    assert!(report.lifecycle_failures.is_empty());
    assert_eq!(report.finished.len(), 1);
  }

  fn blocking_test_task_with_callback(callback: Calcit) -> (NativeAsyncResource, NativeBlockingTask) {
    let blocking = NativeBlockingTask {
      owner_thread: thread::current().id(),
      buffers: Arc::new(Mutex::new(HashMap::new())),
      failures: Arc::new(Mutex::new(vec![])),
    };
    (
      NativeAsyncResource::Task(NativeAsyncTask {
        callback,
        stack: Arc::new(CallStackList::default()),
        lib_name: "fixture".to_owned(),
        method: "blocking".to_owned(),
        control: Arc::new(Mutex::new(NativeAsyncTaskState::default())),
        blocking: Some(blocking.clone()),
        started_at: Instant::now(),
      }),
      blocking,
    )
  }

  fn blocking_test_task() -> (NativeAsyncResource, NativeBlockingTask) {
    blocking_test_task_with_callback(Calcit::Nil)
  }

  fn constant_callback(value: &str) -> Calcit {
    Calcit::Fn {
      id: Arc::from("blocking-fixture-callback"),
      info: Arc::new(calcit::calcit::CalcitFn {
        name: Arc::from("blocking-fixture-callback"),
        def_ns: Arc::from("tests.ffi"),
        def_ref: None,
        usage: calcit::calcit::CalcitFnUsageMeta::default(),
        scope: Arc::new(calcit::calcit::CalcitScope::default()),
        args: Arc::new(calcit::calcit::CalcitFnArgs::Args(vec![])),
        call_shape: calcit::calcit::CalcitFnCallShape::fixed(0),
        body: vec![Calcit::Str(Arc::from(value))],
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        return_type: calcit::calcit::DYNAMIC_TYPE.clone(),
        arg_types: vec![],
        rest_type: None,
      }),
    }
  }

  fn register_test_response(runtime: &NativeAsyncRuntime, owner: FfiAsyncHandle, context: u64, deadline: Instant) -> FfiAsyncHandle {
    let response = NativeAsyncResponse {
      owner_task: owner,
      context,
      deadline,
      resolve: record_response,
    };
    let handle = runtime
      .registry
      .register_for_active_owner(owner, FfiAsyncHandleKind::Response, NativeAsyncResource::Response(response))
      .expect("register response");
    runtime
      .responses
      .lock()
      .expect("response index")
      .insert(handle, response)
      .expect("index response");
    handle
  }

  #[test]
  fn blocking_host_buffers_require_exact_metadata_and_free_once() {
    let (_, blocking) = blocking_test_task();
    let mut output = FfiBuffer::empty();
    store_blocking_host_buffer(&blocking, b"|callback-result".to_vec(), &mut output).expect("store host buffer");
    assert_eq!(blocking.buffers.lock().expect("host buffers").len(), 1);

    let forged = FfiBuffer {
      ptr: output.ptr,
      len: output.len - 1,
      cap: output.cap,
    };
    assert_eq!(free_blocking_host_buffer(&blocking, forged), Err(async_status::INVALID_PAYLOAD));
    assert_eq!(blocking.buffers.lock().expect("host buffers").len(), 1);
    assert_eq!(free_blocking_host_buffer(&blocking, output), Ok(()));
    assert_eq!(free_blocking_host_buffer(&blocking, output), Err(async_status::INVALID_PAYLOAD));
  }

  #[test]
  fn blocking_callback_runs_inline_and_returns_host_owned_edn() {
    let runtime = test_runtime(4);
    let (task, blocking) = blocking_test_task_with_callback(constant_callback("ok"));
    let handle = runtime
      .registry
      .register_with_flags(FfiAsyncHandleKind::OneShot, ASYNC_TASK_FLAG_SERIAL_EVENTS, task)
      .expect("register blocking task");
    let payload = b"[]";
    let mut output = FfiBuffer::empty();
    assert_eq!(
      unsafe { invoke_native_blocking(&runtime, handle.raw(), handle.raw(), payload.as_ptr(), payload.len(), &mut output) },
      async_status::OK
    );
    let output_bytes = unsafe { std::slice::from_raw_parts(output.ptr.cast_const(), output.len) };
    let output_edn = cirru_edn::parse(std::str::from_utf8(output_bytes).expect("callback UTF-8")).expect("callback EDN");
    assert_eq!(output_edn, Edn::str("ok"));
    assert_eq!(free_blocking_host_buffer(&blocking, output), Ok(()));
    assert_eq!(runtime.registry.state(handle).expect("blocking state").next_sequence, 2);

    assert_eq!(runtime.registry.finish(handle), Ok(()));
    let mut late_output = FfiBuffer::empty();
    assert_eq!(
      unsafe {
        invoke_native_blocking(
          &runtime,
          handle.raw(),
          handle.raw(),
          payload.as_ptr(),
          payload.len(),
          &mut late_output,
        )
      },
      async_status::HANDLE_FINISHED
    );
    assert!(late_output.ptr.is_null());
    assert!(matches!(runtime.registry.release(handle), Ok(NativeAsyncResource::Task(_))));
  }

  #[test]
  fn blocking_task_rejects_callback_dispatch_from_a_foreign_thread() {
    let runtime = Arc::new(test_runtime(4));
    let (task, blocking) = blocking_test_task();
    let handle = runtime
      .registry
      .register_with_flags(FfiAsyncHandleKind::OneShot, ASYNC_TASK_FLAG_SERIAL_EVENTS, task)
      .expect("register blocking task");
    assert!(resolve_native_blocking_task(&runtime, handle.raw(), handle.raw()).is_ok());

    let foreign_runtime = Arc::clone(&runtime);
    let status = thread::spawn(move || {
      resolve_native_blocking_task(&foreign_runtime, handle.raw(), handle.raw())
        .err()
        .expect("foreign thread must fail")
    })
    .join()
    .expect("foreign thread");
    assert_eq!(status, async_status::WRONG_THREAD);
    assert_eq!(
      blocking.failures.lock().expect("blocking failures").as_slice(),
      ["blocking FFI callback was invoked from a foreign thread"]
    );
  }

  #[test]
  fn blocking_explicit_finish_is_exactly_once_and_prevents_late_callbacks() {
    let runtime = test_runtime(4);
    let (task, _) = blocking_test_task();
    let handle = runtime
      .registry
      .register_with_flags(FfiAsyncHandleKind::OneShot, ASYNC_TASK_FLAG_SERIAL_EVENTS, task)
      .expect("register blocking task");
    assert_eq!(runtime.registry.finish(handle), Ok(()));
    assert_eq!(
      runtime.registry.finish(handle),
      Err(calcit::ffi_abi::FfiAsyncHandleError::HandleFinished)
    );
    assert_eq!(
      runtime.registry.next_event_sequence(handle),
      Err(calcit::ffi_abi::FfiAsyncHandleError::HandleFinished)
    );
    assert!(matches!(runtime.registry.release(handle), Ok(NativeAsyncResource::Task(_))));
  }

  #[test]
  fn foreign_producer_enters_through_c_payload_boundary_and_host_drains_completion() {
    let runtime = Arc::new(test_runtime(4));
    let handle = runtime
      .registry
      .register_with_flags(FfiAsyncHandleKind::Stream, ASYNC_TASK_FLAG_SERIAL_EVENTS, test_task())
      .expect("register fixture task");

    let producer_runtime = Arc::clone(&runtime);
    let producer = thread::spawn(move || {
      let payload = b"&unit";
      // SAFETY: the payload is readable for the duration of the call.
      unsafe {
        enqueue_native_async_event(
          &producer_runtime,
          handle.raw(),
          handle.raw(),
          FfiAsyncEventKind::Complete as u32,
          FfiAsyncHandle::INVALID.raw(),
          payload.as_ptr(),
          payload.len(),
        )
      }
    });
    assert_eq!(producer.join().expect("producer thread"), async_status::OK);

    let report = runtime
      .queue
      .drain(&runtime.registry, 4, |event| dispatch_native_async_event(&runtime, event))
      .expect("host-thread drain");
    assert_eq!(report.dequeued, 1);
    assert_eq!(report.delivered, 1);
    assert_eq!(report.finished.len(), 1);
    assert_eq!(report.finished[0].task_handle, handle.raw());
    assert!(report.callback_failures.is_empty());
    assert!(report.lifecycle_failures.is_empty());
  }

  #[test]
  fn async_payload_decoders_require_list_unit_and_structured_failure() {
    let args = decode_async_emit(b"[] 1 |two").expect("decode emit list");
    assert_eq!(args.len(), 2);
    assert!(decode_async_emit(b"{} (:not-a-list true)").is_err());
    assert_eq!(validate_async_completion(b"  &unit\n"), Ok(()));
    assert!(validate_async_completion(b"nil").is_err());
    assert_eq!(
      decode_async_failure(b"{} (:code :closed)").expect("decode failure"),
      "{} $ :code :closed"
    );
  }

  #[test]
  fn expired_response_is_rejected_once_and_released() {
    const CONTEXT: u64 = 101;
    let runtime = test_runtime(4);
    let owner = runtime
      .registry
      .register_with_flags(
        FfiAsyncHandleKind::Server,
        ASYNC_TASK_FLAG_SERIAL_EVENTS | ASYNC_TASK_FLAG_REQUIRES_RESPONSE,
        test_task(),
      )
      .expect("register server");
    let response = register_test_response(&runtime, owner, CONTEXT, Instant::now() - Duration::from_millis(1));

    expire_async_responses(&runtime).expect("expire responses");
    assert_eq!(
      runtime.registry.state(response),
      Err(calcit::ffi_abi::FfiAsyncHandleError::StaleHandle)
    );
    let events: Vec<RecordedResponse> = RESPONSE_EVENTS
      .lock()
      .expect("response events")
      .iter()
      .filter(|(context, ..)| *context == CONTEXT)
      .cloned()
      .collect();
    assert_eq!(
      events,
      vec![(CONTEXT, response.raw(), ASYNC_RESPONSE_REJECT, b"{} (:code :timeout)".to_vec())]
    );
    let metrics: serde_json::Value =
      serde_json::from_str(&native_async_metrics_json(&runtime).expect("metrics JSON")).expect("valid metrics JSON");
    assert_eq!(metrics["totals"]["deadlineTimeoutTotal"], 1);
    assert_eq!(metrics["modules"][0]["module"], "fixture");
    assert_eq!(metrics["modules"][0]["method"], "server");
  }

  #[test]
  fn response_resolution_is_exactly_once_and_rejects_stale_reuse() {
    const CONTEXT: u64 = 202;
    let runtime = test_runtime(4);
    let owner = runtime
      .registry
      .register(FfiAsyncHandleKind::Server, test_task())
      .expect("register server");
    let response = register_test_response(&runtime, owner, CONTEXT, Instant::now() + Duration::from_secs(1));
    let capability = NativeAsyncCapability {
      handle: response,
      kind: FfiAsyncHandleKind::Response,
    };

    resolve_async_response_with(&runtime, capability, ASYNC_RESPONSE_RESOLVE, b"|ok").expect("resolve response");
    let error =
      resolve_async_response_with(&runtime, capability, ASYNC_RESPONSE_RESOLVE, b"|late").expect_err("stale response must fail");
    assert!(error.contains("stale"), "error: {error}");
    let events: Vec<RecordedResponse> = RESPONSE_EVENTS
      .lock()
      .expect("response events")
      .iter()
      .filter(|(context, ..)| *context == CONTEXT)
      .cloned()
      .collect();
    assert_eq!(events, vec![(CONTEXT, response.raw(), ASYNC_RESPONSE_RESOLVE, b"|ok".to_vec())]);
  }

  #[test]
  fn concurrent_response_resolution_invokes_module_exactly_once() {
    const CONTEXT: u64 = 303;
    let runtime = Arc::new(test_runtime(4));
    let owner = runtime
      .registry
      .register(FfiAsyncHandleKind::Server, test_task())
      .expect("register server");
    let response = register_test_response(&runtime, owner, CONTEXT, Instant::now() + Duration::from_secs(1));
    let capability = NativeAsyncCapability {
      handle: response,
      kind: FfiAsyncHandleKind::Response,
    };
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let mut workers = vec![];
    for payload in [b"|first".as_slice(), b"|second".as_slice()] {
      let runtime = Arc::clone(&runtime);
      let barrier = Arc::clone(&barrier);
      workers.push(thread::spawn(move || {
        barrier.wait();
        resolve_async_response_with(&runtime, capability, ASYNC_RESPONSE_RESOLVE, payload)
      }));
    }
    barrier.wait();
    let outcomes: Vec<Result<(), String>> = workers
      .into_iter()
      .map(|worker| worker.join().expect("response resolver thread"))
      .collect();

    assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
    let events: Vec<RecordedResponse> = RESPONSE_EVENTS
      .lock()
      .expect("response events")
      .iter()
      .filter(|(context, ..)| *context == CONTEXT)
      .cloned()
      .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].1, response.raw());
    assert_eq!(events[0].2, ASYNC_RESPONSE_RESOLVE);
  }

  #[test]
  fn queued_response_expiry_skips_request_without_finishing_server() {
    const CONTEXT: u64 = 404;
    let runtime = test_runtime(4);
    let owner = runtime
      .registry
      .register_with_flags(
        FfiAsyncHandleKind::Server,
        ASYNC_TASK_FLAG_SERIAL_EVENTS | ASYNC_TASK_FLAG_REQUIRES_RESPONSE,
        test_task(),
      )
      .expect("register server");
    let response = register_test_response(&runtime, owner, CONTEXT, Instant::now() + Duration::from_millis(1));
    let request = b"[] |request";
    assert_eq!(
      unsafe {
        enqueue_native_async_event(
          &runtime,
          owner.raw(),
          owner.raw(),
          FfiAsyncEventKind::Emit as u32,
          response.raw(),
          request.as_ptr(),
          request.len(),
        )
      },
      async_status::OK
    );
    thread::sleep(Duration::from_millis(5));
    expire_async_responses(&runtime).expect("expire queued response");
    let complete = b"&unit";
    assert_eq!(
      unsafe {
        enqueue_native_async_event(
          &runtime,
          owner.raw(),
          owner.raw(),
          FfiAsyncEventKind::Complete as u32,
          FfiAsyncHandle::INVALID.raw(),
          complete.as_ptr(),
          complete.len(),
        )
      },
      async_status::OK
    );

    let report = runtime
      .queue
      .drain(&runtime.registry, 4, |event| dispatch_native_async_event(&runtime, event))
      .expect("drain queued expiry and completion");
    assert_eq!(report.dequeued, 2);
    assert_eq!(report.delivered, 2);
    assert_eq!(report.finished.len(), 1);
    assert!(report.callback_failures.is_empty());
    assert!(report.lifecycle_failures.is_empty());
    let events: Vec<RecordedResponse> = RESPONSE_EVENTS
      .lock()
      .expect("response events")
      .iter()
      .filter(|(context, ..)| *context == CONTEXT)
      .cloned()
      .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].2, ASYNC_RESPONSE_REJECT);
  }

  #[test]
  fn response_handle_must_belong_to_the_enqueuing_server() {
    let runtime = test_runtime(4);
    let first = runtime
      .registry
      .register_with_flags(
        FfiAsyncHandleKind::Server,
        ASYNC_TASK_FLAG_SERIAL_EVENTS | ASYNC_TASK_FLAG_REQUIRES_RESPONSE,
        test_task(),
      )
      .expect("register first server");
    let second = runtime
      .registry
      .register_with_flags(
        FfiAsyncHandleKind::Server,
        ASYNC_TASK_FLAG_SERIAL_EVENTS | ASYNC_TASK_FLAG_REQUIRES_RESPONSE,
        test_task(),
      )
      .expect("register second server");
    let response = register_test_response(&runtime, first, 0, Instant::now() + Duration::from_secs(1));
    let payload = b"[] |request";

    assert_eq!(
      unsafe {
        enqueue_native_async_event(
          &runtime,
          second.raw(),
          second.raw(),
          FfiAsyncEventKind::Emit as u32,
          response.raw(),
          payload.as_ptr(),
          payload.len(),
        )
      },
      async_status::INVALID_HANDLE
    );
    assert_eq!(runtime.queue.len(), Ok(0));

    assert_eq!(
      unsafe {
        enqueue_native_async_event(
          &runtime,
          first.raw(),
          second.raw(),
          FfiAsyncEventKind::Emit as u32,
          FfiAsyncHandle::INVALID.raw(),
          payload.as_ptr(),
          payload.len(),
        )
      },
      async_status::INVALID_HANDLE
    );
  }

  #[test]
  fn runtime_shutdown_cancels_tasks_rejects_responses_and_force_cleans_after_grace() {
    const RESPONSE_CONTEXT: u64 = 808;
    let shutdown_cancels = AtomicUsize::new(0);
    let runtime = test_runtime(4);
    let task = test_task();
    let NativeAsyncResource::Task(task_state) = &task else {
      unreachable!("test task is a task resource");
    };
    task_state.control.lock().expect("task control").control = Some(NativeAsyncTaskControl {
      context: (&shutdown_cancels as *const AtomicUsize) as u64,
      cancel: record_shutdown_cancel,
    });
    let owner = runtime
      .registry
      .register_with_flags(
        FfiAsyncHandleKind::Server,
        ASYNC_TASK_FLAG_SERIAL_EVENTS | ASYNC_TASK_FLAG_REQUIRES_RESPONSE,
        task,
      )
      .expect("register shutdown server");
    let response = register_test_response(&runtime, owner, RESPONSE_CONTEXT, Instant::now() + Duration::from_secs(1));

    assert_eq!(
      shutdown_native_async_runtime(&runtime, Duration::ZERO, false).expect("shutdown runtime"),
      1
    );
    assert_eq!(shutdown_cancels.load(Ordering::Relaxed), 1);
    assert_eq!(
      runtime.registry.state(owner),
      Err(calcit::ffi_abi::FfiAsyncHandleError::StaleHandle)
    );
    assert_eq!(
      runtime.registry.state(response),
      Err(calcit::ffi_abi::FfiAsyncHandleError::StaleHandle)
    );
    assert_eq!(runtime.registry.pending_count(), Ok(0));
    assert_eq!(runtime.queue.task_metrics(owner), Ok(None));
    assert!(!runtime.queue.wait_for_event(Duration::ZERO).expect("closed queue"));
    assert_eq!(
      runtime.registry.register(FfiAsyncHandleKind::Stream, test_task()),
      Err(calcit::ffi_abi::FfiAsyncHandleError::HostClosing)
    );
    let responses: Vec<RecordedResponse> = RESPONSE_EVENTS
      .lock()
      .expect("response events")
      .iter()
      .filter(|(context, ..)| *context == RESPONSE_CONTEXT)
      .cloned()
      .collect();
    assert_eq!(
      responses,
      vec![(
        RESPONSE_CONTEXT,
        response.raw(),
        ASYNC_RESPONSE_REJECT,
        b"{} (:code :host-shutdown)".to_vec()
      )]
    );
    let metrics: serde_json::Value =
      serde_json::from_str(&native_async_metrics_json(&runtime).expect("metrics JSON")).expect("valid metrics JSON");
    assert_eq!(metrics["totals"]["activeTasks"], 0);
    assert_eq!(metrics["totals"]["completedTasks"], 1);
    assert_eq!(metrics["totals"]["cancelRequestedTotal"], 1);
    assert_eq!(metrics["totals"]["cancelSucceededTotal"], 1);
    assert_eq!(metrics["totals"]["cancelFailedTotal"], 0);
  }

  #[test]
  fn runtime_shutdown_drains_cooperative_terminal_before_deadline() {
    let runtime = test_runtime(4);
    let task = test_task();
    let NativeAsyncResource::Task(task_state) = &task else {
      unreachable!("test task is a task resource");
    };
    task_state.control.lock().expect("task control").control = Some(NativeAsyncTaskControl {
      context: (&runtime as *const NativeAsyncRuntime) as u64,
      cancel: complete_shutdown_cancel,
    });
    let owner = runtime
      .registry
      .register_with_flags(FfiAsyncHandleKind::Stream, ASYNC_TASK_FLAG_SERIAL_EVENTS, task)
      .expect("register cooperative stream");

    assert_eq!(
      shutdown_native_async_runtime(&runtime, Duration::from_millis(50), false).expect("shutdown runtime"),
      0
    );
    assert_eq!(
      runtime.registry.state(owner),
      Err(calcit::ffi_abi::FfiAsyncHandleError::StaleHandle)
    );
    assert_eq!(runtime.registry.pending_count(), Ok(0));
    assert!(runtime.queue.is_empty().expect("empty shutdown queue"));
    assert_eq!(runtime.queue.task_metrics(owner), Ok(None));
  }

  #[test]
  fn runtime_shutdown_does_not_cancel_an_already_closing_task_twice() {
    let shutdown_cancels = AtomicUsize::new(0);
    let runtime = test_runtime(4);
    let task = test_task();
    let NativeAsyncResource::Task(task_state) = &task else {
      unreachable!("test task is a task resource");
    };
    task_state.control.lock().expect("task control").control = Some(NativeAsyncTaskControl {
      context: (&shutdown_cancels as *const AtomicUsize) as u64,
      cancel: record_shutdown_cancel,
    });
    let owner = runtime
      .registry
      .register_with_flags(FfiAsyncHandleKind::Stream, ASYNC_TASK_FLAG_SERIAL_EVENTS, task)
      .expect("register closing stream");
    runtime.registry.begin_close(owner).expect("begin explicit close");

    assert_eq!(
      shutdown_native_async_runtime(&runtime, Duration::ZERO, false).expect("shutdown runtime"),
      1
    );
    assert_eq!(shutdown_cancels.load(Ordering::Relaxed), 0);
    assert_eq!(
      runtime.registry.state(owner),
      Err(calcit::ffi_abi::FfiAsyncHandleError::StaleHandle)
    );
  }
}
