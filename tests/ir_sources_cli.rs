use std::fs;
use std::path::Path;
use std::process::Command;

use cirru_edn::Edn;

// `program-ir.cirru` is a file format consumed by external viewers, so the
// artifact shape is checked at the CLI boundary.
fn emit_ir(emit_path: &Path, extra: &[&str]) -> Edn {
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("tests/fixtures/ir-sources.cirru")
    .args(["--emit-path", emit_path.to_str().expect("UTF-8 emit path"), "ir"])
    .args(extra)
    .output()
    .expect("run calcit ir");
  assert!(
    output.status.success(),
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  cirru_edn::parse(&fs::read_to_string(emit_path.join("program-ir.cirru")).expect("IR artifact")).expect("IR should be valid Cirru EDN")
}

fn member<'a>(value: &'a Edn, key: Edn) -> &'a Edn {
  let Edn::Map(map) = value else { panic!("expected a map: {value}") };
  map.get(&key).unwrap_or_else(|| panic!("missing {key}: {value}"))
}

fn has_member(value: &Edn, key: Edn) -> bool {
  matches!(value, Edn::Map(map) if map.get(&key).is_some())
}

fn path_types(def: &Edn) -> Vec<(String, String)> {
  let Edn::List(items) = member(def, Edn::tag("types")) else {
    panic!("types should be a list")
  };
  items
    .iter()
    .map(|item| match item {
      Edn::List(pair) => match (pair.get(0), pair.get(1)) {
        (Some(Edn::Str(path)), Some(Edn::Str(annotation))) => (path.to_string(), annotation.to_string()),
        _ => panic!("type entry should be [path type]: {item}"),
      },
      _ => panic!("type entry should be a list: {item}"),
    })
    .collect()
}

#[test]
fn ir_sources_carry_source_types_and_dependencies() {
  let directory = tempfile::tempdir().expect("create emit directory");
  let ir = emit_ir(directory.path(), &["--sources-only"]);

  assert!(!has_member(&ir, Edn::tag("files")), "--sources-only skips lowered code");
  let meta = member(&ir, Edn::tag("meta"));
  assert_eq!(member(meta, Edn::tag("schema-version")), &Edn::Number(1.0));
  assert_eq!(member(meta, Edn::tag("package")), &Edn::str("app"));
  assert_eq!(member(meta, Edn::tag("calcit-version")), &Edn::str(env!("CARGO_PKG_VERSION")));

  let sources = member(&ir, Edn::tag("sources"));
  let app = member(sources, Edn::str("app.main"));
  assert_eq!(member(app, Edn::tag("origin")), &Edn::tag("project"));
  assert_eq!(member(app, Edn::tag("ns-doc")), &Edn::str("IR sources fixture"));
  let defs = member(app, Edn::tag("defs"));

  let double = member(defs, Edn::str("double"));
  assert_eq!(member(double, Edn::tag("doc")), &Edn::str("Twice the input"));
  assert_eq!(member(double, Edn::tag("head")), &Edn::str("defn"));
  assert_eq!(member(double, Edn::tag("kind")), &Edn::tag("fn"));
  assert_eq!(member(double, Edn::tag("schema")), &Edn::str("Fn Number $ -> Number"));
  assert!(matches!(member(double, Edn::tag("source")), Edn::Quote(_)));
  assert!(matches!(member(double, Edn::tag("hash")), Edn::Str(hash) if hash.starts_with("md5:")));
  let types = path_types(double);
  // `twice` in `let (twice $ * n 2)` and the `(* n 2)` call it binds.
  assert!(types.contains(&("3.1.0.0".to_owned(), "Number".to_owned())), "{types:?}");
  assert!(types.contains(&("3.1.0.1".to_owned(), "Number".to_owned())), "{types:?}");

  // The head of a call to a builtin gets the builtin's signature.
  assert!(
    types.iter().any(|(path, t)| path == "3.1.0.1.0" && t.starts_with("Fn Number")),
    "{types:?}"
  );

  let main = member(defs, Edn::str("main!"));
  // The head of a call to another definition gets that definition's type.
  assert!(path_types(main).contains(&("3.0".to_owned(), "Fn Number $ -> Number".to_owned())));
  assert_eq!(
    member(main, Edn::tag("deps")),
    &Edn::List(cirru_edn::EdnListView(vec![Edn::str("app.main/double")]))
  );

  // Definitions the entry never reaches are still compiled for readers.
  let unused = member(defs, Edn::str("unused"));
  assert_eq!(member(unused, Edn::tag("type")), &Edn::str("Fn String $ -> String"));
  assert!(path_types(unused).contains(&("3".to_owned(), "String".to_owned())));

  // Typed decode calls name nominal types through a data-shape handle, not an import.
  let read_point = member(defs, Edn::str("read-point"));
  // Parameters are recorded even when their type is Dynamic.
  assert!(path_types(read_point).contains(&("2.0".to_owned(), "Dynamic".to_owned())));
  // A macro head such as `defstruct` lowers to another call and gets no type.
  assert!(!path_types(member(defs, Edn::str("Point"))).iter().any(|(path, _)| path == "0"));
  // A macro head written as a fully qualified `ns/def` is excluded too.
  assert!(
    !path_types(member(defs, Edn::str("qualified-macro")))
      .iter()
      .any(|(path, _)| path == "3.0")
  );
  assert_eq!(
    member(read_point, Edn::tag("deps")),
    &Edn::List(cirru_edn::EdnListView(vec![Edn::str("app.main/Point")]))
  );

  // A project namespace without definitions keeps its ns form for readers.
  let empty = member(sources, Edn::str("app.empty"));
  assert_eq!(member(empty, Edn::tag("ns-doc")), &Edn::str("Namespace without definitions"));
  assert!(matches!(member(empty, Edn::tag("ns-code")), Edn::Quote(_)));
  assert!(matches!(member(empty, Edn::tag("defs")), Edn::Map(map) if map.0.is_empty()));

  // Library namespaces list the definitions the compiled program holds.
  let core = member(sources, Edn::str("calcit.core"));
  assert_eq!(member(core, Edn::tag("origin")), &Edn::tag("core"));
  assert!(has_member(member(core, Edn::tag("defs")), Edn::str("*")));
  // Core syntax is a runtime stub; `:kind` names it as syntax.
  assert_eq!(
    member(member(member(core, Edn::tag("defs")), Edn::str("defn")), Edn::tag("kind")),
    &Edn::tag("syntax")
  );
}

#[test]
fn full_ir_keeps_lowered_files_next_to_sources() {
  let directory = tempfile::tempdir().expect("create emit directory");
  let ir = emit_ir(directory.path(), &[]);

  let files = member(member(&ir, Edn::tag("files")), Edn::str("app.main"));
  let compiled = member(files, Edn::tag("defs"));
  assert!(
    matches!(member(compiled, Edn::str("double")), Edn::List(_)),
    "lowered code keeps its list shape"
  );
  let defs = member(member(member(&ir, Edn::tag("sources")), Edn::str("app.main")), Edn::tag("defs"));
  assert!(matches!(member(defs, Edn::str("double")), Edn::Map(_)));
  assert!(has_member(&ir, Edn::tag("meta")));
}
