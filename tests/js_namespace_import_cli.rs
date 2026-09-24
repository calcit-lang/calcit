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
    let path = std::env::temp_dir().join(format!("calcit-js-namespace-import-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&path).expect("temporary project should create");
    Self(path)
  }

  fn path(&self) -> &Path {
    &self.0
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

fn run_calcit(snapshot: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
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
    "{context} failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

fn assert_generated_js_runs(project_path: &Path, emit_path: &Path) {
  let script = r#"
import { symlink } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const projectPath = process.env.CALCIT_JS_PROJECT_PATH;
const emitPath = process.env.CALCIT_JS_EMIT_PATH;
await symlink(resolve("node_modules"), join(projectPath, "node_modules"), process.platform === "win32" ? "junction" : "dir");
const generated = await import(pathToFileURL(join(emitPath, "app.main.mjs")).href);
await generated.main_$x_();
"#;
  let output = Command::new("node")
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .arg("--input-type=module")
    .arg("--eval")
    .arg(script)
    .env("CALCIT_JS_PROJECT_PATH", project_path)
    .env("CALCIT_JS_EMIT_PATH", emit_path)
    .output()
    .expect("node command should run");
  assert_success(&output, "generated JavaScript execution");
}

#[test]
fn nominal_decoder_and_source_alias_share_one_js_namespace_binding() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("calcit/fibo.cirru", &snapshot).expect("project fixture should copy");

  for (args, context) in [
    (vec!["edit", "add-ns", "app.model"], "model namespace"),
    (
      vec![
        "edit",
        "def",
        "app.model/Thing",
        "--code",
        "quote $ defstruct Thing (:name 'String)",
      ],
      "nominal struct",
    ),
    (
      vec![
        "edit",
        "def",
        "app.model/sample",
        "--code",
        "quote $ def sample $ %{} Thing (:name |Ada)",
      ],
      "aliased value",
    ),
    (
      vec!["edit", "add-import", "app.main", "--code", "quote $ app.model :as model"],
      "source namespace alias",
    ),
    (
      vec![
        "edit",
        "def",
        "app.main/probe",
        "--code",
        "quote $ defn probe () (println model/sample) (parse-cirru-edn-as \"|%{} :Thing (:name |Ada)\" app.model/Thing)",
      ],
      "value and nominal decoder references",
    ),
    (
      vec![
        "edit",
        "schema",
        "app.main/probe",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'app.model/Thing)",
      ],
      "probe schema",
    ),
    (
      vec![
        "edit",
        "def",
        "app.main/main!",
        "--overwrite",
        "--code",
        "quote $ defn main! () $ println $ probe",
      ],
      "reachable entry",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  assert_success(&run_calcit(&snapshot, &["--check-only"]), "strict entry check");
  let emit_path = directory.path().join("js-out");
  assert_success(
    &run_calcit(
      &snapshot,
      &["--emit-path", emit_path.to_str().expect("emit path should be UTF-8"), "js"],
    ),
    "JS emission",
  );

  let generated = fs::read_to_string(emit_path.join("app.main.mjs")).expect("generated module should exist");
  let import = "import * as $app_DOT_model from \"./app.model.mjs\";";
  assert_eq!(
    generated.matches(import).count(),
    1,
    "namespace binding must be unique:\n{generated}"
  );
  assert!(generated.contains("$app_DOT_model.sample"), "value alias must remain:\n{generated}");
  assert!(
    generated.contains("nominal:$app_DOT_model.Thing"),
    "nominal decoder must remain:\n{generated}"
  );
  if std::env::var_os("CALCIT_JS_NAMESPACE_IMPORT_RUNTIME").is_some() {
    assert_generated_js_runs(directory.path(), &emit_path);
  }
}
