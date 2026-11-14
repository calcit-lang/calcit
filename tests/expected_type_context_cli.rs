//! One expected-type context serves every form that cannot be typed
//! bottom-up (#2733): an unhinted lambda and a bare tag naming a zero-payload
//! enum variant are accepted wherever the position's type is known, including
//! `if` branches and `&let` bodies in argument position and the members of
//! `[]` / `#{}` / `{}` literals in argument or return position, `match`
//! branches, variant payloads, the later operands of `=`, method and local
//! function arguments, Tuple literals and literals in receiver-fixed slots,
//! the results of callbacks and generic parameters that the call's own
//! expected type fixes, and the operands of `recur`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const FIXTURE: &str = "tests/fixtures/expected-type-context.cirru";
const COLLECTION_ALIASES: &str = "tests/fixtures/expected-collection-aliases.cirru";

fn check(source: &str, label: &str) -> (bool, String) {
  let dir = std::env::temp_dir().join(format!("calcit-expected-type-context-{label}-{}", std::process::id()));
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
fn lambdas_and_tags_take_the_expected_type_of_their_position() {
  let source = fs::read_to_string(FIXTURE).unwrap();
  let (ok, text) = check(&source, "pass");
  assert!(ok, "{text}");
  for def in [
    "lambda-if-arg",
    "lambda-let-arg",
    "lambda-list",
    "lambda-map",
    "tag-if-arg",
    "tag-list-arg",
    "tag-list-return",
    "tag-set-return",
    "tag-map-return",
    "plain-tags",
    "tag-eq",
    "tag-some-return",
    "tag-qualified-option",
    "tag-payload",
    "tag-match-arg",
    "tag-match-return",
    "tag-literal-match",
    "tag-union-match",
    "tag-data-view-match",
    "match-scrutinee-keeps-tag",
    "tag-method-arg",
    "tag-local-fn-arg",
    "tag-tuple-return",
    "tag-tuple-list",
    "tag-assoc-field",
    "tag-map-callback",
    "tag-map-callback-if",
    "tag-method-callback",
    "tag-user-hof",
    "tag-generic-slot",
    "tag-generic-literal",
    "plain-callback-tags",
    "tag-recur-arg",
    "tag-recur-if",
    "tag-recur-list",
    "lambda-recur-arg",
  ] {
    assert!(text.contains(&format!("[passed] demo.main/{def} (fn)")), "{def}:\n{text}");
  }
}

#[test]
fn contextual_forms_run_with_their_typed_values() {
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(FIXTURE)
    .output()
    .expect("calcit command should run");
  let text = String::from_utf8_lossy(&output.stdout);
  assert!(output.status.success(), "{text}{}", String::from_utf8_lossy(&output.stderr));
  assert!(text.contains("2 3 6"), "{text}");
  assert!(text.contains("red green red,green"), "{text}");
  assert!(text.contains("green,red 1 2"), "{text}");
  assert!(text.contains("true false green red"), "{text}");
  assert!(text.contains("green 1"), "{text}");
  assert!(text.contains("red green green 1"), "{text}");
  assert!(text.contains("red green\n"), "{text}");
  assert!(text.contains("red green green 2"), "{text}");
  assert!(text.contains("red,red"), "{text}");
  assert!(text.contains("green,green green,red red red green 3"), "{text}");
  assert!(text.contains("green,green,red\n"), "{text}");
  assert!(text.contains("green red red,green,green 41 green red,green"), "{text}");
}

#[test]
fn a_wrong_tag_is_reported_at_its_position() {
  for (from, to, label, code, position) in [
    (
      "([] :red :green)",
      "([] :red :grene)",
      "list-typo",
      "E_ENUM_TAG_UNKNOWN",
      "collection member",
    ),
    (
      "(if c :red :green)",
      "(if c :red :grene)",
      "if-typo",
      "E_ENUM_TAG_UNKNOWN",
      "value in a typed position",
    ),
    (
      "([] :green :red)",
      "([] :green :rgb)",
      "list-payload",
      "E_ENUM_TAG_NEEDS_PAYLOAD",
      "collection member",
    ),
    ("(|a :red)", "(|a :rde)", "map-typo", "E_ENUM_TAG_UNKNOWN", "collection member"),
    ("(= c :red)", "(= c :rde)", "eq-typo", "E_ENUM_TAG_UNKNOWN", "arg 2 of `=`"),
    ("(Some :red)", "(Some :rde)", "some-typo", "E_ENUM_TAG_UNKNOWN", "variant payload"),
    (
      "(Wrap :one :green)",
      "(Wrap :one :rgb)",
      "payload-needs-payload",
      "E_ENUM_TAG_NEEDS_PAYLOAD",
      "variant payload",
    ),
    (
      "(:zero) :red",
      "(:zero) :rde",
      "match-arg-typo",
      "E_ENUM_TAG_UNKNOWN",
      "value in a typed position",
    ),
    (
      "(:zero) :green",
      "(:zero) :grene",
      "match-return-typo",
      "E_ENUM_TAG_UNKNOWN",
      "return value",
    ),
    ("0 :red", "0 :rde", "literal-match-typo", "E_ENUM_TAG_UNKNOWN", "return value"),
    (
      "(String s) :green",
      "(String s) :grene",
      "union-match-typo",
      "E_ENUM_TAG_UNKNOWN",
      "return value",
    ),
    (
      "(:string s) :red",
      "(:string s) :rde",
      "data-view-match-typo",
      "E_ENUM_TAG_UNKNOWN",
      "return value",
    ),
    (
      "(.unwrap-or o :red)",
      "(.unwrap-or o :rde)",
      "method-arg-typo",
      "E_ENUM_TAG_UNKNOWN",
      "method argument",
    ),
    (
      "f :green",
      "f :grene",
      "local-fn-arg-typo",
      "E_ENUM_TAG_UNKNOWN",
      "argument of a local function",
    ),
    (
      "([] :green 1)",
      "([] :grene 1)",
      "tuple-typo",
      "E_ENUM_TAG_UNKNOWN",
      "collection member",
    ),
    (
      "([] :red :red)",
      "([] :red :rgb)",
      "assoc-field-payload",
      "E_ENUM_TAG_NEEDS_PAYLOAD",
      "collection member",
    ),
    (
      "(fn (n) :green)",
      "(fn (n) :grene)",
      "callback-typo",
      "E_ENUM_TAG_UNKNOWN",
      "callback result",
    ),
    (
      "(fn (m) :red)",
      "(fn (m) :rgb)",
      "method-callback-payload",
      "E_ENUM_TAG_NEEDS_PAYLOAD",
      "callback result",
    ),
    (
      "(fn (k) :red)",
      "(fn (k) |red)",
      "user-hof-wrong-type",
      "declares return type",
      "list<string>",
    ),
    (
      "(recur (&- n 1) :green)",
      "(recur (&- n 1) :grene)",
      "recur-typo",
      "E_ENUM_TAG_UNKNOWN",
      "recur argument",
    ),
    (
      "(recur (&- n 1) ([] :red :green :green))",
      "(recur (&- n 1) ([] :red :rgb :green))",
      "recur-list-payload",
      "E_ENUM_TAG_NEEDS_PAYLOAD",
      "collection member",
    ),
    (
      "(recur (&- n 1) (fn (x) (&+ x 40)))",
      "(recur (&- n 1) 40)",
      "recur-wrong-type",
      "`recur` argument 2",
      "fn(:number) -> :number",
    ),
    (
      "(pick :green)",
      "(pick :grene)",
      "generic-slot-typo",
      "E_ENUM_TAG_UNKNOWN",
      "arg 1 of `pick`",
    ),
    (
      "(pick ([] :green :green :red))",
      "(pick ([] :green :grene :red))",
      "generic-literal-typo",
      "E_ENUM_TAG_UNKNOWN",
      "collection member",
    ),
  ] {
    let (ok, text) = check(&replaced(from, to), label);
    assert!(!ok, "{label}:\n{text}");
    assert!(text.contains(code) && text.contains(position), "{label}:\n{text}");
  }
}

#[test]
fn collection_aliases_offer_member_types_without_weakening_checks() {
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([COLLECTION_ALIASES, "test", "--tag", "unit", "--require-match"])
    .output()
    .expect("calcit command should run");
  assert!(
    output.status.success() && String::from_utf8_lossy(&output.stdout).contains("4 passed; 0 failed"),
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );

  let source = fs::read_to_string(COLLECTION_ALIASES).unwrap();
  assert!(source.contains("([] :red :green)"));
  let source = source.replacen("([] :red :green)", "([] :red :grene)", 1);
  let (ok, text) = check(&source, "collection-alias-typo");
  assert!(!ok, "{text}");
  assert!(text.contains("E_ENUM_TAG_UNKNOWN") && text.contains("collection member"), "{text}");
}
