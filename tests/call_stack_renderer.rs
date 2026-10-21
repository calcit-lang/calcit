use calcit::Calcit;
use calcit::call_stack::{CallStackList, StackKind, display_stack_with_docs};
use calcit::project_state::{ERROR_STATE_FILE, set_active_project_directory_from_snapshot, state_file};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::{Mutex, OnceLock};

// The active project directory is set once per process, so every test shares one
// temporary project and writes its error artifact one at a time.
static PROJECT: OnceLock<tempfile::TempDir> = OnceLock::new();
static ARTIFACT: Mutex<()> = Mutex::new(());

fn render_error_artifact(failure: &str, stack: &CallStackList, expectation: &str) -> String {
  let _guard = ARTIFACT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
  let project = PROJECT.get_or_init(|| {
    let project = tempfile::tempdir().expect("temporary project should create");
    let snapshot = project.path().join("calcit.cirru");
    set_active_project_directory_from_snapshot(snapshot.to_str().expect("temporary path should be UTF-8"));
    project
  });
  display_stack_with_docs(failure, stack, None, None).expect(expectation);
  let content = fs::read_to_string(state_file(project.path(), ERROR_STATE_FILE)).expect("error artifact should be written");
  cirru_edn::parse(&content).expect("error artifact should contain valid Cirru EDN");
  content
}

#[test]
fn non_edn_stack_argument_preserves_the_original_failure_artifact() {
  let stack = CallStackList::default().extend_owned("app.test", "main!", StackKind::Fn, Calcit::Nil, vec![Calcit::Unit]);
  let content = render_error_artifact(
    "[E_TEST] original compiler failure",
    &stack,
    "non-EDN stack arguments must not mask the original failure",
  );
  assert!(content.contains("[E_TEST] original compiler failure"));
  assert!(content.contains("<non-EDN stack argument: not able to generate EDN: Unit>"));
}

#[test]
fn stack_code_without_cirru_form_preserves_the_original_failure_artifact() {
  // A preprocessed hint-fn keeps its schema as a Map value, which has no Cirru form (#1751).
  let mut schema = rpds::HashTrieMap::new_sync();
  schema.insert_mut(Calcit::tag("kind"), Calcit::tag("fn"));
  let code = Calcit::from(vec![Calcit::Str("hint-fn".into()), Calcit::Map(schema)]);
  let stack = CallStackList::default().extend_owned("app.test", "main!", StackKind::Fn, code, vec![]);
  let content = render_error_artifact(
    "[E_TEST] schema-hinted closure failure",
    &stack,
    "stack code without a Cirru form must not mask the original failure",
  );
  assert!(content.contains("[E_TEST] schema-hinted closure failure"));
  assert!(content.contains("hint-fn"));
}

fn query_error(directory: &Path, snapshot: &str) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .current_dir(directory)
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none", snapshot, "query", "error"])
    .output()
    .expect("error query should run")
}

fn assert_query_success(output: &Output) -> String {
  assert!(
    output.status.success(),
    "error query failed:\nstdout: {}\nstderr: {}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout.clone()).expect("diagnostic output should be UTF-8")
}

#[test]
fn legacy_error_query_is_read_only_and_relative_to_the_selected_snapshot() {
  let directory = tempfile::tempdir().unwrap();
  let project = directory.path().join("nested");
  fs::create_dir(&project).unwrap();
  let legacy = project.join(".calcit-error.cirru");
  let content = "{} (:message \"|synthetic legacy error\")\n  :stack $ []\n";
  fs::write(&legacy, content).unwrap();
  let modified = fs::metadata(&legacy).unwrap().modified().unwrap();

  for _ in 0..2 {
    let output = query_error(directory.path(), "nested/calcit.cirru");
    assert!(assert_query_success(&output).contains(content));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("without moving it"), "{stderr}");
    assert!(!stderr.contains("Moved legacy"), "{stderr}");
    assert_eq!(fs::read_to_string(&legacy).unwrap(), content);
    assert_eq!(fs::metadata(&legacy).unwrap().modified().unwrap(), modified);
    assert!(!project.join(".calcit").exists());
    assert!(!directory.path().join(".calcit").exists());
    assert!(!project.join("calcit.cirru").exists());
  }
}

#[test]
fn current_error_state_wins_even_when_empty_or_not_edn() {
  let directory = tempfile::tempdir().unwrap();
  let current = state_file(directory.path(), ERROR_STATE_FILE);
  fs::create_dir(current.parent().unwrap()).unwrap();
  let legacy = directory.path().join(".calcit-error.cirru");
  fs::write(&legacy, "old diagnostic must not appear").unwrap();

  // The error view displays text, rather than parsing or rewriting its contents.
  for content in ["new diagnostic (not EDN", ""] {
    fs::write(&current, content).unwrap();
    let output = query_error(directory.path(), "calcit.cirru");
    let stdout = assert_query_success(&output);
    assert!(!stdout.contains("old diagnostic"), "{stdout}");
    if content.is_empty() {
      assert!(stdout.contains("Error file is empty"), "{stdout}");
    } else {
      assert!(stdout.contains(content), "{stdout}");
    }
    assert!(!String::from_utf8_lossy(&output.stderr).contains("legacy error state"));
    assert_eq!(fs::read_to_string(&current).unwrap(), content);
    assert_eq!(fs::read_to_string(&legacy).unwrap(), "old diagnostic must not appear");
  }
}

#[test]
fn missing_error_state_remains_a_successful_read_without_creating_files() {
  let directory = tempfile::tempdir().unwrap();
  let output = query_error(directory.path(), "calcit.cirru");
  assert!(assert_query_success(&output).contains("No .calcit/error.cirru file found."));
  assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn legacy_error_age_warning_names_the_actual_file() {
  let directory = tempfile::tempdir().unwrap();
  let legacy = directory.path().join(".calcit-error.cirru");
  fs::write(&legacy, "older diagnostic").unwrap();
  let modified = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
  fs::File::options()
    .write(true)
    .open(&legacy)
    .unwrap()
    .set_modified(modified)
    .unwrap();
  let output = query_error(directory.path(), "calcit.cirru");
  let stdout = assert_query_success(&output);
  assert!(stdout.contains("Warning: ./.calcit-error.cirru was modified"), "{stdout}");
  assert!(stdout.contains("older diagnostic"), "{stdout}");
  assert_eq!(fs::metadata(&legacy).unwrap().modified().unwrap(), modified);
  assert!(!directory.path().join(".calcit").exists());
}

#[test]
fn unreadable_current_error_state_does_not_fall_back_to_legacy() {
  let directory = tempfile::tempdir().unwrap();
  let current = state_file(directory.path(), ERROR_STATE_FILE);
  fs::create_dir(current.parent().unwrap()).unwrap();
  let legacy = directory.path().join(".calcit-error.cirru");
  fs::write(&legacy, "old diagnostic must not appear").unwrap();

  // Invalid UTF-8 and a directory both fail reads, including when run as root.
  fs::write(&current, [0xff]).unwrap();
  for is_directory in [false, true] {
    if is_directory {
      fs::remove_file(&current).unwrap();
      fs::create_dir(&current).unwrap();
    }
    let output = query_error(directory.path(), "calcit.cirru");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Failed to read error file"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("old diagnostic"));
    assert_eq!(fs::read_to_string(&legacy).unwrap(), "old diagnostic must not appear");
  }
}

#[cfg(unix)]
#[test]
fn broken_current_error_symlink_does_not_fall_back_to_legacy() {
  let directory = tempfile::tempdir().unwrap();
  let current = state_file(directory.path(), ERROR_STATE_FILE);
  fs::create_dir(current.parent().unwrap()).unwrap();
  std::os::unix::fs::symlink("missing-diagnostic", &current).unwrap();
  fs::write(directory.path().join(".calcit-error.cirru"), "stale diagnostic").unwrap();
  let output = query_error(directory.path(), "calcit.cirru");
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("Failed to get metadata of error file"));
  assert!(!String::from_utf8_lossy(&output.stdout).contains("stale diagnostic"));
  assert!(fs::symlink_metadata(&current).unwrap().is_symlink());
}

#[cfg(unix)]
#[test]
fn legacy_error_query_works_in_a_read_only_directory() {
  use std::os::unix::fs::PermissionsExt;

  let directory = tempfile::tempdir().unwrap();
  let legacy = directory.path().join(".calcit-error.cirru");
  fs::write(&legacy, "read-only legacy diagnostic").unwrap();
  let original_permissions = fs::metadata(directory.path()).unwrap().permissions();
  fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o555)).unwrap();
  let output = query_error(directory.path(), "calcit.cirru");
  fs::set_permissions(directory.path(), original_permissions).unwrap();

  assert!(assert_query_success(&output).contains("read-only legacy diagnostic"));
  assert_eq!(fs::read_to_string(&legacy).unwrap(), "read-only legacy diagnostic");
  assert!(!directory.path().join(".calcit").exists());
}
