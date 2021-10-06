use cirru_edn::Edn;

use calcit_runner::{
  data::edn::{calcit_to_edn, edn_to_calcit},
  primes::{Calcit, CalcitItems, CrListWrap},
};

/// FFI protocol types
type EdnFfi = fn(args: Vec<Edn>) -> Result<Edn, String>;

// &call-dylib-edn
pub fn call_dylib_edn(xs: &CalcitItems) -> Result<Calcit, String> {
  if xs.is_empty() {
    return Err(format!(
      "&call-dylib-edn expected >2 arguments, got {}",
      CrListWrap(xs.to_owned())
    ));
  }
  let lib_name = if let Calcit::Str(s) = &xs[0] {
    s.to_owned()
  } else {
    return Err(format!("&call-dylib-edn expected a lib_name, got {}", xs[0]));
  };

  let method: String = if let Calcit::Str(s) = &xs[1] {
    s.to_owned()
  } else {
    return Err(format!("&call-dylib-edn expected a method name, got {}", xs[1]));
  };
  let mut ys: Vec<Edn> = vec![];
  for (idx, v) in xs.iter().enumerate() {
    if idx > 1 {
      ys.push(calcit_to_edn(v)?);
    }
  }

  unsafe {
    let lib = libloading::Library::new(&lib_name).expect("dylib not found");
    let func: libloading::Symbol<EdnFfi> = lib.get(method.as_bytes()).expect("dy function not found");
    let ret = func(ys.to_owned())?;
    Ok(edn_to_calcit(&ret))
  }
}