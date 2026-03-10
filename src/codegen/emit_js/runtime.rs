use crate::calcit;

#[inline(always)]
pub(super) fn is_cirru_string(s: &str) -> bool {
  s.starts_with('|') || s.starts_with('"')
}

#[inline(always)]
pub(super) fn get_proc_prefix(ns: &str) -> &str {
  if ns == calcit::CORE_NS { "$procs." } else { "$clt." }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn proc_prefix_for_core_and_non_core() {
    assert_eq!(get_proc_prefix(calcit::CORE_NS), "$procs.");
    assert_eq!(get_proc_prefix("app.main"), "$clt.");
  }

  #[test]
  fn cirru_string_detection() {
    assert!(is_cirru_string("|abc"));
    assert!(is_cirru_string("\"abc"));
    assert!(!is_cirru_string("abc"));
  }
}
