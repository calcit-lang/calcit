use core::fmt;
use std::cmp::Ordering;
use std::fmt::Display;
use std::hash::Hash;
use std::{fmt::Debug, ops::Index, sync::Arc};

use finger_vec::FingerVec;

use crate::Calcit;

/// Internal metadata attached to contiguous call nodes. It never
/// changes the language-level list value represented by the stored items.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum CalcitCallKind {
  #[default]
  Normal,
  NumberBinary(CalcitNumberBinaryOp),
  /// A compiler-appended signature, distinct from source-authored hints.
  GeneratedFnHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CalcitNumberBinaryOp {
  Add,
  Subtract,
  Multiply,
  Divide,
  Remainder,
  LessThan,
  GreaterThan,
}

#[derive(Debug, Clone)]
/// abstraction over finger_vec::FingerVec
pub enum CalcitList {
  Vector(Vec<Calcit>),
  Call(Vec<Calcit>, CalcitCallKind),
  List(FingerVec<Calcit>),
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
      (CalcitList::Call(xs, _), CalcitList::Call(ys, _)) => xs == ys,
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

impl Ord for CalcitList {
  fn cmp(&self, other: &Self) -> Ordering {
    self.iter().cmp(other.iter())
  }
}

impl PartialOrd for CalcitList {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Hash for CalcitList {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    for x in self {
      x.hash(state);
    }
  }
}

impl From<FingerVec<Calcit>> for CalcitList {
  fn from(xs: FingerVec<Calcit>) -> CalcitList {
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

impl From<CalcitList> for FingerVec<Calcit> {
  fn from(xs: CalcitList) -> FingerVec<Calcit> {
    match xs {
      CalcitList::List(ys) => ys,
      CalcitList::Vector(ys) | CalcitList::Call(ys, _) => FingerVec::from(ys),
    }
  }
}

impl From<&CalcitList> for FingerVec<Calcit> {
  fn from(xs: &CalcitList) -> FingerVec<Calcit> {
    match xs {
      CalcitList::List(ys) => ys.clone(),
      CalcitList::Vector(ys) | CalcitList::Call(ys, _) => FingerVec::from(ys),
    }
  }
}

impl From<&FingerVec<Calcit>> for CalcitList {
  fn from(xs: &FingerVec<Calcit>) -> CalcitList {
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
    CalcitList::List(FingerVec::new())
  }
}

impl Index<usize> for CalcitList {
  type Output = Calcit;

  fn index(&self, idx: usize) -> &Calcit {
    match self {
      CalcitList::Vector(xs) => &xs[idx],
      CalcitList::Call(xs, _) => &xs[idx],
      CalcitList::List(xs) => &xs[idx],
    }
  }
}

impl<'a> IntoIterator for &'a CalcitList {
  type Item = &'a Calcit;
  type IntoIter = CalcitListIterator<'a>;

  fn into_iter(self) -> Self::IntoIter {
    self.iter()
  }
}

/// Iterates the stored representation directly instead of indexing per item.
pub enum CalcitListIterator<'a> {
  Slice(std::slice::Iter<'a, Calcit>),
  Tree(finger_vec::Iter<'a, Calcit>),
}

impl<'a> Iterator for CalcitListIterator<'a> {
  type Item = &'a Calcit;
  fn next(&mut self) -> Option<Self::Item> {
    match self {
      CalcitListIterator::Slice(it) => it.next(),
      CalcitListIterator::Tree(it) => it.next(),
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    match self {
      CalcitListIterator::Slice(it) => it.size_hint(),
      CalcitListIterator::Tree(it) => it.size_hint(),
    }
  }
}

impl ExactSizeIterator for CalcitListIterator<'_> {}

/// Borrowed read-only range over a Calcit list. Executable calls use this to
/// pass argument tails without allocating a second list.
#[derive(Debug, Clone, Copy)]
pub struct CalcitListView<'a> {
  value: &'a CalcitList,
  start: usize,
  end: usize,
}

impl Display for CalcitListView<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "(&CalcitList")?;
    for x in self {
      write!(f, " {x}")?;
    }
    write!(f, ")")
  }
}

impl<'a> CalcitListView<'a> {
  pub fn len(&self) -> usize {
    self.end - self.start
  }

  pub fn is_empty(&self) -> bool {
    self.start == self.end
  }

  pub fn get(&self, idx: usize) -> Option<&'a Calcit> {
    if idx < self.len() { self.value.get(self.start + idx) } else { None }
  }

  pub fn first(&self) -> Option<&'a Calcit> {
    self.get(0)
  }

  pub fn skip(&self, n: usize) -> Result<Self, String> {
    if n > self.len() {
      Err(format!("cannot skip {n} item(s) from a list view of length {}", self.len()))
    } else {
      Ok(Self {
        value: self.value,
        start: self.start + n,
        end: self.end,
      })
    }
  }

  pub fn to_vec(&self) -> Vec<Calcit> {
    self.iter().cloned().collect()
  }

  pub fn traverse_result<S>(&self, f: &mut dyn FnMut(&Calcit) -> Result<(), S>) -> Result<(), S> {
    for item in self {
      f(item)?;
    }
    Ok(())
  }

  pub fn iter(&self) -> CalcitListViewIterator<'a> {
    CalcitListViewIterator {
      value: self.value,
      index: self.start,
      end: self.end,
    }
  }
}

impl Index<usize> for CalcitListView<'_> {
  type Output = Calcit;

  fn index(&self, idx: usize) -> &Calcit {
    self
      .get(idx)
      .unwrap_or_else(|| panic!("list view index {idx} out of bounds for length {}", self.len()))
  }
}

impl<'list> IntoIterator for &CalcitListView<'list> {
  type Item = &'list Calcit;
  type IntoIter = CalcitListViewIterator<'list>;

  fn into_iter(self) -> Self::IntoIter {
    self.iter()
  }
}

pub struct CalcitListViewIterator<'a> {
  value: &'a CalcitList,
  index: usize,
  end: usize,
}

impl<'a> Iterator for CalcitListViewIterator<'a> {
  type Item = &'a Calcit;

  fn next(&mut self) -> Option<Self::Item> {
    if self.index < self.end {
      let ret = self.value.get(self.index);
      self.index += 1;
      ret
    } else {
      None
    }
  }
}

impl CalcitList {
  pub fn new_inner() -> FingerVec<Calcit> {
    FingerVec::new()
  }

  pub fn new_inner_from(xs: &[Calcit]) -> FingerVec<Calcit> {
    FingerVec::from(xs)
  }

  pub fn len(&self) -> usize {
    match self {
      CalcitList::Vector(xs) => xs.len(),
      CalcitList::Call(xs, _) => xs.len(),
      CalcitList::List(xs) => xs.len(),
    }
  }

  pub fn is_empty(&self) -> bool {
    match self {
      CalcitList::Vector(xs) => xs.is_empty(),
      CalcitList::Call(xs, _) => xs.is_empty(),
      CalcitList::List(xs) => xs.is_empty(),
    }
  }

  pub fn get(&self, idx: usize) -> Option<&Calcit> {
    match self {
      CalcitList::Vector(xs) => xs.get(idx),
      CalcitList::Call(xs, _) => xs.get(idx),
      CalcitList::List(xs) => xs.get(idx),
    }
  }

  pub fn first(&self) -> Option<&Calcit> {
    match self {
      CalcitList::Vector(xs) => xs.first(),
      CalcitList::Call(xs, _) => xs.first(),
      CalcitList::List(xs) => xs.first(),
    }
  }

  pub fn view(&self) -> CalcitListView<'_> {
    CalcitListView {
      value: self,
      start: 0,
      end: self.len(),
    }
  }

  pub fn view_from(&self, start: usize) -> Result<CalcitListView<'_>, String> {
    self.view().skip(start)
  }

  pub fn executable(items: Vec<Calcit>, kind: CalcitCallKind) -> Self {
    CalcitList::Call(items, kind)
  }

  pub fn call_kind(&self) -> CalcitCallKind {
    match self {
      CalcitList::Call(_, kind) => *kind,
      CalcitList::Vector(_) | CalcitList::List(_) => CalcitCallKind::Normal,
    }
  }

  pub fn into_list(self) -> Self {
    match self {
      CalcitList::Vector(xs) => CalcitList::List(FingerVec::from(xs)),
      CalcitList::Call(xs, _) => CalcitList::List(FingerVec::from(xs)),
      CalcitList::List(_) => self.to_owned(),
    }
  }

  pub fn to_vec(&self) -> Vec<Calcit> {
    match self {
      CalcitList::Vector(xs) => xs.to_owned(),
      CalcitList::Call(xs, _) => xs.to_owned(),
      CalcitList::List(xs) => xs.to_vec(),
    }
  }

  pub fn push_right(&self, x: Calcit) -> Self {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = FingerVec::from(xs);
        ys = ys.push(x);
        CalcitList::List(ys)
      }
      CalcitList::Call(xs, _) => CalcitList::List(FingerVec::from(xs).push(x)),
      CalcitList::List(xs) => CalcitList::List(xs.push(x)),
    }
  }

  pub fn push_left(&self, x: Calcit) -> Self {
    match self {
      CalcitList::Vector(xs) => CalcitList::List(FingerVec::from(xs).prepend(x)),
      CalcitList::Call(xs, _) => CalcitList::List(FingerVec::from(xs).prepend(x)),
      CalcitList::List(xs) => CalcitList::List(xs.push_left(x)),
    }
  }

  pub fn drop_left(&self) -> Self {
    match self {
      CalcitList::Vector(xs) | CalcitList::Call(xs, _) => CalcitList::List(FingerVec::from(xs.get(1..).unwrap_or(&[]))),
      CalcitList::List(xs) => CalcitList::List(xs.drop_left()),
    }
  }

  pub fn skip(&self, n: usize) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) | CalcitList::Call(xs, _) => Ok(CalcitList::List(FingerVec::from(xs.get(n..).unwrap_or(&[])))),
      CalcitList::List(xs) => Ok(CalcitList::List(xs.skip(n)?)),
    }
  }

  pub fn butlast(&self) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) | CalcitList::Call(xs, _) => {
        if xs.is_empty() {
          Err(String::from("calling butlast on empty"))
        } else {
          Ok(CalcitList::List(FingerVec::from(&xs[..xs.len() - 1])))
        }
      }
      CalcitList::List(xs) => Ok(CalcitList::List(xs.butlast()?)),
    }
  }

  pub fn slice(&self, start: usize, end: usize) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) => {
        let ys = FingerVec::from(xs);
        Ok(CalcitList::List(ys.slice(start, end)?))
      }
      CalcitList::Call(xs, _) => {
        let ys = FingerVec::from(xs);
        Ok(CalcitList::List(ys.slice(start, end)?))
      }
      CalcitList::List(xs) => Ok(CalcitList::List(xs.slice(start, end)?)),
    }
  }

  pub fn reverse(&self) -> Self {
    match self {
      CalcitList::Vector(xs) | CalcitList::Call(xs, _) => CalcitList::List(xs.iter().rev().cloned().collect()),
      CalcitList::List(xs) => CalcitList::List(xs.reverse()),
    }
  }

  pub fn assoc(&self, idx: usize, x: Calcit) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = FingerVec::from(xs);
        ys = ys.assoc(idx, x)?;
        Ok(CalcitList::List(ys))
      }
      CalcitList::Call(xs, _) => {
        let mut ys = FingerVec::from(xs);
        ys = ys.assoc(idx, x)?;
        Ok(CalcitList::List(ys))
      }
      CalcitList::List(xs) => Ok(CalcitList::List(xs.assoc(idx, x)?)),
    }
  }

  pub fn dissoc(&self, idx: usize) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = FingerVec::from(xs);
        ys = ys.dissoc(idx)?;
        Ok(CalcitList::List(ys))
      }
      CalcitList::Call(xs, _) => {
        let mut ys = FingerVec::from(xs);
        ys = ys.dissoc(idx)?;
        Ok(CalcitList::List(ys))
      }
      CalcitList::List(xs) => Ok(CalcitList::List(xs.dissoc(idx)?)),
    }
  }

  pub fn assoc_before(&self, idx: usize, x: Calcit) -> Result<Self, String> {
    match self {
      CalcitList::Vector(xs) => {
        let mut ys = FingerVec::from(xs);
        ys = ys.assoc_before(idx, x)?;
        Ok(CalcitList::List(ys))
      }
      CalcitList::Call(xs, _) => {
        let mut ys = FingerVec::from(xs);
        ys = ys.assoc_before(idx, x)?;
        Ok(CalcitList::List(ys))
      }
      CalcitList::List(xs) => Ok(CalcitList::List(xs.assoc_before(idx, x)?)),
    }
  }

  pub fn assoc_after(&self, idx: usize, x: Calcit) -> Result<Self, String> {
    let base_list = match self {
      CalcitList::Vector(xs) => FingerVec::from(xs),
      CalcitList::Call(xs, _) => FingerVec::from(xs),
      CalcitList::List(xs) => xs.clone(),
    };
    Ok(CalcitList::List(base_list.assoc_after(idx, x)?))
  }

  pub fn index_of(&self, x: &Calcit) -> Option<usize> {
    match self {
      CalcitList::Vector(xs) => xs.iter().position(|y| y == x),
      CalcitList::Call(xs, _) => xs.iter().position(|y| y == x),
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
      CalcitList::Call(xs, _) => {
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
      CalcitList::Call(xs, _) => {
        for x in xs {
          f(x)?;
        }
        Ok(())
      }
      CalcitList::List(xs) => xs.traverse_result(f),
    }
  }

  pub fn iter(&self) -> CalcitListIterator<'_> {
    match self {
      CalcitList::Vector(xs) | CalcitList::Call(xs, _) => CalcitListIterator::Slice(xs.iter()),
      CalcitList::List(xs) => CalcitListIterator::Tree(xs.iter()),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn values() -> Vec<Calcit> {
    vec![Calcit::Number(1.0), Calcit::Number(2.0), Calcit::Number(3.0), Calcit::Number(4.0)]
  }

  #[test]
  fn borrowed_views_read_all_list_storage_kinds() {
    let vector = CalcitList::Vector(values());
    let call = CalcitList::executable(values(), CalcitCallKind::NumberBinary(CalcitNumberBinaryOp::Add));
    let persistent = CalcitList::List(FingerVec::from(values()));

    for source in [&vector, &call, &persistent] {
      let tail = source.view_from(1).expect("valid argument tail");
      assert_eq!(tail.len(), 3);
      assert_eq!(tail.first(), Some(&Calcit::Number(2.0)));
      assert_eq!(tail[2], Calcit::Number(4.0));
      assert_eq!(tail.iter().cloned().collect::<Vec<_>>(), values()[1..]);

      let nested = tail.skip(2).expect("valid nested tail");
      assert_eq!(nested.to_vec(), vec![Calcit::Number(4.0)]);
    }
  }

  #[test]
  fn borrowed_view_rejects_out_of_bounds_skip() {
    let source = CalcitList::Vector(values());
    let view = source.view_from(1).expect("valid argument tail");

    assert!(view.get(3).is_none());
    assert!(view.skip(4).is_err());
  }

  #[test]
  fn executable_metadata_does_not_change_list_value_semantics() {
    let normal = CalcitList::executable(values(), CalcitCallKind::Normal);
    let specialized = CalcitList::executable(values(), CalcitCallKind::NumberBinary(CalcitNumberBinaryOp::Add));
    let vector = CalcitList::Vector(values());
    let persistent = CalcitList::List(FingerVec::from(values()));

    assert_eq!(normal, specialized);
    assert_eq!(specialized, vector);
    assert_eq!(specialized, persistent);
    assert_eq!(normal.cmp(&specialized), Ordering::Equal);
    assert_eq!(specialized.call_kind(), CalcitCallKind::NumberBinary(CalcitNumberBinaryOp::Add));
    assert_eq!(specialized.push_right(Calcit::Number(5.0)).call_kind(), CalcitCallKind::Normal);
  }
}
