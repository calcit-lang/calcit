use crate::util::string::wrap_js_str;

pub(super) fn to_js_import_name(ns: &str, mjs_mode: bool) -> String {
  let mut path = String::from("./");
  path.push_str(ns);
  if mjs_mode {
    path.push_str(".mjs");
  }
  wrap_js_str(&path)
}

pub(super) fn to_mjs_filename(ns: &str) -> String {
  let mut filename = String::from(ns);
  filename.push_str(".mjs");
  filename
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn import_name_with_and_without_mjs() {
    assert_eq!(to_js_import_name("calcit.core", true), "\"./calcit.core.mjs\"");
    assert_eq!(to_js_import_name("app.main", false), "\"./app.main\"");
  }

  #[test]
  fn mjs_filename_suffix() {
    assert_eq!(to_mjs_filename("app.main"), "app.main.mjs");
  }
}
