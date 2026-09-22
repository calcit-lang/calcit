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
    let path = std::env::temp_dir().join(format!("calcit-js-global-shadow-{}-{nonce}-{counter}", std::process::id()));
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

fn run_calcit(args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .args(args)
    .output()
    .expect("calcit command should run")
}

/// `js/Element` reads a host global. A same-name `:refer` import of a Calcit
/// schema must not shadow it in the generated JavaScript module.
#[test]
fn js_host_global_survives_a_same_name_schema_import() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  let emit_path = directory.path().join("js-out");
  fs::copy("tests/fixtures/js-global-shadow.cirru", &snapshot).expect("fixture should copy");

  let output = run_calcit(&[
    "--emit-path",
    emit_path.to_str().expect("emit path should be UTF-8"),
    snapshot.to_str().expect("snapshot path should be UTF-8"),
    "js",
  ]);
  assert!(
    output.status.success(),
    "js codegen failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );

  let generated = fs::read_to_string(emit_path.join("js-global-shadow.main.mjs")).expect("generated module should exist");
  assert!(
    generated.contains("import { Element } from"),
    "fixture must generate a shadowing import binding:\n{generated}"
  );
  assert!(
    generated.contains("globalThis.Element"),
    "js/Element must be qualified as a host global:\n{generated}"
  );
  assert!(
    generated.contains("typeof globalThis.Element !== 'undefined'") && generated.contains("return globalThis.Element"),
    "both exists? and the value read must use the host global:\n{generated}"
  );
}
