#![no_std]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
  loop {}
}

/// Writes Calcit's canonical decimal text to caller-owned memory.
/// The caller reserves 400 bytes, enough for every finite f64.
#[no_mangle]
pub unsafe extern "C" fn format_f64(value: f64, output: *mut u8) -> usize {
  let mut raw = [0u8; 24];
  let len = unsafe { ryu::raw::format64(value, raw.as_mut_ptr()) };
  let mut exponent_at = len;
  let mut i = 0;
  while i < len {
    if raw[i] == b'e' {
      exponent_at = i;
      break;
    }
    i += 1;
  }
  if exponent_at == len {
    let size = if len >= 2 && raw[len - 2] == b'.' && raw[len - 1] == b'0' {
      len - 2
    } else {
      len
    };
    unsafe { core::ptr::copy_nonoverlapping(raw.as_ptr(), output, size) };
    return size;
  }

  let mut exponent = 0i32;
  let mut cursor = exponent_at + 1;
  let negative_exponent = raw[cursor] == b'-';
  if raw[cursor] == b'-' || raw[cursor] == b'+' {
    cursor += 1;
  }
  while cursor < len {
    exponent = exponent * 10 + (raw[cursor] - b'0') as i32;
    cursor += 1;
  }
  if negative_exponent {
    exponent = -exponent;
  }

  let negative = raw[0] == b'-';
  let begin = usize::from(negative);
  let mut digits_before_dot = 0i32;
  let mut digit_count = 0usize;
  i = begin;
  while i < exponent_at {
    if raw[i] == b'.' {
      digits_before_dot = digit_count as i32;
    } else {
      digit_count += 1;
    }
    i += 1;
  }
  if digits_before_dot == 0 {
    digits_before_dot = digit_count as i32;
  }
  let decimal_index = digits_before_dot + exponent;
  let mut written = 0usize;
  if negative {
    unsafe { output.add(written).write(b'-') };
    written += 1;
  }
  if decimal_index <= 0 {
    unsafe { output.add(written).write(b'0') };
    written += 1;
    unsafe { output.add(written).write(b'.') };
    written += 1;
    let mut zeros = -decimal_index;
    while zeros > 0 {
      unsafe { output.add(written).write(b'0') };
      written += 1;
      zeros -= 1;
    }
  }
  let mut emitted_digits = 0usize;
  i = begin;
  while i < exponent_at {
    if raw[i] != b'.' {
      if decimal_index > 0 && (decimal_index as usize) < digit_count && emitted_digits == decimal_index as usize {
        unsafe { output.add(written).write(b'.') };
        written += 1;
      }
      unsafe { output.add(written).write(raw[i]) };
      written += 1;
      emitted_digits += 1;
    }
    i += 1;
  }
  if decimal_index > digit_count as i32 {
    let mut zeros = decimal_index - digit_count as i32;
    while zeros > 0 {
      unsafe { output.add(written).write(b'0') };
      written += 1;
      zeros -= 1;
    }
  }
  written
}
