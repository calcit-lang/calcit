//! Entry surface of the WASM backend and the FFI ABI modules that live in `src/wasm/`.
//!
//! The implementation files keep their historical public paths so existing consumers do not
//! break: `calcit::ffi_abi`, `calcit::ffi_async`, `calcit::ffi_interface_ir`, `calcit::wasm_cli`
//! and `calcit::codegen::emit_wasm`. Those modules are declared with `#[path]` in `src/lib.rs`
//! and `src/codegen.rs` because the FFI modules compile without the `wasm` feature while the
//! code generator and CLI driver need it.
//!
//! This module is the documented facade: it re-exports the supported surface under one name,
//! `calcit::wasm`. Code outside `src/wasm/` must reach WASM internals only through this facade
//! or through the public paths listed above; `boundary_tests` enforces that.

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

  /// Items of `emit_wasm` that code outside `src/wasm/` may name.
  const EMIT_WASM_PUBLIC: &[&str] = &[
    "WasmBoundary",
    "WasmTarget",
    "check_wasm_command_component",
    "emit_wasm",
    "validate_wasm_boundary",
    "validate_wasm_target",
  ];
  /// Items of `wasm_cli` that code outside `src/wasm/` may name.
  const WASM_CLI_PUBLIC: &[&str] = &["WasmCliOptions", "run"];

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

  /// Names imported through `module::name` or `module::{a, b}` after every `module::` occurrence.
  fn imported_names<'a>(line: &'a str, module: &str) -> Vec<&'a str> {
    let marker = format!("{module}::");
    let mut names = vec![];
    let mut rest = line;
    while let Some(index) = rest.find(&marker) {
      rest = &rest[index + marker.len()..];
      if let Some(group) = rest.strip_prefix('{') {
        let group = group.split('}').next().unwrap_or(group);
        names.extend(
          group
            .split(',')
            .map(|item| item.trim().split("::").next().unwrap_or("").trim())
            .filter(|n| !n.is_empty() && *n != "self"),
        );
      } else {
        let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
        names.push(&rest[..end]);
      }
    }
    names
  }

  #[test]
  fn code_outside_wasm_dir_uses_only_facade_or_public_paths() {
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
      let source = fs::read_to_string(file).expect("read source");
      // Drop comment lines, then scan the whole text so rustfmt-wrapped multi-line `use` groups are seen.
      let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
      // Implementation files may only be mounted by `#[path]` declarations in the two declaration sites.
      if !matches!(rel.as_str(), "src/lib.rs" | "src/codegen.rs") {
        for line in code.lines().filter(|line| line.trim_start().starts_with("#[path")) {
          if line.contains("wasm/") {
            violations.push(format!("{rel}: mounts a src/wasm implementation file directly: {}", line.trim()));
          }
        }
      }
      for name in imported_names(&code, "emit_wasm") {
        if !EMIT_WASM_PUBLIC.contains(&name) {
          violations.push(format!("{rel}: `emit_wasm::{name}` is not part of the WASM facade"));
        }
      }
      for name in imported_names(&code, "wasm_cli") {
        if !WASM_CLI_PUBLIC.contains(&name) {
          violations.push(format!("{rel}: `wasm_cli::{name}` is not part of the WASM facade"));
        }
      }
    }
    assert!(violations.is_empty(), "WASM boundary violations:\n{}", violations.join("\n"));
  }
}
