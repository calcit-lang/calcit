use crate::runner;
use cirru_edn::Edn;
use std::sync::Arc;
use std::thread;

use calcit_runner::{
  builtins,
  call_stack::{display_stack, CallStackList},
  data::edn::{calcit_to_edn, edn_to_calcit},
  primes::{Calcit, CalcitErr, CalcitItems, CrListWrap},
  runner::track,
};
use im_ternary_tree::TernaryTreeList;

/// FFI protocol types
type EdnFfi = fn(args: Vec<Edn>) -> Result<Edn, String>;
type EdnFfiFn = fn(
  args: Vec<Edn>,
  f: Arc<dyn Fn(Vec<Edn>) -> Result<Edn, String> + Send + Sync + 'static>,
  finish: Arc<dyn FnOnce()>,
) -> Result<Edn, String>;

const ABI_VERSION: &str = "0.0.6";

pub fn inject_platform_apis() {
  builtins::register_import_proc("&call-dylib-edn", call_dylib_edn);
  builtins::register_import_proc("echo", echo);
  builtins::register_import_proc("println", echo);
  builtins::register_import_proc("&call-dylib-edn-fn", call_dylib_edn_fn);
  builtins::register_import_proc("&blocking-dylib-edn-fn", blocking_dylib_edn_fn);
  builtins::register_import_proc("async-sleep", builtins::meta::async_sleep);
  builtins::register_import_proc("on-control-c", on_ctrl_c);
}

// &call-dylib-edn
pub fn call_dylib_edn(xs: &CalcitItems, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    return CalcitErr::err_str(format!("&call-dylib-edn expected >2 arguments, got {}", CrListWrap(xs.to_owned())));
  }
  let lib_name: String = if let Calcit::Str(s) = &xs[0] {
    (**s).to_owned()
  } else {
    return CalcitErr::err_str(format!("&call-dylib-edn expected a lib_name, got {}", xs[0]));
  };

  let method: String = if let Calcit::Str(s) = &xs[1] {
    (**s).to_owned()
  } else {
    return CalcitErr::err_str(format!("&call-dylib-edn expected a method name, got {}", xs[1]));
  };
  let mut ys: Vec<Edn> = Vec::with_capacity(xs.len());
  for (idx, v) in xs.into_iter().enumerate() {
    if idx > 1 {
      ys.push(calcit_to_edn(v)?);
    }
  }

  unsafe {
    let lib = libloading::Library::new(&lib_name).expect("dylib not found");

    let lookup_version: libloading::Symbol<fn() -> String> = lib.get("abi_version".as_bytes()).expect("request for ABI_VERSION");
    if lookup_version() != ABI_VERSION {
      return CalcitErr::err_str(format!("ABI versions mismatch: {} {}", lookup_version(), ABI_VERSION));
    }

    let func: libloading::Symbol<EdnFfi> = lib.get(method.as_bytes()).expect("dy function not found");
    let ret = func(ys.to_owned())?;
    Ok(edn_to_calcit(&ret))
  }
}

pub fn echo(xs: &CalcitItems, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let mut s = String::from("");
  for (idx, x) in xs.into_iter().enumerate() {
    if idx > 0 {
      s.push(' ');
    }
    s.push_str(&x.turn_string());
  }
  println!("{}", s);
  Ok(Calcit::Nil)
}

/// pass callback function to FFI function, so it can call multiple times
/// currently for HTTP servers
pub fn call_dylib_edn_fn(xs: &CalcitItems, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() < 3 {
    return CalcitErr::err_str(format!(
      "&call-dylib-edn-fn expected >3 arguments, got {}",
      CrListWrap(xs.to_owned())
    ));
  }

  let lib_name: String = if let Calcit::Str(s) = &xs[0] {
    (**s).to_owned()
  } else {
    return CalcitErr::err_str(format!("&call-dylib-edn-fn expected a lib_name, got {}", xs[0]));
  };

  let method: String = if let Calcit::Str(s) = &xs[1] {
    (**s).to_owned()
  } else {
    return CalcitErr::err_str(format!("&call-dylib-edn-fn expected a method name, got {}", xs[1]));
  };
  let mut ys: Vec<Edn> = Vec::with_capacity(xs.len() - 2);
  let callback = xs[xs.len() - 1].clone();
  for (idx, v) in xs.into_iter().enumerate() {
    if idx > 1 && idx < xs.len() - 1 {
      ys.push(calcit_to_edn(v)?);
    }
  }
  if let Calcit::Fn { .. } = callback {
  } else {
    return CalcitErr::err_str(format!("expected last argument to be callback fn, got: {}", callback));
  }

  track::track_task_add();

  let lib = unsafe {
    let lib_tmp = libloading::Library::new(&lib_name).expect("dylib not found");

    let lookup_version: libloading::Symbol<fn() -> String> = lib_tmp.get("abi_version".as_bytes()).expect("request for ABI_VERSION");
    if lookup_version() != ABI_VERSION {
      return CalcitErr::err_str(format!("ABI versions mismatch: {} {}", lookup_version(), ABI_VERSION));
    }

    lib_tmp
  };
  let copied_stack_1 = Arc::new(call_stack.to_owned());

  let _handle = thread::spawn(move || {
    let func: libloading::Symbol<EdnFfiFn> = unsafe { lib.get(method.as_bytes()).expect("dy function not found") };
    let copied_stack = copied_stack_1.clone();
    match func(
      ys.to_owned(),
      Arc::new(move |ps: Vec<Edn>| -> Result<Edn, String> {
        if let Calcit::Fn {
          def_ns, scope, args, body, ..
        } = &callback
        {
          let mut real_args = TernaryTreeList::Empty;
          for p in ps {
            real_args = real_args.push(edn_to_calcit(&p));
          }
          let r = runner::run_fn(&real_args, scope, args, body, def_ns.to_owned(), &copied_stack);
          match r {
            Ok(ret) => calcit_to_edn(&ret),
            Err(e) => {
              display_stack(&format!("[Error] thread callback failed: {}", e.msg), &e.stack)?;
              Err(format!("Error: {}", e))
            }
          }
        } else {
          // handled above
          unreachable!(format!("expected last argument to be callback fn, got: {}", callback));
        }
      }),
      Arc::new(track::track_task_release),
    ) {
      Ok(ret) => edn_to_calcit(&ret),
      Err(e) => {
        track::track_task_release();
        // let _ = display_stack(&format!("failed to call request: {}", e), &copied_stack_1);
        println!("failure inside ffi thread: {}", e);
        return CalcitErr::err_str(e);
      }
    };
    Ok(Calcit::Nil)
  });

  Ok(Calcit::Nil)
}

/// pass callback function to FFI function, so it can call multiple times
/// currently for HTTP servers
pub fn blocking_dylib_edn_fn(xs: &CalcitItems, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() < 3 {
    return CalcitErr::err_str(format!(
      "&blocking-dylib-edn-fn expected >3 arguments, got {}",
      CrListWrap(xs.to_owned())
    ));
  }

  let lib_name: String = if let Calcit::Str(s) = &xs[0] {
    (**s).to_owned()
  } else {
    return CalcitErr::err_str(format!("&blocking-dylib-edn-fn expected a lib_name, got {}", xs[0]));
  };

  let method: String = if let Calcit::Str(s) = &xs[1] {
    (**s).to_owned()
  } else {
    return CalcitErr::err_str(format!("&blocking-dylib-edn-fn expected a method name, got {}", xs[1]));
  };
  let mut ys: Vec<Edn> = Vec::with_capacity(xs.len() - 2);
  let callback = xs[xs.len() - 1].clone();
  for (idx, v) in xs.into_iter().enumerate() {
    if idx > 1 && idx < xs.len() - 1 {
      ys.push(calcit_to_edn(v)?);
    }
  }
  if let Calcit::Fn { .. } = callback {
  } else {
    return CalcitErr::err_str(format!("expected last argument to be callback fn, got: {}", callback));
  }

  track::track_task_add();

  let lib = unsafe {
    let lib_tmp = libloading::Library::new(&lib_name).expect("dylib not found");

    let lookup_version: libloading::Symbol<fn() -> String> = lib_tmp.get("abi_version".as_bytes()).expect("request for ABI_VERSION");
    if lookup_version() != ABI_VERSION {
      return CalcitErr::err_str(format!("ABI versions mismatch: {} {}", lookup_version(), ABI_VERSION));
    }

    lib_tmp
  };
  let copied_stack = Arc::new(call_stack.to_owned());

  let func: libloading::Symbol<EdnFfiFn> = unsafe { lib.get(method.as_bytes()).expect("dy function not found") };
  match func(
    ys.to_owned(),
    Arc::new(move |ps: Vec<Edn>| -> Result<Edn, String> {
      if let Calcit::Fn {
        def_ns, scope, args, body, ..
      } = &callback
      {
        let mut real_args = TernaryTreeList::Empty;
        for p in ps {
          real_args = real_args.push(edn_to_calcit(&p));
        }
        let r = runner::run_fn(&real_args, scope, args, body, def_ns.to_owned(), &copied_stack.clone());
        match r {
          Ok(ret) => calcit_to_edn(&ret),
          Err(e) => {
            display_stack(&format!("[Error] thread callback failed: {}", e.msg), &e.stack)?;
            Err(format!("Error: {}", e))
          }
        }
      } else {
        // handled above
        unreachable!(format!("expected last argument to be callback fn, got: {}", callback));
      }
    }),
    Arc::new(track::track_task_release),
  ) {
    Ok(ret) => edn_to_calcit(&ret),
    Err(e) => {
      track::track_task_release();
      let _ = display_stack(&format!("failed to call request: {}", e), call_stack);
      return CalcitErr::err_str(e);
    }
  };

  Ok(Calcit::Nil)
}

/// need to put it here since the crate does not compile for dylib
#[no_mangle]
pub fn on_ctrl_c(xs: &CalcitItems, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() == 1 {
    let cb = Arc::new(xs[0].to_owned());
    let copied_stack = Arc::new(call_stack.to_owned());
    ctrlc::set_handler(move || {
      if let Calcit::Fn {
        def_ns, scope, args, body, ..
      } = cb.as_ref()
      {
        if let Err(e) = runner::run_fn(&TernaryTreeList::Empty, scope, args, body, def_ns.to_owned(), &copied_stack) {
          println!("error: {}", e);
        }
      }
    })
    .expect("Error setting Ctrl-C handler");
    Ok(Calcit::Nil)
  } else {
    CalcitErr::err_str(format!("on-control-c expected a callback function {:?}", xs))
  }
}
