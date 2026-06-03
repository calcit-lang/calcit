use super::*;
use std::cell::RefCell;
use std::fs;

fn lock_fixture_tests() -> std::sync::MutexGuard<'static, ()> {
  super::GLOBAL_TEST_LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

/// Run a test body in a dedicated thread with a 32 MiB stack.
/// The suite lock is acquired inside the spawned thread so it is held for the
/// duration of the run and dropped before the thread exits.
fn run_with_large_stack(f: impl FnOnce() + Send + 'static) {
  const STACK_SIZE: usize = 32 * 1024 * 1024;
  std::thread::Builder::new()
    .stack_size(STACK_SIZE)
    .spawn(move || {
      let _guard = lock_fixture_tests();
      f();
    })
    .expect("spawn test thread")
    .join()
    .expect("test thread panicked");
}

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

#[test]
fn type_fail_schema_mismatch_fixtures_report_error_code() {
  run_with_large_stack(|| {
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
  });
}

#[test]
fn type_fail_call_arg_fixture_reports_warning_code() {
  run_with_large_stack(|| {
  let fixtures = [
    (
      "calcit/type-fail/schema-call-arg-type-mismatch.cirru",
      "expects type `:number`, but got `:string`",
    ),
    (
      "calcit/type-fail/type-slot-record-call-arg-type-mismatch.cirru",
      "expects type `type-slot(payload)`, but got `:number`",
    ),
  ];

  for (path, expected_msg) in fixtures {
    let entries = load_fixture_entries(path);
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("call-arg fixture should preprocess with warnings, not hard errors");

    let warnings = warnings.borrow();
    let matched: Vec<&LocatedWarning> = warnings
      .iter()
      .filter(|warning| warning.code() == Some("W_FN_ARG_TYPE_MISMATCH"))
      .collect();
    assert_eq!(
      matched.len(),
      1,
      "expected exactly one arg-type warning for {path}, got: {warnings:?}"
    );
    let warning = matched[0];
    assert_eq!(warning.code(), Some("W_FN_ARG_TYPE_MISMATCH"));
    assert!(
      warning.message().contains(expected_msg),
      "warning message for {path} was: {}",
      warning.message()
    );
  }
  });
}

#[test]
fn type_fail_generic_where_bound_fixture_reports_warning_code() {
  run_with_large_stack(|| {
  let entries = load_fixture_entries("calcit/type-fail/generic-where-bound-mismatch.cirru");
  let require_schema = program::lookup_def_schema("type-fail-generic-where-bound.main", "require-mappable");
  let CalcitTypeAnnotation::Fn(require_fn_annot) = require_schema.as_ref() else {
    panic!("require-mappable schema should load as fn, got {require_schema:?}");
  };
  assert_eq!(
    require_fn_annot.where_bounds.len(),
    1,
    "require-mappable should carry one where-bound, got {require_schema:?}"
  );
  let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

  runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
    .expect("generic where-bound fixture should preprocess with warnings, not hard errors");

  let warnings = warnings.borrow();
  let matched: Vec<&LocatedWarning> = warnings
    .iter()
    .filter(|warning| warning.code() == Some("W_GENERIC_WHERE_BOUND_MISMATCH"))
    .collect();
  assert_eq!(
    matched.len(),
    1,
    "expected exactly one generic where-bound warning, got: {warnings:?}"
  );
  let warning = matched[0];
  assert_eq!(warning.code(), Some("W_GENERIC_WHERE_BOUND_MISMATCH"));
  assert!(
    warning.message().contains("`:number`")
      && warning.message().contains("Mappable")
      && warning.message().contains("require-mappable 1"),
    "warning message was: {}",
    warning.message()
  );
  });
}

#[test]
fn type_fail_core_map_where_bound_fixture_reports_warning_code() {
  run_with_large_stack(|| {
  let entries = load_fixture_entries("calcit/type-fail/core-map-where-bound-mismatch.cirru");
  let map_schema = program::lookup_def_schema("calcit.core", "map");
  let CalcitTypeAnnotation::Fn(map_fn_annot) = map_schema.as_ref() else {
    panic!("core map schema should load as fn, got {map_schema:?}");
  };
  assert_eq!(
    map_fn_annot.where_bounds.len(),
    1,
    "core map should carry one where-bound, got {map_schema:?}"
  );
  let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

  runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
    .expect("core map where-bound fixture should preprocess with warnings, not hard errors");

  let warnings = warnings.borrow();
  let matched: Vec<&LocatedWarning> = warnings
    .iter()
    .filter(|warning| warning.code() == Some("W_GENERIC_WHERE_BOUND_MISMATCH"))
    .collect();
  assert_eq!(
    matched.len(),
    1,
    "expected exactly one generic where-bound warning, got: {warnings:?}"
  );
  let warning = matched[0];
  assert_eq!(warning.code(), Some("W_GENERIC_WHERE_BOUND_MISMATCH"));
  assert!(
    warning.message().contains("`:number`")
      && warning.message().contains("Mappable")
      && warning.message().contains("calcit.core/map")
      && warning.message().contains("calcit.core/inc"),
    "warning message was: {}",
    warning.message()
  );
  });
}

#[test]
fn type_fail_type_slot_fixtures_report_errors() {
  run_with_large_stack(|| {
  let fixtures = [(
    "calcit/type-fail/type-slot-bind-duplicate.cirru",
    "type slot 'payload' already bound",
  )];

  for (path, expected_msg) in fixtures {
    let entries = load_fixture_entries(path);
    let err = run_check_only(&entries).expect_err("type-slot fixture should fail during check-only");

    assert!(err.contains(expected_msg), "fixture {path} msg was: {err}");
  }
  });
}

#[test]
fn type_fail_type_slot_enum_invalid_variant() {
  run_with_large_stack(|| {
  let entries = load_fixture_entries("calcit/type-fail/type-slot-enum-invalid-variant.cirru");
  let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

  runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
    .expect("type-slot enum variant fixture should preprocess with warnings, not hard errors");

  let warnings = warnings.borrow();
  let matched: Vec<&LocatedWarning> = warnings
    .iter()
    .filter(|w| w.message().contains("does not have variant `:nonexistent`"))
    .collect();
  assert_eq!(matched.len(), 1, "expected exactly one invalid-variant warning, got: {warnings:?}");
  assert!(
    matched[0].message().contains("Enum `Action`"),
    "warning should mention enum name, got: {}",
    matched[0].message()
  );
  });
}

#[test]
fn type_fail_type_slot_fixture_is_repeatable_across_program_loads() {
  run_with_large_stack(|| {
  for _ in 0..2 {
    let entries = load_fixture_entries("calcit/type-fail/type-slot-record-call-arg-type-mismatch.cirru");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("type-slot fixture should preprocess with warnings on repeated loads");

    let warning_count = warnings
      .borrow()
      .iter()
      .filter(|warning| warning.code() == Some("W_FN_ARG_TYPE_MISMATCH"))
      .count();
    assert_eq!(warning_count, 1);
  }
  });
}

#[test]
fn run_check_only_surfaces_schema_error_code() {
  run_with_large_stack(|| {
  let entries = load_fixture_entries("calcit/type-fail/schema-required-arity.cirru");
  let err = run_check_only(&entries).expect_err("check-only should fail on schema mismatch fixture");

  assert!(
    err.contains("E_SCHEMA_DEF_MISMATCH"),
    "check-only error should contain code, got: {err}"
  );
  });
}
