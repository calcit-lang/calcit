#[allow(dead_code)]
pub fn is_odd(x: usize) -> bool {
  x & 1 == 1
}
pub fn is_even(x: usize) -> bool {
  x & 1 == 0
}

pub fn is_integer(x: f64) -> bool {
  x.fract().abs() <= f64::EPSILON
}

/// Return the canonical decimal text shared with JS and the WASM formatter.
pub fn format_calcit_number(value: f64) -> String {
  if value == 0.0 && value.is_sign_negative() {
    return "-0".into();
  }
  let mut buffer = ryu::Buffer::new();
  let text = buffer.format(value);
  let Some((mantissa, exponent)) = text.split_once('e') else {
    return text.strip_suffix(".0").unwrap_or(text).to_owned();
  };
  let exponent: i32 = exponent.parse().expect("Ryū must emit a valid decimal exponent");
  let (sign, unsigned) = match mantissa.strip_prefix('-') {
    Some(unsigned) => ("-", unsigned),
    None => ("", mantissa),
  };
  let decimal_index = unsigned.find('.').unwrap_or(unsigned.len()) as i32 + exponent;
  let digits = unsigned.replace('.', "");
  if decimal_index <= 0 {
    format!("{sign}0.{}{}", "0".repeat(-decimal_index as usize), digits)
  } else if decimal_index as usize >= digits.len() {
    format!("{sign}{}{}", digits, "0".repeat(decimal_index as usize - digits.len()))
  } else {
    let index = decimal_index as usize;
    format!("{sign}{}.{}", &digits[..index], &digits[index..])
  }
}

fn is_float_integer(f: f64) -> bool {
  f.fract().abs() <= f64::EPSILON
}

pub fn f64_to_usize(f: f64) -> Result<usize, String> {
  if is_float_integer(f) {
    if f >= 0.0 {
      Ok(f as usize)
    } else {
      Err(format!("usize expected a positive number, but got: {f}"))
    }
  } else {
    Err(format!("cannot extract usize from float: {f}"))
  }
}

pub fn f64_to_i32(f: f64) -> Result<i32, String> {
  if is_float_integer(f) {
    Ok(f as i32)
  } else {
    Err(format!("cannot extract int from float: {f}"))
  }
}

/// Bitwise operand domain: an integer within i32, so no value is saturated or wrapped.
pub fn f64_to_bit_operand(f: f64) -> Result<i32, String> {
  if f.trunc() == f && f >= i32::MIN as f64 && f <= i32::MAX as f64 {
    Ok(f as i32)
  } else {
    Err(format!("expected an integer within i32, but got: {}", format_calcit_number(f)))
  }
}

#[cfg(test)]
mod number_text_tests {
  use super::format_calcit_number;

  #[test]
  fn canonical_number_text_uses_one_shortest_decimal_tie_break() {
    assert_eq!(format_calcit_number(f64::from_bits(0x4308_90af_8f4a_2b7a)), "864310392341871.2");
    assert_eq!(format_calcit_number(f64::from_bits(0xc30c_3fea_8669_5ce2)), "-993946982230940.2");
    assert_eq!(format_calcit_number(-0.0), "-0");
    assert_eq!(format_calcit_number(f64::NAN), "NaN");
    assert_eq!(format_calcit_number(f64::INFINITY), "inf");
    assert_eq!(format_calcit_number(f64::NEG_INFINITY), "-inf");
    assert_eq!(format_calcit_number(f64::from_bits(1)), format!("0.{}5", "0".repeat(323)));
  }
}
