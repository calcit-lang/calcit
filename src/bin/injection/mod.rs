use crate::runner;
use cirru_edn::Edn;
use colored::Colorize;
use std::collections::{HashMap, HashSet};
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
    ASYNC_TASK_FLAG_SERIAL_EVENTS, FfiAsyncEventKind, FfiAsyncHandle, FfiAsyncHandleKind, FfiAsyncHandleRegistry, FfiAsyncHostV1,
    FfiAsyncTaskDescriptor, async_status,
  },
  ffi_async::{FfiAsyncDrainReport, FfiAsyncEventQueue, copy_async_payload},
  runner::track,
};

/// FFI protocol types
type EdnFfi = fn(args: Vec<Edn>) -> Result<Edn, String>;
type EdnFfiFn = fn(
  args: Vec<Edn>,
  f: Arc<dyn Fn(Vec<Edn>) -> Result<Edn, String> + Send + Sync + 'static>,
  finish: Arc<dyn FnOnce()>,
) -> Result<Edn, String>;
/// lazily cache dylibs, in case Linux drops memory of libraries
static DYLIBS: LazyLock<Mutex<HashMap<String, Arc<libloading::Library>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static LEGACY_DYLIB_WARNED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
static TRACE_FFI: AtomicBool = AtomicBool::new(false);
static STDOUT_TO_STDERR: AtomicBool = AtomicBool::new(false);
static SILENCE_PROGRAM_OUTPUT: AtomicBool = AtomicBool::new(false);
static TRACE_FFI_EVENT_ID: AtomicUsize = AtomicUsize::new(1);
static TRACE_FFI_STARTED: LazyLock<Instant> = LazyLock::new(Instant::now);
const ASYNC_HOST_CONTEXT: u64 = 0x4341_4c43_4954_0001;
const ASYNC_EVENT_QUEUE_CAPACITY: usize = 1024;

#[derive(Clone)]
struct NativeAsyncCallback {
  callback: Calcit,
  stack: Arc<CallStackList>,
  lib_name: String,
  method: String,
}

struct NativeAsyncRuntime {
  registry: FfiAsyncHandleRegistry<NativeAsyncCallback>,
  queue: FfiAsyncEventQueue,
}

static NATIVE_ASYNC_RUNTIME: OnceLock<NativeAsyncRuntime> = OnceLock::new();
static NATIVE_ASYNC_HOST: LazyLock<FfiAsyncHostV1> = LazyLock::new(|| FfiAsyncHostV1::new(ASYNC_HOST_CONTEXT, native_async_enqueue));

#[allow(dead_code)]
pub fn set_trace_ffi(v: bool) {
  TRACE_FFI.store(v, Ordering::Relaxed);
  if v {
    let edn_version = cirru_edn::version();
    let cwd = std::env::current_dir()
      .map(|p| p.display().to_string())
      .unwrap_or_else(|_| "<unknown-cwd>".to_string());
    let exe = std::env::current_exe()
      .map(|p| p.display().to_string())
      .unwrap_or_else(|_| "<unknown-exe>".to_string());
    trace_ffi_event(
      "enable",
      format!(
        "cwd={cwd} exe={exe} abi={} edn={edn_version} host={}",
        calcit::FFI_ABI_VERSION,
        std::env::consts::OS,
      ),
    );
    trace_ffi_event("host-build", calcit::FFI_BUILD_ID);
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

/// Bind the async queue to the CLI worker thread before any dylib can publish
/// work. Tests that only inject proc metadata do not initialize global runtime
/// state and therefore cannot accidentally claim host-thread ownership.
pub fn init_async_runtime() -> Result<(), String> {
  if NATIVE_ASYNC_RUNTIME.get().is_some() {
    return Ok(());
  }
  let runtime = NativeAsyncRuntime {
    registry: FfiAsyncHandleRegistry::new(),
    queue: FfiAsyncEventQueue::new(ASYNC_EVENT_QUEUE_CAPACITY).map_err(|error| error.to_string())?,
  };
  NATIVE_ASYNC_RUNTIME
    .set(runtime)
    .map_err(|_| "async FFI runtime was initialized concurrently".to_owned())
}

fn native_async_runtime() -> Result<&'static NativeAsyncRuntime, String> {
  NATIVE_ASYNC_RUNTIME
    .get()
    .ok_or_else(|| "async FFI runtime is not initialized on the CLI worker thread".to_owned())
}

fn async_host_table() -> &'static FfiAsyncHostV1 {
  &NATIVE_ASYNC_HOST
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
  if context != ASYNC_HOST_CONTEXT {
    return async_status::INVALID_HANDLE;
  }
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
  let response_handle = if response_handle == FfiAsyncHandle::INVALID.raw() {
    None
  } else {
    Some(FfiAsyncHandle::from_raw(response_handle))
  };

  match runtime
    .queue
    .enqueue(&runtime.registry, task_handle, response_handle, kind, payload)
  {
    Ok(outcome) => {
      trace_ffi_event(
        "async-enqueue",
        format!(
          "task={} kind={kind:?} sequence={} disposition={:?} producer={:?}",
          task_handle.raw(),
          outcome.sequence,
          outcome.disposition,
          thread::current().id()
        ),
      );
      async_status::OK
    }
    Err(error) => {
      trace_ffi_event(
        "async-enqueue-rejected",
        format!(
          "task={} kind={kind:?} status={} error={error}",
          task_handle.raw(),
          error.status_code()
        ),
      );
      error.status_code()
    }
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
  let task = runtime.registry.clone_value(handle).map_err(|error| error.to_string())?;
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
      let args = decode_async_emit(event.payload())?;
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

pub fn drain_async_events(limit: usize) -> Result<FfiAsyncDrainReport, String> {
  let runtime = native_async_runtime()?;
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
    if let Err(error) = runtime.registry.release(handle) {
      report
        .lifecycle_failures
        .push(calcit::ffi_async::FfiAsyncLifecycleFailure { descriptor, error });
      continue;
    }
    track::track_task_release();
    trace_ffi_event(
      "async-task-release",
      format!(
        "task={} sequence={} pending={}",
        handle.raw(),
        descriptor.sequence,
        track::count_pending_tasks()
      ),
    );
  }

  Ok(report)
}

pub fn exit_when_async_cleared() -> Result<(), String> {
  let runtime = native_async_runtime()?;
  loop {
    drain_async_events(256)?;
    if track::count_pending_tasks() == 0 && runtime.queue.is_empty().map_err(|error| error.to_string())? {
      return Ok(());
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

fn ensure_abi_compatible(lib: &libloading::Library, lib_name: &str) -> Result<(), CalcitErr> {
  trace_ffi_event("lookup-build-id", format!("lib={lib_name}"));
  let dylib_build_id =
    calcit::ffi_abi::lookup_build_id(lib, lib_name).map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?;
  match calcit::ffi_abi::validate_build_id(lib_name, dylib_build_id.as_deref(), calcit::FFI_BUILD_ID, cfg!(debug_assertions))
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error))?
  {
    calcit::ffi_abi::FfiBuildCompatibility::Exact => trace_ffi_event(
      "build-id",
      format!(
        "lib={lib_name} dylib={} host={} compatible=true",
        dylib_build_id.as_deref().unwrap_or("<missing>"),
        calcit::FFI_BUILD_ID
      ),
    ),
    calcit::ffi_abi::FfiBuildCompatibility::Legacy => {
      trace_ffi_event(
        "build-id",
        format!("lib={lib_name} dylib=<missing> host={} legacy=true", calcit::FFI_BUILD_ID),
      );
      let mut warned = LEGACY_DYLIB_WARNED
        .lock()
        .map_err(|_| CalcitErr::use_str(CalcitErrKind::Unexpected, "failed to lock legacy dylib warning cache"))?;
      if warned.insert(lib_name.to_owned()) && !calcit::quiet_tool_output() {
        eprintln!(
          "[warning] Rust-native FFI library `{lib_name}` has no C-safe `calcit_ffi_build_id`; release-host compatibility is temporary and cannot prove compiler compatibility. Rebuild the module using the current FFI guide."
        );
      }
    }
  }

  let expected_edn_version = cirru_edn::version();
  trace_ffi_event("lookup-abi", format!("lib={lib_name}"));
  let lookup_version: libloading::Symbol<fn() -> String> = unsafe { lib.get("abi_version".as_bytes()) }.map_err(|e| {
    CalcitErr::use_str(
      CalcitErrKind::Unexpected,
      format!("failed to lookup `abi_version` in `{lib_name}`: {e}"),
    )
  })?;
  let current = lookup_version();
  trace_ffi_event(
    "abi-version",
    format!("lib={lib_name} current={current} expected={}", calcit::FFI_ABI_VERSION),
  );
  if current != calcit::FFI_ABI_VERSION {
    return CalcitErr::err_str(
      CalcitErrKind::Unexpected,
      format!("ABI versions mismatch: {current} {}", calcit::FFI_ABI_VERSION),
    )
    .map(|_| ());
  }

  trace_ffi_event("lookup-edn-version", format!("lib={lib_name}"));
  let lookup_edn_version: libloading::Symbol<fn() -> String> = unsafe { lib.get("edn_version".as_bytes()) }.map_err(|e| {
    CalcitErr::use_str(
      CalcitErrKind::Unexpected,
      format!("failed to lookup `edn_version` in `{lib_name}`: {e}"),
    )
  })?;
  let current_edn = lookup_edn_version();
  trace_ffi_event(
    "edn-version",
    format!("lib={lib_name} current={current_edn} expected={expected_edn_version}"),
  );
  if current_edn != expected_edn_version {
    return CalcitErr::err_str(
      CalcitErrKind::Unexpected,
      format!("cirru_edn versions mismatch: {current_edn} {expected_edn_version}"),
    )
    .map(|_| ());
  }
  Ok(())
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
  match calcit::ffi_abi::try_call_buffer(&lib, &lib_name, &method, ys.clone())
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
      return Ok(edn_to_calcit(&ret, &Calcit::Nil));
    }
    None => trace_ffi_event("buffer-fallback", format!("lib={lib_name} symbol={method}")),
  }
  ensure_abi_compatible(&lib, &lib_name)?;
  trace_ffi_event("lookup-symbol", format!("lib={lib_name} symbol={method}"));
  let func: libloading::Symbol<EdnFfi> = unsafe { lib.get(method.as_bytes()) }.map_err(|e| {
    CalcitErr::use_str(
      CalcitErrKind::Unexpected,
      format!("failed to load FFI symbol `{method}` in `{lib_name}`: {e}"),
    )
  })?;
  let ret = func(ys.to_owned()).map_err(|e| {
    trace_ffi_event("error", format!("lib={lib_name} symbol={method} {e}"));
    e
  })?;
  trace_ffi_event(
    "return",
    format!(
      "lib={lib_name} symbol={method} ret={}",
      format_edn_args_for_trace(std::slice::from_ref(&ret))
    ),
  );
  Ok(edn_to_calcit(&ret, &Calcit::Nil))
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
  let task = NativeAsyncCallback {
    callback,
    stack: Arc::new(call_stack.to_owned()),
    lib_name: lib_name.to_owned(),
    method: method.to_owned(),
  };
  let handle = runtime
    .registry
    .register_with_flags(FfiAsyncHandleKind::Stream, ASYNC_TASK_FLAG_SERIAL_EVENTS, task)
    .map_err(|error| CalcitErr::use_str(CalcitErrKind::Unexpected, error.to_string()))?;
  let descriptor = FfiAsyncTaskDescriptor::new(handle, FfiAsyncHandleKind::Stream, ASYNC_TASK_FLAG_SERIAL_EVENTS);
  let host = async_host_table();

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
  let status = unsafe { start(request.as_ptr(), request.len(), &descriptor, host) };
  if status == async_status::OK {
    return Ok(Some(Calcit::Unit));
  }

  let purged = runtime.queue.discard_handle_events(handle).unwrap_or(0);
  let _ = runtime.registry.finish(handle);
  let _ = runtime.registry.release(handle);
  track::track_task_release();
  trace_ffi_event(
    "async-start-failed",
    format!(
      "lib={lib_name} symbol={} task={} status={status} purged={purged} pending={}",
      calcit::ffi_abi::async_method_symbol(method),
      handle.raw(),
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
  trace_ffi_event(
    "async-fallback",
    format!(
      "lib={lib_name} symbol={} fallback={method}",
      calcit::ffi_abi::async_method_symbol(&method)
    ),
  );
  ensure_abi_compatible(&lib, &lib_name)?;
  track::track_task_add();
  trace_ffi_event("task-add", format!("kind=callback pending={}", track::count_pending_tasks()));
  let copied_stack_1 = Arc::new(call_stack.to_owned());
  let method_name = method.clone();
  let lib_name_for_thread = lib_name.clone();

  let _handle = thread::spawn(move || {
    trace_ffi_event(
      "thread-start",
      format!(
        "lib={lib_name_for_thread} symbol={method_name} pending={}",
        track::count_pending_tasks()
      ),
    );
    let callback_method_name = method_name.clone();
    let callback_lib_name = lib_name_for_thread.clone();
    trace_ffi_event("lookup-symbol", format!("lib={lib_name_for_thread} symbol={method_name}"));
    let func: libloading::Symbol<EdnFfiFn> = match unsafe { lib.get(method_name.as_bytes()) } {
      Ok(f) => f,
      Err(e) => {
        track::track_task_release();
        trace_ffi_event("task-release", format!("kind=callback pending={}", track::count_pending_tasks()));
        return CalcitErr::err_str(
          CalcitErrKind::Unexpected,
          format!("failed to load FFI symbol `{method_name}` in `{lib_name_for_thread}`: {e}"),
        );
      }
    };
    let copied_stack = copied_stack_1.to_owned();
    match func(
      ys.to_owned(),
      Arc::new(move |ps: Vec<Edn>| -> Result<Edn, String> {
        trace_ffi_event(
          "callback-in",
          format!(
            "lib={callback_lib_name} symbol={callback_method_name} argc={} args={}",
            ps.len(),
            format_edn_args_for_trace(&ps)
          ),
        );
        if let Calcit::Fn { info, .. } = &callback {
          let mut real_args: Vec<Calcit> = vec![];
          for p in ps {
            real_args.push(edn_to_calcit(&p, &Calcit::Nil));
          }
          let r = runner::run_fn(&real_args, info, &copied_stack);
          match r {
            Ok(ret) => {
              let ret_edn = calcit_to_edn(&ret)?;
              trace_ffi_event(
                "callback-out",
                format!(
                  "lib={callback_lib_name} symbol={callback_method_name} ret={}",
                  format_edn_args_for_trace(std::slice::from_ref(&ret_edn))
                ),
              );
              Ok(ret_edn)
            }
            Err(e) => {
              display_stack(&format!("[Error] thread callback failed: {}", e.msg), &e.stack, e.location.as_ref())?;
              Err(format!("Error: {e}"))
            }
          }
        } else {
          Err(format!("expected last argument to be callback fn, got: {callback}"))
        }
      }),
      Arc::new(track::track_task_release),
    ) {
      Ok(ret) => {
        trace_ffi_event(
          "return-callback",
          format!(
            "lib={lib_name_for_thread} symbol={method_name} ret={}",
            format_edn_args_for_trace(std::slice::from_ref(&ret))
          ),
        );
        edn_to_calcit(&ret, &Calcit::Nil)
      }
      Err(e) => {
        track::track_task_release();
        trace_ffi_event("task-release", format!("kind=callback pending={}", track::count_pending_tasks()));
        trace_ffi_event("error-callback", format!("lib={lib_name_for_thread} symbol={method_name} {e}"));
        // let _ = display_stack(&format!("failed to call request: {}", e), &copied_stack_1);
        eprintln!("failure inside ffi thread: {e}");
        return CalcitErr::err_str(CalcitErrKind::Unexpected, e);
      }
    };
    Ok(Calcit::Nil)
  });

  Ok(Calcit::Unit)
}

/// pass callback function to FFI function, blocking the thread,
/// used by calcit-paint, where main thread is required
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

  track::track_task_add();
  trace_ffi_event("task-add", format!("kind=blocking pending={}", track::count_pending_tasks()));

  trace_ffi_event(
    "blocking-call",
    format!(
      "lib={lib_name} resolved={} symbol={method} argc={} args={}",
      resolve_trace_path(&lib_name),
      ys.len(),
      format_edn_args_for_trace(&ys)
    ),
  );

  let lib = unsafe { libloading::Library::new(&lib_name) }
    .map_err(|e| CalcitErr::use_str(CalcitErrKind::Unexpected, format!("failed to load dylib `{lib_name}`: {e}")))?;
  ensure_abi_compatible(&lib, &lib_name)?;
  let copied_stack = Arc::new(call_stack.to_owned());
  let callback_method = method.clone();
  let callback_lib_name = lib_name.clone();

  let func: libloading::Symbol<EdnFfiFn> = unsafe { lib.get(method.as_bytes()) }.map_err(|e| {
    CalcitErr::use_str(
      CalcitErrKind::Unexpected,
      format!("failed to load FFI symbol `{method}` in `{lib_name}`: {e}"),
    )
  })?;
  match func(
    ys.to_owned(),
    Arc::new(move |ps: Vec<Edn>| -> Result<Edn, String> {
      trace_ffi_event(
        "blocking-callback-in",
        format!(
          "lib={callback_lib_name} symbol={callback_method} argc={} args={}",
          ps.len(),
          format_edn_args_for_trace(&ps)
        ),
      );
      if let Calcit::Fn { info, .. } = &callback {
        let mut real_args: Vec<Calcit> = vec![];
        for p in ps {
          real_args.push(edn_to_calcit(&p, &Calcit::Nil));
        }
        let r = runner::run_fn(&real_args, info, &copied_stack);
        match r {
          Ok(ret) => {
            let ret_edn = calcit_to_edn(&ret)?;
            trace_ffi_event(
              "blocking-callback-out",
              format!(
                "lib={callback_lib_name} symbol={callback_method} ret={}",
                format_edn_args_for_trace(std::slice::from_ref(&ret_edn))
              ),
            );
            Ok(ret_edn)
          }
          Err(e) => {
            display_stack(&format!("[Error] thread callback failed: {}", e.msg), &e.stack, e.location.as_ref())?;
            Err(format!("Error: {e}"))
          }
        }
      } else {
        Err(format!("expected last argument to be callback fn, got: {callback}"))
      }
    }),
    Arc::new(track::track_task_release),
  ) {
    Ok(ret) => {
      trace_ffi_event(
        "blocking-return",
        format!(
          "lib={lib_name} symbol={method} ret={}",
          format_edn_args_for_trace(std::slice::from_ref(&ret))
        ),
      );
      edn_to_calcit(&ret, &Calcit::Nil)
    }
    Err(e) => {
      trace_ffi_event("blocking-error", format!("lib={lib_name} symbol={method} {e}"));
      // TODO for more accurate tracking, need to place tracker inside foreign function
      // track::track_task_release();
      let _ = display_stack(&format!("failed to call request: {e}"), call_stack, None);
      return CalcitErr::err_str(CalcitErrKind::Unexpected, e);
    }
  };

  Ok(Calcit::Nil)
}

/// need to put it here since the crate does not compile for dylib
#[unsafe(no_mangle)]
pub fn on_ctrl_c(xs: Vec<Calcit>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() == 1 {
    let cb = Arc::new(xs[0].to_owned());
    let copied_stack = Arc::new(call_stack.to_owned());
    ctrlc::set_handler(move || {
      if let Calcit::Fn { info, .. } = cb.as_ref()
        && let Err(e) = runner::run_fn(&[], info, &copied_stack)
      {
        eprintln!("error: {e}");
      }
    })
    .map_err(|e| CalcitErr::use_str(CalcitErrKind::Unexpected, format!("failed to set Ctrl-C handler: {e}")))?;
    Ok(Calcit::Nil)
  } else {
    CalcitErr::err_str(CalcitErrKind::Arity, format!("on-control-c expected a callback function {xs:?}"))
  }
}

#[cfg(test)]
mod async_callback_tests {
  use super::*;

  #[test]
  fn foreign_producer_enters_through_c_payload_boundary_and_host_drains_completion() {
    let runtime = Arc::new(NativeAsyncRuntime {
      registry: FfiAsyncHandleRegistry::new(),
      queue: FfiAsyncEventQueue::new(4).expect("create host queue"),
    });
    let handle = runtime
      .registry
      .register_with_flags(
        FfiAsyncHandleKind::Stream,
        ASYNC_TASK_FLAG_SERIAL_EVENTS,
        NativeAsyncCallback {
          callback: Calcit::Nil,
          stack: Arc::new(CallStackList::default()),
          lib_name: "fixture".to_owned(),
          method: "complete".to_owned(),
        },
      )
      .expect("register fixture task");

    let producer_runtime = Arc::clone(&runtime);
    let producer = thread::spawn(move || {
      let payload = b"&unit";
      // SAFETY: the payload is readable for the duration of the call.
      unsafe {
        enqueue_native_async_event(
          &producer_runtime,
          ASYNC_HOST_CONTEXT,
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
}
