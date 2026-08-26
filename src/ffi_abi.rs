use std::ffi::{CStr, c_char};
use std::{ptr, slice};

use cirru_edn::{Edn, EdnListView};

pub const BUILD_ID_SYMBOL: &[u8] = b"calcit_ffi_build_id";

type FfiBuildId = unsafe extern "C" fn() -> *const c_char;

pub const BUFFER_PROTOCOL_VERSION: u32 = 1;
pub const BUFFER_PROTOCOL_VERSION_SYMBOL: &[u8] = b"calcit_ffi_buffer_version";
pub const BUFFER_FREE_SYMBOL: &[u8] = b"calcit_ffi_buffer_free";
const BUFFER_METHOD_SUFFIX: &str = "_calcit_ffi_v1";
const MAX_BUFFER_BYTES: usize = 256 * 1024 * 1024;

/// Owned bytes allocated by an FFI module. The module must release this value
/// through `calcit_ffi_buffer_free`; the host only copies from it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiBuffer {
  pub ptr: *mut u8,
  pub len: usize,
  pub cap: usize,
}

impl FfiBuffer {
  fn empty() -> Self {
    Self {
      ptr: ptr::null_mut(),
      len: 0,
      cap: 0,
    }
  }
}

type FfiBufferVersion = unsafe extern "C" fn() -> u32;
type FfiBufferFree = unsafe extern "C" fn(FfiBuffer);
type FfiBufferCall = unsafe extern "C" fn(*const u8, usize, *mut FfiBuffer) -> i32;

#[derive(Debug, PartialEq, Eq)]
pub enum FfiBuildCompatibility {
  Exact,
  Legacy,
}

fn buffer_method_symbol(method: &str) -> String {
  format!("{method}{BUFFER_METHOD_SUFFIX}")
}

fn encode_buffer_request(args: Vec<Edn>) -> Result<Vec<u8>, String> {
  cirru_edn::format(&Edn::List(EdnListView(args)), true)
    .map(String::into_bytes)
    .map_err(|error| format!("failed to encode FFI buffer request: {error}"))
}

unsafe fn copy_and_free_buffer(
  buffer: FfiBuffer,
  free: &libloading::Symbol<FfiBufferFree>,
  lib_name: &str,
  symbol: &str,
) -> Result<Vec<u8>, String> {
  if buffer.len > buffer.cap {
    return Err(format!(
      "FFI buffer `{symbol}` in `{lib_name}` returned len {} larger than capacity {}",
      buffer.len, buffer.cap
    ));
  }
  if buffer.len > MAX_BUFFER_BYTES {
    return Err(format!(
      "FFI buffer `{symbol}` in `{lib_name}` returned {} bytes, exceeding the {} byte safety limit",
      buffer.len, MAX_BUFFER_BYTES
    ));
  }
  if buffer.ptr.is_null() && (buffer.len != 0 || buffer.cap != 0) {
    return Err(format!(
      "FFI buffer `{symbol}` in `{lib_name}` returned a null pointer with len {} and capacity {}",
      buffer.len, buffer.cap
    ));
  }

  let copied = if buffer.len == 0 {
    Vec::new()
  } else {
    // SAFETY: the protocol requires `ptr` to reference `len` initialized bytes
    // until the module's matching free function is called below.
    unsafe { slice::from_raw_parts(buffer.ptr.cast_const(), buffer.len) }.to_vec()
  };
  // SAFETY: ownership stays with the module that created the buffer.
  unsafe { free(buffer) };
  Ok(copied)
}

/// Try the C-safe synchronous byte-buffer protocol. `Ok(None)` means the
/// library or this particular method has not migrated and may use the guarded
/// legacy Rust ABI path.
pub fn try_call_buffer(lib: &libloading::Library, lib_name: &str, method: &str, args: Vec<Edn>) -> Result<Option<Edn>, String> {
  let version: libloading::Symbol<FfiBufferVersion> = match unsafe { lib.get(BUFFER_PROTOCOL_VERSION_SYMBOL) } {
    Ok(version) => version,
    Err(_) => return Ok(None),
  };
  let current_version = unsafe { version() };
  if current_version != BUFFER_PROTOCOL_VERSION {
    return Err(format!(
      "FFI buffer protocol mismatch in `{lib_name}`: dylib={current_version}, host={BUFFER_PROTOCOL_VERSION}"
    ));
  }

  let symbol = buffer_method_symbol(method);
  let call: libloading::Symbol<FfiBufferCall> = match unsafe { lib.get(symbol.as_bytes()) } {
    Ok(call) => call,
    Err(_) => return Ok(None),
  };
  let free: libloading::Symbol<FfiBufferFree> = unsafe { lib.get(BUFFER_FREE_SYMBOL) }
    .map_err(|error| format!("FFI buffer method `{symbol}` in `{lib_name}` is missing `calcit_ffi_buffer_free`: {error}"))?;
  let request = encode_buffer_request(args)?;
  let mut output = FfiBuffer::empty();
  let status = unsafe { call(request.as_ptr(), request.len(), &mut output) };
  let output = unsafe { copy_and_free_buffer(output, &free, lib_name, &symbol) }?;

  if status == 0 {
    let source = std::str::from_utf8(&output)
      .map_err(|error| format!("FFI buffer method `{symbol}` in `{lib_name}` returned non-UTF-8 EDN: {error}"))?;
    let value = cirru_edn::parse(source)
      .map_err(|error| format!("FFI buffer method `{symbol}` in `{lib_name}` returned invalid Cirru EDN: {error}"))?;
    Ok(Some(value))
  } else {
    let message = String::from_utf8_lossy(&output);
    Err(format!(
      "FFI buffer method `{symbol}` in `{lib_name}` failed with status {status}: {message}"
    ))
  }
}

pub fn validate_build_id(
  lib_name: &str,
  dylib_build_id: Option<&str>,
  host_build_id: &str,
  require_build_id: bool,
) -> Result<FfiBuildCompatibility, String> {
  match dylib_build_id {
    Some(dylib_build_id) if dylib_build_id == host_build_id => Ok(FfiBuildCompatibility::Exact),
    Some(dylib_build_id) => Err(format!(
      "Refusing Rust-native FFI library `{lib_name}` before invoking a Rust ABI symbol because its build identity differs from the Calcit host. dylib: `{dylib_build_id}`; host: `{host_build_id}`. Rebuild both with the same rustc, target, debug-assertion mode, and panic strategy."
    )),
    None if require_build_id => Err(format!(
      "Refusing legacy Rust-native FFI library `{lib_name}` before invoking a Rust ABI symbol: this debug Calcit host cannot prove that the dylib uses a compatible build. Export the C-safe `calcit_ffi_build_id` symbol described in the FFI guide, or run a release Calcit host built with the same toolchain as the dylib. Expected host identity: `{host_build_id}`."
    )),
    None => Ok(FfiBuildCompatibility::Legacy),
  }
}

/// Read the optional static C build identity without invoking a Rust ABI symbol.
pub fn lookup_build_id(lib: &libloading::Library, lib_name: &str) -> Result<Option<String>, String> {
  let lookup: libloading::Symbol<FfiBuildId> = match unsafe { lib.get(BUILD_ID_SYMBOL) } {
    Ok(lookup) => lookup,
    Err(_) => return Ok(None),
  };
  let ptr = unsafe { lookup() };
  if ptr.is_null() {
    return Err(format!(
      "FFI library `{lib_name}` returned a null pointer from `calcit_ffi_build_id`"
    ));
  }
  let value = unsafe { CStr::from_ptr(ptr) }
    .to_str()
    .map_err(|error| format!("FFI library `{lib_name}` returned invalid UTF-8 from `calcit_ffi_build_id`: {error}"))?;
  Ok(Some(value.to_owned()))
}

#[cfg(test)]
mod tests {
  use super::{BUFFER_PROTOCOL_VERSION, FfiBuildCompatibility, buffer_method_symbol, encode_buffer_request, validate_build_id};
  use cirru_edn::Edn;

  #[test]
  fn buffer_method_names_are_versioned_without_changing_source_calls() {
    assert_eq!(buffer_method_symbol("run_wat"), "run_wat_calcit_ffi_v1");
    assert_eq!(BUFFER_PROTOCOL_VERSION, 1);
  }

  #[test]
  fn buffer_requests_are_canonical_edn_lists() {
    let encoded = encode_buffer_request(vec![Edn::Number(1.0), Edn::str("two")]).expect("encode request");
    let source = std::str::from_utf8(&encoded).expect("UTF-8 request");
    let decoded = cirru_edn::parse(source).expect("parse request");
    assert_eq!(decoded, Edn::List(cirru_edn::EdnListView(vec![Edn::Number(1.0), Edn::str("two")])));
  }

  #[test]
  fn exact_identity_is_accepted() {
    assert_eq!(
      validate_build_id("demo", Some("same-build"), "same-build", true).expect("exact identity should pass"),
      FfiBuildCompatibility::Exact
    );
  }

  #[test]
  fn mismatched_identity_is_rejected_before_rust_abi_calls() {
    let error = validate_build_id("demo", Some("release-build"), "debug-build", false).expect_err("different identities must fail");
    assert!(error.contains("before invoking a Rust ABI symbol"), "error: {error}");
    assert!(error.contains("release-build"), "error: {error}");
    assert!(error.contains("debug-build"), "error: {error}");
  }

  #[test]
  fn debug_hosts_reject_legacy_dylibs_before_rust_abi_calls() {
    let error = validate_build_id("demo", None, "debug-build", true).expect_err("debug host must require build identity");
    assert!(error.contains("Refusing legacy Rust-native FFI library"), "error: {error}");
    assert!(error.contains("calcit_ffi_build_id"), "error: {error}");
  }

  #[test]
  fn release_hosts_keep_a_temporary_legacy_path() {
    assert_eq!(
      validate_build_id("demo", None, "release-build", false).expect("release compatibility path should remain"),
      FfiBuildCompatibility::Legacy
    );
  }
}
