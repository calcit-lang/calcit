use crate::{
  primes::{Calcit, CalcitItems},
  program,
};

pub fn ffi_message(xs: &CalcitItems) -> Result<Calcit, String> {
  if !xs.is_empty() {
    match &xs[0] {
      Calcit::Str(s) | Calcit::Symbol(s, ..) => {
        let items = xs.to_owned().slice(1..);
        program::send_ffi_message(s.to_owned(), items);
        Ok(Calcit::Nil)
      }
      a => Err(format!("&ffi-message expected string, got {}", a)),
    }
  } else {
    Err(String::from("&ffi-message expected arguments but got empty"))
  }
}

// &call-dylib:str->str
pub fn call_dylib_str_to_str(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Str(lib_name)), Some(Calcit::Str(method)), Some(Calcit::Str(file))) => unsafe {
      let lib = libloading::Library::new(lib_name).expect("dylib not found");
      let func: libloading::Symbol<fn(name_a: String) -> Result<String, String>> =
        lib.get(method.as_bytes()).expect("dy function not found");

      Ok(Calcit::Str(func(file.to_owned())?))
    },
    (Some(_), Some(_), Some(_)) => Err(String::from("&call-dylib:str->str expected 3 strings, not satisfied")),
    (_, _, _) => Err(String::from("&call-dylib:str->str expected 3 arguments, not satisfied")),
  }
}

// &call-dylib:str->unit
pub fn call_dylib_str_to_unit(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Str(lib_name)), Some(Calcit::Str(method)), Some(Calcit::Str(file))) => unsafe {
      let lib = libloading::Library::new(lib_name).expect("dylib not found");
      let func: libloading::Symbol<fn(name_a: String) -> Result<(), String>> =
        lib.get(method.as_bytes()).expect("dy function not found");

      func(file.to_owned())?;
      Ok(Calcit::Nil)
    },
    (Some(_), Some(_), Some(_)) => Err(String::from("&call-dylib:str->unit expected 3 strings, not satisfied")),
    (_, _, _) => Err(String::from(
      "&call-dylib:str->unit expected 3 arguments, not satisfied",
    )),
  }
}

// &call-dylib:str-str->str
pub fn call_dylib_str_str_to_str(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2), xs.get(3)) {
    (Some(Calcit::Str(lib_name)), Some(Calcit::Str(method)), Some(Calcit::Str(file)), Some(Calcit::Str(content))) => unsafe {
      let lib = libloading::Library::new(lib_name).expect("dylib not found");
      let func: libloading::Symbol<fn(a: String, b: String) -> Result<String, String>> =
        lib.get(method.as_bytes()).expect("dy function not found");

      Ok(Calcit::Str(func(file.to_owned(), content.to_owned())?.to_owned()))
    },
    (Some(_), Some(_), Some(_), Some(_)) => Err(String::from(
      "&call-dylib:str-str->str expected 4 strings, not satisfied",
    )),
    (_, _, _, _) => Err(String::from(
      "&call-dylib:str-str->str expected 4 arguments, not satisfied",
    )),
  }
}

// &call-dylib:str->bool
pub fn call_dylib_str_to_bool(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Str(lib_name)), Some(Calcit::Str(method)), Some(Calcit::Str(file))) => unsafe {
      let lib = libloading::Library::new(lib_name).expect("dylib not found");
      let func: libloading::Symbol<fn(name_a: String) -> Result<bool, String>> =
        lib.get(method.as_bytes()).expect("dy function not found");

      Ok(Calcit::Bool(func(file.to_owned())?.to_owned()))
    },
    (Some(_), Some(_), Some(_)) => Err(String::from("&call-dylib:str->bool expected 3 strings, not satisfied")),
    (_, _, _) => Err(String::from(
      "&call-dylib:str->bool expected 3 arguments, not satisfied",
    )),
  }
}

// &call-dylib:->str
pub fn call_dylib_to_str(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(lib_name)), Some(Calcit::Str(method))) => unsafe {
      let lib = libloading::Library::new(lib_name).expect("dylib not found");
      let func: libloading::Symbol<fn() -> Result<String, String>> =
        lib.get(method.as_bytes()).expect("dy function not found");

      Ok(Calcit::Str(func()?.to_owned()))
    },
    (Some(_), Some(_)) => Err(String::from("&call-dylib:->str expected 3 strings, not satisfied")),
    (_, _) => Err(String::from("&call-dylib:->str expected 3 arguments, not satisfied")),
  }
}

// &call-dylib-vec:str->tuple-str2
pub fn call_dylib_vec_str_to_tuple_str2(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Str(lib_name)), Some(Calcit::Str(method)), Some(Calcit::List(xs))) => unsafe {
      let mut args: Vec<String> = vec![];
      for x in xs {
        match x {
          Calcit::Str(s) => {
            args.push(s.to_owned());
          }
          _ => return Err(String::from("&call-dylib:vec-str->tuple-str2 expected string numbers")),
        }
      }

      let lib = libloading::Library::new(lib_name).expect("dylib not found");
      let func: libloading::Symbol<fn(a: Vec<String>) -> Result<(String, String), String>> =
        lib.get(method.as_bytes()).expect("dy function not found");

      let (stdout, stderr) = func(args)?.to_owned();
      Ok(Calcit::List(im::vector![Calcit::Str(stdout,), Calcit::Str(stderr)]))
    },
    (Some(_), Some(_), Some(_)) => Err(String::from(
      "&call-dylib:vec-str->tuple-str2 expected 2 strings and a list, not satisfied",
    )),
    (_, _, _) => Err(String::from(
      "&call-dylib:vec-str->tuple-str2 expected 3 argument, not satisfied",
    )),
  }
}

// &call-dylib:str->vec-str
pub fn call_dylib_str_to_vec_str(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Str(lib_name)), Some(Calcit::Str(method)), Some(Calcit::Str(file))) => unsafe {
      let lib = libloading::Library::new(lib_name).expect("dylib not found");
      let func: libloading::Symbol<fn(name_a: String) -> Result<Vec<String>, String>> =
        lib.get(method.as_bytes()).expect("dy function not found");

      let children = func(file.to_owned())?.to_owned();
      let mut ret = im::vector![];
      for c in children {
        ret.push_back(Calcit::Str(c.to_owned()));
      }
      Ok(Calcit::List(ret))
    },
    (Some(_), Some(_), Some(_)) => Err(String::from(
      "&call-dylib:str->vec-str expected 3 strings, not satisfied",
    )),
    (_, _, _) => Err(String::from(
      "&call-dylib:str->vec-str expected 3 arguments, not satisfied",
    )),
  }
}
