//! Entry surface of the WASM backend and the FFI ABI modules that live in `src/wasm/`.
//!
//! The implementation files keep their historical public paths so existing consumers do not
//! break: `calcit::ffi_abi`, `calcit::ffi_async`, `calcit::ffi_interface_ir`, `calcit::wasm_cli`
//! and `calcit::codegen::emit_wasm`. Those modules are declared with `#[path]` in `src/lib.rs`
//! and `src/codegen.rs` because the FFI modules compile without the `wasm` feature while the
//! code generator and CLI driver need it.
//!
//! This module is the documented facade: it re-exports the supported surface under one name,
//! `calcit::wasm`. Code in this repository outside `src/wasm/` must reach WASM internals only
//! through this facade; `boundary_tests` enforces that. The historical paths above remain public
//! as compatibility aliases for external consumers and are not removed in patch releases.

// FFI ABI modules: available without the `wasm` feature.
#[cfg(not(target_arch = "wasm32"))]
pub use crate::ffi_abi;
#[cfg(not(target_arch = "wasm32"))]
pub use crate::ffi_async;
pub use crate::ffi_interface_ir;

// Code generator and CLI driver: require the `wasm` feature.
#[cfg(feature = "wasm")]
pub use crate::codegen::emit_wasm::{
  WasmBoundary, WasmTarget, check_wasm_command_component, emit_wasm, validate_wasm_boundary, validate_wasm_target,
};
#[cfg(feature = "wasm")]
pub use crate::wasm_cli::{WasmCliOptions, run};

#[cfg(test)]
mod boundary_tests {
  use std::fs;
  use std::path::{Path, PathBuf};

  /// Module names that are only reachable through `calcit::wasm` for in-repo callers.
  const FACADE_MODULES: &[&str] = &["emit_wasm", "wasm_cli", "ffi_abi", "ffi_async", "ffi_interface_ir"];
  /// Files allowed to name the historical paths: the two `#[path]` mount sites.
  const MOUNT_ALLOWLIST: &[&str] = &["src/lib.rs", "src/codegen.rs"];

  fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read dir") {
      let path = entry.expect("dir entry").path();
      if path.is_dir() {
        rust_files(&path, out);
      } else if path.extension().is_some_and(|ext| ext == "rs") {
        out.push(path);
      }
    }
  }

  /// Byte offsets where `name` occurs as a whole path segment (not part of a longer identifier).
  fn segment_hits(code: &str, name: &str) -> Vec<usize> {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    code
      .match_indices(name)
      .filter(|(at, _)| {
        let before = code[..*at].chars().next_back();
        let after = code[*at + name.len()..].chars().next();
        !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
      })
      .map(|(at, _)| at)
      .collect()
  }

  #[test]
  fn code_outside_wasm_dir_uses_only_the_facade() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![];
    for dir in ["src", "tests", "benches"] {
      let dir = root.join(dir);
      if dir.is_dir() {
        rust_files(&dir, &mut files);
      }
    }
    let wasm_dir = root.join("src/wasm");
    let mut violations = vec![];
    for file in files.iter().filter(|f| !f.starts_with(&wasm_dir)) {
      let rel = file.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
      if MOUNT_ALLOWLIST.contains(&rel.as_str()) {
        continue;
      }
      let source = fs::read_to_string(file).expect("read source");
      // Drop comment lines, then scan the whole text so rustfmt-wrapped multi-line `use` groups are seen.
      let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
      for line in code.lines().filter(|line| line.trim_start().starts_with("#[path")) {
        if line.contains("wasm/") {
          violations.push(format!("{rel}: mounts a src/wasm implementation file directly: {}", line.trim()));
        }
      }
      for name in FACADE_MODULES {
        for at in segment_hits(&code, name) {
          if !code[..at].ends_with("wasm::") {
            violations.push(format!(
              "{rel}: `{name}` must be reached as `calcit::wasm::{name}` (or a facade re-export)"
            ));
          }
        }
      }
    }
    assert!(violations.is_empty(), "WASM boundary violations:\n{}", violations.join("\n"));
  }
}
