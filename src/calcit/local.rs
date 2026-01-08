use std::sync::{Arc, LazyLock, RwLock};

use crate::program::EntryBook;

/// names for local variables
static LOCAL_NAMES: LazyLock<RwLock<EntryBook<()>>> = LazyLock::new(|| RwLock::new(EntryBook::default()));

use super::{Calcit, CalcitSymbolInfo};

#[derive(Debug, Clone)]
pub struct CalcitLocal {
  /** represent local variable by idx, string value put inside dictionary */
  pub idx: u16,
  pub sym: Arc<str>,
  pub info: Arc<CalcitSymbolInfo>,
  pub location: Option<Arc<Vec<u16>>>,
  /// optional type annotation gathered during preprocessing
  pub type_info: Option<Arc<Calcit>>,
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

#[cfg(test)]
mod tests {
  use super::*;
  use cirru_edn::EdnTag;

  #[test]
  fn tracks_symbols_and_displays_args() {
    let sym = Arc::from("stage1_local_var");
    let idx = CalcitLocal::track_sym(&sym);
    assert!(idx <= u16::MAX);
    assert_eq!(CalcitLocal::display_args(&[idx]), format!("({sym})"));
  }

  #[test]
  fn stores_optional_type_info() {
    let info = Arc::new(CalcitSymbolInfo {
      at_ns: Arc::from("tests.ns"),
      at_def: Arc::from("demo"),
    });
    let type_hint = Arc::new(Calcit::tag("sample/type"));
    let local = CalcitLocal {
      idx: 0,
      sym: Arc::from("typed-var"),
      info,
      location: None,
      type_info: Some(type_hint),
    };
    match local.type_info.as_deref() {
      Some(Calcit::Tag(t)) => assert_eq!(t, &EdnTag::from("sample/type")),
      other => panic!("unexpected type info: {:?}", other),
    }
  }
}
