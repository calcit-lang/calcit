use core::fmt;
use std::fmt::Display;
use std::hash::Hash;
use std::{fmt::Debug, ops::Index, sync::Arc};

use im_ternary_tree::TernaryTreeList;

use crate::Calcit;

#[derive(Debug, Clone, Ord, PartialOrd)]
/// abstraction over im_ternary_tree::TernaryTreeList
pub enum CalcitList {
  Vector(Vec<Calcit>),
  List(TernaryTreeList<Calcit>),
}

impl Display for CalcitList {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "(&CalcitList")?;
    for x in self {
      write!(f, " {x}")?;
    }
    write!(f, ")")
  }
}

impl PartialEq for CalcitList {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (CalcitList::Vector(xs), CalcitList::Vector(ys)) => xs == ys,
      (CalcitList::List(xs), CalcitList::List(ys)) => xs == ys,
      (a, b) => {
        let a_size = a.len();
        let b_size = b.len();
        if a_size != b_size {
          return false;
        }
        for idx in 0..a_size {
          if a[idx] != b[idx] {
            return false;
          }
        }
        true
      }
    }
  }
}

impl Eq for CalcitList {}

impl Hash for CalcitList {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    for x in self {
      x.hash(state);
    }
  }
}

impl From<TernaryTreeList<Calcit>> for CalcitList {
  fn from(xs: TernaryTreeList<Calcit>) -> CalcitList {
    CalcitList::List(xs)
  }
}

impl From<CalcitList> for Calcit {
  fn from(xs: CalcitList) -> Calcit {
    Calcit::List(Arc::new(xs))
  }
}

impl From<&CalcitList> for Calcit {
  fn from(xs: &CalcitList) -> Calcit {
    Calcit::List(Arc::new(xs.to_owned()))
  }
}

impl From<CalcitList> for TernaryTreeList<Calcit> {
  fn from(xs: CalcitList) -> TernaryTreeList<Calcit> {
    let mut ys = TernaryTreeList::Empty;
    for x in &xs {
      ys = ys.push((*x).to_owned());
    }
    ys
  }
}

impl From<&CalcitList> for TernaryTreeList<Calcit> {
  fn from(xs: &CalcitList) -> TernaryTreeList<Calcit> {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push((*x).to_owned());
    }
    ys
  }
}

impl From<&TernaryTreeList<Calcit>> for CalcitList {
  fn from(xs: &TernaryTreeList<Calcit>) -> CalcitList {
    let mut ys = vec![];
    for x in xs {
      ys.push(x.to_owned());
    }
    CalcitList::Vector(ys)
  }
}

impl From<&Vec<Arc<Calcit>>> for CalcitList {
  fn from(xs: &Vec<Arc<Calcit>>) -> CalcitList {
    let mut ys = vec![];
    for x in xs {
      ys.push((**x).to_owned());
    }
    CalcitList::Vector(ys)
  }
}

impl From<&[Calcit]> for CalcitList {
  fn from(xs: &[Calcit]) -> CalcitList {
    CalcitList::Vector(xs.to_owned())
  }
}

impl From<&[Calcit; 2]> for CalcitList {
  fn from(xs: &[Calcit; 2]) -> CalcitList {
    CalcitList::Vector(xs.to_vec())
  }
}

impl From<&[Calcit; 3]> for CalcitList {
  fn from(xs: &[Calcit; 3]) -> CalcitList {
    CalcitList::Vector(xs.to_vec())
  }
}

impl Default for CalcitList {
  fn default() -> CalcitList {
    CalcitList::List(TernaryTreeList::Empty)
  }
}

impl Index<usize> for CalcitList {
  type Output = Calcit;

  fn index(&self, idx: usize) -> &Calcit {
    match self {
      CalcitList::Vector(xs) => &xs[idx],
      CalcitList::List(xs) => &xs[idx],
    }
  }
}

// experimental code to turn `&TernaryTree<_>` into iterator
impl<'a> IntoIterator for &'a CalcitList {
  type Item = &'a Calcit;
  type IntoIter = CalcitListIterator<'a>;

  fn into_iter(self) -> Self::IntoIter {
    CalcitListIterator {
      value: self,
      index: 0,
      size: self.len(),
    }
  }
}

pub struct CalcitListIterator<'a> {
  value: &'a CalcitList,
  index: usize,
  size: usize,
}

impl<'a> Iterator for CalcitListIterator<'a> {
  type Item = &'a Calcit;
  fn next(&mut self) -> Option<Self::Item> {
    if self.index < self.size {
      // println!("get: {} {}", self.value.format_inline(), self.index);
      let ret = self.value.get(self.index);
      self.index += 1;
      ret
    } else {
      None
    }
  }
}

impl CalcitList {
  pub fn new_inner() -> TernaryTreeList<Calcit> {
    TernaryTreeList::Empty
  }

  pub fn new_inner_from(xs: &[Calcit]) -> TernaryTreeList<Calcit> {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push(x.to_owned());
    }
    ys
  }

  pub fn len(&self) -> usize {
    match self {
      CalcitList::Vector(xs) => xs.len(),
      CalcitList::List(xs) => xs.len(),
    }
  }

  pub fn is_empty(&self) -> bool {
    match self {
      CalcitList::Vector(xs) => xs.is_empty(),
      CalcitList::List(xs) => xs.is_empty(),
    }
  }

  pub fn get(&self, idx: usize) -> Option<&Calcit> {
    match self {
      CalcitList::Vector(xs) => xs.get(idx),
      CalcitList::List(xs) => xs.get(idx),
    }
  }

  pub fn first(&self) -> Option<&Calcit> {
    match self {
      CalcitList::Vector(xs) => xs.first(),
      CalcitList::List(xs) => xs.first(),
    }
  }

  pub fn into_list(self) -> Self {
    match self {
      CalcitList::Vector(xs) => CalcitList::List(TernaryTreeList::from(xs)),
      CalcitList::List(_) => self.to_owned(),
    }
  }

  pub fn to_vec(&self) -> Vec<Calcit> {
    match self {
      CalcitList::Vector(xs) => xs.to_owned(),
      CalcitList::List(xs) => xs.to_vec(),
    }
  }

  pub fn push_right(&self, x: Calcit) -> Self {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = TernaryTreeList::from(xs);
        ys = ys.push(x);
        CalcitList::List(ys)
      }
      CalcitList::List(xs) => CalcitList::List(xs.push(x)),
    }
  }

  pub fn push_left(&self, x: Calcit) -> Self {
    match self {
      CalcitList::Vector(xs) => CalcitList::List(TernaryTreeList::from(xs).prepend(x)),
      CalcitList::List(xs) => CalcitList::List(xs.push_left(x)),
    }
  }

  pub fn drop_left(&self) -> Self {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = TernaryTreeList::Empty;
        for x in xs.iter().skip(1) {
          ys = ys.push(x.to_owned());
        }
        CalcitList::List(ys)
      }
      CalcitList::List(xs) => CalcitList::List(xs.drop_left()),
    }
  }

  pub fn skip(&self, n: usize) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = TernaryTreeList::Empty;
        for x in xs.iter().skip(n) {
          ys = ys.push(x.to_owned());
        }
        Ok(CalcitList::List(ys))
      }
      CalcitList::List(xs) => Ok(CalcitList::List(xs.skip(n)?)),
    }
  }

  pub fn butlast(&self) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = TernaryTreeList::Empty;
        for x in xs.iter().take(xs.len() - 1) {
          ys = ys.push(x.to_owned());
        }
        Ok(CalcitList::List(ys))
      }
      CalcitList::List(xs) => Ok(CalcitList::List(xs.butlast()?)),
    }
  }

  pub fn slice(&self, start: usize, end: usize) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) => {
        let ys = TernaryTreeList::from(xs);
        Ok(CalcitList::List(ys.slice(start, end)?))
      }
      CalcitList::List(xs) => Ok(CalcitList::List(xs.slice(start, end)?)),
    }
  }

  pub fn reverse(&self) -> Self {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = TernaryTreeList::Empty;
        for x in xs.iter() {
          ys = ys.prepend(x.to_owned());
        }
        CalcitList::List(ys)
      }
      CalcitList::List(xs) => CalcitList::List(xs.reverse()),
    }
  }

  pub fn assoc(&self, idx: usize, x: Calcit) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = TernaryTreeList::from(xs);
        ys = ys.assoc(idx, x)?;
        Ok(CalcitList::List(ys))
      }
      CalcitList::List(xs) => Ok(CalcitList::List(xs.assoc(idx, x)?)),
    }
  }

  pub fn dissoc(&self, idx: usize) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = TernaryTreeList::from(xs);
        ys = ys.dissoc(idx)?;
        Ok(CalcitList::List(ys))
      }
      CalcitList::List(xs) => Ok(CalcitList::List(xs.dissoc(idx)?)),
    }
  }

  pub fn assoc_before(&self, idx: usize, x: Calcit) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = TernaryTreeList::from(xs);
        ys = ys.assoc_before(idx, x)?;
        Ok(CalcitList::List(ys))
      }
      CalcitList::List(xs) => Ok(CalcitList::List(xs.assoc_before(idx, x)?)),
    }
  }

  pub fn assoc_after(&self, idx: usize, x: Calcit) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = TernaryTreeList::from(xs);
        ys = ys.assoc_after(idx, x)?;
        Ok(CalcitList::List(ys))
      }
      CalcitList::List(xs) => Ok(CalcitList::List(xs.assoc_after(idx, x)?)),
    }
  }

  pub fn index_of(&self, x: &Calcit) -> Option<usize> {
    match self {
      CalcitList::Vector(xs) => xs.iter().position(|y| y == x),
      CalcitList::List(xs) => xs.index_of(x),
    }
  }

  pub fn traverse(&self, f: &mut dyn FnMut(&Calcit)) {
    match self {
      CalcitList::Vector(xs) => {
        for x in xs {
          f(x);
        }
      }
      CalcitList::List(xs) => {
        xs.traverse(f);
      }
    }
  }

  pub fn traverse_result<S>(&self, f: &mut dyn FnMut(&Calcit) -> Result<(), S>) -> Result<(), S> {
    // self.0.traverse_result(f)
    match self {
      CalcitList::Vector(xs) => {
        for x in xs {
          f(x)?;
        }
        Ok(())
      }
      CalcitList::List(xs) => xs.traverse_result(f),
    }
  }

  pub fn iter(&self) -> CalcitListIterator {
    CalcitListIterator {
      value: self,
      index: 0,
      size: self.len(),
    }
  }
}
