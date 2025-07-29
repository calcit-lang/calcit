use std::sync::Arc;

pub fn is_digit(c: char) -> bool {
  c.is_ascii_digit()
}

pub fn is_letter(c: char) -> bool {
  c.is_ascii_alphabetic()
}

// TODO, not ready to use
#[allow(dead_code)]
pub fn matches_float(xs: &str) -> bool {
  if xs.is_empty() {
    return false;
  }
  xs.parse::<f64>().is_ok()
}

#[allow(dead_code)]
pub fn matches_simple_var(xs: &str) -> bool {
  if xs.is_empty() {
    return false;
  }
  xs.chars()
    .all(|x| is_letter(x) || is_digit(x) || matches!(x, '-' | '!' | '*' | '?'))
}

pub fn matches_digits(xs: &str) -> bool {
  if xs.is_empty() {
    return false;
  }
  xs.chars().all(is_digit)
}

pub fn matches_js_var(xs: &str) -> bool {
  if xs.is_empty() {
    return false;
  }
  for (idx, x) in xs.chars().enumerate() {
    if is_letter(x) || x == '_' || x == '$' || (idx > 0 && is_digit(x)) {
      // ok
    } else {
      return false;
    }
  }
  true
}

pub fn has_ns_part(x: &str) -> bool {
  match x.find('/') {
    Some(try_slash_pos) => try_slash_pos >= 1 && try_slash_pos < x.len() - 1,
    None => false,
  }
}

/// js/JSON.stringify -like API
pub fn wrap_js_str(s: &str) -> String {
  let mut c: String = String::with_capacity(s.len() + 2);
  c.push('"');
  for i in s.escape_default() {
    c.push(i);
  }
  c.push('"');
  c
}

pub fn extract_ns_def(s: &str) -> Result<(String, String), String> {
  let pieces: Vec<&str> = s.split('/').collect();
  if pieces.len() == 2 {
    Ok((pieces[0].to_owned(), pieces[1].to_owned()))
  } else {
    Err(format!("invalid ns format: {s}"))
  }
}

pub fn extract_pkg_from_ns(ns: Arc<str>) -> Option<Arc<str>> {
  let p2: Vec<&str> = ns.split('.').collect();
  if !p2.is_empty() { Some(p2[0].into()) } else { None }
}

/// strip first shebang line if detected
pub fn strip_shebang(content: &mut String) {
  if content.starts_with("#!") {
    *content = content.lines().skip(1).collect::<Vec<&str>>().join("\n")
  }
}
