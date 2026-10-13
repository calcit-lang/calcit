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

// Host facade: the single place in this directory that names main-crate internals.
pub(crate) mod host;

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
  use std::collections::HashSet;
  use std::fs;
  use std::path::{Path, PathBuf};

  /// Module names that are only reachable through `calcit::wasm` for in-repo callers.
  const FACADE_MODULES: &[&str] = &["emit_wasm", "wasm_cli", "ffi_abi", "ffi_async", "ffi_interface_ir"];
  /// Crate-root modules whose source lives in `src/wasm/`; files there may name them directly.
  const WASM_OWN_MODULES: &[&str] = &["wasm", "wasm_cli", "ffi_abi", "ffi_async", "ffi_interface_ir"];
  /// The only file under `src/wasm/` allowed to name main-crate internals.
  const HOST_ALLOWLIST: &[&str] = &["src/wasm/host.rs"];
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

  /// Replaces comments (and string/char literals when `blank_strings`) with spaces, keeping newlines.
  fn blank(source: &str, blank_strings: bool) -> String {
    let c: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    let fill = |out: &mut String, from: usize, to: usize| {
      for &ch in &c[from..to.min(c.len())] {
        out.push(if ch == '\n' { '\n' } else { ' ' });
      }
    };
    let starts = |s: &str, at: usize| s.chars().enumerate().all(|(k, ch)| c.get(at + k) == Some(&ch));
    let is_ident = |ch: char| ch.is_alphanumeric() || ch == '_';
    while i < c.len() {
      let raw_hashes = if blank_strings && c[i] == 'r' && (i == 0 || !is_ident(c[i - 1])) {
        let hashes = c[i + 1..].iter().take_while(|&&ch| ch == '#').count();
        (c.get(i + 1 + hashes) == Some(&'"')).then_some(hashes)
      } else {
        None
      };
      if starts("//", i) {
        let end = c[i..].iter().position(|&ch| ch == '\n').map_or(c.len(), |p| i + p);
        fill(&mut out, i, end);
        i = end;
      } else if starts("/*", i) {
        let (mut depth, mut j) = (1, i + 2);
        while j < c.len() && depth > 0 {
          if starts("/*", j) {
            depth += 1;
            j += 2;
          } else if starts("*/", j) {
            depth -= 1;
            j += 2;
          } else {
            j += 1;
          }
        }
        fill(&mut out, i, j);
        i = j;
      } else if let Some(hashes) = raw_hashes {
        let close: String = std::iter::once('"').chain(std::iter::repeat_n('#', hashes)).collect();
        let end = (i + 2 + hashes..c.len())
          .find(|&j| starts(&close, j))
          .map_or(c.len(), |j| j + close.len());
        fill(&mut out, i, end);
        i = end;
      } else if blank_strings && c[i] == '"' {
        let mut j = i + 1;
        while j < c.len() && c[j] != '"' {
          j += if c[j] == '\\' { 2 } else { 1 };
        }
        fill(&mut out, i, j + 1);
        i = j + 1;
      } else if blank_strings && c[i] == '\'' && (c.get(i + 1) == Some(&'\\') || c.get(i + 2) == Some(&'\'')) {
        // Char literal; a lifetime such as `'a` has no closing quote right after one char.
        let end = if c[i + 1] == '\\' {
          (i + 3..c.len()).find(|&j| c[j] == '\'').map_or(c.len(), |j| j + 1)
        } else {
          i + 3
        };
        fill(&mut out, i, end);
        i = end;
      } else {
        out.push(c[i]);
        i += 1;
      }
    }
    out
  }

  /// Walks `::`-joined paths with nested `{}` groups (also across lines) and calls `visit` with
  /// the segments before each identifier and the identifier itself.
  fn walk_paths(code: &str, visit: &mut dyn FnMut(&[String], &str)) {
    let chars: Vec<char> = code.chars().collect();
    let mut groups: Vec<Vec<String>> = vec![];
    let mut current: Vec<String> = vec![];
    let (mut cont, mut at_start) = (false, false);
    let mut i = 0;
    while i < chars.len() {
      let ch = chars[i];
      if ch.is_alphanumeric() || ch == '_' {
        let end = (i..chars.len())
          .find(|&j| !(chars[j].is_alphanumeric() || chars[j] == '_'))
          .unwrap_or(chars.len());
        let ident: String = chars[i..end].iter().collect();
        if !cont {
          current = if at_start {
            groups.last().cloned().unwrap_or_default()
          } else {
            vec![]
          };
        }
        let parent = current.clone();
        current.push(ident.clone());
        cont = false;
        at_start = false;
        visit(&parent, &ident);
        i = end;
        continue;
      }
      match ch {
        ':' if chars.get(i + 1) == Some(&':') => {
          cont = true;
          i += 2;
          continue;
        }
        '{' => {
          groups.push(if cont { current.clone() } else { vec![] });
          cont = false;
          at_start = true;
        }
        '}' => {
          groups.pop();
          current = groups.last().cloned().unwrap_or_default();
          cont = false;
          at_start = false;
        }
        ',' => {
          current = groups.last().cloned().unwrap_or_default();
          cont = false;
          at_start = true;
        }
        c if c.is_whitespace() => {}
        _ => {
          cont = false;
          at_start = false;
          current.clear();
        }
      }
      i += 1;
    }
  }

  /// Reports facade modules that are not reached through `calcit::wasm::`, `crate::wasm::` or an
  /// imported `wasm` alias.
  fn scan_paths(code: &str) -> Vec<String> {
    let compact: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let alias_imported = compact.contains("usecalcit::wasm;")
      || compact.contains("usecrate::wasm;")
      || compact.contains("calcit::wasm::{self")
      || compact.contains("crate::wasm::{self");
    let mut found = vec![];
    let mut imported: HashSet<String> = HashSet::new();
    walk_paths(code, &mut |parent, ident| {
      if FACADE_MODULES.contains(&ident) {
        let facade = matches!(parent, [a, b] if (a == "calcit" || a == "crate") && b == "wasm");
        let aliased = alias_imported && parent == ["wasm"];
        let bare = parent.is_empty() && imported.contains(ident);
        if facade {
          imported.insert(ident.to_owned());
        }
        if !(facade || aliased || bare) {
          let full: Vec<String> = parent.iter().cloned().chain([ident.to_owned()]).collect();
          found.push(format!("`{}`", full.join("::")));
        }
      }
    });
    found
  }

  /// Names reached as `crate::<name>` (also inside `use crate::{…}` groups) other than the WASM
  /// modules that live in this directory.
  fn scan_host_paths(code: &str) -> Vec<String> {
    let mut found = vec![];
    // `crate::codegen` is only fine as the prefix of `crate::codegen::emit_wasm`.
    let mut bare_codegen: i32 = 0;
    walk_paths(code, &mut |parent, ident| {
      if parent == ["crate"] && ident == "codegen" {
        bare_codegen += 1;
      } else if parent == ["crate"] && !WASM_OWN_MODULES.contains(&ident) {
        found.push(format!("`crate::{ident}`"));
      } else if parent == ["crate", "codegen"] {
        if ident == "emit_wasm" {
          bare_codegen -= 1;
        } else {
          found.push(format!("`crate::codegen::{ident}`"));
        }
      }
    });
    if bare_codegen > 0 {
      found.push("`crate::codegen` without `emit_wasm`".to_owned());
    }
    found
  }

  /// Boundary violations of one file's source; `rel` is its repository-relative path.
  fn scan_source(rel: &str, source: &str) -> Vec<String> {
    if MOUNT_ALLOWLIST.contains(&rel) {
      return vec![];
    }
    let mut violations = vec![];
    for line in blank(source, false).lines().filter(|line| line.trim_start().starts_with("#[path")) {
      if line.contains("wasm/") {
        violations.push(format!("{rel}: mounts a src/wasm implementation file directly: {}", line.trim()));
      }
    }
    for path in scan_paths(&blank(source, true)) {
      violations.push(format!("{rel}: {path} must be reached through the `calcit::wasm` facade"));
    }
    violations
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
      violations.extend(scan_source(&rel, &fs::read_to_string(file).expect("read source")));
    }
    assert!(violations.is_empty(), "WASM boundary violations:\n{}", violations.join("\n"));
  }

  /// Violations of one `src/wasm/` file: main-crate internals must come through `wasm::host`.
  fn scan_host_source(rel: &str, source: &str) -> Vec<String> {
    if HOST_ALLOWLIST.contains(&rel) {
      return vec![];
    }
    scan_host_paths(&blank(source, true))
      .into_iter()
      .map(|path| format!("{rel}: {path} must be reached through `crate::wasm::host`"))
      .collect()
  }

  #[test]
  fn wasm_dir_reaches_main_crate_only_through_host() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![];
    rust_files(&root.join("src/wasm"), &mut files);
    let mut violations = vec![];
    for file in &files {
      let rel = file.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
      violations.extend(scan_host_source(&rel, &fs::read_to_string(file).expect("read source")));
    }
    assert!(violations.is_empty(), "WASM host facade violations:\n{}", violations.join("\n"));
  }

  fn host_ok(source: &str) {
    let v = scan_host_source("src/wasm/x.rs", source);
    assert!(v.is_empty(), "expected no violations for {source:?}, got {v:?}");
  }

  fn host_bad(source: &str) {
    assert!(
      !scan_host_source("src/wasm/x.rs", source).is_empty(),
      "expected a violation for {source:?}"
    );
  }

  #[test]
  fn host_scanner_accepts_facade_and_own_modules() {
    host_ok("use crate::wasm::host::calcit::Calcit;\nuse crate::wasm::{WasmTarget, host};");
    host_ok("use crate::ffi_abi::{\n  A,\n  B,\n};\nfn f() { crate::ffi_async::go(); }");
    host_ok("use crate::codegen::emit_wasm::X;");
    host_ok("// use crate::calcit::X;\nlet s = \"crate::util::y\"; /* crate::program */");
    host_ok("use crate::wasm::host::{self, calcit};\nfn f() { host::util::go(); }");
    host_ok("fn f() { let _ = my_crate::calcit::X; }");
  }

  #[test]
  fn host_scanner_rejects_direct_internals() {
    host_bad("use crate::calcit::Calcit;");
    host_bad("fn f() { crate::util::number::format(1); }");
    host_bad("use crate::{\n  wasm::host,\n  program,\n};");
    host_bad("use crate::{ProgramEntries, call_stack};");
    host_bad("use crate::codegen::destruct_list_match_shape;");
    host_bad("fn f() { let x = crate::load_core_snapshot(); }");
    host_bad("use crate::wasm::{host};\nuse crate::runner::preprocess::X;");
    host_bad("use crate::codegen;\nfn f() { codegen::go(); }");
    host_bad("use crate::{codegen};");
    host_bad("use crate::codegen as cg;");
    host_bad("use crate::codegen::{self};");
    host_bad("use crate::{codegen::emit_wasm::X, codegen};");
  }

  #[test]
  fn host_allowlist_covers_only_host_rs() {
    assert!(!scan_host_source("src/wasm/mod.rs", "use crate::calcit::X;").is_empty());
    assert!(scan_host_source("src/wasm/host.rs", "pub(crate) use crate::{calcit, program};").is_empty());
  }

  fn ok(source: &str) {
    let v = scan_source("src/x.rs", source);
    assert!(v.is_empty(), "expected no violations for {source:?}, got {v:?}");
  }

  fn bad(source: &str) {
    assert!(!scan_source("src/x.rs", source).is_empty(), "expected a violation for {source:?}");
  }

  #[test]
  fn scanner_accepts_facade_qualified_uses() {
    ok("use calcit::wasm::ffi_abi::FfiBuffer;");
    ok("use calcit::wasm::{ffi_abi::X, emit_wasm, ffi_async::{Y, Z}};");
    ok("use calcit::{\n  builtins,\n  wasm::{\n    ffi_abi,\n    ffi_async::Q,\n  },\n};");
    ok("use crate::wasm::ffi_interface_ir::A;\nfn f() { crate::wasm::ffi_abi::go(); }");
    ok("fn f() { calcit::wasm::ffi_abi::go(); }");
    ok("use calcit::wasm;\nfn f() { wasm::ffi_abi::go(); }");
    ok("use calcit::wasm::{self, WasmTarget};\nfn f() { wasm::ffi_async::go(); }");
    ok("use calcit::wasm::ffi_abi;\nfn f() { ffi_abi::go(); }");
  }

  #[test]
  fn scanner_ignores_comments_and_literals() {
    ok("let x = 1; // calcit::codegen::emit_wasm::run\n");
    ok("/* use calcit::ffi_abi::X; */ fn f() {}");
    ok("/* outer /* nested ffi_abi::X */ still ffi_async::Y */ fn f() {}");
    ok("let s = \"use calcit::ffi_abi::X; \\\" ffi_async::Y\";");
    ok("let s = r#\"wasm_cli::run\"#; let c = '\"'; let d = '\\'';");
    ok("fn f<'a>(x: &'a str) { let _ = \"ffi_abi::X\"; }");
    ok("let name = \"ffi_abi\"; let calcit_ffi_async_version = 1;");
  }

  #[test]
  fn scanner_rejects_old_and_lookalike_paths() {
    bad("use calcit::ffi_abi::X;");
    bad("use calcit::codegen::emit_wasm::{WasmBoundary, WasmTarget};");
    bad("use calcit::wasm_cli::run;");
    bad("use calcit::{ffi_abi::X, wasm::Y};");
    bad("use calcit::wasm::{Y, ffi_abi::X};\nuse calcit::ffi_async::Z;");
    bad("use crate::ffi_interface_ir::A;");
    bad("fn f() { foo_wasm::ffi_abi::go(); }");
    bad("fn f() { codegen::wasm::ffi_abi::go(); }");
    bad("use calcit::codegen::wasm::ffi_abi::X;");
    // `wasm::` alone is not enough without importing the facade as `wasm`.
    bad("fn f() { wasm::ffi_abi::go(); }");
    // A bare name is only fine after a facade import.
    bad("fn f() { ffi_abi::go(); }");
    bad("use calcit::{\n  ffi_abi::{\n    A,\n  },\n};");
    bad("#[path = \"wasm/ffi_abi.rs\"]\nmod ffi_abi;");
  }

  #[test]
  fn scanner_handles_plain_blocks_and_fields() {
    ok("impl A {\n fn add_task(&mut self, life: calcit::wasm::ffi_abi::X, m: Y) {}\n}");
    ok("fn f() { match life {\n calcit::wasm::ffi_abi::X => {} } if s.life != calcit::wasm::ffi_abi::X {} }");
    bad("fn f() { match life {\n calcit::ffi_abi::X => {} } }");
  }

  #[test]
  fn scanner_allows_mount_sites() {
    assert!(scan_source("src/lib.rs", "#[path = \"wasm/ffi_abi.rs\"]\npub mod ffi_abi;").is_empty());
  }
}
