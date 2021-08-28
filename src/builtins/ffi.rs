use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::{
  primes::{Calcit, CalcitItems},
  program,
};

pub fn ffi_message(xs: &CalcitItems) -> Result<Calcit, String> {
  if xs.len() >= 1 {
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
    (Some(Calcit::Str(file_name)), Some(Calcit::Str(method)), Some(Calcit::Str(file))) => unsafe {
      let lib = libloading::Library::new(file_name).expect("dylib not found");
      let func: libloading::Symbol<unsafe extern "C" fn(name_a: *const c_char) -> *mut c_char> =
        lib.get(method.as_bytes()).expect("dy function not found");
      let a = CString::new(file.as_bytes()).expect("should not fail");
      let c_name = a.as_ptr();

      let ret = CStr::from_ptr(func(c_name)).to_str().unwrap();
      Ok(Calcit::Str(ret.to_owned()))
    },
    (Some(_), Some(_), Some(_)) => Err(String::from("&call-dylib:str->str expected 3 strings, not satisfied")),
    (_, _, _) => Err(String::from("&call-dylib:str->str expected 3 arguments, not satisfied")),
  }
}

// &call-dylib:str:str->str
pub fn call_dylib_str_str_to_str(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2), xs.get(3)) {
    (Some(Calcit::Str(file_name)), Some(Calcit::Str(method)), Some(Calcit::Str(file)), Some(Calcit::Str(content))) => unsafe {
      let lib = libloading::Library::new(file_name).expect("dylib not found");
      let func: libloading::Symbol<unsafe extern "C" fn(name_a: *const c_char, content: *const c_char) -> *mut c_char> =
        lib.get(method.as_bytes()).expect("dy function not found");

      let a = CString::new(file.as_bytes()).expect("should not fail");
      let c_name = a.as_ptr();

      let b = CString::new(content.as_bytes()).expect("should not fail");
      let c_content = b.as_ptr();

      let ret = CStr::from_ptr(func(c_name, c_content)).to_str().unwrap();
      Ok(Calcit::Str(ret.to_owned()))
    },
    (Some(_), Some(_), Some(_), Some(_)) => Err(String::from(
      "&call-dylib:str:str->str expected 4 strings, not satisfied",
    )),
    (_, _, _, _) => Err(String::from(
      "&call-dylib:str:str->str expected 4 arguments, not satisfied",
    )),
  }
}
