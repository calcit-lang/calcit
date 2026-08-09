use super::*;
use calcit::calcit::{Calcit, CalcitProc, CalcitTypeAnnotation};
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

fn load_fixture_entries_with_entry(path: &str, selected_entry: Option<&str>) -> ProgramEntries {
  builtins::effects::init_effects_states();

  let content = fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read fixture: {path}"));
  let data = cirru_edn::parse(&content).unwrap_or_else(|e| panic!("Failed to parse fixture {path}: {e}"));
  let mut snapshot = snapshot::load_snapshot_data(&data, path).unwrap_or_else(|e| panic!("Failed to load fixture {path}: {e}"));
  snapshot
    .select_entry(selected_entry)
    .unwrap_or_else(|e| panic!("Failed to select fixture entry for {path}: {e}"));

  prepare_snapshot_entries(snapshot)
}

fn load_snippet_entries(snippet: &str) -> ProgramEntries {
  builtins::effects::init_effects_states();

  let mut snapshot = snapshot::Snapshot::default();
  snapshot.files.insert(
    "app.main".to_owned(),
    snapshot::create_file_from_snippet(snippet).expect("test snippet should parse"),
  );

  prepare_snapshot_entries(snapshot)
}

fn prepare_snapshot_entries(mut snapshot: snapshot::Snapshot) -> ProgramEntries {
  let core_snapshot = calcit::load_core_snapshot().expect("load core snapshot");

  for (k, v) in core_snapshot.files {
    snapshot.files.insert(k.to_owned(), v.to_owned());
  }

  {
    let mut prgm = program::PROGRAM_CODE_DATA.write().expect("open program data");
    *prgm = program::extract_program_data(&snapshot).expect("extract program data");
  }

  let selected_entry = snapshot.active_entry().expect("selected fixture entry");
  let config_init = selected_entry.init_fn.to_string();
  let config_reload = selected_entry.reload_fn.to_string();
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

fn load_fixture_entries(path: &str) -> ProgramEntries {
  load_fixture_entries_with_entry(path, None)
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
fn option_migration_source_calls_fail_during_preprocessing() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "do\n  = |dev $ get-env |mode\n  let\n      x $ get-env |mode\n    some? x\n  update-in ({} (:a ({}))) ([] :a) $ fn (x) do (assoc x :b 1)\n  let\n      op $ :: :session/connect\n      tag-name $ nth op 0\n    starts-with? tag-name :session/",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("Option migration examples should preprocess with warnings, not reach runtime");

    let warnings = warnings.borrow();
    for operation in ["=", "some?", "assoc"] {
      assert!(
        warnings.iter().any(|warning| {
          warning.code() == Some("W_NOMINAL_ENUM_LEGACY_USE")
            && warning.message().contains(&format!("`{operation}` consumes nominal enum `Option`"))
        }),
        "ordinary source call `{operation}` should report an Option migration warning, got: {warnings:?}"
      );
    }
    assert!(
      warnings.iter().any(|warning| {
        warning.code() == Some("W_PROC_ARG_TYPE_MISMATCH")
          && warning.message().contains("Proc `starts-with?` arg 1 expects type `:string`")
          && warning.message().contains("Option")
      }),
      "starts-with? should reject an Option argument during preprocessing, got: {warnings:?}"
    );
  });
}

#[test]
fn required_struct_field_access_does_not_fall_back_to_option_lookup() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "do\n  let\n      record $ {} (:name |Ada)\n      name $ :name record\n    , name\n  let\n      Person $ defstruct Person (:name 'String)\n      person $ %{} Person (:name |Ada)\n      name $ :name person\n    assert-type name 'String\n  let\n      Person $ defstruct Person (:name 'String)\n      person $ %{} Person (:name |Ada)\n    :missing person\n  let\n      Person $ defstruct Person (:name 'String)\n      person $ %{} Person (:name |Ada)\n    get person :name",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("required-field examples should preprocess with actionable diagnostics");

    let warnings = warnings.borrow();
    let required_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("W_REQUIRED_STRUCT_FIELD_TYPE"))
      .collect::<Vec<_>>();
    assert_eq!(
      required_warnings.len(),
      1,
      "only the Map-backed field syntax should require a Struct declaration, got: {warnings:?}"
    );
    assert!(
      required_warnings[0]
        .message()
        .contains("use `(get value :name)` only when absence is intentional")
    );

    let struct_get_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("W_STRUCT_FIELD_OPTIONAL_LOOKUP"))
      .collect::<Vec<_>>();
    assert_eq!(
      struct_get_warnings.len(),
      1,
      "`get` on a typed Struct should point back to required field syntax, got: {warnings:?}"
    );
    assert!(struct_get_warnings[0].message().contains("Use `(:name value)`"));

    let unknown_field_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("W_UNKNOWN_STRUCT_FIELD"))
      .collect::<Vec<_>>();
    assert_eq!(
      unknown_field_warnings.len(),
      1,
      "a missing declared Struct field should fail before runtime, got: {warnings:?}"
    );
    assert!(
      unknown_field_warnings[0]
        .message()
        .contains("Field `:missing` does not exist in struct `Person`")
    );
  });
}

#[test]
fn collection_path_operations_do_not_traverse_struct_fields() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "do\n  let\n      Person $ defstruct Person (:name 'String)\n      person $ %{} Person (:name |Ada)\n      people $ {} (:current person)\n    do\n      get-in person ([] :name)\n      get-in people ([] :current :name)\n      assoc-in person ([] :name) |Grace\n      update-in person ([] :name) $ fn (current) (, current)\n      dissoc-in person ([] :name)",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("Struct path misuse should preprocess far enough to report every operation");

    let warnings = warnings.borrow();
    let path_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("W_STRUCT_PATH_OPERATION"))
      .collect::<Vec<_>>();
    assert_eq!(
      path_warnings.len(),
      5,
      "every collection path operation that reaches a Struct should be rejected, got: {warnings:?}"
    );
    for operation in ["get-in", "assoc-in", "update-in", "dissoc-in"] {
      assert!(
        path_warnings
          .iter()
          .any(|warning| warning.message().contains(&format!("`{operation}`"))),
        "missing Struct path diagnostic for `{operation}`, got: {warnings:?}"
      );
    }
    assert!(
      path_warnings
        .iter()
        .all(|warning| warning.message().contains("use `(:field value)` for reads or `assoc`/`update`")),
      "Struct path diagnostics should point AI edits back to typed field operations, got: {warnings:?}"
    );
  });
}

#[test]
fn type_fail_trait_method_generic_receiver_fixture_reports_warning_code() {
  run_with_large_stack(|| {
    let entries = load_fixture_entries("calcit/type-fail/trait-method-generic-receiver-mismatch.cirru");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("trait method fixture should preprocess with warnings, not hard errors");

    let warnings = warnings.borrow();
    let matched: Vec<&LocatedWarning> = warnings
      .iter()
      .filter(|warning| warning.code() == Some("W_METHOD_ARG_TYPE_MISMATCH"))
      .collect();
    assert_eq!(matched.len(), 1, "expected one method argument warning, got: {warnings:?}");
    assert!(
      matched[0].message().contains(".unwrap-or") && matched[0].message().contains("expects type `:string`, but got `:number`"),
      "warning message was: {}",
      matched[0].message()
    );
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

fn contains_with_type_slot(value: &Calcit) -> bool {
  match value {
    Calcit::Proc(CalcitProc::WithTypeSlot) => true,
    Calcit::List(items) => items.iter().any(contains_with_type_slot),
    Calcit::Fn { info, .. } => info.body.iter().any(contains_with_type_slot),
    Calcit::Macro { info, .. } => info.body.iter().any(contains_with_type_slot),
    _ => false,
  }
}

#[test]
fn with_type_slot_multi_body_is_erased_before_runtime() {
  run_with_large_stack(|| {
    let entries = load_fixture_entries("calcit/type-fail/type-slot-enum-invalid-variant.cirru");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    let ns = entries.init_ns.as_ref();
    let def = "legacy-main!";

    runner::preprocess::ensure_ns_def_compiled(ns, def, &warnings, &CallStackList::default())
      .expect("legacy with-type-slot fixture should preprocess");
    let compiled = program::lookup_compiled_def(ns, def).expect("legacy fixture should have compiled output");
    assert!(
      !contains_with_type_slot(&compiled.preprocessed_code),
      "with-type-slot must not escape preprocessing: {}",
      compiled.preprocessed_code
    );

    let result = calcit::run_program(Arc::from(ns), Arc::from(def), &[]).expect("legacy fixture should run");
    assert_eq!(result, Calcit::Number(2.0));
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
fn type_slot_bindings_are_scoped_to_selected_entry() {
  run_with_large_stack(|| {
    for selected_entry in [None, Some("server")] {
      let entries = load_fixture_entries_with_entry("calcit/type-fail/type-slot-entry-scope.cirru", selected_entry);
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect("entry-scoped with-type-slot fixture should preprocess without duplicate slot errors");
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
