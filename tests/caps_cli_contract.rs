use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDir(PathBuf);

impl TestDir {
  fn new(label: &str) -> Self {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock after epoch").as_nanos();
    let path = std::env::temp_dir().join(format!("calcit-caps-contract-{label}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&path).expect("create caps contract test directory");
    Self(path)
  }

  fn path(&self) -> &Path {
    &self.0
  }
}

impl Drop for TestDir {
  fn drop(&mut self) {
    if let Err(error) = fs::remove_dir_all(&self.0) {
      eprintln!("failed to remove caps contract test directory {}: {error}", self.0.display());
    }
  }
}

fn run_caps(args: &[&str], modules_dir: &Path) -> Output {
  Command::new(env!("CARGO_BIN_EXE_caps"))
    .args(args)
    .env("CALCIT_MODULES_DIR", modules_dir)
    .output()
    .expect("run caps")
}

#[test]
fn top_level_help_keeps_the_public_command_surface() {
  let test_dir = TestDir::new("help");
  let output = run_caps(&["--help"], &test_dir.path().join("modules"));
  assert!(output.status.success());
  let stdout = String::from_utf8(output.stdout).expect("UTF-8 help output");
  for command in [
    "outdated", "upgrade", "download", "add", "remove", "tree", "why", "version", "status", "verify", "reset", "clean",
  ] {
    assert!(
      stdout.lines().any(|line| line.trim_start().starts_with(command)),
      "missing command `{command}` in:\n{stdout}"
    );
  }
}

#[test]
fn version_get_reads_the_explicit_deps_file_without_mutation() {
  let test_dir = TestDir::new("version-get");
  let deps_file = test_dir.path().join("deps.cirru");
  let source = "{} (:version |1.2.3) (:dependencies $ {})\n";
  fs::write(&deps_file, source).expect("write deps.cirru");

  let output = run_caps(
    &[deps_file.to_str().expect("UTF-8 temporary path"), "version", "get"],
    &test_dir.path().join("modules"),
  );
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  assert_eq!(String::from_utf8(output.stdout).expect("UTF-8 version output"), "1.2.3\n");
  assert_eq!(fs::read_to_string(deps_file).expect("read unchanged deps.cirru"), source);
}

#[test]
fn missing_explicit_deps_file_is_a_failure() {
  let test_dir = TestDir::new("missing-input");
  let deps_file = test_dir.path().join("missing.cirru");
  let output = run_caps(
    &[deps_file.to_str().expect("UTF-8 temporary path"), "version", "get"],
    &test_dir.path().join("modules"),
  );
  assert!(!output.status.success());
  assert!(
    String::from_utf8(output.stderr)
      .expect("UTF-8 missing-file error")
      .contains(&format!("Error: no {} found!", deps_file.display()))
  );
}
