//! Every `E_WASM_*` diagnostic emitted by the compiler is listed in the WASM support
//! matrix (docs/installation/wasm-support.md), so a new code cannot ship undocumented.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Collect compiler diagnostic tokens without counting the wildcard prefix.
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

/// Match the runner's diagnostic-table parser, including multi-code cells.
fn diagnostic_table_codes(matrix: &str) -> BTreeSet<String> {
  let mut codes = BTreeSet::new();
  let mut in_section = false;
  let mut in_table = false;
  for line in matrix.lines().map(str::trim) {
    if line.starts_with("## ") {
      in_section = line == "## 诊断";
      in_table = false;
      continue;
    }
    if !in_section {
      continue;
    }
    let cells: Vec<_> = line.split('|').map(str::trim).collect();
    if !in_table {
      in_table = cells == ["", "编号", "含义", "处理方式", ""];
      continue;
    }
    if cells.len() < 5 || !cells[0].is_empty() || cells.last() != Some(&"") {
      in_table = false;
      continue;
    }
    for token in cells[1].split('`').skip(1).step_by(2) {
      if let Some(suffix) = token.strip_prefix("E_WASM_")
        && !suffix.is_empty()
        && suffix.chars().all(|c| c.is_ascii_uppercase() || c == '_')
      {
        codes.insert(token.to_owned());
      }
    }
  }
  codes
}

#[test]
fn only_diagnostic_table_code_cells_count_as_coverage() {
  let fixture = include_str!("fixtures/wasm-support-matrix-parser.md");
  let expected = BTreeSet::from(["E_WASM_FIRST".to_owned(), "E_WASM_SECOND".to_owned(), "E_WASM_THIRD".to_owned()]);
  assert_eq!(diagnostic_table_codes(fixture), expected);
  assert_eq!(diagnostic_table_codes(&fixture.replace('\n', "\r\n")), expected);
  let missing_row = fixture
    .lines()
    .filter(|line| !line.starts_with("| `E_WASM_FIRST`"))
    .collect::<Vec<_>>()
    .join("\n");
  assert!(missing_row.contains("`E_WASM_FIRST`"));
  assert!(!diagnostic_table_codes(&missing_row).contains("E_WASM_FIRST"));
}

#[test]
fn every_wasm_diagnostic_code_is_documented() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR"));
  let mut codes = BTreeSet::new();
  collect_codes(&root.join("src"), &mut codes);
  assert!(!codes.is_empty(), "no E_WASM_* codes found in src");
  let matrix = fs::read_to_string(root.join("docs/installation/wasm-support.md")).expect("read WASM support matrix");
  let documented = diagnostic_table_codes(&matrix);
  let missing: Vec<_> = codes.iter().filter(|code| !documented.contains(*code)).collect();
  assert!(
    missing.is_empty(),
    "document these codes in the diagnostic table of docs/installation/wasm-support.md: {missing:?}"
  );
}
