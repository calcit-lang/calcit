use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

use super::{CalcitSymbolInfo, CalcitTypeAnnotation};

#[derive(Default)]
struct LocalNameInterner {
  by_idx: Vec<Arc<str>>,
  by_name: HashMap<Arc<str>, u16, foldhash::fast::FixedState>,
}

/// names for local variables
static LOCAL_NAMES: LazyLock<RwLock<LocalNameInterner>> = LazyLock::new(|| RwLock::new(LocalNameInterner::default()));

#[derive(Debug, Clone)]
pub struct CalcitLocal {
  /** represent local variable by idx, string value put inside dictionary */
  pub idx: u16,
  pub sym: Arc<str>,
  pub info: Arc<CalcitSymbolInfo>,
  pub location: Option<Arc<Vec<u16>>>,
  /// type annotation gathered during preprocessing, defaults to Dynamic
  pub type_info: Arc<CalcitTypeAnnotation>,
}

impl CalcitLocal {
  pub fn lookup_sym(sym: &str) -> Option<u16> {
    LOCAL_NAMES.read().expect("read local names").by_name.get(sym).copied()
  }

  pub fn track_sym(sym: &Arc<str>) -> u16 {
    if let Some(&idx) = LOCAL_NAMES.read().expect("read local names").by_name.get(sym.as_ref()) {
      return idx;
    }
    let mut locals = LOCAL_NAMES.write().expect("write local names");
    if let Some(&idx) = locals.by_name.get(sym.as_ref()) {
      return idx;
    }
    let idx = u16::try_from(locals.by_idx.len()).expect("local variable table exceeded u16::MAX distinct names");
    locals.by_idx.push(Arc::clone(sym));
    locals.by_name.insert(Arc::clone(sym), idx);
    idx
  }

  pub fn read_name(idx: u16) -> String {
    let locals = LOCAL_NAMES.read().expect("read local names");
    match locals.by_idx.get(idx as usize) {
      Some(s) => s.to_string(),
      None => format!("<local:{idx}>"),
    }
  }

  /// Append comma-separated local variable names under a single read lock.
  pub(crate) fn format_names(indices: impl IntoIterator<Item = u16>) -> String {
    use std::fmt::Write as _;
    let locals = LOCAL_NAMES.read().expect("read local names");
    let mut vars = String::new();
    for (i, idx) in indices.into_iter().enumerate() {
      if i > 0 {
        vars.push(',');
      }
      match locals.by_idx.get(idx as usize) {
        Some(s) => vars.push_str(s),
        None => {
          let _ = write!(vars, "<local:{idx}>");
        }
      }
    }
    vars
  }

  /// display local variables from numbers
  pub fn display_args(xs: &[u16]) -> String {
    use std::fmt::Write as _;
    let locals = LOCAL_NAMES.read().expect("read local names");
    let mut s = "(".to_owned();
    for (i, &idx) in xs.iter().enumerate() {
      if i > 0 {
        s.push(' ');
      }
      match locals.by_idx.get(idx as usize) {
        Some(name) => s.push_str(name),
        None => {
          let _ = write!(s, "<local:{idx}>");
        }
      }
    }
    s.push(')');
    s
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tracks_symbols_and_displays_args() {
    let sym = Arc::from("stage1_local_var");
    let idx = CalcitLocal::track_sym(&sym);
    assert_eq!(CalcitLocal::display_args(&[idx]), format!("({sym})"));
  }

  #[test]
  fn stores_optional_type_info() {
    let info = Arc::new(CalcitSymbolInfo {
      at_ns: Arc::from("tests.ns"),
      at_def: Arc::from("demo"),
    });
    let type_hint = Arc::new(CalcitTypeAnnotation::from_tag_name("sample/type"));
    let local = CalcitLocal {
      idx: 0,
      sym: Arc::from("typed-var"),
      info,
      location: None,
      type_info: type_hint,
    };
    assert!(matches!(local.type_info.as_ref(), CalcitTypeAnnotation::Tag));
  }
}
