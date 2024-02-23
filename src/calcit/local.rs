use std::sync::{Arc, RwLock};

use crate::program::EntryBook;

lazy_static! {
  /// names for local variables
  static ref LOCAL_NAMES: RwLock<EntryBook<()>> = RwLock::new(EntryBook::default());
}

use super::CalcitSymbolInfo;

#[derive(Debug, Clone)]
pub struct CalcitLocal {
  /** represent local varaible by idx, string value put inside dictionary */
  pub idx: u16,
  pub sym: Arc<str>,
  pub info: Arc<CalcitSymbolInfo>,
  pub location: Option<Arc<Vec<u8>>>,
}

impl CalcitLocal {
  pub fn track_sym(sym: &Arc<str>) -> u16 {
    let mut locals = LOCAL_NAMES.write().expect("read local names");
    match locals.lookup_mut(sym) {
      Some((_, idx)) => idx,
      None => {
        let idx = locals.len();
        locals.insert(sym.clone(), ());
        idx as u16
      }
    }
  }

  pub fn read_name(idx: u16) -> String {
    let locals = LOCAL_NAMES.read().expect("read local names");
    let (_, s) = locals.load(idx);
    s.to_string()
  }

  /// display local variables from numbers
  pub fn display_args(xs: &[u16]) -> String {
    let mut s = "(".to_owned();
    let mut first = true;
    for i in xs {
      let name = Self::read_name(*i);
      if first {
        first = false;
      } else {
        s.push(' ');
      }
      s.push_str(&name);
    }
    s.push(')');
    s
  }
}
