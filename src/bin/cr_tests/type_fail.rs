use super::*;
use std::cell::RefCell;
use std::fs;
use std::sync::{LazyLock, Mutex};

static FIXTURE_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn load_fixture_entries(path: &str) -> ProgramEntries {
  builtins::effects::init_effects_states();

  let content = fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read fixture: {path}"));
  let data = cirru_edn::parse(&content).unwrap_or_else(|e| panic!("Failed to parse fixture {path}: {e}"));
  let mut snapshot = snapshot::load_snapshot_data(&data, path).unwrap_or_else(|e| panic!("Failed to load fixture {path}: {e}"));
  let core_snapshot = calcit::load_core_snapshot().expect("load core snapshot");

  for (k, v) in core_snapshot.files {
    snapshot.files.insert(k.to_owned(), v.to_owned());
  }

  {
    let mut prgm = program::PROGRAM_CODE_DATA.write().expect("open program data");
    *prgm = program::extract_program_data(&snapshot).expect("extract program data");
  }

  let config_init = snapshot.configs.init_fn.to_string();
  let config_reload = snapshot.configs.reload_fn.to_string();
  let (init_ns, init_def) = util::string::extract_ns_def(&config_init).expect("extract init ns/def");
  let (reload_ns, reload_def) = util::string::extract_ns_def(&config_reload).expect("extract reload ns/def");

  program::clear_runtime_caches_for_reload(init_ns.clone().into(), reload_ns.clone().into(), true).expect("clear runtime caches");

  let warmup_warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
  runner::preprocess::ensure_ns_def_compiled(
    calcit::calcit::CORE_NS,
    calcit::calcit::BUILTIN_IMPLS_ENTRY,
    &warmup_warnings,
    &CallStackList::default(),
  )
  .expect("preprocess builtin impls");

  ProgramEntries {
    init_fn: Arc::from(config_init),
    reload_fn: Arc::from(config_reload),
    init_def: init_def.into(),
    init_ns: init_ns.into(),
    reload_ns: reload_ns.into(),
    reload_def: reload_def.into(),
  }
}

fn lock_fixture_tests() -> std::sync::MutexGuard<'static, ()> {
  FIXTURE_TEST_LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

#[test]
fn type_fail_schema_mismatch_fixtures_report_error_code() {
  let _guard = lock_fixture_tests();

  let fixtures = [
    (
      "calcit/type-fail/schema-required-arity.cirru",
      "schema has 2 required arg(s) but code has 1",
    ),
    (
      "calcit/type-fail/schema-rest-missing.cirru",
      "code has & rest param but schema has no :rest",
    ),
    (
      "calcit/type-fail/schema-rest-unexpected.cirru",
      "schema has :rest but code has no & param",
    ),
    (
      "calcit/type-fail/schema-kind-mismatch.cirru",
      "schema :kind is :macro but code uses defn",
    ),
  ];

  for (path, expected_msg) in fixtures {
    let entries = load_fixture_entries(path);
    let err = run_check_only(&entries).expect_err("fixture should fail during check-only");

    assert!(
      err.contains("E_SCHEMA_DEF_MISMATCH"),
      "fixture {path} should surface schema error code, got: {err}"
    );
    assert!(err.contains(expected_msg), "fixture {path} msg was: {err}");
  }
}

#[test]
fn type_fail_call_arg_fixture_reports_warning_code() {
  let _guard = lock_fixture_tests();
  let entries = load_fixture_entries("calcit/type-fail/schema-call-arg-type-mismatch.cirru");
  let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

  runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
    .expect("call-arg fixture should preprocess with warnings, not hard errors");

  let warnings = warnings.borrow();
  assert_eq!(warnings.len(), 1, "expected exactly one warning, got: {warnings:?}");
  let warning = &warnings[0];
  assert_eq!(warning.code(), Some("W_FN_ARG_TYPE_MISMATCH"));
  assert!(
    warning.message().contains("expects type `:number`, but got `:string`"),
    "warning message was: {}",
    warning.message()
  );
}

#[test]
fn run_check_only_surfaces_schema_error_code() {
  let _guard = lock_fixture_tests();
  let entries = load_fixture_entries("calcit/type-fail/schema-required-arity.cirru");
  let err = run_check_only(&entries).expect_err("check-only should fail on schema mismatch fixture");

  assert!(
    err.contains("E_SCHEMA_DEF_MISMATCH"),
    "check-only error should contain code, got: {err}"
  );
}
