pub mod number;
pub mod string;

use std::cmp::Ordering;

use crate::primes::Calcit;
use im_ternary_tree::TernaryTreeList;

pub fn skip(xs: &TernaryTreeList<Calcit>, skipped: usize) -> Result<TernaryTreeList<Calcit>, String> {
  xs.slice(skipped, xs.len())
}

pub fn slice(xs: &TernaryTreeList<Calcit>, from: usize, to: usize) -> Result<TernaryTreeList<Calcit>, String> {
  xs.slice(from, to)
}

pub fn contains(xs: &TernaryTreeList<Calcit>, y: &Calcit) -> bool {
  xs.index_of(y).is_some()
}

pub fn insert(xs: &TernaryTreeList<Calcit>, pos: usize, y: Calcit) -> TernaryTreeList<Calcit> {
  let mut ys: TernaryTreeList<Calcit> = TernaryTreeList::Empty;

  match pos.cmp(&xs.len()) {
    Ordering::Less => ys = ys.assoc_before(pos, y).unwrap(), // TODO
    Ordering::Equal => {
      ys = xs.to_owned();
      ys = ys.push(y.to_owned());
    }
    Ordering::Greater => {
      println!("[Error] TODO error")
    }
  }
  ys
}
