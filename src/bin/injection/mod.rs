use crate::runner;
use cirru_edn::Edn;
use std::sync::Arc;
use std::thread;

use calcit_runner::{
  builtins,
  data::edn::{calcit_to_edn, edn_to_calcit},
  primes::{Calcit, CalcitErr, CalcitItems, CrListWrap},
  runner::track,
};

/// FFI protocol types
type EdnFfi = fn(args: Vec<Edn>) -> Result<Edn, String>;
type EdnFfiFn = fn(
  args: Vec<Edn>,
  f: Arc<dyn Fn(Vec<Edn>) -> Result<Edn, String> + Send + Sync + 'static>,
  finish: Box<dyn FnOnce()>,
) -> Result<Edn, String>;

const ABI_VERSION: &str = "0.0.1";

pub fn inject_platform_apis() {
  builtins::register_import_proc("&call-dylib-edn", call_dylib_edn);
  builtins::register_import_proc("echo", echo);
  builtins::register_import_proc("println", echo);
  builtins::register_import_proc("&call-dylib-edn-fn", call_dylib_edn_fn);
  builtins::register_import_proc("async-sleep", builtins::meta::async_sleep);
  builtins::register_import_proc("on-control-c", on_ctrl_c);
}

// &call-dylib-edn
pub fn call_dylib_edn(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    return Err(CalcitErr::use_string(format!(
      "&call-dylib-edn expected >2 arguments, got {}",
      CrListWrap(xs.to_owned())
    )));
  }
  let lib_name = if let Calcit::Str(s) = &xs[0] {
    s.to_owned()
  } else {
    return Err(CalcitErr::use_string(format!("&call-dylib-edn expected a lib_name, got {}", xs[0])));
  };

  let method: String = if let Calcit::Str(s) = &xs[1] {
    s.to_owned()
  } else {
    return Err(CalcitErr::use_string(format!(
      "&call-dylib-edn expected a method name, got {}",
      xs[1]
    )));
  };
  let mut ys: Vec<Edn> = vec![];
  for (idx, v) in xs.iter().enumerate() {
    if idx > 1 {
      ys.push(calcit_to_edn(v).map_err(CalcitErr::use_string)?);
    }
  }

  unsafe {
    let lib = libloading::Library::new(&lib_name).expect("dylib not found");

    let lookup_version: libloading::Symbol<fn() -> String> = lib.get("abi_version".as_bytes()).expect("request for ABI_VERSION");
    if lookup_version() != ABI_VERSION {
      return Err(CalcitErr::use_string(format!(
        "ABI versions mismatch: {} {}",
        lookup_version(),
        ABI_VERSION
      )));
    }

    let func: libloading::Symbol<EdnFfi> = lib.get(method.as_bytes()).expect("dy function not found");
    let ret = func(ys.to_owned()).map_err(CalcitErr::use_string)?;
    Ok(edn_to_calcit(&ret))
  }
}

pub fn echo(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  let mut s = String::from("");
  for (idx, x) in xs.iter().enumerate() {
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
pub fn call_dylib_edn_fn(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() < 3 {
    return Err(CalcitErr::use_string(format!(
      "&call-dylib-edn-fn expected >3 arguments, got {}",
      CrListWrap(xs.to_owned())
    )));
  }

  let lib_name = if let Calcit::Str(s) = &xs[0] {
    s.to_owned()
  } else {
    return Err(CalcitErr::use_string(format!(
      "&call-dylib-edn-fn expected a lib_name, got {}",
      xs[0]
    )));
  };

  let method: String = if let Calcit::Str(s) = &xs[1] {
    s.to_owned()
  } else {
    return Err(CalcitErr::use_string(format!(
      "&call-dylib-edn-fn expected a method name, got {}",
      xs[1]
    )));
  };
  let mut ys: Vec<Edn> = vec![];
  let callback = xs[xs.len() - 1].clone();
  for (idx, v) in xs.iter().enumerate() {
    if idx > 1 && idx < xs.len() - 1 {
      ys.push(calcit_to_edn(v).map_err(CalcitErr::use_string)?);
    }
  }
  if let Calcit::Fn(..) = callback {
  } else {
    return Err(CalcitErr::use_string(format!(
      "expected last argument to be callback fn, got: {}",
      callback
    )));
  }

  track::track_task_add();

  let lib = unsafe {
    let lib_tmp = libloading::Library::new(&lib_name).expect("dylib not found");

    let lookup_version: libloading::Symbol<fn() -> String> = lib_tmp.get("abi_version".as_bytes()).expect("request for ABI_VERSION");
    if lookup_version() != ABI_VERSION {
      return Err(CalcitErr::use_string(format!(
        "ABI versions mismatch: {} {}",
        lookup_version(),
        ABI_VERSION
      )));
    }

    lib_tmp
  };

  let _handle = thread::spawn(move || {
    let func: libloading::Symbol<EdnFfiFn> = unsafe { lib.get(method.as_bytes()).expect("dy function not found") };
    match func(
      ys.to_owned(),
      Arc::new(move |ps: Vec<Edn>| -> Result<Edn, String> {
        if let Calcit::Fn(_, def_ns, _, def_scope, args, body) = &callback {
          let mut real_args = im::vector![];
          for p in ps {
            real_args.push_back(edn_to_calcit(&p));
          }
          let r = runner::run_fn(&real_args, def_scope, args, body, def_ns);
          match r {
            Ok(ret) => calcit_to_edn(&ret),
            Err(e) => {
              println!("[Error] thread callback failed: {}", e);
              Err(format!("Error: {}", e))
            }
          }
        } else {
          // handled above
          unreachable!(format!("expected last argument to be callback fn, got: {}", callback));
        }
      }),
      Box::new(track::track_task_release),
    ) {
      Ok(ret) => edn_to_calcit(&ret),
      Err(e) => {
        track::track_task_release();
        println!("failed to call request: {}", e);
        return Err(CalcitErr::use_string(e));
      }
    };
    Ok(Calcit::Nil)
  });

  Ok(Calcit::Nil)
}

/// need to put it here since the crate does not compile for dylib
#[no_mangle]
pub fn on_ctrl_c(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() == 1 {
    let cb = Arc::new(xs[0].to_owned());
    ctrlc::set_handler(move || {
      if let Calcit::Fn(_name, def_ns, _, def_scope, args, body) = cb.as_ref() {
        if let Err(e) = runner::run_fn(&im::vector![], def_scope, args, body, def_ns) {
          println!("error: {}", e);
        }
      }
    })
    .expect("Error setting Ctrl-C handler");
    Ok(Calcit::Nil)
  } else {
    Err(CalcitErr::use_string(format!("on-control-c expected a callback function {:?}", xs)))
  }
}
