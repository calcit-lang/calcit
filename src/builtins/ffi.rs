use crate::{
  primes::{Calcit, CalcitErr, CalcitItems},
  program,
};

pub fn ffi_message(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if !xs.is_empty() {
    match &xs[0] {
      Calcit::Str(s) | Calcit::Symbol(s, ..) => {
        let items = xs.to_owned().slice(1..);
        program::send_ffi_message(s.to_owned(), items);
        Ok(Calcit::Nil)
      }
      a => Err(CalcitErr::use_string(format!("&ffi-message expected string, got {}", a))),
    }
  } else {
    Err(CalcitErr::use_str("&ffi-message expected arguments but got empty"))
  }
}
