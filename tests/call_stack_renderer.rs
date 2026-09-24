use calcit::Calcit;
use calcit::call_stack::{CallStackList, StackKind, display_stack_with_docs};
use calcit::project_state::{ERROR_STATE_FILE, set_active_project_directory_from_snapshot, state_file};
use std::fs;

#[test]
fn non_edn_stack_argument_preserves_the_original_failure_artifact() {
  let project = tempfile::tempdir().expect("temporary project should create");
  let snapshot = project.path().join("calcit.cirru");
  set_active_project_directory_from_snapshot(snapshot.to_str().expect("temporary path should be UTF-8"));

  let stack = CallStackList::default().extend_owned("app.test", "main!", StackKind::Fn, Calcit::Nil, vec![Calcit::Unit]);
  display_stack_with_docs("[E_TEST] original compiler failure", &stack, None, None)
    .expect("non-EDN stack arguments must not mask the original failure");

  let content = fs::read_to_string(state_file(project.path(), ERROR_STATE_FILE)).expect("error artifact should be written");
  cirru_edn::parse(&content).expect("error artifact should contain valid Cirru EDN");
  assert!(content.contains("[E_TEST] original compiler failure"));
  assert!(content.contains("<non-EDN stack argument: not able to generate EDN: Unit>"));
}
