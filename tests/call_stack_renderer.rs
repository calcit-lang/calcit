use calcit::Calcit;
use calcit::call_stack::{CallStackList, StackKind, display_stack_with_docs};
use calcit::project_state::{ERROR_STATE_FILE, set_active_project_directory_from_snapshot, state_file};
use std::fs;
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
