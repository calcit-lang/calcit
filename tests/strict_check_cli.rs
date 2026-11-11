use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
  fn create() -> Self {
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("test clock should be valid")
      .as_nanos();
    let counter = TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("calcit-strict-check-cli-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&path).expect("temporary project should create");
    Self(path)
  }

  fn snapshot(&self) -> PathBuf {
    self.0.join("calcit.cirru")
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

fn run_calcit(snapshot: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .args(args)
    .output()
    .expect("calcit command should run")
}

fn assert_success(output: &Output, context: &str) {
  assert!(
    output.status.success(),
    "{context} failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn slot_bound_to_union_accepts_callbacks_over_that_union() {
  // `:dispatch-op` is bound to the union `ViewOp`; a callback typed over the
  // same union is the slot's callback, and a slot value fits a `ViewOp`
  // parameter (#2642). A callback over one member
  // cannot take every slot value and stays rejected.
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/slot-union-callback.cirru", &snapshot).unwrap();
  assert_success(
    &run_calcit(&snapshot, &["--check-only", "--all-defs"]),
    "union-typed callback for a union-bound slot",
  );

  // A slot bound to one member passes where the union is expected.
  let member_directory = TestDirectory::create();
  let member_bound = member_directory.snapshot();
  let source = fs::read_to_string("tests/fixtures/slot-union-callback.cirru").unwrap();
  fs::write(
    &member_bound,
    source.replace("(:dispatch-op 'demo.main/ViewOp)", "(:dispatch-op 'demo.main/AppOp)"),
  )
  .unwrap();
  assert_success(
    &run_calcit(&member_bound, &["--check-only", "--all-defs"]),
    "member-bound slot value passed to a union parameter",
  );

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "demo.main/on-op!",
        "--code",
        "quote $ Fn demo.main/AppOp (-> Unit)",
      ],
    ),
    "narrow the callback to one member",
  );
  let rejected = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
  assert!(!rejected.status.success(), "a member-only callback cannot take every slot value");
  let output = format!(
    "{}{}",
    String::from_utf8_lossy(&rejected.stdout),
    String::from_utf8_lossy(&rejected.stderr)
  );
  assert!(output.contains("E_FN_ARG_TYPE_MISMATCH"), "{output}");
}

#[test]
fn enum_payload_lambdas_take_the_declared_payload_type() {
  // A lambda passed as a variant payload gets the declared `Fn Task (-> String)`
  // like a Struct field does, so a misspelled field on `t` is caught (#2644).
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/enum-payload-lambda.cirru", &snapshot).unwrap();
  let rejected = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
  assert!(!rejected.status.success(), "misspelled payload fields are reported");
  let output = format!(
    "{}{}",
    String::from_utf8_lossy(&rejected.stdout),
    String::from_utf8_lossy(&rejected.stderr)
  );
  // A generic payload keeps its concrete `Task` input; only `T` stays open.
  for definition in ["demo.main/anonymous-ctor", "demo.main/named-ctor", "demo.main/generic-ctor"] {
    let section = output
      .split(&format!("## `{definition}`"))
      .nth(1)
      .and_then(|rest| rest.split("\n## ").next())
      .unwrap_or_default();
    assert!(
      section.contains("E_UNKNOWN_STRUCT_FIELD") && section.contains("`:titel`"),
      "{definition} reports the unknown field of `Task`:\n{output}"
    );
  }
}

#[test]
fn option_fn_field_lambdas_take_the_payload_type() {
  // An `Option (Fn Click (-> Unit))` field takes an unhinted lambda through a
  // map literal, a direct constructor and `Some $ fn` alike, and the lambda
  // sees `Click` as its input type (D4).
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/option-fn-field-lambda.cirru", &snapshot).unwrap();
  assert_success(
    &run_calcit(&snapshot, &["--check-only", "--all-defs"]),
    "unhinted lambdas in an Option (Fn …) field",
  );

  let misspelled_directory = TestDirectory::create();
  let misspelled = misspelled_directory.snapshot();
  let source = fs::read_to_string("tests/fixtures/option-fn-field-lambda.cirru").unwrap();
  fs::write(&misspelled, source.replace("(:x e)", "(:xx e)")).unwrap();
  let rejected = run_calcit(&misspelled, &["--check-only", "--all-defs"]);
  assert!(!rejected.status.success(), "misspelled fields on the callback input are reported");
  let output = format!(
    "{}{}",
    String::from_utf8_lossy(&rejected.stdout),
    String::from_utf8_lossy(&rejected.stderr)
  );
  for definition in [
    "demo.main/map-literal",
    "demo.main/map-literal-some",
    "demo.main/direct-ctor",
    "demo.main/direct-ctor-some",
  ] {
    let section = output
      .split(&format!("## `{definition}`"))
      .nth(1)
      .and_then(|rest| rest.split("\n## ").next())
      .unwrap_or_default();
    assert!(
      section.contains("E_UNKNOWN_STRUCT_FIELD") && section.contains("`:xx`"),
      "{definition} reports the unknown field of `Click`:\n{output}"
    );
  }

  let wrong_result_directory = TestDirectory::create();
  let wrong_result = wrong_result_directory.snapshot();
  fs::write(&wrong_result, source.replace("fn (e) (println |ctor (:x e))", "fn (e) (:x e)")).unwrap();
  let rejected = run_calcit(&wrong_result, &["--check-only", "--all-defs"]);
  assert!(
    !rejected.status.success(),
    "a callback returning Number for a Unit result is reported"
  );
}

#[test]
fn explicit_dynamic_alias_does_not_recover_asserted_trait_capabilities() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/enum-slot-alias.cirru", &snapshot).unwrap();
  for (kind, target, code) in [
    (
      "def",
      "OpenCountable",
      "quote $ def OpenCountable $ assert-type ([] 1) calcit.core/Countable",
    ),
    ("schema", "OpenCountable", "quote Dynamic"),
    ("def", "AliasCountable", "quote $ def AliasCountable OpenCountable"),
    ("def", "consume-countable", "quote $ defn consume-countable (x) &unit"),
    (
      "schema",
      "consume-countable",
      "quote $ Fn T (-> Unit) (:generics T) (:where $ T calcit.core/Countable)",
    ),
    ("def", "main!", "quote $ defn main! () (consume-countable AliasCountable) &unit"),
  ] {
    let target = format!("app.main/{target}");
    let mut args = vec!["edit", kind, &target, "--input-format", "cirru", "--code", code];
    if target == "app.main/main!" {
      args.push("--overwrite");
    }
    assert_success(&run_calcit(&snapshot, &args), "prepare asserted capability alias");
  }

  let rejected = run_calcit(&snapshot, &["--check-only"]);
  assert!(!rejected.status.success(), "explicit Dynamic must not recover a Trait capability");
  assert!(
    String::from_utf8_lossy(&rejected.stderr).contains("_GENERIC_WHERE_BOUND_MISMATCH"),
    "{}",
    String::from_utf8_lossy(&rejected.stderr)
  );

  assert_success(
    &run_calcit(&snapshot, &["edit", "schema", "app.main/OpenCountable", "--clear"]),
    "remove the open declaration for the inference control",
  );
  assert_success(
    &run_calcit(&snapshot, &["--check-only"]),
    "a missing schema can retain independently checked trait evidence",
  );
}

#[test]
fn enum_schema_and_inferred_aliases_prove_entry_slots_without_accepting_open_inputs() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/enum-slot-alias.cirru", &snapshot).unwrap();
  let operations = vec![
    serde_json::json!([
      "edit",
      "def",
      "app.main/Wrong",
      "--input-format",
      "cirru",
      "--code",
      "quote $ def Wrong |text"
    ]),
    serde_json::json!([
      "edit",
      "schema",
      "app.main/Wrong",
      "--input-format",
      "cirru",
      "--code",
      "quote String"
    ]),
  ];
  let operations = serde_json::to_string(&operations).unwrap();
  let preview = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", &operations, "--dry-run", "--format", "json"],
  );
  assert_success(&preview, "preview entry slot callback fixture");
  let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "transaction",
        "--code",
        &operations,
        "--expect-revision",
        preview["scoped_revision"].as_str().unwrap(),
        "--format",
        "json",
      ],
    ),
    "apply guarded slot fixture",
  );
  for entry in ["default", "inferred"] {
    for args in [vec!["--check-only", "--all-defs"], vec!["--check-only"], vec![]] {
      let mut selected = vec!["--entry", entry];
      selected.extend(args);
      assert_success(
        &run_calcit(&snapshot, &selected),
        "declared/inferred Enum aliases work in both callback directions",
      );
    }
  }
  // A runtime effect in the representative is not evaluated by type checking.
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/Effectful",
        "--input-format",
        "cirru",
        "--code",
        "quote $ def Effectful $ do (println |SLOT_INITIALIZER_EXECUTED) (:: :sample)",
      ],
    ),
    "add effectful representative",
  );
  assert_success(
    &run_calcit(&snapshot, &["config", "set-type-slot", "dispatch-op", "app.main/Effectful"]),
    "bind effectful representative",
  );
  for args in [vec!["--check-only"], vec!["--check-only", "--all-defs"], vec![]] {
    let output = run_calcit(&snapshot, &args);
    assert_success(&output, "representative inference must not execute the initializer");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("SLOT_INITIALIZER_EXECUTED"));
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/UnannotatedWrong",
        "--input-format",
        "cirru",
        "--code",
        "quote $ def UnannotatedWrong |text",
      ],
    ),
    "add inferred wrong kind",
  );
  for (kind, code) in [("def", "quote $ def ExplicitDynamic $ :: :sample"), ("schema", "quote Dynamic")] {
    assert_success(
      &run_calcit(
        &snapshot,
        &["edit", kind, "app.main/ExplicitDynamic", "--input-format", "cirru", "--code", code],
      ),
      "explicit Dynamic representative",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/ThroughDynamic",
        "--input-format",
        "cirru",
        "--code",
        "quote $ def ThroughDynamic ExplicitDynamic",
      ],
    ),
    "alias of explicitly Dynamic value",
  );
  for binding in [
    "app.main/Wrong",
    "app.main/UnannotatedWrong",
    "app.main/ExplicitDynamic",
    "app.main/ThroughDynamic",
    ":dynamic",
  ] {
    assert_success(
      &run_calcit(&snapshot, &["config", "set-type-slot", "dispatch-op", binding]),
      "select negative slot binding",
    );
    let result = run_calcit(&snapshot, &["--check-only", "--all-defs", "--format", "json"]);
    assert!(
      !result.status.success(),
      "{binding} must not prove Enum: {}",
      String::from_utf8_lossy(&result.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(
      report["data"]["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .any(
          |item| item["diagnostics"].as_array().unwrap().iter().any(|diagnostic| diagnostic["code"]
            .as_str()
            .is_some_and(|code| code.contains("TYPE_MISMATCH") || code.starts_with("E_CALL_ARGUMENT_")))
        ),
      "{report}"
    );
  }
  assert_success(
    &run_calcit(&snapshot, &["config", "rm-type-slot", "dispatch-op"]),
    "remove slot binding",
  );
  let result = run_calcit(&snapshot, &["--check-only", "--all-defs", "--format", "json"]);
  assert!(!result.status.success());
  assert!(String::from_utf8_lossy(&result.stdout).contains("E_UNBOUND_TYPE_SLOT"));
  for (name, code) in [
    ("app.main/CycleA", "quote $ def CycleA CycleB"),
    ("app.main/CycleB", "quote $ def CycleB CycleA"),
  ] {
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", name, "--input-format", "cirru", "--code", code]),
      "add cyclic representative",
    );
  }
  assert_success(
    &run_calcit(&snapshot, &["config", "set-type-slot", "dispatch-op", "app.main/CycleA"]),
    "bind cyclic representative",
  );
  let result = run_calcit(&snapshot, &["--check-only"]);
  assert_eq!(
    result.status.code(),
    Some(1),
    "cycles must fail normally, not overflow or terminate by signal"
  );
  assert!(String::from_utf8_lossy(&result.stderr).contains("TYPE_MISMATCH"));
}

#[test]
fn entry_slot_preparation_respects_scope_dependencies_and_snapshot_reuse() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/enum-slot-alias.cirru", &snapshot).unwrap();
  let edit = |target: &str, code: &str| {
    assert_success(
      &run_calcit(
        &snapshot,
        &["edit", "def", target, "--overwrite", "--input-format", "cirru", "--code", code],
      ),
      target,
    );
  };
  edit("app.main/Broken", "quote $ def Broken $ missing-function 1");
  edit("app.main/LocalOp", "quote $ defenum LocalOp (:sample)");
  edit(
    "app.main/main!",
    "quote $ defn main! () $ with-type-slot (:dispatch-op LocalOp) $ forward (LocalOp :sample)",
  );
  // This callback uses the scoped slot but does not require anonymous Enum.
  edit("app.main/forward", "quote $ defn forward (op) &unit");
  assert_success(
    &run_calcit(&snapshot, &["config", "set-type-slot", "dispatch-op", "app.main/Broken"]),
    "bind unused broken representative",
  );
  for args in [vec!["--check-only"], vec![]] {
    assert_success(
      &run_calcit(&snapshot, &args),
      "scoped override must precede entry evidence preparation",
    );
  }
  // Restore the shared program to verify dependent representatives and JS
  // inventory compilation through the same public entry points.
  fs::copy("tests/fixtures/enum-slot-alias.cirru", &snapshot).unwrap();
  edit(
    "app.main/ARepresentative",
    "quote $ def ARepresentative $ do (accept callback) (:: :sample)",
  );
  edit(
    "app.main/ZRepresentative",
    "quote $ def ZRepresentative $ do (deftype-slot :inventory-once) (:: :sample)",
  );
  for (slot, binding) in [
    ("a-dependent", "app.main/ARepresentative"),
    ("dispatch-op", "app.main/ZRepresentative"),
  ] {
    assert_success(
      &run_calcit(&snapshot, &["config", "set-type-slot", slot, binding]),
      "bind representative dependency",
    );
  }
  edit("app.main/SlotAlias", "quote $ defn SlotAlias (op) &unit");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "app.main/SlotAlias",
        "--input-format",
        "cirru",
        "--code",
        "quote $ Fn *a-dependent (-> Unit)",
      ],
    ),
    "slot inside a named schema alias",
  );
  edit(
    "app.main/require-dependent",
    "quote $ defn require-dependent (callback) $ callback (:: :sample)",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "app.main/require-dependent",
        "--input-format",
        "cirru",
        "--code",
        "quote $ Fn SlotAlias (-> Unit)",
      ],
    ),
    "dependent slot schema",
  );
  edit("app.main/main!", "quote $ defn main! () $ require-dependent callback");
  for args in [vec!["--check-only"], vec!["--check-only", "--all-defs"], vec![]] {
    assert_success(
      &run_calcit(&snapshot, &args),
      "representative dependency compiles before callback validation",
    );
  }
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .args(["--tips-level", "none", "--emit-path"])
    .arg(directory.0.join("js-out"))
    .arg(&snapshot)
    .arg("js")
    .output()
    .unwrap();
  assert_success(&output, "snapshot reuses representative without redeclaring its slot");
}

#[test]
fn malformed_struct_match_reports_the_pattern_without_log_capability_noise() {
  let directory = TestDirectory::create();
  for (arm, expected) in [
    (
      "(Tiny item) item",
      "struct-match expected (StructDef binding & body) or (_ binding & body)",
    ),
    ("(Tiny item) item &unit", "struct-match expected struct or symbol as pattern"),
    (
      "Tiny item",
      "struct-match expected (StructDef binding & body) or (_ binding & body)",
    ),
    ("_ other", "struct-match expected (StructDef binding & body) or (_ binding & body)"),
    ("()", "struct-match expected (StructDef binding & body) or (_ binding & body)"),
  ] {
    let snippet = format!("let\n    Tiny $ defstruct Tiny $ :value Number\n  struct-match (Tiny :value 1)\n    {arm}");
    let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
      .current_dir(&directory.0)
      .args(["--tips-level", "none", "eval", &snippet])
      .output()
      .expect("run the public CLI with its real platform API registration");
    assert!(!output.status.success(), "malformed struct-match arms must fail preprocessing");
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains(expected), "arm={arm}: {diagnostic}");
    assert!(
      diagnostic.contains("app.main/main!"),
      "the caller must remain in the source trace: {diagnostic}"
    );
    assert!(!diagnostic.contains("E_MACRO_CAPABILITY_MISSING"), "{diagnostic}");
    assert!(!diagnostic.contains("capability :log"), "{diagnostic}");
  }
}

#[test]
fn public_check_retains_cached_callee_warnings_on_their_source_definition() {
  for (caller, bad, passed) in [("a-caller", "z-bad", 2), ("z-caller", "a-bad", 3)] {
    let directory = TestDirectory::create();
    let snapshot = directory.snapshot();
    fs::copy("tests/fixtures/def-value-schema.cirru", &snapshot).expect("copy public-check fixture");
    let bad_id = format!("app.check-probe/{bad}");
    let mut operations = vec![
      serde_json::json!(["config", "set", "target", "native"]),
      serde_json::json!(["edit", "add-ns", "app.check-probe"]),
      serde_json::json!(["edit", "add-ns", "app.other-probe"]),
    ];
    let number_fn = "quote $ Fn (-> Number)";
    for (id, code, schema) in [
      (
        format!("app.check-probe/{caller}"),
        format!("quote $ defn {caller} () ({bad} |x)"),
        number_fn,
      ),
      (
        bad_id.clone(),
        format!("quote $ defn {bad} (x) (inc x)"),
        "quote $ Fn String (-> Number)",
      ),
      ("app.check-probe/good".to_owned(), "quote $ defn good () 1".to_owned(), number_fn),
      (format!("app.other-probe/{bad}"), format!("quote $ defn {bad} () 1"), number_fn),
    ] {
      operations.push(serde_json::json!(["edit", "def", id, "--input-format", "cirru", "--code", code]));
      operations.push(serde_json::json!([
        "edit",
        "schema",
        id,
        "--input-format",
        "cirru",
        "--code",
        schema
      ]));
    }
    let operations = serde_json::to_string(&operations).unwrap();
    let preview = run_calcit(
      &snapshot,
      &["edit", "transaction", "--code", &operations, "--dry-run", "--format", "json"],
    );
    assert_success(&preview, "preview diagnostic ownership fixture");
    let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "transaction",
          "--code",
          &operations,
          "--expect-revision",
          preview["scoped_revision"].as_str().unwrap(),
          "--format",
          "json",
        ],
      ),
      "apply guarded diagnostic ownership fixture",
    );
    let original = fs::read(&snapshot).unwrap();

    let output = run_calcit(
      &snapshot,
      &[
        "--check-only",
        "--ns",
        "app.check-probe",
        "--ns",
        "app.other-probe",
        "--format",
        "json",
      ],
    );
    assert!(!output.status.success(), "the known String-to-Number mismatch must fail checking");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["data"]["summary"]["complete"], true, "{report}");
    assert_eq!(report["data"]["summary"]["passed"], false, "{report}");
    let definitions = report["data"]["definitions"].as_array().unwrap();
    let bad_result = definitions.iter().find(|row| row["id"] == bad_id).unwrap();
    assert_eq!(bad_result["status"], "warning", "{report}");
    let same_name = format!("app.other-probe/{bad}");
    assert_eq!(definitions.iter().find(|row| row["id"] == same_name).unwrap()["status"], "passed");
    assert_eq!(
      definitions.iter().find(|row| row["definition"] == "good").unwrap()["status"],
      "passed"
    );
    assert_eq!(report["data"]["summary"]["definitions_passed"], passed, "{report}");
    assert_eq!(report["data"]["summary"]["definitions_checked"], 4, "{report}");
    let diagnostics = report["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1, "{report}");
    assert_eq!(diagnostics[0]["code"], "E_FN_ARG_TYPE_MISMATCH");
    assert_eq!(diagnostics[0]["expected"], ":number");
    assert_eq!(diagnostics[0]["actual"], ":string");
    assert_eq!(
      diagnostics[0]["location"],
      serde_json::json!({"ns":"app.check-probe", "def":bad, "coord":[3,1]})
    );

    let independent = run_calcit(
      &snapshot,
      &[
        "--check-only",
        "--keep-going",
        "--init-fn",
        &bad_id,
        "--reload-fn",
        &bad_id,
        "--format",
        "json",
      ],
    );
    assert!(!independent.status.success());
    let independent: serde_json::Value = serde_json::from_slice(&independent.stdout).unwrap();
    assert_eq!(independent["data"]["summary"]["failed"], 1);
    assert_eq!(independent["data"]["summary"]["passed"], 0);
    assert_eq!(fs::read(&snapshot).unwrap(), original, "checking must not write the snapshot");
  }
}

#[test]
fn method_on_impl_traits_alias_reports_the_failed_trait_load() {
  // #1712: a trait that fails to load leaves the `impl-traits` alias without impls;
  // the method call must surface that failure instead of `unknown method`.
  // #2128: the invalid signature is now reported at its declaration during preprocessing.
  let output = run_calcit(Path::new("tests/fixtures/enum-impl-load-error.cirru"), &["--check-only"]);
  assert!(!output.status.success(), "a trait that fails to load must fail checking");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("at app.core/HookAtomOps"), "stderr: {stderr}");
  assert!(
    stderr.contains("does not allow Dynamic inside method signatures"),
    "stderr: {stderr}"
  );
  assert!(!stderr.contains("unknown method `.deref`"), "stderr: {stderr}");
}

#[test]
fn immutable_value_schemas_reject_contradictions_before_codegen() {
  for (target, schema) in [
    ("app.values/initial-state", "quote $ Map Tag String"),
    ("app.main/initial-state", "quote $ Map Tag String"),
    ("app.main/state-alias", "quote $ Map Tag String"),
    ("app.reader/state-alias", "quote $ Map Tag String"),
    ("app.main/members", "quote $ Map String String"),
    ("app.main/answer", "quote String"),
    ("app.main/label", "quote Number"),
  ] {
    let directory = TestDirectory::create();
    let snapshot = directory.snapshot();
    fs::copy("tests/fixtures/def-value-schema.cirru", &snapshot).expect("copy value contract fixture");
    assert_success(&run_calcit(&snapshot, &["--check-only"]), "valid value schemas");
    assert_success(
      &run_calcit(&snapshot, &["test", "--tag", "def-value-contract", "--require-match"]),
      "Calcit attached value semantics",
    );
    assert_success(
      &run_calcit(&snapshot, &["edit", "schema", target, "--input-format", "cirru", "--code", schema]),
      "introduce contradictory value schema",
    );
    let original = fs::read(&snapshot).unwrap();
    let output_directory = directory.0.join("rejected-js");
    for args in [
      vec!["--check-only"],
      vec!["--check-only", "--keep-going", "--format", "edn"],
      vec!["--emit-path", output_directory.to_str().unwrap(), "js"],
    ] {
      let output = run_calcit(&snapshot, &args);
      assert!(!output.status.success(), "contradictory {target} schema was accepted");
      let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
      );
      assert!(report.contains("E_SCHEMA_DEF_MISMATCH"), "{report}");
      assert!(report.contains(target), "{report}");
      if !args.contains(&"--format") {
        assert!(report.contains(&format!("at {target} @2")), "{report}");
      }
      assert_eq!(fs::read(&snapshot).unwrap(), original);
    }
    assert!(!output_directory.join("app.main.mjs").exists());
  }
}

#[test]
fn mixed_collection_value_schemas_reject_known_members_and_preserve_open_policy() {
  // Invalid initializers cannot execute attached tests; valid mixed values
  // replay as collection-value-contract on native/JS in the shared runner.
  for (name, source, accepted) in [
    ("style", "quote $ def style $ {} (:width open-value) (:display :block)", true),
    ("style", "quote $ def style $ {} (:width open-value) (:display false)", false),
    ("style", "quote $ def style $ {} (:width false) (:display :block)", false),
    ("style", "quote $ def style $ {} (12 |bad-key) (:display :block)", false),
    ("values", "quote $ def values $ [] 12 false :block", false),
    ("members", "quote $ def members $ #{} 12 false :block", false),
    ("nested", "quote $ def nested $ {} (:items $ [] 12 false :block)", false),
    ("sequence-value", "quote $ def sequence-value $ [] 12 |ready :block", true),
    ("sequence-value", "quote $ def sequence-value $ [] open-value :block", true),
    ("sequence-value", "quote $ def sequence-value $ [] open-value false", false),
    ("sequence-value", "quote $ def sequence-value $ [] 12 false :block", false),
    ("pair-value", "quote $ def pair-value $ [] 12 false", false),
    ("pair-value", "quote $ def pair-value $ [] 12 |ready |extra", false),
    ("ref-value", "quote $ def ref-value $ ref $ [] 12 :block |sans", true),
    ("ref-value", "quote $ def ref-value $ ref $ [] 12 false :block", false),
    // A merged literal is checked against the receiver's contract too (#2589).
    (
      "style",
      "quote $ def style $ .merge base-style $ {} (:width 12) (:display :inline)",
      true,
    ),
    (
      "style",
      "quote $ def style $ .merge base-style $ {} (:width open-value) (:display :inline)",
      true,
    ),
    (
      "style",
      "quote $ def style $ .merge base-style $ {} (:display :inline) (:height true)",
      false,
    ),
    (
      "style",
      "quote $ def style $ merge base-style $ {} (:width open-value) (:height true)",
      false,
    ),
    (
      "app.collection-shadow/nested-values",
      "quote $ def nested-values $ [] 12 :block |sans",
      true,
    ),
    (
      "app.collection-shadow/nested-values",
      "quote $ def nested-values $ [] false 'bad",
      false,
    ),
  ] {
    let directory = TestDirectory::create();
    let snapshot = directory.snapshot();
    fs::copy("tests/fixtures/def-value-schema.cirru", &snapshot).unwrap();
    let target = if name.contains('/') {
      name.to_owned()
    } else {
      format!("app.collection-value/{name}")
    };
    let (namespace, _) = target.rsplit_once('/').unwrap();
    let probe = format!("{namespace}/probe!");
    let operations = serde_json::json!([
      ["config", "set", "target", "native"],
      [
        "edit",
        "def",
        "app.collection-value/ref-value",
        "--input-format",
        "cirru",
        "--code",
        "quote $ def ref-value $ ref $ [] 12 :block |sans"
      ],
      [
        "edit",
        "schema",
        "app.collection-value/ref-value",
        "--input-format",
        "cirru",
        "--code",
        "quote (Ref (List app.collection-value/Scalar))"
      ],
      [
        "edit",
        "def",
        "app.collection-value/open-value",
        "--input-format",
        "cirru",
        "--code",
        "quote $ def open-value 12"
      ],
      [
        "edit",
        "schema",
        "app.collection-value/open-value",
        "--input-format",
        "cirru",
        "--code",
        "quote Dynamic"
      ],
      [
        "edit",
        "def",
        "app.collection-value/base-style",
        "--input-format",
        "cirru",
        "--code",
        "quote $ def base-style $ {} (:display :block)"
      ],
      [
        "edit",
        "schema",
        "app.collection-value/base-style",
        "--input-format",
        "cirru",
        "--code",
        "quote $ Map Tag app.collection-value/Scalar"
      ],
      ["edit", "def", target, "--overwrite", "--input-format", "cirru", "--code", source],
      [
        "edit",
        "def",
        &probe,
        "--input-format",
        "cirru",
        "--code",
        format!("quote $ defn probe! () (identity {target}) &unit")
      ],
      [
        "edit",
        "schema",
        &probe,
        "--input-format",
        "cirru",
        "--code",
        "quote $ Fn (-> Unit)"
      ]
    ])
    .to_string();
    let preview = run_calcit(
      &snapshot,
      &["edit", "transaction", "--code", &operations, "--dry-run", "--format", "json"],
    );
    assert_success(&preview, "preview collection initializer fixture");
    let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "transaction",
          "--code",
          &operations,
          "--expect-revision",
          preview["scoped_revision"].as_str().unwrap(),
        ],
      ),
      "apply guarded collection fixture",
    );
    let original = fs::read(&snapshot).unwrap();
    let output_directory = directory.0.join("checked-js");
    for mode in [
      vec!["--check-only"],
      vec!["--check-only", "--ns", namespace],
      vec!["--emit-path", output_directory.to_str().unwrap(), "js"],
    ] {
      let mut args = vec!["--init-fn", &probe, "--reload-fn", &probe];
      args.extend(mode);
      let output = run_calcit(&snapshot, &args);
      assert_eq!(
        output.status.success(),
        accepted,
        "{target}: {source}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
      );
      if !accepted {
        let report = format!(
          "{}{}",
          String::from_utf8_lossy(&output.stdout),
          String::from_utf8_lossy(&output.stderr)
        );
        let expected_text = if source.contains("merge") {
          "arg 2 expects type `map<tag, type app.collection-value/Scalar>`"
        } else {
          "E_SCHEMA_DEF_MISMATCH"
        };
        assert!(report.contains(expected_text) && report.contains(&target), "{report}");
        assert!(!output_directory.join(format!("{namespace}.mjs")).exists());
      }
      assert_eq!(fs::read(&snapshot).unwrap(), original);
    }
  }
}

#[test]
fn immutable_value_schema_check_does_not_execute_the_initializer() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/def-value-schema.cirru", &snapshot).expect("copy effect contract fixture");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/answer",
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        "quote $ def answer $ do (println |initializer-effect-marker) 42",
      ],
    ),
    "add an effectful but correctly typed initializer",
  );
  let output_directory = directory.0.join("generated-js");
  for args in [
    vec!["--check-only"],
    vec!["--check-only", "--keep-going", "--format", "edn"],
    vec!["--emit-path", output_directory.to_str().unwrap(), "js"],
  ] {
    let output = run_calcit(&snapshot, &args);
    assert_success(&output, "check effectful initializer without evaluating it");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("initializer-effect-marker"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("initializer-effect-marker"));
  }
  let runtime = run_calcit(&snapshot, &[]);
  assert_success(&runtime, "execute the unchanged value contract");
  assert_eq!(
    String::from_utf8_lossy(&runtime.stdout)
      .matches("initializer-effect-marker")
      .count(),
    1
  );
}

#[test]
fn large_case_under_a_dependency_chain_checks_without_aborting() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/large-case-check.cirru", &snapshot).expect("large case fixture should copy");
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "ordinary large case check");
  assert_success(
    &run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "edn"]),
    "definition graph must check the same large case",
  );
  assert_success(
    &run_calcit(&snapshot, &["test", "app.main/lookup", "--require-match"]),
    "Calcit tests retain first, middle, last and default branch results",
  );

  let patterns = (0..160)
    .map(|index| {
      if index == 159 {
        format!("(|p{index} (inc |not-a-number))")
      } else {
        format!("(|p{index} {index})")
      }
    })
    .collect::<Vec<_>>()
    .join(" ");
  edit_definition(
    &snapshot,
    "lookup",
    &format!("quote $ defn lookup (name) $ match name {patterns} (_ -1)"),
    true,
  );
  let invalid = run_calcit(&snapshot, &["--check-only"]);
  assert!(!invalid.status.success(), "an invalid late branch must still fail type checking");
  assert!(
    invalid.status.code().is_some(),
    "late-branch diagnostics must not abort the process"
  );
  let stderr = String::from_utf8_lossy(&invalid.stderr);
  assert!(!stderr.contains("stack overflow"), "{stderr}");
  assert!(stderr.contains("number") && stderr.contains("string"), "{stderr}");
}

#[test]
fn trait_bearing_enum_cycle_preserves_nominal_evidence_in_graph_checks() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/enum-impl-cycle.cirru", &snapshot).expect("enum cycle fixture should copy");
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "ordinary enum cycle check");
  assert_success(
    &run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "edn"]),
    "definition graph must preserve the same nominal constructor evidence",
  );
  assert_success(&run_calcit(&snapshot, &["test", "--require-match"]), "attached nominal method test");
  let original = fs::read(&snapshot).unwrap();
  let workflow = run_calcit(&snapshot, &["fix", "--workflow", "strict", "--verify", "--format", "json"]);
  // Both graph validation and independent return proof must succeed, including
  // assert='s lexical raise branch and the recursive equality dependency.
  assert_success(
    &workflow,
    "strict verification must preserve nominal cycle and independent exit evidence",
  );
  let report: serde_json::Value = serde_json::from_slice(&workflow.stdout).expect("strict workflow must preserve a structured report");
  assert_eq!(report["data"]["workflow"]["status"], "passed");
  let results = report["data"]["workflow"]["verification"]["results"].as_array().unwrap();
  assert_eq!(results.len(), 1);
  assert_eq!(results[0]["status"], "passed");
  let definitions = results[0]["report"]["data"]["definitions"].as_array().unwrap();
  assert_eq!(definitions.len(), 7);
  assert!(definitions.iter().all(|definition| definition["status"] == "passed"));
  let diagnostics = report["diagnostics"].as_array().unwrap();
  assert!(diagnostics.is_empty(), "{diagnostics:?}");
  assert_eq!(fs::read(&snapshot).unwrap(), original);

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.reader/%make-plugin",
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn %make-plugin (value) (%:: Plugin :item |wrong-payload)",
      ],
    ),
    "introduce an invalid enum payload",
  );
  let invalid = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "edn"]);
  assert!(
    !invalid.status.success(),
    "source-backed nominal recovery must not waive payload checking"
  );

  fs::copy("tests/fixtures/enum-impl-cycle.cirru", &snapshot).expect("valid fixture should restore before cyclic alias check");
  assert_success(
    &run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "edn"]),
    "restored fixture must pass before introducing the cyclic alias",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.reader/Plugin",
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        "quote $ def Plugin Plugin",
      ],
    ),
    "introduce a cyclic source alias",
  );
  let cyclic = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "edn"]);
  assert!(!cyclic.status.success(), "cyclic source aliases must not supply nominal evidence");
  assert!(
    cyclic.status.code().is_some(),
    "cyclic source resolution must fail normally, not abort"
  );
  assert!(!String::from_utf8_lossy(&cyclic.stderr).contains("stack overflow"));
}

fn edit_definition(snapshot: &Path, name: &str, code: &str, overwrite: bool) {
  let target = format!("app.main/{name}");
  let mut args = vec!["edit", "def", target.as_str(), "--code", code];
  if overwrite {
    args.push("--overwrite");
  }
  assert_success(&run_calcit(snapshot, &args), "edit definition");
}

fn set_schema(snapshot: &Path, name: &str, code: &str) {
  let target = format!("app.main/{name}");
  assert_success(
    &run_calcit(snapshot, &["edit", "schema", target.as_str(), "--code", code]),
    "edit schema",
  );
}

fn edit_schema(snapshot: &Path, name: &str, return_type: &str) {
  let target = format!("app.main/{name}");
  let code = format!("quote $ Fn (-> {return_type})");
  assert_success(
    &run_calcit(snapshot, &["edit", "schema", target.as_str(), "--code", code.as_str()]),
    "edit schema",
  );
}

fn prepare_project() -> (TestDirectory, PathBuf) {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");

  edit_definition(&snapshot, "bad-a", "quote $ defn bad-a () $ inc |x", false);
  edit_definition(&snapshot, "bad-b", "quote $ defn bad-b () $ inc |y", false);
  edit_definition(&snapshot, "cycle-a", "quote $ defn cycle-a () $ cycle-z", false);
  edit_definition(
    &snapshot,
    "cycle-z",
    "quote $ defn cycle-z () $ do (inc |cycle-error) (cycle-a)",
    false,
  );
  edit_definition(&snapshot, "dependent", "quote $ defn dependent () $ bad-a", false);
  edit_definition(&snapshot, "reload!", "quote $ defn reload! () nil", true);
  edit_definition(&snapshot, "main!", "quote $ defn main! () $ do (dependent) (bad-b) (cycle-a)", true);

  for name in ["bad-a", "bad-b", "cycle-a", "cycle-z", "dependent", "main!"] {
    edit_schema(&snapshot, name, "Number");
  }
  edit_schema(&snapshot, "reload!", "Nil");
  (directory, snapshot)
}

#[test]
fn recur_arguments_follow_the_lexical_function_contract() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  assert_success(&run_calcit(&snapshot, &["config", "set", "target", "native"]), "set native target");
  edit_definition(
    &snapshot,
    "typed-recur",
    "quote $ defn typed-recur (label index) $ if (>= index 2) index $ recur label (inc index)",
    false,
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "app.main/typed-recur",
        "--code",
        "quote $ Fn String Number (-> Number)",
      ],
    ),
    "declare typed recur contract",
  );
  edit_definition(&snapshot, "main!", "quote $ defn main! () $ typed-recur |point 0", true);
  edit_schema(&snapshot, "main!", "Number");

  assert_success(&run_calcit(&snapshot, &["--check-only"]), "correct recur argument order");
  assert_success(
    &run_calcit(&snapshot, &["--check-only", "--ns", "app.main", "--format", "json"]),
    "correct public contract",
  );

  edit_definition(
    &snapshot,
    "typed-recur",
    "quote $ defn typed-recur (label index) $ if (>= index 2) index $ recur (inc index) label",
    true,
  );
  let strict = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "json"]);
  assert!(!strict.status.success(), "wrong recur argument order must fail strict checking");
  let report: serde_json::Value = serde_json::from_slice(&strict.stdout).expect("strict check should return JSON");
  let diagnostics = report["data"]["definitions"]
    .as_array()
    .expect("definitions should be an array")
    .iter()
    .find(|definition| definition["definition"] == "app.main/typed-recur")
    .expect("typed recur definition should be present")["diagnostics"]
    .as_array()
    .expect("diagnostics should be an array");
  let mismatches = diagnostics
    .iter()
    .filter(|diagnostic| diagnostic["code"] == "E_RECUR_ARG_TYPE_MISMATCH")
    .collect::<Vec<_>>();
  assert_eq!(mismatches.len(), 2, "report: {report}");
  assert_eq!(mismatches[0]["expected"], ":string");
  assert_eq!(mismatches[0]["actual"], ":number");
  assert_eq!(mismatches[0]["path"], serde_json::json!([3, 3, 1, 1]));
  assert_eq!(mismatches[1]["expected"], ":number");
  assert_eq!(mismatches[1]["actual"], ":string");
  assert_eq!(mismatches[1]["path"], serde_json::json!([3, 3, 2]));
  assert!(
    mismatches[0]["message"]
      .as_str()
      .expect("recur diagnostic should include a message")
      .contains("app.main/typed-recur")
  );

  let public = run_calcit(&snapshot, &["--check-only", "--ns", "app.main", "--format", "json"]);
  assert!(!public.status.success(), "public analysis must reject wrong recur arguments");
  let report: serde_json::Value = serde_json::from_slice(&public.stdout).expect("public check should return JSON");
  let recur_diagnostics = report["diagnostics"]
    .as_array()
    .expect("diagnostics should be an array")
    .iter()
    .filter(|diagnostic| diagnostic["code"] == "E_RECUR_ARG_TYPE_MISMATCH")
    .collect::<Vec<_>>();
  assert_eq!(recur_diagnostics.len(), 2, "report: {report}");
  assert_eq!(recur_diagnostics[0]["location"]["def"], "typed-recur");
  assert_eq!(recur_diagnostics[0]["location"]["coord"], serde_json::json!([3, 3, 1, 1]));
  assert_eq!(recur_diagnostics[1]["location"]["coord"], serde_json::json!([3, 3, 2]));

  // A direct self-call in tail position is lowered to `recur` (#1553); the
  // lowered call follows the same contract as a written `recur`.
  edit_definition(
    &snapshot,
    "typed-recur",
    "quote $ defn typed-recur (label index) $ if (>= index 2) index $ typed-recur label (inc index)",
    true,
  );
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "correct self-call argument order");
  edit_definition(
    &snapshot,
    "typed-recur",
    "quote $ defn typed-recur (label index) $ if (>= index 2) index $ typed-recur (inc index) label",
    true,
  );
  let self_call = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "json"]);
  assert!(!self_call.status.success(), "wrong self-call argument order must fail checking");
  let report: serde_json::Value = serde_json::from_slice(&self_call.stdout).expect("check should return JSON");
  let mismatches = report["data"]["definitions"]
    .as_array()
    .expect("definitions should be an array")
    .iter()
    .find(|definition| definition["definition"] == "app.main/typed-recur")
    .expect("typed recur definition should be present")["diagnostics"]
    .as_array()
    .expect("diagnostics should be an array")
    .iter()
    .filter(|diagnostic| diagnostic["code"] == "E_RECUR_ARG_TYPE_MISMATCH")
    .map(|diagnostic| diagnostic["path"].clone())
    .collect::<Vec<_>>();
  assert_eq!(
    mismatches,
    vec![serde_json::json!([3, 3, 1, 1]), serde_json::json!([3, 3, 2])],
    "report: {report}"
  );
}

#[test]
fn methods_on_a_schema_alias_dispatch_through_the_aliased_type() {
  // `Coord` is a plain schema alias for `List Number`, so `.rest xs` keeps the
  // list contract and `recur` accepts it (#2590); unknown methods still fail.
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  assert_success(&run_calcit(&snapshot, &["config", "set", "target", "native"]), "set native target");
  let set_schema = |name: &str, code: &str| {
    let target = format!("app.main/{name}");
    assert_success(
      &run_calcit(&snapshot, &["edit", "schema", target.as_str(), "--code", code]),
      "edit schema",
    );
  };
  edit_definition(&snapshot, "Coord", "quote $ def Coord $ []", false);
  set_schema("Coord", "quote $ List Number");
  edit_definition(
    &snapshot,
    "drain",
    "quote $ defn drain (xs acc) $ if (.empty? xs) acc $ recur (.rest xs) (+ acc (.len xs))",
    false,
  );
  set_schema("drain", "quote $ Fn app.main/Coord Number (-> Number)");
  edit_definition(&snapshot, "main!", "quote $ defn main! () $ println $ drain ([] 1 2 3) 0", true);
  edit_schema(&snapshot, "main!", "Unit");

  assert_success(&run_calcit(&snapshot, &["--check-only"]), "methods on a schema alias");
  let run = run_calcit(&snapshot, &[]);
  assert_success(&run, "run recursion over a schema alias");
  assert!(
    String::from_utf8_lossy(&run.stdout).contains('6'),
    "{}",
    String::from_utf8_lossy(&run.stdout)
  );

  edit_definition(&snapshot, "bad", "quote $ defn bad (xs) $ .no-such-method xs", false);
  set_schema("bad", "quote $ Fn app.main/Coord (-> Number)");
  edit_definition(&snapshot, "main!", "quote $ defn main! () $ println $ bad $ [] 1", true);
  let unknown = run_calcit(&snapshot, &["--check-only"]);
  assert!(!unknown.status.success(), "unknown method on a schema alias must fail checking");
  let stderr = String::from_utf8_lossy(&unknown.stderr);
  let stdout = String::from_utf8_lossy(&unknown.stdout);
  assert!(
    stderr.contains("unknown method `.no-such-method` for type app.main/Coord")
      || stdout.contains("unknown method `.no-such-method` for type app.main/Coord"),
    "stdout:\n{stdout}\nstderr:\n{stderr}"
  );
}

/// #2521: the bundled core `cirru-quote?` guard narrows a Dynamic argument
/// before it fills a CirruQuote struct field; a same-named user function does not.
#[test]
fn core_cirru_quote_guard_narrows_dynamic_for_struct_fields() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  assert_success(&run_calcit(&snapshot, &["config", "set", "target", "native"]), "set native target");
  edit_definition(&snapshot, "QuoteBox", "quote $ defstruct QuoteBox (:code CirruQuote)", false);
  edit_definition(
    &snapshot,
    "checked-quote",
    "quote $ defn checked-quote (value) $ if (cirru-quote? value) (QuoteBox :code value) (raise |Expected-CirruQuote)",
    false,
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "app.main/checked-quote",
        "--code",
        "quote $ Fn Dynamic (-> app.main/QuoteBox)",
      ],
    ),
    "declare Dynamic to QuoteBox contract",
  );
  let public = run_calcit(&snapshot, &["--check-only", "--ns", "app.main", "--format", "json"]);
  assert_success(&public, "core cirru-quote? guard narrows the field value");

  // A user predicate with the same short name is an ordinary Bool function.
  edit_definition(
    &snapshot,
    "checked-quote",
    "quote $ defn checked-quote (value) $ if (app.main/cirru-quote? value) (QuoteBox :code value) (raise |Expected-CirruQuote)",
    true,
  );
  edit_definition(&snapshot, "cirru-quote?", "quote $ defn cirru-quote? (x) true", false);
  let spoofed = run_calcit(&snapshot, &["--check-only", "--ns", "app.main", "--format", "json"]);
  assert!(
    !spoofed.status.success(),
    "a user predicate must not narrow Dynamic\nstdout:\n{}",
    String::from_utf8_lossy(&spoofed.stdout)
  );
  assert!(
    String::from_utf8_lossy(&spoofed.stdout).contains("QuoteBox"),
    "the field boundary should still be reported: {}",
    String::from_utf8_lossy(&spoofed.stdout)
  );
}

#[test]
fn keep_going_reports_independent_failures_and_blocks_dependents() {
  let (_directory, snapshot) = prepare_project();
  let output = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "json"]);
  assert!(!output.status.success(), "strict failures must retain a non-zero exit status");
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON envelope");
  assert_eq!(report["schema_version"], 1);
  assert_eq!(report["command"], "check-only");
  assert_eq!(report["data"]["summary"]["failed"], 3);
  assert_eq!(report["data"]["summary"]["blocked"], 3);
  assert_eq!(report["data"]["summary"]["cascaded"], 0);

  let definitions = report["data"]["definitions"].as_array().expect("definitions should be an array");
  let by_name = definitions
    .iter()
    .map(|item| (item["definition"].as_str().expect("definition should be text"), item))
    .collect::<std::collections::HashMap<_, _>>();
  assert_eq!(by_name["app.main/bad-a"]["status"], "failed");
  assert_eq!(by_name["app.main/bad-b"]["status"], "failed");
  assert_eq!(by_name["app.main/bad-a"]["diagnostics"][0]["expected"], ":number");
  assert_eq!(by_name["app.main/bad-a"]["diagnostics"][0]["actual"], ":string");
  assert_eq!(by_name["app.main/cycle-z"]["status"], "failed");
  assert_eq!(by_name["app.main/cycle-a"]["status"], "blocked");
  assert_eq!(by_name["app.main/cycle-a"]["blocked_by"], serde_json::json!(["app.main/cycle-z"]));
  assert_eq!(by_name["app.main/dependent"]["status"], "blocked");
  assert_eq!(by_name["app.main/dependent"]["blocked_by"], serde_json::json!(["app.main/bad-a"]));
  assert_eq!(by_name["app.main/main!"]["status"], "blocked");

  let repeated = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "json"]);
  assert_eq!(
    output.stdout, repeated.stdout,
    "definition and diagnostic ordering should be stable"
  );
}

#[test]
fn keep_going_supports_markdown_human_and_native_edn_reports() {
  let (_directory, snapshot) = prepare_project();
  let human = run_calcit(&snapshot, &["--check-only", "--keep-going"]);
  assert!(!human.status.success());
  let human_stdout = String::from_utf8_lossy(&human.stdout);
  assert!(human_stdout.starts_with("# Strict check\n"), "stdout:\n{human_stdout}");
  assert!(human_stdout.contains("## `app.main/bad-a`\n"), "stdout:\n{human_stdout}");
  assert!(human_stdout.contains("- status: **BLOCKED**"), "stdout:\n{human_stdout}");
  assert!(
    human_stdout.contains("```text\n"),
    "diagnostic text should have a Markdown boundary:\n{human_stdout}"
  );

  let edn = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "edn"]);
  assert!(!edn.status.success());
  let edn_stdout = String::from_utf8(edn.stdout).expect("EDN output should be UTF-8");
  let parsed = cirru_edn::parse(&edn_stdout).expect("stdout should contain one Cirru EDN envelope");
  let cirru_edn::Edn::Map(root) = parsed else {
    panic!("EDN envelope should be a map");
  };
  assert_eq!(root.get(&cirru_edn::Edn::tag("command")), Some(&cirru_edn::Edn::str("check-only")));
}

#[test]
fn default_check_only_remains_fail_fast_and_format_requires_keep_going() {
  let (_directory, snapshot) = prepare_project();
  let fail_fast = run_calcit(&snapshot, &["--check-only"]);
  assert!(!fail_fast.status.success());
  assert!(!String::from_utf8_lossy(&fail_fast.stdout).contains("# Strict check"));

  let invalid = run_calcit(&snapshot, &["--check-only", "--format", "json"]);
  assert!(!invalid.status.success());
  assert!(
    String::from_utf8_lossy(&invalid.stderr).contains("only available with `--check-only --keep-going`"),
    "stderr:\n{}",
    String::from_utf8_lossy(&invalid.stderr)
  );

  let explicit_strict = run_calcit(&snapshot, &["--check-only", "--keep-going", "--strict-types", "--format", "json"]);
  assert!(
    !explicit_strict.status.success(),
    "type errors must still fail explicit strict checking"
  );
  let report: serde_json::Value = serde_json::from_slice(&explicit_strict.stdout).expect("strict check should emit one JSON envelope");
  assert_eq!(report["data"]["summary"]["failed"], 3);
  assert!(
    !String::from_utf8_lossy(&explicit_strict.stderr).contains("quality gate"),
    "strict preprocessing must not run a statistical quality gate"
  );
}

#[test]
fn typed_edn_decoder_rejects_unknown_alias_and_missing_definition() {
  // Imported type names are resolved through the namespace's imports; an unknown
  // alias or a missing definition must stop preprocessing instead of widening to Dynamic.
  // Positive alias / `:refer` / full-path cases live in `test-edn.main/test-imported-type-names`.
  for (decoder, type_name, expected) in [
    (
      "parse-cirru-edn-as",
      "schema/Missing",
      "cannot resolve named type `test-edn.schema/Missing`",
    ),
    (
      "try-parse-cirru-edn-as",
      "schema/Missing",
      "cannot resolve named type `test-edn.schema/Missing`",
    ),
    ("parse-cirru-edn-as", "nope/External", "cannot resolve named type `nope/External`"),
    (
      "try-parse-cirru-edn-as",
      "nope/External",
      "cannot resolve named type `nope/External`",
    ),
  ] {
    let directory = TestDirectory::create();
    let snapshot = directory.snapshot();
    fs::copy("calcit/test-edn.cirru", &snapshot).expect("copy typed EDN fixture");
    fs::copy("calcit/util.cirru", directory.0.join("util.cirru")).expect("copy util module");
    let code = format!("quote $ defn probe-decoder () ({decoder} \"|%{{}} :External (:label |linked)\" {type_name})");
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          "test-edn.main/probe-decoder",
          "--input-format",
          "cirru",
          "--code",
          &code,
        ],
      ),
      "add decoder probe",
    );
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "add-test",
          "test-edn.main/probe-decoder",
          "probe",
          "--input-format",
          "cirru",
          "--code",
          "quote $ probe-decoder",
        ],
      ),
      "attach decoder probe test",
    );
    let output = run_calcit(&snapshot, &["test", "test-edn.main/probe-decoder", "--require-match"]);
    assert!(!output.status.success(), "{decoder} {type_name} was accepted");
    let report = format!(
      "{}{}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
    assert!(report.contains(&format!("{decoder} cannot derive a decoder")), "{report}");
    assert!(report.contains(expected), "{report}");
  }
}

fn edit_def_at(snapshot: &Path, target: &str, code: &str) {
  assert_success(
    &run_calcit(snapshot, &["edit", "def", target, "--overwrite", "--code", code]),
    "edit definition",
  );
}

fn edit_schema_at(snapshot: &Path, target: &str, return_type: &str) {
  let code = format!("quote $ Fn (-> {return_type})");
  assert_success(
    &run_calcit(snapshot, &["edit", "schema", target, "--code", code.as_str()]),
    "edit schema",
  );
}

/// Entry closure is clean; `unreferenced-bad` and `app.zeta/zeta-bad` are not referenced by any entry.
/// `app.alpha/alpha-user` sorts before the namespace it depends on, and `reversed` flips the creation order.
fn prepare_all_defs_project(reversed: bool, with_unreferenced_bad: bool) -> (TestDirectory, PathBuf) {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  edit_def_at(&snapshot, "app.main/main!", "quote $ defn main! () 1");
  edit_def_at(&snapshot, "app.main/reload!", "quote $ defn reload! () nil");
  edit_schema_at(&snapshot, "app.main/main!", "Number");
  edit_schema_at(&snapshot, "app.main/reload!", "Nil");

  let build_zeta = |snapshot: &Path| {
    assert_success(&run_calcit(snapshot, &["edit", "add-ns", "app.zeta"]), "add zeta namespace");
    edit_def_at(snapshot, "app.zeta/zeta-bad", "quote $ defn zeta-bad () $ inc |x");
    edit_schema_at(snapshot, "app.zeta/zeta-bad", "Number");
  };
  let build_alpha = |snapshot: &Path| {
    assert_success(&run_calcit(snapshot, &["edit", "add-ns", "app.alpha"]), "add alpha namespace");
    assert_success(
      &run_calcit(
        snapshot,
        &["edit", "add-import", "app.alpha", "--code", "quote $ app.zeta :refer $ zeta-bad"],
      ),
      "import zeta-bad",
    );
    edit_def_at(snapshot, "app.alpha/alpha-user", "quote $ defn alpha-user () $ zeta-bad");
    edit_schema_at(snapshot, "app.alpha/alpha-user", "Number");
  };
  if reversed {
    // Imports are resolved lazily, so the dependent namespace may exist before its provider.
    assert_success(&run_calcit(&snapshot, &["edit", "add-ns", "app.alpha"]), "add alpha namespace");
    assert_success(&run_calcit(&snapshot, &["edit", "add-ns", "app.zeta"]), "add zeta namespace");
    assert_success(
      &run_calcit(
        &snapshot,
        &["edit", "add-import", "app.alpha", "--code", "quote $ app.zeta :refer $ zeta-bad"],
      ),
      "import zeta-bad",
    );
    edit_def_at(&snapshot, "app.alpha/alpha-user", "quote $ defn alpha-user () $ zeta-bad");
    edit_schema_at(&snapshot, "app.alpha/alpha-user", "Number");
    edit_def_at(&snapshot, "app.zeta/zeta-bad", "quote $ defn zeta-bad () $ inc |x");
    edit_schema_at(&snapshot, "app.zeta/zeta-bad", "Number");
  } else {
    build_zeta(&snapshot);
    build_alpha(&snapshot);
  }

  if with_unreferenced_bad {
    edit_def_at(&snapshot, "app.main/unreferenced-bad", "quote $ defn unreferenced-bad () $ inc |y");
    edit_schema_at(&snapshot, "app.main/unreferenced-bad", "Number");
  }
  edit_def_at(&snapshot, "app.main/unreferenced-ok", "quote $ defn unreferenced-ok () 2");
  edit_schema_at(&snapshot, "app.main/unreferenced-ok", "Number");
  (directory, snapshot)
}

fn definition_statuses(report: &serde_json::Value) -> std::collections::BTreeMap<String, String> {
  report["data"]["definitions"]
    .as_array()
    .expect("definitions should be an array")
    .iter()
    .filter(|item| !item["definition"].as_str().unwrap().contains("$meta"))
    .map(|item| {
      (
        item["definition"].as_str().unwrap().to_owned(),
        item["status"].as_str().unwrap().to_owned(),
      )
    })
    .collect()
}

#[test]
fn all_defs_reports_unreferenced_definitions_without_changing_default_checks() {
  let (_directory, snapshot) = prepare_all_defs_project(false, true);

  // Default entry-driven checks and execution never reach the unreferenced definitions.
  assert_success(
    &run_calcit(&snapshot, &["--check-only"]),
    "default check ignores unreferenced definitions",
  );
  let reachable = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "json"]);
  assert_success(&reachable, "keep-going check stays entry-reachable");
  let reachable_report: serde_json::Value = serde_json::from_slice(&reachable.stdout).unwrap();
  assert_eq!(reachable_report["data"]["scope"], "reachable");
  assert!(!definition_statuses(&reachable_report).contains_key("app.main/unreferenced-bad"));
  assert_success(&run_calcit(&snapshot, &[]), "default run is unaffected");

  let all = run_calcit(&snapshot, &["--check-only", "--all-defs", "--format", "json"]);
  assert!(!all.status.success(), "all-defs must reject unreferenced type errors");
  let report: serde_json::Value = serde_json::from_slice(&all.stdout).expect("all-defs should emit one JSON envelope");
  assert_eq!(report["data"]["scope"], "all-defs");
  let statuses = definition_statuses(&report);
  assert_eq!(statuses["app.main/unreferenced-bad"], "failed");
  assert_eq!(statuses["app.main/unreferenced-ok"], "passed");
  assert_eq!(statuses["app.zeta/zeta-bad"], "failed");
  assert_eq!(statuses["app.alpha/alpha-user"], "blocked");
  assert_eq!(statuses["app.main/main!"], "passed");
  let bad = report["data"]["definitions"]
    .as_array()
    .unwrap()
    .iter()
    .find(|item| item["definition"] == "app.main/unreferenced-bad")
    .unwrap();
  assert_eq!(bad["diagnostics"][0]["code"], "E_FN_ARG_TYPE_MISMATCH");

  // The human report shares the keep-going Markdown shape and names the scope.
  let human = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
  assert!(!human.status.success());
  let human_stdout = String::from_utf8_lossy(&human.stdout);
  assert!(human_stdout.contains("- scope: all-defs"), "stdout:\n{human_stdout}");
  assert!(human_stdout.contains("## `app.main/unreferenced-bad`"), "stdout:\n{human_stdout}");
}

#[test]
fn all_defs_treats_calcit_prefixed_packages_as_project_code() {
  // Packages such as calcit.std share the `calcit.` prefix but are not bundled core:
  // their definitions are roots and edges, while calcit.core stays out of the graph.
  let build = |with_bad: bool| {
    let directory = TestDirectory::create();
    let snapshot = directory.snapshot();
    let fixture = fs::read_to_string("calcit/add.cirru").expect("minimal snapshot fixture should read");
    fs::write(
      &snapshot,
      fixture
        .replace(":package |app", ":package |calcit.demo")
        .replace("app.main", "calcit.demo.main"),
    )
    .expect("prefixed package snapshot should write");
    assert_success(
      &run_calcit(&snapshot, &["edit", "add-ns", "calcit.demo.util"]),
      "add prefixed namespace",
    );
    edit_def_at(&snapshot, "calcit.demo.util/util-ok", "quote $ defn util-ok () 2");
    edit_schema_at(&snapshot, "calcit.demo.util/util-ok", "Number");
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "add-import",
          "calcit.demo.main",
          "--code",
          "quote $ calcit.demo.util :refer $ util-ok",
        ],
      ),
      "import util-ok",
    );
    edit_def_at(&snapshot, "calcit.demo.main/main!", "quote $ defn main! () $ util-ok");
    edit_def_at(&snapshot, "calcit.demo.main/reload!", "quote $ defn reload! () nil");
    edit_schema_at(&snapshot, "calcit.demo.main/main!", "Number");
    edit_schema_at(&snapshot, "calcit.demo.main/reload!", "Nil");
    if with_bad {
      edit_def_at(&snapshot, "calcit.demo.util/util-bad", "quote $ defn util-bad () $ inc |x");
      edit_schema_at(&snapshot, "calcit.demo.util/util-bad", "Number");
      edit_def_at(&snapshot, "calcit.demo.util/util-user", "quote $ defn util-user () $ util-bad");
      edit_schema_at(&snapshot, "calcit.demo.util/util-user", "Number");
    }
    (directory, snapshot)
  };
  let args = ["--check-only", "--all-defs", "--format", "json"];

  let (_clean_directory, clean) = build(false);
  let output = run_calcit(&clean, &args);
  assert_success(&output, "a clean calcit.-prefixed package passes all-defs");
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  let statuses = definition_statuses(&report);
  assert!(
    statuses.keys().all(|definition| !definition.starts_with("calcit.core/")),
    "{statuses:?}"
  );
  assert_eq!(statuses["calcit.demo.util/util-ok"], "passed");
  assert_eq!(statuses["calcit.demo.main/main!"], "passed");

  let (_bad_directory, bad) = build(true);
  let output = run_calcit(&bad, &args);
  assert!(!output.status.success(), "prefixed package errors must still fail");
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  let statuses = definition_statuses(&report);
  assert!(
    statuses.keys().all(|definition| !definition.starts_with("calcit.core/")),
    "{statuses:?}"
  );
  assert_eq!(statuses["calcit.demo.util/util-bad"], "failed");
  assert_eq!(statuses["calcit.demo.util/util-user"], "blocked");
  assert_eq!(statuses["calcit.demo.util/util-ok"], "passed");
}

#[test]
fn all_defs_results_do_not_depend_on_definition_or_namespace_order() {
  let (_forward_directory, forward) = prepare_all_defs_project(false, true);
  let (_reversed_directory, reversed) = prepare_all_defs_project(true, true);
  let args = ["--check-only", "--all-defs", "--format", "json"];
  let first = run_calcit(&forward, &args);
  let second = run_calcit(&reversed, &args);
  assert!(!first.status.success() && !second.status.success());
  assert_eq!(
    String::from_utf8_lossy(&first.stdout),
    String::from_utf8_lossy(&second.stdout),
    "creation order of definitions and namespaces must not change the report"
  );
  assert_eq!(first.stdout, run_calcit(&forward, &args).stdout, "repeated runs must be identical");

  // A definition's verdict is its own: removing an unrelated failing definition leaves every other row unchanged.
  let (_clean_directory, without_bad) = prepare_all_defs_project(false, false);
  let full: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
  let partial_output = run_calcit(&without_bad, &args);
  let partial: serde_json::Value = serde_json::from_slice(&partial_output.stdout).unwrap();
  let mut expected = definition_statuses(&full);
  expected.remove("app.main/unreferenced-bad");
  assert_eq!(definition_statuses(&partial), expected);
}

#[test]
fn all_defs_requires_direct_check_only_and_rejects_incremental() {
  let (_directory, snapshot) = prepare_all_defs_project(false, false);
  for args in [
    vec!["--all-defs"],
    vec!["--check-only", "--all-defs", "--incremental"],
    vec!["--all-defs", "--check-only", "js"],
  ] {
    let output = run_calcit(&snapshot, &args);
    assert!(!output.status.success(), "{args:?} should be rejected");
    assert!(String::from_utf8_lossy(&output.stderr).contains("--all-defs"), "{args:?}");
  }
}

/// #2449: `.each` relates the receiver's elements to the callback exactly like `each`,
/// while a mismatched callback or an open receiver stays rejected.
#[test]
fn each_method_keeps_the_element_callback_relation() {
  let fixture = fs::read_to_string("tests/fixtures/method-each-contract.cirru").expect("each fixture should read");
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::write(&snapshot, &fixture).expect("each fixture should write");
  assert_success(
    &run_calcit(&snapshot, &["--check-only", "--all-defs"]),
    "typed element callbacks through .each",
  );

  let typed_callback = "hint-fn $ Fn app.main/Res (-> Unit)";
  let open_receiver = "hint-fn $ Fn (List app.main/Res) (List app.main/Res) (-> Unit)";
  for (label, source, code) in [
    (
      "callback for another element type",
      fixture
        .replace(typed_callback, "hint-fn $ Fn String (-> Unit)")
        .replace("assert |kind $ = (:kind r) :font", ", &unit"),
      "E_METHOD_ARG_TYPE_MISMATCH",
    ),
    (
      "open receiver",
      fixture
        .replace(open_receiver, "hint-fn $ Fn (List Dynamic) (List Dynamic) (-> Unit)")
        .replace(
          ":schema $ quote $ Fn (List app.main/Res) (List app.main/Res) (-> Unit)",
          ":schema $ quote $ Fn (List Dynamic) (List Dynamic) (-> Unit)",
        ),
      "E_CALL_ARGUMENT_UNPROVEN",
    ),
  ] {
    assert_ne!(source, fixture, "{label}: fixture edit should apply");
    fs::write(&snapshot, &source).expect("variant should write");
    let output = run_calcit(&snapshot, &["--check-only"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{label} should be rejected");
    assert!(stderr.contains(code) && stderr.contains("`.each`"), "{label}:\n{stderr}");
  }
}

#[test]
fn keep_going_and_all_defs_fail_on_an_option_erased_to_nil() {
  let (_directory, snapshot) = prepare_all_defs_project(false, false);
  edit_def_at(
    &snapshot,
    "app.main/erase-field",
    "quote $ defn erase-field (m) (hint-fn $ Fn (Map Tag Dynamic) (-> Dynamic)) (.unwrap-or (get m :a) nil)",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "app.main/erase-field",
        "--code",
        "quote $ Fn (Map Tag Dynamic) (-> Dynamic)",
      ],
    ),
    "edit erase-field schema",
  );
  // #2124: blocking since 0.36.0, so the erasure fails the definition that writes it.
  let all = run_calcit(&snapshot, &["--check-only", "--all-defs", "--format", "json"]);
  assert!(!all.status.success(), "an Option erased to nil must fail --all-defs");
  let report: serde_json::Value = serde_json::from_slice(&all.stdout).expect("all-defs should emit one JSON envelope");
  let erased = option_erased_diagnostics(&report);
  assert_eq!(erased.len(), 1, "{erased:?}");
  assert_eq!(erased[0]["severity"], "error");
  assert_eq!(erased[0]["definition"], "app.main/erase-field");
  assert!(!erased[0]["path"].as_array().unwrap().is_empty());
  let statuses = definition_statuses(&report);
  assert_eq!(statuses["app.main/erase-field"], "failed");
  let advisories = report["data"]["advisories"].as_array().expect("advisories should be an array");
  assert!(
    advisories.iter().all(|item| item["code"] != "E_OPTION_ERASED_TO_NIL"),
    "the error is not repeated as a note: {advisories:?}"
  );

  let human = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
  assert!(!human.status.success());
  let output = format!(
    "{}{}",
    String::from_utf8_lossy(&human.stdout),
    String::from_utf8_lossy(&human.stderr)
  );
  assert!(output.contains("E_OPTION_ERASED_TO_NIL"), "output:\n{output}");

  // The entry-reachable keep-going report never reaches erase-field.
  let reachable = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "json"]);
  assert_success(&reachable, "keep-going check stays entry-reachable");
  let reachable_report: serde_json::Value = serde_json::from_slice(&reachable.stdout).unwrap();
  assert!(option_erased_diagnostics(&reachable_report).is_empty());
}

#[test]
fn scoped_check_fails_on_an_option_erasure_reached_in_another_namespace() {
  // The erasure is written in a macro of `app.macros`, so the error sits there,
  // outside the `--ns app.main` scope; the scoped check must still fail.
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  assert_success(&run_calcit(&snapshot, &["config", "set", "target", "native"]), "set native target");
  assert_success(&run_calcit(&snapshot, &["edit", "add-ns", "app.macros"]), "create macro namespace");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.macros/read-field",
        "--code",
        "quote $ defmacro read-field (value field) $ quasiquote $ match (get ~value ~field) ((:some v) v) ((:none) nil)",
      ],
    ),
    "declare erasing macro",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "add-import", "app.main", "--code", "quote $ app.macros :refer $ read-field"],
    ),
    "import the macro",
  );
  edit_definition(&snapshot, "use-field", "quote $ defn use-field (m) $ read-field m :a", false);
  set_schema(&snapshot, "use-field", "quote $ Fn (Map Tag Dynamic) (-> Dynamic)");
  let output = run_calcit(&snapshot, &["--check-only", "--ns", "app.main"]);
  let report = format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(
    !output.status.success() && report.contains("macro `app.macros/read-field` turns an Option back into nil"),
    "the reached erasure must fail the scoped check:\n{report}"
  );
}

fn option_erased_diagnostics(report: &serde_json::Value) -> Vec<serde_json::Value> {
  report["data"]["definitions"]
    .as_array()
    .expect("definitions should be an array")
    .iter()
    .flat_map(|item| item["diagnostics"].as_array().cloned().unwrap_or_default())
    .filter(|item| item["code"] == "E_OPTION_ERASED_TO_NIL")
    .collect()
}

#[test]
fn option_erasure_written_in_a_macro_body_is_reported_once_on_the_macro() {
  let (_directory, snapshot) = prepare_all_defs_project(false, false);
  edit_def_at(
    &snapshot,
    "app.main/read-field",
    "quote $ defmacro read-field (value field) $ quasiquote $ match (get ~value ~field) ((:some v) v) ((:none) nil)",
  );
  edit_def_at(&snapshot, "app.main/wrap", "quote $ defmacro wrap (body) $ quasiquote ~body");
  // The branch is written in `outer` and only passed through `wrap`.
  edit_def_at(
    &snapshot,
    "app.main/outer",
    "quote $ defmacro outer (m) $ quasiquote $ wrap $ match (get ~m :d) ((:some v) v) ((:none) nil)",
  );
  for (name, code) in [
    ("use-a", "quote $ defn use-a (m) $ read-field m :a"),
    ("use-b", "quote $ defn use-b (m) $ read-field m :b"),
    (
      "use-wrapped",
      "quote $ defn use-wrapped (m) $ wrap $ match (get m :c) ((:some v) v) ((:none) nil)",
    ),
    ("use-outer", "quote $ defn use-outer (m) $ outer m"),
  ] {
    let target = format!("app.main/{name}");
    edit_def_at(&snapshot, &target, code);
    assert_success(
      &run_calcit(
        &snapshot,
        &["edit", "schema", &target, "--code", "quote $ Fn (Map Tag Dynamic) (-> Dynamic)"],
      ),
      "edit caller schema",
    );
  }

  let all = run_calcit(&snapshot, &["--check-only", "--all-defs", "--format", "json"]);
  let report: serde_json::Value = serde_json::from_slice(&all.stdout).expect("all-defs should emit one JSON envelope");
  let erased = option_erased_diagnostics(&report);
  let definitions = erased
    .iter()
    .map(|item| item["definition"].as_str().unwrap())
    .collect::<std::collections::BTreeSet<_>>();
  // Both expansions share one error on the macro body; the branch the caller
  // passed through `wrap` stays on the caller, and the one `outer` passes
  // through `wrap` stays on `outer`.
  assert_eq!(erased.len(), 3, "{erased:?}");
  assert_eq!(
    definitions,
    std::collections::BTreeSet::from(["app.main/outer", "app.main/read-field", "app.main/use-wrapped"])
  );
  let on_macro = erased.iter().find(|item| item["definition"] == "app.main/read-field").unwrap();
  // `defmacro read-field (value field) $ quasiquote $ match … ((:none) nil)`
  assert_eq!(on_macro["path"], serde_json::json!([3, 1, 3]));
  let message = on_macro["message"].as_str().unwrap();
  assert!(message.contains("in the body of macro `app.main/read-field`"), "{message}");
  assert!(message.contains("expanded at 2 call sites, first at app.main/use-"), "{message}");
}

#[test]
fn proven_trait_method_inside_get_survives_the_proof_audit() {
  // The direct callable generated for `.get-name` has no source code; the
  // default proof audit must reuse it rather than report it missing (#2598).
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/trait-method-in-get.cirru", &snapshot).unwrap();
  for args in [
    vec!["--check-only"],
    vec!["--check-only", "--all-defs"],
    vec!["--check-only", "--ns", "app.min"],
  ] {
    assert_success(&run_calcit(&snapshot, &args), &args.join(" "));
  }
}

#[test]
fn unrelated_warning_does_not_skip_the_proof_audit_of_other_definitions() {
  // `bad-call` has an ordinary argument warning; `bad-map` passes ordinary
  // checking but has no return proof. Both must be reported (#2591).
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/proof-audit-after-warning.cirru", &snapshot).unwrap();

  let output = run_calcit(&snapshot, &["--check-only", "--all-defs", "--format", "json"]);
  assert!(!output.status.success());
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  let status = |id: &str| {
    let row = report["data"]["definitions"]
      .as_array()
      .unwrap()
      .iter()
      .find(|row| row["definition"] == id)
      .unwrap_or_else(|| panic!("{id} missing: {report}"));
    let codes = row["diagnostics"]
      .as_array()
      .unwrap()
      .iter()
      .filter_map(|diagnostic| diagnostic["code"].as_str().map(str::to_owned))
      .collect::<Vec<_>>();
    (row["status"].as_str().unwrap().to_owned(), codes)
  };
  assert_eq!(
    status("app.proof/bad-call"),
    ("failed".to_owned(), vec!["E_FN_ARG_TYPE_MISMATCH".to_owned()])
  );
  assert_eq!(
    status("app.proof/bad-map"),
    ("failed".to_owned(), vec!["E_FN_RETURN_UNPROVEN".to_owned()])
  );
  assert_eq!(status("app.proof/take-string").0, "passed");

  let output = run_calcit(&snapshot, &["--check-only", "--ns", "app.proof"]);
  assert!(!output.status.success());
  let text = String::from_utf8_lossy(&output.stdout);
  assert!(text.contains("- [failed] app.proof/bad-map (fn)"), "{text}");
  assert!(text.contains("- [warning] app.proof/bad-call (fn)"), "{text}");
  assert!(text.contains("source definitions passed: 3;"), "{text}");
  // The audit re-checks dependencies, but the ordinary warning is reported once.
  assert_eq!(text.matches("arg 1 expects type `:string`").count(), 1, "{text}");
}

#[test]
fn referred_union_short_name_is_the_declared_type() {
  // `Tree` imported with `:refer` names `app.model/Tree` in decoder targets,
  // schemas and body hints alike (#2577).
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/referred-union-name.cirru", &snapshot).unwrap();
  for args in [vec!["--check-only"], vec!["--check-only", "--all-defs"]] {
    assert_success(&run_calcit(&snapshot, &args), &args.join(" "));
  }
}

#[test]
fn recursive_union_relation_terminates() {
  // #2687: relating two recursive unions used to recurse through their
  // members until the stack overflowed. The pair in progress is now assumed
  // to hold, so the check finishes and real mismatches are still reported.
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/recursive-union-relation.cirru", &snapshot).unwrap();
  let output = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
  let stdout = String::from_utf8_lossy(&output.stdout);
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(!stderr.contains("overflowed its stack"), "{stderr}");
  assert!(
    stderr.contains("1 failed, 0 blocked"),
    "only `narrow` should fail\nstdout:\n{stdout}\nstderr:\n{stderr}"
  );
  for (def, status) in [("accept", "PASSED"), ("accept-mutual", "PASSED"), ("narrow", "FAILED")] {
    let section = format!("## `demo.main/{def}`\n\n- status: **{status}**");
    assert!(stdout.contains(&section), "missing {section}\n{stdout}");
  }
}

#[test]
fn later_union_argument_widens_generic_binding() {
  // #2608: a later `Map Tag Scalar` sets the shared variable to the union,
  // so a narrower first literal is accepted in either argument order.
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  edit_definition(&snapshot, "Scalar", "quote $ defunion Scalar Number String", false);
  edit_definition(
    &snapshot,
    "merge-maps",
    "quote $ defn merge-maps (left right) (hint-fn $ Fn (Map Tag V) (Map Tag V) (-> (Map Tag V)) (:generics V)) (&merge left right)",
    false,
  );
  set_schema(
    &snapshot,
    "merge-maps",
    "quote $ Fn (Map Tag V) (Map Tag V) (-> (Map Tag V)) (:generics V)",
  );
  edit_definition(
    &snapshot,
    "same-ref",
    "quote $ defn same-ref (a b) (hint-fn $ Fn (Ref V) (Ref V) (-> Bool) (:generics V)) (identical? a b)",
    false,
  );
  set_schema(&snapshot, "same-ref", "quote $ Fn (Ref V) (Ref V) (-> Bool) (:generics V)");
  let body = |calls: &str| {
    format!(
      "quote $ defn main! () (hint-fn $ Fn (-> Unit)) (let ((overrides $ assert-type ({{}} (:color |red)) (Map Tag app.main/Scalar)) (words $ assert-type ({{}} (:a |x)) (Map Tag String)) (r1 $ assert-type (ref 1) (Ref Number)) (r2 $ assert-type (ref 2) (Ref app.main/Scalar))) {calls} &unit)"
    )
  };
  edit_definition(&snapshot, "reload!", "quote $ defn reload! () &unit", true);
  edit_schema(&snapshot, "reload!", "Unit");
  let check = |calls: &str| {
    edit_definition(&snapshot, "main!", &body(calls), true);
    edit_schema(&snapshot, "main!", "Unit");
    run_calcit(&snapshot, &["--check-only"])
  };

  for calls in [
    "(merge-maps ({} (:width 10)) overrides) (merge-maps overrides ({} (:width 10)))",
    "(&merge ({} (:width 10)) overrides) (&merge overrides ({} (:width 10)))",
  ] {
    assert_success(&check(calls), calls);
  }

  // Real mismatches and invariant references are still refused.
  for (calls, code, detail) in [
    ("(merge-maps ({} (:width 10)) words)", "E_FN_ARG_TYPE_MISMATCH", "map<tag, string>"),
    (
      "(&merge ({} (:width 10)) words)",
      "E_PROC_ARG_TYPE_MISMATCH",
      "in call at app.main/main!",
    ),
    ("(same-ref r1 r2)", "E_FN_ARG_TYPE_MISMATCH", "ref<"),
  ] {
    let output = check(calls);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{calls} must fail");
    assert!(stderr.contains(code) && stderr.contains(detail), "{calls} stderr:\n{stderr}");
  }
}

#[test]
fn incremental_check_reports_snapshot_level_diagnostics() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  // Outside the entry closure, so a warm cache stays warm after it changes.
  edit_definition(&snapshot, "spread", "quote $ defn spread (& xs) (count xs)", false);
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "schema", "app.main/spread", "--code", "quote $ Fn (& Number) (-> Number)"],
    ),
    "edit spread schema",
  );
  let output = run_calcit(&snapshot, &["--check-only", "--incremental"]);
  assert_success(&output, "cold incremental check");
  assert!(String::from_utf8_lossy(&output.stdout).contains("status=cold"));

  // 0.37.0 removed the bare `& T` rest spelling; the changed source misses the cache and fails to load.
  let source = fs::read_to_string(&snapshot).expect("snapshot should read");
  fs::write(&snapshot, source.replace("Fn (& Number) (-> Number)", "Fn & Number (-> Number)")).expect("snapshot should write");
  for run in ["first", "second"] {
    let output = run_calcit(&snapshot, &["--check-only", "--incremental"]);
    assert!(
      !output.status.success(),
      "{run} incremental check must reject the bare rest spelling"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
      stderr.contains("[E_LEGACY_TYPE_SYNTAX]") && stderr.contains("bare `& T`"),
      "{run} stderr:\n{stderr}"
    );
  }
  assert_success(
    &run_calcit(&snapshot, &["edit", "format"]),
    "edit format migrates the rest spelling",
  );
  assert!(
    fs::read_to_string(&snapshot)
      .expect("snapshot should read")
      .contains("Fn (& Number) (-> Number)")
  );

  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "schema", "app.main/spread", "--code", "quote $ Fn Fn (-> Number)"],
    ),
    "edit spread schema to a bare Fn",
  );
  let output = run_calcit(&snapshot, &["--check-only", "--incremental"]);
  assert!(!output.status.success(), "--incremental must fail on a bare broad type");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("[E_BROAD_TYPE] [Error]"), "stderr:\n{stderr}");
}

#[test]
fn string_type_slot_bindings_are_rejected_until_edit_format() {
  // 0.37.0 removed `:type-slots` bindings written as strings; `edit format` writes symbols.
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  let content = fs::read_to_string(&snapshot).expect("snapshot should be readable");
  assert!(
    content.contains(":type-slots $ {}\n"),
    "fixture should have an empty type-slot map:\n{content}"
  );
  fs::write(
    &snapshot,
    content.replacen(":type-slots $ {}\n", ":type-slots $ {} (:payload |app.main/main!)\n", 1),
  )
  .expect("snapshot should be writable");

  let output = run_calcit(&snapshot, &["--check-only"]);
  assert!(!output.status.success(), "a string type-slot binding must fail to load");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(
    stderr.contains("[E_LEGACY_TYPE_SYNTAX]") && stderr.contains("type slot `:payload` is bound with the string"),
    "stderr:\n{stderr}"
  );

  assert_success(
    &run_calcit(&snapshot, &["edit", "format"]),
    "edit format migrates the type-slot binding",
  );
  let content = fs::read_to_string(&snapshot).expect("snapshot should be readable");
  assert!(
    content.contains(":payload 'app.main/main!"),
    "edit format should write a symbol:\n{content}"
  );
}

#[test]
fn quoted_compound_types_are_rejected_until_edit_format() {
  // #2583: 0.37.0 rejects `quote $ <compound type>` in type-only positions at load,
  // tests included; `edit format` writes them bare.
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  edit_definition(
    &snapshot,
    "xs",
    "quote $ defn xs () (hint-fn $ Fn (-> (List Number))) (assert-type ([] 1) (List Number))",
    false,
  );
  set_schema(&snapshot, "xs", "quote $ Fn (-> (List Number))");
  let content = fs::read_to_string(&snapshot).expect("snapshot should be readable");
  let bare = "assert-type ([] 1) (List Number)";
  assert!(content.contains(bare), "edit should write the bare spelling:\n{content}");
  fs::write(&snapshot, content.replace(bare, "assert-type ([] 1) (quote (List Number))")).expect("snapshot should be writable");

  let output = run_calcit(&snapshot, &["--check-only"]);
  assert!(!output.status.success(), "a quoted compound type must fail to load");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(
    stderr.contains("[E_LEGACY_TYPE_SYNTAX] app.main/xs quotes the compound type `quote $ List Number`")
      && stderr.contains("Write `List Number` instead"),
    "stderr:\n{stderr}"
  );

  assert_success(&run_calcit(&snapshot, &["edit", "format"]), "edit format");
  assert!(fs::read_to_string(&snapshot).expect("snapshot should be readable").contains(bare));
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "check after edit format");
}

#[test]
fn dynamic_bound_type_slots_fail_the_check() {
  // 0.37.0: a type slot bound to `Dynamic` (or left unbound outside strict
  // project source) is an error, no longer the W_UNBOUND_TYPE_SLOT note.
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  edit_definition(&snapshot, "slotted", "quote $ defn slotted (op) (println op)", false);
  set_schema(&snapshot, "slotted", "quote $ Fn *dispatch-op (-> Unit)");
  edit_definition(&snapshot, "main!", "quote $ defn main! () (slotted 1)", true);
  set_schema(&snapshot, "main!", "quote $ Fn (-> Unit)");
  assert_success(
    &run_calcit(&snapshot, &["config", "set-type-slot", "dispatch-op", ":dynamic"]),
    "bind the slot to Dynamic",
  );
  for args in [&["--check-only"][..], &["--check-only", "--incremental"][..], &[][..]] {
    let output = run_calcit(&snapshot, args);
    assert!(!output.status.success(), "{args:?} must fail on a Dynamic-bound slot");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
      stderr.contains("[E_UNBOUND_TYPE_SLOT] app.main/slotted uses type slot `*dispatch-op`")
        && stderr.contains("Found 1 use(s) of type slots")
        && !stderr.contains("W_UNBOUND_TYPE_SLOT"),
      "{args:?} stderr:\n{stderr}"
    );
  }
  // Code generation without the preflight and the scoped namespace check
  // must refuse the open slot too, and so must the test runner.
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        "app.main/slotted",
        "slot-use",
        "--input-format",
        "cirru",
        "--code",
        "quote $ slotted 1",
      ],
    ),
    "add a test that uses the slot",
  );
  for args in [
    &["test"][..],
    &["--no-proof-audit", "js"][..],
    &["ir"][..],
    &["--check-only", "--ns", "app.main"][..],
  ] {
    if args[0] == "--check-only" {
      assert_success(&run_calcit(&snapshot, &["config", "set", "target", "native"]), "set native target");
    }
    let output = run_calcit(&snapshot, args);
    let report = format!(
      "{}{}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
    assert!(
      !output.status.success() && report.contains("uses type slot `*dispatch-op`"),
      "{args:?} must fail on a Dynamic-bound slot:\n{report}"
    );
  }
  assert_success(
    &run_calcit(&snapshot, &["edit", "def", "app.main/Op", "--code", "quote $ defenum Op (:ping)"]),
    "add a named slot type",
  );
  assert_success(
    &run_calcit(&snapshot, &["config", "set-type-slot", "dispatch-op", "app.main/Op"]),
    "bind the slot to a named type",
  );
  edit_definition(&snapshot, "main!", "quote $ defn main! () (slotted $ %:: Op :ping)", true);
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "check with a named slot binding");
}

#[test]
fn bare_broad_types_fail_the_check_and_block_dependents() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  edit_definition(&snapshot, "broad", "quote $ defn broad (f) (f 1)", false);
  edit_definition(&snapshot, "user", "quote $ defn user () $ broad inc", false);
  edit_definition(&snapshot, "main!", "quote $ defn main! () $ do (user) nil", true);
  edit_definition(&snapshot, "reload!", "quote $ defn reload! () nil", true);
  let set_broad_schema = |schema: &str| {
    assert_success(
      &run_calcit(&snapshot, &["edit", "schema", "app.main/broad", "--code", schema]),
      "edit broad schema",
    );
  };
  set_broad_schema("quote $ Fn Fn (-> Dynamic)");
  edit_schema(&snapshot, "user", "Dynamic");
  edit_schema(&snapshot, "main!", "Nil");
  edit_schema(&snapshot, "reload!", "Nil");

  // Fail-fast check and the run preflight both refuse the bare `Fn`.
  for args in [&["--check-only"][..], &[][..]] {
    let output = run_calcit(&snapshot, args);
    assert!(!output.status.success(), "{args:?} must fail on a bare broad type");
    assert!(
      String::from_utf8_lossy(&output.stderr).contains("[E_BROAD_TYPE] [Error]"),
      "{args:?} stderr:\n{}",
      String::from_utf8_lossy(&output.stderr)
    );
  }

  let output = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "json"]);
  assert!(!output.status.success(), "the strict check must fail");
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON envelope");
  let by_name = report["data"]["definitions"]
    .as_array()
    .expect("definitions should be an array")
    .iter()
    .map(|item| (item["definition"].as_str().expect("definition should be text"), item))
    .collect::<std::collections::HashMap<_, _>>();
  assert_eq!(by_name["app.main/broad"]["status"], "failed");
  assert_eq!(by_name["app.main/broad"]["diagnostics"][0]["code"], "E_BROAD_TYPE");
  assert_eq!(by_name["app.main/broad"]["diagnostics"][0]["severity"], "error");
  assert_eq!(by_name["app.main/user"]["status"], "blocked");
  assert_eq!(by_name["app.main/user"]["blocked_by"], serde_json::json!(["app.main/broad"]));
  assert_eq!(by_name["app.main/main!"]["status"], "blocked");
  assert_eq!(by_name["app.main/main!"]["blocked_by"], serde_json::json!(["app.main/user"]));
  assert_eq!(by_name["app.main/reload!"]["status"], "passed");

  // `AnyFn` is the same type, written on purpose with a reviewed reason (#2635).
  set_broad_schema("quote $ Fn AnyFn (-> Dynamic) (:open :reflect)");
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "AnyFn check");
  assert_success(
    &run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "json"]),
    "AnyFn strict check",
  );

  // Without the reason the open family is an error since 0.39.0 (#2635).
  set_broad_schema("quote $ Fn AnyFn (-> Dynamic)");
  let output = run_calcit(&snapshot, &["--check-only"]);
  assert!(!output.status.success(), "an unreasoned AnyFn must fail the check");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("[E_OPEN_WITHOUT_REASON] [Error]"), "stderr:\n{stderr}");
  let output = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "json"]);
  assert!(!output.status.success(), "an unreasoned AnyFn must fail the strict check");
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("strict check json");
  let results = report["data"]["definitions"].as_array().expect("strict check definitions");
  let broad = results
    .iter()
    .find(|row| row["definition"] == "app.main/broad")
    .expect("broad definition row");
  assert_eq!(broad["status"], "failed");
  assert_eq!(broad["diagnostics"][0]["code"], "E_OPEN_WITHOUT_REASON");
}

#[test]
fn lambdas_in_return_position_and_bound_once_take_the_expected_callable_type() {
  // #2637: the declared `Fn` result types a lambda written as the last body
  // form (through `let` bodies and `if` branches), and a lambda bound once by
  // `let` takes the callback type of its only use. `dup-macro` passes that
  // use to a macro that copies it into both `if` branches.
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/contextual-lambda-positions.cirru", &snapshot).unwrap();
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "inferred lambdas check without hints");
  let run = run_calcit(&snapshot, &[]);
  assert_success(&run, "inferred lambdas run");
  assert!(
    String::from_utf8_lossy(&run.stdout).contains("3 abc? 0 ([] \"|abc!\") 1 3 ([] \"|abc!\")"),
    "{}",
    String::from_utf8_lossy(&run.stdout)
  );

  // The same positions with a body that cannot accept `Task` now fail, which
  // shows each parameter is typed rather than left open.
  fs::copy("tests/fixtures/contextual-lambda-mismatch.cirru", &snapshot).unwrap();
  let check = run_calcit(&snapshot, &["--check-only"]);
  assert!(!check.status.success(), "typed parameters reject `&+` on a Task");
  let report = format!(
    "{}{}",
    String::from_utf8_lossy(&check.stdout),
    String::from_utf8_lossy(&check.stderr)
  );
  // `hint-after` returns the lambda before a trailing `hint-fn`;
  // `called-once` keeps the per-call re-check of a directly called binding.
  for def in [
    "tail-lambda",
    "let-tail",
    "if-tail",
    "bound-once",
    "bound-returned",
    "hint-after",
    "called-once",
  ] {
    assert!(
      report.contains(&format!("got `'demo.main/Task` in call at demo.main/{def}")),
      "{def} should see a Task parameter:\n{report}"
    );
  }
}
