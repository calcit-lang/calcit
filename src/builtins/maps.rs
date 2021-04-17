use crate::primes::CalcitData;
use crate::primes::CalcitData::*;

use crate::builtins::math::is_even;

pub fn call_new_map(xs: im::Vector<CalcitData>) -> Result<CalcitData, String> {
  if is_even(xs.len()) {
    let n = xs.len() >> 1;
    let mut ys = im::HashMap::new();
    for i in 0..n {
      ys.insert(xs[i << 1].clone(), xs[(i << 1) + 1].clone());
    }
    Ok(CalcitMap(ys))
  } else {
    Err(String::from("&{} expected even number of arguments"))
  }
}
