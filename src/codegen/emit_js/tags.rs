use crate::util::string::wrap_js_str;

pub(super) fn tag_access(name: &str) -> String {
  if is_simple_tag_name(name) {
    format!("_t_.{name}")
  } else {
    format!("_t_[{}]", wrap_js_str(name))
  }
}

fn is_simple_tag_name(name: &str) -> bool {
  let mut chars = name.chars();
  match chars.next() {
    Some(c) if c.is_ascii_alphabetic() => (),
    _ => return false,
  }
  chars.all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn simple_tag_uses_dot_access() {
    assert_eq!(tag_access("ok"), "_t_.ok");
    assert_eq!(tag_access("Result0"), "_t_.Result0");
  }

  #[test]
  fn complex_tag_uses_bracket_access() {
    assert_eq!(tag_access("starts-with?"), "_t_[\"starts-with?\"]");
    assert_eq!(tag_access("0abc"), "_t_[\"0abc\"]");
  }
}
