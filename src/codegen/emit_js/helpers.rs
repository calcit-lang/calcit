use std::fs;
use std::path::Path;

use cirru_parser::Cirru;

pub(super) fn write_file_if_changed(filename: &Path, content: &str) -> Result<bool, String> {
  if filename.exists()
    && fs::read_to_string(filename).map_err(|e| format!("failed to read JavaScript artifact {}: {e}", filename.display()))? == content
  {
    return Ok(false);
  }
  fs::write(filename, content).map_err(|e| format!("failed to write JavaScript artifact {}: {e}", filename.display()))?;
  Ok(true)
}

pub(super) fn is_js_unavailable_procs(name: &str) -> bool {
  matches!(
    name,
    "&reset-gensym-index!"
      | "&get-def-doc"
      | "&get-def-schema"
      | "gensym"
      | "macroexpand"
      | "macroexpand-all"
      | "to-cirru-edn"
      | "extract-cirru-edn"
  )
}

pub(super) fn cirru_to_js(code: &Cirru) -> Result<String, String> {
  match code {
    Cirru::List(xs) => {
      let mut chunk = "[".to_owned();
      for x in xs {
        chunk.push_str(&cirru_to_js(x)?);
        chunk.push(',');
      }
      if chunk.ends_with(',') {
        chunk.pop();
      }
      chunk.push(']');
      Ok(chunk)
    }
    Cirru::Leaf(s) => Ok(format!("\"{}\"", s.escape_default())),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn unavailable_proc_list_contains_known_entries() {
    assert!(is_js_unavailable_procs("macroexpand"));
    assert!(!is_js_unavailable_procs("println"));
  }

  #[test]
  fn cirru_list_to_json_like_array() {
    let code = Cirru::List(vec![Cirru::Leaf("a".into()), Cirru::Leaf("b".into())]);
    assert_eq!(cirru_to_js(&code).expect("cirru js"), "[\"a\",\"b\"]");
  }

  #[test]
  fn failed_artifact_write_is_not_reported_as_written() {
    let root = tempfile::tempdir().expect("temporary output root");
    let output = root.path().join("missing-parent/app.main.mjs");
    assert!(write_file_if_changed(&output, "export default null;").is_err());
    assert!(!output.exists());
  }
}
