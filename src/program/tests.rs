use super::*;
use crate::calcit::{CalcitImport, ImportInfo};
use crate::data::cirru::code_to_calcit;
use std::sync::{LazyLock, Mutex};

static PROGRAM_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn lock_program_test_state() -> std::sync::MutexGuard<'static, ()> {
  PROGRAM_TEST_LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

fn reset_program_test_state() {
  PROGRAM_RUNTIME_DATA_STATE.write().expect("reset runtime data").clear();
  PROGRAM_COMPILED_DATA_STATE.write().expect("reset compiled data").clear();
  PROGRAM_CODE_DATA.write().expect("reset program code").clear();
  *PROGRAM_DEF_ID_INDEX.write().expect("reset def id index") = ProgramDefIdIndex::default();
}

fn compiled_def_for_test(def_id: DefId, deps: Vec<DefId>) -> CompiledDef {
  CompiledDef {
    def_id,
    version_id: 0,
    kind: CompiledDefKind::Value,
    preprocessed_code: Calcit::Nil,
    codegen_form: Calcit::Nil,
    deps,
    type_summary: None,
    source_code: None,
    schema: DYNAMIC_TYPE.clone(),
    doc: Arc::from(""),
    examples: vec![],
  }
}

#[test]
fn reload_invalidation_collects_transitive_dependents() {
  let mut compiled: ProgramCompiledData = HashMap::new();
  compiled.insert(
    Arc::from("app.main"),
    CompiledFileData {
      defs: HashMap::from([
        (Arc::from("a"), compiled_def_for_test(DefId(1), vec![])),
        (Arc::from("b"), compiled_def_for_test(DefId(2), vec![DefId(1)])),
        (Arc::from("c"), compiled_def_for_test(DefId(3), vec![DefId(2)])),
        (Arc::from("d"), compiled_def_for_test(DefId(4), vec![])),
      ]),
    },
  );

  let mut index = ProgramDefIdIndex::default();
  index.by_ns.insert(
    Arc::from("app.main"),
    HashMap::from([
      (Arc::from("a"), DefId(1)),
      (Arc::from("b"), DefId(2)),
      (Arc::from("c"), DefId(3)),
      (Arc::from("d"), DefId(4)),
    ]),
  );

  let mut changes = snapshot::ChangesDict::default();
  changes.changed.insert(
    Arc::from("app.main"),
    snapshot::FileChangeInfo {
      ns: None,
      added_defs: HashMap::new(),
      removed_defs: HashSet::new(),
      changed_defs: HashMap::from([(String::from("a"), Cirru::Leaf(Arc::from("1")))]),
    },
  );

  let affected = collect_reload_affected_def_ids(&changes, &compiled, &index);
  assert_eq!(affected, HashSet::from([DefId(1), DefId(2), DefId(3)]));
}

#[test]
fn reload_invalidation_expands_namespace_header_changes() {
  let compiled: ProgramCompiledData = HashMap::from([
    (
      Arc::from("app.main"),
      CompiledFileData {
        defs: HashMap::from([
          (Arc::from("a"), compiled_def_for_test(DefId(1), vec![])),
          (Arc::from("b"), compiled_def_for_test(DefId(2), vec![])),
        ]),
      },
    ),
    (
      Arc::from("app.consumer"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("use-main"), compiled_def_for_test(DefId(3), vec![DefId(2)]))]),
      },
    ),
  ]);

  let mut index = ProgramDefIdIndex::default();
  index.by_ns.insert(
    Arc::from("app.main"),
    HashMap::from([(Arc::from("a"), DefId(1)), (Arc::from("b"), DefId(2))]),
  );
  index
    .by_ns
    .insert(Arc::from("app.consumer"), HashMap::from([(Arc::from("use-main"), DefId(3))]));

  let mut changes = snapshot::ChangesDict::default();
  changes.changed.insert(
    Arc::from("app.main"),
    snapshot::FileChangeInfo {
      ns: Some(Cirru::Leaf(Arc::from("ns"))),
      added_defs: HashMap::new(),
      removed_defs: HashSet::new(),
      changed_defs: HashMap::new(),
    },
  );

  let affected = collect_reload_affected_def_ids(&changes, &compiled, &index);
  assert_eq!(affected, HashSet::from([DefId(1), DefId(2), DefId(3)]));
}

#[test]
fn snapshot_fallback_preserves_dependency_metadata() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let dep_id = register_program_def_id("dep.ns", "value");
  let _ = register_program_def_id("app.main", "dep");

  let runtime_value = Calcit::from(vec![Calcit::Import(CalcitImport {
    ns: Arc::from("dep.ns"),
    def: Arc::from("value"),
    info: Arc::new(ImportInfo::SameFile { at_def: Arc::from("dep") }),
    def_id: Some(dep_id.0),
  })]);

  let fallback = build_runtime_only_snapshot_fallback_compiled_def("app.main", "dep", runtime_value);
  assert_eq!(fallback.deps, vec![dep_id]);
}

#[test]
fn write_runtime_ready_normalizes_thunk_into_lazy_cell() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let thunk_ns = "tests.runtime";
  let thunk_def = "lazy-demo";
  let thunk_code = Arc::new(Calcit::Nil);
  let thunk_info = Arc::new(CalcitThunkInfo {
    ns: Arc::from(thunk_ns),
    def: Arc::from(thunk_def),
  });

  write_runtime_ready(
    thunk_ns,
    thunk_def,
    Calcit::Thunk(CalcitThunk::Code {
      code: thunk_code.clone(),
      info: thunk_info.clone(),
    }),
  )
  .expect("write thunk into runtime");

  match lookup_runtime_cell(thunk_ns, thunk_def) {
    Some(RuntimeCell::Lazy { code, info }) => {
      assert_eq!(code, thunk_code);
      assert_eq!(info, thunk_info);
    }
    other => panic!("expected lazy runtime cell, got {other:?}"),
  }
}

#[test]
fn clear_runtime_caches_for_changes_clears_transitive_dependents() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let def_a = ensure_def_id("app.main", "a");
  let def_b = ensure_def_id("app.main", "b");
  let def_c = ensure_def_id("app.main", "c");
  let def_d = ensure_def_id("app.main", "d");

  {
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("seed compiled data");
    compiled.insert(
      Arc::from("app.main"),
      CompiledFileData {
        defs: HashMap::from([
          (Arc::from("a"), compiled_def_for_test(def_a, vec![])),
          (Arc::from("b"), compiled_def_for_test(def_b, vec![def_a])),
          (Arc::from("c"), compiled_def_for_test(def_c, vec![def_b])),
          (Arc::from("d"), compiled_def_for_test(def_d, vec![])),
        ]),
      },
    );
  }

  write_runtime_ready("app.main", "a", Calcit::Number(1.0)).expect("seed runtime a");
  write_runtime_ready("app.main", "b", Calcit::Number(2.0)).expect("seed runtime b");
  write_runtime_ready("app.main", "c", Calcit::Number(3.0)).expect("seed runtime c");
  write_runtime_ready("app.main", "d", Calcit::Number(4.0)).expect("seed runtime d");

  let mut changes = snapshot::ChangesDict::default();
  changes.changed.insert(
    Arc::from("app.main"),
    snapshot::FileChangeInfo {
      ns: None,
      added_defs: HashMap::new(),
      removed_defs: HashSet::new(),
      changed_defs: HashMap::from([(String::from("a"), Cirru::Leaf(Arc::from("1")))]),
    },
  );

  clear_runtime_caches_for_changes(&changes, false).expect("clear runtime caches for changes");

  assert_eq!(lookup_runtime_cell("app.main", "a"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_runtime_cell("app.main", "b"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_runtime_cell("app.main", "c"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_runtime_ready("app.main", "d"), Some(Calcit::Number(4.0)));

  let compiled = PROGRAM_COMPILED_DATA_STATE.read().expect("read compiled data");
  let compiled_file = compiled.get("app.main").expect("compiled file should remain for unaffected defs");
  assert!(!compiled_file.defs.contains_key("a"));
  assert!(!compiled_file.defs.contains_key("b"));
  assert!(!compiled_file.defs.contains_key("c"));
  assert!(compiled_file.defs.contains_key("d"));
}

#[test]
fn clear_runtime_caches_for_changes_expands_namespace_header_invalidation() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let main_a = ensure_def_id("app.main", "a");
  let main_b = ensure_def_id("app.main", "b");
  let consumer_use = ensure_def_id("app.consumer", "use-main");
  let helper_keep = ensure_def_id("app.helper", "keep");

  {
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("seed compiled data");
    compiled.insert(
      Arc::from("app.main"),
      CompiledFileData {
        defs: HashMap::from([
          (Arc::from("a"), compiled_def_for_test(main_a, vec![])),
          (Arc::from("b"), compiled_def_for_test(main_b, vec![])),
        ]),
      },
    );
    compiled.insert(
      Arc::from("app.consumer"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("use-main"), compiled_def_for_test(consumer_use, vec![main_b]))]),
      },
    );
    compiled.insert(
      Arc::from("app.helper"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("keep"), compiled_def_for_test(helper_keep, vec![]))]),
      },
    );
  }

  write_runtime_ready("app.main", "a", Calcit::Number(1.0)).expect("seed runtime main/a");
  write_runtime_ready("app.main", "b", Calcit::Number(2.0)).expect("seed runtime main/b");
  write_runtime_ready("app.consumer", "use-main", Calcit::Number(3.0)).expect("seed runtime consumer/use-main");
  write_runtime_ready("app.helper", "keep", Calcit::Number(9.0)).expect("seed runtime helper/keep");

  let mut changes = snapshot::ChangesDict::default();
  changes.changed.insert(
    Arc::from("app.main"),
    snapshot::FileChangeInfo {
      ns: Some(Cirru::Leaf(Arc::from("ns"))),
      added_defs: HashMap::new(),
      removed_defs: HashSet::new(),
      changed_defs: HashMap::new(),
    },
  );

  clear_runtime_caches_for_changes(&changes, false).expect("clear runtime caches for namespace header change");

  assert_eq!(lookup_runtime_cell("app.main", "a"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_runtime_cell("app.main", "b"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_runtime_cell("app.consumer", "use-main"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_runtime_ready("app.helper", "keep"), Some(Calcit::Number(9.0)));

  let compiled = PROGRAM_COMPILED_DATA_STATE.read().expect("read compiled data");
  assert!(!compiled.get("app.main").is_some_and(|file| file.defs.contains_key("a")));
  assert!(!compiled.get("app.main").is_some_and(|file| file.defs.contains_key("b")));
  assert!(!compiled.get("app.consumer").is_some_and(|file| file.defs.contains_key("use-main")));
  assert!(compiled.get("app.helper").is_some_and(|file| file.defs.contains_key("keep")));
}

#[test]
fn snapshot_prefers_source_backed_compiled_def_even_with_warnings() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let warn_code =
    code_to_calcit(&Cirru::Leaf(Arc::from("missing-symbol")), "app.warn", "warny", vec![]).expect("build source-backed code");

  PROGRAM_CODE_DATA.write().expect("seed program code").insert(
    Arc::from("app.warn"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::from([(
        Arc::from("warny"),
        ProgramDefEntry {
          code: warn_code.clone(),
          schema: DYNAMIC_TYPE.clone(),
          doc: Arc::from(""),
          examples: vec![],
        },
      )]),
    },
  );
  let _ = ensure_def_id("app.warn", "warny");

  write_runtime_ready("app.warn", "warny", Calcit::Number(42.0)).expect("seed runtime fallback value");

  let snapshot = clone_compiled_program_snapshot().expect("clone compiled snapshot");
  let compiled = snapshot
    .get("app.warn")
    .and_then(|file| file.defs.get("warny"))
    .expect("snapshot should include source-backed compiled def");

  assert_eq!(compiled.kind, CompiledDefKind::LazyValue);
  assert_eq!(compiled.codegen_form, warn_code);
  assert_eq!(compiled.source_code, Some(compiled.codegen_form.clone()));
}

#[test]
fn runtime_snapshot_fallback_only_allows_runtime_only_defs() {
  let runtime_only = SnapshotFillTask {
    ns: Arc::from("app.runtime"),
    def: Arc::from("demo"),
    source_backed: false,
    runtime_value: Some(Calcit::Number(42.0)),
  };
  assert!(should_use_runtime_snapshot_fallback(&runtime_only));

  let source_backed = SnapshotFillTask {
    ns: Arc::from("app.source"),
    def: Arc::from("demo"),
    source_backed: true,
    runtime_value: Some(Calcit::Number(42.0)),
  };
  assert!(!should_use_runtime_snapshot_fallback(&source_backed));
}

#[test]
fn lookup_codegen_type_hint_prefers_compiled_schema_over_runtime_value() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let schema = Arc::new(CalcitTypeAnnotation::String);
  store_compiled_output(
    "app.codegen",
    "typed",
    CompiledDefPayload {
      version_id: 0,
      preprocessed_code: Calcit::Nil,
      codegen_form: Calcit::Nil,
      deps: vec![],
      type_summary: None,
      source_code: None,
      schema: schema.clone(),
      doc: Arc::from(""),
      examples: vec![],
    },
  );
  write_runtime_ready("app.codegen", "typed", Calcit::Number(42.0)).expect("seed runtime value");

  let hint = lookup_codegen_type_hint("app.codegen", "typed").expect("lookup codegen type hint");
  assert!(matches!(hint.as_ref(), CalcitTypeAnnotation::String));
}

#[test]
fn lookup_codegen_type_hint_falls_back_to_runtime_value() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let _ = ensure_def_id("app.codegen", "runtime-only");
  write_runtime_ready("app.codegen", "runtime-only", Calcit::Number(42.0)).expect("seed runtime value");

  let hint = lookup_codegen_type_hint("app.codegen", "runtime-only").expect("lookup runtime fallback type hint");
  assert!(matches!(hint.as_ref(), CalcitTypeAnnotation::Number));
}

#[test]
fn compiled_executable_code_only_exposes_executable_kinds() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  store_compiled_output(
    "app.compiled",
    "callable",
    CompiledDefPayload {
      version_id: 0,
      preprocessed_code: Calcit::Number(1.0),
      codegen_form: Calcit::Nil,
      deps: vec![],
      type_summary: None,
      source_code: None,
      schema: DYNAMIC_TYPE.clone(),
      doc: Arc::from(""),
      examples: vec![],
    },
  );

  {
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("adjust compiled kind");
    compiled
      .get_mut("app.compiled")
      .and_then(|file| file.defs.get_mut("callable"))
      .expect("compiled callable")
      .kind = CompiledDefKind::Fn;
  }

  assert_eq!(
    lookup_compiled_executable_code("app.compiled", "callable"),
    Some(Calcit::Number(1.0))
  );

  {
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("adjust compiled kind");
    compiled
      .get_mut("app.compiled")
      .and_then(|file| file.defs.get_mut("callable"))
      .expect("compiled callable")
      .kind = CompiledDefKind::LazyValue;
  }

  assert_eq!(lookup_compiled_executable_code("app.compiled", "callable"), None);
}
