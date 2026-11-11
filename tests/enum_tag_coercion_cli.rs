//! A bare tag at a position whose expected type is a definite enum names its
//! zero-payload variant (#2701): call arguments, local function arguments,
//! struct fields, map literals rewritten to structs, and returned forms.
//! Unknown tags, tags of payload variants and tags shared by several union
//! members are rejected at check time.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const FIXTURE: &str = "tests/fixtures/enum-tag-coercion.cirru";

fn check(source: &str, label: &str) -> (bool, String) {
  let dir = std::env::temp_dir().join(format!("calcit-enum-tag-coercion-{label}-{}", std::process::id()));
  let _ = fs::remove_dir_all(&dir);
  fs::create_dir_all(&dir).unwrap();
  let snapshot: PathBuf = dir.join("calcit.cirru");
  fs::write(&snapshot, source).unwrap();
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(&snapshot)
    .args(["--check-only", "--ns", "demo.main"])
    .output()
    .expect("calcit command should run");
  let _ = fs::remove_dir_all(&dir);
  let text = format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  (output.status.success(), text)
}

fn replaced(from: &str, to: &str) -> String {
  let source = fs::read_to_string(FIXTURE).unwrap();
  assert!(source.contains(from), "fixture should contain `{from}`");
  source.replace(from, to)
}

#[test]
fn bare_tags_at_enum_positions_pass() {
  let source = fs::read_to_string(FIXTURE).unwrap();
  let (ok, text) = check(&source, "pass");
  assert!(ok, "{text}");
  for def in ["default-filter", "pick-filter", "show-tag", "main!"] {
    assert!(text.contains(&format!("[passed] demo.main/{def} (fn)")), "{def}:\n{text}");
  }
}

#[test]
fn bare_tags_run_as_enum_values() {
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(FIXTURE)
    .output()
    .expect("calcit command should run");
  let text = String::from_utf8_lossy(&output.stdout);
  assert!(output.status.success(), "{text}{}", String::from_utf8_lossy(&output.stderr));
  assert!(text.contains("all active done done 2 all every"), "{text}");
  assert!(text.contains("directdone"), "{text}");
  assert!(text.contains("mapactive"), "{text}");
}

#[test]
fn unknown_tag_names_the_enum_and_its_variants() {
  let (ok, text) = check(&replaced("(show-filter :all)", "(show-filter :actve)"), "arg-typo");
  assert!(!ok, "{text}");
  assert!(text.contains("E_ENUM_TAG_UNKNOWN"), "{text}");
  assert!(text.contains("Filter has no `:actve` variant"), "{text}");
  assert!(text.contains(":active :all :done :tagged"), "{text}");

  let (ok, text) = check(&replaced("(:filter :done)", "(:filter :don)"), "field-typo");
  assert!(!ok, "{text}");
  assert!(text.contains("E_ENUM_TAG_UNKNOWN"), "{text}");

  let (ok, text) = check(
    &replaced("defn default-filter () :active", "defn default-filter () :activ"),
    "return-typo",
  );
  assert!(!ok, "{text}");
  assert!(text.contains("E_ENUM_TAG_UNKNOWN"), "{text}");

  let (ok, text) = check(&replaced("if done? :done :all", "if done? :done :al"), "if-typo");
  assert!(!ok, "{text}");
  assert!(text.contains("E_ENUM_TAG_UNKNOWN"), "{text}");

  let (ok, text) = check(&replaced("println $ local :done", "println $ local :dne"), "local-typo");
  assert!(!ok, "{text}");
  assert!(text.contains("E_ENUM_TAG_UNKNOWN"), "{text}");
}

#[test]
fn payload_variant_tag_is_rejected() {
  let (ok, text) = check(&replaced("(show-filter :all)", "(show-filter :tagged)"), "payload");
  assert!(!ok, "{text}");
  assert!(text.contains("E_ENUM_TAG_NEEDS_PAYLOAD"), "{text}");
}

#[test]
fn tag_shared_by_union_members_is_ambiguous() {
  let (ok, text) = check(&replaced("(show-either (Mode :all))", "(show-either :all)"), "ambiguous");
  assert!(!ok, "{text}");
  assert!(text.contains("E_ENUM_TAG_AMBIGUOUS"), "{text}");
  assert!(text.contains("Filter, Mode"), "{text}");

  // A tag only one member owns is not ambiguous.
  let (ok, text) = check(&replaced("(show-either (Mode :all))", "(show-either :dark)"), "unique-member");
  assert!(ok, "{text}");
}

#[test]
fn tag_and_dynamic_positions_keep_plain_tags() {
  // `show-tag :actve` is a plain Tag argument, never checked against an enum.
  let (ok, text) = check(&replaced("(show-tag :all)", "(show-tag :actve)"), "tag-typo-free");
  assert!(ok, "{text}");
}
