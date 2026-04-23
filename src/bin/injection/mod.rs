use crate::runner;
use cirru_edn::Edn;
use colored::Colorize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
use std::time::Instant;

use calcit::{
  builtins,
  builtins::{RegisteredProcDescriptor, RegisteredProcPlatform, RegisteredProcStability},
  calcit::{Calcit, CalcitErr, CalcitErrKind},
  call_stack::{CallStackList, display_stack},
  data::edn::{calcit_to_edn, edn_to_calcit, sanitize_edn_for_format},
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
static TRACE_FFI: AtomicBool = AtomicBool::new(false);
static TRACE_FFI_EVENT_ID: AtomicUsize = AtomicUsize::new(1);
static TRACE_FFI_STARTED: LazyLock<Instant> = LazyLock::new(Instant::now);

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
        "cwd={cwd} exe={exe} abi={ABI_VERSION} edn={edn_version} host={}",
        std::env::consts::OS
      ),
    );
  }
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
  let expected_edn_version = cirru_edn::version();
  trace_ffi_event("lookup-abi", format!("lib={lib_name}"));
  let lookup_version: libloading::Symbol<fn() -> String> = unsafe { lib.get("abi_version".as_bytes()) }.map_err(|e| {
    CalcitErr::use_str(
      CalcitErrKind::Unexpected,
      format!("failed to lookup `abi_version` in `{lib_name}`: {e}"),
    )
  })?;
  let current = lookup_version();
  trace_ffi_event("abi-version", format!("lib={lib_name} current={current} expected={ABI_VERSION}"));
  if current != ABI_VERSION {
    return CalcitErr::err_str(CalcitErrKind::Unexpected, format!("ABI versions mismatch: {current} {ABI_VERSION}")).map(|_| ());
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

const ABI_VERSION: &str = "0.0.9";

pub fn inject_platform_apis() {
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
    },
  );
  builtins::register_import_proc("echo", stdout_println);
  builtins::register_import_proc("println", stdout_println);
  builtins::register_import_proc("eprintln", stderr_println);
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
    },
  );
  builtins::register_import_proc("async-sleep", builtins::meta::async_sleep);
  builtins::register_import_proc("on-control-c", on_ctrl_c);
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
  println!("{s}");
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
  eprintln!("{s}");
  Ok(Calcit::Nil)
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

  track::track_task_add();
  trace_ffi_event("task-add", format!("kind=callback pending={}", track::count_pending_tasks()));

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
  ensure_abi_compatible(&lib, &lib_name)?;
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

  Ok(Calcit::Nil)
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
