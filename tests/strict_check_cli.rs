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
fn immutable_value_schemas_reject_contradictions_before_codegen() {
  for (target, schema) in [
    ("app.values/initial-state", "quote $ :: 'Map 'Tag 'String"),
    ("app.main/initial-state", "quote $ :: 'Map 'Tag 'String"),
    ("app.main/state-alias", "quote $ :: 'Map 'Tag 'String"),
    ("app.reader/state-alias", "quote $ :: 'Map 'Tag 'String"),
    ("app.main/members", "quote $ :: 'Map 'String 'String"),
    ("app.main/answer", "quote $ :: 'String"),
    ("app.main/label", "quote $ :: 'Number"),
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
    &format!("quote $ defn lookup (name) $ case-default name -1 {patterns}"),
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

fn edit_schema(snapshot: &Path, name: &str, return_type: &str) {
  let target = format!("app.main/{name}");
  let code = format!("quote $ :: 'Fn $ {{}} (:args $ []) (:return '{return_type})");
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
  edit_definition(&snapshot, "reload!", "quote $ defn reload! () nil", false);
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
        "quote $ :: 'Fn $ {} (:args $ [] 'String 'Number) (:return 'Number)",
      ],
    ),
    "declare typed recur contract",
  );
  edit_definition(&snapshot, "main!", "quote $ defn main! () $ typed-recur |point 0", true);
  edit_schema(&snapshot, "main!", "Number");
  edit_definition(&snapshot, "reload!", "quote $ defn reload! () &unit", false);
  edit_schema(&snapshot, "reload!", "Unit");

  assert_success(&run_calcit(&snapshot, &["--check-only"]), "correct recur argument order");
  assert_success(
    &run_calcit(&snapshot, &["analyze", "check-public", "--ns", "app.main", "--format", "json"]),
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
    .filter(|diagnostic| diagnostic["code"] == "W_RECUR_ARG_TYPE_MISMATCH")
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

  let public = run_calcit(&snapshot, &["analyze", "check-public", "--ns", "app.main", "--format", "json"]);
  assert!(!public.status.success(), "public analysis must reject wrong recur arguments");
  let report: serde_json::Value = serde_json::from_slice(&public.stdout).expect("public check should return JSON");
  let recur_diagnostics = report["diagnostics"]
    .as_array()
    .expect("diagnostics should be an array")
    .iter()
    .filter(|diagnostic| diagnostic["code"] == "W_RECUR_ARG_TYPE_MISMATCH")
    .collect::<Vec<_>>();
  assert_eq!(recur_diagnostics.len(), 2, "report: {report}");
  assert_eq!(recur_diagnostics[0]["location"]["def"], "typed-recur");
  assert_eq!(recur_diagnostics[0]["location"]["coord"], serde_json::json!([3, 3, 1, 1]));
  assert_eq!(recur_diagnostics[1]["location"]["coord"], serde_json::json!([3, 3, 2]));
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
  let code = format!("quote $ :: 'Fn $ {{}} (:args $ []) (:return '{return_type})");
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
  assert_eq!(bad["diagnostics"][0]["code"], "W_FN_ARG_TYPE_MISMATCH");

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
