//! Every `E_WASM_*` diagnostic emitted by the compiler is listed in the WASM support
//! matrix (docs/installation/wasm-support.md), so a new code cannot ship undocumented.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn collect_codes(dir: &Path, codes: &mut BTreeSet<String>) {
  for entry in fs::read_dir(dir).expect("read source dir") {
    let path = entry.expect("dir entry").path();
    if path.is_dir() {
      collect_codes(&path, codes);
    } else if path.extension().is_some_and(|ext| ext == "rs") {
      let text = fs::read_to_string(&path).expect("read source file");
      let mut rest = text.as_str();
      while let Some(start) = rest.find("E_WASM_") {
        let tail = &rest[start..];
        let end = tail.find(|c: char| !(c.is_ascii_uppercase() || c == '_')).unwrap_or(tail.len());
        let code = tail[..end].trim_end_matches('_');
        if code.len() > "E_WASM_".len() {
          codes.insert(code.to_owned());
        }
        rest = &tail[end..];
      }
    }
  }
}

#[test]
fn every_wasm_diagnostic_code_is_documented() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR"));
  let mut codes = BTreeSet::new();
  collect_codes(&root.join("src"), &mut codes);
  assert!(!codes.is_empty(), "no E_WASM_* codes found in src");
  let matrix = fs::read_to_string(root.join("docs/installation/wasm-support.md")).expect("read WASM support matrix");
  let missing: Vec<_> = codes.iter().filter(|code| !matrix.contains(&format!("`{code}`"))).collect();
  assert!(
    missing.is_empty(),
    "document these codes in docs/installation/wasm-support.md: {missing:?}"
  );
}
