use core::fmt;
use std::fmt::Display;
use std::{fmt::Debug, ops::Index, sync::Arc};

use im_ternary_tree::TernaryTreeList;

use crate::Calcit;

#[derive(Debug, PartialEq, Clone, Eq, Ord, PartialOrd, Hash)]
/// abstraction over im_ternary_tree::TernaryTreeList
pub struct CalcitList(pub TernaryTreeList<Arc<Calcit>>);

impl Display for CalcitList {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "CalcitList({})", self.0.format_inline())
  }
}

impl From<TernaryTreeList<Arc<Calcit>>> for CalcitList {
  fn from(xs: TernaryTreeList<Arc<Calcit>>) -> CalcitList {
    CalcitList(xs)
  }
}

impl From<CalcitList> for TernaryTreeList<Calcit> {
  fn from(xs: CalcitList) -> TernaryTreeList<Calcit> {
    let mut ys = TernaryTreeList::Empty;
    for x in &xs.0 {
      ys = ys.push((**x).to_owned());
    }
    ys
  }
}

impl From<&CalcitList> for TernaryTreeList<Calcit> {
  fn from(xs: &CalcitList) -> TernaryTreeList<Calcit> {
    let mut ys = TernaryTreeList::Empty;
    for x in &xs.0 {
      ys = ys.push((**x).to_owned());
    }
    ys
  }
}

// TODO maybe slow
impl From<&TernaryTreeList<Calcit>> for CalcitList {
  fn from(xs: &TernaryTreeList<Calcit>) -> CalcitList {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push(Arc::new(x.to_owned()));
    }
    CalcitList(ys)
  }
}

impl From<Vec<Calcit>> for CalcitList {
  fn from(xs: Vec<Calcit>) -> CalcitList {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push(Arc::new(x));
    }
    CalcitList(ys)
  }
}

impl From<&Vec<Arc<Calcit>>> for CalcitList {
  fn from(xs: &Vec<Arc<Calcit>>) -> CalcitList {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push(x.to_owned());
    }
    CalcitList(ys)
  }
}

impl From<Vec<Arc<Calcit>>> for CalcitList {
  fn from(xs: Vec<Arc<Calcit>>) -> CalcitList {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push(x.to_owned());
    }
    CalcitList(ys)
  }
}

impl From<&[Arc<Calcit>]> for CalcitList {
  fn from(xs: &[Arc<Calcit>]) -> CalcitList {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push(x.to_owned());
    }
    CalcitList(ys)
  }
}

impl From<&[&Arc<Calcit>]> for CalcitList {
  fn from(xs: &[&Arc<Calcit>]) -> CalcitList {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push((*x).to_owned());
    }
    CalcitList(ys)
  }
}

impl From<&[Calcit; 2]> for CalcitList {
  fn from(xs: &[Calcit; 2]) -> CalcitList {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push(Arc::new(x.to_owned()));
    }
    CalcitList(ys)
  }
}

impl From<&[Arc<Calcit>; 3]> for CalcitList {
  fn from(xs: &[Arc<Calcit>; 3]) -> CalcitList {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push(x.to_owned());
    }
    CalcitList(ys)
  }
}

impl From<&[Arc<Calcit>; 1]> for Calcit {
  fn from(xs: &[Arc<Calcit>; 1]) -> Calcit {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push(x.to_owned());
    }
    Calcit::List(CalcitList(ys))
  }
}

impl From<&[Arc<Calcit>; 2]> for Calcit {
  fn from(xs: &[Arc<Calcit>; 2]) -> Calcit {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push(x.to_owned());
    }
    Calcit::List(CalcitList(ys))
  }
}

impl From<&[Arc<Calcit>; 3]> for Calcit {
  fn from(xs: &[Arc<Calcit>; 3]) -> Calcit {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push(x.to_owned());
    }
    Calcit::List(CalcitList(ys))
  }
}

impl Default for CalcitList {
  fn default() -> CalcitList {
    CalcitList(TernaryTreeList::Empty)
  }
}

impl Index<usize> for CalcitList {
  type Output = Arc<Calcit>;

  fn index(&self, idx: usize) -> &Arc<Calcit> {
    &self.0[idx]
  }
}

// experimental code to turn `&TernaryTree<_>` into iterator
impl<'a> IntoIterator for &'a CalcitList {
  type Item = &'a Arc<Calcit>;
  type IntoIter = CalcitListRefIntoIterator<'a>;

  fn into_iter(self) -> Self::IntoIter {
    CalcitListRefIntoIterator { value: &self.0, index: 0 }
  }
}

pub struct CalcitListRefIntoIterator<'a> {
  value: &'a TernaryTreeList<Arc<Calcit>>,
  index: usize,
}

impl<'a> Iterator for CalcitListRefIntoIterator<'a> {
  type Item = &'a Arc<Calcit>;
  fn next(&mut self) -> Option<Self::Item> {
    if self.index < self.value.len() {
      // println!("get: {} {}", self.value.format_inline(), self.index);
      let ret = self.value.ref_get(self.index);
      self.index += 1;
      ret
    } else {
      None
    }
  }
}

impl CalcitList {
  /// create a new list without Arc
  pub fn new_compact() -> CalcitCompactList {
    TernaryTreeList::Empty
  }
  /// create a new list without Arc
  pub fn new_inner() -> TernaryTreeList<Arc<Calcit>> {
    TernaryTreeList::Empty
  }

  pub fn new_inner_from(xs: &[Arc<Calcit>]) -> TernaryTreeList<Arc<Calcit>> {
    let mut ys = TernaryTreeList::Empty;
    for x in xs {
      ys = ys.push(x.to_owned());
    }
    ys
  }

  pub fn len(&self) -> usize {
    self.0.len()
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn get(&self, idx: usize) -> Option<Arc<Calcit>> {
    self.0.get(idx).map(|x| x.to_owned())
  }

  /// referce to inner Calcit value
  pub fn get_inner(&self, idx: usize) -> Option<&Calcit> {
    self.0.get(idx).map(|x| &**x)
  }

  pub fn to_vec(&self) -> Vec<Calcit> {
    self.0.iter().map(|x| (**x).to_owned()).collect()
  }

  pub fn push_right(&self, x: Calcit) -> Self {
    let mut ys = self.0.clone();
    ys = ys.push_right(Arc::new(x));
    CalcitList(ys)
  }

  pub fn push_right_arc(&self, x: Arc<Calcit>) -> Self {
    let mut ys = self.0.clone();
    ys = ys.push_right(x);
    CalcitList(ys)
  }

  pub fn push_left(&self, x: Calcit) -> Self {
    let mut ys = self.0.clone();
    ys = ys.push_left(Arc::new(x));
    CalcitList(ys)
  }

  pub fn drop_left(&self) -> Self {
    let mut ys = self.0.clone();
    ys = ys.drop_left();
    CalcitList(ys)
  }

  pub fn skip(&self, n: usize) -> Result<Self, String> {
    let mut ys = self.0.clone();
    ys = ys.skip(n)?;
    Ok(CalcitList(ys))
  }

  pub fn butlast(&self) -> Result<Self, String> {
    let mut ys = self.0.clone();
    ys = ys.butlast()?;
    Ok(CalcitList(ys))
  }

  pub fn slice(&self, start: usize, end: usize) -> Result<Self, String> {
    let mut ys = self.0.clone();
    ys = ys.slice(start, end)?;
    Ok(CalcitList(ys))
  }

  pub fn reverse(&self) -> Self {
    let mut ys = self.0.clone();
    ys = ys.reverse();
    CalcitList(ys)
  }

  pub fn assoc(&self, idx: usize, x: Calcit) -> Result<Self, String> {
    let mut ys = self.0.clone();
    ys = ys.assoc(idx, Arc::new(x))?;
    Ok(CalcitList(ys))
  }

  pub fn dissoc(&self, idx: usize) -> Result<Self, String> {
    let mut ys = self.0.clone();
    ys = ys.dissoc(idx)?;
    Ok(CalcitList(ys))
  }

  pub fn assoc_before(&self, idx: usize, x: Calcit) -> Result<Self, String> {
    let mut ys = self.0.clone();
    ys = ys.assoc_before(idx, Arc::new(x))?;
    Ok(CalcitList(ys))
  }

  pub fn assoc_after(&self, idx: usize, x: Calcit) -> Result<Self, String> {
    let mut ys = self.0.clone();
    ys = ys.assoc_after(idx, Arc::new(x))?;
    Ok(CalcitList(ys))
  }

  pub fn index_of(&self, x: &Calcit) -> Option<usize> {
    // TODO slow
    self.0.index_of(&Arc::new(x.to_owned()))
  }
}

pub type CalcitCompactList = TernaryTreeList<Calcit>;
