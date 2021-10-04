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
