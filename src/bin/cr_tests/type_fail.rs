use super::*;
use calcit::calcit::{
  Calcit, CalcitFnTypeAnnotation, CalcitProc, CalcitTypeAnnotation, MacroExpansionType, MacroSignature, MacroSyntaxType, SchemaKind,
};
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::sync::Arc;

fn lock_fixture_tests() -> std::sync::MutexGuard<'static, ()> {
  super::GLOBAL_TEST_LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

struct StrictTypesReset {
  previous: bool,
}

impl StrictTypesReset {
  fn enabled() -> Self {
    let previous = runner::preprocess::is_strict_types_enabled();
    runner::preprocess::set_strict_types(true);
    Self { previous }
  }

  fn disabled() -> Self {
    let previous = runner::preprocess::is_strict_types_enabled();
    runner::preprocess::set_strict_types(false);
    Self { previous }
  }
}

impl Drop for StrictTypesReset {
  fn drop(&mut self) {
    runner::preprocess::set_strict_types(self.previous);
  }
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
  load_snippet_entries_with_main_schema(snippet, None)
}

fn load_snippet_entries_with_main_schema(snippet: &str, main_schema: Option<Arc<CalcitTypeAnnotation>>) -> ProgramEntries {
  builtins::effects::init_effects_states();

  let mut snapshot = snapshot::Snapshot::default();
  let mut file = snapshot::create_file_from_snippet(snippet).expect("test snippet should parse");
  annotate_snippet_macros(&mut file);
  if let Some(schema) = main_schema
    && let Some(entry) = file.defs.get_mut("main!")
  {
    entry.schema = schema;
  }
  snapshot.files.insert("app.main".to_owned(), file);

  prepare_snapshot_entries(snapshot)
}

/// Give every `defmacro` in a snippet file a Macro schema with one Syntax input
/// per parameter, as a checked library would declare.
fn annotate_snippet_macros(file: &mut snapshot::FileInSnapShot) {
  for entry in file.defs.values_mut() {
    let Some(required_inputs) = (match &entry.code {
      cirru_parser::Cirru::List(items) if matches!(items.first(), Some(cirru_parser::Cirru::Leaf(head)) if head.as_ref() == "defmacro") => {
        match items.get(2) {
          Some(cirru_parser::Cirru::List(params)) => Some(vec![MacroSyntaxType::Syntax; params.len()]),
          _ => None,
        }
      }
      _ => None,
    }) else {
      continue;
    };
    entry.schema = Arc::new(CalcitTypeAnnotation::Macro(Arc::new(MacroSignature {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      required_inputs: Arc::new(required_inputs),
      optional_inputs: Arc::new(vec![]),
      rest_input: None,
      expansion: MacroExpansionType::Dynamic,
      capabilities: Arc::new(HashSet::new()),
      features: Arc::new(HashSet::new()),
    })));
  }
}

/// Load `app.main` from `snippet` with `library_snippet` as the dependency
/// namespace `library_ns`, which is not a project namespace.
fn load_snippet_entries_with_library(library_ns: &str, library_snippet: &str, snippet: &str) -> ProgramEntries {
  builtins::effects::init_effects_states();

  let mut snapshot = snapshot::Snapshot::default();
  let mut file = snapshot::create_file_from_snippet(snippet).expect("test snippet should parse");
  annotate_snippet_macros(&mut file);
  snapshot.files.insert("app.main".to_owned(), file);

  let mut library = snapshot::create_file_from_snippet(library_snippet).expect("library snippet should parse");
  annotate_snippet_macros(&mut library);
  library.ns.code = vec!["ns", library_ns].into();
  prepare_snapshot_entries_with_libraries(snapshot, vec![(library_ns.to_owned(), library)])
}

fn prepare_snapshot_entries(snapshot: snapshot::Snapshot) -> ProgramEntries {
  prepare_snapshot_entries_with_libraries(snapshot, vec![])
}

fn prepare_snapshot_entries_with_libraries(
  mut snapshot: snapshot::Snapshot,
  libraries: Vec<(String, snapshot::FileInSnapShot)>,
) -> ProgramEntries {
  let project_namespaces: HashSet<String> = snapshot.files.keys().cloned().collect();
  snapshot.files.extend(libraries);

  for (k, v) in &cached_core_snapshot().files {
    snapshot.files.insert(k.to_owned(), v.to_owned());
  }
  runner::preprocess::set_project_namespaces(&project_namespaces);

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
fn type_fail_struct_update_after_option_match_rejects_invalid_fields_and_values() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    for (update, diagnostic) in [
      ("assoc draft :ready? |wrong", "expects type `:bool`, but got `:string`"),
      ("assoc draft :missing true", "Field `:missing` does not exist"),
      (
        "assoc draft :maybe $ Option :some |wrong",
        "expects type `'calcit.core/Option<:number>`, but got `enum Option<:string>`",
      ),
    ] {
      let snippet = format!(
        "let\n    Draft $ defstruct Draft (:ready? Bool) (:maybe $ :: Option 'Number)\n    drafts $ {{}} $ |a $ Draft :ready? false :maybe (Option :some 1)\n  match (get drafts |a)\n    (:some draft) $ {update}\n    (:none) &unit"
      );
      let entries = load_snippet_entries(&snippet);
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect("invalid updates should produce type diagnostics, not unrelated preprocessing failures");
      assert!(
        warnings.borrow().iter().any(|warning| warning.message().contains(diagnostic)),
        "expected {diagnostic}: {:?}",
        warnings.borrow()
      );
      assert!(
        warnings
          .borrow()
          .iter()
          .all(|warning| warning.code() != Some("E_NOMINAL_ENUM_LEGACY_USE")),
        "stored values must use ordinary field diagnostics, not nullable-receiver warnings"
      );
    }
  });
}

/// Comparing a nominal `Option` value with `nil` is a type mismatch; Option values are consumed with methods or `match` (#1554).
#[test]
fn type_fail_nominal_option_compared_with_nil_reports_arg_type_mismatch() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_snippet_entries("let ((value (Option :some 1)))\n  = value nil");
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("nil comparison should produce a type diagnostic, not a preprocessing failure");
    let warnings = warnings.borrow();
    assert!(
      warnings.iter().any(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH")
        && warning.message().contains("enum Option<:number>")
        && warning.message().contains(":nil")),
      "expected E_FN_ARG_TYPE_MISMATCH for Option vs nil: {warnings:?}"
    );
  });
}

// Pins the contract that `nil->option` rejects `JsNullish<T>` arguments: a Rust
// type-fail test is required because `:tests` run only after a clean check and
// cannot express an expected preprocessing diagnostic (#1554).
#[test]
fn strict_type_fail_nil_to_option_rejects_js_nullish_argument() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_snippet_entries(
      "let\n    f $ fn (x)\n      hint-fn $ Fn (JsNullish Number)\n        -> $ Option Dynamic\n      nil->option x\n  f 1",
    );
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("nil->option with JsNullish should produce a type diagnostic, not a preprocessing failure");
    let warnings = warnings.borrow();
    assert!(
      warnings.iter().any(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH")
        && warning.message().contains("calcit.core/nil->option")
        && warning.message().contains("js-nullish<:number>")),
      "nil->option must keep rejecting JsNullish arguments: {warnings:?}"
    );

    // Ordinary values, nil and an inferred nil join stay warning-free.
    for source in [
      "nil->option nil",
      "let ((value (Option :some 1)))\n  nil->option value",
      // A nil join goes straight into the bridge; binding it first is a strict error.
      "nil->option (if (> 1 0) 1 nil)",
    ] {
      let accepted = load_snippet_entries(source);
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&accepted.init_ns, &accepted.init_def, &warnings, &CallStackList::default())
        .expect("nil->option of an accepted value should preprocess");
      assert!(
        warnings.borrow().is_empty(),
        "`{source}` should stay clean: {:?}",
        warnings.borrow()
      );
    }
  });
}

// `map xs nil->option` is accepted when the list element type is known, including
// a literal whose elements infer an internal nil join.
#[test]
fn strict_type_fail_nil_to_option_as_map_callback_uses_element_types() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let typed = load_snippet_entries("let ((xs $ assert-type ([] 1 2) $ quote $ List Number)) $ map xs nil->option");
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&typed.init_ns, &typed.init_def, &warnings, &CallStackList::default())
      .expect("a typed list should accept nil->option as a callback");
    assert!(
      warnings.borrow().is_empty(),
      "typed callback use stays clean: {:?}",
      warnings.borrow()
    );

    // A literal that mixes values with nil infers an internal nil join, which
    // is exactly the argument nil->option accepts.
    let joined = load_snippet_entries("map ([] 1 nil 2) nil->option");
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&joined.init_ns, &joined.init_def, &warnings, &CallStackList::default())
      .expect("a nil-joined list literal should accept nil->option as a callback");
    assert!(
      warnings.borrow().is_empty(),
      "nil-joined callback use stays clean: {:?}",
      warnings.borrow()
    );
  });
}

// The bridge drops every inferred nil layer, also when nested nil-join
// branches stack them or when it is passed to `map` by name, so the result
// satisfies a declared `Option<Number>` payload.
#[test]
fn strict_type_fail_nil_to_option_flattens_nil_joins() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    for source in [
      "let\n    f $ fn (a b)\n      hint-fn $ Fn Bool Bool (-> $ Option Number)\n      nil->option $ if a (if b 1 nil) nil\n  f true false",
      "let\n    f $ fn ()\n      hint-fn $ Fn (-> $ List $ Option Number)\n      map ([] 1 nil 2) nil->option\n  f",
      "let\n    f $ fn (c xs)\n      hint-fn $ Fn Bool (List Number) (-> $ List $ Option Number)\n      map (map xs $ fn (x) (if c x nil)) nil->option\n  f true ([] 1 2)",
    ] {
      let entries = load_snippet_entries(source);
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect("nil->option over a nil join should preprocess");
      assert!(
        warnings.borrow().is_empty(),
        "`{source}` should prove an Option<Number> payload: {:?}",
        warnings.borrow()
      );
    }
  });
}

/// `value.:field` must report the same strict diagnostics as `(:field value)` (#1988).
#[test]
fn type_fail_tag_access_shorthand_matches_tag_call_diagnostics() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let collect = |access: &str| -> Vec<(Option<String>, String)> {
      let snippet = format!("let\n    Ev $ defstruct Ev (:type Tag)\n    ev $ Ev :type :create\n  {access}");
      let entries = load_snippet_entries(&snippet);
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect("field access diagnostics should be warnings, not preprocessing failures");
      let diagnostics: Vec<(Option<String>, String)> = warnings
        .borrow()
        .iter()
        .map(|w| (w.code().map(str::to_owned), w.message().to_string()))
        // The snippet harness leaks unrelated `eprintln` lookups from core macros; ignore them.
        .filter(|(_, m)| !m.contains("eprintln"))
        .collect();
      diagnostics
    };
    let shorthand = collect("ev.:missing");
    let call = collect(":missing ev");
    assert_eq!(shorthand, call, "shorthand and tag call must report identical diagnostics");
    assert!(
      shorthand.iter().any(|(code, _)| code.as_deref() == Some("E_UNKNOWN_STRUCT_FIELD")),
      "unknown field must be reported with E_UNKNOWN_STRUCT_FIELD: {shorthand:?}"
    );
  });
}

/// Ensures nested external methods reject explicit arguments during preprocessing.
#[test]
fn nested_external_method_rejects_explicit_extra_argument() {
  run_with_large_stack(|| {
    let entries = load_fixture_entries("calcit/test-js.cirru");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(
      &entries.init_ns,
      "test-nested-external-arity-fail",
      &warnings,
      &CallStackList::default(),
    )
    .expect("negative external-method fixture should preprocess with an arity diagnostic");

    let warnings = warnings.borrow();
    assert!(
      warnings.iter().any(|warning| warning
        .message()
        .contains("Method `.upper` expects 1 args (including receiver), got 2")),
      "expected explicit-argument arity diagnostic for nested external method: {warnings:?}"
    );
  });
}

#[test]
fn result_generic_declaration_order_preserves_ok_and_err_payload_types() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "do\n  match (parse-float |41)\n    (:ok value) (+ value 1)\n    (:err error) (str error |!)\n  match (parse-float |bad)\n    (:ok value) (+ value 1)\n    (:err error) (starts-with? error |b)",
    );
    run_check_only(&entries).expect("Result<Number,String> must bind ok to Number and err to String");
    for snippet in [
      "match (parse-float |41)\n  (:ok value) (starts-with? value |4)\n  (:err _) false",
      "match (parse-float |bad)\n  (:ok _) 0\n  (:err error) (+ error 1)",
    ] {
      let entries = load_snippet_entries(snippet);
      run_check_only(&entries).expect_err("Result payloads must not accept operations for the opposite type");
    }
  });
}

#[test]
fn strict_fs_path_core_methods_bind_result_map_callback_payloads() {
  run_with_large_stack(|| {
    let main_schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      open_reason: Default::default(),
      cases: Default::default(),
      guard: None,
      runtime_arity: None,
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("calcit.core/FsPath"), Arc::new(vec![]))),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));
    let _strict = StrictTypesReset::enabled();
    injection::inject_platform_apis();
    let entries = load_snippet_entries_with_main_schema("fs:path |.", Some(main_schema));

    run_check_only(&entries).expect("reachable FsPath methods should bind result:map callbacks without core warnings");
  });
}

#[test]
fn removed_map_kv_method_points_to_filter_map_kv() {
  run_with_large_stack(|| {
    let _restore = StrictTypesReset {
      previous: runner::preprocess::is_strict_types_enabled(),
    };
    for strict in [false, true] {
      runner::preprocess::set_strict_types(strict);
      // A callback written for the old method must not fail before the retirement.
      for snippet in [
        "({} (:a 1)) .map-kv $ fn (k v) ([] k (+ v 10))",
        ".map-kv ({} (:a 1)) $ fn (k v) ([] k (+ v 10))",
      ] {
        let entries = load_snippet_entries(snippet);
        let err = run_check_only(&entries).expect_err(&format!(
          "the removed `.map-kv` method must not resolve (strict={strict}): {snippet}"
        ));
        assert!(err.contains("E_RETIRED_METHOD"), "missing retirement diagnostic: {err}");
        assert!(err.contains("`.filter-map-kv`"), "missing replacement spelling: {err}");
      }
    }
  });
}

#[test]
fn local_method_sharing_a_retired_core_name_still_dispatches() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _restore = StrictTypesReset {
      previous: runner::preprocess::is_strict_types_enabled(),
    };
    for strict in [false, true] {
      runner::preprocess::set_strict_types(strict);
      // A local struct's own `.write-text` is not the retired core FsPath alias, in either call form.
      let entries = load_fixture_entries("calcit/type-fail/local-retired-name-method.cirru");
      run_check_only(&entries).unwrap_or_else(|err| panic!("local `.write-text` must dispatch (strict={strict}): {err}"));
      // The core FsPath alias stays retired in both call forms.
      for snippet in [
        ".write-text (fs:path |/calcit-retired-method-does-not-exist/file) |content",
        "(fs:path |/calcit-retired-method-does-not-exist/file) .write-text |content",
      ] {
        let entries = load_snippet_entries(snippet);
        let err = run_check_only(&entries).expect_err(&format!("core `.write-text` must stay retired (strict={strict}): {snippet}"));
        assert!(err.contains("E_RETIRED_METHOD"), "missing retirement diagnostic: {err}");
        assert!(err.contains("`.write-text!`"), "missing replacement spelling: {err}");
      }
    }
  });
}

#[test]
fn strict_map_list_kv_preserves_key_value_and_result_types() {
  run_with_large_stack(|| {
    let main_schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      open_reason: Default::default(),
      cases: Default::default(),
      guard: None,
      runtime_arity: None,
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: Arc::new(CalcitTypeAnnotation::Unit),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));
    let _strict = StrictTypesReset::enabled();

    let entries = load_snippet_entries_with_main_schema(
      "let\n    output $ map-list-kv\n      {} $ :a |x\n      fn (key value) (starts-with? value |x)\n  assert-type output $ quote $ List Bool\n  , &unit",
      Some(main_schema.clone()),
    );
    run_check_only(&entries).expect("map-list-kv should preserve K, V, and callback result U in strict mode");

    let entries = load_snippet_entries_with_main_schema(
      "let\n    output $ map-list-kv\n      {} $ :a |x\n      fn (key value) (+ value 1)\n  assert-type output $ quote $ List Number\n  , &unit",
      Some(main_schema),
    );
    run_check_only(&entries).expect_err("map-list-kv callback values must keep the Map value type");
  });
}

#[test]
fn self_referential_enum_type_ref_validation_is_finite() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("defenum Tree (:leaf Number) (:branch Tree)\ndefn main! ()\n  Tree :branch $ Tree :leaf 1");

    run_check_only(&entries).expect("self-referential enum payload validation must not recurse while rebuilding Tree");
  });
}

#[test]
fn strict_rejects_bare_zero_argument_enum_constructor_value_positions() {
  run_with_large_stack(|| {
    let option_number = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Option"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
    ));
    let main_schema = |return_type| {
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        open_reason: Default::default(),
        cases: Default::default(),
        guard: None,
        runtime_arity: None,
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![],
        return_type,
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      })))
    };
    let _strict = StrictTypesReset::enabled();

    // Core no longer defines `%none`; a project-defined zero-argument `%name`
    // constructor keeps the source convention that this diagnostic protects.
    let constructor = "defn %none ()\n  Option :none\n\n";
    for body in [
      "let\n      _ &unit\n    , %none",
      "if true %none (%none)",
      "match (Option :some 1)\n    (:some _) %none\n    (:none) (%none)",
    ] {
      let snippet = format!("{constructor}defn main! ()\n  {body}");
      let entries = load_snippet_entries_with_main_schema(&snippet, Some(main_schema(option_number.clone())));
      let error = run_check_only(&entries).expect_err("bare %none must not satisfy an Option return or branch type");
      assert!(error.contains("E_BARE_ENUM_CONSTRUCTOR_VALUE"), "unexpected error: {error}");
      assert!(
        error.contains("(%none)"),
        "diagnostic must suggest invoking the constructor: {error}"
      );
    }

    let entries = load_snippet_entries_with_main_schema(
      &format!("{constructor}defn main! ()\n  = %none %none"),
      Some(main_schema(Arc::new(CalcitTypeAnnotation::Bool))),
    );
    let error = run_check_only(&entries).expect_err("comparing the same bare constructor must not mask the missing invocation");
    assert!(error.contains("E_BARE_ENUM_CONSTRUCTOR_VALUE"), "unexpected error: {error}");
    assert!(
      error.contains("mask the missing invocation"),
      "comparison diagnostic must explain the trap: {error}"
    );

    let entries = load_snippet_entries_with_main_schema(
      &format!("{constructor}defn main! ()\n  option:none? %none"),
      Some(main_schema(Arc::new(CalcitTypeAnnotation::Bool))),
    );
    let error = run_check_only(&entries).expect_err("bare %none must not satisfy an Option-valued call argument");
    assert!(error.contains("E_BARE_ENUM_CONSTRUCTOR_VALUE"), "unexpected error: {error}");
  });
}

#[test]
fn strict_accepts_invoked_enum_constructor_and_intentional_constructor_function() {
  run_with_large_stack(|| {
    let option_number = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Option"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
    ));
    let main_schema = |return_type| {
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        open_reason: Default::default(),
        cases: Default::default(),
        guard: None,
        runtime_arity: None,
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![],
        return_type,
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      })))
    };
    let _strict = StrictTypesReset::enabled();

    // A single Cirru line is a call, so this snippet body is `(%none)`.
    let constructor = "defn %none ()\n  Option :none\n\n";
    let invoked = load_snippet_entries_with_main_schema(
      &format!("{constructor}defn main! ()\n  %none"),
      Some(main_schema(option_number.clone())),
    );
    run_check_only(&invoked).expect("(%none) must remain a valid Option value");

    let constructor_type = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      open_reason: Default::default(),
      cases: Default::default(),
      guard: None,
      runtime_arity: None,
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: option_number,
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));
    let function_value = load_snippet_entries_with_main_schema(
      &format!("{constructor}defn main! ()\n  let\n      _ &unit\n    , %none"),
      Some(main_schema(constructor_type)),
    );
    run_check_only(&function_value).expect("bare %none must remain valid when a constructor function is expected");
  });
}

#[test]
fn reset_diagnostic_renders_the_call_head_once() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("let ((state $ ref 1)) (reset! state |wrong)");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("mismatched reset should produce a warning");
    let warnings = warnings.borrow();
    let matched: Vec<_> = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_RESET_ARG_TYPE_MISMATCH"))
      .collect();
    assert_eq!(matched.len(), 1, "{warnings:?}");
    let message = matched[0].message();
    assert!(message.contains("arg 2 expects type `:number`, but got `:string`"), "{message}");
    let expression = message.split("Expression: ").nth(1).expect("diagnostic includes expression");
    assert_eq!(expression.matches("reset!").count(), 1, "{expression}");
  });
}

#[test]
fn strict_public_dissoc_keeps_a_known_collection_contract() {
  run_with_large_stack(|| {
    // Removing an entry from a typed Map keeps the Map contract, while a key
    // of the wrong type is still rejected by the Map key check.
    for (key_type, value, passes) in [
      ("String", "dissoc (deref state) key", true),
      ("String", "dissoc (deref state) key key", true),
      ("Number", "dissoc (deref state) key", false),
    ] {
      let snippet = format!(
        "hint-fn $ Fn (-> Unit)\n\nlet\n    write $ fn (state key)\n      hint-fn $ Fn\n        Ref $ Map String Number\n        , {key_type} (-> Unit) (:features :state)\n      reset! state $ {value}\n      , &unit\n  , &unit"
      );
      assert_strict_snippet(&snippet, passes, &format!("key={key_type}, value={value}"));
    }
    // A List receiver keeps List<T> only when its single index is a proven Number.
    for (index_type, passes) in [("Number", true), ("Dynamic", false)] {
      let snippet = format!(
        "hint-fn $ Fn (-> Unit)\n\nlet\n    write $ fn (state index)\n      hint-fn $ Fn\n        Ref $ List String\n        , {index_type} (-> Unit) (:features :state)\n      reset! state $ dissoc (deref state) index\n      , &unit\n  , &unit"
      );
      assert_strict_snippet(&snippet, passes, &format!("list index={index_type}"));
    }
  });
}

fn assert_strict_snippet(snippet: &str, passes: bool, label: &str) {
  let mut entries = load_snippet_entries(snippet);
  entries.reload_fn = entries.init_fn.clone();
  entries.reload_ns = entries.init_ns.clone();
  entries.reload_def = entries.init_def.clone();
  let _strict = StrictTypesReset::enabled();
  let result = run_check_only(&entries);
  assert_eq!(result.is_ok(), passes, "{label}: {result:?}");
}

#[test]
fn reset_rejects_incompatible_inferred_reference_payloads() {
  run_with_large_stack(|| {
    for snippet in [
      "let ((state $ ref 1)) (reset! state |wrong)",
      "let ((state $ ref $ {} $ :states $ {} $ :cursor $ [])) (reset! state $ {} $ :states $ {} $ :login $ {} (:data 1))",
      "reset! 1 2",
      "fn (state value)\n  hint-fn $ Fn (Ref T) T (-> Unit) (:features :state) (:generics T)\n  reset! state |wrong\n  , &unit",
      "fn (state value)\n  hint-fn $ Fn (Ref Number) T (-> Unit) (:features :state) (:generics T)\n  reset! state value\n  , &unit",
      "fn (state value)\n  hint-fn $ Fn\n    Ref $ List T\n    List U\n    -> Unit\n    :generics T U\n    :features :state\n  reset! state value\n  , &unit",
      "fn (state)\n  hint-fn $ Fn (Ref Number) (-> Unit) (:features :state)\n  reset! state |wrong\n  , &unit",
    ] {
      let entries = load_snippet_entries(snippet);
      run_check_only(&entries).expect_err("incompatible reset! must fail before typed reads can use stale evidence");
    }
    for snippet in [
      "let ((state $ ref 1)) (reset! state 2) (+ @state 1)",
      "fn (state value)\n  hint-fn $ Fn (Ref T) T (-> Unit) (:features :state) (:generics T)\n  reset! state value\n  , &unit",
      "fn (state value)\n  hint-fn $ Fn\n    Ref $ List T\n    List T\n    -> Unit\n    :generics T\n    :features :state\n  reset! state value\n  , &unit",
      "let ((state $ ref $ {} (:count 1))) (reset! state $ {} (:count 2)) (get-in @state $ [] :count)",
      "fn (state)\n  hint-fn $ Fn (Ref Number) (-> Unit) (:features :state)\n  reset! state 2\n  , &unit",
      "fn (state)\n  hint-fn $ Fn (Ref Dynamic) (-> Unit) (:features :state)\n  reset! state |open-boundary\n  , &unit",
    ] {
      let entries = load_snippet_entries(snippet);
      run_check_only(&entries).expect("compatible reference writes should retain their payload types");
    }
  });
}

#[test]
fn strict_reset_requires_known_outer_payload_or_explicit_open_reference() {
  run_with_large_stack(|| {
    for (payload, passes) in [("Number", false), ("(List Dynamic)", false), ("T", false), ("Dynamic", true)] {
      let snippet = format!(
        "hint-fn $ Fn (-> Unit)\n\nlet\n    write $ fn (state value)\n      hint-fn $ Fn (Ref {payload}) Dynamic (-> Unit) (:features :state) (:generics T)\n      reset! state value\n      , &unit\n  , &unit"
      );
      let mut entries = load_snippet_entries(&snippet);
      // Both entries use the explicitly hinted function, so a missing schema
      // on the synthetic reload stub cannot masquerade as write rejection.
      entries.reload_fn = entries.init_fn.clone();
      entries.reload_ns = entries.init_ns.clone();
      entries.reload_def = entries.init_def.clone();
      let _strict = StrictTypesReset::enabled();
      let result = run_check_only(&entries);
      if passes {
        result.expect("Ref<Dynamic> intentionally accepts an unknown payload");
      } else {
        result.expect_err("strict reset! must not write whole Dynamic into a constrained reference");
      }
    }
  });
}

#[test]
fn strict_reset_checks_nested_unknowns_and_literal_empty_values() {
  run_with_large_stack(|| {
    for (payload, input, value, passes) in [
      ("(List Number)", "(List Dynamic)", "value", false),
      ("(Map Tag Number)", "(Map Tag Dynamic)", "value", false),
      ("(Option Number)", "(Option Dynamic)", "value", false),
      ("(List Dynamic)", "(List Dynamic)", "value", true),
      ("(List Number)", "Number", "([])", true),
      ("(Set Number)", "Number", "(#{})", true),
      ("(Map Tag Number)", "Number", "({})", true),
      ("(List $ List Number)", "Number", "([] $ [])", true),
      ("(Option Number)", "Number", "(Option :none)", true),
      ("(List Number)", "Dynamic", "([] value)", false),
    ] {
      let snippet = format!(
        "hint-fn $ Fn (-> Unit)\n\nlet\n    write $ fn (state value)\n      hint-fn $ Fn (Ref {payload}) {input} (-> Unit) (:features :state)\n      reset! state {value}\n      , &unit\n  , &unit"
      );
      let mut entries = load_snippet_entries(&snippet);
      entries.reload_fn = entries.init_fn.clone();
      entries.reload_ns = entries.init_ns.clone();
      entries.reload_def = entries.init_def.clone();
      let _strict = StrictTypesReset::enabled();
      let result = run_check_only(&entries);
      assert_eq!(
        result.is_ok(),
        passes,
        "payload={payload}, input={input}, value={value}: {result:?}"
      );
    }
  });
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
fn hint_fn_rejects_two_arguments_before_reading_either() {
  run_with_large_stack(|| {
    for (source, expected_coord) in [
      ("type-of $ hint-fn ({} (:args ([])) (:return (quote Number))) (:: fn () 1)", None),
      ("let ((f $ fn () 1)) (hint-fn f (:: fn () 1))", Some(vec![3, 2, 1])),
    ] {
      let entries = load_snippet_entries(source);
      let error = runner::preprocess::ensure_ns_def_compiled(
        &entries.init_ns,
        &entries.init_def,
        &RefCell::new(vec![]),
        &CallStackList::default(),
      )
      .expect_err("the two-argument form was removed in 0.32.0");
      assert_eq!(error.code.as_deref(), Some("E_LEGACY_HINT_FN_TARGET"), "{error}");
      let location = error.location.as_ref().expect("the removal diagnostic has a source location");
      assert_eq!(location.ns.as_ref(), "app.main", "{error}");
      assert_eq!(location.def.as_ref(), "main!", "{error}");
      if let Some(expected_coord) = expected_coord {
        assert_eq!(*location.coord, expected_coord, "{error}");
      }
    }
  });
}

#[test]
fn hint_fn_rejects_malformed_forms_instead_of_ignoring_them() {
  run_with_large_stack(|| {
    let cases = [
      (
        "hint-fn",
        "hint-fn expects one function type such as `hint-fn $ Fn Number (-> String)`; got 0 arguments",
      ),
      (
        "let ((target $ fn () 1)) (hint-fn target ({} (:args ([])) (:return 'Number)) :extra)",
        "hint-fn expects one function type such as `hint-fn $ Fn Number (-> String)`; got 3 arguments",
      ),
      (
        "fn (value)\n  hint-fn $ :: 'Fn $ {} (:args $ [] 'String) (:return 'String)\n  , value",
        "hint-fn expects a function type",
      ),
      (
        "let ((target 7))\n  hint-fn target $ {} (:args $ []) (:return 'Number)\n  , target",
        "two-argument `hint-fn target schema` was removed in 0.32.0",
      ),
    ];

    for (source, expected) in cases {
      let entries = load_snippet_entries(source);
      let error = runner::preprocess::ensure_ns_def_compiled(
        &entries.init_ns,
        &entries.init_def,
        &RefCell::new(vec![]),
        &CallStackList::default(),
      )
      .expect_err("malformed hint-fn source must fail preprocessing");
      assert!(
        error.msg.contains(expected),
        "hint-fn diagnostic should mention {expected:?}, got: {error}"
      );
      let location = error.location.as_ref().expect("hint-fn diagnostics must retain source location");
      assert_eq!(location.ns.as_ref(), "app.main", "{error}");
      assert_eq!(location.def.as_ref(), "main!", "{error}");
    }
  });
}

/// #2378: the two-argument `(hint-fn f schema)` form was removed in 0.32.0.
/// Signed and unsigned targets are both rejected at the target with a
/// migration hint, while a body hint keeps compiling.
#[test]
fn two_argument_hint_fn_is_rejected() {
  run_with_large_stack(|| {
    for target in ["signed", "unsigned"] {
      let entries = load_snippet_entries(&format!(
        "let
    signed $ fn (x)
      hint-fn $ Fn Dynamic (-> Number)
      , 1
    unsigned $ fn (x) x
  hint-fn {target} $ Fn Number (-> Number)
  {target} 1"
      ));
      let error = runner::preprocess::ensure_ns_def_compiled(
        &entries.init_ns,
        &entries.init_def,
        &RefCell::new(vec![]),
        &CallStackList::default(),
      )
      .expect_err("the two-argument form must fail preprocessing");
      assert!(
        error.msg.contains("E_LEGACY_HINT_FN_TARGET") || error.code.as_deref() == Some("E_LEGACY_HINT_FN_TARGET"),
        "{error}"
      );
      assert!(
        error.msg.contains(&format!("hint-fn {target} schema"))
          && error.msg.contains("removed in 0.32.0")
          && error.msg.contains("into the body of the function"),
        "{error}"
      );
      let location = error.location.as_ref().expect("the diagnostic points at the target");
      assert_eq!(location.ns.as_ref(), "app.main", "{error}");
      assert_eq!(location.def.as_ref(), "main!", "{error}");
    }

    let entries = load_snippet_entries(
      "let
    body $ fn (x)
      hint-fn $ Fn Number (-> Number)
      , x
  body 3",
    );
    runner::preprocess::ensure_ns_def_compiled(
      &entries.init_ns,
      &entries.init_def,
      &RefCell::new(vec![]),
      &CallStackList::default(),
    )
    .expect("a body hint keeps compiling");
  });
}

#[test]
fn contextual_callback_return_does_not_merge_unrelated_generic_binders() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    for (name, snippet) in [(
      "callback returning unrelated binder B must not satisfy A",
      "let\n    accept $ fn (expected callback)\n      hint-fn $ Fn A\n        Fn $ -> A\n        -> Unit\n        :generics A\n      , &unit\n    caller $ fn (expected other)\n      hint-fn $ Fn A B (-> Unit) (:generics A B)\n      accept expected $ fn () other\n      , &unit\n  caller 1 |wrong",
    )] {
      let entries = load_snippet_entries(snippet);
      let warnings = RefCell::new(vec![]);
      if let Err(error) =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      {
        assert!(
          matches!(
            error.code.as_deref(),
            Some("E_FN_RETURN_UNPROVEN" | "E_ERASED_GENERIC_RELATION" | "E_FN_ARG_TYPE_MISMATCH")
          ),
          "{name}: expected a typed callback error, got: {} [{:?}]",
          error.msg,
          error.code
        );
        continue;
      }
      let warnings = warnings.borrow();
      assert!(
        warnings.iter().any(|warning| {
          warning.code() == Some("E_LOCAL_FN_ARG_TYPE_MISMATCH") || warning.code() == Some("E_FN_RETURN_TYPE_MISMATCH")
        }),
        "{name}: invalid callback passed without a typed diagnostic: {warnings:?}"
      );
    }
  });
}

/// #1655: a `&let`-bound lambda literal has no contextual signature, so each
/// call re-checks its body with the call's argument types.
#[test]
fn let_bound_lambda_body_is_checked_per_call() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let f = "    f $ fn (x)\n      hint-fn $ Fn Number (-> Number)\n      - x 1\n";
    let diagnostics = |body: &str| {
      let entries = load_snippet_entries(&format!("let\n{f}{body}"));
      let warnings = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      let mut found: Vec<String> = warnings
        .borrow()
        .iter()
        .map(|warning| format!("{:?}: {}", warning.code(), warning.message()))
        .collect();
      if let Err(error) = result {
        found.push(format!("{:?}: {}", error.code, error.msg));
      }
      found
    };

    for (label, body) in [
      ("pass through", "    g $ fn (h x) (h x)\n  g f |abc"),
      ("fixed argument", "    apply1 $ fn (h) (h |abc)\n  apply1 f"),
      ("call a Number", "    g $ fn (h x) (h x)\n  g 1 3"),
      (
        "captured callee shadowed at the call",
        "    h f\n    g $ fn (y) (h y)\n  let\n      h $ fn (s) (str s)\n    g |abc",
      ),
    ] {
      let found = diagnostics(body);
      assert!(
        found.iter().any(|item| item.contains("with the argument types of the call to `")),
        "{label}: the call should re-check the lambda body: {found:?}"
      );
    }

    for (label, body) in [
      ("matching argument", "    g $ fn (h x) (h x)\n  g f 3"),
      ("unknown argument", "    g $ fn (h x) (h x)\n  fn (y) (g f y)"),
      ("shadowed name", "    g $ fn (h x) (h x)\n    use $ fn (g) (g f |abc)\n  , use"),
      (
        "caller shadows a captured callee",
        "    h $ fn (s) (str s)\n    g $ fn (y) (h y)\n  let\n      h f\n    g |abc",
      ),
      (
        "captured callee reassigned before the call",
        "    h $ fn (x) (+ x 1)\n    g $ fn (y) (h y)\n  set! h $ fn (s) (str s)\n  g |abc",
      ),
    ] {
      let found = diagnostics(body);
      assert!(
        !found.iter().any(|item| item.contains("with the argument types of the call to `")),
        "{label}: no diagnostic should come from re-checking: {found:?}"
      );
    }

    // Each call reports its own argument types, even at the same body location.
    let found = diagnostics("    g $ fn (h x) (h x)\n  g f |abc\n  g f :tag");
    let rechecked = found
      .iter()
      .filter(|item| item.contains("with the argument types of the call to `g`"))
      .count();
    assert_eq!(rechecked, 2, "both calls should be reported: {found:?}");
  });
}

#[test]
fn contextual_callback_output_does_not_bind_output_only_generic() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_snippet_entries(
      "let\n    consume $ fn (callback expected)\n      hint-fn $ Fn (Fn (-> T)) T (-> T) (:generics T)\n      callback\n  consume (fn () |wrong) 17",
    );
    let warnings = RefCell::new(vec![]);
    let result = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
    let warnings = warnings.borrow();
    assert!(
      result.is_ok(),
      "the invalid callback result should be diagnosed without breaking call preprocessing: {result:?}"
    );
    assert!(
      warnings.iter().any(|warning| {
        warning.code() == Some("E_FN_RETURN_TYPE_MISMATCH")
          && warning
            .message()
            .contains("declares return type `:number`, but body returns `:string`")
      }),
      "the later Number argument should determine T, then the callback's String result should be diagnosed: {warnings:?}"
    );
    assert!(
      !warnings.iter().any(|warning| {
        warning.code() == Some("E_LOCAL_FN_ARG_TYPE_MISMATCH") && warning.message().contains("arg 2 expects type `:string`")
      }),
      "the callback result must not specialize T before the later ordinary argument: {warnings:?}"
    );
  });
}

#[test]
fn contextual_callback_without_input_keeps_output_slot_open() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    builtins::effects::init_effects_states();
    let check_with_main_return = |main_return: CalcitTypeAnnotation| {
      let mut snapshot = snapshot::Snapshot::default();
      let mut file = snapshot::create_file_from_snippet(
        "defn invoke (callback) $ callback\n\ndefn main! () $ invoke $ fn () 1\n\ndefn reload! () &unit",
      )
      .expect("generic callback snippet should parse");
      let output = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
      let callback = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        open_reason: Default::default(),
        cases: Default::default(),
        guard: None,
        runtime_arity: None,
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![],
        return_type: output.clone(),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      })));
      file.defs.get_mut("invoke").expect("invoke definition").schema =
        Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
          open_reason: Default::default(),
          cases: Default::default(),
          guard: None,
          runtime_arity: None,
          generics: Arc::new(vec![Arc::from("T")]),
          where_bounds: Arc::new(vec![]),
          arg_types: vec![callback],
          return_type: output,
          fn_kind: SchemaKind::Fn,
          rest_type: None,
          features: Arc::new(HashSet::new()),
        })));
      for name in ["main!", "reload!"] {
        file.defs.get_mut(name).expect("entry definition").schema =
          Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
            open_reason: Default::default(),
            cases: Default::default(),
            guard: None,
            runtime_arity: None,
            generics: Arc::new(vec![]),
            where_bounds: Arc::new(vec![]),
            arg_types: vec![],
            return_type: Arc::new(if name == "main!" {
              main_return.clone()
            } else {
              CalcitTypeAnnotation::Unit
            }),
            fn_kind: SchemaKind::Fn,
            rest_type: None,
            features: Arc::new(HashSet::new()),
          })));
      }
      snapshot.files.insert("app.main".to_owned(), file);
      let entries = prepare_snapshot_entries(snapshot);
      let warnings = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      (result, warnings.into_inner())
    };

    let (ok_res, ok_warnings) = check_with_main_return(CalcitTypeAnnotation::Number);
    assert!(ok_res.is_ok(), "matching return contract should succeed: {ok_res:?}");
    assert!(
      ok_warnings.is_empty(),
      "an unbound output slot stays open during callback checking and resolves to Number: {ok_warnings:?}"
    );

    let (mismatch_res, mismatch_warnings) = check_with_main_return(CalcitTypeAnnotation::String);
    assert!(
      mismatch_res.is_ok(),
      "mismatch should surface as a return warning: {mismatch_res:?}"
    );
    assert!(
      mismatch_warnings.iter().any(|warning| {
        warning.code() == Some("E_FN_RETURN_TYPE_MISMATCH")
          && warning
            .message()
            .contains("declares return type `:string`, but body returns `:number`")
      }),
      "single zero-arg callback return must not erase T to Dynamic: {mismatch_warnings:?}"
    );
  });
}

#[test]
fn contextual_callback_indirect_callable_keeps_output_slot_open() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_snippet_entries(
      "let\n    invoke $ fn (callback)\n      hint-fn $ Fn (Fn (-> T)) (-> T) (:generics T)\n      callback\n  invoke $ fn () 1",
    );
    let warnings = RefCell::new(vec![]);
    let result = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
    assert!(
      result.is_ok(),
      "an indirect generic callback output slot must remain open: {result:?}"
    );
    assert!(
      warnings.borrow().is_empty(),
      "an indirect unbound output slot must not report a false return mismatch"
    );
  });
}

#[test]
fn contextual_callback_method_owned_output_generic_is_open() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_snippet_entries(
      "let\n    present $ Option :some 3\n    chained $ present .and-then $ fn (value)\n      Option :some $ str value\n  assert-type chained $ quote $ Option String",
    );
    let warnings = RefCell::new(vec![]);
    let result = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
    assert!(
      result.is_ok(),
      "a method-owned output generic should be inferred from its inline callback: {result:?}"
    );
    let warnings = warnings.borrow();
    assert!(
      !warnings.iter().any(|warning| {
        matches!(
          warning.code(),
          Some("E_METHOD_ARG_TYPE_MISMATCH" | "E_FN_RETURN_TYPE_MISMATCH" | "E_LOCAL_FN_ARG_TYPE_MISMATCH")
        )
      }),
      "a valid callback result must not produce a method type diagnostic: {warnings:?}"
    );
  });
}

#[test]
fn contextual_callback_does_not_erase_captured_lexical_output_binder() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_snippet_entries(
      "let\n    outer $ fn (value)\n      hint-fn $ Fn T (-> Unit) (:generics T)\n      let\n          invoke $ fn (callback)\n            hint-fn $ Fn (Fn (-> T)) (-> Unit)\n            , &unit\n        invoke $ fn () 1\n      , &unit\n  outer 2",
    );
    let warnings = RefCell::new(vec![]);
    let result = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
    if let Err(error) = result {
      assert!(
        matches!(
          error.code.as_deref(),
          Some("E_FN_RETURN_UNPROVEN" | "E_ERASED_GENERIC_RELATION" | "E_FN_ARG_TYPE_MISMATCH")
        ),
        "expected a typed callback error, got: {} [{:?}]",
        error.msg,
        error.code
      );
      return;
    }
    let warnings = warnings.borrow();
    assert!(
      warnings.iter().any(|warning| {
        warning.code() == Some("E_LOCAL_FN_ARG_TYPE_MISMATCH") || warning.code() == Some("E_FN_RETURN_TYPE_MISMATCH")
      }),
      "a concrete callback result must not satisfy a captured lexical generic without a typed diagnostic: {warnings:?}"
    );
  });
}

#[test]
fn contextual_callback_preserves_proven_parts_of_composite_output() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_snippet_entries(
      "let\n    check $ fn (key callback)\n      hint-fn $ Fn K (Fn (-> $ Map K V)) (-> Unit) (:generics K V)\n      , &unit\n    bad $ fn ()\n      hint-fn $ Fn\n        -> $ Map String Bool\n      {} $ |wrong true\n  check 1 bad",
    );
    let warnings = RefCell::new(vec![]);
    let result = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
    let warnings = warnings.borrow();
    assert!(
      result.is_ok(),
      "the callback mismatch should be reported without aborting preprocessing: {result:?}"
    );
    assert!(
      warnings
        .iter()
        .any(|warning| warning.message().contains("map<:number") && warning.message().contains("map<:string")),
      "the proven Number key constraint must reject the callback's String-keyed map: {warnings:?}"
    );
  });
}

#[test]
fn contextual_callback_nested_zero_arg_return_preserves_generic_map_and_diagnostics() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();

    // 1. Exact #2864 reproduction plus same-function nested call pass cleanly with zero warnings.
    for (label, snippet) in [
      (
        "direct #2864 reproduction",
        "let\n    frame $ fn (build)\n      hint-fn $ Fn (Fn (-> Value)) (-> Value) (:generics Value)\n      build\n    build $ fn ()\n      hint-fn $ Fn (-> (Map Tag Number))\n      {} (:a 1)\n    use-map $ fn (x)\n      hint-fn $ Fn (Map Tag V) (-> Unit) (:generics V)\n      , &unit\n  use-map $ frame build",
      ),
      (
        "same-function nested zero-arg generic call",
        "let\n    frame $ fn (build)\n      hint-fn $ Fn (Fn (-> Value)) (-> Value) (:generics Value)\n      build\n    build $ fn ()\n      hint-fn $ Fn (-> (Map Tag Number))\n      {} (:a 1)\n    use-map $ fn (x)\n      hint-fn $ Fn (Map Tag V) (-> Unit) (:generics V)\n      , &unit\n  use-map $ frame $ fn () (frame build)",
      ),
    ] {
      let entries = load_snippet_entries(snippet);
      let warnings = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      assert!(result.is_ok(), "{label} should preprocess cleanly: {result:?}");
      assert!(
        warnings.borrow().is_empty(),
        "{label} should produce no warnings: {:?}",
        warnings.borrow()
      );
    }

    // 2. Concrete map assertion still rejects mismatched value type (not widened to Dynamic).
    {
      let entries = load_snippet_entries(
        "let\n    frame $ fn (build)\n      hint-fn $ Fn (Fn (-> Value)) (-> Value) (:generics Value)\n      build\n    build $ fn ()\n      hint-fn $ Fn (-> (Map Tag Number))\n      {} (:a 1)\n    m $ frame build\n  assert-type m (Map Tag String)\n  , &unit",
      );
      let warnings = RefCell::new(vec![]);
      let err = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect_err("assert-type mismatch must fail preprocessing with E_ASSERT_TYPE_MISMATCH");
      assert_eq!(err.code.as_deref(), Some("E_ASSERT_TYPE_MISMATCH"), "{err}");
      assert!(
        err.msg.contains("map<:tag,:number>") && err.msg.contains("map<:tag,:string>"),
        "assert-type must see the preserved Map<Tag, Number>, got: {err}"
      );
    }

    // 3. Explicit Fn (-> Dynamic) still reports E_ERASED_GENERIC_RELATION into Map<Tag, V>.
    {
      let entries = load_snippet_entries(
        "let\n    frame $ fn (build)\n      hint-fn $ Fn (Fn (-> Value)) (-> Value) (:generics Value)\n      build\n    dyn-build $ fn ()\n      hint-fn $ Fn (-> Dynamic)\n      {} (:a 1)\n    use-map $ fn (x)\n      hint-fn $ Fn (Map Tag V) (-> Unit) (:generics V)\n      , &unit\n  use-map $ frame dyn-build",
      );
      let warnings = RefCell::new(vec![]);
      let err = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect_err("explicit Dynamic callback must still fail E_ERASED_GENERIC_RELATION");
      assert_eq!(err.code.as_deref(), Some("E_ERASED_GENERIC_RELATION"), "{err}");
    }

    // 4. Named 1-arg callback contradicting ordinary argument reports arg 1 mismatch, not arg 2.
    {
      let entries = load_snippet_entries(
        "let\n    consume1 $ fn (cb expected)\n      hint-fn $ Fn (Fn Number (-> T)) T (-> T) (:generics T)\n      cb 1\n    bad $ fn (n)\n      hint-fn $ Fn Number (-> String)\n      , |wrong\n  consume1 bad 17",
      );
      let warnings = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      assert!(result.is_ok(), "mismatch should surface cleanly: {result:?}");
      let warnings = warnings.borrow();
      assert!(
        warnings
          .iter()
          .any(|w| { w.code() == Some("E_LOCAL_FN_ARG_TYPE_MISMATCH") && w.message().contains("arg 1") }),
        "expected arg 1 mismatch on bad callback, got: {warnings:?}"
      );
      assert!(
        !warnings.iter().any(|w| w.message().contains("arg 2")),
        "ordinary argument 2 must win over deferred callback return: {warnings:?}"
      );
    }

    // 5. Named conflicting sibling zero-arg callbacks must not mask each other.
    {
      let entries = load_snippet_entries(
        "let\n    pick $ fn (flag a b)\n      hint-fn $ Fn Bool (Fn (-> Value)) (Fn (-> Value)) (-> Value) (:generics Value)\n      if flag (a) (b)\n    a $ fn ()\n      hint-fn $ Fn (-> Number)\n      , 1\n    b $ fn ()\n      hint-fn $ Fn (-> String)\n      , |bad\n  pick true a b",
      );
      let warnings = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      assert!(result.is_ok(), "mismatch should surface cleanly: {result:?}");
      assert!(
        warnings.borrow().iter().any(|w| w.code() == Some("E_LOCAL_FN_ARG_TYPE_MISMATCH")),
        "conflicting sibling zero-arg callbacks must report E_LOCAL_FN_ARG_TYPE_MISMATCH, got: {:?}",
        warnings.borrow()
      );
    }

    // 6. Conflicting sibling callbacks with shared Map<K, V> keep strict V conflict check.
    {
      let entries = load_snippet_entries(
        "let\n    check2 $ fn (key left right)\n      hint-fn $ Fn K (Fn (-> $ Map K V)) (Fn (-> $ Map K V)) (-> Unit) (:generics K V)\n      , &unit\n    good $ fn ()\n      hint-fn $ Fn (-> $ Map Number Bool)\n      {} (1 true)\n    bad $ fn ()\n      hint-fn $ Fn (-> $ Map Number String)\n      {} (1 |bad)\n  check2 1 good bad",
      );
      let warnings = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      assert!(result.is_ok(), "mismatch should surface cleanly: {result:?}");
      assert!(
        warnings.borrow().iter().any(|w| w.code() == Some("E_LOCAL_FN_ARG_TYPE_MISMATCH")),
        "sibling callback map conflict must report E_LOCAL_FN_ARG_TYPE_MISMATCH, got: {:?}",
        warnings.borrow()
      );
    }

    // 7. Zero-arg callback satisfies or fails a :where trait bound on its return generic.
    {
      let entries = load_snippet_entries(
        "let\n    need-countable $ fn (build)\n      hint-fn $ Fn (Fn (-> Value)) (-> Unit) (:generics Value) (:where $ Value Countable)\n      , &unit\n    make-map $ fn ()\n      hint-fn $ Fn (-> (Map Tag Number))\n      {} (:a 1)\n  need-countable make-map",
      );
      let warnings = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      assert!(result.is_ok(), "valid trait-bounded callback should succeed: {result:?}");
      assert!(
        warnings.borrow().is_empty(),
        "Map<Tag, Number> satisfies Countable without warnings: {:?}",
        warnings.borrow()
      );
    }
    {
      let entries = load_snippet_entries(
        "let\n    need-countable $ fn (build)\n      hint-fn $ Fn (Fn (-> Value)) (-> Unit) (:generics Value) (:where $ Value Countable)\n      , &unit\n    make-number $ fn ()\n      hint-fn $ Fn (-> Number)\n      , 1\n  need-countable make-number",
      );
      let warnings = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      assert!(result.is_ok(), "where bound failure reports as a warning: {result:?}");
      assert!(
        warnings.borrow().iter().any(|w| w.code() == Some("E_GENERIC_WHERE_BOUND_MISMATCH")),
        "Number return from zero-arg callback must fail Countable bound: {:?}",
        warnings.borrow()
      );
    }
  });
}

#[test]
fn strict_type_fail_reachable_unbound_slot_reports_stable_error_code() {
  run_with_large_stack(|| {
    let entries = load_fixture_entries("calcit/type-fail/type-slot-unbound-strict.cirru");
    let _strict = StrictTypesReset::enabled();
    let err = run_check_only(&entries).expect_err("reachable unbound type slot should fail strict check-only");

    assert!(err.contains("E_UNBOUND_TYPE_SLOT"), "unexpected strict type-slot error: {err}");
    assert!(err.contains("*payload"), "slot name should be actionable: {err}");
    assert!(
      err.contains("schema.args.0"),
      "schema path should identify the unresolved slot: {err}"
    );
  });
}

#[test]
fn strict_type_fail_reachable_whole_dynamic_schema_reports_stable_error_code() {
  run_with_large_stack(|| {
    let entries = load_fixture_entries("calcit/type-fail/whole-dynamic-schema-strict.cirru");
    let _strict = StrictTypesReset::enabled();
    let err = run_check_only(&entries).expect_err("reachable whole-Dynamic schema should fail strict check-only");

    assert!(
      err.contains("E_WHOLE_DYNAMIC_PUBLIC_SCHEMA"),
      "unexpected strict whole-Dynamic error: {err}"
    );
    assert!(
      err.contains("whole-Dynamic function schema"),
      "schema intent should be explicit: {err}"
    );
    assert!(err.contains("at schema"), "schema root path should be actionable: {err}");
    assert!(
      err.contains("structured `Fn` schema"),
      "replacement contract should be named: {err}"
    );
  });
}

#[test]
fn strict_mode_accepts_macro_generated_closures_without_public_schemas() {
  run_with_large_stack(|| {
    let main_schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      open_reason: Default::default(),
      cases: Default::default(),
      guard: None,
      runtime_arity: None,
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: Arc::new(CalcitTypeAnnotation::Number),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));
    let entries = load_snippet_entries_with_main_schema("defn main! ()\n  if-let (x $ Option :some 1) x 0", Some(main_schema));
    let _strict = StrictTypesReset::enabled();

    run_check_only(&entries).expect("strict mode should accept the internal closure generated by if-let");
  });
}

#[test]
fn strict_mode_rejects_source_definition_reached_from_macro_expansion_without_public_schema() {
  run_with_large_stack(|| {
    let main_schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      open_reason: Default::default(),
      cases: Default::default(),
      guard: None,
      runtime_arity: None,
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: Arc::new(CalcitTypeAnnotation::Number),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));
    let entries = load_snippet_entries_with_main_schema(
      "defn independent-source (x) x\n\ndefmacro call-independent ()\n  quasiquote $ independent-source 1\n\ndefn main! ()\n  call-independent",
      Some(main_schema),
    );
    let _strict = StrictTypesReset::enabled();

    let err =
      run_check_only(&entries).expect_err("a source definition reached through a macro expansion must still declare a public schema");
    assert!(err.contains("E_WHOLE_DYNAMIC_PUBLIC_SCHEMA"), "unexpected strict error: {err}");
    assert!(err.contains("independent-source"), "error should name the source definition: {err}");
  });
}

#[test]
fn strict_mode_keeps_source_optional_parameter_validation_under_macro_ancestry() {
  run_with_large_stack(|| {
    let main_schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      open_reason: Default::default(),
      cases: Default::default(),
      guard: None,
      runtime_arity: None,
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: Arc::new(CalcitTypeAnnotation::Number),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));
    let entries = load_snippet_entries_with_main_schema(
      "defn independent-source (x ? y) x\n\ndefmacro call-independent ()\n  quasiquote $ independent-source 1\n\ndefn main! ()\n  call-independent",
      Some(main_schema),
    );
    let _strict = StrictTypesReset::enabled();

    let err = run_check_only(&entries).expect_err("source-owned optional parameters must stay invalid under macro ancestry");
    assert!(err.contains("E_LEGACY_OPTIONAL_PARAM"), "unexpected strict error: {err}");
  });
}

#[test]
fn if_join_preserves_option_payload_for_arithmetic_checks() {
  run_with_large_stack(|| {
    let entries =
      load_snippet_entries("defn bad () :number\n  let\n      x $ if (> 1 2) 0 (min ([] 1 2))\n    * 10 x\n\ndefn main! ()\n  bad\n");

    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, "bad", &warnings, &CallStackList::default())
      .expect("the Option-tainted arithmetic body should preprocess without a hard failure");
    assert!(
      warnings
        .borrow()
        .iter()
        .any(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH") && warning.message().contains("Option")),
      "expected an Option payload mismatch diagnostic: {:?}",
      warnings.borrow()
    );
  });
}

#[test]
fn non_diverging_if_branch_still_cannot_enter_an_option_contract() {
  run_with_large_stack(|| {
    let entries =
      load_snippet_entries("defn main! ()\n  let\n      value $ if (> 1 0) (Option :some 1) |wrong\n    option:unwrap value");
    let _strict = StrictTypesReset::enabled();
    let error = run_check_only(&entries).expect_err("a returning String branch must not be treated like raise");
    assert!(error.contains("E_DYNAMIC_NOMINAL_ARGUMENT"), "unexpected error: {error}");
  });
}

#[test]
fn defimpl_rejects_legacy_tag_arguments() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("defn main! ()\n  defimpl :LegacyImpl :LegacyTrait\n    .dummy $ fn (x) x");

    let err = run_check_only(&entries).expect_err("legacy tag defimpl must be rejected");
    assert!(err.contains("E_LEGACY_DEFIMPL_TAG"), "unexpected error: {err}");
  });
}

#[test]
fn defimpl_rejects_impl_name_as_first_argument() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "def Sized $ deftrait Sized $ .size $ Fn Self (-> Number)\ndefstruct Box (:n Number)\ndef BoxSizedImpl $ defimpl BoxSizedImpl Sized\n  .size $ fn (self) 1\ndefn main! () (Box :n 1)",
    );

    let warnings = RefCell::new(vec![]);
    let err = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, "BoxSizedImpl", &warnings, &CallStackList::default())
      .expect_err("defimpl naming an impl instead of a type must be rejected");
    assert!(err.to_string().contains("E_LEGACY_DEFIMPL_NAME"), "unexpected error: {err}");
  });
}

#[test]
fn top_level_impl_traits_is_rejected() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "def Sized $ deftrait Sized $ .size $ Fn Self (-> Number)\ndefstruct Box0 (:n Number)\ndef SizedImpl $ defimpl Box0 Sized\n  .size $ fn (self) 1\ndef Box $ impl-traits Box0 SizedImpl\ndefn main! () (Box :n 1)",
    );

    let warnings = RefCell::new(vec![]);
    let err = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, "Box", &warnings, &CallStackList::default())
      .expect_err("top-level impl-traits must be rejected");
    let collected = warnings
      .borrow()
      .iter()
      .map(|warning| warning.to_string())
      .collect::<Vec<_>>()
      .join("\n");
    assert!(
      err.to_string().contains("E_LEGACY_IMPL_TRAITS") || collected.contains("E_LEGACY_IMPL_TRAITS"),
      "unexpected error: {err}\n{collected}"
    );
  });
}

#[test]
fn top_level_qualified_impl_traits_is_rejected() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "def Sized $ deftrait Sized $ .size $ Fn Self (-> Number)\ndefstruct Box0 (:n Number)\ndef SizedImpl $ defimpl Box0 Sized\n  .size $ fn (self) 1\ndef Box $ calcit.core/impl-traits Box0 SizedImpl\ndefn main! () (Box :n 1)",
    );

    let warnings = RefCell::new(vec![]);
    let err = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, "Box", &warnings, &CallStackList::default())
      .expect_err("top-level impl-traits must be rejected");
    let collected = warnings
      .borrow()
      .iter()
      .map(|warning| warning.to_string())
      .collect::<Vec<_>>()
      .join("\n");
    assert!(
      err.to_string().contains("E_LEGACY_IMPL_TRAITS") || collected.contains("E_LEGACY_IMPL_TRAITS"),
      "unexpected error: {err}\n{collected}"
    );
  });
}

#[test]
fn public_check_reaches_unused_definitions_without_changing_entry_check_semantics() {
  run_with_large_stack(|| {
    builtins::effects::init_effects_states();
    let mut snapshot = snapshot::Snapshot::default();
    snapshot
      .entries
      .get_mut(snapshot::DEFAULT_ENTRY_NAME)
      .expect("default entry")
      .target = Some(snapshot::SnapshotTarget::Node);
    let mut file = snapshot::create_file_from_snippet(concat!(
      "defn main! () &unit\n\n",
      "defn reload! () &unit\n\n",
      "defn unused-bad ()\n  + 1 |bad",
    ))
    .expect("public-check snippet should parse");
    let fn_schema = |return_type| {
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        open_reason: Default::default(),
        cases: Default::default(),
        guard: None,
        runtime_arity: None,
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![],
        return_type: Arc::new(return_type),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      })))
    };
    file.defs.get_mut("main!").expect("main entry").schema = fn_schema(CalcitTypeAnnotation::Unit);
    file.defs.get_mut("reload!").expect("reload entry").schema = fn_schema(CalcitTypeAnnotation::Unit);
    file.defs.get_mut("unused-bad").expect("unused entry").schema = fn_schema(CalcitTypeAnnotation::Number);
    snapshot.files.insert("app.main".to_owned(), file);

    let project_namespaces = HashSet::from(["app.main".to_owned()]);
    let entries = prepare_snapshot_entries(snapshot.clone());
    let _strict = StrictTypesReset::enabled();
    run_check_only(&entries).expect("ordinary entry check must retain reachability semantics");

    let error = public_api_check::run(
      &calcit::cli_args::CheckPublicCommand {
        ns: vec!["app.main".to_owned()],
        format: "json".to_owned(),
        deps: false,
        summary_only: false,
      },
      &snapshot,
      &project_namespaces,
      None,
    )
    .expect_err("public check must preprocess an unused invalid definition");
    assert!(error.contains("2 passed"), "unexpected public-check outcome: {error}");
  });
}

#[test]
fn public_check_runs_default_proof_audit_after_ordinary_check() {
  run_with_large_stack(|| {
    builtins::effects::init_effects_states();
    let mut snapshot = snapshot::Snapshot::default();
    snapshot
      .entries
      .get_mut(snapshot::DEFAULT_ENTRY_NAME)
      .expect("default entry")
      .target = Some(snapshot::SnapshotTarget::Node);
    let mut file = snapshot::create_file_from_snippet(concat!(
      "defn main! () &unit\n\n",
      "defn reload! () &unit\n\n",
      "defn as-number (x) x",
    ))
    .expect("public-check snippet should parse");
    let fn_schema = |arg_types, return_type| {
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        open_reason: Default::default(),
        cases: Default::default(),
        guard: None,
        runtime_arity: None,
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types,
        return_type: Arc::new(return_type),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      })))
    };
    file.defs.get_mut("main!").expect("main entry").schema = fn_schema(vec![], CalcitTypeAnnotation::Unit);
    file.defs.get_mut("reload!").expect("reload entry").schema = fn_schema(vec![], CalcitTypeAnnotation::Unit);
    file.defs.get_mut("as-number").expect("unproven entry").schema =
      fn_schema(vec![calcit::calcit::DYNAMIC_TYPE.clone()], CalcitTypeAnnotation::Number);
    snapshot.files.insert("app.main".to_owned(), file);

    // #2506: the ordinary pass accepts the declared return; the default proof
    // audit, as in plain `--check-only`, rejects it for the selected definition.
    let project_namespaces = HashSet::from(["app.main".to_owned()]);
    let entries = prepare_snapshot_entries(snapshot.clone());
    let _strict = StrictTypesReset::enabled();
    // The ordinary pass alone accepts the definition, so a public-check
    // failure below can only come from the proof audit.
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, "as-number", &warnings, &CallStackList::default())
      .expect("the ordinary pass accepts a declared return");
    assert!(
      warnings.borrow().is_empty(),
      "unexpected ordinary warnings: {:?}",
      warnings.borrow()
    );
    let error = public_api_check::run(
      &calcit::cli_args::CheckPublicCommand {
        ns: vec!["app.main".to_owned()],
        format: "json".to_owned(),
        deps: false,
        summary_only: false,
      },
      &snapshot,
      &project_namespaces,
      None,
    )
    .expect_err("public check must run the default proof audit");
    assert!(error.contains("2 passed"), "unexpected public-check outcome: {error}");
  });
}

#[test]
fn strict_mode_runs_definition_tests_with_generated_function_schemas() {
  run_with_large_stack(|| {
    builtins::effects::init_effects_states();
    injection::inject_platform_apis();
    let namespace = format!("app.strict-definition-test-{}", std::process::id());
    let mut project = snapshot::Snapshot::default();
    let mut file = snapshot::create_file_from_snippet(&format!(
      "ns {namespace}\n  :require\n    calcit.test :refer $ is=\n\ndefn checked () true"
    ))
    .expect("test project should parse");
    let checked = file.defs.get_mut("checked").expect("checked should exist");
    checked.schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      open_reason: Default::default(),
      cases: Default::default(),
      guard: None,
      runtime_arity: None,
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: Arc::new(CalcitTypeAnnotation::Bool),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));
    checked.tests.push(snapshot::TestEntry {
      name: "core-assert-equality".to_owned(),
      code: Cirru::List(vec![Cirru::leaf("is="), Cirru::leaf("1"), Cirru::leaf("1")]),
      tags: HashSet::new(),
    });
    project.files.insert(namespace.clone(), file);
    for (core_ns, core_file) in &cached_core_snapshot().files {
      project.files.insert(core_ns.clone(), core_file.clone());
    }
    let project_namespaces = HashSet::from([namespace]);
    runner::preprocess::set_project_namespaces(&project_namespaces);
    let options = TestCommand {
      target: None,
      name: None,
      tag: vec![],
      exclude_tag: vec![],
      affected: vec![],
      list: false,
      fail_fast: false,
      require_match: false,
      summary_only: false,
      format: "human".to_owned(),
    };
    let _strict = StrictTypesReset::enabled();

    run_tests(&options, &project, &project_namespaces).expect(
      "strict mode should preserve trusted core macro provenance while executing a definition test through a typed synthetic wrapper",
    );
  });
}

#[test]
fn strict_type_fail_dynamic_nominal_argument_reports_decoder_migration() {
  run_with_large_stack(|| {
    builtins::effects::init_effects_states();
    let mut snapshot = snapshot::Snapshot::default();
    let mut file = snapshot::create_file_from_snippet(concat!(
      "defstruct Person $ :name 'String\n\n",
      "defn open-value () $ {} (:name |Ada)\n\n",
      "defn consume-person (person)\n  :name person\n\n",
      "defn main! ()\n  consume-person\n    open-value\n\n",
      "defn reload! () $ identity &unit",
    ))
    .expect("dynamic nominal boundary snippet should parse");
    let person = Arc::new(calcit::calcit::CalcitStructDef {
      field_defaults: Default::default(),
      definition_ref: None,
      name: cirru_edn::EdnTag::new("Person"),
      fields: Arc::new(vec![cirru_edn::EdnTag::new("name")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::String)]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let fn_schema = |args, return_type| {
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
        open_reason: Default::default(),
        cases: Default::default(),
        guard: None,
        runtime_arity: None,
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: args,
        return_type,
        fn_kind: calcit::calcit::SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      })))
    };
    file.defs.get_mut("open-value").expect("open-value entry").schema = fn_schema(vec![], Arc::new(CalcitTypeAnnotation::Dynamic));
    file.defs.get_mut("consume-person").expect("consume-person entry").schema = fn_schema(
      vec![Arc::new(CalcitTypeAnnotation::Struct(person, Arc::new(vec![])))],
      Arc::new(CalcitTypeAnnotation::String),
    );
    file.defs.get_mut("main!").expect("main entry").schema = fn_schema(vec![], Arc::new(CalcitTypeAnnotation::String));
    file.defs.get_mut("reload!").expect("reload entry").schema = fn_schema(vec![], Arc::new(CalcitTypeAnnotation::Unit));
    snapshot.files.insert("app.main".to_owned(), file);
    let entries = prepare_snapshot_entries(snapshot);
    let _strict = StrictTypesReset::enabled();
    let err = run_check_only(&entries).expect_err("Dynamic data must not enter a closed nominal argument in strict mode");

    assert!(
      err.contains("E_DYNAMIC_NOMINAL_ARGUMENT"),
      "unexpected strict boundary error: {err}"
    );
    assert!(err.contains("argument 1"), "argument position should be actionable: {err}");
    assert!(err.contains("struct Person"), "closed nominal target should be explicit: {err}");
    assert!(err.contains("parse-cirru-edn-as"), "text decoder migration should be named: {err}");
    assert!(
      err.contains("decode-map-as"),
      "evaluated-value decoder migration should be named: {err}"
    );
  });
}

#[test]
fn strict_type_fail_dynamic_nominal_method_reports_stable_error_code() {
  run_with_large_stack(|| {
    let entries = load_fixture_entries("calcit/type-fail/dynamic-nominal-method-strict.cirru");
    let _strict = StrictTypesReset::enabled();
    let err = run_check_only(&entries).expect_err("Dynamic nominal method dispatch should fail strict check-only");

    assert!(err.contains("E_DYNAMIC_POSTFIX_METHOD"), "unexpected strict dispatch error: {err}");
    assert!(err.contains("`.unwrap-or`"), "method should be explicit: {err}");
    assert!(
      err.contains("statically known Option or Result schema"),
      "receiver migration should be actionable: {err}"
    );
    assert!(
      err.contains("`option:*` or `result:*`"),
      "explicit-boundary alternative should be named: {err}"
    );
  });
}

#[test]
fn strict_type_fail_general_dynamic_method_reports_receiver_source() {
  run_with_large_stack(|| {
    let fixture = "calcit/type-fail/dynamic-method-dispatch-strict.cirru";
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries(fixture);
    let err = run_check_only(&entries).expect_err("ordinary method dispatch on Dynamic must fail strict check-only");

    assert!(err.contains("E_DYNAMIC_POSTFIX_METHOD"), "unexpected strict dispatch error: {err}");
    assert!(err.contains("`.custom`"), "method should be explicit: {err}");
    assert!(
      err.contains("receiver schema that resolves to Dynamic"),
      "receiver source should be classified: {err}"
    );
    assert!(
      err.contains("narrow or validate the receiver"),
      "migration should be actionable: {err}"
    );
  });
}

#[test]
fn strict_type_fail_raw_primitive_reports_stable_error_code() {
  run_with_large_stack(|| {
    let fixture = "calcit/type-fail/raw-primitive-strict.cirru";
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries(fixture);
    let err = run_check_only(&entries).expect_err("hand-written raw primitive must fail strict check-only");

    assert!(err.contains("E_RAW_PRIMITIVE_IN_TYPED_CODE"), "unexpected strict raw error: {err}");
    assert!(err.contains("`&get-raw`"), "primitive should be explicit: {err}");
    assert!(
      err.contains("typed collection lookup returning Option<T>"),
      "migration should point to the typed public API: {err}"
    );
  });
}

#[test]
fn strict_type_fail_unsupported_indexed_receiver_reports_stable_error_code() {
  run_with_large_stack(|| {
    builtins::effects::init_effects_states();
    let mut snapshot = snapshot::Snapshot::default();
    let mut file = snapshot::create_file_from_snippet(concat!(
      "defn read-first (value)\n  first value\n\n",
      "defn main! ()\n  read-first 1\n\n",
      "defn reload! () $ identity &unit",
    ))
    .expect("unsupported indexed receiver snippet should parse");
    let fn_schema = |args, return_type| {
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
        open_reason: Default::default(),
        cases: Default::default(),
        guard: None,
        runtime_arity: None,
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: args,
        return_type,
        fn_kind: calcit::calcit::SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      })))
    };
    file.defs.get_mut("read-first").expect("read-first entry").schema =
      fn_schema(vec![Arc::new(CalcitTypeAnnotation::Number)], calcit::calcit::DYNAMIC_TYPE.clone());
    file.defs.get_mut("main!").expect("main entry").schema = fn_schema(vec![], calcit::calcit::DYNAMIC_TYPE.clone());
    file.defs.get_mut("reload!").expect("reload entry").schema = fn_schema(vec![], Arc::new(CalcitTypeAnnotation::Unit));
    snapshot.files.insert("app.main".to_owned(), file);

    let entries = prepare_snapshot_entries(snapshot);
    let _strict = StrictTypesReset::enabled();
    let err = run_check_only(&entries).expect_err("Number receiver must fail strict indexed access during full preprocessing");

    assert!(
      err.contains("E_UNSUPPORTED_INDEXED_RECEIVER"),
      "unexpected strict indexed receiver error: {err}"
    );
    assert!(err.contains("`first`"), "operation should be explicit: {err}");
    assert!(err.contains("`:number`"), "resolved receiver type should be explicit: {err}");
    assert!(
      err.contains("List<T>, String, or Enum"),
      "supported receivers should be named: {err}"
    );
  });
}

#[test]
fn strict_type_fail_untyped_js_object_access_reports_stable_error_code() {
  run_with_large_stack(|| {
    let fixture = "calcit/type-fail/untyped-js-object-access-strict.cirru";
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries(fixture);
    let err = run_check_only(&entries).expect_err("literal access on a bare JsObject must fail strict check-only");

    assert!(
      err.contains("E_UNTYPED_JS_OBJECT_ACCESS"),
      "unexpected strict JS object access error: {err}"
    );
    assert!(err.contains(".-length"), "operation should be explicit: {err}");
    assert!(err.contains("inferred `JsObject`"), "receiver evidence should be explicit: {err}");
    assert!(err.contains("external-object trait"), "migration should be actionable: {err}");
  });
}

#[test]
fn strict_type_fail_unbound_trait_field_tag_key_reports_stable_error_code() {
  run_with_large_stack(|| {
    let fixture = "calcit/type-fail/unbound-trait-field-tag-key-strict.cirru";
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries(fixture);
    let err = run_check_only(&entries).expect_err("a Tag key on a type-variable receiver must fail strict check-only");

    assert!(
      err.contains("E_UNTYPED_JS_OBJECT_ACCESS"),
      "unexpected strict type-variable host key error: {err}"
    );
    assert!(err.contains("`js-set`"), "operation should be explicit: {err}");
    assert!(
      err.contains("type variable `MissingStyle`"),
      "receiver evidence should be explicit: {err}"
    );
  });
}

#[test]
fn strict_type_fail_js_nullish_dereference_reports_stable_error_code() {
  run_with_large_stack(|| {
    let fixture = "calcit/type-fail/js-nullish-dereference-strict.cirru";
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries(fixture);
    let err = run_check_only(&entries).expect_err("direct dereference of a JsNullish host value must fail strict check-only");

    assert!(
      err.contains("E_JS_FFI_NULLABLE_DEREF"),
      "unexpected nullable dereference error: {err}"
    );
    assert!(err.contains(".-length"), "operation should be explicit: {err}");
    assert!(err.contains("js-present?"), "narrowing migration should be actionable: {err}");
  });
}

#[test]
fn strict_type_fail_js_nullish_predicate_reports_stable_error_code() {
  run_with_large_stack(|| {
    let fixture = "calcit/type-fail/js-nullish-predicate-strict.cirru";
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries(fixture);
    let err = run_check_only(&entries).expect_err("legacy nil? on a JsNullish host value must fail strict check-only");

    assert!(
      err.contains("E_JS_FFI_NULLABLE_PREDICATE"),
      "unexpected nullable predicate error: {err}"
    );
    assert!(err.contains("`nil?`"), "legacy predicate should be explicit: {err}");
    assert!(err.contains("js-nullish?"), "predicate migration should be actionable: {err}");
  });
}

#[test]
fn strict_type_fail_host_value_equality_reports_stable_warning_code() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries("calcit/type-fail/host-value-equality-strict.cirru");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("host equality misuse should preprocess with warnings");
    let warnings = warnings.borrow();
    let host_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_HOST_VALUE_EQUALITY"))
      .map(|warning| warning.message().to_owned())
      .collect::<Vec<_>>();
    for (definition, position) in [
      ("same-counter?", "compares arg 1 by value"),
      ("same-counter?", "compares arg 2 by value"),
      ("host-member", "hashes arg 1 as a Set member"),
      ("counter-key", "hashes arg 1 as a Map key"),
    ] {
      assert!(
        host_warnings
          .iter()
          .any(|message| message.contains(position) && message.contains(&format!("main/{definition}"))),
        "missing `{position}` diagnostic for {definition}: {host_warnings:?}"
      );
    }
    assert_eq!(
      host_warnings.len(),
      4,
      "plain JsObject and explicit identity must stay accepted: {host_warnings:?}"
    );
    assert!(
      host_warnings.iter().all(|message| message.contains("identical?")),
      "identity migration should be actionable: {host_warnings:?}"
    );
  });
}

#[test]
fn strict_type_fail_trait_requires_reports_declaration_and_attachment_codes() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    for (fixture, code, detail) in [
      ("trait-requires-cycle-strict", "E_TRAIT_REQUIRES", "trait requires cycle"),
      (
        "trait-requires-member-conflict-strict",
        "E_TRAIT_REQUIRES",
        "member `label` is declared by both",
      ),
      (
        "trait-requires-not-trait-strict",
        "E_TRAIT_REQUIRES",
        "which is not a trait definition",
      ),
      (
        "trait-requires-kind-mismatch-strict",
        "E_TRAIT_REQUIRES",
        "is an external-object trait but requires",
      ),
      (
        "impl-missing-required-trait-strict",
        "E_IMPL_MISSING_REQUIRED_TRAIT",
        "requires an impl of trait",
      ),
    ] {
      let entries = load_fixture_entries(&format!("calcit/type-fail/{fixture}.cirru"));
      let err = run_check_only(&entries).expect_err(&format!("{fixture} must fail strict check-only"));
      assert!(err.contains(code), "{fixture} should report {code}: {err}");
      assert!(err.contains(detail), "{fixture} should explain `{detail}`: {err}");
    }
  });
}

#[test]
fn strict_type_fail_trait_coherence_duplicate_and_local_requires() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _strict = StrictTypesReset::enabled();
    for (name, snippet, code, detail) in [
      (
        "duplicate-struct-def",
        "def ShowT $ deftrait ShowT $ .show $ Fn Self (-> String)\ndefstruct Box0 (:n Number)\ndef impl1 $ defimpl Box0 ShowT $ .show $ fn (x) |a\ndef impl2 $ defimpl Box0 ShowT $ .show $ fn (x) |b\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((B (impl-traits Box0 impl1 impl2))) &unit",
        "E_DUPLICATE_TRAIT_IMPL",
        "duplicate implementation of trait",
      ),
      (
        "duplicate-enum-def",
        "def ShowT $ deftrait ShowT $ .show $ Fn Self (-> String)\ndefenum Choice0 (:n Number)\ndef impl1 $ defimpl Choice0 ShowT $ .show $ fn (x) |a\ndef impl2 $ defimpl Choice0 ShowT $ .show $ fn (x) |b\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((C (impl-traits Choice0 impl1 impl2))) &unit",
        "E_DUPLICATE_TRAIT_IMPL",
        "duplicate implementation of trait",
      ),
      (
        "duplicate-typeref-param",
        "def ShowT $ deftrait ShowT $ .show $ Fn Self (-> String)\ndefstruct Box (:n Number) (:impls ShowT)\ndefimpl Box ShowT $ .show $ fn (x) |a\ndefn attach-again (b)\n  hint-fn $ Fn Box (-> Box)\n  let ((impl2 (defimpl Box ShowT $ .show $ fn (x) |b))) (impl-traits b impl2)\ndefn main! () (do (attach-again (Box :n 1)) &unit)",
        "E_DUPLICATE_TRAIT_IMPL",
        "duplicate implementation of trait",
      ),
      (
        "duplicate-nested-typeref-impl-traits",
        "def HelperT $ deftrait HelperT $ .helper $ Fn Self (-> String)\ndefstruct Widget0 (:n Number)\ndef helper-impl $ defimpl Widget0 HelperT $ .helper $ fn (x) |h\ndefn attach-again (w)\n  hint-fn $ Fn Widget0 (-> Widget0)\n  &struct:impl-traits (&struct:impl-traits w helper-impl) helper-impl\ndefn main! () (do (attach-again (Widget0 :n 1)) &unit)",
        "E_DUPLICATE_TRAIT_IMPL",
        "duplicate implementation of trait",
      ),
      (
        "duplicate-chained-impl-traits",
        "def ShowT $ deftrait ShowT $ .show $ Fn Self (-> String)\ndefstruct Box0 (:n Number)\ndef impl1 $ defimpl Box0 ShowT $ .show $ fn (x) |a\ndef impl2 $ defimpl Box0 ShowT $ .show $ fn (x) |b\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((B (impl-traits (impl-traits Box0 impl1) impl2))) &unit",
        "E_DUPLICATE_TRAIT_IMPL",
        "duplicate implementation of trait",
      ),
      (
        "missing-parent-on-struct-value",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndef ChildT $ deftrait ChildT (requires ParentT) $ .greet $ Fn Self (-> String)\ndefstruct Widget0 (:n Number)\ndef child-impl $ defimpl Widget0 ChildT $ .greet $ fn (x) |hi\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((w (&struct:impl-traits (Widget0 :n 1) child-impl))) &unit",
        "E_IMPL_MISSING_REQUIRED_TRAIT",
        "requires an impl of trait",
      ),
      (
        "missing-parent-on-nested-struct-value",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndef HelperT $ deftrait HelperT $ .helper $ Fn Self (-> String)\ndef ChildT $ deftrait ChildT (requires ParentT) $ .greet $ Fn Self (-> String)\ndefstruct Widget0 (:n Number)\ndef helper-impl $ defimpl Widget0 HelperT $ .helper $ fn (x) |h\ndef child-impl $ defimpl Widget0 ChildT $ .greet $ fn (x) |hi\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((w (&struct:impl-traits (&struct:impl-traits (Widget0 :n 1) helper-impl) child-impl))) &unit",
        "E_IMPL_MISSING_REQUIRED_TRAIT",
        "requires an impl of trait",
      ),
      (
        "missing-parent-on-let-bound-struct-value",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndef HelperT $ deftrait HelperT $ .helper $ Fn Self (-> String)\ndef ChildT $ deftrait ChildT (requires ParentT) $ .greet $ Fn Self (-> String)\ndefstruct Widget0 (:n Number)\ndef helper-impl $ defimpl Widget0 HelperT $ .helper $ fn (x) |h\ndef child-impl $ defimpl Widget0 ChildT $ .greet $ fn (x) |hi\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let\n      w1 $ &struct:impl-traits (Widget0 :n 1) helper-impl\n      w2 $ &struct:impl-traits w1 child-impl\n    , &unit",
        "E_IMPL_MISSING_REQUIRED_TRAIT",
        "requires an impl of trait",
      ),
      (
        "imported-parent-alias-member-conflict",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndef ParentAlias ParentT\ndef ChildT $ deftrait ChildT (requires ParentAlias) $ .label $ Fn Self (-> String)\ndefn main! () (do ChildT &unit)",
        "E_TRAIT_REQUIRES",
        "member `label` is declared by both",
      ),
      (
        "missing-parent-on-enum-value",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndef ChildT $ deftrait ChildT (requires ParentT) $ .greet $ Fn Self (-> String)\ndefenum Choice0 (:n Number)\ndef child-impl $ defimpl Choice0 ChildT $ .greet $ fn (x) |hi\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((c (impl-traits (%:: Choice0 :n 1) child-impl))) &unit",
        "E_IMPL_MISSING_REQUIRED_TRAIT",
        "requires an impl of trait",
      ),
      (
        "local-deftrait-missing-parent-impl",
        "defn main! ()\n  hint-fn $ Fn (-> Unit)\n  let\n      LocalParent $ deftrait LocalParent $ .label $ Fn Self (-> String)\n      LocalChild $ deftrait LocalChild (requires LocalParent) $ .greet $ Fn Self (-> String)\n      Box0 $ defstruct Box0 $ :n Number\n      c-impl $ defimpl Box0 LocalChild $ .greet $ fn (x) |hi\n      Box $ impl-traits Box0 c-impl\n    , &unit",
        "E_IMPL_MISSING_REQUIRED_TRAIT",
        "requires an impl of trait",
      ),
      (
        "local-deftrait-member-conflict",
        "defn main! ()\n  hint-fn $ Fn (-> Unit)\n  let\n      LocalParent $ deftrait LocalParent $ .label $ Fn Self (-> String)\n      LocalChild $ deftrait LocalChild (requires LocalParent) $ .label $ Fn Self (-> String)\n    , &unit",
        "E_TRAIT_REQUIRES",
        "member `label` is declared by both",
      ),
      (
        "missing-parent-after-struct-assoc",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndef ChildT $ deftrait ChildT (requires ParentT) $ .greet $ Fn Self (-> String)\ndefstruct Widget0 (:n Number)\ndef child-impl $ defimpl Widget0 ChildT $ .greet $ fn (x) |hi\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((w (&struct:impl-traits (&struct:assoc (Widget0 :n 1) :n 2) child-impl))) &unit",
        "E_IMPL_MISSING_REQUIRED_TRAIT",
        "requires an impl of trait",
      ),
      (
        "duplicate-on-if-joined-typeref",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndefenum Choice (:generics T) (:impls ParentT) (:empty) (:item T)\ndefimpl Choice ParentT $ .label $ fn (x) |ok\ndef extra-parent $ defimpl Choice ParentT $ .label $ fn (x) |dup\ndefn attach-on-if (flag)\n  hint-fn $ Fn Bool (-> (Choice Number))\n  let ((c (if flag (%:: Choice :empty) (%:: Choice :item 1)))) (impl-traits c extra-parent)\ndefn main! () (do (attach-on-if true) &unit)",
        "E_DUPLICATE_TRAIT_IMPL",
        "duplicate implementation of trait",
      ),
    ] {
      let entries = load_snippet_entries(snippet);
      let err = run_check_only(&entries).expect_err(&format!("{name} must fail strict check-only"));
      assert!(err.contains(code), "{name} should report {code}: {err}");
      assert!(err.contains(detail), "{name} should explain `{detail}`: {err}");
    }
  });
}

#[test]
fn strict_type_check_allows_trait_parent_alias_and_decorated_typeref_impl_attachment() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _strict = StrictTypesReset::enabled();
    for (name, snippet) in [
      (
        "parent-alias-satisfied",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndef ParentAlias ParentT\ndef ChildT $ deftrait ChildT (requires ParentAlias) $ .greet $ Fn Self (-> String)\ndefstruct Widget0 (:n Number)\ndef parent-impl $ defimpl Widget0 ParentT $ .label $ fn (x) |ok\ndef child-impl $ defimpl Widget0 ChildT $ .greet $ fn (x) |hi\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((w (impl-traits (Widget0 :n 1) parent-impl child-impl))) &unit",
      ),
      (
        "decorated-typeref-param",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndef ChildT $ deftrait ChildT (requires ParentT) $ .greet $ Fn Self (-> String)\ndefstruct Widget0 (:n Number)\ndef parent-impl $ defimpl Widget0 ParentT $ .label $ fn (x) |ok\ndef child-impl $ defimpl Widget0 ChildT $ .greet $ fn (x) |hi\ndefn attach-child (w)\n  hint-fn $ Fn Widget0 (-> Widget0)\n  &struct:impl-traits w child-impl\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((w (attach-child (&struct:impl-traits (Widget0 :n 1) parent-impl)))) &unit",
      ),
      (
        "nested-decorated-typeref-param",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndef HelperT $ deftrait HelperT $ .helper $ Fn Self (-> String)\ndef ChildT $ deftrait ChildT (requires ParentT) $ .greet $ Fn Self (-> String)\ndefstruct Widget0 (:n Number)\ndef parent-impl $ defimpl Widget0 ParentT $ .label $ fn (x) |ok\ndef helper-impl $ defimpl Widget0 HelperT $ .helper $ fn (x) |h\ndef child-impl $ defimpl Widget0 ChildT $ .greet $ fn (x) |hi\ndefn attach-nested (w)\n  hint-fn $ Fn Widget0 (-> Widget0)\n  let\n      w2 $ &struct:impl-traits (&struct:impl-traits w helper-impl) child-impl\n      _h $ .helper w2\n      _g $ .greet w2\n    , w2\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((w (attach-nested (&struct:impl-traits (Widget0 :n 1) parent-impl)))) &unit",
      ),
      (
        "let-bound-decorated-typeref-param",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndef HelperT $ deftrait HelperT $ .helper $ Fn Self (-> String)\ndef ChildT $ deftrait ChildT (requires ParentT) $ .greet $ Fn Self (-> String)\ndefstruct Widget0 (:n Number)\ndef parent-impl $ defimpl Widget0 ParentT $ .label $ fn (x) |ok\ndef helper-impl $ defimpl Widget0 HelperT $ .helper $ fn (x) |h\ndef child-impl $ defimpl Widget0 ChildT $ .greet $ fn (x) |hi\ndefn attach-let (w)\n  hint-fn $ Fn Widget0 (-> Widget0)\n  let\n      w1 $ &struct:impl-traits w helper-impl\n      w2 $ &struct:impl-traits w1 child-impl\n    , w2\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((w (attach-let (&struct:impl-traits (Widget0 :n 1) parent-impl)))) &unit",
      ),
      (
        "param-struct-assoc-defers-missing-parent",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndef ChildT $ deftrait ChildT (requires ParentT) $ .greet $ Fn Self (-> String)\ndefstruct Widget0 (:n Number)\ndef parent-impl $ defimpl Widget0 ParentT $ .label $ fn (x) |ok\ndef child-impl $ defimpl Widget0 ChildT $ .greet $ fn (x) |hi\ndefn attach-after-assoc (w)\n  hint-fn $ Fn Widget0 (-> Widget0)\n  &struct:impl-traits (&struct:assoc w :n 2) child-impl\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((w (attach-after-assoc (&struct:impl-traits (Widget0 :n 1) parent-impl)))) &unit",
      ),
      (
        "if-joined-typeref-defers-missing-parent",
        "def ParentT $ deftrait ParentT $ .label $ Fn Self (-> String)\ndef ChildT $ deftrait ChildT (requires ParentT) $ .greet $ Fn Self (-> String)\ndefenum Choice (:generics T) (:empty) (:item T)\ndef parent-impl $ defimpl Choice ParentT $ .label $ fn (x) |ok\ndef child-impl $ defimpl Choice ChildT $ .greet $ fn (x) |hi\ndefn attach-on-if (flag)\n  hint-fn $ Fn Bool (-> (Choice Number))\n  let\n      decorated $ impl-traits Choice parent-impl\n      joined $ if flag (decorated :empty) (decorated :item 1)\n    impl-traits joined child-impl\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((c (attach-on-if true))) &unit",
      ),
    ] {
      let entries = load_snippet_entries(snippet);
      run_check_only(&entries).unwrap_or_else(|err| panic!("{name} must pass strict check-only: {err}"));
    }
  });
}

#[test]
fn strict_type_fail_pass_callback_cannot_escape_non_pass_slot() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _strict = StrictTypesReset::enabled();
    for (name, snippet, target_def, expected_code) in [
      (
        "store-pass-into-pure-field",
        "defstruct Handler (:on-click $ Fn Number (-> Unit))\ndefn keep (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> Handler)\n  Handler :on-click cb\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((h (keep (fn (n) &unit)))) &unit",
        "keep",
        "E_FN_ARG_TYPE_MISMATCH",
      ),
      (
        "store-pass-into-io-field",
        "defstruct Handler (:on-click $ Fn Number (-> Unit) (:features :io))\ndefn keep (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> Handler)\n  Handler :on-click cb\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((h (keep (fn (n) &unit)))) &unit",
        "keep",
        "E_FN_ARG_TYPE_MISMATCH",
      ),
      (
        "return-pass-as-pure-fn",
        "defn escape-ret (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> (Fn Number (-> Unit)))\n  , cb\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((f (escape-ret (fn (n) &unit)))) &unit",
        "escape-ret",
        "E_FN_RETURN_TYPE_MISMATCH",
      ),
      (
        "return-pass-as-anyfn",
        "defn escape-anyfn-ret (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> AnyFn)\n  , cb\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((f (escape-anyfn-ret (fn (n) &unit)))) &unit",
        "escape-anyfn-ret",
        "E_FN_RETURN_TYPE_MISMATCH",
      ),
      (
        "pass-into-pure-hof-param",
        "defn consume-pure (cb)\n  hint-fn $ Fn (Fn Number (-> Unit)) (-> Unit)\n  cb 1\ndefn forward-bad (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> Unit)\n  consume-pure cb\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  forward-bad (fn (n) &unit)",
        "forward-bad",
        "E_FN_ARG_TYPE_MISMATCH",
      ),
    ] {
      let entries = load_snippet_entries(snippet);
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, target_def, &warnings, &CallStackList::default())
        .unwrap_or_else(|err| panic!("{name} should report warning during preprocess: {err}"));
      assert!(
        warnings.borrow().iter().any(|warning| warning.code() == Some(expected_code)),
        "{name} should report {expected_code}: {:?}",
        warnings.borrow()
      );
      let entries = load_snippet_entries(snippet);
      run_check_only(&entries).expect_err(&format!("{name} must fail strict check-only"));
    }

    for (name, snippet, target_def, expected_code) in [
      (
        "escape-pass-via-captured-closure-fn",
        "defn retain (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> (Fn Number (-> Unit)))\n  fn (x) (cb x)\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_f (retain (fn (n) &unit)))) &unit",
        "retain",
        "E_FN_RETURN_TYPE_MISMATCH",
      ),
      (
        "escape-pass-via-captured-closure-dyn",
        "defn retain (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> AnyFn)\n  fn (x) (cb x)\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_f (retain (fn (n) &unit)))) &unit",
        "retain",
        "E_FN_RETURN_TYPE_MISMATCH",
      ),
      (
        "escape-pass-via-let-wrapped-closure",
        "defn retain (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> (Fn Number (-> Unit)))\n  let ((wrapped (fn (x) (cb x))))\n    , wrapped\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_f (retain (fn (n) &unit)))) &unit",
        "retain",
        "E_FN_RETURN_TYPE_MISMATCH",
      ),
      (
        "escape-pass-via-hinted-captured-closure",
        "defn retain (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> (Fn Number (-> Unit)))\n  fn (x)\n    hint-fn $ Fn Number (-> Unit)\n    cb x\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_f (retain (fn (n) &unit)))) &unit",
        "retain",
        "E_FN_RETURN_TYPE_MISMATCH",
      ),
      (
        "escape-pass-via-captured-closure-in-struct-fn",
        "defstruct Holder (:f (Fn Number (-> Unit)))\ndefn stash (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> Holder)\n  Holder :f (fn (x) (cb x))\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_h (stash (fn (n) &unit)))) &unit",
        "stash",
        "E_FN_ARG_TYPE_MISMATCH",
      ),
      (
        "escape-pass-via-captured-closure-in-struct-anyfn",
        "defstruct Holder (:f AnyFn)\ndefn stash (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> Holder)\n  Holder :f (fn (x) (cb x))\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_h (stash (fn (n) &unit)))) &unit",
        "stash",
        "E_FN_ARG_TYPE_MISMATCH",
      ),
      (
        "escape-pass-via-captured-closure-non-pass-arg",
        "defn register (f)\n  hint-fn $ Fn (Fn Number (-> Unit)) (-> Unit)\n  , &unit\ndefn relay (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> Unit)\n  register (fn (x) (cb x))\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  relay (fn (n) &unit)",
        "relay",
        "E_FN_ARG_TYPE_MISMATCH",
      ),
    ] {
      let entries = load_snippet_entries(snippet);
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, target_def, &warnings, &CallStackList::default())
        .unwrap_or_else(|err| panic!("{name} should preprocess with warning/error: {err}"));
      assert!(
        warnings.borrow().iter().any(|warning| warning.code() == Some(expected_code)),
        "{name} should report {expected_code}: {:?}",
        warnings.borrow()
      );
      let entries = load_snippet_entries(snippet);
      run_check_only(&entries).expect_err(&format!("{name} must fail strict check-only"));
    }

    let ok_snippet = "defn run-now (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> Unit)\n  cb 1\ndefn forward-ok (xs cb)\n  hint-fn $ Fn (List Number) (Fn Number (-> Unit) (:features :pass)) (-> Unit)\n  run-now cb\n  run-now (fn (x) (cb (+ x 1)))\n  each xs (fn (x) (cb (+ x 1)))\n  let ((wrapped (fn (x) (hint-fn $ Fn Number (-> Unit)) (cb x))))\n    wrapped 1\n  , &unit\ndefn shadow-ok (cb)\n  hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> (Fn Number (-> Number)))\n  fn (cb) (+ cb 1)\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  forward-ok ([] 1 2) (fn (n) &unit)\n  let ((_s (shadow-ok (fn (n) &unit)))) &unit";
    let entries = load_snippet_entries(ok_snippet);
    run_check_only(&entries).unwrap_or_else(|err| panic!("forwarding :pass callback to :pass slots must pass: {err}"));
  });
}

#[test]
fn strict_type_fail_impl_completeness_across_set_if_join_and_pending_defs() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _strict = StrictTypesReset::enabled();
    let base_defs = r#"defstruct Widget0 (:n Number)
deftrait Parent (.label AnyFn)
deftrait Child (requires Parent) (.greet AnyFn)
def ParentImpl (defimpl Widget0 Parent (.label (fn (x) |parent)))
def ChildImpl (defimpl Widget0 Child (.greet (fn (x) |child)))"#;

    let prepare_with_schemas = |snippet: &str, extra_schemas: &[(&str, &str)]| {
      builtins::effects::init_effects_states();
      let mut snapshot = snapshot::Snapshot::default();
      let mut file = snapshot::create_file_from_snippet(snippet).expect("test snippet should parse");
      for (name, schema_src) in [
        ("Parent", "Trait"),
        ("Child", "Trait"),
        ("ParentImpl", "Impl"),
        ("ChildImpl", "Impl"),
        ("Widget0", "StructDef"),
      ]
      .into_iter()
      .chain(extra_schemas.iter().copied())
      {
        if let Some(entry) = file.defs.get_mut(name) {
          let schema = cirru_parser::parse(schema_src)
            .unwrap_or_else(|e| panic!("parse schema for {name}: {e}"))
            .remove(0);
          let schema = match schema {
            cirru_parser::Cirru::List(mut items) if items.len() == 1 => items.remove(0),
            other => other,
          };
          entry.schema =
            snapshot::parse_schema_annotation_for_write(&schema).unwrap_or_else(|e| panic!("parse annotation for {name}: {e}"));
        }
      }
      snapshot.files.insert("app.main".to_owned(), file);
      prepare_snapshot_entries(snapshot)
    };

    // 1. `set!` preserves impl incompleteness when overwriting with an incomplete parameter or differing impls.
    let set_incomplete_snippet = format!(
      "{base_defs}\n{}",
      r#"defn check-set-incomplete (w)
  let ((local (Widget0 :n 1)))
    set! local w
    impl-traits local ChildImpl
defn check-set-impl-mismatch ()
  let ((local (Widget0 :n 1)) (decorated (impl-traits (Widget0 :n 2) ParentImpl)))
    set! local decorated
    impl-traits local ChildImpl
defn main! () &unit"#
    );
    let entries = prepare_with_schemas(
      &set_incomplete_snippet,
      &[
        ("check-set-incomplete", "Fn Widget0 (-> Widget0)"),
        ("check-set-impl-mismatch", "Fn (-> Widget0)"),
        ("main!", "Fn (-> Unit)"),
      ],
    );
    for target in ["check-set-incomplete", "check-set-impl-mismatch"] {
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, target, &warnings, &CallStackList::default())
        .unwrap_or_else(|err| panic!("{target} should defer missing-parent check when set! writes incomplete/mismatched impls: {err}"));
    }

    // 2. `if` joins intersect differing impl tables: no false E_DUPLICATE_TRAIT_IMPL or E_IMPL_MISSING_REQUIRED_TRAIT,
    //    while matching branches continue enforcing both diagnostics.
    let if_join_ok_snippet = format!(
      "{base_defs}\n{}",
      r#"defn if-no-dup (flag)
  let ((decorated (impl-traits (Widget0 :n 1) ParentImpl)) (plain (Widget0 :n 2)))
    impl-traits (if flag decorated plain) ParentImpl
defn if-no-missing (flag)
  let ((decorated (impl-traits (Widget0 :n 1) ParentImpl)) (plain (Widget0 :n 2)))
    impl-traits (if flag plain decorated) ChildImpl
defn main! () &unit"#
    );
    let entries = prepare_with_schemas(
      &if_join_ok_snippet,
      &[
        ("if-no-dup", "Fn Bool (-> Widget0)"),
        ("if-no-missing", "Fn Bool (-> Widget0)"),
        ("main!", "Fn (-> Unit)"),
      ],
    );
    run_check_only(&entries).unwrap_or_else(|err| panic!("if-join with differing branch impls must defer/intersect safely: {err}"));

    for (name, body, schema, expected_code) in [
      (
        "if-both-dup",
        r#"defn if-both-dup (flag)
  let ((a (impl-traits (Widget0 :n 1) ParentImpl)) (b (impl-traits (Widget0 :n 2) ParentImpl)))
    impl-traits (if flag a b) ParentImpl
defn main! ()
  let ((_x (if-both-dup true))) &unit"#,
        "Fn Bool (-> Widget0)",
        "E_DUPLICATE_TRAIT_IMPL",
      ),
      (
        "if-both-missing",
        r#"defn if-both-missing (flag)
  let ((a (Widget0 :n 1)) (b (Widget0 :n 2)))
    impl-traits (if flag a b) ChildImpl
defn main! ()
  let ((_x (if-both-missing true))) &unit"#,
        "Fn Bool (-> Widget0)",
        "E_IMPL_MISSING_REQUIRED_TRAIT",
      ),
    ] {
      let snippet = format!("{base_defs}\n{body}");
      let entries = prepare_with_schemas(&snippet, &[(name, schema), ("main!", "Fn (-> Unit)")]);
      let err = run_check_only(&entries).expect_err(&format!("{name} must fail strict check-only with {expected_code}"));
      assert!(err.contains(expected_code), "{name} should report {expected_code}: {err}");
    }

    // 3. Pending nominal definition with an in-progress impl attachment defers missing-parent check
    //    while still reporting duplicate attachments.
    let pending_def_snippet = r#"deftrait Parent (.label AnyFn)
deftrait Child (requires Parent) (.greet AnyFn)
defstruct Widget (:n Number) (:impls Parent)
defimpl Widget Parent (.label (fn (x) (let ((child-impl (&impl::new Child $ :: .greet (fn (w) |child))) (_w (impl-traits Widget child-impl))) |parent)))
defn main! ()
  let ((_refs ([] Widget))) &unit"#;
    let entries = prepare_with_schemas(pending_def_snippet, &[("Widget", "StructDef"), ("main!", "Fn (-> Unit)")]);
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, "Widget", &warnings, &CallStackList::default())
      .unwrap_or_else(|err| panic!("pending nominal def impl compilation must defer missing-parent check: {err}"));

    let pending_dup_snippet = r#"deftrait Parent (.label AnyFn)
deftrait Child (requires Parent) (.greet AnyFn)
defstruct Widget (:n Number) (:impls Parent)
defimpl Widget Parent (.label (fn (x) (let ((child-impl (&impl::new Child $ :: .greet (fn (w) |child))) (_w (impl-traits (impl-traits Widget child-impl) child-impl))) |parent)))
defn main! ()
  let ((_refs ([] Widget))) &unit"#;
    let entries = prepare_with_schemas(pending_dup_snippet, &[("Widget", "StructDef"), ("main!", "Fn (-> Unit)")]);
    let err = run_check_only(&entries).expect_err("duplicate impl inside pending nominal def must still report E_DUPLICATE_TRAIT_IMPL");
    assert!(
      err.contains("E_DUPLICATE_TRAIT_IMPL"),
      "pending nominal def must preserve duplicate check: {err}"
    );

    // 4. Definition-valued (`StructDef` / `EnumDef`) joins routed through `let`, `do`, chained `impl-traits`,
    //    diverging-branch `if`, constructor calls, and `set!` preserve incompleteness when branches diverge,
    //    while still enforcing missing-parent / duplicate checks when branches agree or constructors are local.
    let def_valued_base = r#"defstruct Widget0 (:n Number)
defenum Choice0 (:n Number)
deftrait Parent (.label AnyFn)
deftrait Child (requires Parent) (.greet AnyFn)
deftrait Helper (.helper AnyFn)
def ParentImpl (defimpl Widget0 Parent (.label (fn (x) |parent)))
def ChildImpl (defimpl Widget0 Child (.greet (fn (x) |child)))
def HelperImpl (defimpl Widget0 Helper (.helper (fn (x) |helper)))
def EnumParentImpl (defimpl Choice0 Parent (.label (fn (x) |parent)))
def EnumChildImpl (defimpl Choice0 Child (.greet (fn (x) |child)))"#;

    let prepare_def_valued = |snippet: &str, extra_schemas: &[(&str, &str)]| {
      let mut all_schemas = vec![
        ("Choice0", "EnumDef"),
        ("Helper", "Trait"),
        ("HelperImpl", "Impl"),
        ("EnumParentImpl", "Impl"),
        ("EnumChildImpl", "Impl"),
        ("main!", "Fn (-> Unit)"),
      ];
      all_schemas.extend_from_slice(extra_schemas);
      prepare_with_schemas(snippet, &all_schemas)
    };

    let def_valued_ok_snippet = format!(
      "{def_valued_base}\n{}",
      r#"defn def-if-join-let-deferred (flag)
  let
      d $ if flag (impl-traits Widget0 ParentImpl) Widget0
      _a $ impl-traits d ChildImpl
      _b $ impl-traits d ParentImpl
    , &unit
defn def-if-join-forwarded (flag)
  let
      d $ if flag (impl-traits Widget0 ParentImpl) Widget0
      forwarded $ do &unit (impl-traits (if flag d (raise |unreachable)) HelperImpl)
      _attached $ impl-traits forwarded ChildImpl
    , &unit
defn def-if-join-construct (flag)
  let
      d $ if flag (impl-traits Widget0 ParentImpl) Widget0
      _inst $ impl-traits (d :n 1) ChildImpl
    , &unit
defn enum-def-if-join-let-deferred (flag)
  let
      e $ if flag (impl-traits Choice0 EnumParentImpl) Choice0
      _a $ impl-traits e EnumChildImpl
    , &unit
defn def-set-mixed-direct ()
  let ((local Widget0))
    set! local (impl-traits Widget0 ParentImpl)
    impl-traits local ChildImpl
    , &unit
defn def-set-mixed-if (flag)
  let ((local Widget0))
    set! local (if flag (impl-traits Widget0 ParentImpl) Widget0)
    impl-traits local ChildImpl
    , &unit
defn def-set-branch-if (flag)
  let ((local Widget0))
    if flag
      set! local (impl-traits Widget0 ParentImpl)
      , &unit
    impl-traits local ChildImpl
    , &unit
defn def-set-branch-match ()
  let ((local Widget0))
    match (Option :some 1)
      (:some _) (set! local (impl-traits Widget0 ParentImpl))
      (:none) &unit
    impl-traits local ChildImpl
    , &unit
defn def-set-branch-let ()
  let ((local Widget0))
    let ((_tmp 1))
      set! local (impl-traits Widget0 ParentImpl)
      , &unit
    impl-traits local ChildImpl
    , &unit
defn main! ()
  def-if-join-let-deferred true
  def-if-join-forwarded true
  def-if-join-construct true
  enum-def-if-join-let-deferred true
  def-set-mixed-direct
  def-set-mixed-if true
  def-set-branch-if true
  def-set-branch-match
  def-set-branch-let
  , &unit"#
    );
    let entries = prepare_def_valued(
      &def_valued_ok_snippet,
      &[
        ("def-if-join-let-deferred", "Fn Bool (-> Unit)"),
        ("def-if-join-forwarded", "Fn Bool (-> Unit)"),
        ("def-if-join-construct", "Fn Bool (-> Unit)"),
        ("enum-def-if-join-let-deferred", "Fn Bool (-> Unit)"),
        ("def-set-mixed-direct", "Fn (-> Unit)"),
        ("def-set-mixed-if", "Fn Bool (-> Unit)"),
        ("def-set-branch-if", "Fn Bool (-> Unit)"),
        ("def-set-branch-match", "Fn (-> Unit)"),
        ("def-set-branch-let", "Fn (-> Unit)"),
      ],
    );
    run_check_only(&entries)
      .unwrap_or_else(|err| panic!("definition-valued incomplete joins through let/do/set! must defer safely: {err}"));

    for (name, body, schema, expected_code) in [
      (
        "def-if-join-both-missing",
        r#"defn def-if-join-both-missing (flag)
  let
      d $ if flag Widget0 Widget0
      _a $ impl-traits d ChildImpl
    , &unit
defn main! () (def-if-join-both-missing true)"#,
        "Fn Bool (-> Unit)",
        "E_IMPL_MISSING_REQUIRED_TRAIT",
      ),
      (
        "def-if-join-both-dup",
        r#"defn def-if-join-both-dup (flag)
  let
      d $ if flag (impl-traits Widget0 ParentImpl) (impl-traits Widget0 ParentImpl)
      _a $ impl-traits d ParentImpl
    , &unit
defn main! () (def-if-join-both-dup true)"#,
        "Fn Bool (-> Unit)",
        "E_DUPLICATE_TRAIT_IMPL",
      ),
      (
        "local-defenum-let-missing",
        r#"defn local-defenum-let-missing ()
  let
      LocalChoice $ defenum LocalChoice (:n Number)
      c-impl $ defimpl LocalChoice Child (.greet (fn (x) |hi))
      _attached $ impl-traits LocalChoice c-impl
    , &unit
defn main! () (local-defenum-let-missing)"#,
        "Fn (-> Unit)",
        "E_IMPL_MISSING_REQUIRED_TRAIT",
      ),
    ] {
      let snippet = format!("{def_valued_base}\n{body}");
      let entries = prepare_def_valued(&snippet, &[(name, schema)]);
      let err = run_check_only(&entries).expect_err(&format!("{name} must fail strict check-only with {expected_code}"));
      assert!(err.contains(expected_code), "{name} should report {expected_code}: {err}");
    }
  });
}

#[test]
fn strict_type_fail_retired_methods_report_migration_guidance() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    for (entry, method, replacement) in [
      ("join-list", ".join", ".intersperse"),
      ("values-map", ".values", ".distinct-values"),
      ("contains-list", ".contains?", ".contains-index?"),
      ("contains-string", ".contains?", ".contains-index?"),
    ] {
      let entries = load_fixture_entries_with_entry("calcit/type-fail/retired-method-strict.cirru", Some(entry));
      let err = run_check_only(&entries).expect_err(&format!("retired {method} ({entry}) must fail strict check-only"));
      assert!(err.contains("E_RETIRED_METHOD"), "{entry} should report E_RETIRED_METHOD: {err}");
      assert!(err.contains(&format!("method `{method}`")), "{entry} should name {method}: {err}");
      assert!(err.contains(replacement), "{entry} should suggest {replacement}: {err}");
    }
  });
}

#[test]
fn strict_type_fail_retired_alias_methods_report_preferred_spelling() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    for (snippet, method, replacement) in [
      ("let\n    x 1.5\n  .round? x", ".round?", ".integer?"),
      (
        ".write-text (fs:path |/calcit-retired-method-does-not-exist/file) |content",
        ".write-text",
        ".write-text!",
      ),
      ("let\n    r $ FfiResponse :raw nil\n  .resolve r 1", ".resolve", ".resolve!"),
      ("let\n    r $ FfiResponse :raw nil\n  .reject r |failed", ".reject", ".reject!"),
      (".reduce ([] 1 2) 0 &+", ".reduce", ".fold"),
      (".bind ([] 1 2) $ fn (x) ([] x)", ".bind", ".flat-map"),
      (".join-str ([] |a |b) |-", ".join-str", ".join-string"),
      (".nth ([] 1 2) 0", ".nth", ".get"),
      (".mappend (&{} :a 1) (&{} :b 2)", ".mappend", ".merge"),
      (".add (&{} :a 1) ([] :b 2)", ".add", ".assoc"),
      (".add (#{} 1) 2", ".add", ".include"),
      (".mappend (#{} 1) (#{} 2)", ".mappend", ".union"),
      (".contains? (&{} :a 1) :a", ".contains?", ".contains-key?"),
      (".includes? (&{} :a 1) 1", ".includes?", ".contains-value?"),
      (".contains? (#{} 1) 1", ".contains?", ".includes?"),
      (".count |abc", ".count", ".len"),
    ] {
      let entries = load_snippet_entries(snippet);
      let err = run_check_only(&entries).expect_err(&format!("retired {method} must fail strict check-only"));
      assert!(err.contains("E_RETIRED_METHOD"), "{method} should report E_RETIRED_METHOD: {err}");
      assert!(err.contains(&format!("method `{method}`")), "{method} should be named: {err}");
      assert!(
        err.contains(&format!("`{replacement}`")),
        "{method} should suggest {replacement}: {err}"
      );
    }
  });
}

#[test]
fn retired_string_count_rejects_prefix_and_postfix_in_both_modes() {
  run_with_large_stack(|| {
    let _restore = StrictTypesReset {
      previous: runner::preprocess::is_strict_types_enabled(),
    };
    for strict in [false, true] {
      runner::preprocess::set_strict_types(strict);
      for snippet in [
        ".count |abc",
        "|abc .count",
        "let\n    text |abc\n  text .count",
        "(&str:concat |a |bc) .count",
      ] {
        let entries = load_snippet_entries(snippet);
        let err = run_check_only(&entries).expect_err(&format!("String .count must remain retired (strict={strict}): {snippet}"));
        assert!(err.contains("E_RETIRED_METHOD"), "missing retirement diagnostic: {err}");
        assert!(err.contains("`.len`"), "missing replacement spelling: {err}");
      }
    }
  });
}

#[test]
fn retired_core_functions_report_preferred_spelling() {
  run_with_large_stack(|| {
    for (snippet, name, replacement, version) in [
      ("some? 1", "some?", "non-nil?", "0.29.0"),
      ("join-str ([] 1 2) |-", "join-str", "join-string", "0.29.0"),
      (
        "add-watch (ref 0) :k $ fn (current previous) &unit",
        "add-watch",
        "add-watch!",
        "0.29.0",
      ),
      ("cpu-time", "cpu-time", "monotonic-time-ms", "0.29.0"),
      ("join ([] 1 2) 0", "join", "intersperse", "0.31.0"),
      ("section-by ([] 1 2 3) 2", "section-by", "chunks", "0.31.0"),
      ("turn-str 1", "turn-str", "to-string", "0.31.0"),
      ("some-in? ({} (:a 1)) ([] :a)", "some-in?", ".ok? $ get-in x path", "0.31.0"),
      ("round? 1", "round?", "integer?", "0.31.0"),
      ("map-kv ({} (:a 1)) $ fn (k v) ([] k v)", "map-kv", "filter-map-kv", "0.41.0"),
      (
        "foldl-shortcut ([] 1) 0 0 $ fn (acc x) (:: false acc)",
        "foldl-shortcut",
        "fold-while",
        "0.31.0",
      ),
      ("case-default 1 0 (1 :a)", "case-default", "match", "0.31.0"),
      ("%some 1", "%some", "Option :some value", "0.31.0"),
      ("%none", "%none", "Option :none", "0.31.0"),
      ("atom 0", "atom", "ref", "0.31.0"),
      ("vals ({} (:a 1))", "vals", "distinct-values", "0.31.0"),
      ("w-log 1", "w-log", "dbg", "0.31.0"),
      ("wo-log 1", "wo-log", "x", "0.31.0"),
      ("flipped &- 1 2", "flipped", "(f b a)", "0.31.0"),
      (
        "list-match ([] 1) (() 0) ((h t) h)",
        "list-match",
        "match (destruct-list xs) ((:none) ...) ((:some head tail) ...)",
        "0.31.0",
      ),
      ("let-sugar ((a 1)) a", "let-sugar", "let[]", "0.31.0"),
      ("apply-args (1) (fn (x) x)", "apply-args", "loop ((param value) ...) body", "0.31.0"),
      ("&format-ternary-tree $ [] 1", "&format-ternary-tree", "to-lispy-string", "0.33.0"),
    ] {
      for source in [
        snippet.to_owned(),
        format!("ns app.main\n  :require\n    calcit.core :refer $ {name}\n\n{snippet}"),
        format!(
          "ns app.main\n  :require\n    calcit.core :as core\n\n{}",
          snippet.replacen(name, &format!("core/{name}"), 1)
        ),
        snippet.replacen(name, &format!("calcit.core/{name}"), 1),
      ] {
        let entries = load_snippet_entries(&source);
        let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
        let _ = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
        let warnings = warnings.borrow();
        assert!(
          warnings.iter().any(|warning| {
            warning.message().contains(&format!("unknown `{name}`"))
              && warning.message().contains(&format!("removed from calcit.core in {version}"))
              && warning.message().contains(&format!("`{replacement}`"))
          }),
          "retired `{name}` should point to `{replacement}` in {source}: {warnings:?}"
        );
      }
    }

    let mut snapshot = snapshot::Snapshot::default();
    snapshot.files.insert(
      "app.main".to_owned(),
      snapshot::create_file_from_snippet("ns app.main\n  :require\n    app.helper :refer $ join\n\njoin")
        .expect("custom import fixture should parse"),
    );
    snapshot.files.insert(
      "app.helper".to_owned(),
      snapshot::create_file_from_snippet("defn join () 1").expect("custom definition should parse"),
    );
    let entries = prepare_snapshot_entries(snapshot);
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("a same-spelling function from another namespace remains available");
    assert!(
      warnings.borrow().is_empty(),
      "custom imports are not retired core names: {:?}",
      warnings.borrow()
    );
  });
}

#[test]
fn assert_type_expands_qualified_function_alias_at_runtime() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let mut snapshot = snapshot::Snapshot::default();
    let mut main =
      snapshot::create_file_from_snippet("ns app.main\n  :require\n    app.lib :as lib\n\ndo (assert-type handler lib/Identity) &unit")
        .expect("main fixture should parse");
    let handler = snapshot::create_file_from_snippet("defn handler (x)\n  hint-fn $ Fn String (-> String)\n  , x")
      .expect("handler fixture should parse");
    main.defs.extend(handler.defs.into_iter().filter(|(name, _)| name == "handler"));
    snapshot.files.insert("app.main".to_owned(), main);
    snapshot.files.insert(
      "app.lib".to_owned(),
      snapshot::create_file_from_snippet("deftype Identity $ Fn String (-> String)").expect("alias fixture should parse"),
    );
    let entries = prepare_snapshot_entries(snapshot);
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("qualified function alias must pass static checking");
    let alias = CalcitTypeAnnotation::TypeRef(Arc::from("app.lib/Identity"), Arc::new(vec![]));
    assert!(
      !calcit::calcit::value_matches_type_annotation(&Calcit::Number(1.0), &alias),
      "a non-function must not pass the qualified function alias"
    );
    let mut other = calcit::calcit::CalcitStructDef::from_fields(cirru_edn::EdnTag::new("Identity"), vec![]);
    other.definition_ref = Some(Arc::from("app.other/Identity"));
    let same_name = Calcit::Struct(calcit::calcit::CalcitStructValue {
      struct_ref: Arc::new(other),
      values: Arc::new(vec![]),
    });
    assert!(
      !calcit::calcit::value_matches_type_annotation(&same_name, &alias),
      "an unrelated same-named struct must not pass the function alias"
    );
    calcit::run_program(entries.init_ns, entries.init_def, &[]).expect("the same assertion must pass at runtime (#2913)");
  });
}

#[test]
fn assert_type_resolves_struct_through_as_alias() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let build = |target: &str| {
      let mut snapshot = snapshot::Snapshot::default();
      snapshot.files.insert(
        "app.main".to_owned(),
        snapshot::create_file_from_snippet(&format!(
          "ns app.main\n  :require\n    app.lib :as lib\n\nlet ((t (lib/Task :n 1))) (assert-type t {target}) t"
        ))
        .expect("alias fixture should parse"),
      );
      snapshot.files.insert(
        "app.lib".to_owned(),
        snapshot::create_file_from_snippet("defstruct Task (:n Number)").expect("struct fixture should parse"),
      );
      prepare_snapshot_entries(snapshot)
    };

    let entries = build("lib/Task");
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("`assert-type t lib/Task` should resolve the `:as` alias (#2682)");
    let app_warnings: Vec<_> = warnings
      .borrow()
      .iter()
      .filter(|warning| !warning.message().contains(" in calcit.core/"))
      .map(|warning| warning.message().to_owned())
      .collect();
    assert!(app_warnings.is_empty(), "unexpected warnings: {app_warnings:?}");

    let entries = build("lib/Nope");
    let warnings = RefCell::new(vec![]);
    let result = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
    assert!(
      result.is_err()
        || warnings
          .borrow()
          .iter()
          .any(|warning| !warning.message().contains(" in calcit.core/")),
      "an unknown alias target must still be reported"
    );
  });
}

#[test]
fn assert_type_qualifies_same_namespace_union_in_type_forms() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    for (value, target, passes) in [
      ("([] |a)", "$ List app.main/V", true),
      ("([] |a)", "$ List V", true),
      ("|a", "V", true),
      ("([] :k)", "$ List V", false),
      (":k", "V", false),
    ] {
      let mut snapshot = snapshot::Snapshot::default();
      let mut file = snapshot::create_file_from_snippet(&format!("let ((t {value})) (assert-type t {target}) t")).expect("parse");
      let extra = snapshot::create_file_from_snippet("def V $ defunion V String Number").expect("parse");
      file.defs.extend(extra.defs.into_iter().filter(|(k, _)| k == "V"));
      snapshot.files.insert("app.main".to_owned(), file);
      let entries = prepare_snapshot_entries(snapshot);
      let warnings = RefCell::new(vec![]);
      let r = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      if passes {
        assert!(
          r.is_ok(),
          "`assert-type t {target}` on {value} should pass (#2682): {:?}",
          r.err().map(|e| e.msg)
        );
        let ran = calcit::run_program(entries.init_ns.clone(), entries.init_def, &[]);
        assert!(
          ran.is_ok(),
          "`assert-type t {target}` on {value} should hold at runtime: {:?}",
          ran.err()
        );
      } else {
        assert!(r.is_err(), "`assert-type t {target}` on {value} must be rejected");
      }
    }
  });
}

#[test]
fn assert_type_resolves_short_names_in_type_forms() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    for (value, target) in [
      ("(lib/Task :n 1)", "'app.lib/Task"),
      ("|a", "lib/U"),
      ("|a", "'app.lib/U"),
      ("([] (lib/Task :n 1))", "$ List lib/Task"),
      ("([] (lib/Task :n 1))", "$ List app.lib/Task"),
      ("|a", "U"),
      ("([] (lib/Task :n 1))", "$ List Task"),
      ("([] |a)", "$ List U"),
      ("([] |a)", "$ List lib/U"),
      ("(lib/Task :n 1)", "Task"),
    ] {
      let mut snapshot = snapshot::Snapshot::default();
      snapshot.files.insert(
        "app.main".to_owned(),
        snapshot::create_file_from_snippet(&format!(
          "ns app.main\n  :require\n    app.lib :as lib\n    app.lib :refer $ Task U\n\nlet ((t {value})) (assert-type t {target}) t"
        ))
        .expect("parse"),
      );
      snapshot.files.insert(
        "app.lib".to_owned(),
        snapshot::create_file_from_snippet("defstruct Task (:n Number)\n\ndef U $ defunion U String Number").expect("parse"),
      );
      let entries = prepare_snapshot_entries(snapshot);
      let warnings = RefCell::new(vec![]);
      let r = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      let w: Vec<_> = warnings
        .borrow()
        .iter()
        .filter(|w| !w.message().contains(" in calcit.core/"))
        .map(|w| w.message().to_owned())
        .collect();
      assert!(r.is_ok(), "`assert-type t {target}` on {value} should pass (#2682): {:?}", r.err());
      assert!(w.is_empty(), "unexpected warnings for `{target}`: {w:?}");
      let ran = calcit::run_program(entries.init_ns.clone(), entries.init_def, &[]);
      assert!(
        ran.is_ok(),
        "`assert-type t {target}` on {value} should also hold at runtime: {:?}",
        ran.err()
      );
    }
  });
}

#[test]
fn strict_type_fail_unsafe_coerce_requires_lexical_ffi_scope() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let unscoped = load_fixture_entries("calcit/type-fail/unsafe-coerce-unscoped-strict.cirru");
    let err = run_check_only(&unscoped).expect_err("unscoped unsafe-coerce must fail strict check-only");
    assert!(err.contains("E_UNSCOPED_UNSAFE_COERCE"), "unexpected strict FFI error: {err}");
    assert!(
      err.contains("`:js-ffi` or `:erased-cache` boundary"),
      "missing lexical-boundary guidance: {err}"
    );

    let scoped = load_fixture_entries("calcit/type-fail/unsafe-coerce-scoped-strict.cirru");
    // A non-host coercion target is not proof evidence, so the default
    // preflight (#2121) rejects the adapter's return; the ordinary pass
    // still accepts the scoped coercion without leaking capability.
    let err = run_check_only(&scoped).expect_err("a non-host coercion target does not prove the adapter return");
    assert!(err.contains("E_FN_RETURN_UNPROVEN"), "unexpected scoped proof error: {err}");
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&scoped.init_ns, &scoped.init_def, &warnings, &CallStackList::default())
      .expect("a marked adapter may contain unsafe-coerce without leaking capability to its caller");

    let global_leak = load_fixture_entries("calcit/type-fail/unsafe-coerce-global-leak-strict.cirru");
    let err = run_check_only(&global_leak).expect_err("a marked caller must not authorize a referenced top-level value");
    assert!(
      err.contains("E_UNSCOPED_UNSAFE_COERCE"),
      "unexpected global capability error: {err}"
    );
  });
}

#[test]
fn strict_type_fail_erased_cache_feature_scopes_typed_cache_restore() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let path = "calcit/type-fail/unsafe-coerce-erased-cache-strict.cirru";
    let ns = "type-fail-unsafe-coerce-erased-cache-strict.main";
    let content = fs::read_to_string(path).expect("read erased-cache fixture");
    let with_feature = |feature: &str| {
      let edited = content.replace(":features :erased-cache", feature);
      assert_ne!(edited, content, "fixture should declare the erased-cache feature");
      let data = cirru_edn::parse(&edited).expect("parse edited fixture");
      prepare_snapshot_entries(snapshot::load_snapshot_data(&data, path).expect("load edited fixture"))
    };
    // The strict workflow audit re-preprocesses each definition without
    // trusting coercion contracts; the adapter must still prove its return.
    let audit_restore = || {
      let warnings = RefCell::new(vec![]);
      runner::preprocess::with_assertion_proof(|| {
        runner::preprocess::trace_definition_source_expressions(ns, "restore-cached", &warnings, &CallStackList::default()).map(|_| ())
      })
    };

    run_check_only(&load_fixture_entries(path)).expect("an :erased-cache adapter may contain unsafe-coerce");
    audit_restore().expect("an :erased-cache adapter proves its generic return through the restore");

    let err = run_check_only(&with_feature("")).expect_err("an unmarked cache restore must fail strict check-only");
    assert!(err.contains("E_UNSCOPED_UNSAFE_COERCE"), "unexpected unscoped error: {err}");
    assert!(err.contains("--add-feature erased-cache"), "missing erased-cache guidance: {err}");

    // `:js-ffi` scopes the coercion, but only host handle targets count as
    // evidence there, so a Calcit generic return keeps its proof obligation.
    // The default preflight carries the same proof (#2121), so check-only
    // rejects the js-ffi restore too.
    let err = run_check_only(&with_feature(":features :js-ffi")).expect_err("js-ffi scopes but does not prove the restore");
    assert!(err.contains("E_ERASED_GENERIC_RELATION"), "unexpected js-ffi check error: {err}");
    let err = audit_restore().expect_err("js-ffi must not prove a Calcit cache restore");
    assert_eq!(
      err.code.as_deref(),
      Some("E_ERASED_GENERIC_RELATION"),
      "unexpected js-ffi restore error: {err:?}"
    );
  });
}

#[test]
fn assertion_proof_keeps_assert_equal_option_erasure_quiet() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let path = "calcit/type-fail/assert-equal-option-proof.cirru";
    let ns = "type-fail-assert-equal-option-proof.main";
    injection::inject_platform_apis();
    run_check_only(&load_fixture_entries(path)).expect("ordinary check accepts assert= on Option values");
    // The audit ignores the `unsafe-coerce ... 'Dynamic` contract inside
    // assert=, but an explicit erasure is still not a nullable-API use.
    let warnings = RefCell::new(vec![]);
    runner::preprocess::with_assertion_proof(|| {
      runner::preprocess::trace_definition_source_expressions(ns, "main!", &warnings, &CallStackList::default()).map(|_| ())
    })
    .expect("assertion proof accepts assert= on Option values");
    let codes = warnings
      .borrow()
      .iter()
      .filter_map(|warning| warning.code().map(str::to_owned))
      .collect::<Vec<_>>();
    assert!(
      !codes.iter().any(|code| code == "E_NOMINAL_ENUM_LEGACY_USE"),
      "explicit Dynamic erasure must not warn as a nullable use: {codes:?}"
    );
  });
}

/// #2121: the default preflight proves directional Dynamic flows, while
/// the internal non-strict mode keeps accepting them.
#[test]
fn default_proof_rejects_directional_dynamic_flows() {
  let cases = [
    ("p03", "E_FN_RETURN_UNPROVEN"),
    ("p12", "E_FN_RETURN_UNPROVEN"),
    ("p21", "E_CALL_ARGUMENT_UNPROVEN"),
    // `and a b` expands to branches that return `false`; with every branch
    // typed, the report names them instead of an unknown join (#2651).
    ("p30", "E_BRANCH_TYPE_MISMATCH"),
    ("p31", "E_CALL_ARGUMENT_UNPROVEN"),
  ];
  for (case, code) in cases {
    run_with_large_stack(move || {
      let path = format!("calcit/type-fail/directional-default-{case}.cirru");
      injection::inject_platform_apis();
      {
        let _strict = StrictTypesReset::enabled();
        let err = run_check_only(&load_fixture_entries(&path)).expect_err("default proof must reject the directional flow");
        assert!(err.contains(code), "{case}: expected {code}, got: {err}");
        if case == "p30" {
          assert!(
            err.contains(": `:number`, branch 2: `:bool` (does not satisfy the return type)"),
            "p30 must name each branch type: {err}"
          );
        }
      }
      let _compat = StrictTypesReset::disabled();
      run_check_only(&load_fixture_entries(&path)).expect("non-strict mode keeps the old behavior");
    });
  }
}

#[test]
fn default_proof_accepts_explicit_dynamic_schemas() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    injection::inject_platform_apis();
    run_check_only(&load_fixture_entries("calcit/type-fail/directional-default-explicit-dynamic.cirru"))
      .expect("values declared Dynamic stay accepted under the default proof");
  });
}

/// #2499: callback parameters are contravariant at the strict Dynamic/nominal
/// boundary, including rest parameters. A callback accepting Dynamic (fixed or
/// rest) fits a slot that only passes FsPath; the reverse directions, a rest-only
/// FsPath callback in a fixed Dynamic slot, a Dynamic result and a wrong arity
/// stay rejected.
#[test]
fn strict_callback_parameters_are_contravariant_at_the_nominal_boundary() {
  run_with_large_stack(|| {
    let fixture = "calcit/type-fail/callback-contravariance-strict.cirru";
    let _strict = StrictTypesReset::enabled();
    injection::inject_platform_apis();
    run_check_only(&load_fixture_entries(fixture)).expect("open fixed and rest callbacks must satisfy FsPath callback slots");
    for (entry, detail) in [
      ("reverse", "`fn(dynamic) -> unit`"),
      ("result", "-> dynamic` data"),
      ("reverse-rest", "`fn(& dynamic) -> unit`"),
      ("rest-for-fixed", "`fn(& type calcit.core/FsPath) -> unit` data"),
    ] {
      let err = run_check_only(&load_fixture_entries_with_entry(fixture, Some(entry)))
        .expect_err(&format!("{entry} must fail strict check-only"));
      assert!(err.contains("E_DYNAMIC_NOMINAL_ARGUMENT"), "{entry}: unexpected error: {err}");
      assert!(err.contains(detail), "{entry} should mention `{detail}`: {err}");
    }

    let entries = load_fixture_entries_with_entry(fixture, Some("arity"));
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("a wrong callback arity is reported as a diagnostic");
    assert!(
      warnings
        .borrow()
        .iter()
        .any(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH") && warning.message().contains("fn(dynamic, dynamic)")),
      "open parameters must not excuse a wrong arity: {:?}",
      warnings.borrow()
    );
  });
}

/// #2612: a fixed-arity callable fills a variadic callback slot when it takes every fixed
/// value and each extra parameter accepts the rest element. Element mismatches, too few
/// parameters and a nominal parameter behind a Dynamic rest stay rejected.
#[test]
fn strict_fixed_arity_callbacks_fill_variadic_slots() {
  run_with_large_stack(|| {
    let fixture = "calcit/type-fail/variadic-fixed-arity-strict.cirru";
    let _strict = StrictTypesReset::enabled();
    injection::inject_platform_apis();
    run_check_only(&load_fixture_entries(fixture)).expect("fixed-arity callbacks must satisfy variadic slots");

    let err = run_check_only(&load_fixture_entries_with_entry(fixture, Some("nominal-extra")))
      .expect_err("a FsPath parameter must not take Dynamic rest values");
    assert!(err.contains("E_DYNAMIC_NOMINAL_ARGUMENT"), "unexpected error: {err}");

    for (entry, detail) in [
      ("wrong-element", "fn(:string, :string) -> :string"),
      ("too-few", "fn(:number) -> :number"),
    ] {
      let entries = load_fixture_entries_with_entry(fixture, Some(entry));
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect("a mismatching callback is reported as a diagnostic");
      assert!(
        warnings
          .borrow()
          .iter()
          .any(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH") && warning.message().contains(detail)),
        "{entry} should be rejected: {:?}",
        warnings.borrow()
      );
    }
  });
}

#[test]
fn strict_family_bounds_admit_named_members_only() {
  run_with_large_stack(|| {
    let fixture = "calcit/type-fail/family-bound-generics.cirru";
    let _strict = StrictTypesReset::enabled();
    injection::inject_platform_apis();
    run_check_only(&load_fixture_entries(fixture)).expect("named enums and structs satisfy family bounds");
    for (entry, family) in [
      ("dynamic", "AnyEnum"),
      ("family", "AnyEnum"),
      ("number", "AnyEnum"),
      ("struct-for-enum", "AnyEnum"),
      ("enum-for-struct", "AnyStruct"),
    ] {
      let entries = load_fixture_entries_with_entry(fixture, Some(entry));
      let warnings = RefCell::new(vec![]);
      let _ = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      let expected = format!("family bound `{family}`");
      assert!(
        warnings
          .borrow()
          .iter()
          .any(|warning| warning.code() == Some("E_GENERIC_WHERE_BOUND_MISMATCH") && warning.message().contains(&expected)),
        "{entry} must report a family bound mismatch: {:?}",
        warnings.borrow()
      );
    }
  });
}

#[test]
fn strict_type_fail_erased_generic_relation_reports_stable_error_code() {
  run_with_large_stack(|| {
    let fixture = "calcit/type-fail/erased-generic-relation-strict.cirru";
    let _strict = StrictTypesReset::enabled();
    let strict_entries = load_fixture_entries(fixture);
    let err = run_check_only(&strict_entries).expect_err("an open generic binding must not be narrowed by a concrete callback");

    assert!(
      err.contains("E_ERASED_GENERIC_RELATION"),
      "unexpected strict generic-relation error: {err}"
    );
    assert!(err.contains("/compare-open`"), "callee should be explicit: {err}");
    assert!(err.contains("argument 2"), "narrowing argument should be identified: {err}");
    assert!(
      err.contains("passes `fn(:number) -> :number`"),
      "actual type should be rendered: {err}"
    );
    assert!(
      err.contains("required by `fn('T) -> 'T`"),
      "expected type should be rendered: {err}"
    );
    assert!(err.contains("generic relation `'T`"), "generic relation should be named: {err}");
    assert!(err.contains("decode or narrow the value"), "migration should be actionable: {err}");
  });
}

#[test]
fn strict_type_fail_generic_callback_output_keeps_map_value_type() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries("calcit/type-fail/generic-callback-map-output-strict.cirru");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("invalid map update should preprocess with a warning");
    let warnings = warnings.borrow();
    assert!(
      warnings
        .iter()
        .any(|warning| warning.code() == Some("E_PROC_ARG_TYPE_MISMATCH") && warning.message().contains("&map:assoc")),
      "generic callback output must retain its value type: {warnings:?}"
    );
  });
}

#[test]
fn strict_explicit_trait_call_rejects_unproven_dispatch_before_runtime() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    for (snippet, code) in [
      ("&trait-call calcit.core/Countable :count 1", "E_TRAIT_CALL_IMPL_MISSING"),
      ("&trait-call calcit.core/Countable :unknown $ [] 1", "E_TRAIT_CALL_METHOD_MISSING"),
      ("&trait-call calcit.core/Countable :count ([] 1) 2", "E_TRAIT_CALL_ARITY"),
      ("&trait-call 1 :count $ [] 1", "E_UNPROVEN_TRAIT_CALL"),
    ] {
      let entries = load_snippet_entries(snippet);
      let err = run_check_only(&entries).expect_err("invalid explicit trait call must fail during preprocessing");
      assert!(err.contains(code), "expected {code} for `{snippet}`, got: {err}");
    }
  });
}

#[test]
fn strict_specialized_map_contracts_reject_concrete_callbacks_for_open_payload() {
  run_with_large_stack(|| {
    let main_schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      open_reason: Default::default(),
      cases: Default::default(),
      guard: None,
      runtime_arity: None,
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: Arc::new(CalcitTypeAnnotation::Dynamic),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));
    let _strict = StrictTypesReset::enabled();
    let entries = load_snippet_entries_with_main_schema(
      "let\n    consume $ fn (xs)\n      hint-fn $ Fn (List Dynamic) (-> Dynamic)\n      map xs $ fn (value)\n        hint-fn $ Fn Number (-> Number)\n        + value 1\n  consume $ [] 1",
      Some(main_schema.clone()),
    );
    let error = run_check_only(&entries).expect_err("a Number-only callback must not narrow List<Dynamic> through core map");

    assert!(error.contains("E_ERASED_GENERIC_RELATION"), "unexpected strict map error: {error}");
    assert!(error.contains("calcit.core/map"), "callee should be explicit: {error}");
    assert!(error.contains("argument 2"), "callback position should be explicit: {error}");
    assert!(
      error.contains("fn(:number) -> :number"),
      "actual callback type should be explicit: {error}"
    );
    assert!(
      error.contains("fn(dynamic)"),
      "specialized callback contract should expose the open payload: {error}"
    );
    assert!(
      error.contains("decode or narrow the payload"),
      "migration should be actionable: {error}"
    );

    let option_entries = load_snippet_entries_with_main_schema(
      "let\n    consume $ fn (value)\n      hint-fn $ Fn (Option Dynamic) (-> Dynamic)\n      value .map $ fn (item)\n        hint-fn $ Fn Number (-> Number)\n        + item 1\n  consume $ Option :some 1",
      Some(main_schema),
    );
    let option_error = run_check_only(&option_entries).expect_err("Option<Dynamic>.map must reject a Number-only callback");
    assert!(
      option_error.contains("E_ERASED_GENERIC_RELATION"),
      "unexpected strict Option.map error: {option_error}"
    );
    assert!(option_error.contains("`.map"), "method should be explicit: {option_error}");
    assert!(
      option_error.contains("argument 2"),
      "method callback position should include the receiver: {option_error}"
    );
  });
}

#[test]
fn type_fail_typed_rest_spread_checks_the_list_element_contract() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    for (parameters, fixed_types, call, expected, actual) in [
      ("label & values", "String", "accept |prefix & items", "list<string>", "list<number>"),
      ("label tail & values", "String String", "accept 1 & items", ":string", ":number"),
      ("label & values", "String", "accept 1 & items & items", ":string", ":number"),
    ] {
      for named in [false, true] {
        for explicit in [false, true] {
          let call = if explicit {
            call.replacen("accept", "&call-spread accept", 1)
          } else {
            call.to_owned()
          };
          let snippet = if named {
            format!(
              "defn accept ({parameters})\n  hint-fn $ Fn {fixed_types} & String (-> Unit)\n  , &unit\n\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let\n      items $ [] 1\n    {call}"
            )
          } else {
            format!(
              "let\n    accept $ fn ({parameters})\n      hint-fn $ Fn {fixed_types} & String (-> Unit)\n      , &unit\n    items $ [] 1\n  {call}"
            )
          };
          let entries = load_snippet_entries(&snippet);
          let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
          runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
            .expect("ordinary checking should record the argument mismatch");
          let warnings = warnings.borrow();
          let mismatches = warnings
            .iter()
            .filter(|warning| matches!(warning.code(), Some("E_FN_ARG_TYPE_MISMATCH" | "E_LOCAL_FN_ARG_TYPE_MISMATCH")))
            .collect::<Vec<_>>();
          assert_eq!(mismatches.len(), 1, "a Number spread cannot satisfy rest String: {warnings:?}");
          assert_eq!(mismatches[0].expected(), Some(expected));
          assert_eq!(mismatches[0].actual(), Some(actual));
          assert_eq!(mismatches[0].location().def, entries.init_def);
        }
      }
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
        .filter(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH"))
        .collect();
      assert_eq!(
        matched.len(),
        1,
        "expected exactly one arg-type warning for {path}, got: {warnings:?}"
      );
      let warning = matched[0];
      assert_eq!(warning.code(), Some("E_FN_ARG_TYPE_MISMATCH"));
      assert!(
        warning.message().contains(expected_msg),
        "warning message for {path} was: {}",
        warning.message()
      );
    }
  });
}

#[test]
fn named_callback_schema_rejects_wrong_arguments_and_non_callable_aliases() {
  run_with_large_stack(|| {
    let fn_schema = |args: Vec<Arc<CalcitTypeAnnotation>>, return_type: CalcitTypeAnnotation| {
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        open_reason: Default::default(),
        cases: Default::default(),
        guard: None,
        runtime_arity: None,
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: args,
        return_type: Arc::new(return_type),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      })))
    };

    let generic_schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      open_reason: Default::default(),
      cases: Default::default(),
      guard: None,
      runtime_arity: None,
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))],
      return_type: Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));

    for (alias_schema, applied_args, expected_message) in [
      (
        fn_schema(vec![Arc::new(CalcitTypeAnnotation::String)], CalcitTypeAnnotation::Number),
        vec![],
        "calling `callback` arg 1 expects type",
      ),
      (
        generic_schema,
        vec![Arc::new(CalcitTypeAnnotation::String)],
        "calling `callback` arg 1 expects type",
      ),
      (Arc::new(CalcitTypeAnnotation::Number), vec![], "non-function type"),
      (
        Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("app.main/NamedCallback"), Arc::new(vec![]))),
        vec![],
        "non-function type",
      ),
    ] {
      builtins::effects::init_effects_states();
      let mut snapshot = snapshot::Snapshot::default();
      let generic_case = !applied_args.is_empty();
      let snippet = if generic_case {
        "def NamedCallback &unit\n\ndefn invoke (callback) (callback 42)\n\ndefn main! () $ invoke $ fn (text) text\n\ndefn reload! () &unit"
      } else {
        "def NamedCallback &unit\n\ndefn invoke (callback) (callback 42)\n\ndefn main! () $ invoke $ fn (text) $ count text\n\ndefn reload! () &unit"
      };
      let mut file = snapshot::create_file_from_snippet(snippet).expect("named callback snippet should parse");
      file.defs.get_mut("NamedCallback").expect("alias definition").schema = alias_schema;
      file.defs.get_mut("invoke").expect("callback definition").schema = fn_schema(
        vec![Arc::new(CalcitTypeAnnotation::TypeRef(
          Arc::from("app.main/NamedCallback"),
          Arc::new(applied_args.clone()),
        ))],
        if generic_case {
          CalcitTypeAnnotation::String
        } else {
          CalcitTypeAnnotation::Number
        },
      );
      file.defs.get_mut("main!").expect("main definition").schema = fn_schema(
        vec![],
        if generic_case {
          CalcitTypeAnnotation::String
        } else {
          CalcitTypeAnnotation::Number
        },
      );
      file.defs.get_mut("reload!").expect("reload definition").schema = fn_schema(vec![], CalcitTypeAnnotation::Unit);
      snapshot.files.insert("app.main".to_owned(), file);
      let entries = prepare_snapshot_entries(snapshot);
      if !applied_args.is_empty() {
        let applied = CalcitTypeAnnotation::TypeRef(Arc::from("app.main/NamedCallback"), Arc::new(applied_args));
        let resolved = applied.resolve_to_fn().expect("applied generic callback should resolve");
        assert_eq!(resolved.arg_types, vec![Arc::new(CalcitTypeAnnotation::String)]);
        assert_eq!(resolved.return_type, Arc::new(CalcitTypeAnnotation::String));
        assert!(resolved.generics.is_empty());
        let missing = CalcitTypeAnnotation::TypeRef(Arc::from("app.main/NamedCallback"), Arc::new(vec![]));
        assert!(
          missing.resolve_to_fn().is_none(),
          "generic callback application requires its type argument"
        );
      }
      let _strict = StrictTypesReset::enabled();
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect("named callback fixture should preprocess with diagnostics");
      let warnings = warnings.borrow();
      assert!(
        warnings.iter().any(|warning| warning.message().contains(expected_message)),
        "expected {expected_message} among diagnostics: {warnings:?}"
      );
    }
  });
}

#[test]
fn schema_alias_definitions_must_inhabit_their_schema_for_all_defs_checks() {
  run_with_large_stack(|| {
    let fn_schema = |args: Vec<Arc<CalcitTypeAnnotation>>, return_type: CalcitTypeAnnotation| {
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        open_reason: Default::default(),
        cases: Default::default(),
        guard: None,
        runtime_arity: None,
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: args,
        return_type: Arc::new(return_type),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      })))
    };
    let alias_ref = |name: &str| {
      Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from(format!("app.main/{name}")),
        Arc::new(vec![]),
      ))
    };
    // Each alias is consumed only as a type, as in `calcit/test-types.cirru`.
    // Ordinary loading never reaches the alias value; `--all-defs` does, so
    // the alias body must be a real inhabitant rather than a `&unit` stub.
    for (callback_alias, items_alias, accepted) in [
      ("defn NamedCallback (text) (count text)", "def Items $ [] 1 2", true),
      ("def NamedCallback &unit", "def Items $ [] 1 2", false),
      ("defn NamedCallback (text) (count text)", "def Items &unit", false),
    ] {
      let snippet = format!(
        "{callback_alias}\n\n{items_alias}\n\ndefn invoke (callback text) (callback text)\n\ndefn total (items) (count items)\n\ndefn main! () $ &+ (invoke (fn (t) (count t)) |abc) (total $ [] 1 2)\n\ndefn reload! () &unit"
      );
      let mut snapshot = snapshot::Snapshot::default();
      builtins::effects::init_effects_states();
      let mut file = snapshot::create_file_from_snippet(&snippet).expect("alias snippet should parse");
      file.defs.get_mut("NamedCallback").expect("callback alias").schema =
        fn_schema(vec![Arc::new(CalcitTypeAnnotation::String)], CalcitTypeAnnotation::Number);
      file.defs.get_mut("Items").expect("list alias").schema =
        Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Number)));
      file.defs.get_mut("invoke").expect("invoke").schema = fn_schema(
        vec![alias_ref("NamedCallback"), Arc::new(CalcitTypeAnnotation::String)],
        CalcitTypeAnnotation::Number,
      );
      file.defs.get_mut("total").expect("total").schema = fn_schema(vec![alias_ref("Items")], CalcitTypeAnnotation::Number);
      file.defs.get_mut("main!").expect("main").schema = fn_schema(vec![], CalcitTypeAnnotation::Number);
      file.defs.get_mut("reload!").expect("reload").schema = fn_schema(vec![], CalcitTypeAnnotation::Unit);
      snapshot.files.insert("app.main".to_owned(), file);
      let entries = prepare_snapshot_entries(snapshot);
      let _strict = StrictTypesReset::enabled();
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect("entry-reachable code only uses the aliases as types");
      let failures: Vec<String> = ["NamedCallback", "Items", "invoke", "total"]
        .into_iter()
        .filter_map(|def| {
          runner::preprocess::ensure_ns_def_compiled("app.main", def, &warnings, &CallStackList::default())
            .err()
            .map(|err| format!("{def}: [{}] {}", err.code.as_deref().unwrap_or(""), err.msg))
        })
        .collect();
      if accepted {
        assert!(failures.is_empty(), "inhabited aliases should pass all-defs checks: {failures:?}");
      } else {
        assert_eq!(failures.len(), 1, "only the placeholder alias should fail: {failures:?}");
        assert!(
          failures[0].contains("E_SCHEMA_DEF_MISMATCH") && failures[0].contains("replace the `&unit` placeholder"),
          "placeholder alias should report a guided schema mismatch: {failures:?}"
        );
      }
    }
  });
}

#[test]
fn mixed_public_equality_reports_guided_type_mismatches() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("do\n  = 1 |one\n  = 1 1 |one\n  not= 1 |one\n  /= 1 |one");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("mixed public equality should preprocess with migration warnings, not hard errors");

    let warnings = warnings.borrow();
    let equality_warnings = warnings
      .iter()
      .filter(|warning| {
        warning.code() == Some("E_FN_ARG_TYPE_MISMATCH")
          && ["calcit.core/=`", "calcit.core/not=`", "calcit.core//=`"]
            .iter()
            .any(|operation| warning.message().contains(operation))
      })
      .collect::<Vec<_>>();
    assert_eq!(
      equality_warnings.len(),
      4,
      "expected fixed and variadic equality warnings: {warnings:?}"
    );
    assert!(
      equality_warnings.iter().all(|warning| {
        warning
          .message()
          .contains("public equality now requires operands of one static type")
          && warning.message().contains("narrow Dynamic/FFI values")
          && warning.message().contains("type predicate")
          && warning.message().contains("pattern-match nominal values")
      }),
      "every equality mismatch should carry migration guidance: {equality_warnings:?}"
    );
  });
}

#[test]
fn option_truthiness_is_an_error_except_as_the_last_and_operand() {
  run_with_large_stack(|| {
    // `get` lowers to a call without a coordinate; the error still points at
    // the written condition. The last `and` operand is a returned value.
    let entries = load_snippet_entries("let\n    m $ {} (:a 1)\n  [] (if (get m :a) 1 2) (and true (get m :a))");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::take_advisories();
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("Option conditions preprocess into blocking advisories");
    let nil_checks = runner::preprocess::take_advisories()
      .into_iter()
      .filter(|advisory| advisory.code() == Some(snapshot::OPTION_NIL_CHECK_ERROR))
      .collect::<Vec<_>>();
    assert_eq!(nil_checks.len(), 2, "both `if` and `and` conditions fail: {nil_checks:?}");
    assert_ne!(nil_checks[0].location().def.as_ref(), calcit::calcit::GENERATED_DEF);
  });
}

#[test]
fn nil_tests_on_option_literals_are_errors() {
  run_with_large_stack(|| {
    // `nil?` resolves to a builtin proc whose head carries no coordinate, and
    // inside `let` the call has none either; the error falls back to the
    // enclosing source form instead of degrading to a warning.
    let entries = load_snippet_entries(
      "let\n    a $ nil? None\n    b $ nil? (Some 1)\n    c $ non-nil? None\n    d $ non-nil? (Some 1)\n  [] a b c d",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::take_advisories();
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("nil tests on Option literals preprocess into blocking advisories");
    let nil_checks = runner::preprocess::take_advisories()
      .into_iter()
      .filter(|advisory| advisory.code() == Some(snapshot::OPTION_NIL_CHECK_ERROR))
      .collect::<Vec<_>>();
    assert_eq!(nil_checks.len(), 4, "every literal nil test fails: {nil_checks:?}");
    for check in &nil_checks {
      assert_ne!(check.location().def.as_ref(), calcit::calcit::GENERATED_DEF);
      assert!(!check.message().contains("/??"), "names the def: {}", check.message());
    }
    assert!(
      !warnings
        .borrow()
        .iter()
        .any(|warning| warning.code() == Some("E_NOMINAL_ENUM_LEGACY_USE")),
      "no leftover warning: {:?}",
      warnings.borrow()
    );
  });
}

#[test]
fn data_view_nil_arm_on_an_option_is_not_a_nil_check() {
  run_with_large_stack(|| {
    // The lowered `:nil` arm tests a generated local with `nil?`; only the
    // user-written `nil? x` is an error. `either` also tests a gensym local,
    // but its nil check is the user's, so it still fails.
    let entries = load_snippet_entries(
      "let\n    m $ {} (:a 1)\n    x $ get m :a\n  [] (nil? x) (either x 0)\n    match (data-view x)\n      (:nil) 1\n      _ 2",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::take_advisories();
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("data-view match preprocesses");
    let nil_checks = runner::preprocess::take_advisories()
      .into_iter()
      .filter(|advisory| advisory.code() == Some(snapshot::OPTION_NIL_CHECK_ERROR))
      .collect::<Vec<_>>();
    assert_eq!(
      nil_checks.len(),
      2,
      "`nil? x` and `either` fail, the data-view guard does not: {nil_checks:?}"
    );
    assert!(
      nil_checks.iter().all(|check| !check.message().contains("match_v")),
      "{nil_checks:?}"
    );
  });
}

#[test]
fn direct_option_helper_calls_warn_but_core_macro_expansions_do_not() {
  run_with_large_stack(|| {
    // `if-let` expands to `option:fold`, and `f` is a local alias of a helper;
    // only the helper calls written by name warn (#2653).
    let entries = load_snippet_entries(
      "let\n    m $ {} (:a 1)\n    x $ get m :a\n  [] (option:unwrap-or x 0) (option:map x inc) (if-let (v x) v 0) (.unwrap-or x 0)\n    let\n        f option:unwrap\n      f x",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("helper calls preprocess with warnings");
    let helper_calls = warnings
      .borrow()
      .iter()
      .filter(|warning| warning.code() == Some("E_OPTION_HELPER_CALL"))
      .map(|warning| warning.message().to_owned())
      .collect::<Vec<_>>();
    assert_eq!(helper_calls.len(), 2, "{helper_calls:?}");
    assert!(helper_calls[0].contains("`.unwrap-or`"), "{helper_calls:?}");
    assert!(helper_calls[1].contains("`.map`"), "{helper_calls:?}");
  });
}

#[test]
fn option_migration_source_calls_fail_during_preprocessing() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "do\n  = |dev $ get-env |mode\n  let\n      x $ get-env |mode\n    non-nil? x\n  update-in ({} (:a ({}))) ([] :a) $ fn (x) do (assoc x :b 1)\n  let\n      op $ :: :session/connect\n      tag-name $ nth op 0\n    starts-with? tag-name :session/",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("Option migration examples should preprocess with warnings, not reach runtime");

    let warnings = warnings.borrow();
    // `non-nil?` on an Option is blocking since 0.41.0 (#2653).
    let advisories = runner::preprocess::take_advisories();
    assert!(
      advisories.iter().any(|advisory| {
        advisory.code() == Some(snapshot::OPTION_NIL_CHECK_ERROR) && advisory.message().contains("`non-nil?` on Option value")
      }),
      "`non-nil?` on an Option should be an error, got: {advisories:?}"
    );
    for operation in ["=", "assoc"] {
      assert!(
        warnings.iter().any(|warning| {
          warning.code() == Some("E_NOMINAL_ENUM_LEGACY_USE")
            && warning.message().contains(&format!("`{operation}` consumes nominal enum `Option`"))
        }),
        "ordinary source call `{operation}` should report an Option migration warning, got: {warnings:?}"
      );
    }
    assert!(
      warnings.iter().any(|warning| {
        warning.code() == Some("E_PROC_ARG_TYPE_MISMATCH")
          && warning.message().contains("Proc `starts-with?` arg 1 expects type `:string`")
          && warning.message().contains("Option")
      }),
      "starts-with? should reject an Option argument during preprocessing, got: {warnings:?}"
    );
  });
}

#[test]
fn buffer_list_procs_reject_tag_receivers() {
  run_with_large_stack(|| {
    for (operation, source) in [
      ("&buf-list:push", "&buf-list:push :buf-list 1"),
      ("&buf-list:concat", "&buf-list:concat :buf-list ([] 1)"),
      ("&buf-list:count", "&buf-list:count :buf-list"),
      ("&buf-list:to-list", "&buf-list:to-list :buf-list"),
    ] {
      let entries = load_snippet_entries(source);
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect("invalid buffer-list input should produce a type diagnostic before runtime");

      let warnings = warnings.borrow();
      assert!(
        warnings.iter().any(|warning| {
          warning.code() == Some("E_PROC_ARG_TYPE_MISMATCH")
            && warning
              .message()
              .contains(&format!("Proc `{operation}` arg 1 expects type `:buf-list`"))
            && warning.message().contains("got `:tag`")
        }),
        "{operation} should reject a Tag receiver during preprocessing, got: {warnings:?}"
      );
    }
  });
}

#[test]
fn enum_definition_constructor_requires_an_entry() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("&enum-def:new :Empty");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("missing EnumDef entry should be diagnosed during preprocessing");

    let warnings = warnings.borrow();
    assert!(
      warnings.iter().any(|warning| {
        warning.code() == Some("E_PROC_ARITY") && warning.message().contains("Proc `&enum-def:new` expects at least 2 args, got 1")
      }),
      "expected an arity diagnostic for an EnumDef without entries, got: {warnings:?}"
    );
  });
}

#[test]
fn union_definition_requires_two_member_forms_with_a_local_name() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("let ((name :CssValue)) (&union-def:new name (quote Number))");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("the primitive arity check should diagnose the missing member without resolving the name");
    let warnings = warnings.borrow();
    assert!(
      warnings.iter().any(|warning| {
        warning.code() == Some("E_PROC_ARITY") && warning.message().contains("Proc `&union-def:new` expects at least 3 args, got 2")
      }),
      "expected a UnionDef arity diagnostic, got: {warnings:?}"
    );
  });
}

#[test]
fn parse_cirru_list_rejects_non_string_before_runtime() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("parse-cirru-list 1");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("invalid parser input should produce a type warning before runtime");
    assert!(
      warnings.borrow().iter().any(|warning| {
        warning.code() == Some("E_PROC_ARG_TYPE_MISMATCH")
          && warning.message().contains("Proc `parse-cirru-list` arg 1 expects type `:string`")
          && warning.message().contains("got `:number`")
      }),
      "parse-cirru-list should reject a Number argument during preprocessing: {:?}",
      warnings.borrow()
    );
  });
}

#[test]
fn format_cirru_rejects_non_list_before_runtime() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("format-cirru 1");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("invalid formatter input should produce a type warning before runtime");
    assert!(
      warnings.borrow().iter().any(|warning| {
        warning.code() == Some("E_PROC_ARG_TYPE_MISMATCH")
          && warning.message().contains("Proc `format-cirru` arg 1 expects type `list`")
          && warning.message().contains("got `:number`")
      }),
      "format-cirru should reject a Number argument during preprocessing: {:?}",
      warnings.borrow()
    );
  });
}

#[test]
fn format_cirru_one_liner_rejects_non_list_before_runtime() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("format-cirru-one-liner 1");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("invalid one-line formatter input should produce a type warning before runtime");
    assert!(
      warnings.borrow().iter().any(|warning| {
        warning.code() == Some("E_PROC_ARG_TYPE_MISMATCH")
          && warning
            .message()
            .contains("Proc `format-cirru-one-liner` arg 1 expects type `list`")
          && warning.message().contains("got `:number`")
      }),
      "format-cirru-one-liner should reject a Number argument during preprocessing: {:?}",
      warnings.borrow()
    );
  });
}

#[test]
fn strict_mode_rejects_implicit_option_and_result_stringification() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let main_schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      open_reason: Default::default(),
      cases: Default::default(),
      guard: None,
      runtime_arity: None,
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: Arc::new(CalcitTypeAnnotation::String),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));
    for (source, operation, nominal) in [
      ("defn main! ()\n  str $ Option :some :remove", "str", "Option"),
      ("defn main! ()\n  &turn-string $ Result :err |failed", "&turn-string", "Result"),
    ] {
      let entries = load_snippet_entries_with_main_schema(source, Some(main_schema.clone()));
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      let error = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect_err("strict stringification should fail during preprocessing");
      assert_eq!(error.code.as_deref(), Some("E_NOMINAL_ENUM_STRINGIFICATION"));
      assert!(error.msg.contains(&format!("`{operation}`")), "unexpected error: {error:?}");
      assert!(error.msg.contains(nominal), "unexpected error: {error:?}");
      assert!(error.msg.contains("to-lispy-string"), "unexpected error: {error:?}");
    }
  });
}

#[test]
fn strict_mode_keeps_explicit_enum_representation_and_plain_stringification() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let main_schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      open_reason: Default::default(),
      cases: Default::default(),
      guard: None,
      runtime_arity: None,
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: Arc::new(CalcitTypeAnnotation::String),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));
    let entries = load_snippet_entries_with_main_schema(
      "defn main! ()\n  str |prefix 1 $ to-lispy-string $ Option :some :remove",
      Some(main_schema),
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("strict code should accept ordinary values and an explicit Option representation");
  });
}

#[test]
fn dynamic_option_method_receiver_fails_during_preprocessing() {
  run_with_large_stack(|| {
    let entries =
      load_snippet_entries("defn consume-dynamic (value)\n  value .unwrap-or 0\n\ndefn main! ()\n  consume-dynamic (Option :some 1)");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("Dynamic Option method misuse should report before runtime");

    let warnings = warnings.borrow();
    let matched = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_DYNAMIC_NOMINAL_METHOD_RECEIVER"))
      .collect::<Vec<_>>();
    assert_eq!(matched.len(), 1, "expected one Dynamic Option-method warning, got: {warnings:?}");
    assert!(matched[0].message().contains("`.unwrap-or`"));
    assert!(matched[0].message().contains("`option:*` or `result:*`"));
  });
}

#[test]
fn option_fold_callbacks_share_return_type_during_preprocessing() {
  run_with_large_stack(|| {
    let snippets = [
      "option:fold (Option :some 1) (fn () |wrong) identity",
      "option:fold (Option :some 1) (fn () |wrong) (fn (x) 1)",
      "defmacro fold-or (base fallback)\n  quasiquote $ option:fold\n    , ~base\n    fn () ~fallback\n    , identity\n\ndefn main! () $ fold-or (Option :some 1) |wrong",
    ];

    for snippet in snippets {
      let entries = load_snippet_entries(snippet);
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect("option:fold mismatch should surface as a preprocessing warning");
      let warnings = warnings.borrow();
      assert!(
        warnings
          .iter()
          .any(|warning| { warning.code() == Some("E_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/option:fold") }),
        "option:fold callback mismatch should be reported for {snippet:?}, got: {warnings:?}"
      );
    }
  });
}

#[test]
fn option_returning_api_type_mismatches_include_unwrap_and_match_help() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("do\n  &+ (find-index ([] 1 2) $ fn (x) = x 2) 1\n  starts-with? (get-env |HOME) |/");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("Option-returning API examples should preprocess with migration warnings");

    let warnings = warnings.borrow();
    for inferred in ["Option<:number>", "Option<:string>"] {
      let warning = warnings
        .iter()
        .find(|warning| warning.code() == Some("E_PROC_ARG_TYPE_MISMATCH") && warning.message().contains(inferred))
        .unwrap_or_else(|| panic!("missing mismatch for {inferred}; warnings: {warnings:?}"));
      assert!(warning.message().contains("inferred type"), "warning: {warning:?}");
      assert!(
        warning.message().contains("typed `*-or` query helper when available"),
        "warning: {warning:?}"
      );
      assert!(warning.message().contains(".unwrap-or"), "warning: {warning:?}");
      assert!(warning.message().contains("native `match`"), "warning: {warning:?}");
    }
  });
}

#[test]
fn required_struct_field_access_inside_local_fn_keeps_its_diagnostic() {
  run_with_large_stack(|| {
    // Expanding the `{}` receiver drops symbol locations; inside the `fn` macro body the
    // access must still count as source code, with or without a return hint (#1748).
    for hint in ["", "\n        hint-fn $ Fn (-> String)"] {
      let entries = load_snippet_entries(&format!(
        "let\n    read $ fn (){hint}\n        decode-map-as (:file ({{}} (:file |a))) 'String\n  read"
      ));
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      let _ = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      let warnings = warnings.borrow();
      assert!(
        warnings
          .iter()
          .any(|warning| warning.code() == Some("E_REQUIRED_STRUCT_FIELD_TYPE")),
        "hint {hint:?} must keep the required field diagnostic, got: {warnings:?}"
      );
    }
  });
}

#[test]
fn required_struct_field_access_does_not_fall_back_to_option_lookup() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "do\n  let\n      record $ {} (:name |Ada)\n      name $ :name record\n    , name\n  let\n      Person $ defstruct Person (:name 'String)\n      person $ %{} Person (:name |Ada)\n      name $ :name person\n    assert-type name 'String\n  let\n      Person $ defstruct Person (:name 'String)\n      person $ %{} Person (:name |Ada)\n    :missing person\n  let\n      Person $ defstruct Person (:name 'String)\n      person $ %{} Person (:name |Ada)\n    get person :name",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    let error = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect_err("a statically known Struct must reject optional `get` lookup");
    assert_eq!(error.code.as_deref(), Some("E_STRUCT_FIELD_OPTIONAL_LOOKUP"));
    assert!(error.msg.contains("use `(:field value)`"));
    assert!(error.location.is_some(), "the current source location must remain available");
    assert!(
      !error.msg.contains("0.14.15"),
      "current Struct errors must not require a historical CLI"
    );
    assert!(!error.msg.contains("required-struct-field-v1"));

    let warnings = warnings.borrow();
    let required_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_REQUIRED_STRUCT_FIELD_TYPE"))
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

    let unknown_field_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_UNKNOWN_STRUCT_FIELD"))
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

/// `let{}` binds Struct fields by name; misspelt fields and non-Struct receivers
/// must be reported at check time whether the data is a symbol or an expression (#2697).
#[test]
fn let_struct_destructure_reports_field_and_receiver_errors() {
  run_with_large_stack(|| {
    let codes = |body: &str| -> Vec<(Option<String>, String)> {
      let entries = load_snippet_entries(&format!(
        "let\n    Task $ defstruct Task (:id Number) (:text String)\n    task $ Task :id 1 :text |a\n  {body}"
      ));
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      let _ = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      warnings
        .borrow()
        .iter()
        .map(|w| (w.code().map(str::to_owned), w.message().to_string()))
        .filter(|(_, m)| !m.contains("eprintln"))
        .collect()
    };
    let has = |diagnostics: &[(Option<String>, String)], code: &str, text: &str| {
      diagnostics.iter().any(|(c, m)| c.as_deref() == Some(code) && m.contains(text))
    };

    let ok = codes("let{} (id text) task $ + id $ .len text");
    assert!(ok.is_empty(), "declared fields should check cleanly: {ok:?}");

    for body in ["let{} (id txt) task id", "let{} (id txt) (do task) id"] {
      let diagnostics = codes(body);
      assert!(
        has(
          &diagnostics,
          "E_UNKNOWN_STRUCT_FIELD",
          "Field `:txt` does not exist in struct `Task`"
        ),
        "`{body}` must report the misspelt field: {diagnostics:?}"
      );
    }

    let mismatch = codes("let{} (text) task $ + text 1");
    assert!(
      mismatch.iter().any(|(c, _)| c.as_deref() == Some("E_FN_ARG_TYPE_MISMATCH")),
      "bindings must keep the declared field type: {mismatch:?}"
    );

    for body in ["let{} (id) ({} (:id 1)) id", "let{} (id) (identity $ {} (:id 1)) id"] {
      let diagnostics = codes(body);
      assert!(
        has(&diagnostics, "E_REQUIRED_STRUCT_FIELD_TYPE", "required field access `(:id value)`"),
        "`{body}` must reject a non-Struct receiver: {diagnostics:?}"
      );
    }
  });
}

#[test]
fn removed_tag_match_reports_the_published_migration_path() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("tag-match (Option :some 1)\n  (:some x) x\n  (:none) 0");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    let error = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect_err("removed tag-match must fail during preprocessing");
    assert_eq!(error.code.as_deref(), Some("E_REMOVED_TAG_MATCH"));
    assert!(error.msg.contains("use native `match`"));
    assert!(error.msg.contains("Calcit 0.14.15 `calcit fix --rule tag-match-to-match-v1`"));
    assert!(
      error.msg.contains("before upgrading to 0.14.16 or later"),
      "unexpected migration message: {}",
      error.msg
    );

    let shadow_entries = load_snippet_entries("let\n    tag-match $ fn (x) x\n  tag-match 1");
    let shadow_warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(
      &shadow_entries.init_ns,
      &shadow_entries.init_def,
      &shadow_warnings,
      &CallStackList::default(),
    )
    .expect("a lexical binding named tag-match must remain ordinary user code");
  });
}

#[test]
fn collection_path_operations_do_not_traverse_struct_fields() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "do\n  let\n      Person $ defstruct Person (:name 'String)\n      person $ %{} Person (:name |Ada)\n      people $ {} (:current person)\n    do\n      get-in person ([] :name)\n      get-in people ([] :current :name)\n      contains-in? person ([] :name)\n      assoc-in person ([] :name) |Grace\n      update-in person ([] :name) $ fn (current) (, current)\n      dissoc-in person ([] :name)",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("Struct path misuse should preprocess far enough to report every operation");

    let warnings = warnings.borrow();
    let path_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_STRUCT_PATH_OPERATION"))
      .collect::<Vec<_>>();
    assert_eq!(
      path_warnings.len(),
      6,
      "every collection path operation that reaches a Struct should be rejected, got: {warnings:?}"
    );
    for operation in ["get-in", "contains-in?", "assoc-in", "update-in", "dissoc-in"] {
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
      .filter(|warning| warning.code() == Some("E_METHOD_ARG_TYPE_MISMATCH"))
      .collect();
    assert_eq!(matched.len(), 1, "expected one method argument warning, got: {warnings:?}");
    assert!(
      matched[0].message().contains(".unwrap-or") && matched[0].message().contains("expects type `:string`, but got `:number`"),
      "warning message was: {}",
      matched[0].message()
    );

    let callback_warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(
      "type-fail-trait-method-generic-receiver.main",
      "invalid-result-callback",
      &callback_warnings,
      &CallStackList::default(),
    )
    .expect("method callback fixture should preprocess with warnings, not hard errors");
    let callback_warnings = callback_warnings.borrow();
    let return_warnings: Vec<&LocatedWarning> = callback_warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_FN_RETURN_TYPE_MISMATCH"))
      .collect();
    assert_eq!(
      return_warnings.len(),
      1,
      "expected one callback return warning, got: {callback_warnings:?}"
    );
    assert!(
      return_warnings[0].message().contains("Result") && return_warnings[0].message().contains("number"),
      "warning message was: {}",
      return_warnings[0].message()
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
      .filter(|warning| warning.code() == Some("E_GENERIC_WHERE_BOUND_MISMATCH"))
      .collect();
    assert_eq!(
      matched.len(),
      1,
      "expected exactly one generic where-bound warning, got: {warnings:?}"
    );
    let warning = matched[0];
    assert_eq!(warning.code(), Some("E_GENERIC_WHERE_BOUND_MISMATCH"));
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
      .filter(|warning| warning.code() == Some("E_GENERIC_WHERE_BOUND_MISMATCH"))
      .collect();
    assert_eq!(
      matched.len(),
      1,
      "expected exactly one generic where-bound warning, got: {warnings:?}"
    );
    let warning = matched[0];
    assert_eq!(warning.code(), Some("E_GENERIC_WHERE_BOUND_MISMATCH"));
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
fn type_fail_slice_receiver_trait_fixture_reports_warning_code() {
  run_with_large_stack(|| {
    let entries = load_fixture_entries("calcit/type-fail/slice-receiver-trait-mismatch.cirru");
    let slice_schema = program::lookup_def_schema("calcit.core", "slice");
    let CalcitTypeAnnotation::Fn(slice_fn_annot) = slice_schema.as_ref() else {
      panic!("core slice schema should load as fn, got {slice_schema:?}");
    };
    assert_eq!(slice_fn_annot.where_bounds.len(), 1, "core slice should carry one where-bound");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("slice receiver mismatch should preprocess with a warning");

    let warnings = warnings.borrow();
    let matched: Vec<&LocatedWarning> = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_GENERIC_WHERE_BOUND_MISMATCH"))
      .collect();
    assert_eq!(
      matched.len(),
      1,
      "expected exactly one slice where-bound warning, got: {warnings:?}"
    );
    let warning = matched[0];
    assert!(
      warning.message().contains("`:number`")
        && warning.message().contains("Sliceable")
        && warning.message().contains("calcit.core/slice"),
      "warning message was: {}",
      warning.message()
    );
  });
}

#[test]
fn type_fail_update_collection_contract_fixture_reports_warning_codes() {
  run_with_large_stack(|| {
    let entries = load_fixture_entries("calcit/type-fail/update-collection-contract-mismatch.cirru");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("collection update mismatches should preprocess with warnings");

    let warnings = warnings.borrow();
    let matched: Vec<&LocatedWarning> = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/update"))
      .collect();
    assert_eq!(matched.len(), 3, "expected update key and callback warnings, got: {warnings:?}");
    assert!(
      matched
        .iter()
        .any(|warning| { warning.message().contains("arg 2 expects type `:number`") && warning.message().contains("got `:tag`") }),
      "List update should require a Number index: {matched:?}"
    );
    assert!(
      matched.iter().any(|warning| {
        warning
          .message()
          .contains("arg 3 expects type `fn(:number) -> :number (:features :pass)`")
          && warning.message().contains("got `fn(:string) -> :string`")
      }),
      "Map update should bind the updater to its value type: {matched:?}"
    );
    assert!(
      matched.iter().any(|warning| {
        warning
          .message()
          .contains("arg 3 expects type `fn(:number) -> :number (:features :pass)`")
          && warning.message().contains("got `fn(:number) -> :string`")
      }),
      "inline update callbacks should receive the bound member type before validating their return type: {matched:?}"
    );
  });
}

#[test]
fn type_fail_collection_member_contract_fixture_reports_warning_codes() {
  run_with_large_stack(|| {
    let entries = load_fixture_entries("calcit/type-fail/collection-member-contract-mismatch.cirru");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("collection member mismatches should preprocess with warnings");

    let warnings = warnings.borrow();
    let matched: Vec<&LocatedWarning> = warnings
      .iter()
      .filter(|warning| {
        warning.code() == Some("E_FN_ARG_TYPE_MISMATCH")
          && (warning.message().contains("calcit.core/get")
            || warning.message().contains("calcit.core/includes?")
            || warning.message().contains("calcit.core/contains?")
            || warning.message().contains("calcit.core/assoc")
            || warning.message().contains("calcit.core/dissoc"))
      })
      .collect();
    assert_eq!(
      matched.len(),
      17,
      "expected lookup, membership, key, association, and dissociation warnings, got: {warnings:?}"
    );
    assert!(
      matched.iter().any(|warning| {
        warning.message().contains("calcit.core/get")
          && warning.message().contains("arg 2 expects type `:number`")
          && warning.message().contains("got `:tag`")
      }),
      "List lookup should require a Number index: {matched:?}"
    );
    assert!(
      matched.iter().any(|warning| {
        warning.message().contains("calcit.core/get")
          && warning.message().contains("arg 2 expects type `:tag`")
          && warning.message().contains("got `:number`")
      }),
      "Map lookup should require its key type: {matched:?}"
    );
    assert_eq!(
      matched
        .iter()
        .filter(|warning| {
          warning.message().contains("calcit.core/includes?")
            && warning.message().contains("arg 2 expects type `:number`")
            && warning.message().contains("got `:tag`")
        })
        .count(),
      2,
      "Set members and Map values should both preserve their Number type: {matched:?}"
    );
    assert!(
      matched.iter().any(|warning| {
        warning.message().contains("calcit.core/includes?")
          && warning.message().contains("arg 2 expects type `:string`")
          && warning.message().contains("got `:number`")
      }),
      "String membership should require a String substring: {matched:?}"
    );
    assert_eq!(
      matched
        .iter()
        .filter(|warning| {
          warning.message().contains("calcit.core/contains?")
            && warning.message().contains("arg 2 expects type `:number`")
            && warning.message().contains("got `:tag`")
        })
        .count(),
      3,
      "List and Enum indices plus Set members should preserve their Number contract: {matched:?}"
    );
    assert!(
      matched.iter().any(|warning| {
        warning.message().contains("calcit.core/contains?")
          && warning.message().contains("arg 2 expects type `:tag`")
          && warning.message().contains("got `:number`")
      }),
      "Map containment should require its key type: {matched:?}"
    );
    assert_eq!(
      matched
        .iter()
        .filter(|warning| {
          warning.message().contains("calcit.core/assoc")
            && warning.message().contains("arg 2 expects type `:number`")
            && warning.message().contains("got `:tag`")
        })
        .count(),
      2,
      "List and Enum association should require a Number index: {matched:?}"
    );
    assert_eq!(
      matched
        .iter()
        .filter(|warning| {
          warning.message().contains("calcit.core/assoc")
            && warning.message().contains("arg 3 expects type `:number`")
            && warning.message().contains("got `:tag`")
        })
        .count(),
      2,
      "List and Map association should preserve their Number member/value type: {matched:?}"
    );
    assert!(
      matched.iter().any(|warning| {
        warning.message().contains("calcit.core/assoc")
          && warning.message().contains("arg 2 expects type `:tag`")
          && warning.message().contains("got `:number`")
      }),
      "Map association should require its key type: {matched:?}"
    );
    assert!(
      matched.iter().any(|warning| {
        warning.message().contains("calcit.core/assoc")
          && warning.message().contains("arg 3 expects type `:number`")
          && warning.message().contains("got `:tag`")
      }),
      "Map association should require its value type: {matched:?}"
    );
    assert!(
      matched.iter().any(|warning| {
        warning.message().contains("calcit.core/dissoc")
          && warning.message().contains("arg 2 expects type `:number`")
          && warning.message().contains("got `:tag`")
      }),
      "List dissociation should require a Number index: {matched:?}"
    );
    assert_eq!(
      matched
        .iter()
        .filter(|warning| {
          warning.message().contains("calcit.core/dissoc")
            && warning.message().contains("expects type `:tag`")
            && warning.message().contains("got `:number`")
        })
        .count(),
      2,
      "Map dissociation should validate both the first and later rest keys: {matched:?}"
    );
    let proc_warnings = warnings
      .iter()
      .filter(|warning| {
        warning.code() == Some("E_PROC_ARG_TYPE_MISMATCH")
          && ["&map:dissoc", "&list:concat", "&merge"]
            .iter()
            .any(|proc| warning.message().contains(&format!("Proc `{proc}`")))
      })
      .collect::<Vec<_>>();
    assert_eq!(
      proc_warnings.len(),
      3,
      "typed native collection rests should be validated: {warnings:?}"
    );
    assert!(
      proc_warnings.iter().any(|warning| {
        warning.message().contains("Proc `&map:dissoc`")
          && warning.message().contains("arg 3 expects type `:tag`")
          && warning.message().contains("got `:number`")
      }),
      "native map dissoc should retain the map key binding for later keys: {proc_warnings:?}"
    );
    assert!(
      proc_warnings.iter().any(|warning| {
        warning.message().contains("Proc `&list:concat`")
          && warning.message().contains("arg 3 expects type `list<number>`")
          && warning.message().contains("got `list<tag>`")
      }),
      "native list concat should preserve its first list item type: {proc_warnings:?}"
    );
    assert!(
      proc_warnings.iter().any(|warning| {
        warning.message().contains("Proc `&merge`")
          && warning.message().contains("arg 3 expects type `map<tag, number>`")
          && warning.message().contains("got `map<tag, tag>`")
      }),
      "native map merge should preserve its first map key/value types: {proc_warnings:?}"
    );
    let filter_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/filter"))
      .collect::<Vec<_>>();
    assert_eq!(
      filter_warnings.len(),
      5,
      "collection filters should validate predicate contracts: {warnings:?}"
    );
    assert_eq!(
      filter_warnings
        .iter()
        .filter(|warning| {
          warning
            .message()
            .contains("arg 2 expects type `fn(:number) -> :bool (:features :pass)`")
            && warning.message().contains("got `fn(:number) -> :number`")
        })
        .count(),
      3,
      "named and inline List/Set filters should pass their Number member type to a Bool predicate: {filter_warnings:?}"
    );
    assert!(
      filter_warnings.iter().any(|warning| {
        warning
          .message()
          .contains("arg 2 expects type `fn(tuple<:tag, :number>) -> :bool (:features :pass)`")
          && warning.message().contains("got `fn(:number) -> :number`")
      }),
      "Map filters should pass each entry as a `Tuple K V` predicate argument (#2561): {filter_warnings:?}"
    );
    assert!(
      filter_warnings.iter().any(|warning| {
        warning.message().contains("got `fn(:number) -> :number`") && warning.message().contains("([] (&syntax defn)")
      }),
      "inline filter callbacks should receive the bound member type and retain their inferred return: {filter_warnings:?}"
    );
    assert!(
      filter_warnings
        .iter()
        .any(|warning| warning.message().contains("got `fn<'T>('T) -> 'T`")),
      "generic filter callbacks must not use the expected Bool output to prove an unknown payload: {filter_warnings:?}"
    );
    let iteration_warnings = warnings
      .iter()
      .filter(|warning| {
        warning.code() == Some("E_FN_ARG_TYPE_MISMATCH")
          && ["calcit.core/any?", "calcit.core/every?", "calcit.core/each"]
            .iter()
            .any(|name| warning.message().contains(name))
      })
      .collect::<Vec<_>>();
    assert_eq!(
      iteration_warnings.len(),
      3,
      "collection iteration callbacks should preserve member or pair contracts: {warnings:?}"
    );
    assert!(
      iteration_warnings.iter().any(|warning| {
        warning.message().contains("calcit.core/any?")
          && warning.message().contains("expects type `fn(:number) -> :bool (:features :pass)`")
          && warning.message().contains("got `fn(:number) -> :number`")
      }),
      "any? should require a Bool predicate over the List member type: {iteration_warnings:?}"
    );
    assert!(
      iteration_warnings.iter().any(|warning| {
        warning.message().contains("calcit.core/every?")
          && warning
            .message()
            .contains("expects type `fn(tuple<:tag, :number>) -> :bool (:features :pass)`")
          && warning.message().contains("got `fn(:number) -> :number`")
      }),
      "every? should expose Map entries as typed Tuple pairs: {iteration_warnings:?}"
    );
    assert!(
      iteration_warnings.iter().any(|warning| {
        warning.message().contains("calcit.core/each")
          && warning
            .message()
            .contains("expects type `fn(:string) -> dynamic (:features :pass)`")
          && warning.message().contains("got `fn(:number) -> :number`")
      }),
      "each should constrain callback input while allowing any return type: {iteration_warnings:?}"
    );
    let map_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/map"))
      .collect::<Vec<_>>();
    assert_eq!(
      map_warnings.len(),
      3,
      "collection maps should validate mapper contracts: {warnings:?}"
    );
    assert_eq!(
      map_warnings
        .iter()
        .filter(|warning| {
          warning
            .message()
            .contains("arg 2 expects type `fn(:string) -> :number (:features :pass)`")
            && warning.message().contains("got `fn(:number) -> :number`")
        })
        .count(),
      2,
      "List and Set maps should pass their String member type to the mapper: {map_warnings:?}"
    );
    assert!(
      map_warnings.iter().any(|warning| {
        warning
          .message()
          .contains("arg 2 expects type `fn(tuple<:tag, :number>) -> tuple<'K2, 'V2> (:features :pass)`")
          && warning.message().contains("got `fn(:number) -> :number`")
      }),
      "Map mappers should consume a `Tuple K V` entry and return a `Tuple K2 V2` pair (#2561): {map_warnings:?}"
    );
    let fold_warnings = warnings
      .iter()
      .filter(|warning| {
        matches!(warning.code(), Some("E_PROC_ARG_TYPE_MISMATCH") | Some("E_FN_ARG_TYPE_MISMATCH"))
          && (warning.message().contains("Proc `foldl`") || warning.message().contains("calcit.core/reduce"))
      })
      .collect::<Vec<_>>();
    assert_eq!(
      fold_warnings.len(),
      4,
      "collection folds should validate accumulator/member reducer contracts: {warnings:?}"
    );
    assert_eq!(
      fold_warnings
        .iter()
        .filter(|warning| {
          warning
            .message()
            .contains("expects type `fn(:number, :string) -> :number (:features :pass)`")
            && warning.message().contains("got `fn(:number, & :number) -> :number`")
        })
        .count(),
      3,
      "List/Set foldl and Set reduce should preserve String member types: {fold_warnings:?}"
    );
    assert!(
      fold_warnings.iter().any(|warning| {
        warning.message().contains("Proc `foldl`")
          && warning
            .message()
            .contains("expects type `fn(:number, tuple<:tag, :number>) -> :number (:features :pass)`")
          && warning.message().contains("got `fn(:number, & :number) -> :number`")
      }),
      "Map foldl should expose the key and value types of each entry: {fold_warnings:?}"
    );
    let sort_warnings = warnings
      .iter()
      .filter(|warning| {
        warning.code() == Some("E_PROC_ARG_TYPE_MISMATCH")
          && (warning.message().contains("Proc `sort`") || warning.message().contains("Proc `&list:sort`"))
      })
      .collect::<Vec<_>>();
    assert_eq!(
      sort_warnings.len(),
      3,
      "collection sorts should validate comparator contracts: {warnings:?}"
    );
    assert!(
      sort_warnings.iter().all(|warning| warning
        .message()
        .contains("expects type `fn(:string, :string) -> :number (:features :pass)`")),
      "sort comparators should receive the concrete List member type and return Number: {sort_warnings:?}"
    );
    assert_eq!(
      sort_warnings
        .iter()
        .filter(|warning| warning.message().contains("got `fn(:number, & :number) -> :number`"))
        .count(),
      2,
      "sort and native list sort should both reject a comparator over the wrong member type: {sort_warnings:?}"
    );
    assert!(
      sort_warnings
        .iter()
        .any(|warning| warning.message().contains("got `fn(:number) -> :number`")),
      "sort should reject a comparator with the wrong arity: {sort_warnings:?}"
    );
    let sort_by_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/&list:sort-by"))
      .collect::<Vec<_>>();
    assert_eq!(
      sort_by_warnings.len(),
      1,
      "list sort-by should validate function selectors against the member type: {warnings:?}"
    );
    assert!(
      sort_by_warnings[0]
        .message()
        .contains("expects type `fn(:string) -> 'K (:features :pass)`")
        && sort_by_warnings[0].message().contains("got `fn(:number) -> :number`"),
      "sort-by should preserve the String member type while leaving its key type generic: {sort_by_warnings:?}"
    );
    let list_apply_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/&list:apply"))
      .collect::<Vec<_>>();
    assert_eq!(
      list_apply_warnings.len(),
      1,
      "list apply should validate every function against the shared input/output contract: {warnings:?}"
    );
    assert!(
      list_apply_warnings[0]
        .message()
        .contains("arg 2 expects type `list<fn(string) -> 'ListApplyOutput>`")
        && list_apply_warnings[0].message().contains("got `list<fn(number) -> number>`"),
      "list apply should reject functions whose input disagrees with the List item type: {list_apply_warnings:?}"
    );
    assert!(
      list_apply_warnings[0]
        .message()
        .contains("Normalize the input/functions to T -> U, or split heterogeneous transformations into separate calls"),
      "list apply warnings should provide an executable migration direction: {list_apply_warnings:?}"
    );
    let list_apply_method_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_METHOD_ARG_TYPE_MISMATCH") && warning.message().contains("Method `.apply`"))
      .collect::<Vec<_>>();
    assert_eq!(
      list_apply_method_warnings.len(),
      1,
      "the public List.apply method should preserve its receiver binding: {warnings:?}"
    );
    assert!(
      list_apply_method_warnings[0]
        .message()
        .contains("arg 2 expects type `list<fn(string) -> 'U>`")
        && list_apply_method_warnings[0].message().contains("got `list<fn(number) -> number>`"),
      "List.apply should reject functions that disagree with the receiver item type: {list_apply_method_warnings:?}"
    );
    assert!(
      list_apply_method_warnings[0]
        .message()
        .contains("Normalize the input/functions to T -> U, or split heterogeneous transformations into separate calls"),
      "List.apply method warnings should provide an executable migration direction: {list_apply_method_warnings:?}"
    );
    let interleave_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/interleave"))
      .collect::<Vec<_>>();
    assert_eq!(
      interleave_warnings.len(),
      1,
      "interleave should reject lists with different member types: {warnings:?}"
    );
    assert!(
      interleave_warnings[0]
        .message()
        .contains("arg 2 expects type `list<number>`, but got `list<tag>`"),
      "interleave should report the member type bound by its first input: {interleave_warnings:?}"
    );
    assert!(
      interleave_warnings[0]
        .message()
        .contains("Normalize both lists to the same type, or declare List<Dynamic> explicitly"),
      "interleave should provide an executable homogeneous/open-boundary migration: {interleave_warnings:?}"
    );
    let partial_binding_warnings = warnings
      .iter()
      .filter(|warning| {
        warning.code() == Some("E_FN_ARG_TYPE_MISMATCH")
          && warning
            .message()
            .contains("type-fail-collection-member-contract.main/partial-generic-check")
      })
      .collect::<Vec<_>>();
    assert_eq!(
      partial_binding_warnings.len(),
      1,
      "a failed nested generic match must not leak bindings into later arguments: {partial_binding_warnings:?}"
    );
    assert!(
      partial_binding_warnings[0]
        .message()
        .contains("arg 1 expects type `map<'T, number>`")
        && partial_binding_warnings[0].message().contains("got `map<tag, string>`"),
      "the original mismatching generic container should remain actionable: {partial_binding_warnings:?}"
    );
    let partial_variadic_binding_warnings = warnings
      .iter()
      .filter(|warning| {
        warning.code() == Some("E_FN_ARG_TYPE_MISMATCH")
          && warning
            .message()
            .contains("type-fail-collection-member-contract.main/partial-variadic-check")
      })
      .collect::<Vec<_>>();
    assert_eq!(
      partial_variadic_binding_warnings.len(),
      1,
      "a rejected variadic generic argument must not leak bindings into later rest arguments: {partial_variadic_binding_warnings:?}"
    );
    assert!(
      partial_variadic_binding_warnings[0]
        .message()
        .contains("arg 1 expects type `map<'T, number>`")
        && partial_variadic_binding_warnings[0].message().contains("got `map<tag, string>`"),
      "the original mismatching variadic generic container should remain actionable: {partial_variadic_binding_warnings:?}"
    );
    // `add-watch!` and `remove-watch!` are the watcher procs themselves.
    let watch_warnings = warnings
      .iter()
      .filter(|warning| {
        warning.code() == Some("E_PROC_ARG_TYPE_MISMATCH")
          && (warning.message().contains("Proc `add-watch!`") || warning.message().contains("Proc `remove-watch!`"))
      })
      .collect::<Vec<_>>();
    assert_eq!(
      watch_warnings.len(),
      3,
      "Ref watcher calls should validate keys and callbacks: {warnings:?}"
    );
    assert!(
      watch_warnings.iter().any(|warning| {
        warning.message().contains("Proc `add-watch!` arg 2 expects type `:tag`") && warning.message().contains("got `:string`")
      }),
      "add-watch! should require a Tag key: {watch_warnings:?}"
    );
    assert!(
      watch_warnings.iter().any(|warning| {
        warning
          .message()
          .contains("Proc `add-watch!` arg 3 expects type `fn(:string, :string) -> :unit (:features :state :io)`")
          && warning.message().contains("got `fn(:number) -> :number`")
      }),
      "add-watch! should bind both callback inputs to the Ref payload and require Unit: {watch_warnings:?}"
    );
    assert!(
      watch_warnings.iter().any(|warning| {
        warning.message().contains("Proc `remove-watch!` arg 2 expects type `:tag`") && warning.message().contains("got `:string`")
      }),
      "remove-watch! should require a Tag key: {watch_warnings:?}"
    );
    let variadic_method_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_METHOD_ARG_TYPE_MISMATCH") && warning.message().contains("Method `.merge` variadic"))
      .collect::<Vec<_>>();
    assert_eq!(
      variadic_method_warnings.len(),
      1,
      "variadic methods should validate every argument against receiver-bound generics: {warnings:?}"
    );
    assert!(
      variadic_method_warnings[0]
        .message()
        .contains("expects type `map<tag, number>`, but got `map<tag, tag>`"),
      "variadic method diagnostics should render substituted full container types: {variadic_method_warnings:?}"
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

#[test]
fn data_definition_rejects_extra_parentheses_around_indented_forms() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("defn main! ()\n  defenum BadShownMaybe ('T)\n    ({} ('T Debug))\n    (:some 'T)\n    (:none)");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    let err = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect_err("extra parentheses around indentation-generated forms should fail");
    let message = err.to_string();
    assert!(
      message.contains("remove the extra outer parentheses"),
      "unexpected data definition syntax error: {message}"
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
        .filter(|warning| warning.code() == Some("E_FN_ARG_TYPE_MISMATCH"))
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
fn struct_callback_unbound_slot_reports_the_binding_and_field_location() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    for strict in [false, true] {
      for bound in [false, true] {
        let _strict = if strict {
          StrictTypesReset::enabled()
        } else {
          StrictTypesReset::disabled()
        };
        builtins::effects::init_effects_states();
        let mut snapshot = snapshot::Snapshot::default();
        let mut file = snapshot::create_file_from_snippet(
          "def slot $ deftype-slot :op\n\ndef Handler $ fn (d!) &unit\n\ndefenum Op $ :inc\n\ndefstruct Props $ :on-click $ quote (JsNullish app.main/Handler)\n\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , slot\n    Props :on-click $ fn (d!)\n      d! $ :: :inc\n    , &unit",
        )
        .expect("callback fixture should parse");
        // A named callback alias gets its contract from CodeEntry metadata,
        // exactly as the published Respo event-handler definitions do.
        let schema = cirru_parser::parse("Fn\n  Fn *op $ -> Unit\n  -> Unit").unwrap().remove(0);
        file.defs.get_mut("Handler").unwrap().schema = snapshot::parse_schema_annotation_for_write(&schema).unwrap();
        snapshot.files.insert("app.main".to_owned(), file);
        if bound {
          snapshot
            .entries
            .get_mut("default")
            .unwrap()
            .type_slots
            .insert("op".to_owned(), "app.main/Op".to_owned());
        }
        let entries = prepare_snapshot_entries(snapshot);
        let warnings = RefCell::new(vec![]);
        let result =
          runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
        if bound {
          result.expect("a concrete entry binding should retain the callback contract");
          assert!(warnings.borrow().is_empty(), "{warnings:?}");
          run_check_only(&entries).expect("the same callback with a concrete nominal slot should pass");
        } else {
          let error = result.expect_err("an already-rejected unbound-slot callback should identify its actual cause");
          assert_eq!(error.code.as_deref(), Some("E_UNBOUND_TYPE_SLOT"));
          assert!(error.msg.contains("*op"), "{error}");
          assert!(error.msg.contains("on-click"), "{error}");
          assert!(
            error.msg.contains("calcit config set-type-slot :op namespace/definition"),
            "{error}"
          );
          let location = error.location.expect("callback failure should retain source coordinates");
          assert_eq!(location.ns.as_ref(), "app.main");
          assert_eq!(location.def.as_ref(), "main!");
          assert!(!location.coord.is_empty());
        }
      }
    }
  });
}

#[test]
fn struct_callback_unbound_slot_location_respects_macro_source_origins() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    for generated in [false, true] {
      // Compatibility mode reaches the constructor rather than rejecting the
      // source-written callback hint first; both paths share the renderer.
      let _strict = StrictTypesReset::disabled();
      builtins::effects::init_effects_states();
      let (macro_code, body, expected_coord) = if generated {
        (
          "defmacro into-props () $ quasiquote $ let\n    handler $ fn (d!)\n      hint-fn $ Fn\n        Fn *op $ -> Unit\n        -> Unit\n      , &unit\n  Props :on-click handler",
          "  into-props",
          vec![5, 0],
        )
      } else {
        (
          "defmacro into-props (handler)\n  quasiquote $ Props :on-click ~handler",
          "  let\n      handler $ fn (d!)\n        hint-fn $ Fn (Fn *op (-> Unit)) (-> Unit)\n        , &unit\n    into-props handler",
          vec![5, 2, 1],
        )
      };
      let snippet = format!(
        "def slot $ deftype-slot :op\ndefstruct Props\n  :on-click $ quote $ Fn (Fn *op (-> Unit)) (-> Unit)\n{macro_code}\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , slot\n{body}\n  , &unit"
      );
      let mut file = snapshot::create_file_from_snippet(&snippet).expect("macro callback fixture should parse");
      annotate_snippet_macros(&mut file);
      let mut snapshot = snapshot::Snapshot::default();
      snapshot.files.insert("app.main".to_owned(), file);
      let entries = prepare_snapshot_entries(snapshot);
      let warnings = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      assert!(
        result.is_err(),
        "generated callback: {generated}; expected the slot diagnostic, warnings: {warnings:?}"
      );
      let error = result.expect_err("the missing slot must still reject a macro-wrapped callback");
      assert_eq!(error.code.as_deref(), Some("E_UNBOUND_TYPE_SLOT"), "{error}");
      let location = error.location.expect("the slot error should retain a source origin");
      assert_eq!(location.ns.as_ref(), "app.main");
      assert_eq!(location.def.as_ref(), "main!");
      assert_eq!(*location.coord, expected_coord, "generated callback: {generated}");
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

#[test]
fn calling_a_dynamic_value_requires_fn_narrowing() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "let\n    call-open $ fn (x)\n      hint-fn $ Fn Dynamic (-> Dynamic)\n      x 1\n    call-narrowed $ fn (x)\n      hint-fn $ Fn Dynamic (-> Dynamic)\n      if (fn? x) (x 1) nil\n    call-typed $ fn (g)\n      hint-fn $ Fn\n        Fn Number $ -> Number\n        -> Number\n      g 1\n    call-computed $ fn (x)\n      hint-fn $ Fn Dynamic (-> Dynamic)\n      (identity x) 1\n    call-threaded $ fn (x)\n      hint-fn $ Fn Dynamic (-> Dynamic)\n      -> 1 x\n  [] call-open call-narrowed call-typed call-computed call-threaded",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("a Dynamic call head should preprocess with a warning");

    let warnings = warnings.borrow();
    let dynamic_calls = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_DYNAMIC_CALL"))
      .collect::<Vec<_>>();
    // The unnarrowed variable, the computed head and the caller-written head
    // that `->` moves into place report; `fn?` and an Fn schema prove the head.
    assert_eq!(dynamic_calls.len(), 3, "{warnings:?}");
    assert!(
      dynamic_calls[0].message().contains("variable `x`") && dynamic_calls[0].message().contains("`(if (fn? f)"),
      "{dynamic_calls:?}"
    );
  });
}

#[test]
fn literal_equality_narrows_only_the_true_branch() {
  run_with_large_stack(|| {
    let probe = |params: &str, schema: &str, body: &str| {
      format!("let\n    probe $ fn ({params})\n      hint-fn $ {schema}\n      {body}\n  , probe")
    };
    for (label, snippet, passes) in [
      ("tag", probe("x", "Fn Dynamic (-> Tag)", "if (= x :a) x :b"), true),
      ("tag-literal-first", probe("x", "Fn Dynamic (-> Tag)", "if (= :a x) x :b"), true),
      ("string", probe("x", "Fn Dynamic (-> Number)", "if (= x |a) (&str:count x) 0"), true),
      (
        "number-native",
        probe("x", "Fn Dynamic (-> Number)", "if (&= x 1) (&+ x 1) 0"),
        true,
      ),
      ("bool", probe("x", "Fn Dynamic (-> Bool)", "if (= x true) x false"), true),
      ("cond", probe("x", "Fn Dynamic (-> Tag)", "cond ((= x :a) x) (true :b)"), true),
      ("false-branch", probe("x", "Fn Dynamic (-> Tag)", "if (= x :a) :b x"), false),
      ("two-locals", probe("x y", "Fn Dynamic Dynamic (-> Tag)", "if (= x y) x :b"), false),
      ("nil-literal", probe("x", "Fn Dynamic (-> Tag)", "if (= x nil) x :b"), false),
    ] {
      assert_strict_snippet(&snippet, passes, label);
    }
  });
}

/// #1554: a value-position use of a branch without an else (or an explicit nil
/// join) is a strict `E_IMPLICIT_NIL_BRANCH` at a call argument or a concrete
/// return, an advisory at a `let` binding, and a `W_IMPLICIT_NIL_BRANCH` warning
/// in non-strict mode; statement position, an explicit else and an `Option`
/// result stay silent.
#[test]
fn value_position_nil_branch_is_a_strict_error() {
  run_with_large_stack(|| {
    let number_fn = |body: &str, ret: &str| format!("fn (x) (hint-fn $ Fn Number (-> {ret})) {body}");
    let compile = |function: String| {
      let entries = load_snippet_entries(&format!("let\n    probe $ {function}\n  probe 1"));
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      let _ = runner::preprocess::take_advisories();
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      let warnings = warnings.into_inner();
      (result, warnings, runner::preprocess::take_advisories())
    };
    let is_implicit = |code: Option<&str>| code.is_some_and(|code| code.ends_with("_IMPLICIT_NIL_BRANCH"));

    let strict = StrictTypesReset::enabled();
    for (label, function, missing_else) in [
      ("call-arg", number_fn("(str (if (> x 0) x))", "String"), true),
      ("call-arg-nil-join", number_fn("(str (if (> x 0) x nil))", "String"), false),
      ("typed-return", number_fn("(if (> x 0) x)", "Number"), true),
    ] {
      let (result, _, _) = compile(function);
      let err = result.expect_err(&format!("{label}: a value-position nil branch is a strict error"));
      assert_eq!(err.code.as_deref(), Some("E_IMPLICIT_NIL_BRANCH"), "{label}: {err:?}");
      assert!(
        err.msg.contains("Option :some value") && err.msg.contains("nil->option"),
        "{label}: {err:?}"
      );
      let reason = if missing_else { "without an else" } else { "`nil` while another" };
      assert!(err.msg.contains(reason), "{label}: {err:?}");
      // `case` / `cond` without a default raise on no match, so they never yield nil.
      assert!(!err.msg.contains("`case`"), "{label}: {err:?}");
    }
    // `when` always returns Unit in both branches under strict Bool, so returning
    // it from a typed Number function is diagnosed as a return-type mismatch.
    let (when_res, when_warnings, _) = compile(number_fn("(when (> x 0) x)", "Number"));
    assert!(
      when_res.is_ok(),
      "typed-return-when should preprocess with return mismatch warning: {when_res:?}"
    );
    assert!(
      when_warnings
        .iter()
        .any(|warning| warning.code() == Some("E_FN_RETURN_TYPE_MISMATCH") && warning.expected() == Some(":number")),
      "typed-return-when expected E_FN_RETURN_TYPE_MISMATCH, got {when_warnings:?}"
    );

    // Since `Optional<T>` was removed (#2233) a nil join has no Calcit type,
    // so binding one is a value use like any other.
    for (label, function) in [
      ("let-binding", number_fn("(let ((y (if (> x 0) x))) y)", "Dynamic")),
      ("let-nil-join", number_fn("(let ((y (if (> x 0) x nil))) y)", "Dynamic")),
      // Literal branches carry no coordinate; the location comes from the condition.
      ("let-literal-branches", number_fn("(let ((y (if (> x 0) nil 1))) y)", "Dynamic")),
      ("let-cond", number_fn("(let ((y (cond ((> x 0) nil) (true 1)))) y)", "Dynamic")),
      ("let-all-literals", number_fn("(let ((y (if (> 2 1) nil 1))) y)", "Dynamic")),
    ] {
      let (result, _, _) = compile(function);
      let err = result.expect_err(&format!("{label}: a `let` nil branch is a strict error"));
      assert_eq!(err.code.as_deref(), Some("E_IMPLICIT_NIL_BRANCH"), "{label}: {err:?}");
      assert!(err.msg.contains("let binding"), "{label}: {err:?}");
    }

    for (label, function) in [
      ("statement", number_fn("(do (if (> x 0) (println |hit)) x)", "Number")),
      ("explicit-else", number_fn("(let ((y (if (> x 0) x 0))) y)", "Number")),
      (
        "option-result",
        number_fn("(let ((y (if (> x 0) (Option :some x) (Option :none)))) y)", "Dynamic"),
      ),
      ("nil-option-arg", number_fn("(nil->option (if (> x 0) x))", "Dynamic")),
    ] {
      let (result, warnings, advisories) = compile(function);
      assert!(result.is_ok(), "{label}: {result:?}");
      assert!(warnings.iter().all(|w| !is_implicit(w.code())), "{label}: {warnings:?}");
      assert!(advisories.iter().all(|a| !is_implicit(a.code())), "{label}: {advisories:?}");
    }
    drop(strict);

    let _compat = StrictTypesReset::disabled();
    let (result, warnings, advisories) = compile(number_fn("(str (if (> x 0) x))", "String"));
    assert!(result.is_ok(), "compat mode does not block: {result:?}");
    assert!(warnings.iter().all(|w| !is_implicit(w.code())), "{warnings:?}");
    assert!(
      advisories.iter().any(|a| a.code() == Some("W_IMPLICIT_NIL_BRANCH")),
      "compat mode keeps the warning: {advisories:?}"
    );
    // Compatibility mode binds a nil join as an open value, not a nullable `T`.
    let (result, warnings, advisories) = compile(number_fn("(let ((y (if (> x 0) x))) (&+ y 1))", "Dynamic"));
    assert!(result.is_ok(), "compat mode does not block a let nil branch: {result:?}");
    assert!(
      warnings
        .iter()
        .all(|w| !w.code().is_some_and(|code| code.ends_with("TYPE_MISMATCH"))),
      "an open binding is not checked as a nullable Number: {warnings:?}"
    );
    assert!(
      advisories.iter().any(|a| a.code() == Some("W_IMPLICIT_NIL_BRANCH")),
      "compat mode notes the let nil branch: {advisories:?}"
    );
  });
}

/// #1554: a branch written wholly by a library macro is skipped, but a library
/// macro that wraps the caller's form in an else-less `if` is reported at the
/// caller; `nil->option` around the macro call, then an Option helper, is the fix.
#[test]
fn library_macro_wrapping_caller_form_reports_implicit_nil_at_the_caller() {
  run_with_large_stack(|| {
    let library = "defmacro maybe (c x)\n  quasiquote $ if ~c ~x\n\ndefmacro maybe-nil (c x)\n  quasiquote $ if ~c ~x nil\n\ndefmacro quiet ()\n  quasiquote $ if (> 1 0) |a";
    let compile = |body: &str| {
      let snippet = format!(
        "ns app.main $ :require $ lib.core :refer (maybe maybe-nil quiet)\n\nlet\n    probe $ fn (flag)\n      hint-fn $ Fn Bool (-> String)\n      , {body}\n  probe true"
      );
      let entries = load_snippet_entries_with_library("lib.core", library, &snippet);
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      let _ = runner::preprocess::take_advisories();
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      (result, warnings.into_inner(), runner::preprocess::take_advisories())
    };
    let is_implicit = |code: Option<&str>| code.is_some_and(|code| code.ends_with("_IMPLICIT_NIL_BRANCH"));
    let _strict = StrictTypesReset::enabled();

    let (result, _, _) = compile("(str |x (maybe flag |b))");
    let err = result.expect_err("a library macro wrapping the caller's form in an else-less `if` is reported");
    assert_eq!(err.code.as_deref(), Some("E_IMPLICIT_NIL_BRANCH"), "{err:?}");
    let location = err.location.as_ref().expect("the error points at the caller");
    assert_eq!(location.ns.as_ref(), "app.main", "{err:?}");

    for (label, body) in [
      ("nil-option-fix", "(str |x (.unwrap-or (nil->option (maybe-nil flag |b)) |))"),
      ("wholly-generated", "(str |x (quiet))"),
    ] {
      let (result, warnings, advisories) = compile(body);
      assert!(result.is_ok(), "{label}: {result:?}");
      assert!(warnings.iter().all(|w| !is_implicit(w.code())), "{label}: {warnings:?}");
      assert!(advisories.iter().all(|a| !is_implicit(a.code())), "{label}: {advisories:?}");
    }
  });
}

/// #1554: a live payload that contradicts the nullable contract it flows into is
/// still a mismatch (return, call argument, `let` binding), while a matching
/// payload stays accepted.
#[test]
fn nil_branch_live_payload_still_checks_against_the_contract() {
  run_with_large_stack(|| {
    let nullable_return = |body: &str| format!("fn (flag) (hint-fn $ Fn Bool (-> $ JsNullish Number)) {body}");
    let cases = [
      ("wrong-live-payload", false, nullable_return("(if flag |bad nil)"), Some("E_FN_RETURN_TYPE_MISMATCH")),
      ("wrong-reversed-payload", false, nullable_return("(if flag nil |bad)"), Some("E_FN_RETURN_TYPE_MISMATCH")),
      // 2-arg `if` false branch yields Unit, so in strict mode branches (String, Unit) mismatch JsNullish<Number>.
      ("missing-else-payload", true, nullable_return("(if flag |bad)"), Some("E_BRANCH_TYPE_MISMATCH")),
      ("matching-payload", false, nullable_return("(if flag 7 nil)"), None),
      (
        "wrong-argument-payload",
        false,
        "fn (flag) (hint-fn $ Fn Bool (-> Unit)) $ let ((take $ fn (v) (hint-fn $ Fn (JsNullish Number) (-> Unit)) &unit)) $ take $ if flag |bad nil".to_owned(),
        Some("E_LOCAL_FN_ARG_TYPE_MISMATCH"),
      ),
    ];
    for (label, strict, function, expected_code) in cases {
      let _strict_guard = if strict { Some(StrictTypesReset::enabled()) } else { None };
      let snippet = format!("let\n    probe $ {function}\n  probe true");
      let entries = load_snippet_entries(&snippet);
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      let warnings = warnings.borrow();
      let mut codes = warnings.iter().filter_map(LocatedWarning::code).collect::<Vec<_>>();
      if let Err(err) = &result
        && let Some(code) = err.code.as_deref()
      {
        codes.push(code);
      }
      match expected_code {
        Some(code) => assert!(
          codes.contains(&code),
          "{label}: expected {code}, got result={result:?}, warnings={warnings:?}"
        ),
        None => assert!(
          result.is_ok() && !codes.iter().any(|code| code.ends_with("TYPE_MISMATCH")),
          "{label}: a matching payload should be accepted, got result={result:?}, warnings={warnings:?}"
        ),
      }
    }
  });
}

/// #1554 regression set: every case is one definition in
/// `tests/fixtures/nil-branch-contracts.cirru`. A nil-yielding branch keeps its
/// inferred nullable evidence for every contract check (returns, function /
/// local / proc / method arguments, struct fields, `assert-type`, callbacks,
/// `let` / `do` / nested bodies and `.?-field`), so `E_IMPLICIT_NIL_BRANCH` may
/// reject more programs than main but must never accept one that main rejected.
/// Columns: case, rejected on main, rejected now, the `IMPLICIT_NIL_BRANCH` code
/// reported (`E_` as the error, `W_` as an advisory), or `None` when the case
/// keeps another diagnostic or none.
const NIL_BRANCH_CONTRACT_CASES: &[(&str, bool, bool, Option<&str>)] = &[
  ("ret-if-noelse", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-if-niljoin", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-when", true, true, None),
  ("ret-cond", true, true, None),
  ("ret-case", true, true, None),
  ("ret-nested-if", false, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-nested-if-else-nil", false, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-let-body", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-do-body", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-ok-match", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-option", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-option-bad", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-local", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-defn", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-defn-niljoin", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-defn-when", true, true, None),
  ("arg-defn-nested", false, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-proc-add", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-proc-plus", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-proc-strcount", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-method-slice", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-method-slice-recv", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-method-count", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("struct-field", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("struct-field-pct", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("list-literal", false, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("map-callback", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("hof-typed-fn-arg", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("assert-type", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("let-bound-ret", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("let-bound-arg-defn", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("let-bound-ok-arg", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("let-bound-fn-literal", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("let-bound-list", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("let-cross-def", false, true, None),
  ("let-cross-def2", false, true, None),
  ("direct-cross-def", false, true, None),
  ("let-bound-nilq", false, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("let-bound-plus", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-bool-ok", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-string-niljoin-ok", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-cond2", true, true, None),
  ("arg-defn-ok", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-defn-niljoin-ok", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-local-ok", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("arg-method-ok", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("struct-ctor-bad", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("struct-ctor-nil", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("struct-pct-nil", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("list-ret-ok", false, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("list-arg", false, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("tuple-arg", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("list-arg-nullable", false, false, None),
  ("map-callback-ok", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("hof-ok", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("qfield-nilq", false, false, None),
  ("qfield-take-str", true, true, None),
  ("qfield-let-take-str", true, true, None),
  ("qfield-nil-option", false, false, None),
  ("qfield-js-present", true, true, None),
  ("qfield-js-nullish-option", true, true, None),
  ("qfield-str-concat", false, false, None),
  ("m-slice-ok", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("m-slice-bad", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("m-slice-ok2", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("local-ok", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("local-ret-typed-fn-literal", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  // 2-arg `if` false branch yields Unit (not nil), so it no longer satisfies JsNullish<Number>.
  ("n3-jsnullish-ret", false, true, None),
  ("n3-jsnullish-ret-join", false, false, None),
  ("n3-jsnullish-arg", false, false, None),
  ("n3-dynamic-ret", false, false, None),
  ("n3-let-value", false, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("n3-statement", false, false, None),
  ("n3-nil-option", false, false, None),
];

#[test]
fn nil_branch_contracts_keep_every_main_rejection() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries("tests/fixtures/nil-branch-contracts.cirru");
    let mut mismatches = vec![];
    for (label, rejected_on_main, rejected, code) in NIL_BRANCH_CONTRACT_CASES {
      assert!(!rejected_on_main || *rejected, "{label}: a main rejection must stay rejected");
      let def = format!("case-{label}");
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      let _ = runner::preprocess::take_advisories();
      let result = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &def, &warnings, &CallStackList::default());
      let actual_rejected = result.is_err() || !warnings.borrow().is_empty();
      let error_code = result.as_ref().err().and_then(|err| err.code.clone());
      let advisory = runner::preprocess::take_advisories()
        .iter()
        .find(|note| note.code() == Some("W_IMPLICIT_NIL_BRANCH"))
        .map(|note| note.code().unwrap_or_default().to_owned());
      let implicit_error = error_code.filter(|code| code == "E_IMPLICIT_NIL_BRANCH");
      let actual_code = implicit_error.or(advisory);
      if actual_rejected != *rejected || actual_code.as_deref() != *code {
        mismatches.push(format!(
          "{label}: rejected={actual_rejected} (expected {rejected}), code={actual_code:?} (expected {code:?}), result={result:?}, warnings={:?}",
          warnings.borrow()
        ));
      }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
  });
}

#[test]
fn unwrap_or_nil_on_an_open_option_reports_the_erasure() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "let\n    erase-method $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> Dynamic)\n      .unwrap-or (get m :a) nil\n    erase-postfix $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> Dynamic)\n      let\n          a $ get m :a\n        a .unwrap-or nil\n    erase-call $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> Dynamic)\n      option:unwrap-or (get m :a) nil\n    erase-match $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> Dynamic)\n      match (get m :a)\n        (:some v) v\n        (:none) nil\n    keep-match $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> Dynamic)\n      match (get m :a)\n        (:some v) v\n        (:none) 0\n    to-host $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic)\n        -> $ JsNullish Dynamic\n      .unwrap-or (get m :a) nil\n    keep-option $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> Dynamic)\n      get m :a\n    with-default $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> Dynamic)\n      .unwrap-or (get m :a) 0\n  [] erase-method erase-postfix erase-call erase-match keep-match to-host keep-option with-default",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    let _ = runner::preprocess::take_advisories();

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("erasing an Option to nil should preprocess");

    // The erasure is not a preprocess warning: it is collected with the
    // advisories, and the check splits it out as a blocking error (#2124).
    let warnings = warnings.borrow();
    assert!(
      warnings.iter().all(|warning| warning.code() != Some("E_OPTION_ERASED_TO_NIL")),
      "{warnings:?}"
    );
    let advisories = runner::preprocess::take_advisories();
    let erased = advisories
      .iter()
      .filter(|advisory| advisory.code() == Some("E_OPTION_ERASED_TO_NIL"))
      .collect::<Vec<_>>();
    // The method in both call forms, the direct call and the `(:none) nil` branch report; keeping
    // the Option, choosing a concrete default or declaring a `JsNullish`
    // return for a host does not.
    assert_eq!(erased.len(), 4, "{advisories:?}");
    assert!(erased[0].message().contains("back into nil"), "{erased:?}");
  });
}

#[test]
fn percent_struct_literal_on_a_named_defstruct_is_an_advisory() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "defstruct Point (:x Number) (:y Number)\ndefn main! ()\n  let\n      a $ %{} Point (:x 1) (:y 2)\n      b $ Point :x 1 :y 2\n      L $ defstruct L (:z Number)\n      c $ %{} L (:z 1)\n      d $ %{} _ (:z 1)\n    [] a b c d",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    let _ = runner::preprocess::take_advisories();

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("`%{}` on a known Struct should still preprocess");

    let advisories = runner::preprocess::take_advisories();
    let notes = advisories
      .iter()
      .filter(|advisory| advisory.code() == Some("W_PERCENT_STRUCT_LITERAL"))
      .map(|advisory| advisory.message().to_owned())
      .collect::<Vec<_>>();
    // Only the written `%{} Point`: the direct constructor (which lowers
    // through `%{}`), a `let`-bound prototype and `%{} _` are not noted.
    assert_eq!(notes.len(), 1, "{advisories:?}");
    assert!(notes[0].contains("`Point :x … :y …`"), "{notes:?}");
    assert!(notes[0].contains("named-struct-constructor-v1"), "{notes:?}");
    assert!(
      warnings
        .borrow()
        .iter()
        .all(|warning| warning.code() != Some("W_PERCENT_STRUCT_LITERAL")),
      "the note must not block the check"
    );
    let (blocking, _) = runner::preprocess::split_blocking_advisories(advisories);
    assert!(
      blocking.iter().all(|advisory| advisory.code() != Some("W_PERCENT_STRUCT_LITERAL")),
      "the note must stay in the non-blocking partition"
    );
  });
}

#[test]
fn legacy_decoder_target_type_is_an_advisory() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "let\n    xs $ decode-map-as ([] 1 2) $ :: 'List 'Number\n    n $ parse-cirru-edn-as |1 :number\n    ys $ decode-map-as ([] 1 2) $ quote $ List Number\n  [] xs n ys",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    let _ = runner::preprocess::take_advisories();

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("a legacy decoder target type should still preprocess");

    let advisories = runner::preprocess::take_advisories();
    let legacy = advisories
      .iter()
      .filter(|advisory| advisory.code() == Some("W_LEGACY_TYPE_SYNTAX"))
      .map(|advisory| advisory.message().to_owned())
      .collect::<Vec<_>>();
    // The compact `quote $ List Number` target is already canonical.
    assert_eq!(legacy.len(), 2, "{advisories:?}");
    assert!(legacy[0].contains("quote $ List Number"), "{legacy:?}");
    assert!(legacy[1].contains("'Number"), "{legacy:?}");
    assert!(
      warnings
        .borrow()
        .iter()
        .all(|warning| warning.code() != Some("W_LEGACY_TYPE_SYNTAX")),
      "the advisory must not block the check"
    );
  });
}

#[test]
fn legacy_quoted_declaration_types_are_rejected() {
  let mut file = snapshot::create_file_from_snippet(
    "defstruct Point (:x 'Number) (:y Number)\n\ndefenum Shape (:circle 'Number) (:none)\n\ndefunion Input 'String Number\n\ndefstruct Box (:items $ quote $ List Number)\n\ndefunion Pair String $ quote $ List Number\n\ndefn main! () $ let\n    Local $ defstruct Local (:v 'String)\n  , Local",
  )
  .expect("test snippet should parse");
  file.defs.get_mut("main!").expect("main! def").tests.push(snapshot::TestEntry {
    name: "legacy".into(),
    code: cirru_parser::parse("do $ defunion T 'String Number").expect("test code")[0].to_owned(),
    tags: HashSet::new(),
  });
  let mut snapshot = snapshot::Snapshot::default();
  snapshot.files.insert("app.main".to_owned(), file);
  let namespaces = HashSet::from(["app.main".to_owned()]);

  let hits = snapshot::collect_legacy_quoted_declaration_hits(&snapshot, &namespaces);
  let messages = hits.iter().map(|hit| hit.message()).collect::<Vec<_>>();
  // Bare names and compound `quote $ List Number` types are already canonical.
  assert_eq!(hits.len(), 5, "{messages:?}");
  let rewritten = hits.iter().map(|hit| hit.rewritten.as_str()).collect::<Vec<_>>();
  assert_eq!(
    rewritten,
    [
      "defunion Input String Number",
      "defstruct Point (:x Number) (:y Number)",
      "defenum Shape (:circle Number) (:none)",
      "defstruct Local (:v String)",
      "defunion T String Number",
    ],
    "{messages:?}"
  );
  assert!(
    hits[..3].iter().all(|hit| hit.coord.is_empty()),
    "top-level declarations sit at the root of their own CodeEntry"
  );
  assert!(
    hits[3..].iter().all(|hit| !hit.coord.is_empty()),
    "nested declarations point inside their enclosing form"
  );
  assert!(
    messages
      .iter()
      .all(|message| message.contains("E_LEGACY_TYPE_SYNTAX") && message.contains("calcit <snapshot> edit format")),
    "{messages:?}"
  );

  // `edit format` rewrites all of them, tests included.
  snapshot::canonicalize_snapshot_type_syntax(&mut snapshot);
  let left = snapshot::collect_legacy_quoted_declaration_hits(&snapshot, &namespaces);
  assert!(left.is_empty(), "{left:?}");
}

#[test]
fn option_erased_to_nil_is_exempt_only_as_a_direct_host_argument() {
  run_with_large_stack(|| {
    let entries = load_fixture_entries("calcit/type-fail/option-nil-host-position.cirru");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    let _ = runner::preprocess::take_advisories();

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("host-position fixture should preprocess");

    let advisories = runner::preprocess::take_advisories();
    let mut erased = advisories
      .iter()
      .filter(|advisory| advisory.code() == Some("E_OPTION_ERASED_TO_NIL"))
      .map(|advisory| advisory.location().def.to_string())
      .collect::<Vec<_>>();
    erased.sort();
    // A `js/` call, native and external-object method calls (prefix and
    // postfix), a host-property `set!`, an argument for a `JsNullish`
    // parameter and a `(:none) nil` branch that is the host argument stay
    // quiet, and so does a host argument reached through an `if` branch, a
    // `let` tail or a macro expansion (#2565); the `:js-ffi` feature alone
    // does not exempt a nil tested by business code, bound to a local,
    // `set!` on a local, passed to an ordinary parameter or consumed by an
    // ordinary call before the host sees it. A local function named `do` is
    // such an ordinary call; a postfix receiver is a host value like an argument.
    assert_eq!(
      erased,
      [
        "bound-in-ffi",
        "business-in-ffi",
        "local-do-before-host",
        "local-set-in-ffi",
        "to-plain-param",
        "wrapped-before-host"
      ],
      "{advisories:?}"
    );
  });
}

/// #2565: a consumer that only tests the erased value gets an exact
/// `match` rewrite in the hint; any other consumer keeps the general advice.
#[test]
fn option_erased_to_nil_suggests_rewrites_that_keep_results() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "let\n    with-or $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> Dynamic)\n      or (.unwrap-or (get m :a) nil) 1\n    with-if $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> String)\n      if (.unwrap-or (get m :a) nil) |yes |no\n    with-nil $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> Bool)\n      nil? $ (get m :a) .unwrap-or nil\n    with-match $ fn (m d)\n      hint-fn $ Fn (Map Tag Dynamic) String (-> Dynamic)\n      or\n        match (get m :a)\n          (:some x) (str x)\n          (:none) nil\n        , d\n    with-capture $ fn (m v)\n      hint-fn $ Fn (Map Tag Dynamic) Dynamic (-> Dynamic)\n      or (.unwrap-or (get m :a) nil) v |z\n    shadowed $ fn (m x)\n      hint-fn $ Fn (Map Tag Dynamic) String (-> Dynamic)\n      or\n        match (get m :a)\n          (:some x) (str x)\n          (:none) nil\n        , x\n    with-str $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> String)\n      str $ .unwrap-or (get m :a) nil\n  [] with-or with-if with-nil with-match with-capture shadowed with-str",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    let _ = runner::preprocess::take_advisories();

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("erasing an Option to nil should preprocess");

    let advisories = runner::preprocess::take_advisories();
    let hints = advisories
      .iter()
      .filter(|advisory| advisory.code() == Some("E_OPTION_ERASED_TO_NIL"))
      .map(|advisory| advisory.hint().map(str::to_owned))
      .collect::<Vec<_>>();
    assert_eq!(
      hints,
      [
        // Under strict Bool, `if` / `or` cannot consume arbitrary Dynamic payloads,
        // so Option erasure rewrite hints stay limited to predicate consumers (`nil?` / `non-nil?`).
        None,
        None,
        Some("rewrite `nil? $ (get m :a) .unwrap-or nil` as `match (get m :a) ((:some v) (nil? v)) $ (:none) true`".to_owned()),
        None,
        None,
        None,
        // `str` turns nil into text, so no rewrite keeps its result.
        None,
      ],
      "{advisories:?}"
    );
  });
}

/// #2565: the rewrite reads `or` as the core macro; a local `or` gets none,
/// and so does an `if` without an else branch, whose rewrite would keep a
/// `(:none) nil` branch.
#[test]
fn option_erased_to_nil_rewrites_only_core_consumers() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "let\n    or $ fn (a b)\n      hint-fn $ Fn Dynamic Dynamic (-> Dynamic)\n      , b\n    local-or $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> Dynamic)\n      or (.unwrap-or (get m :a) nil) 1\n    bare-if $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic) (-> Dynamic)\n      if (.unwrap-or (get m :a) nil) |yes\n  [] local-or bare-if",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    let _ = runner::preprocess::take_advisories();

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("erasing an Option to nil should preprocess");

    let advisories = runner::preprocess::take_advisories();
    let hints = advisories
      .iter()
      .filter(|advisory| advisory.code() == Some("E_OPTION_ERASED_TO_NIL"))
      .map(|advisory| advisory.hint().map(str::to_owned))
      .collect::<Vec<_>>();
    assert_eq!(hints, [None, None], "{advisories:?}");
  });
}

#[test]
fn option_erased_in_a_js_nullish_function_initializer_is_not_exempt() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "let\n    to-host-let $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic)\n        -> $ JsNullish Dynamic\n      let\n          x 1\n        .unwrap-or (get m :a) nil\n    to-host-do $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic)\n        -> $ JsNullish Dynamic\n      do 1 $ .unwrap-or (get m :a) nil\n    to-host-match $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic)\n        -> $ JsNullish Dynamic\n      match (get m :a)\n        (:some v) v\n        (:none) nil\n    business-in-nullish $ fn (m)\n      hint-fn $ Fn (Map Tag Dynamic)\n        -> $ JsNullish Dynamic\n      let\n          ignored $ .unwrap-or (get m :a) nil\n        , nil\n  [] to-host-let to-host-do to-host-match business-in-nullish",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    let _ = runner::preprocess::take_advisories();

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("an Option erasure inside a nullish-returning function should preprocess");

    let advisories = runner::preprocess::take_advisories();
    let erased = advisories
      .iter()
      .filter(|advisory| advisory.code() == Some("E_OPTION_ERASED_TO_NIL"))
      .collect::<Vec<_>>();
    assert_eq!(erased.len(), 1, "{advisories:?}");
    assert_eq!(erased[0].location().def.as_ref(), "main!", "{advisories:?}");
    // The sole advisory must point inside `business-in-nullish`'s let
    // initializer. This also proves that the three other tail-return forms
    // (let, do, and match) remain exempt.
    assert_eq!(
      erased[0].location().coord.as_ref(),
      &vec![3, 1, 3, 1, 3, 1, 0, 1, 1, 1],
      "{advisories:?}"
    );
  });
}

/// #2233: legacy `Optional<T>` was removed in 0.33.0. Loading a Snapshot that
/// still spells it fails once with `E_LEGACY_OPTIONAL_TYPE`, listing every
/// position (struct fields, enum payloads, a `def` value schema, a local
/// `hint-fn`, `assert-type` and a decoder target) while the `Option<T>` forms
/// stay unlisted.
#[test]
fn legacy_optional_is_rejected_when_loading() {
  let path = "tests/fixtures/legacy-optional-advisory.cirru";
  let content = fs::read_to_string(path).expect("read fixture");
  let data = cirru_edn::parse(&content).expect("parse fixture");
  let error = snapshot::load_snapshot_data(&data, path).expect_err("legacy Optional must fail the load");
  assert!(
    error.contains("[E_LEGACY_OPTIONAL_TYPE]") && error.contains("removed in 0.33.0"),
    "{error}"
  );
  for listed in [
    "legacy-optional-advisory.main/Box `Optional Number`",
    "legacy-optional-advisory.main/Choice `Optional String`",
    "legacy-optional-advisory.main/fallback :schema `Optional Number`",
    "legacy-optional-advisory.main/use-assert-type `Optional Number`",
    "legacy-optional-advisory.main/use-decoder `:: 'Optional 'Number`",
    "legacy-optional-advisory.main/use-hint-fn `Optional Number`",
  ] {
    assert!(error.contains(listed), "missing `{listed}`: {error}");
  }
  assert!(!error.contains("OptionBox") && !error.contains("use-option-hint"), "{error}");
}

#[test]
fn constant_truthy_conditions_report_with_replacement_guidance() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "let\n    xs $ [] 1 2\n    s |text\n    n? $ = 1 1\n  [] (if (count xs) |non-empty |empty) (if s |yes |no) (when (count xs) |some) (and (count xs) n?) (or s |fallback) (if (empty? xs) |a |b) (if (> (count xs) 0) |a |b) (and n? (count xs)) (if n? |a |b) (if s s false)",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("constant-truthy conditions should preprocess with warnings");

    let warnings = warnings.borrow();
    let truthy = warnings
      .iter()
      .filter(|warning| warning.code() == Some("E_CONSTANT_TRUTHY_CONDITION"))
      .collect::<Vec<_>>();
    // Every constant-truthy operand across `if`, `when`, `and`, and `or`
    // (including value-looking fallbacks like `|fallback` and tail `(count xs)`)
    // reports `E_CONSTANT_TRUTHY_CONDITION`; only the Bool tests stay quiet.
    assert_eq!(truthy.len(), 8, "expected eight constant-truthy conditions, got: {warnings:?}");
    assert_eq!(
      truthy.iter().filter(|warning| warning.message().contains("type Number")).count(),
      4,
      "{truthy:?}"
    );
    assert_eq!(
      truthy
        .iter()
        .filter(|warning| warning.message().contains("type String") && warning.message().contains("`blank?`"))
        .count(),
      4,
      "{truthy:?}"
    );
    assert_eq!(
      truthy.iter().filter(|warning| warning.message().contains("always taken")).count(),
      4,
      "{truthy:?}"
    );
    assert_eq!(
      truthy
        .iter()
        .filter(|warning| warning.message().contains("`and` operand has type"))
        .count(),
      2,
      "{truthy:?}"
    );
    assert_eq!(
      truthy
        .iter()
        .filter(|warning| warning.message().contains("`or` operand has type"))
        .count(),
      2,
      "{truthy:?}"
    );
  });
}

/// `&enum:nth` and `%::` keep their declared receiver contracts: a known
/// non-Enum receiver is rejected during strict checking instead of failing at
/// runtime (#2181). A Rust type-fail test is needed because `:tests` only run
/// after a clean check and cannot express an expected preprocessing diagnostic.
#[test]
fn strict_type_fail_enum_primitives_reject_known_non_enum_receivers() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    for (source, proc_name, expected, actual) in [
      (
        "let\n    bad $ fn (xs)\n      hint-fn $ Fn (List String) (-> Tag)\n      &enum:nth xs 0\n  bad $ [] |x",
        "&enum:nth",
        "`:enum`",
        "`list<string>`",
      ),
      ("let ((xs ([] 1)))\n  %:: xs :a", "%::", "`:enum-def`", "`list<number>`"),
    ] {
      let entries = load_snippet_entries(source);
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect("a non-Enum receiver should produce a type diagnostic, not a preprocessing failure");
      let warnings = warnings.borrow();
      assert!(
        warnings.iter().any(|warning| warning.code() == Some("E_PROC_ARG_TYPE_MISMATCH")
          && warning
            .message()
            .contains(&format!("Proc `{proc_name}` arg 1 expects type {expected}, but got {actual}"))),
        "`{source}` should reject the non-Enum receiver: {warnings:?}"
      );
    }

    // Enum receivers, including nominal enum values, stay warning-free.
    for source in [
      "let\n    good $ fn (xs)\n      hint-fn $ Fn Enum (-> Tag)\n      &enum:nth xs 0\n  good $ :: :ns |demo",
      "&enum:nth (Option :some 1) 1",
      "&enum:nth (%:: (defenum E (:a Number)) :a 1) 1",
    ] {
      let accepted = load_snippet_entries(source);
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&accepted.init_ns, &accepted.init_def, &warnings, &CallStackList::default())
        .expect("an Enum receiver should preprocess");
      assert!(
        warnings.borrow().is_empty(),
        "`{source}` should stay clean: {:?}",
        warnings.borrow()
      );
    }
  });
}

#[test]
fn generic_return_kind_guard_cannot_forge_payload_type() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let forge = |branch: &str| {
      format!(
        "let\n    forge $ fn (x)\n      hint-fn $ Fn T (-> T) (:generics T)\n      if (list? x) {branch} $ raise |expected-list\n  , &unit"
      )
    };
    // A kind guard on `x: T` says nothing about the members `T` was bound to.
    for branch in ["([] |wrong)", "(let ((y ([] |wrong))) y)"] {
      let entries = load_snippet_entries(&forge(branch));
      let error = runner::preprocess::ensure_ns_def_compiled(
        &entries.init_ns,
        &entries.init_def,
        &RefCell::new(vec![]),
        &CallStackList::default(),
      )
      .expect_err("a forged List payload must not prove the generic return");
      assert!(
        matches!(error.code.as_deref(), Some("E_FN_RETURN_UNPROVEN" | "E_ERASED_GENERIC_RELATION")),
        "{branch}: {error}"
      );
      let location = error
        .location
        .as_ref()
        .expect("the producer diagnostic must keep its source location");
      assert_eq!(location.ns.as_ref(), "app.main", "{error}");
      assert_eq!(location.def.as_ref(), "main!", "{error}");
    }
    // Returning the guarded input itself, or a local alias of it, keeps `T`.
    for branch in ["x", "(let ((y x)) y)"] {
      let entries = load_snippet_entries(&forge(branch));
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .unwrap_or_else(|error| panic!("{branch}: the guarded input still proves T: {error}"));
    }
  });
}

/// Compact quote type expressions report grammar errors instead of falling
/// back to a List value type (#2433).
#[test]
fn type_fail_compact_type_expr_reports_syntax_errors() {
  run_with_large_stack(|| {
    for (code, expected) in [
      ("assert-type 1 $ quote $ Fn Number -> Number", "bare `->`"),
      ("assert-type ([] 1) $ quote $ List number", "`number` is not a type name"),
      ("assert-type ([] 1) $ quote $ list Number", "`list` is not a type name"),
      ("assert-type 1 $ quote $ Fn Number", "needs a return clause"),
      // A bare type application is read as a compact type, with the same grammar checks.
      ("assert-type ([] 1) $ List number", "`number` is not a type name"),
    ] {
      let entries = load_snippet_entries(code);
      let warnings = RefCell::new(vec![]);
      let error = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect_err(code);
      let message = error.to_string();
      assert!(message.contains("E_TYPE_EXPR_SYNTAX"), "{code}: unexpected error {message}");
      assert!(message.contains(expected), "{code}: expected `{expected}` in {message}");
    }

    let entries = load_snippet_entries("assert-type ([] |a) $ quote $ List Number");
    let warnings = RefCell::new(vec![]);
    let error = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect_err("a compact List type keeps its element type");
    assert!(error.to_string().contains("E_ASSERT_TYPE_MISMATCH"), "unexpected error: {error}");

    // Without its quote the type keeps the same element check (#2581).
    let entries = load_snippet_entries("assert-type ([] |a) $ List Number");
    let warnings = RefCell::new(vec![]);
    let error = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect_err("a bare List type keeps its element type");
    assert!(error.to_string().contains("E_ASSERT_TYPE_MISMATCH"), "unexpected error: {error}");
  });
}

#[test]
fn type_fail_tuple_reads_and_list_operations_report_tuple_diagnostics() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let tuple = "assert-type ([] 1 |a) (quote $ Tuple Number String)";
    for (body, expected) in [
      ("nth t 2", "index 2 is out of range"),
      ("let ((i 0)) (nth t i)", "must be a literal number"),
    ] {
      let entries = load_snippet_entries(&format!("let ((t ({tuple}))) ({body})"));
      let error = runner::preprocess::ensure_ns_def_compiled(
        &entries.init_ns,
        &entries.init_def,
        &RefCell::new(vec![]),
        &CallStackList::default(),
      )
      .expect_err("an unproven tuple position must be rejected");
      assert_eq!(error.code.as_deref(), Some("E_TUPLE_INDEX"), "{body}: {error}");
      assert!(error.msg.contains(expected), "{body}: {error}");
    }

    for (body, code) in [
      // `let[]` expands to `&list:nth`, which checks the same literal index.
      ("let[] (a b c) t a", "E_TUPLE_INDEX"),
      // A heterogeneous tuple has no element type, so it is never a List.
      ("conj t 1", "E_FN_ARG_TYPE_MISMATCH"),
      // Shape-rebuilding traits need the value used as `List T` first.
      ("rest t", "E_GENERIC_WHERE_BOUND_MISMATCH"),
      // A tuple read still goes through the ordinary proc arity check.
      ("&list:nth t 0 1", "E_PROC_ARG_TYPE_MISMATCH"),
      // Only a Number index reads a homogeneous tuple as `List T`.
      (
        "let ((h (assert-type ([] 1 2) (quote $ Tuple Number Number)))) (&list:nth h :bad)",
        "E_TUPLE_INDEX",
      ),
    ] {
      let entries = load_snippet_entries(&format!("let ((t ({tuple}))) ({body})"));
      let warnings = RefCell::new(vec![]);
      let _ = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      let warnings = warnings.borrow();
      assert!(
        warnings.iter().any(|warning| warning.code() == Some(code)),
        "{body} should report {code}, got: {warnings:?}"
      );
    }

    let entries = load_snippet_entries("let ((t (assert-type ([] |a 1) (quote $ Tuple Number String)))) t");
    let error = runner::preprocess::ensure_ns_def_compiled(
      &entries.init_ns,
      &entries.init_def,
      &RefCell::new(vec![]),
      &CallStackList::default(),
    )
    .expect_err("a literal is checked position by position");
    assert_eq!(error.code.as_deref(), Some("E_ASSERT_TYPE_MISMATCH"), "{error}");
  });
}

/// A type name that resolves to nothing fails the check in a schema, a
/// `hint-fn` and an `assert-type`, and the error names the nearest real
/// type (#2691).
#[test]
fn strict_type_fail_unknown_type_name_suggests_candidates() {
  run_with_large_stack(|| {
    let cases = [
      (
        "calcit/type-fail/unknown-type-name-schema.cirru",
        "`Nubmer` in schema at schema.args.0.item",
        "`Number`",
      ),
      (
        "calcit/type-fail/unknown-type-name-alias.cirru",
        "`lib/Pointt` in schema at schema.args.0",
        "`lib/Point`",
      ),
      (
        "calcit/type-fail/unknown-type-name-def.cirru",
        "`Nubmer` in schema at schema",
        "`Number`",
      ),
      (
        "calcit/type-fail/unknown-type-name-hint-fn.cirru",
        "`Nubmer` in hint-fn",
        "`Number`",
      ),
      (
        "calcit/type-fail/unknown-type-name-assert-type.cirru",
        "`Strnig` in assert-type",
        "`String`",
      ),
    ];
    let _strict = StrictTypesReset::enabled();
    for (fixture, place, candidate) in cases {
      let entries = load_fixture_entries(fixture);
      let err = run_check_only(&entries).expect_err(fixture);
      assert!(
        err.contains("E_UNKNOWN_TYPE_NAME"),
        "{fixture} should report E_UNKNOWN_TYPE_NAME: {err}"
      );
      assert!(err.contains(place), "{fixture} should point at {place}: {err}");
      assert!(
        err.contains(&format!("did you mean {candidate}")),
        "{fixture} should suggest {candidate}: {err}"
      );
    }
  });
}

/// `Self` is the receiver type only in a `deftrait` signature or a `defimpl`
/// method hint; an ordinary `defn` hint still reports an unknown type name
/// (#2727).
#[test]
fn strict_type_fail_self_outside_defimpl_hint() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries("calcit/type-fail/self-outside-defimpl-hint-fn.cirru");
    let err = run_check_only(&entries).expect_err("a plain defn hint must reject Self");
    assert!(err.contains("E_UNKNOWN_TYPE_NAME"), "{err}");
    assert!(err.contains("`Self` in hint-fn"), "{err}");
  });
}

/// A plain trait in a type position is rejected with `E_TRAIT_AS_TYPE` and
/// points to the `:where` bound (or `assert-traits`) that states the capability.
#[test]
fn strict_type_fail_trait_as_type_points_to_where_bound() {
  run_with_large_stack(|| {
    let cases = [
      (
        "calcit/type-fail/trait-as-type-schema.cirru",
        "`lib/Labeled` as a type in schema at schema.args.0.item",
        "(:where $ T lib/Labeled)",
      ),
      (
        "calcit/type-fail/trait-as-type-value-schema.cirru",
        "`lib/Labeled` as a type in schema at schema.item",
        "(:where $ T lib/Labeled)",
      ),
      (
        "calcit/type-fail/trait-as-type-hint-fn.cirru",
        "`lib/Labeled` as a type in hint-fn at hint-fn.args.0",
        "(:where $ T lib/Labeled)",
      ),
      (
        "calcit/type-fail/trait-as-type-assert-type.cirru",
        "as a type in assert-type",
        "assert-traits x app.lib/Labeled",
      ),
    ];
    let _strict = StrictTypesReset::enabled();
    for (fixture, place, hint) in cases {
      let entries = load_fixture_entries(fixture);
      let err = run_check_only(&entries).expect_err(fixture);
      assert!(err.contains("E_TRAIT_AS_TYPE"), "{fixture} should report E_TRAIT_AS_TYPE: {err}");
      assert!(err.contains(place), "{fixture} should point at {place}: {err}");
      assert!(err.contains(hint), "{fixture} should suggest {hint}: {err}");
    }
  });
}

/// Each `defunion` rule rejects its fixture with a stable code: member
/// restrictions, exhaustive type-pattern `match`, no predicate or literal
/// narrowing, no methods or stringification, no Dynamic into a union, and
/// no inline `Union` in schemas.
#[test]
fn strict_type_fail_union_rules_report_stable_codes() {
  run_with_large_stack(|| {
    let cases = [
      ("calcit/type-fail/union-not-exhaustive.cirru", "E_UNION_MATCH_NOT_EXHAUSTIVE"),
      ("calcit/type-fail/union-unknown-member.cirru", "E_UNION_MATCH_PATTERN"),
      ("calcit/type-fail/union-tag-pattern.cirru", "E_UNION_MATCH_PATTERN"),
      ("calcit/type-fail/union-type-pattern-on-dynamic.cirru", "E_UNION_MATCH_PATTERN"),
      ("calcit/type-fail/union-predicate.cirru", "E_UNION_PREDICATE"),
      ("calcit/type-fail/union-literal-eq.cirru", "E_UNION_LITERAL_EQ"),
      ("calcit/type-fail/union-str.cirru", "E_UNION_REQUIRES_MATCH"),
      ("calcit/type-fail/union-method.cirru", "E_UNION_REQUIRES_MATCH"),
      ("calcit/type-fail/union-postfix-method.cirru", "E_UNION_REQUIRES_MATCH"),
      ("calcit/type-fail/union-to-string.cirru", "E_FN_ARG_TYPE_MISMATCH"),
      ("calcit/type-fail/union-member-not-in-union.cirru", "E_FN_ARG_TYPE_MISMATCH"),
      ("calcit/type-fail/union-dynamic-to-union.cirru", "E_CALL_ARGUMENT_UNPROVEN"),
      ("calcit/type-fail/union-member-dynamic.cirru", "E_UNION_MEMBER_INVALID"),
      ("calcit/type-fail/union-member-result.cirru", "E_UNION_MEMBER_INVALID"),
      ("calcit/type-fail/union-option-nil.cirru", "E_FN_ARG_TYPE_MISMATCH"),
      ("calcit/type-fail/union-js-nullish-other.cirru", "E_FN_ARG_TYPE_MISMATCH"),
      ("calcit/type-fail/union-self-member.cirru", "E_UNION_MEMBER_INVALID"),
      ("calcit/type-fail/union-member-dup-kind.cirru", "E_UNION_MEMBER_INVALID"),
      ("calcit/type-fail/union-dup-pattern-name.cirru", "E_UNION_MEMBER_INVALID"),
      ("calcit/type-fail/union-unused-invalid.cirru", "E_UNION_MEMBER_INVALID"),
      ("calcit/type-fail/union-inline.cirru", "E_UNION_INLINE"),
      ("calcit/type-fail/union-where-bound.cirru", "E_GENERIC_WHERE_BOUND_MISMATCH"),
    ];
    let _strict = StrictTypesReset::enabled();
    for (fixture, code) in cases {
      let entries = load_fixture_entries(fixture);
      // Collected diagnostics (named `W_` before 0.42.0, #2699) fail the check
      // with a count only, so their codes are read from the collected list.
      if matches!(code, "E_FN_ARG_TYPE_MISMATCH" | "E_GENERIC_WHERE_BOUND_MISMATCH") {
        let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
        let _ = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
        let warnings = warnings.borrow();
        assert!(
          warnings.iter().any(|warning| warning.code() == Some(code)),
          "{fixture} should report {code}: {warnings:?}"
        );
      } else {
        let err = run_check_only(&entries).expect_err(fixture);
        assert!(err.contains(code), "{fixture} should report {code}: {err}");
      }
    }
  });
}

#[test]
fn proof_dissoc_in_keeps_known_map_root() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    // #2506 + bounded mixed path lowering: literal paths through Map/List
    // containers preserve the declared Map root.
    let kept = load_fixture_entries("calcit/type-fail/dissoc-in-map-root.cirru");
    run_check_only(&kept).expect("dissoc-in on a known Map root proves the declared Map return");

    let list_kept = load_fixture_entries("calcit/type-fail/dissoc-in-list-payload.cirru");
    run_check_only(&list_kept).expect("dissoc-in over literal Map+List path proves the declared Map return");

    // A dynamic path remains unexpanded and keeps the open public contract.
    let mut open_file = snapshot::create_file_from_snippet(
      "defn remove-dyn (data path)\n  dissoc-in data path\n\ndefn main! ()\n  remove-dyn ({} (:groups ([] 1 2))) ([] :groups :sub)\n  , &unit\n\ndefn reload! () &unit",
    )
    .expect("dynamic dissoc-in snippet should parse");
    let fn_schema = |arg_types: Vec<Arc<CalcitTypeAnnotation>>, return_type: CalcitTypeAnnotation| {
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        open_reason: Default::default(),
        cases: Default::default(),
        guard: None,
        runtime_arity: None,
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types,
        return_type: Arc::new(return_type),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      })))
    };
    let map_list_type = CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::Tag),
      Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Number))),
    );
    open_file.defs.get_mut("remove-dyn").expect("remove-dyn").schema = fn_schema(
      vec![
        Arc::new(map_list_type.clone()),
        Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Tag))),
      ],
      map_list_type,
    );
    open_file.defs.get_mut("main!").expect("main!").schema = fn_schema(vec![], CalcitTypeAnnotation::Unit);
    open_file.defs.get_mut("reload!").expect("reload!").schema = fn_schema(vec![], CalcitTypeAnnotation::Unit);
    let mut snapshot = snapshot::Snapshot::default();
    snapshot.files.insert("app.main".to_owned(), open_file);
    let open = prepare_snapshot_entries(snapshot);
    let err = run_check_only(&open).expect_err("a dynamic path must not be claimed by the typed path rule");
    assert!(err.contains("E_FN_RETURN_UNPROVEN"), "unexpected dissoc-in proof error: {err}");
  });
}

/// #2566: a bare `Enum` / `Struct` schema slot names the type every checked
/// call passes, lists several types, and stays unchanged without calls.
#[test]
fn broad_type_notes_name_the_type_checked_calls_pass() {
  run_with_large_stack(|| {
    let path = "tests/fixtures/broad-type-candidates.cirru";
    let content = fs::read_to_string(path).expect("read fixture");
    let data = cirru_edn::parse(&content).expect("parse fixture");
    let snapshot = snapshot::load_snapshot_data(&data, path).expect("load fixture");
    let namespaces = HashSet::from(["broad-type-candidates.main".to_owned()]);
    let _ = runner::preprocess::take_advisories();
    super::push_broad_type_advisories(&snapshot, &namespaces);
    let entries = prepare_snapshot_entries(snapshot);
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("fixture should preprocess");
    // The fixture itself adds no blocking warnings; the test harness does
    // not register host effects such as `eprintln` used by core macros.
    let blocking = warnings
      .borrow()
      .iter()
      .filter(|warning| warning.location().ns.as_ref() == "broad-type-candidates.main")
      .map(LocatedWarning::to_string)
      .collect::<Vec<_>>();
    assert!(blocking.is_empty(), "{blocking:?}");

    let notes = runner::preprocess::take_advisories()
      .into_iter()
      .filter(|advisory| advisory.code() == Some("E_BROAD_TYPE"))
      .map(|advisory| (advisory.location().def.to_string(), advisory.hint().map(str::to_owned)))
      .collect::<Vec<_>>();
    // A `hint-fn` inside a body adds its own note without a slot; the
    // schema's note is the one that can carry a candidate.
    let hint = |def: &str| {
      let mut own = notes.iter().filter(|(name, _)| name == def).peekable();
      assert!(own.peek().is_some(), "no note for {def}: {notes:?}");
      own.find_map(|(_, hint)| hint.clone())
    };
    let unique = |def: &str, place: &str, name: &str| {
      let hint = hint(def).unwrap_or_else(|| panic!("no hint for {def}: {notes:?}"));
      assert!(
        hint.starts_with(&format!(
          "write `broad-type-candidates.main/{name}` at {place} of `broad-type-candidates.main/{def}`"
        )),
        "{hint}"
      );
    };
    unique("dispatch!", "arg 1", "Op");
    unique("show-point", "arg 1", "Point");
    // Calls of the `d!` parameter inside the body; passing `dispatch!`, which
    // itself accepts any enum, neither names nor rules out a type.
    unique("on-click", "arg 1 → arg 1", "Op");
    // A `let` that rebinds `d!` to another function of the same type is not the parameter.
    unique("on-shadowed", "arg 1 → arg 1", "Op");
    // A Respo-style `defcomp` keeps the `defn` parameter list.
    unique("comp-button", "arg 1 → arg 1", "Op");
    let mixed = hint("mixed!").expect("mixed! lists its types");
    assert!(
      mixed.contains("`broad-type-candidates.main/Msg`, `broad-type-candidates.main/Op`") && mixed.contains(":generics"),
      "{mixed}"
    );
    assert_eq!(hint("unused!"), None, "{notes:?}");
    // One call passes `Op`, another an anonymous `::` value: no candidate.
    assert_eq!(hint("partly-anonymous!"), None, "{notes:?}");
    // A spread the check cannot expand fills the slot with unknown values.
    assert_eq!(hint("spread-op!"), None, "{notes:?}");
  });
}

/// Effect boundaries (#2710): undeclared `:state` / `:io` follow the entry
/// policy, propagate through calls and references, and leave local Refs,
/// assertion failure output, featured lambdas and `dbg` alone.
#[test]
fn effect_features_follow_entry_policy() {
  run_with_large_stack(|| {
    let path = "calcit/type-fail/effect-features.cirru";
    let ns = "type-fail-effect-features.main";
    let expected = [
      ("read-store", "STATE_FEATURE"),
      ("calls-log", "IO_FEATURE"),
      ("passes-log", "IO_FEATURE"),
      ("call-cb", "IO_FEATURE"),
      ("reads-env", "IO_FEATURE"),
      ("uses-dbg", "DBG_IN_ENTRY"),
      ("calls-inline-io", "IO_FEATURE"),
      ("shadow-ref", "STATE_FEATURE"),
      ("passes-inline-io", "IO_FEATURE"),
      ("passes-local-io", "IO_FEATURE"),
      ("passes-sort-by-io", "IO_FEATURE"),
      ("passes-user-pass", "IO_FEATURE"),
    ];
    let collect_with = |policies: HashMap<String, snapshot::FeaturePolicy>| {
      let entries = load_fixture_entries(path);
      program::configure_entry_feature_policy(&policies);
      let _ = runner::preprocess::take_advisories();
      let warnings = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      let advisories = runner::preprocess::take_advisories();
      program::configure_entry_feature_policy(&HashMap::new());
      result.expect("effect diagnostics are advisories, not compile errors");
      let mut found = advisories
        .iter()
        .filter_map(|advisory| {
          let code = advisory.code()?;
          let kind = code.strip_prefix("W_").or_else(|| code.strip_prefix("E_"))?;
          let kind = kind.strip_suffix("_REQUIRED").unwrap_or(kind);
          matches!(kind, "STATE_FEATURE" | "IO_FEATURE" | "DBG_IN_ENTRY").then(|| {
            let def = advisory.message().split_whitespace().nth(1).unwrap_or_default().to_owned();
            (def, code.to_owned(), runner::preprocess::is_blocking_advisory(advisory))
          })
        })
        .collect::<Vec<_>>();
      found.sort();
      found
    };
    let collect = |policy: snapshot::FeaturePolicy| {
      collect_with(HashMap::from([
        ("state".to_owned(), policy),
        ("io".to_owned(), policy),
        ("dbg".to_owned(), policy),
      ]))
    };

    for (policy, prefix, blocking) in [
      (snapshot::FeaturePolicy::Warn, "W_", false),
      (snapshot::FeaturePolicy::Error, "E_", true),
    ] {
      let found = collect(policy);
      let mut wanted = expected
        .iter()
        .map(|(def, kind)| {
          let suffix = if *kind == "DBG_IN_ENTRY" { "" } else { "_REQUIRED" };
          (format!("{ns}/{def}"), format!("{prefix}{kind}{suffix}"), blocking)
        })
        .collect::<Vec<_>>();
      wanted.sort();
      assert_eq!(found, wanted, "effect diagnostics under {policy:?}");
    }

    assert!(collect(snapshot::FeaturePolicy::Allow).is_empty(), ":allow reports nothing");

    // Without `:feature-policy`, `:state` / `:io` fail the check (#2726) and
    // `dbg` stays allowed.
    let mut wanted = expected
      .iter()
      .filter(|(_, kind)| *kind != "DBG_IN_ENTRY")
      .map(|(def, kind)| (format!("{ns}/{def}"), format!("E_{kind}_REQUIRED"), true))
      .collect::<Vec<_>>();
    wanted.sort();
    assert_eq!(collect_with(HashMap::new()), wanted, "effect diagnostics without an entry policy");
  });
}

#[test]
fn effect_features_track_aliases_spread_and_trait_methods() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _strict = StrictTypesReset::enabled();
    builtins::effects::init_effects_states();
    let snippet = r#"defref *counter 0

deftrait Store
  .save! $ Fn Self (-> Unit) (:features :io)

deftrait InlineStore
  .persist! $ Fn Self (-> Unit) (:features :io)

deftrait Foldable
  .each-item $ Fn Self (Fn Number (-> Unit) (:features :pass)) (-> Unit)

deftrait CustomDeref
  .deref $ Fn Self (-> Number)

deftype IoCallback (Fn Number (-> Unit) (:features :io))

deftype PureCallback (Fn Number (-> Unit))

defstruct Disk (:id Number) (:impls Store InlineStore Foldable)

defstruct Box (:v Number) (:impls CustomDeref)

defenum Choice
  :pure $ Fn Number (-> Unit)

defn save-disk (d)
  println d

defimpl Disk Store
  .save! save-disk

defimpl Disk InlineStore
  .persist! $ fn (d) (println d)

defimpl Disk Foldable
  .each-item $ fn (d cb)
    hint-fn $ Fn Disk (Fn Number (-> Unit) (:features :pass)) (-> Unit)
    cb 1

defimpl Box CustomDeref
  .deref $ fn (b) (:v b)

defn log-line (x) (println x)

defn pure-now (x) &unit

defn alias-hof (xs)
  let ((run each))
    run xs $ fn (y)
      hint-fn $ Fn Number (-> Unit) (:features :io)
      println y

defn sort-shadow-alias (xs)
  let
      run &list:sort-by
      run run
    run xs $ fn (y)
      hint-fn $ Fn Number (-> Number) (:features :io)
      println y
      , y

defn alias-param-call (cb)
  cb 1

defn alias-param-pass (xs cb)
  each xs cb

defn alias-pure-pass (xs cb)
  each xs cb

defn make-logger ()
  fn (y)
    hint-fn $ Fn Number (-> Unit) (:features :io)
    println y

defn make-pure-cb ()
  fn (y) &unit

defn call-ret-pass-direct (xs)
  each xs $ make-logger

defn call-ret-pass-if (flag xs)
  each xs $ if flag (make-logger) pure-now

defn call-ret-pass-pure (xs)
  each xs $ make-pure-cb

defn param-pass-hof (xs runner)
  runner xs $ fn (y)
    hint-fn $ Fn Number (-> Unit) (:features :io)
    println y

defn inline-pass-lambda ()
  (fn (cb) (hint-fn $ Fn (Fn Number (-> Unit) (:features :pass)) (-> Unit)) (cb 1))
    fn (y)
      hint-fn $ Fn Number (-> Unit) (:features :io)
      println y

defn compound-if-pass-cb (flag xs)
  each xs $ if flag
    fn (y)
      hint-fn $ Fn Number (-> Unit) (:features :io)
      println y
    fn (y) &unit

defn compound-let-pass-cb (xs)
  each xs $ let
      cb $ fn (y)
        hint-fn $ Fn Number (-> Unit) (:features :io)
        println y
    , cb

defn spread-local-call (cb xs)
  cb & xs

defn trait-where-io (s)
  .save! s

defn impl-inline-io (d)
  &trait-call InlineStore :persist! d

defn impl-method-inline-io (d)
  .persist! d

defn impl-method-inline-pass (d)
  .each-item d $ fn (n)
    hint-fn $ Fn Number (-> Unit) (:features :io)
    println n

defn explicit-trait-call-io (d)
  &trait-call Store :save! d

defn trait-where-pass (c)
  .each-item c $ fn (n)
    hint-fn $ Fn Number (-> Unit) (:features :io)
    println n

defn deref-global-method ()
  .deref *counter

defn deref-local-method ()
  let ((r (ref 1)))
    .deref r

defn deref-custom-method (b)
  .deref b

defn match-shadow-pure (cb c)
  match c
    (:pure cb) (cb 1)

defn main! ()
  alias-hof $ [] 1
  sort-shadow-alias $ [] 1
  alias-param-call log-line
  alias-param-pass ([] 1) log-line
  alias-pure-pass ([] 1) pure-now
  call-ret-pass-direct $ [] 1
  call-ret-pass-if true $ [] 1
  call-ret-pass-pure $ [] 1
  param-pass-hof ([] 1) each
  inline-pass-lambda
  compound-if-pass-cb true $ [] 1
  compound-let-pass-cb $ [] 1
  spread-local-call log-line $ [] 1
  trait-where-io $ Disk :id 1
  impl-inline-io $ Disk :id 2
  impl-method-inline-io $ Disk :id 4
  impl-method-inline-pass $ Disk :id 5
  explicit-trait-call-io $ Disk :id 3
  deref-global-method
  deref-local-method
  deref-custom-method $ Box :v 1
  match-shadow-pure log-line $ Choice :pure pure-now
  , &unit
"#;
    let mut snapshot = snapshot::Snapshot::default();
    let mut file = snapshot::create_file_from_snippet(snippet).expect("effect tracking fixture should parse");
    let schemas = [
      ("save-disk", "Fn Disk (-> Unit) (:features :io)"),
      ("log-line", "Fn Number (-> Unit) (:features :io)"),
      ("pure-now", "Fn Number (-> Unit)"),
      ("alias-hof", "Fn (List Number) (-> Unit)"),
      ("sort-shadow-alias", "Fn (List Number) (-> (List Number))"),
      ("alias-param-call", "Fn IoCallback (-> Unit)"),
      ("alias-param-pass", "Fn (List Number) IoCallback (-> Unit)"),
      ("alias-pure-pass", "Fn (List Number) PureCallback (-> Unit)"),
      ("make-logger", "Fn (-> (Fn Number (-> Unit) (:features :io)))"),
      ("make-pure-cb", "Fn (-> (Fn Number (-> Unit)))"),
      ("call-ret-pass-direct", "Fn (List Number) (-> Unit)"),
      ("call-ret-pass-if", "Fn Bool (List Number) (-> Unit)"),
      ("call-ret-pass-pure", "Fn (List Number) (-> Unit)"),
      (
        "param-pass-hof",
        "Fn (List Number) (Fn (List Number) (Fn Number (-> Unit) (:features :pass)) (-> Unit)) (-> Unit)",
      ),
      ("inline-pass-lambda", "Fn (-> Unit)"),
      ("compound-if-pass-cb", "Fn Bool (List Number) (-> Unit)"),
      ("compound-let-pass-cb", "Fn (List Number) (-> Unit)"),
      (
        "spread-local-call",
        "Fn (Fn (Variadic Number) (-> Unit) (:features :io)) (List Number) (-> Unit)",
      ),
      ("trait-where-io", "Fn T (-> Unit) (:generics T) (:where (T Store))"),
      ("impl-inline-io", "Fn Disk (-> Unit)"),
      ("impl-method-inline-io", "Fn Disk (-> Unit)"),
      ("impl-method-inline-pass", "Fn Disk (-> Unit)"),
      ("explicit-trait-call-io", "Fn Disk (-> Unit)"),
      ("trait-where-pass", "Fn T (-> Unit) (:generics T) (:where (T Foldable))"),
      ("deref-global-method", "Fn (-> Number)"),
      ("deref-local-method", "Fn (-> Number)"),
      ("deref-custom-method", "Fn Box (-> Number)"),
      ("match-shadow-pure", "Fn (Fn Number (-> Unit) (:features :io)) Choice (-> Unit)"),
      ("main!", "Fn (-> Unit) (:features :io :state)"),
    ];
    for (name, schema_src) in schemas {
      let schema = cirru_parser::parse(schema_src)
        .unwrap_or_else(|e| panic!("parse schema for {name}: {e}"))
        .remove(0);
      file.defs.get_mut(name).unwrap_or_else(|| panic!("missing def {name}")).schema =
        snapshot::parse_schema_annotation_for_write(&schema).unwrap_or_else(|e| panic!("parse annotation for {name}: {e}"));
    }
    snapshot.files.insert("app.main".to_owned(), file);
    let entries = prepare_snapshot_entries(snapshot);

    program::configure_entry_feature_policy(&HashMap::from([
      ("state".to_owned(), snapshot::FeaturePolicy::Error),
      ("io".to_owned(), snapshot::FeaturePolicy::Error),
      ("dbg".to_owned(), snapshot::FeaturePolicy::Error),
    ]));
    let _ = runner::preprocess::take_advisories();
    let warnings = RefCell::new(vec![]);
    let targets = [
      "main!",
      "alias-hof",
      "sort-shadow-alias",
      "alias-param-call",
      "alias-param-pass",
      "alias-pure-pass",
      "call-ret-pass-direct",
      "call-ret-pass-if",
      "call-ret-pass-pure",
      "param-pass-hof",
      "inline-pass-lambda",
      "compound-if-pass-cb",
      "compound-let-pass-cb",
      "spread-local-call",
      "trait-where-io",
      "impl-inline-io",
      "impl-method-inline-io",
      "impl-method-inline-pass",
      "explicit-trait-call-io",
      "trait-where-pass",
      "deref-global-method",
      "deref-local-method",
      "deref-custom-method",
      "match-shadow-pure",
    ];
    for target in targets {
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, target, &warnings, &CallStackList::default())
        .unwrap_or_else(|err| panic!("preprocess failed for {target}: {err}"));
    }
    let advisories = runner::preprocess::take_advisories();
    program::configure_entry_feature_policy(&HashMap::new());

    let mut found = advisories
      .iter()
      .filter_map(|advisory| {
        let code = advisory.code()?;
        matches!(code, "E_STATE_FEATURE_REQUIRED" | "E_IO_FEATURE_REQUIRED").then(|| {
          let def = advisory.message().split_whitespace().nth(1).unwrap_or_default().to_owned();
          (def, code.to_owned())
        })
      })
      .collect::<Vec<_>>();
    found.sort();

    let mut wanted = vec![
      ("app.main/alias-hof".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/sort-shadow-alias".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/alias-param-call".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/alias-param-pass".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/call-ret-pass-direct".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/call-ret-pass-if".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/param-pass-hof".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/inline-pass-lambda".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/compound-if-pass-cb".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/compound-let-pass-cb".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/spread-local-call".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/trait-where-io".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/impl-inline-io".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/impl-method-inline-io".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/impl-method-inline-pass".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/explicit-trait-call-io".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/trait-where-pass".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/deref-global-method".to_owned(), "E_STATE_FEATURE_REQUIRED".to_owned()),
    ];
    wanted.sort();
    assert_eq!(
      found, wanted,
      "all aliased/spread/trait/method effect edges should match: {warnings:?}"
    );
  });
}

#[test]
fn effect_features_track_computed_callees() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let snippet = r#"defn make-logger ()
  fn (x)
    hint-fn $ Fn Number (-> Unit) (:features :io)
    println x
defn make-logger-factory ()
  , make-logger
defn make-hof ()
  fn (cb) (cb 1)
defn computed-call-io ()
  (make-logger) 1
defn computed-nested-io ()
  ((make-logger-factory)) 1
defn computed-spread-io ()
  let ((args ([] 1)))
    (make-logger) & args
defn computed-hof-io ()
  (make-hof) (fn (x) (hint-fn $ Fn Number (-> Unit) (:features :io)) (println x))
defn computed-hof-pure ()
  (make-hof) (fn (x) (do x &unit))
defn main! ()
  computed-call-io
  computed-nested-io
  computed-spread-io
  computed-hof-io
  computed-hof-pure
  , &unit"#;

    builtins::effects::init_effects_states();
    let mut snapshot = snapshot::Snapshot::default();
    let mut file = snapshot::create_file_from_snippet(snippet).expect("test snippet should parse");
    for (name, schema_src) in [
      ("make-logger", "Fn (-> (Fn Number (-> Unit) (:features :io)))"),
      ("make-logger-factory", "Fn (-> (Fn (-> (Fn Number (-> Unit) (:features :io)))))"),
      ("make-hof", "Fn (-> (Fn (Fn Number (-> Unit) (:features :pass)) (-> Unit)))"),
      ("computed-call-io", "Fn (-> Unit)"),
      ("computed-nested-io", "Fn (-> Unit)"),
      ("computed-spread-io", "Fn (-> Unit)"),
      ("computed-hof-io", "Fn (-> Unit)"),
      ("computed-hof-pure", "Fn (-> Unit)"),
      ("main!", "Fn (-> Unit) (:features :io)"),
    ] {
      let schema = cirru_parser::parse(schema_src)
        .unwrap_or_else(|e| panic!("parse schema for {name}: {e}"))
        .remove(0);
      file.defs.get_mut(name).unwrap_or_else(|| panic!("missing def {name}")).schema =
        snapshot::parse_schema_annotation_for_write(&schema).unwrap_or_else(|e| panic!("parse annotation for {name}: {e}"));
    }
    snapshot.files.insert("app.main".to_owned(), file);
    let entries = prepare_snapshot_entries(snapshot);

    program::configure_entry_feature_policy(&HashMap::from([
      ("state".to_owned(), snapshot::FeaturePolicy::Error),
      ("io".to_owned(), snapshot::FeaturePolicy::Error),
      ("dbg".to_owned(), snapshot::FeaturePolicy::Error),
    ]));
    let _ = runner::preprocess::take_advisories();
    let warnings = RefCell::new(vec![]);
    for target in [
      "computed-call-io",
      "computed-nested-io",
      "computed-spread-io",
      "computed-hof-io",
      "computed-hof-pure",
    ] {
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, target, &warnings, &CallStackList::default())
        .unwrap_or_else(|err| panic!("preprocess failed for {target}: {err}"));
    }
    let advisories = runner::preprocess::take_advisories();
    program::configure_entry_feature_policy(&HashMap::new());

    let mut found = advisories
      .iter()
      .filter_map(|advisory| {
        let code = advisory.code()?;
        matches!(code, "E_STATE_FEATURE_REQUIRED" | "E_IO_FEATURE_REQUIRED").then(|| {
          let def = advisory.message().split_whitespace().nth(1).unwrap_or_default().to_owned();
          (def, code.to_owned())
        })
      })
      .collect::<Vec<_>>();
    found.sort();

    let mut wanted = vec![
      ("app.main/computed-call-io".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/computed-nested-io".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/computed-spread-io".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
      ("app.main/computed-hof-io".to_owned(), "E_IO_FEATURE_REQUIRED".to_owned()),
    ];
    wanted.sort();
    assert_eq!(
      found, wanted,
      "computed callees should charge returned and :pass callback effects: {warnings:?}"
    );
  });
}

/// A `deftype` alias applied to the wrong number of type arguments fails
/// with `E_TYPE_ALIAS_ARITY`; a value that breaks the aliased shape is an
/// ordinary argument mismatch (#2725).
#[test]
fn strict_type_fail_deftype_alias_checks_arity_and_shape() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries("calcit/type-fail/deftype-arity.cirru");
    let err = run_check_only(&entries).expect_err("deftype arity");
    assert!(err.contains("E_TYPE_ALIAS_ARITY"), "{err}");
    assert!(err.contains("`(Pair A B)`"), "{err}");

    let entries = load_fixture_entries("calcit/type-fail/deftype-shape-mismatch.cirru");
    run_check_only(&entries).expect_err("a two-slot alias rejects a String slot");
  });
}

#[test]
fn strict_type_fail_deftrait_self_receiver_and_self_reference() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries("calcit/type-fail/trait-self-receiver.cirru");
    let err = run_check_only(&entries).expect_err("a Self trait method must take Self first");
    assert!(err.contains("E_TRAIT_SELF_RECEIVER"), "{err}");
    assert!(err.contains("`.make`"), "{err}");

    let entries = load_fixture_entries("calcit/type-fail/trait-names-itself.cirru");
    let err = run_check_only(&entries).expect_err("a trait signature cannot name the trait itself");
    assert!(err.contains("E_TRAIT_AS_TYPE"), "{err}");
    assert!(err.contains("write `Self`"), "{err}");
  });
}

#[test]
fn strict_type_fail_defimpl_infers_self_and_rejects_mismatch() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries("calcit/type-fail/defimpl-self-mismatch.cirru");
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("defimpl return mismatch should report a warning diagnostic");
    assert!(
      warnings.borrow().iter().any(|warning| {
        warning.code() == Some("E_FN_RETURN_TYPE_MISMATCH")
          && warning
            .message()
            .contains("declares return type `:number`, but body returns `:string`")
      }),
      "expected E_FN_RETURN_TYPE_MISMATCH for defimpl inline lambda return mismatch, got: {:?}",
      warnings.borrow()
    );
    run_check_only(&entries).expect_err("defimpl inline lambda must infer Self signature and reject return type mismatch");
  });
}

#[test]
fn strict_type_fail_local_method_shares_receiver_payload_type() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    injection::inject_platform_apis();
    for (replacement, should_reject) in [("2", false), ("|wrong-payload", true)] {
      let entries = load_snippet_entries(&format!(
        "deftrait ReplaceOps\n  .replace $ Fn Self Self $ -> Self\n\ndefn main! ()\n  hint-fn $ Fn $ -> Unit\n  let\n      LocalBox $ defstruct LocalBox (:generics T) (:value T)\n      box-impl $ defimpl LocalBox ReplaceOps\n        .replace $ fn (self other) other\n      BoxWithImpl $ impl-traits LocalBox box-impl\n      receiver $ BoxWithImpl :value 1\n      other $ BoxWithImpl :value {replacement}\n    .replace receiver other\n  , &unit"
      ));
      // Compile directly so the ordinary argument checker must reject the bad call.
      let warnings = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      let warnings = warnings.into_inner();
      let mismatch = matches!(&result, Err(err) if err.code.as_deref() == Some("E_METHOD_ARG_TYPE_MISMATCH"))
        || warnings.iter().any(|warning| warning.code() == Some("E_METHOD_ARG_TYPE_MISMATCH"));
      if should_reject {
        assert!(
          mismatch,
          "expected a receiver-bound argument mismatch, got {result:?}, {warnings:?}"
        );
      } else {
        assert!(
          result.is_ok() && warnings.is_empty(),
          "same-payload call should compile, got {result:?}, {warnings:?}"
        );
      }
    }
  });
}

#[test]
fn strict_type_fail_local_method_keeps_callback_generics_independent() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    injection::inject_platform_apis();
    for (callback_schema, should_reject) in [("Fn T (-> T) (:generics T)", false), ("Fn Number (-> Number)", true)] {
      let entries = load_snippet_entries(&format!(
        "deftrait CallbackOps\n  .use-callback $ Fn Self (Fn T (-> T) (:generics T)) (-> Unit)\n\ndefn main! ()\n  hint-fn $ Fn $ -> Unit\n  let\n      LocalBox $ defstruct LocalBox (:generics T) (:value T)\n      box-impl $ defimpl LocalBox CallbackOps\n        .use-callback $ fn (self callback) &unit\n      BoxWithImpl $ impl-traits LocalBox box-impl\n      receiver $ BoxWithImpl :value 1\n      callback $ fn (x)\n        hint-fn $ {callback_schema}\n        , x\n    .use-callback receiver callback"
      ));
      let warnings = RefCell::new(vec![]);
      let result =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      let warnings = warnings.into_inner();
      if should_reject {
        assert!(
          matches!(&result, Err(err) if err.code.as_deref() == Some("E_METHOD_ARG_TYPE_MISMATCH"))
            || warnings.iter().any(|warning| warning.code() == Some("E_METHOD_ARG_TYPE_MISMATCH")),
          "expected the method to reject a monomorphic callback, got {result:?}, {warnings:?}"
        );
      } else {
        assert!(
          result.is_ok() && warnings.is_empty(),
          "polymorphic callback should compile, got {result:?}, {warnings:?}"
        );
      }
    }
  });
}

#[test]
fn strict_type_fail_defimpl_alias_generic_and_local_nominal_contracts() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    injection::inject_platform_apis();

    // 1. #2838: `defimpl` referencing a trait through an `:as` namespace alias must infer Self and reject return mismatch.
    let alias_entries = load_snippet_entries_with_library(
      "lib.gauge",
      "def Gauge $ deftrait Gauge\n  .reading $ Fn Self $ -> Number",
      "ns app.main $ :require $ lib.gauge :as g\n\ndefstruct Meter $ :v Number\n\ndef MeterImpl $ defimpl Meter g/Gauge\n  .reading $ fn (self) |bad-reading\n\ndefn main! () &unit",
    );
    let alias_warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&alias_entries.init_ns, "MeterImpl", &alias_warnings, &CallStackList::default())
      .expect("aliased defimpl return mismatch should report a warning diagnostic");
    assert!(
      alias_warnings.borrow().iter().any(|warning| {
        warning.code() == Some("E_FN_RETURN_TYPE_MISMATCH")
          && warning
            .message()
            .contains("declares return type `:number`, but body returns `:string`")
      }),
      "expected E_FN_RETURN_TYPE_MISMATCH for :as aliased trait in defimpl, got: {:?}",
      alias_warnings.borrow()
    );

    // 2. #2840: Generic `defimpl` with `Self` return must reject an unhinted lambda returning a non-Self value.
    let generic_entries = load_snippet_entries(
      "def BoxOps $ deftrait BoxOps\n  .clone-box $ Fn Self $ -> Self\n\ndefstruct GenericBox (:generics T) (:value T)\n\ndef GenericBoxImpl $ defimpl GenericBox BoxOps\n  .clone-box $ fn (self) true\n\ndefn main! () &unit",
    );
    let generic_warnings = RefCell::new(vec![]);
    let generic_res = runner::preprocess::ensure_ns_def_compiled(
      &generic_entries.init_ns,
      "GenericBoxImpl",
      &generic_warnings,
      &CallStackList::default(),
    );
    assert!(
      matches!(&generic_res, Err(err) if err.code.as_deref() == Some("E_FN_RETURN_TYPE_MISMATCH"))
        || generic_warnings
          .borrow()
          .iter()
          .any(|warning| warning.code() == Some("E_FN_RETURN_TYPE_MISMATCH")),
      "expected generic defimpl Self return mismatch to fail, got res={generic_res:?}, warnings={:?}",
      generic_warnings.borrow()
    );

    // 3. #2839: Local `let` nominal Struct decorated with `impl-traits` must enforce method argument types and return types.
    let local_arg_entries = load_snippet_entries(
      "def StepOps $ deftrait StepOps\n  .step $ Fn Self Number $ -> Number\n\ndefn main! ()\n  hint-fn $ Fn $ -> Number\n  let\n      LocalCounter $ defstruct LocalCounter $ :v Number\n      counter-impl $ defimpl LocalCounter StepOps\n        .step $ fn (self delta) (+ (:v self) delta)\n      CounterWithImpl $ impl-traits LocalCounter counter-impl\n      c $ CounterWithImpl :v 1\n    .step c |wrong-arg",
    );
    let local_arg_warnings = RefCell::new(vec![]);
    let local_arg_res = runner::preprocess::ensure_ns_def_compiled(
      &local_arg_entries.init_ns,
      &local_arg_entries.init_def,
      &local_arg_warnings,
      &CallStackList::default(),
    );
    assert!(
      matches!(&local_arg_res, Err(err) if matches!(err.code.as_deref(), Some("E_METHOD_ARG_TYPE_MISMATCH" | "E_LOCAL_FN_ARG_TYPE_MISMATCH")))
        || local_arg_warnings
          .borrow()
          .iter()
          .any(|warning| matches!(warning.code(), Some("E_METHOD_ARG_TYPE_MISMATCH" | "E_LOCAL_FN_ARG_TYPE_MISMATCH"))),
      "expected local impl-traits method call with wrong argument type to fail, got res={local_arg_res:?}, warnings={:?}",
      local_arg_warnings.borrow()
    );

    // 4. Contextual check_contextual_return: dynamic contextual return still refines to concrete Child, and explicit Number contract is preserved.
    let child_refine_entries = load_snippet_entries(
      "defstruct Child (:value Number)\n\ndefstruct Parent (:v Number)\n\ndef ExplicitOps $ deftrait ExplicitOps\n  .val $ Fn Self $ -> Number\n\ndef ParentExplicitImpl $ defimpl Parent ExplicitOps\n  .val $ fn (self) (:v self)\n\ndefn run-open (cb)\n  hint-fn $ Fn (Fn (-> Dynamic)) (-> Unit)\n  cb\n  , &unit\n\ndefn read-open ()\n  hint-fn $ Fn $ -> Unit\n  run-open $ fn () (Child :value 1)\n\ndefn read-explicit ()\n  hint-fn $ Fn $ -> Number\n  let\n      p $ impl-traits (Parent :v 2) ParentExplicitImpl\n    + 1 (.val p)\n\ndefn main! () &unit",
    );
    for target in ["read-open", "read-explicit"] {
      let warnings = RefCell::new(vec![]);
      let res = runner::preprocess::ensure_ns_def_compiled(&child_refine_entries.init_ns, target, &warnings, &CallStackList::default());
      assert!(res.is_ok(), "expected {target} to compile cleanly, got {res:?}");
      let app_warnings: Vec<_> = warnings
        .borrow()
        .iter()
        .filter(|w| !w.message().contains("eprintln"))
        .cloned()
        .collect();
      assert!(
        app_warnings.is_empty(),
        "expected {target} to have no warnings, got {app_warnings:?}"
      );
    }

    // 5. Method generic `T` colliding with implementing type's `T` must stay distinct.
    let collision_entries = load_snippet_entries(
      "def SwapOps $ deftrait SwapOps\n  .take $ Fn Self T (-> T) (:generics T)\n\ndefstruct GenericHolder (:generics T) (:value T)\n\ndef GenericHolderImpl $ defimpl GenericHolder SwapOps\n  .take $ fn (self other) (:value self)\n\ndefn main! ()\n  hint-fn $ Fn $ -> Number\n  let\n      h $ impl-traits (GenericHolder :value 1) GenericHolderImpl\n    .take h 2",
    );
    let collision_err = run_check_only(&collision_entries)
      .expect_err("expected colliding method generic T vs impl generic T to reject returning (:value self)");
    assert!(
      collision_err.contains("E_ERASED_GENERIC_RELATION") || collision_err.contains("E_FN_RETURN_TYPE_MISMATCH"),
      "unexpected error for colliding method generic T vs impl generic T: {collision_err}"
    );
  });
}

struct ProofAuditReset {
  previous: bool,
}

impl ProofAuditReset {
  fn disabled() -> Self {
    let previous = runner::preprocess::default_proof_enabled();
    runner::preprocess::set_default_proof_audit(false);
    Self { previous }
  }
}

impl Drop for ProofAuditReset {
  fn drop(&mut self) {
    runner::preprocess::set_default_proof_audit(self.previous);
  }
}

#[test]
fn strict_bool_conditions_reject_non_bool_statically_and_at_runtime() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();

    // 1. Static rejections across Dynamic, Nil, Unit, and Nullable(Number) for all control forms.
    let nullable_lib = |body: &str| -> ProgramEntries {
      let fn_schema = |arg_types: Vec<Arc<CalcitTypeAnnotation>>, return_type: CalcitTypeAnnotation| {
        Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
          open_reason: Default::default(),
          cases: Default::default(),
          guard: None,
          runtime_arity: None,
          generics: Arc::new(vec![]),
          where_bounds: Arc::new(vec![]),
          arg_types,
          return_type: Arc::new(return_type),
          fn_kind: SchemaKind::Fn,
          rest_type: None,
          features: Arc::new(HashSet::new()),
        })))
      };
      let mut library = snapshot::create_file_from_snippet("ns lib.nullable\n\ndefn make-nullable () 7")
        .expect("nullable library snippet should parse");
      library.ns.code = vec!["ns", "lib.nullable"].into();
      library.defs.get_mut("make-nullable").expect("make-nullable").schema = fn_schema(
        vec![],
        CalcitTypeAnnotation::Nullable(Arc::new(CalcitTypeAnnotation::Number), calcit::calcit::NullableOrigin::Inferred),
      );
      let app_snippet = format!(
        "ns app.main\n  :require\n    lib.nullable :refer $ make-nullable\n\ndefn main! ()\n  let ((maybe (make-nullable)))\n    {body}"
      );
      let file = snapshot::create_file_from_snippet(&app_snippet).expect("nullable caller snippet should parse");
      let mut snapshot = snapshot::Snapshot::default();
      snapshot.files.insert("app.main".to_owned(), file);
      prepare_snapshot_entries_with_libraries(snapshot, vec![("lib.nullable".to_owned(), library)])
    };

    let static_cases = [
      (
        "if Dynamic",
        "defn probe (x)\n  hint-fn $ Fn Dynamic (-> Number)\n  if x 1 2\n\ndefn main! () (probe nil)",
      ),
      (
        "when Dynamic",
        "defn probe (x)\n  hint-fn $ Fn Dynamic (-> Unit)\n  when x 1\n\ndefn main! () (probe nil)",
      ),
      (
        "when-not Dynamic",
        "defn probe (x)\n  hint-fn $ Fn Dynamic (-> Unit)\n  when-not x 1\n\ndefn main! () (probe nil)",
      ),
      (
        "if-not Dynamic",
        "defn probe (x)\n  hint-fn $ Fn Dynamic (-> Number)\n  if-not x 1 2\n\ndefn main! () (probe nil)",
      ),
      (
        "and Dynamic",
        "defn probe (x)\n  hint-fn $ Fn Dynamic (-> Bool)\n  and x true\n\ndefn main! () (probe nil)",
      ),
      (
        "or Dynamic",
        "defn probe (x)\n  hint-fn $ Fn Dynamic (-> Bool)\n  or false x\n\ndefn main! () (probe nil)",
      ),
      ("if Nil", "if nil 1 2"),
      ("when Nil", "when nil 1"),
      ("when-not Nil", "when-not nil 1"),
      ("if-not Nil", "if-not nil 1 2"),
      ("and Nil", "and nil true"),
      ("or Nil", "or false nil"),
      ("if Unit", "if &unit 1 2"),
      ("when Unit", "when &unit 1"),
      ("when-not Unit", "when-not &unit 1"),
      ("if-not Unit", "if-not &unit 1 2"),
      ("and Unit", "and &unit true"),
      ("or Unit", "or false &unit"),
    ];
    for (label, snippet) in static_cases {
      let entries = load_snippet_entries(snippet);
      let warnings = RefCell::new(vec![]);
      let target = if snippet.contains("defn probe") {
        "probe"
      } else {
        &entries.init_def
      };
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, target, &warnings, &CallStackList::default())
        .unwrap_or_else(|err| panic!("{label} should diagnose in check_warnings rather than hard-fail: {err}"));
      assert!(
        warnings.borrow().iter().any(|w| w.code() == Some("E_NON_BOOL_CONDITION")),
        "{label} should report E_NON_BOOL_CONDITION, got {:?}",
        warnings.borrow()
      );
    }
    for (label, body) in [
      ("if Nullable", "(if maybe 1 2)"),
      ("when Nullable", "(when maybe 1)"),
      ("when-not Nullable", "(when-not maybe 1)"),
      ("if-not Nullable", "(if-not maybe 1 2)"),
      ("and Nullable", "(and maybe true)"),
      ("or Nullable", "(or false maybe)"),
    ] {
      let entries = nullable_lib(body);
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .unwrap_or_else(|err| panic!("{label} should diagnose in check_warnings rather than hard-fail: {err}"));
      assert!(
        warnings.borrow().iter().any(|w| w.code() == Some("E_NON_BOOL_CONDITION")),
        "{label} should report E_NON_BOOL_CONDITION, got {:?}",
        warnings.borrow()
      );
    }

    // 2. `not` rejects non-Bool inputs statically via callable contract diagnostics.
    for (label, snippet, expected_code) in [
      ("not Nil", "not nil", "E_PROC_ARG_TYPE_MISMATCH"),
      ("not Unit", "not &unit", "E_PROC_ARG_TYPE_MISMATCH"),
      (
        "not Dynamic",
        "defn probe (x)\n  hint-fn $ Fn Dynamic (-> Bool)\n  not x\n\ndefn main! () (probe nil)",
        "E_CALL_ARGUMENT_UNPROVEN",
      ),
    ] {
      let entries = load_snippet_entries(snippet);
      let warnings = RefCell::new(vec![]);
      let target = if snippet.contains("defn probe") {
        "probe"
      } else {
        &entries.init_def
      };
      let compiled = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, target, &warnings, &CallStackList::default());
      let matched_err = matches!(&compiled, Err(err) if err.code.as_deref() == Some(expected_code));
      let matched_warning = warnings.borrow().iter().any(|w| w.code() == Some(expected_code));
      assert!(
        matched_err || matched_warning,
        "{label} expected {expected_code}, got res={compiled:?}, warnings={:?}",
        warnings.borrow()
      );
    }
    {
      let entries = nullable_lib("(not maybe)");
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .unwrap_or_else(|err| panic!("not Nullable should preprocess with warning: {err}"));
      assert!(
        warnings.borrow().iter().any(|w| w.code() == Some("E_PROC_ARG_TYPE_MISMATCH")),
        "not Nullable expected E_PROC_ARG_TYPE_MISMATCH, got {:?}",
        warnings.borrow()
      );
    }

    // 3. Native runtime rejects coerced non-Bool values on `if` and `not`.
    for (label, expr, expected_msg) in [
      ("runtime if nil", "if (as-bool nil) |yes |no", "if condition expects a Bool"),
      ("runtime if 0", "if (as-bool 0) |yes |no", "if condition expects a Bool"),
      ("runtime if unit", "if (as-bool &unit) |yes |no", "if condition expects a Bool"),
      ("runtime not nil", "not (as-bool nil)", "not requires a Bool argument"),
      ("runtime not 0", "not (as-bool 0)", "not requires a Bool argument"),
      ("runtime not unit", "not (as-bool &unit)", "not requires a Bool argument"),
    ] {
      let snippet = format!(
        "defn main! ()\n  let\n      as-bool $ fn (v)\n        hint-fn $ Fn Dynamic (-> Bool) (:features :js-ffi)\n        unsafe-coerce v Bool\n    {expr}"
      );
      let entries = load_snippet_entries(&snippet);
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .unwrap_or_else(|err| panic!("{label} should preprocess cleanly: {err}"));
      assert!(warnings.borrow().is_empty(), "{label} unexpected warnings: {:?}", warnings.borrow());
      let runtime_err =
        calcit::run_program(entries.init_ns, entries.init_def, &[]).expect_err(&format!("{label} must fail at runtime"));
      assert!(
        runtime_err.msg.contains(expected_msg),
        "{label} expected runtime error containing `{expected_msg}`, got `{}`",
        runtime_err.msg
      );
    }
  });
}

#[test]
fn strict_typed_get_in_string_non_number_key_keeps_runtime_error() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let snippet = "defn main! ()\n  do\n    &get-in |abc ([] :missing)\n    , &unit";
    let entries = load_snippet_entries(snippet);
    let warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .unwrap_or_else(|err| panic!("&get-in string tag key should preprocess cleanly: {err}"));
    assert!(warnings.borrow().is_empty(), "unexpected warnings: {:?}", warnings.borrow());
    let runtime_err =
      calcit::run_program(entries.init_ns, entries.init_def, &[]).expect_err("&get-in |abc ([] :missing) must fail at runtime");
    assert!(
      runtime_err.msg.contains("&get-raw expected a Number index for string"),
      "expected runtime error containing `&get-raw expected a Number index for string`, got `{}`",
      runtime_err.msg
    );
  });
}

#[test]
fn wasm_boundary_defs_reject_dynamic_schema_and_allow_zero_arg_unannotated_export() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let fn_schema = |arg_types: Vec<Arc<CalcitTypeAnnotation>>, return_type: CalcitTypeAnnotation| {
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        open_reason: Default::default(),
        cases: Default::default(),
        guard: None,
        runtime_arity: None,
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types,
        return_type: Arc::new(return_type),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      })))
    };

    let mut file = snapshot::create_file_from_snippet(concat!(
      "deftype OpenItems (List Dynamic)\n\n",
      "deftype ConcreteItems (List Number)\n\n",
      "defwasm-export export-hint-param (x)\n  hint-fn $ Fn Dynamic (-> Number)\n  1\n\n",
      "defwasm-export export-hint-ret (x)\n  hint-fn $ Fn Number (-> Dynamic)\n  x\n\n",
      "defwasm-export export-schema-param (x) 1\n\n",
      "defwasm-import import-schema-param (x) |host |read\n\n",
      "defwasm-import import-schema-ret (x) |host |get\n\n",
      "defwasm-export export-partial-hint (x)\n  hint-fn $ {} (:args ([] Number))\n  , x\n\n",
      "defwasm-export export-partial-hint-dyn-arg (x)\n  hint-fn $ {} (:args ([] Dynamic))\n  , 1\n\n",
      "defwasm-export export-alias-dyn (xs) 1\n\n",
      "defwasm-import import-alias-dyn (xs) |host |read\n\n",
      "defwasm-export export-alias-ok (xs) 1\n\n",
      "defwasm-export zero-typed ()\n  hint-fn $ Fn (-> Number)\n  , 42\n\n",
      "defwasm-export zero-unhinted-project () 42\n\n",
      "defn main! () &unit",
    ))
    .expect("wasm boundary snippet should parse");
    file.defs.get_mut("export-schema-param").expect("export-schema-param").schema =
      fn_schema(vec![calcit::calcit::DYNAMIC_TYPE.clone()], CalcitTypeAnnotation::Number);
    file.defs.get_mut("import-schema-param").expect("import-schema-param").schema =
      fn_schema(vec![calcit::calcit::DYNAMIC_TYPE.clone()], CalcitTypeAnnotation::Number);
    file.defs.get_mut("import-schema-ret").expect("import-schema-ret").schema =
      fn_schema(vec![Arc::new(CalcitTypeAnnotation::Number)], CalcitTypeAnnotation::Dynamic);
    file.defs.get_mut("export-partial-hint").expect("export-partial-hint").schema =
      fn_schema(vec![Arc::new(CalcitTypeAnnotation::Number)], CalcitTypeAnnotation::Number);
    file
      .defs
      .get_mut("export-partial-hint-dyn-arg")
      .expect("export-partial-hint-dyn-arg")
      .schema = fn_schema(vec![Arc::new(CalcitTypeAnnotation::Number)], CalcitTypeAnnotation::Number);
    file.defs.get_mut("export-alias-dyn").expect("export-alias-dyn").schema = fn_schema(
      vec![Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("OpenItems"), Arc::new(vec![])))],
      CalcitTypeAnnotation::Number,
    );
    file.defs.get_mut("import-alias-dyn").expect("import-alias-dyn").schema = fn_schema(
      vec![Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("OpenItems"), Arc::new(vec![])))],
      CalcitTypeAnnotation::Number,
    );
    file.defs.get_mut("export-alias-ok").expect("export-alias-ok").schema = fn_schema(
      vec![Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("ConcreteItems"),
        Arc::new(vec![]),
      ))],
      CalcitTypeAnnotation::Number,
    );

    let mut lib_wasm =
      snapshot::create_file_from_snippet("ns lib.wasm\n\ndefwasm-export zero-unhinted () 42").expect("lib.wasm snippet should parse");
    lib_wasm.ns.code = vec!["ns", "lib.wasm"].into();

    let mut snapshot = snapshot::Snapshot::default();
    snapshot.files.insert("app.main".to_owned(), file);
    let entries = prepare_snapshot_entries_with_libraries(snapshot, vec![("lib.wasm".to_owned(), lib_wasm)]);

    for bad_def in [
      "export-hint-param",
      "export-hint-ret",
      "export-schema-param",
      "import-schema-param",
      "import-schema-ret",
      "export-partial-hint-dyn-arg",
      "export-alias-dyn",
      "import-alias-dyn",
    ] {
      let warnings = RefCell::new(vec![]);
      let err = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, bad_def, &warnings, &CallStackList::default())
        .expect_err(&format!("{bad_def} must be rejected by E_WASM_BOUNDARY_DYNAMIC"));
      assert_eq!(
        err.code.as_deref(),
        Some("E_WASM_BOUNDARY_DYNAMIC"),
        "expected E_WASM_BOUNDARY_DYNAMIC for {bad_def}, got {err}"
      );
    }

    let project_zero_warnings = RefCell::new(vec![]);
    let project_zero_err = runner::preprocess::ensure_ns_def_compiled(
      &entries.init_ns,
      "zero-unhinted-project",
      &project_zero_warnings,
      &CallStackList::default(),
    )
    .expect_err("schema-less zero-arg defwasm-export in project ns must still require a schema");
    assert_eq!(
      project_zero_err.code.as_deref(),
      Some("E_WHOLE_DYNAMIC_PUBLIC_SCHEMA"),
      "expected E_WHOLE_DYNAMIC_PUBLIC_SCHEMA for zero-unhinted-project, got {project_zero_err}"
    );

    for ok_def in ["zero-typed", "export-partial-hint", "export-alias-ok"] {
      let ok_warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, ok_def, &ok_warnings, &CallStackList::default())
        .unwrap_or_else(|err| panic!("{ok_def} must be allowed: {err}"));
      assert!(
        ok_warnings.borrow().is_empty(),
        "{ok_def} unexpected warnings: {:?}",
        ok_warnings.borrow()
      );
    }

    let lib_warnings = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled("lib.wasm", "zero-unhinted", &lib_warnings, &CallStackList::default())
      .expect("schema-less zero-arg defwasm-export must remain allowed");
    assert!(
      lib_warnings.borrow().iter().all(|w| w.code() != Some("E_WASM_BOUNDARY_DYNAMIC")),
      "unexpected E_WASM_BOUNDARY_DYNAMIC warning: {:?}",
      lib_warnings.borrow()
    );
  });
}

#[test]
fn stage1_open_dynamic_return_requires_proof_under_no_proof_audit() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let _no_audit = ProofAuditReset::disabled();

    let fn_schema =
      |arg_types: Vec<Arc<CalcitTypeAnnotation>>, return_type: CalcitTypeAnnotation, open_reason: calcit::calcit::FnOpenReasonNote| {
        Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
          open_reason,
          cases: Default::default(),
          guard: None,
          runtime_arity: None,
          generics: Arc::new(vec![]),
          where_bounds: Arc::new(vec![]),
          arg_types,
          return_type: Arc::new(return_type),
          fn_kind: SchemaKind::Fn,
          rest_type: None,
          features: Arc::new(HashSet::new()),
        })))
      };

    let build_entries = |main_body: &str| {
      builtins::effects::init_effects_states();
      let mut library = snapshot::create_file_from_snippet(concat!(
        "ns lib.dynamic\n",
        "defn make-open () 1\n\n",
        "defn make-open-arg (x) x\n\n",
        "defn make-intrinsic () 1",
      ))
      .expect("library snippet should parse");
      library.ns.code = vec!["ns", "lib.dynamic"].into();
      let mut fn_head_entry = library.defs.get("make-open").expect("make-open").clone();
      fn_head_entry.code = cirru_parser::parse("fn ()\n  hint-fn $ Fn (-> Dynamic)\n  , 1")
        .expect("fn-headed helper should parse")
        .into_iter()
        .next()
        .expect("single fn expression");
      library.defs.insert("make-open-fn-head".to_owned(), fn_head_entry);
      library.defs.get_mut("make-open").expect("make-open").schema =
        fn_schema(vec![], CalcitTypeAnnotation::Dynamic, Default::default());
      library.defs.get_mut("make-open-arg").expect("make-open-arg").schema = fn_schema(
        vec![Arc::new(CalcitTypeAnnotation::Number)],
        CalcitTypeAnnotation::Dynamic,
        Default::default(),
      );
      library.defs.get_mut("make-intrinsic").expect("make-intrinsic").schema = fn_schema(
        vec![],
        CalcitTypeAnnotation::Dynamic,
        calcit::calcit::FnOpenReasonNote(Some(calcit::calcit::FnOpenReason::Intrinsic)),
      );

      let app_snippet = format!(
        "ns app.main\n  :require\n    lib.dynamic :refer $ make-open make-open-arg make-open-fn-head make-intrinsic\n\ndefn take-num (x) x\n\ndefn main! ()\n  {main_body}\n\ndefn reload! () &unit"
      );
      let mut app = snapshot::create_file_from_snippet(&app_snippet).expect("app snippet should parse");
      app.defs.get_mut("take-num").expect("take-num").schema = fn_schema(
        vec![Arc::new(CalcitTypeAnnotation::Number)],
        CalcitTypeAnnotation::Number,
        Default::default(),
      );

      let mut snapshot = snapshot::Snapshot::default();
      snapshot.files.insert("app.main".to_owned(), app);
      prepare_snapshot_entries_with_libraries(snapshot, vec![("lib.dynamic".to_owned(), library)])
    };

    // Unproven explicit-open calls fail even when CALCIT_PROOF_AUDIT=0 (no default proof audit).
    for (label, body) in [
      ("direct defn arg", "take-num (make-open)"),
      ("let-bound defn arg", "let ((v (make-open))) (take-num v)"),
      (
        "shadowing let-bound call",
        "let ((fetch (fn () (hint-fn $ Fn (-> Dynamic)) (make-open)))) (let ((fetch (fetch))) (&+ fetch 1))",
      ),
      ("direct spread call", "&+ (make-open-arg & ([] 1)) 1"),
      ("let-bound spread call", "let ((v (make-open-arg & ([] 1)))) (&+ v 1)"),
      ("if branch open call", "&+ (if true (make-open) (make-open)) 1"),
      ("if mixed branch open call", "&+ (if false 1 (make-open)) 1"),
      ("do tail open call", "&+ (do 1 (make-open)) 1"),
      ("let tail open call", "&+ (let ((x 1)) (make-open)) 1"),
      ("let bound open tail", "&+ (let ((v (make-open))) v) 1"),
      ("fn-headed hinted open call", "&+ (make-open-fn-head) 1"),
      ("direct proc &+ arg", "&+ (make-open) 1"),
      ("let-bound proc &+ arg", "let ((v (make-open))) (&+ v 1)"),
      ("direct not arg", "not (make-open)"),
      ("let-bound not arg", "let ((v (make-open))) (not v)"),
    ] {
      let entries = build_entries(body);
      let err = run_check_only_with_output(&entries, false)
        .expect_err(&format!("{label} must fail E_CALL_ARGUMENT_UNPROVEN even without proof audit"));
      assert!(
        err.contains("E_CALL_ARGUMENT_UNPROVEN"),
        "{label} expected E_CALL_ARGUMENT_UNPROVEN, got: {err}"
      );
    }

    // `:open :intrinsic` results, decoded results, and discarded open side-effects pass when proof audit is disabled.
    for (label, body) in [
      ("intrinsic into defn", "take-num (make-intrinsic)"),
      ("intrinsic into proc &+", "&+ (make-intrinsic) 1"),
      ("if decoded concrete result", "&+ (if true (decode-map-as (make-open) 'Number) 1) 1"),
      ("do discarded open before concrete", "&+ (do (make-open) 1) 1"),
      ("do discarded open before intrinsic", "&+ (do (make-open) (make-intrinsic)) 1"),
      ("narrowed number?", "let ((v (make-open))) (if (number? v) (&+ v 1) 0)"),
      ("narrowed bool?", "let ((v (make-open))) (if (bool? v) (not v) false)"),
    ] {
      let entries = build_entries(body);
      run_check_only_with_output(&entries, false)
        .unwrap_or_else(|err| panic!("{label} should pass when proof audit is disabled: {err}"));
    }
  });
}

#[test]
fn union_slot_enum_literal_requires_a_unique_variant() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _strict = StrictTypesReset::enabled();
    for (remote, payload, expected_ok) in [
      ("(:remote String)", "true", true),
      ("(:states (List String) Bool)", "true", false),
      ("(:remote String)", "|wrong", false),
      ("(:remote String)", "true false", false),
      ("(:remote String)", "(source)", false),
    ] {
      builtins::effects::init_effects_states();
      let code = format!(
        "def slot $ deftype-slot :op\n\ndefenum Local $ :states (List String) Bool\n\ndefenum Remote {remote}\n\ndef Ui $ defunion Ui Local Remote\n\ndefn source () true\n\ndef Handler $ fn (d!) &unit\n\ndefstruct Props $ :on-click $ quote (JsNullish app.main/Handler)\n\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , slot\n    Props :on-click $ fn (d!)\n      d! $ :: :states ([] |a) {payload}\n    , &unit"
      );
      let mut file = snapshot::create_file_from_snippet(&code).expect("fixture should parse");
      let schema = cirru_parser::parse("Fn\n  Fn *op $ -> Unit\n  -> Unit").unwrap().remove(0);
      file.defs.get_mut("Handler").unwrap().schema = snapshot::parse_schema_annotation_for_write(&schema).unwrap();
      let open_schema = cirru_parser::parse("Fn (-> Dynamic) (:open :decode)").unwrap().remove(0);
      file.defs.get_mut("source").unwrap().schema = snapshot::parse_schema_annotation_for_write(&open_schema).unwrap();
      let mut snapshot = snapshot::Snapshot::default();
      snapshot.files.insert("app.main".to_owned(), file);
      snapshot
        .entries
        .get_mut("default")
        .unwrap()
        .type_slots
        .insert("op".to_owned(), "app.main/Ui".to_owned());
      let entries = prepare_snapshot_entries(snapshot);
      let result = run_check_only(&entries);
      assert_eq!(result.is_ok(), expected_ok, "remote={remote}, payload={payload}: {result:?}");
    }
  });
}

#[test]
fn union_argument_literal_checks_payload_for_named_functions() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _strict = StrictTypesReset::enabled();
    let _no_audit = ProofAuditReset::disabled();
    for (payload, expected_ok) in [("true", true), ("|wrong", false), ("true false", false), ("(source)", false)] {
      builtins::effects::init_effects_states();
      let code = format!(
        "defenum Local (:states Bool)\n\ndefenum Remote (:other String)\n\ndef Ui $ defunion Ui Local Remote\n\ndefn source () true\n\ndefn accept (value) &unit\n\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  accept $ :: :states {payload}\n  , &unit"
      );
      let mut file = snapshot::create_file_from_snippet(&code).expect("fixture should parse");
      let schema = cirru_parser::parse("Fn Ui (-> Unit)").unwrap().remove(0);
      file.defs.get_mut("accept").unwrap().schema = snapshot::parse_schema_annotation_for_write(&schema).unwrap();
      let open_schema = cirru_parser::parse("Fn (-> Dynamic) (:open :decode)").unwrap().remove(0);
      file.defs.get_mut("source").unwrap().schema = snapshot::parse_schema_annotation_for_write(&open_schema).unwrap();
      let mut snapshot = snapshot::Snapshot::default();
      snapshot.files.insert("app.main".to_owned(), file);
      let entries = prepare_snapshot_entries(snapshot);
      let result = run_check_only(&entries);
      assert_eq!(result.is_ok(), expected_ok, "payload={payload}: {result:?}");
    }
  });
}

#[test]
fn union_external_object_trait_accepts_and_narrows() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _strict = StrictTypesReset::enabled();
    let _no_audit = ProofAuditReset::disabled();

    let host_lib = r#"
defexternal BlobHost
  :target :browser
  :size Number
  :type String

defexternal FileHost
  :target :browser
  requires BlobHost
  :name String
"#;

    let app_code = r#"
ns app.main
  :require
    js-ffi.browser :refer $ BlobHost FileHost

defunion UploadFile String js-ffi.browser/BlobHost

defn describe-upload (file)
  hint-fn $ Fn UploadFile (-> String) (:features :js-ffi)
  match file
    (String path) path
    (BlobHost blob) (blob :type)

defn describe-qualified (file)
  hint-fn $ Fn UploadFile (-> String) (:features :js-ffi)
  match file
    (String path) path
    (js-ffi.browser/BlobHost blob) (blob :type)

defn accept-upload (file)
  hint-fn $ Fn UploadFile (-> String)
  match file
    (String path) path
    (BlobHost _blob) |blob

defn pass-file-host (f)
  hint-fn $ Fn FileHost (-> String)
  accept-upload f

defn main! ()
  hint-fn $ Fn (-> Unit)
  describe-upload |local.txt
  describe-qualified |local.txt
  accept-upload |local.txt
  , pass-file-host
  , &unit
"#;

    let build_snapshot = |target: Option<snapshot::SnapshotTarget>| {
      builtins::effects::init_effects_states();
      let mut snap = snapshot::Snapshot::default();
      snap.files.insert(
        "js-ffi.browser".to_owned(),
        snapshot::create_file_from_snippet(host_lib).expect("host lib should parse"),
      );
      snap.files.insert(
        "app.main".to_owned(),
        snapshot::create_file_from_snippet(app_code).expect("app code should parse"),
      );
      let entry = snap.entries.get_mut("default").expect("default entry");
      entry.mode = snapshot::SnapshotRunMode::Js;
      entry.target = target;
      snap
    };

    struct CodegenModeReset(bool);
    impl Drop for CodegenModeReset {
      fn drop(&mut self) {
        calcit::codegen::set_codegen_mode(self.0);
      }
    }
    let _js_mode = {
      let prev = calcit::codegen::codegen_mode();
      calcit::codegen::set_codegen_mode(true);
      CodegenModeReset(prev)
    };

    // Browser target: full check including browser-targeted external field access passes.
    let browser_entries = prepare_snapshot_entries(build_snapshot(Some(snapshot::SnapshotTarget::Browser)));
    run_check_only(&browser_entries).expect("browser entry should accept defunion with external-object trait and match narrowing");

    // Node target: using UploadFile without invoking browser-only host operations passes (#2903).
    let node_app = r#"
ns app.main
  :require
    js-ffi.browser :refer $ BlobHost FileHost

defunion UploadFile String js-ffi.browser/BlobHost

defn accept-upload (file)
  hint-fn $ Fn UploadFile (-> String)
  match file
    (String path) path
    (BlobHost _blob) |blob

defn pass-file-host (f)
  hint-fn $ Fn FileHost (-> String)
  accept-upload f

defn main! ()
  hint-fn $ Fn (-> Unit)
  accept-upload |local.txt
  , &unit
"#;
    builtins::effects::init_effects_states();
    let mut node_snap = snapshot::Snapshot::default();
    node_snap.files.insert(
      "js-ffi.browser".to_owned(),
      snapshot::create_file_from_snippet(host_lib).expect("host lib should parse"),
    );
    node_snap.files.insert(
      "app.main".to_owned(),
      snapshot::create_file_from_snippet(node_app).expect("node app should parse"),
    );
    let node_entry = node_snap.entries.get_mut("default").expect("default entry");
    node_entry.mode = snapshot::SnapshotRunMode::Js;
    node_entry.target = Some(snapshot::SnapshotTarget::Node);
    let node_entries = prepare_snapshot_entries(node_snap);
    run_check_only(&node_entries).expect("node entry should accept defunion referencing browser external-object trait");
  });
}

#[test]
fn union_external_object_trait_rejects_invalid_and_overlapping() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _strict = StrictTypesReset::enabled();
    let _no_audit = ProofAuditReset::disabled();

    let host_lib = r#"
defexternal BlobHost
  :size Number
  :type String

defexternal FileHost
  requires BlobHost
  :name String

defexternal OtherHost
  :id String

deftrait OrdinaryTrait
  .label $ Fn Self (-> String)
"#;

    let check_case = |label: &str, app_code: &str, expected_code: &str| {
      builtins::effects::init_effects_states();
      let mut snap = snapshot::Snapshot::default();
      snap.files.insert(
        "js-ffi.browser".to_owned(),
        snapshot::create_file_from_snippet(host_lib).expect("host lib should parse"),
      );
      snap.files.insert(
        "app.main".to_owned(),
        snapshot::create_file_from_snippet(app_code).expect("app code should parse"),
      );
      let entries = prepare_snapshot_entries(snap);
      if matches!(expected_code, "E_FN_ARG_TYPE_MISMATCH" | "E_HOST_VALUE_EQUALITY") {
        let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
        let _ = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
        let warnings = warnings.borrow();
        assert!(
          warnings.iter().any(|warning| warning.code() == Some(expected_code)),
          "{label} should report {expected_code}: {warnings:?}"
        );
      } else {
        let err = run_check_only(&entries).expect_err(label);
        assert!(err.contains(expected_code), "{label} should report {expected_code}: {err}");
      }
    };

    // 1. Ordinary non-external trait stays rejected.
    check_case(
      "ordinary trait in defunion",
      "ns app.main\n  :require\n    js-ffi.browser :refer $ OrdinaryTrait\n\ndefunion Bad String OrdinaryTrait\n\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , Bad\n  , &unit",
      "E_UNION_MEMBER_INVALID",
    );

    // 2. Two distinct external-object traits share runtime kind :js-object -> rejected.
    check_case(
      "two external-object traits",
      "ns app.main\n  :require\n    js-ffi.browser :refer $ BlobHost OtherHost\n\ndefunion TwoHosts BlobHost OtherHost\n\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , TwoHosts\n  , &unit",
      "E_UNION_MEMBER_INVALID",
    );

    // 3. Parent + child external-object traits overlap -> rejected.
    check_case(
      "parent and child external-object traits",
      "ns app.main\n  :require\n    js-ffi.browser :refer $ BlobHost FileHost\n\ndefunion SubHosts BlobHost FileHost\n\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , SubHosts\n  , &unit",
      "E_UNION_MEMBER_INVALID",
    );

    // 4. Nested union with another external-object trait -> rejected after flattening.
    check_case(
      "nested union overlapping external-object trait",
      "ns app.main\n  :require\n    js-ffi.browser :refer $ BlobHost OtherHost\n\ndefunion UploadFile String BlobHost\n\ndefunion Extended UploadFile OtherHost\n\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , Extended\n  , &unit",
      "E_UNION_MEMBER_INVALID",
    );

    // 5. Unrelated external-object trait cannot flow into UploadFile.
    check_case(
      "unrelated external-object trait rejected",
      "ns app.main\n  :require\n    js-ffi.browser :refer $ BlobHost OtherHost\n\ndefunion UploadFile String BlobHost\n\ndefn take-upload (u)\n  hint-fn $ Fn UploadFile (-> Unit)\n  , &unit\n\ndefn bad (other)\n  hint-fn $ Fn OtherHost (-> Unit)\n  take-upload other\n\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , bad\n  , &unit",
      "E_FN_ARG_TYPE_MISMATCH",
    );

    // 6. Bare JsObject cannot flow into UploadFile without js-cast.
    check_case(
      "bare JsObject rejected",
      "ns app.main\n  :require\n    js-ffi.browser :refer $ BlobHost\n\ndefunion UploadFile String BlobHost\n\ndefn take-upload (u)\n  hint-fn $ Fn UploadFile (-> Unit)\n  , &unit\n\ndefn bad (raw)\n  hint-fn $ Fn JsObject (-> Unit)\n  take-upload raw\n\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , bad\n  , &unit",
      "E_FN_ARG_TYPE_MISMATCH",
    );

    // 7. Value equality on UploadFile is rejected as host value equality.
    check_case(
      "value equality on host union rejected",
      "ns app.main\n  :require\n    js-ffi.browser :refer $ BlobHost\n\ndefunion UploadFile String BlobHost\n\ndefn same? (a b)\n  hint-fn $ Fn UploadFile UploadFile (-> Bool)\n  = a b\n\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , same?\n  , &unit",
      "E_HOST_VALUE_EQUALITY",
    );
  });
}

#[cfg(feature = "wasm")]
#[test]
fn union_external_object_trait_rejected_on_wasm() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _strict = StrictTypesReset::enabled();
    let _no_audit = ProofAuditReset::disabled();

    let host_lib = r#"
defexternal BlobHost
  :size Number
  :type String
"#;
    let app_code = r#"
ns app.main
  :require
    js-ffi.browser :refer $ BlobHost

defunion UploadFile String BlobHost

defn describe-upload (file)
  hint-fn $ Fn UploadFile (-> String)
  match file
    (String path) path
    (BlobHost _blob) |blob

defn main! ()
  hint-fn $ Fn (-> Unit)
  describe-upload |local.txt
  , &unit
"#;
    builtins::effects::init_effects_states();
    let mut snap = snapshot::Snapshot::default();
    snap.files.insert(
      "js-ffi.browser".to_owned(),
      snapshot::create_file_from_snippet(host_lib).expect("host lib should parse"),
    );
    snap.files.insert(
      "app.main".to_owned(),
      snapshot::create_file_from_snippet(app_code).expect("app code should parse"),
    );
    let entries = prepare_snapshot_entries(snap);
    run_check_only(&entries).expect("snapshot should preprocess cleanly before WASM target validation");

    for target in [calcit::wasm::emit_wasm::WasmTarget::Core, calcit::wasm::emit_wasm::WasmTarget::Wasi] {
      let err = calcit::wasm::emit_wasm::validate_wasm_target(&entries.init_ns, &entries.init_def, target)
        .expect_err("WASM/WASI target must reject union with external-object host trait member");
      assert!(
        err.contains("E_WASM_UNSUPPORTED_JS_FFI"),
        "expected E_WASM_UNSUPPORTED_JS_FFI for {target:?}, got: {err}"
      );
    }
  });
}

#[test]
fn closed_defunion_trait_propagation_and_tightened_bounds() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _strict = StrictTypesReset::enabled();
    let _no_audit = ProofAuditReset::disabled();

    let run_app = |app_code: &str| {
      builtins::effects::init_effects_states();
      let mut snap = snapshot::Snapshot::default();
      snap.files.insert(
        "app.main".to_owned(),
        snapshot::create_file_from_snippet(app_code).expect("app code should parse"),
      );
      let entries = prepare_snapshot_entries(snap);
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      let compile_res =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      (compile_res, warnings.into_inner(), entries)
    };

    // Positive: recursive union (CirruNode and custom Tree) propagates Debug/Eq/Hash;
    // union of Compare scalars propagates Compare; String | Ref Number propagates Debug.
    let positive_code = r#"
ns app.main

defunion Tree Number (List Tree)
defunion OrderedScalar Number String Tag Bool Symbol
defunion MixedDebug String (Ref Number)

defn debug-generic (x)
  hint-fn $ Fn T (-> String) (:generics T) (:where $ T Debug)
  x .debug

defn check-tree (t)
  hint-fn $ Fn Tree (-> Number)
  let
      d $ debug-generic t
      xs $ distinct ([] t)
    assert-type d String
    assert-type xs (List Tree)
    hash t

defn check-cirru (node)
  hint-fn $ Fn CirruNode (-> Number)
  let
      _d $ debug-generic node
      _xs $ distinct ([] node)
      _freq $ frequencies ([] node)
      _grp $ group-by ([] node) (fn (n) n)
      _idx $ index-of ([] node) node
    hash node

defn check-ordered (a b)
  hint-fn $ Fn OrderedScalar OrderedScalar (-> (List OrderedScalar))
  let
      _c $ compare a b
    sort ([] a b)

defn check-mixed-debug (m)
  hint-fn $ Fn MixedDebug (-> String)
  debug-generic m

defn main! ()
  hint-fn $ Fn (-> Unit)
  , check-tree check-cirru check-ordered check-mixed-debug
  , &unit
"#;
    let (res, warnings, entries) = run_app(positive_code);
    assert!(res.is_ok(), "positive closed-defunion test failed: {res:?}");
    assert!(warnings.is_empty(), "unexpected warnings in positive test: {warnings:?}");
    run_check_only(&entries).expect("positive closed-defunion snapshot should pass check-only");

    // Negative cases: CirruNode / Tree reject Compare; MixedDebug rejects Eq / Hash / Compare.
    for (label, bad_code) in [
      (
        "CirruNode rejected by sort Compare",
        "ns app.main\ndefn bad (n)\n  hint-fn $ Fn CirruNode (-> (List CirruNode))\n  sort ([] n)\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , bad\n  , &unit",
      ),
      (
        "Tree rejected by compare Compare",
        "ns app.main\ndefunion Tree Number (List Tree)\ndefn bad (a b)\n  hint-fn $ Fn Tree Tree (-> Number)\n  compare a b\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , bad\n  , &unit",
      ),
      (
        "MixedDebug rejected by hash Hash",
        "ns app.main\ndefunion MixedDebug String (Ref Number)\ndefn bad (m)\n  hint-fn $ Fn MixedDebug (-> Number)\n  hash m\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , bad\n  , &unit",
      ),
      (
        "MixedDebug rejected by distinct Eq Hash",
        "ns app.main\ndefunion MixedDebug String (Ref Number)\ndefn bad (m)\n  hint-fn $ Fn MixedDebug (-> (List MixedDebug))\n  distinct ([] m)\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , bad\n  , &unit",
      ),
    ] {
      let (_res, warnings, _entries) = run_app(bad_code);
      assert!(
        warnings.iter().any(|w| w.code() == Some("E_GENERIC_WHERE_BOUND_MISMATCH")),
        "{label} should emit E_GENERIC_WHERE_BOUND_MISMATCH, got: {warnings:?}"
      );
    }
  });
}

#[test]
fn tightened_core_generic_schemas_enforce_bounds_and_cases() {
  run_with_large_stack(|| {
    injection::inject_platform_apis();
    let _strict = StrictTypesReset::enabled();
    let _no_audit = ProofAuditReset::disabled();

    let run_app = |app_code: &str| {
      builtins::effects::init_effects_states();
      let mut snap = snapshot::Snapshot::default();
      snap.files.insert(
        "app.main".to_owned(),
        snapshot::create_file_from_snippet(app_code).expect("app code should parse"),
      );
      let entries = prepare_snapshot_entries(snap);
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      let compile_res =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      (compile_res, warnings.into_inner(), entries)
    };

    // Positive: empty collections (Never -> TypeVar), Never-then-concrete, Buffer,
    // Option, 2-arg custom sort on Fn values, &list:sort-by with :tag selector,
    // and constrained generic callers all pass cleanly.
    let positive_code = r#"
ns app.main

defstruct Person (:name String)

defn constrained-sort (xs)
  hint-fn $ Fn (List T) (-> (List T)) (:generics T) (:where $ T Compare)
  sort xs

defn take-numbers (ys)
  hint-fn $ Fn (List Number) (-> Number)
  count ys

defn main! ()
  hint-fn $ Fn (-> Unit)
  let
      f1 $ fn (x) (+ x 1)
      f2 $ fn (x) (+ x 2)
      buf $ &buffer 1 2
      _d1 $ distinct ([])
      _d2 $ &list:distinct ([])
      _s1 $ &list:to-set ([])
      _dv $ distinct-values (&{})
      _fr $ frequencies ([])
      _gb $ group-by ([]) (fn (x) x)
      _so $ sort ([])
      _sb $ &list:sort-by ([]) (fn (x) x)
      _sb-method $ .sort-by ([] 3 1 2) (fn (x) (hint-fn $ Fn Number (-> Number)) x)
      _sb-method-id $ .sort-by ([] 3 1 2) identity
      _sb-method-tag $ .sort-by ([] (Person :name |b) (Person :name |a)) :name
      _chained-sort $ let ((xs $ distinct ([]))) (sort xs)
      _chained-freq $ frequencies (distinct ([]))
      _chained-nums $ take-numbers (distinct ([]))
      _chained-method $ .sort (.slice (distinct ([])) 0)
      _chained-method-sb $ .sort-by (distinct ([])) (fn (x) x)
      _ix1 $ index-of ([]) 1
      _ix2 $ &list:index-of-from ([]) 0 1
      _ix3 $ &list:last-index-of ([]) 1
      _buf-d $ distinct ([] buf)
      _opt-f $ frequencies ([] (Some 1) (Some 2))
      _sort-tags $ constrained-sort ([] :b :a)
      _sort-custom $ sort ([] f1 f2) (fn (_a _b) 0)
      _sort-tag-sel $ &list:sort-by ([] (Person :name |b) (Person :name |a)) :name
      _cmp $ compare :a :b
      _h $ hash buf
    , &unit
"#;
    let (res, warnings, entries) = run_app(positive_code);
    assert!(res.is_ok(), "positive tightened generics test failed: {res:?}");
    assert!(
      warnings.is_empty(),
      "unexpected warnings in tightened generics positive test: {warnings:?}"
    );
    run_check_only(&entries).expect("positive tightened generics snapshot should pass check-only");

    // Negative: every un-bounded / non-satisfying call reports E_GENERIC_WHERE_BOUND_MISMATCH.
    for (label, bad_code) in [
      (
        "distinct rejects Fn",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (distinct ([] (fn (x) x)))))\n    &unit",
      ),
      (
        "&list:distinct rejects Fn",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (&list:distinct ([] (fn (x) x)))))\n    &unit",
      ),
      (
        "&list:to-set rejects Ref",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (&list:to-set ([] (ref 1)))))\n    &unit",
      ),
      (
        "distinct-values rejects Fn map values",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (distinct-values ({} (:a (fn (x) x))))))\n    &unit",
      ),
      (
        "frequencies rejects BufList",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (frequencies ([] (&buf-list:new)))))\n    &unit",
      ),
      (
        "group-by rejects Fn keys",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (group-by ([] 1 2) (fn (x) (fn () x)))))\n    &unit",
      ),
      (
        "index-of rejects Fn elements",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (index-of ([] (fn (x) x)) (fn (y) y))))\n    &unit",
      ),
      (
        "index-of empty list then Fn element rejects Fn",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (index-of ([]) (fn (x) x))))\n    &unit",
      ),
      (
        "&list:index-of-from rejects Ref",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (&list:index-of-from ([] (ref 1)) 0 (ref 2))))\n    &unit",
      ),
      (
        "&list:last-index-of rejects BufList",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (&list:last-index-of ([] (&buf-list:new)) (&buf-list:new))))\n    &unit",
      ),
      (
        "1-arg sort rejects Fn",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (sort ([] (fn (x) x)))))\n    &unit",
      ),
      (
        "&list:sort-by key-fn rejects Fn keys",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (&list:sort-by ([] 1 2) (fn (x) (fn () x)))))\n    &unit",
      ),
      (
        ".sort-by method key-fn rejects Fn keys",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (.sort-by ([] 1 2) (fn (x) (hint-fn $ Fn Number (-> (Fn Number (-> Number)))) (fn (y) y)))))\n    &unit",
      ),
      (
        "hash rejects Ref",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (hash (ref 1))))\n    &unit",
      ),
      (
        "compare rejects Buffer",
        "ns app.main\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (compare (&buffer 1) (&buffer 2))))\n    &unit",
      ),
      (
        "unconstrained generic caller distinct rejected",
        "ns app.main\ndefn bad (xs)\n  hint-fn $ Fn (List T) (-> (List T)) (:generics T)\n  distinct xs\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , bad\n  , &unit",
      ),
      (
        "unconstrained generic caller sort rejected",
        "ns app.main\ndefn bad (xs)\n  hint-fn $ Fn (List T) (-> (List T)) (:generics T)\n  sort xs\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , bad\n  , &unit",
      ),
      (
        "unconstrained generic caller group-by rejected",
        "ns app.main\ndefn bad (xs)\n  hint-fn $ Fn (List T) (-> Dynamic) (:generics T)\n  group-by xs (fn (x) x)\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , bad\n  , &unit",
      ),
      (
        "unconstrained generic capture in empty group-by rejected",
        "ns app.main\ndefn bad (y)\n  hint-fn $ Fn T (-> Dynamic) (:generics T)\n  group-by ([]) (fn (_x) y)\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  , bad\n  , &unit",
      ),
    ] {
      let (_res, warnings, _entries) = run_app(bad_code);
      assert!(
        warnings.iter().any(|w| w.code() == Some("E_GENERIC_WHERE_BOUND_MISMATCH")),
        "{label} should emit E_GENERIC_WHERE_BOUND_MISMATCH, got: {warnings:?}"
      );
    }

    let (trait_err, _trait_warnings, _entries) = run_app(
      "ns app.main\ndeftrait CustomMarker\n  .cmark $ Fn Self (-> Number)\ndefstruct MyBox (:x Number) (:impls CustomMarker)\ndefimpl MyBox CustomMarker\n  .cmark $ fn (self) 9\ndefn main! ()\n  hint-fn $ Fn (-> Unit)\n  let ((_x (&trait-call CustomMarker :cmark MyBox)))\n    &unit",
    );
    let trait_err = trait_err.expect_err("StructDef must fail static &trait-call for instance-only trait");
    assert_eq!(trait_err.code(), Some("E_TRAIT_CALL_IMPL_MISSING"), "unexpected: {trait_err:?}");
  });
}
