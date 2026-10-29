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
        "let\n    Draft $ defstruct Draft (:ready? 'Bool) (:maybe $ :: Option 'Number)\n    drafts $ {{}} $ |a $ Draft :ready? false :maybe (Option :some 1)\n  match (get drafts |a)\n    (:some draft) $ {update}\n    (:none) &unit"
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
          .all(|warning| warning.code() != Some("W_NOMINAL_ENUM_LEGACY_USE")),
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
      warnings.iter().any(|warning| warning.code() == Some("W_FN_ARG_TYPE_MISMATCH")
        && warning.message().contains("enum Option<:number>")
        && warning.message().contains(":nil")),
      "expected W_FN_ARG_TYPE_MISMATCH for Option vs nil: {warnings:?}"
    );
  });
}

// Pins today's contract while `nil->option` still takes `Optional<T>`: a Rust
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
      warnings.iter().any(|warning| warning.code() == Some("W_FN_ARG_TYPE_MISMATCH")
        && warning.message().contains("calcit.core/nil->option")
        && warning.message().contains("js-nullish<:number>")),
      "nil->option must keep rejecting JsNullish arguments: {warnings:?}"
    );

    // Ordinary values, nil and a legacy Optional argument stay warning-free.
    for source in [
      "nil->option nil",
      "let ((value (Option :some 1)))\n  nil->option value",
      "let\n    f $ fn (x)\n      hint-fn $ Fn (Optional Number)\n        -> $ Option Number\n      nil->option x\n  f 1",
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

// `map xs nil->option` is accepted only when the list element type is known;
// an untyped list literal reports E_ERASED_GENERIC_RELATION today.
#[test]
fn strict_type_fail_nil_to_option_as_map_callback_needs_typed_elements() {
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

    let untyped = load_snippet_entries("map ([] 1 nil 2) nil->option");
    let warnings = RefCell::new(vec![]);
    let error = runner::preprocess::ensure_ns_def_compiled(&untyped.init_ns, &untyped.init_def, &warnings, &CallStackList::default())
      .expect_err("an untyped list cannot prove the callback contract");
    assert!(error.to_string().contains("E_ERASED_GENERIC_RELATION"), "unexpected error: {error}");
  });
}

/// `value.:field` must report the same strict diagnostics as `(:field value)` (#1988).
#[test]
fn type_fail_tag_access_shorthand_matches_tag_call_diagnostics() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let collect = |access: &str| -> Vec<(Option<String>, String)> {
      let snippet = format!("let\n    Ev $ defstruct Ev (:type 'Tag)\n    ev $ Ev :type :create\n  {access}");
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
      shorthand.iter().any(|(code, _)| code.as_deref() == Some("W_UNKNOWN_STRUCT_FIELD")),
      "unknown field must be reported with W_UNKNOWN_STRUCT_FIELD: {shorthand:?}"
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
fn legacy_map_kv_contract_warns_in_compatibility_and_fails_in_strict_mode() {
  run_with_large_stack(|| {
    let snippets = [
      "let\n    output $ map-kv\n      {} $ :a 1\n      fn (k v) ([] k 1)\n  assert-type output $ quote $ Map Tag String\n  , output",
      "({} (:a 1)) .map-kv $ fn (k v) (:: :skip)",
    ];

    for snippet in snippets {
      let entries = load_snippet_entries(snippet);
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
        .expect("compatibility mode should retain the legacy map-kv boundary as a warning");
      let warnings = warnings.borrow();
      let contract_warnings = warnings
        .iter()
        .filter(|warning| warning.code() == Some("W_MAP_KV_UNPROVEN_CONTRACT"))
        .collect::<Vec<_>>();
      assert_eq!(contract_warnings.len(), 1, "expected one map-kv migration warning: {warnings:?}");
      assert!(contract_warnings[0].message().contains("result is Dynamic"));
      assert!(contract_warnings[0].message().contains("filter-map-kv"));
    }

    let main_schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
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
    for snippet in snippets {
      let entries = load_snippet_entries_with_main_schema(snippet, Some(main_schema.clone()));
      let error = run_check_only(&entries).expect_err("strict mode must reject the unproven map-kv contract");
      assert!(
        error.contains("E_MAP_KV_UNPROVEN_CONTRACT"),
        "unexpected strict map-kv error: {error}"
      );
      assert!(
        error.contains("MapEntryDecision"),
        "migration must name the typed replacement: {error}"
      );
    }
  });
}

#[test]
fn strict_map_list_kv_preserves_key_value_and_result_types() {
  run_with_large_stack(|| {
    let main_schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
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
    let entries = load_snippet_entries("defenum Tree (:leaf 'Number) (:branch Tree)\ndefn main! ()\n  Tree :branch $ Tree :leaf 1");

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
fn postfix_map_kv_gate_ignores_concrete_non_map_receivers() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("|text .map-kv $ fn (value) value");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    let result = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());

    if let Err(error) = result {
      assert!(!error.to_string().contains("E_MAP_KV_UNPROVEN_CONTRACT"), "{error}");
    }
    assert!(
      warnings
        .borrow()
        .iter()
        .all(|warning| warning.code() != Some("W_MAP_KV_UNPROVEN_CONTRACT")),
      "a concrete non-Map receiver must not be diagnosed as the legacy core map-kv contract"
    );
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
      .filter(|warning| warning.code() == Some("W_RESET_ARG_TYPE_MISMATCH"))
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
        "hint-fn $ Fn (-> Unit)\n\nlet\n    write $ fn (state key)\n      hint-fn $ Fn\n        Ref $ Map String Number\n        , {key_type} (-> Unit)\n      reset! state $ {value}\n      , &unit\n  , &unit"
      );
      assert_strict_snippet(&snippet, passes, &format!("key={key_type}, value={value}"));
    }
    // A List receiver keeps List<T> only when its single index is a proven Number.
    for (index_type, passes) in [("Number", true), ("Dynamic", false)] {
      let snippet = format!(
        "hint-fn $ Fn (-> Unit)\n\nlet\n    write $ fn (state index)\n      hint-fn $ Fn\n        Ref $ List String\n        , {index_type} (-> Unit)\n      reset! state $ dissoc (deref state) index\n      , &unit\n  , &unit"
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
      "fn (state value)\n  hint-fn $ Fn (Ref T) T (-> Unit) (:generics T)\n  reset! state |wrong\n  , &unit",
      "fn (state value)\n  hint-fn $ Fn (Ref Number) T (-> Unit) (:generics T)\n  reset! state value\n  , &unit",
      "fn (state value)\n  hint-fn $ Fn\n    Ref $ List T\n    List U\n    -> Unit\n    :generics T U\n  reset! state value\n  , &unit",
      "fn (state)\n  hint-fn $ Fn (Ref Number) (-> Unit)\n  reset! state |wrong\n  , &unit",
    ] {
      let entries = load_snippet_entries(snippet);
      run_check_only(&entries).expect_err("incompatible reset! must fail before typed reads can use stale evidence");
    }
    for snippet in [
      "let ((state $ ref 1)) (reset! state 2) (+ @state 1)",
      "fn (state value)\n  hint-fn $ Fn (Ref T) T (-> Unit) (:generics T)\n  reset! state value\n  , &unit",
      "fn (state value)\n  hint-fn $ Fn\n    Ref $ List T\n    List T\n    -> Unit\n    :generics T\n  reset! state value\n  , &unit",
      "let ((state $ ref $ {} (:count 1))) (reset! state $ {} (:count 2)) (get-in @state $ [] :count)",
      "fn (state)\n  hint-fn $ Fn (Ref Number) (-> Unit)\n  reset! state 2\n  , &unit",
      "fn (state)\n  hint-fn $ Fn (Ref Dynamic) (-> Unit)\n  reset! state |open-boundary\n  , &unit",
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
        "hint-fn $ Fn (-> Unit)\n\nlet\n    write $ fn (state value)\n      hint-fn $ Fn (Ref {payload}) Dynamic (-> Unit) (:generics T)\n      reset! state value\n      , &unit\n  , &unit"
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
        "hint-fn $ Fn (-> Unit)\n\nlet\n    write $ fn (state value)\n      hint-fn $ Fn (Ref {payload}) {input} (-> Unit)\n      reset! state {value}\n      , &unit\n  , &unit"
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
          warning.code() == Some("W_LOCAL_FN_ARG_TYPE_MISMATCH") || warning.code() == Some("W_FN_RETURN_TYPE_MISMATCH")
        }),
        "{name}: invalid callback passed without a typed diagnostic: {warnings:?}"
      );
    }
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
        warning.code() == Some("W_FN_RETURN_TYPE_MISMATCH")
          && warning
            .message()
            .contains("declares return type `:number`, but body returns `:string`")
      }),
      "the later Number argument should determine T, then the callback's String result should be diagnosed: {warnings:?}"
    );
    assert!(
      !warnings.iter().any(|warning| {
        warning.code() == Some("W_LOCAL_FN_ARG_TYPE_MISMATCH") && warning.message().contains("arg 2 expects type `:string`")
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
    let mut snapshot = snapshot::Snapshot::default();
    let mut file = snapshot::create_file_from_snippet(
      "defn invoke (callback) $ callback\n\ndefn main! () $ invoke $ fn () 1\n\ndefn reload! () &unit",
    )
    .expect("generic callback snippet should parse");
    let output = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let callback = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
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
          runtime_arity: None,
          generics: Arc::new(vec![]),
          where_bounds: Arc::new(vec![]),
          arg_types: vec![],
          return_type: Arc::new(if name == "main!" {
            CalcitTypeAnnotation::String
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
    let result = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
    assert!(result.is_ok(), "an unbound callback result slot must remain open: {result:?}");
    assert!(
      warnings.borrow().is_empty(),
      "an unbound output slot must not report a false return mismatch"
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
          Some("W_METHOD_ARG_TYPE_MISMATCH" | "W_FN_RETURN_TYPE_MISMATCH" | "W_LOCAL_FN_ARG_TYPE_MISMATCH")
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
        warning.code() == Some("W_LOCAL_FN_ARG_TYPE_MISMATCH") || warning.code() == Some("W_FN_RETURN_TYPE_MISMATCH")
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
        .any(|warning| warning.code() == Some("W_FN_ARG_TYPE_MISMATCH") && warning.message().contains("Option")),
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
      .filter(|warning| warning.code() == Some("W_HOST_VALUE_EQUALITY"))
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
      ("optionally 1", "optionally", "nil->option", "0.31.0"),
      ("join ([] 1 2) 0", "join", "intersperse", "0.31.0"),
      ("section-by ([] 1 2 3) 2", "section-by", "chunks", "0.31.0"),
      ("turn-str 1", "turn-str", "to-string", "0.31.0"),
      ("some-in? ({} (:a 1)) ([] :a)", "some-in?", ".ok? $ get-in x path", "0.31.0"),
      ("round? 1", "round?", "integer?", "0.31.0"),
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
      !codes.iter().any(|code| code == "W_NOMINAL_ENUM_LEGACY_USE"),
      "explicit Dynamic erasure must not warn as a nullable use: {codes:?}"
    );
  });
}

/// #2121: the default preflight proves directional Dynamic flows, while
/// `--compat-types` keeps accepting them.
#[test]
fn default_proof_rejects_directional_dynamic_flows() {
  let cases = [
    ("p03", "E_FN_RETURN_UNPROVEN"),
    ("p12", "E_FN_RETURN_UNPROVEN"),
    ("p21", "E_CALL_ARGUMENT_UNPROVEN"),
    ("p30", "E_FN_RETURN_UNPROVEN"),
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
      }
      let _compat = StrictTypesReset::disabled();
      run_check_only(&load_fixture_entries(&path)).expect("--compat-types keeps the old behavior");
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
        .any(|warning| warning.code() == Some("W_PROC_ARG_TYPE_MISMATCH") && warning.message().contains("&map:assoc")),
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
            .filter(|warning| matches!(warning.code(), Some("W_FN_ARG_TYPE_MISMATCH" | "W_LOCAL_FN_ARG_TYPE_MISMATCH")))
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
fn named_callback_schema_rejects_wrong_arguments_and_non_callable_aliases() {
  run_with_large_stack(|| {
    let fn_schema = |args: Vec<Arc<CalcitTypeAnnotation>>, return_type: CalcitTypeAnnotation| {
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
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
        warning.code() == Some("W_FN_ARG_TYPE_MISMATCH")
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
fn option_migration_source_calls_fail_during_preprocessing() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries(
      "do\n  = |dev $ get-env |mode\n  let\n      x $ get-env |mode\n    non-nil? x\n  update-in ({} (:a ({}))) ([] :a) $ fn (x) do (assoc x :b 1)\n  let\n      op $ :: :session/connect\n      tag-name $ nth op 0\n    starts-with? tag-name :session/",
    );
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("Option migration examples should preprocess with warnings, not reach runtime");

    let warnings = warnings.borrow();
    for operation in ["=", "non-nil?", "assoc"] {
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
fn parse_cirru_list_rejects_non_string_before_runtime() {
  run_with_large_stack(|| {
    let entries = load_snippet_entries("parse-cirru-list 1");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("invalid parser input should produce a type warning before runtime");
    assert!(
      warnings.borrow().iter().any(|warning| {
        warning.code() == Some("W_PROC_ARG_TYPE_MISMATCH")
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
        warning.code() == Some("W_PROC_ARG_TYPE_MISMATCH")
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
        warning.code() == Some("W_PROC_ARG_TYPE_MISMATCH")
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
      .filter(|warning| warning.code() == Some("W_DYNAMIC_NOMINAL_METHOD_RECEIVER"))
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
          .any(|warning| { warning.code() == Some("W_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/option:fold") }),
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
        .find(|warning| warning.code() == Some("W_PROC_ARG_TYPE_MISMATCH") && warning.message().contains(inferred))
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
          .any(|warning| warning.code() == Some("W_REQUIRED_STRUCT_FIELD_TYPE")),
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
      .filter(|warning| warning.code() == Some("W_STRUCT_PATH_OPERATION"))
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
      .filter(|warning| warning.code() == Some("W_METHOD_ARG_TYPE_MISMATCH"))
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
      .filter(|warning| warning.code() == Some("W_FN_RETURN_TYPE_MISMATCH"))
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
      .filter(|warning| warning.code() == Some("W_GENERIC_WHERE_BOUND_MISMATCH"))
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
      .filter(|warning| warning.code() == Some("W_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/update"))
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
        warning.message().contains("arg 3 expects type `fn(:number) -> :number`")
          && warning.message().contains("got `fn(:string) -> :string`")
      }),
      "Map update should bind the updater to its value type: {matched:?}"
    );
    assert!(
      matched.iter().any(|warning| {
        warning.message().contains("arg 3 expects type `fn(:number) -> :number`")
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
        warning.code() == Some("W_FN_ARG_TYPE_MISMATCH")
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
        warning.code() == Some("W_PROC_ARG_TYPE_MISMATCH")
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
      .filter(|warning| warning.code() == Some("W_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/filter"))
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
          warning.message().contains("arg 2 expects type `fn(:number) -> :bool`")
            && warning.message().contains("got `fn(:number) -> :number`")
        })
        .count(),
      3,
      "named and inline List/Set filters should pass their Number member type to a Bool predicate: {filter_warnings:?}"
    );
    assert!(
      filter_warnings.iter().any(|warning| {
        warning.message().contains("arg 2 expects type `fn(list<dynamic>) -> :bool`")
          && warning.message().contains("got `fn(:number) -> :number`")
      }),
      "Map filters should expose their heterogeneous pair as a List-shaped predicate argument: {filter_warnings:?}"
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
        warning.code() == Some("W_FN_ARG_TYPE_MISMATCH")
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
          && warning.message().contains("expects type `fn(:number) -> :bool`")
          && warning.message().contains("got `fn(:number) -> :number`")
      }),
      "any? should require a Bool predicate over the List member type: {iteration_warnings:?}"
    );
    assert!(
      iteration_warnings.iter().any(|warning| {
        warning.message().contains("calcit.core/every?")
          && warning.message().contains("expects type `fn(list<dynamic>) -> :bool`")
          && warning.message().contains("got `fn(:number) -> :number`")
      }),
      "every? should expose Map entries as heterogeneous List pairs: {iteration_warnings:?}"
    );
    assert!(
      iteration_warnings.iter().any(|warning| {
        warning.message().contains("calcit.core/each")
          && warning.message().contains("expects type `fn(:string) -> dynamic`")
          && warning.message().contains("got `fn(:number) -> :number`")
      }),
      "each should constrain callback input while allowing any return type: {iteration_warnings:?}"
    );
    let map_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("W_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/map"))
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
          warning.message().contains("arg 2 expects type `fn(:string) -> :number`")
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
          .contains("arg 2 expects type `fn(list<dynamic>) -> list<dynamic>`")
          && warning.message().contains("got `fn(:number) -> :number`")
      }),
      "Map mappers should consume and return heterogeneous List pairs: {map_warnings:?}"
    );
    let fold_warnings = warnings
      .iter()
      .filter(|warning| {
        matches!(warning.code(), Some("W_PROC_ARG_TYPE_MISMATCH") | Some("W_FN_ARG_TYPE_MISMATCH"))
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
          warning.message().contains("expects type `fn(:number, :string) -> :number`")
            && warning.message().contains("got `fn(:number, & :number) -> :number`")
        })
        .count(),
      3,
      "List/Set foldl and Set reduce should preserve String member types: {fold_warnings:?}"
    );
    assert!(
      fold_warnings.iter().any(|warning| {
        warning.message().contains("Proc `foldl`")
          && warning.message().contains("expects type `fn(:number, list<dynamic>) -> :number`")
          && warning.message().contains("got `fn(:number, & :number) -> :number`")
      }),
      "Map foldl should expose entries as heterogeneous List pairs: {fold_warnings:?}"
    );
    let sort_warnings = warnings
      .iter()
      .filter(|warning| {
        warning.code() == Some("W_PROC_ARG_TYPE_MISMATCH")
          && (warning.message().contains("Proc `sort`") || warning.message().contains("Proc `&list:sort`"))
      })
      .collect::<Vec<_>>();
    assert_eq!(
      sort_warnings.len(),
      3,
      "collection sorts should validate comparator contracts: {warnings:?}"
    );
    assert!(
      sort_warnings
        .iter()
        .all(|warning| warning.message().contains("expects type `fn(:string, :string) -> :number`")),
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
      .filter(|warning| warning.code() == Some("W_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/&list:sort-by"))
      .collect::<Vec<_>>();
    assert_eq!(
      sort_by_warnings.len(),
      1,
      "list sort-by should validate function selectors against the member type: {warnings:?}"
    );
    assert!(
      sort_by_warnings[0].message().contains("expects type `fn(:string) -> 'SortKey`")
        && sort_by_warnings[0].message().contains("got `fn(:number) -> :number`"),
      "sort-by should preserve the String member type while leaving its key type generic: {sort_by_warnings:?}"
    );
    let list_apply_warnings = warnings
      .iter()
      .filter(|warning| warning.code() == Some("W_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/&list:apply"))
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
      .filter(|warning| warning.code() == Some("W_METHOD_ARG_TYPE_MISMATCH") && warning.message().contains("Method `.apply`"))
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
      .filter(|warning| warning.code() == Some("W_FN_ARG_TYPE_MISMATCH") && warning.message().contains("calcit.core/interleave"))
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
        warning.code() == Some("W_FN_ARG_TYPE_MISMATCH")
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
        warning.code() == Some("W_FN_ARG_TYPE_MISMATCH")
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
        warning.code() == Some("W_PROC_ARG_TYPE_MISMATCH")
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
          .contains("Proc `add-watch!` arg 3 expects type `fn(:string, :string) -> :unit`")
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
      .filter(|warning| warning.code() == Some("W_METHOD_ARG_TYPE_MISMATCH") && warning.message().contains("Method `.merge` variadic"))
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
      .filter(|warning| warning.code() == Some("W_DYNAMIC_CALL"))
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
/// under `--compat-types`; statement position, an explicit else and an `Option`
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
      ("typed-return-when", number_fn("(when (> x 0) x)", "Number"), true),
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

    for (label, function) in [
      ("let-binding", number_fn("(let ((y (if (> x 0) x))) y)", "Dynamic")),
      ("let-nil-join", number_fn("(let ((y (if (> x 0) x nil))) y)", "Dynamic")),
    ] {
      let (result, warnings, advisories) = compile(function);
      assert!(result.is_ok(), "{label}: a `let` binding is only noted: {result:?}");
      assert!(warnings.iter().all(|w| !is_implicit(w.code())), "{label}: {warnings:?}");
      let noted = advisories.iter().filter(|a| a.code() == Some("W_IMPLICIT_NIL_BRANCH")).count();
      assert_eq!(noted, 1, "{label}: {advisories:?}");
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
  });
}

/// #1554: a branch written wholly by a library macro is skipped, but a library
/// macro that wraps the caller's form in an else-less `if` is reported at the
/// caller; `nil->option` around the macro call, then an Option helper, is the fix.
#[test]
fn library_macro_wrapping_caller_form_reports_implicit_nil_at_the_caller() {
  run_with_large_stack(|| {
    let library = "defmacro maybe (c x)\n  quasiquote $ if ~c ~x\n\ndefmacro quiet ()\n  quasiquote $ if (> 1 0) |a";
    let compile = |body: &str| {
      let snippet = format!(
        "ns app.main $ :require $ lib.core :refer (maybe quiet)\n\nlet\n    probe $ fn (flag)\n      hint-fn $ Fn Bool (-> String)\n      , {body}\n  probe true"
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
      ("nil-option-fix", "(str |x (.unwrap-or (nil->option (maybe flag |b)) |))"),
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
      ("wrong-live-payload", nullable_return("(if flag |bad nil)"), Some("W_FN_RETURN_TYPE_MISMATCH")),
      ("wrong-reversed-payload", nullable_return("(if flag nil |bad)"), Some("W_FN_RETURN_TYPE_MISMATCH")),
      ("missing-else-payload", nullable_return("(if flag |bad)"), Some("W_FN_RETURN_TYPE_MISMATCH")),
      ("matching-payload", nullable_return("(if flag 7 nil)"), None),
      ("let-bound-wrong-payload", nullable_return("(let ((y (if flag |bad nil))) y)"), Some("W_FN_RETURN_TYPE_MISMATCH")),
      ("let-bound-matching-payload", nullable_return("(let ((y (if flag 7 nil))) y)"), None),
      (
        "wrong-argument-payload",
        "fn (flag) (hint-fn $ Fn Bool (-> Unit)) $ let ((take $ fn (v) (hint-fn $ Fn (JsNullish Number) (-> Unit)) &unit)) $ take $ if flag |bad nil".to_owned(),
        Some("W_LOCAL_FN_ARG_TYPE_MISMATCH"),
      ),
    ];
    for (label, function, expected_code) in cases {
      let snippet = format!("let\n    probe $ {function}\n  probe true");
      let entries = load_snippet_entries(&snippet);
      let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
      let _ = runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default());
      let warnings = warnings.borrow();
      let codes = warnings.iter().filter_map(LocatedWarning::code).collect::<Vec<_>>();
      match expected_code {
        Some(code) => assert!(codes.contains(&code), "{label}: expected {code}, got {warnings:?}"),
        None => assert!(
          !codes.iter().any(|code| code.ends_with("TYPE_MISMATCH")),
          "{label}: a matching payload should be accepted, got {warnings:?}"
        ),
      }
    }
  });
}

/// #1554 regression set: every case is one definition in
/// `tests/fixtures/nil-branch-contracts.cirru`. Until `Optional<T>` is removed
/// (0.32.0) a nil-yielding branch keeps its legacy nullable evidence for every
/// contract check (returns, function / local / proc / method arguments, struct
/// fields, `assert-type`, callbacks, `let` / `do` / nested bodies and `.?-field`),
/// so `E_IMPLICIT_NIL_BRANCH` may reject more programs than main but must never
/// accept one that main rejected.
/// Columns: case, rejected on main, rejected now, the `IMPLICIT_NIL_BRANCH` code
/// reported (`E_` as the error, `W_` as an advisory), or `None` when the case
/// keeps another diagnostic or none.
const NIL_BRANCH_CONTRACT_CASES: &[(&str, bool, bool, Option<&str>)] = &[
  ("ret-if-noelse", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-if-niljoin", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("ret-when", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
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
  ("arg-defn-when", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
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
  ("map-callback", true, true, None),
  ("hof-typed-fn-arg", true, true, None),
  ("assert-type", true, true, None),
  ("let-bound-ret", true, true, Some("W_IMPLICIT_NIL_BRANCH")),
  ("let-bound-arg-defn", true, true, Some("W_IMPLICIT_NIL_BRANCH")),
  ("let-bound-ok-arg", true, true, Some("W_IMPLICIT_NIL_BRANCH")),
  ("let-bound-fn-literal", true, true, None),
  ("let-bound-list", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("let-cross-def", false, false, None),
  ("let-cross-def2", false, false, None),
  ("direct-cross-def", false, false, None),
  ("let-bound-nilq", false, false, Some("W_IMPLICIT_NIL_BRANCH")),
  ("let-bound-plus", true, true, Some("W_IMPLICIT_NIL_BRANCH")),
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
  ("map-callback-ok", true, true, None),
  ("hof-ok", true, true, None),
  ("qfield-nilq", false, false, None),
  ("qfield-take-str", true, true, None),
  ("qfield-let-take-str", true, true, None),
  ("qfield-nil-option", false, false, None),
  ("qfield-js-present", true, true, None),
  ("qfield-js-nullish-option", true, true, None),
  ("qfield-str-concat", false, false, None),
  ("qfield-ret-opt", false, false, None),
  ("m-slice-ok", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("m-slice-bad", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("m-slice-ok2", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("local-ok", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("local-ret-typed-fn-literal", true, true, Some("E_IMPLICIT_NIL_BRANCH")),
  ("n3-jsnullish-ret", false, false, None),
  ("n3-jsnullish-ret-join", false, false, None),
  ("n3-jsnullish-arg", false, false, None),
  ("n3-dynamic-ret", false, false, None),
  ("n3-let-value", false, false, Some("W_IMPLICIT_NIL_BRANCH")),
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

    // The erasure is advisory for now, so it must not block the check.
    let warnings = warnings.borrow();
    assert!(
      warnings.iter().all(|warning| warning.code() != Some("W_OPTION_ERASED_TO_NIL")),
      "{warnings:?}"
    );
    let advisories = runner::preprocess::take_advisories();
    let erased = advisories
      .iter()
      .filter(|advisory| advisory.code() == Some("W_OPTION_ERASED_TO_NIL"))
      .collect::<Vec<_>>();
    // The method in both call forms, the direct call and the `(:none) nil` branch report; keeping
    // the Option, choosing a concrete default or declaring a `JsNullish`
    // return for a host does not.
    assert_eq!(erased.len(), 4, "{advisories:?}");
    assert!(erased[0].message().contains("back into nil"), "{erased:?}");
  });
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
      .filter(|advisory| advisory.code() == Some("W_OPTION_ERASED_TO_NIL"))
      .map(|advisory| advisory.location().def.to_string())
      .collect::<Vec<_>>();
    erased.sort();
    // A `js/` call, native and external-object method calls (prefix and
    // postfix), a host-property `set!`, an argument for a `JsNullish`
    // parameter and a `(:none) nil` branch that is the host argument stay
    // quiet; the `:js-ffi` feature alone does not exempt a nil tested by
    // business code, bound to a local, `set!` on a local or passed to an
    // ordinary parameter.
    assert_eq!(
      erased,
      ["bound-in-ffi", "business-in-ffi", "local-set-in-ffi", "to-plain-param"],
      "{advisories:?}"
    );
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
      .expect("an Option erasure inside a nullish-returning function should remain advisory");

    let advisories = runner::preprocess::take_advisories();
    let erased = advisories
      .iter()
      .filter(|advisory| advisory.code() == Some("W_OPTION_ERASED_TO_NIL"))
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

/// #1554: legacy `Optional<T>` outside a public function schema is only noted
/// (`W_LEGACY_OPTIONAL_TYPE`) before its removal in 0.32.0. Struct fields, enum
/// payloads, a `def` value schema, a local `hint-fn`, `assert-type` and a
/// decoder target are each reported once; the strict check still passes and
/// the `Option<T>` forms stay quiet.
#[test]
fn legacy_optional_outside_public_schemas_is_only_noted() {
  run_with_large_stack(|| {
    let _strict = StrictTypesReset::enabled();
    let entries = load_fixture_entries("tests/fixtures/legacy-optional-advisory.cirru");
    let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
    let _ = runner::preprocess::take_advisories();

    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .expect("legacy Optional positions must not fail the strict check");

    let warnings = warnings.into_inner();
    assert!(
      warnings
        .iter()
        .all(|warning| !warning.code().is_some_and(|code| code.contains("LEGACY_OPTIONAL"))),
      "the advisory must never become a warning: {warnings:?}"
    );
    let advisories = runner::preprocess::take_advisories();
    let noted = advisories
      .iter()
      .filter(|advisory| advisory.code() == Some("W_LEGACY_OPTIONAL_TYPE"))
      .filter(|advisory| advisory.location().ns.as_ref() == "legacy-optional-advisory.main")
      .collect::<Vec<_>>();
    let mut positions = noted.iter().map(|advisory| advisory.location().def.to_string()).collect::<Vec<_>>();
    positions.sort();
    assert_eq!(
      positions,
      ["Box", "Choice", "fallback", "use-assert-type", "use-decoder", "use-hint-fn"],
      "{advisories:?}"
    );
    for (def, position) in [
      ("Box", "struct field `value`"),
      ("Choice", "enum variant `maybe` payload"),
      ("fallback", "the value schema"),
      ("use-hint-fn", "a hint-fn schema"),
      ("use-assert-type", "an assert-type target"),
      ("use-decoder", "a parse-cirru-edn-as target"),
    ] {
      let advisory = noted
        .iter()
        .find(|advisory| advisory.location().def.as_ref() == def)
        .expect("position is noted");
      assert!(
        advisory.message().contains(position) && advisory.message().contains("removed in 0.32.0"),
        "{def}: {}",
        advisory.message()
      );
    }
  });
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
      .filter(|warning| warning.code() == Some("W_CONSTANT_TRUTHY_CONDITION"))
      .collect::<Vec<_>>();
    // `if (count xs)`, `if s`, `when (count xs)`, `and (count xs) …`, and
    // `or s …`, and a hand-written `if s s false`; the Bool tests and the
    // value-returning `and` tail stay quiet.
    assert_eq!(truthy.len(), 6, "expected six constant-truthy conditions, got: {warnings:?}");
    assert_eq!(
      truthy.iter().filter(|warning| warning.message().contains("type Number")).count(),
      3,
      "{truthy:?}"
    );
    assert_eq!(
      truthy
        .iter()
        .filter(|warning| warning.message().contains("type String") && warning.message().contains("`blank?`"))
        .count(),
      3,
      "{truthy:?}"
    );
    assert!(
      truthy.iter().all(|warning| warning.message().contains("always taken")),
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
        warnings.iter().any(|warning| warning.code() == Some("W_PROC_ARG_TYPE_MISMATCH")
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
      "&enum:nth (%:: (defenum E (:a 'Number)) :a 1) 1",
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
      // Without its quote a type application reads as a call; report it instead of widening to Dynamic.
      ("assert-type ([] |a) (List String)", "write `quote $ List String`"),
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
  });
}
