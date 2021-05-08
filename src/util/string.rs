pub fn is_digit(c: char) -> bool {
  let n = c as u32;
  // ascii table https://tool.oschina.net/commons?type=4
  (48..=57).contains(&n)
}

pub fn is_letter(c: char) -> bool {
  let n = c as u32;
  (65..=90).contains(&n) || (97..=122).contains(&n)
}

// TODO, not ready to use
#[allow(dead_code)]
pub fn matches_float(xs: &str) -> bool {
  if xs.is_empty() {
    return false;
  }
  let mut buffer = xs.to_string();
  if xs.starts_with('-') {
    buffer = xs.strip_prefix('-').unwrap().to_string();
  }

  if buffer.is_empty() {
    return false;
  }

  let mut count_digits = 0;
  let mut count_dot = 0;
  for x in buffer.chars() {
    if is_digit(x) {
      count_digits += 1
    } else if x == '.' {
      count_dot += 1
    } else {
      return false;
    }
  }

  if count_digits < 1 {
    return false;
  }
  if count_dot > 1 {
    return false;
  }

  true
}

#[allow(dead_code)]
pub fn matches_simple_var(xs: &str) -> bool {
  if xs.is_empty() {
    return false;
  }
  for x in xs.chars() {
    if is_letter(x) || is_digit(x) || x == '-' || x == '!' || x == '*' || x == '?' {
      // ok
    } else {
      return false;
    }
  }
  true
}

pub fn matches_js_var(xs: &str) -> bool {
  if xs.is_empty() {
    false
  } else {
    for (idx, x) in xs.chars().enumerate() {
      if is_letter(x) || x == '_' || x == '$' || (idx > 0 && is_digit(x)) {
        // ok
      } else {
        return false;
      }
    }
    true
  }
}

pub fn has_ns_part(x: &str) -> bool {
  match x.find('/') {
    Some(try_slash_pos) => try_slash_pos >= 1 && try_slash_pos < x.len() - 1,
    None => false,
  }
}

/// js/JSON.stringify -like API
pub fn wrap_js_str(s: &str) -> String {
  let mut c: String = String::from("");
  c.push('"');
  for i in s.escape_default() {
    c.push(i);
  }
  c.push('"');
  c
}

pub fn extract_ns_def(s: &str) -> Result<(String, String), String> {
  let pieces: Vec<&str> = (&s).split('/').collect();
  if pieces.len() == 2 {
    Ok((pieces[0].to_owned(), pieces[1].to_owned()))
  } else {
    Err(format!("invalid ns format: {}", s))
  }
}

pub fn extract_pkg_from_def(s: &str) -> Option<String> {
  let pieces: Vec<&str> = (&s).split('/').collect();
  if pieces.len() >= 1 {
    let p2: Vec<&str> = (pieces[0]).split('.').collect();
    if p2.len() >= 1 {
      Some(p2[0].to_string())
    } else {
      None
    }
  } else {
    None
  }
}
