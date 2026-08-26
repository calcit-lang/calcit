use std::ffi::{CStr, c_char};

pub const BUILD_ID_SYMBOL: &[u8] = b"calcit_ffi_build_id";

type FfiBuildId = unsafe extern "C" fn() -> *const c_char;

#[derive(Debug, PartialEq, Eq)]
pub enum FfiBuildCompatibility {
  Exact,
  Legacy,
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
  use super::{FfiBuildCompatibility, validate_build_id};

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
