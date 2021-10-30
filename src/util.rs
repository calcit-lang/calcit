pub mod number;
pub mod string;

use std::cmp::Ordering;

use crate::primes::Calcit;

pub fn skip(xs: &rpds::VectorSync<Calcit>, skipped: usize) -> rpds::VectorSync<Calcit> {
  let mut ys: rpds::VectorSync<Calcit> = rpds::Vector::new_sync();
  for (idx, x) in xs.iter().enumerate() {
    if idx >= skipped {
      ys.push_back_mut(x.to_owned());
    }
  }
  ys
}

pub fn slice(xs: &rpds::VectorSync<Calcit>, from: usize, to: usize) -> rpds::VectorSync<Calcit> {
  let mut ys: rpds::VectorSync<Calcit> = rpds::Vector::new_sync();
  for (idx, x) in xs.iter().enumerate() {
    if idx >= from && idx < to {
      ys.push_back_mut(x.to_owned());
    }
  }
  ys
}

pub fn contains(xs: &rpds::VectorSync<Calcit>, y: &Calcit) -> bool {
  for x in xs.iter() {
    if x == y {
      return true;
    }
  }
  false
}

pub fn insert(xs: &rpds::VectorSync<Calcit>, pos: usize, y: Calcit) -> rpds::VectorSync<Calcit> {
  let mut ys: rpds::VectorSync<Calcit> = rpds::Vector::new_sync();

  match pos.cmp(&xs.len()) {
    Ordering::Less => {
      for (idx, x) in xs.iter().enumerate() {
        if idx == pos {
          ys.push_back_mut(y.to_owned());
        }
        ys.push_back_mut(x.to_owned());
      }
    }
    Ordering::Equal => {
      ys = xs.to_owned();
      ys.push_back_mut(y.to_owned());
    }
    Ordering::Greater => {
      println!("[Error] TODO error")
    }
  }
  ys
}
