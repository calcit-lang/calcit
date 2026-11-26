use super::*;
use crate::calcit::data_shape::{DataShapeGraph, DataShapeNode};
use crate::calcit::{CalcitImpl, CalcitImport, CalcitStructDef, CalcitSyntax, ImportInfo};
use crate::call_stack::CallStackList;
use crate::data::cirru::code_to_calcit;
use crate::run_program_with_docs;
use cirru_edn::EdnTag;
use std::collections::HashSet;

fn cirru_leaf(value: &str) -> Cirru {
  Cirru::Leaf(value.into())
}

fn cirru_list(items: Vec<Cirru>) -> Cirru {
  Cirru::List(items)
}

fn import_rule(source: &str, kind: &str, target: Cirru) -> Cirru {
  cirru_list(vec![cirru_leaf(source), cirru_leaf(kind), target])
}

#[test]
fn bundled_core_initialization_releases_program_lock_for_schema_lookups() {
  use std::process::{Command, Stdio};
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::time::{Duration, Instant};

  const CHILD: &str = "CALCIT_CORE_INITIALIZATION_LOCK_CHILD";
  static LOOKUPS: AtomicUsize = AtomicUsize::new(0);

  fn audited_lookup(ns: &str, def: &str) -> Option<Calcit> {
    // A queued writer must not be blocked by the initializer's own read lock.
    // try_write makes the old lock order fail immediately, without a hang.
    drop(
      PROGRAM_CODE_DATA
        .try_write()
        .expect("core schema initialization must not hold the program lock"),
    );
    LOOKUPS.fetch_add(1, Ordering::Relaxed);
    lookup_def_code(ns, def)
  }

  if std::env::var_os(CHILD).is_some() {
    calcit::set_def_code_lookup_hook_for_test(audited_lookup);
    assert!(!has_bundled_core_source(), "an empty program is not the bundled core");
    assert!(
      LOOKUPS.load(Ordering::Relaxed) > 0,
      "the cold initializer must exercise schema lookups"
    );

    let snapshot = crate::load_core_snapshot().expect("embedded core snapshot");
    let source = snapshot
      .files
      .iter()
      .map(|(ns, file)| {
        let name: Arc<str> = Arc::from(ns.as_str());
        (name.clone(), extract_file_data(file, name).expect("embedded source"))
      })
      .collect();
    *PROGRAM_CODE_DATA.write().expect("seed bundled source") = source;
    assert!(has_bundled_core_source(), "initialization order must not change the source proof");
    assert!(has_bundled_core_source(), "a repeated check reuses the verified source");
    PROGRAM_CODE_DATA
      .write()
      .expect("replace a core predicate")
      .get_mut(calcit::CORE_NS)
      .expect("core namespace")
      .defs
      .get_mut("nil?")
      .expect("core predicate")
      .code = Calcit::Nil;
    assert!(!has_bundled_core_source(), "a replaced predicate must still lose its builtin proof");
    return;
  }

  // A fresh process guarantees a cold LazyLock and isolates lookup callbacks
  // from other tests. Bound it so a future lock regression cannot stall CI.
  let mut child = Command::new(std::env::current_exe().expect("unit test executable"))
    .args([
      "--exact",
      "program::tests::bundled_core_initialization_releases_program_lock_for_schema_lookups",
      "--nocapture",
    ])
    .env(CHILD, "1")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("spawn cold core initialization test");
  let deadline = Instant::now() + Duration::from_secs(30);
  loop {
    if child.try_wait().expect("poll cold core initialization test").is_some() {
      break;
    }
    if Instant::now() >= deadline {
      child.kill().expect("stop stalled core initialization test");
      let output = child.wait_with_output().expect("collect stalled child");
      panic!("cold core initialization timed out: {}", String::from_utf8_lossy(&output.stderr));
    }
    std::thread::sleep(Duration::from_millis(10));
  }
  let output = child.wait_with_output().expect("collect cold core initialization test");
  assert!(
    output.status.success(),
    "cold core initialization failed:\n{}\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn strict_edn_decoder_nominals_are_compiled_dependencies() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let ns: Arc<str> = Arc::from("tests.strict-edn-dependencies");
  let def: Arc<str> = Arc::from("Person");
  let dep_id = ensure_def_id(&ns, &def);
  let graph = DataShapeGraph::from_nodes(
    0,
    vec![DataShapeNode::Struct {
      nominal: Arc::new(CalcitStructDef::from_fields(EdnTag::new("Person"), vec![])),
      nominal_path: Some((ns, def)),
      type_args: Arc::from([]),
      fields: vec![],
    }],
  )
  .expect("valid test data shape");
  let code = Calcit::from(vec![
    Calcit::Syntax(CalcitSyntax::ParseCirruEdnAs, Arc::from(calcit::CORE_NS)),
    Calcit::Str(Arc::from("%{} :Person")),
    Calcit::tag("Person"),
    graph.into_calcit_handle(),
  ]);

  assert_eq!(collect_compiled_deps(&code), vec![dep_id]);
}

#[test]
fn runtime_value_provenance_resolves_only_source_backed_definitions() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let target = Calcit::Str(Arc::from("impl-metadata-value"));
  PROGRAM_CODE_DATA.write().expect("seed source program").insert(
    Arc::from("tests.runtime-provenance"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::from([
        (
          Arc::from("z-owner"),
          ProgramDefEntry {
            tags: Default::default(),
            code: Calcit::Nil,
            schema: DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: None,
          },
        ),
        (
          Arc::from("a-owner"),
          ProgramDefEntry {
            tags: Default::default(),
            code: Calcit::Nil,
            schema: DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: None,
          },
        ),
      ]),
    },
  );
  write_runtime_ready("tests.runtime-provenance", "runtime-only", target.clone()).expect("seed runtime-only value");
  write_runtime_ready("tests.runtime-provenance", "z-owner", target.clone()).expect("seed source-backed value");
  write_runtime_ready("tests.runtime-provenance", "a-owner", target.clone()).expect("seed second source-backed value");

  assert_eq!(
    find_source_def_for_runtime_value("tests.runtime-provenance", &target).as_deref(),
    Some("a-owner"),
    "provenance is source-backed and deterministic"
  );
  let tagged_impl = Calcit::Impl(Arc::new(CalcitImpl {
    name: EdnTag::new("z-owner"),
    origin: None,
    fields: Arc::new(vec![]),
    values: Arc::new(vec![]),
  }));
  assert_eq!(
    find_source_def_for_runtime_value("tests.runtime-provenance", &tagged_impl).as_deref(),
    Some("z-owner"),
    "an impl tag identifies its source owner even before that owner is Ready"
  );
  assert_eq!(find_source_def_for_runtime_value("tests.missing", &target), None);

  let origin = Arc::new(crate::calcit::CalcitTrait::new(EdnTag::new("z-owner"), vec![], vec![]));
  let trait_impl = Calcit::Impl(Arc::new(CalcitImpl {
    name: origin.name.clone(),
    origin: Some(origin.clone()),
    fields: Arc::new(vec![]),
    values: Arc::new(vec![]),
  }));
  write_runtime_ready("tests.runtime-provenance", "z-owner", Calcit::Trait(origin)).expect("seed trait owner");
  assert_eq!(
    find_source_def_for_runtime_value("tests.runtime-provenance", &trait_impl),
    None,
    "a trait tag is not the identity of an impl that has no ready source owner"
  );
  write_runtime_ready("tests.runtime-provenance", "a-owner", trait_impl.clone()).expect("seed actual impl owner");
  assert_eq!(
    find_source_def_for_runtime_value("tests.runtime-provenance", &trait_impl).as_deref(),
    Some("a-owner"),
    "trait-backed implementations must use their own source identity"
  );
}

#[test]
fn nominal_impl_wrapper_type_ref_matches_its_concrete_enum_value() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let wrapper_code = code_to_calcit(
    &cirru_list(vec![
      cirru_leaf("def"),
      cirru_leaf("alert-actions-plugin"),
      cirru_list(vec![
        cirru_leaf("impl-traits"),
        cirru_list(vec![
          cirru_leaf("defenum"),
          cirru_leaf("PluginNode"),
          cirru_list(vec![cirru_leaf(":value"), cirru_leaf("Dynamic")]),
        ]),
        cirru_leaf("PluginActions"),
      ]),
    ]),
    "tests.nominal-wrapper",
    "alert-actions-plugin",
    vec![],
  )
  .expect("build nominal impl wrapper source");

  PROGRAM_CODE_DATA.write().expect("seed nominal wrapper source").insert(
    Arc::from("tests.nominal-wrapper"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::from([(
        Arc::from("alert-actions-plugin"),
        ProgramDefEntry {
          tags: Default::default(),
          code: wrapper_code,
          schema: DYNAMIC_TYPE.clone(),
          doc: Arc::from(""),
          examples: vec![],
          ffi: None,
        },
      )]),
    },
  );

  let wrapper_ref = CalcitTypeAnnotation::TypeRef(Arc::from("tests.nominal-wrapper/alert-actions-plugin"), Arc::new(vec![]));
  let concrete = CalcitTypeAnnotation::Enum(
    Arc::new(wrapper_ref.resolve_to_enum().expect("resolve wrapper to its underlying enum")),
    Arc::new(vec![]),
  );

  assert!(wrapper_ref.is_compatible_with(&concrete));
  assert!(concrete.is_compatible_with(&wrapper_ref));
}

#[test]
fn recursive_type_alias_relation_is_finite_and_incompatible() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let alias = |target: &str| ProgramDefEntry {
    tags: Default::default(),
    code: Calcit::Nil,
    schema: Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from(target), Arc::new(vec![]))),
    doc: Arc::from(""),
    examples: vec![],
    ffi: None,
  };
  PROGRAM_CODE_DATA.write().expect("seed recursive aliases").insert(
    Arc::from("tests.alias-cycle"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::from([
        (Arc::from("Left"), alias("tests.alias-cycle/Right")),
        (Arc::from("Right"), alias("tests.alias-cycle/Left")),
      ]),
    },
  );

  let recursive = CalcitTypeAnnotation::TypeRef(Arc::from("tests.alias-cycle/Left"), Arc::new(vec![]));
  assert!(!recursive.is_compatible_with(&CalcitTypeAnnotation::Number));
  reset_program_test_state();
}

#[test]
fn import_rule_validation_rejects_short_rules_without_panicking() {
  let error =
    validate_import_rules(&[cirru_list(vec![cirru_leaf("audit.invalid")])]).expect_err("short import rule should be rejected");

  assert!(error.contains("exactly 3 items"), "error: {error}");
  assert!(error.contains("import rule 1"), "error: {error}");
}

#[test]
fn import_rule_validation_reports_unknown_rule_kind() {
  let error = validate_import_rules(&[import_rule("audit.lib", ":rename", cirru_leaf("lib"))])
    .expect_err("unknown import rule kind should be rejected");

  assert!(error.contains("unknown import rule kind `:rename`"), "error: {error}");
}

#[test]
fn import_rule_validation_warns_for_duplicate_local_bindings() {
  let rules = [
    import_rule("audit.one", ":as", cirru_leaf("shared")),
    import_rule("audit.two", ":as", cirru_leaf("shared")),
  ];
  let warnings = validate_import_rules(&rules).expect("duplicate local bindings should remain executable");

  assert_eq!(warnings.len(), 1);
  assert!(
    warnings[0].contains("duplicate import binding `shared`"),
    "warning: {}",
    warnings[0]
  );
  assert!(warnings[0].contains("rule 2 takes precedence"), "warning: {}", warnings[0]);
}

#[test]
fn import_rule_validation_warns_for_duplicate_refer_within_one_rule() {
  let rules = [import_rule(
    "audit.math",
    ":refer",
    cirru_list(vec![cirru_leaf("add"), cirru_leaf("add")]),
  )];
  let warnings = validate_import_rules(&rules).expect("duplicate refer in one rule should remain executable");

  assert_eq!(warnings.len(), 1);
  assert!(
    warnings[0].contains("duplicate import binding `add` within rule 1"),
    "warning: {}",
    warnings[0]
  );
  assert!(warnings[0].contains("takes precedence"), "warning: {}", warnings[0]);
}

#[test]
fn import_rule_validation_accepts_supported_rule_kinds() {
  let rules = [
    import_rule("audit.lib", ":as", cirru_leaf("lib")),
    import_rule("audit.math", ":refer", cirru_list(vec![cirru_leaf("add"), cirru_leaf("subtract")])),
    import_rule("|chalk", ":default", cirru_leaf("chalk")),
  ];

  assert!(
    validate_import_rules(&rules)
      .expect("supported import rules should pass validation")
      .is_empty()
  );
}

#[test]
fn redundant_import_markers_warn_without_changing_resolution() {
  // Namespace syntax is parsed before definition tests; verify the parser boundary here.
  let canonical = [
    import_rule("audit.math", ":refer", cirru_list(vec![cirru_leaf("add"), cirru_leaf("subtract")])),
    import_rule("audit.other", ":as", cirru_leaf("other")),
    import_rule("|chalk", ":default", cirru_leaf("chalk")),
  ];
  let legacy = [
    cirru_list(vec![
      cirru_leaf("[]"),
      cirru_leaf("audit.math"),
      cirru_leaf(":refer"),
      cirru_list(vec![cirru_leaf("[]"), cirru_leaf("add"), cirru_leaf("[]"), cirru_leaf("subtract")]),
    ]),
    cirru_list(vec![
      cirru_leaf("[]"),
      cirru_leaf("audit.other"),
      cirru_leaf(":as"),
      cirru_leaf("other"),
    ]),
    cirru_list(vec![
      cirru_leaf("[]"),
      cirru_leaf("|chalk"),
      cirru_leaf(":default"),
      cirru_leaf("chalk"),
    ]),
  ];
  assert!(validate_import_rules(&canonical).unwrap().is_empty());
  let warnings = validate_import_rules(&legacy).unwrap();
  assert_eq!(warnings.len(), 1, "coalesce syntax advice rather than flooding each namespace");
  assert!(warnings[0].contains("rule 1 prefix"));
  assert!(warnings[0].contains("rule 1 :refer list"));
  assert!(warnings[0].contains("rule 2 prefix"));
  assert!(warnings[0].contains("rule 3 prefix"));
  assert!(warnings[0].contains("respo.core :refer $ render! clear-cache!"));
  for (canonical, legacy) in canonical.iter().zip(&legacy) {
    assert_eq!(extract_import_rule(canonical).unwrap(), extract_import_rule(legacy).unwrap());
  }
  let comment = cirru_list(vec![cirru_leaf(";"), cirru_leaf("[]"), cirru_list(vec![cirru_leaf("[]")])]);
  assert!(validate_import_rules(&[comment]).unwrap().is_empty());
}

#[test]
fn duplicate_import_binding_uses_the_later_rule() {
  let first = import_rule("audit.one", ":as", cirru_leaf("shared"));
  let second = import_rule("audit.two", ":as", cirru_leaf("shared"));
  let ns_form = cirru_list(vec![
    cirru_leaf("ns"),
    cirru_leaf("app.main"),
    cirru_list(vec![cirru_leaf(":require"), first, second]),
  ]);

  let imports = extract_import_map(&ns_form, "app.main").expect("duplicate imports should remain executable");
  assert_eq!(
    imports.get("shared").map(|rule| rule.as_ref()),
    Some(&ImportRule::NsAs(Arc::from("audit.two")))
  );
}

#[test]
fn namespace_validation_rejects_leaf_without_panicking() {
  let error = extract_import_map(&cirru_leaf("app.main"), "app.main").expect_err("leaf namespace form should be rejected");

  assert!(error.contains("invalid ns form in 'app.main'"), "error: {error}");
}

#[test]
fn namespace_validation_rejects_unknown_clause() {
  let ns_form = cirru_list(vec![
    cirru_leaf("ns"),
    cirru_leaf("app.main"),
    cirru_list(vec![cirru_leaf(":unknown")]),
  ]);
  let error = extract_import_map(&ns_form, "app.main").expect_err("unknown namespace clause should be rejected");

  assert!(error.contains("expected `:require`"), "error: {error}");
}

#[test]
fn namespace_validation_explains_require_macros_migration() {
  let ns_form = cirru_list(vec![
    cirru_leaf("ns"),
    cirru_leaf("app.main"),
    cirru_list(vec![
      cirru_leaf(":require-macros"),
      import_rule("legacy.macros", ":refer", cirru_list(vec![cirru_leaf("defcomp")])),
    ]),
  ]);
  let error = extract_import_map(&ns_form, "app.main").expect_err("legacy macro imports should report a migration error");

  assert!(error.contains("legacy `:require-macros`"), "error: {error}");
  assert!(error.contains("ordinary `:require`"), "error: {error}");
  assert!(error.contains("macros and values now share import rules"), "error: {error}");
}

#[test]
fn namespace_validation_explains_legacy_colon_ns_migration() {
  let ns_form = cirru_list(vec![cirru_leaf(":ns"), cirru_leaf("app.main")]);

  let error = extract_import_map(&ns_form, "app.main").expect_err("legacy :ns form should be rejected");
  assert!(error.contains("legacy `:ns`"), "error: {error}");
  assert!(error.contains("`ns app.main`"), "error: {error}");

  let ns_form_with_imports = cirru_list(vec![
    cirru_leaf(":ns"),
    cirru_leaf("app.main"),
    cirru_list(vec![
      cirru_leaf(":require"),
      import_rule("foo.core", ":refer", cirru_list(vec![cirru_leaf("bar")])),
    ]),
  ]);
  let error = extract_import_map(&ns_form_with_imports, "app.main").expect_err("legacy :ns with imports should be rejected");
  assert!(error.contains("preserve its `:require` rules"), "error: {error}");
}

fn lock_program_test_state() -> ProgramTestStateGuard {
  super::lock_program_test_state()
}

fn reset_program_test_state() {
  PROGRAM_RUNTIME_DATA_STATE.write().expect("reset runtime data").clear();
  PROGRAM_COMPILED_DATA_STATE.write().expect("reset compiled data").clear();
  PROGRAM_CODE_DATA.write().expect("reset program code").clear();
  // Keep allocating fresh ids so values from before the reset cannot alias new definitions.
  let mut index = PROGRAM_DEF_ID_INDEX.write().expect("reset def id index");
  *index = ProgramDefIdIndex {
    next_id: index.next_id,
    ..ProgramDefIdIndex::default()
  };
}

#[test]
fn program_test_state_guard_never_reuses_rolled_back_def_ids() {
  // #2298: an Import keeps the def id it was preprocessed with. After a guard
  // rolls the registry back, that id must not resolve to a later definition.
  let ns = "tests.guard-rolled-back-def-id";
  let stale = {
    let _guard = lock_program_test_state();
    ensure_def_id(ns, "rolled-back")
  };
  let _guard = lock_program_test_state();
  assert_eq!(lookup_def_id(ns, "rolled-back"), None);
  reset_program_test_state();
  let fresh = ensure_def_id(ns, "fresh");
  assert_ne!(stale, fresh, "a rolled-back def id was reassigned");
  write_runtime_ready(ns, "fresh", Calcit::Number(1.0)).expect("seed fresh runtime value");
  assert_eq!(lookup_runtime_ready_by_id(stale), None);
}

#[test]
fn program_test_state_guard_restores_registry_after_panic() {
  let namespace: Arc<str> = Arc::from("tests.guard-panic-restoration");
  let result = std::panic::catch_unwind(|| {
    let _guard = lock_program_test_state();
    reset_program_test_state();
    PROGRAM_CODE_DATA.write().unwrap_or_else(|error| error.into_inner()).insert(
      namespace.clone(),
      ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::new(),
      },
    );
    panic!("exercise panic restoration");
  });
  assert!(result.is_err());

  let _guard = lock_program_test_state();
  assert!(
    !PROGRAM_CODE_DATA
      .read()
      .unwrap_or_else(|error| error.into_inner())
      .contains_key(namespace.as_ref())
  );
}

fn compiled_def_for_test(def_id: DefId, deps: Vec<DefId>) -> CompiledDef {
  CompiledDef {
    proven_guard: None,
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

  let fallback = build_runtime_only_snapshot_fallback_compiled_def("app.main", "dep", runtime_value)
    .expect("runtime-only fallback should serialize import-based value");
  assert_eq!(fallback.deps, vec![dep_id]);
}

#[test]
fn initializer_metadata_probe_defers_transitive_pending_dependencies() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let pending = ensure_def_id("tests.initializer", "pending");
  let helper = ensure_def_id("tests.initializer", "helper");
  let value = ensure_def_id("tests.initializer", "value");
  let unrelated = ensure_def_id("tests.initializer", "unrelated");
  let mut function = compiled_def_for_test(helper, vec![pending]);
  function.kind = CompiledDefKind::Fn;
  write_compiled_def("tests.initializer", "helper", function);
  let mut initializer = compiled_def_for_test(value, vec![helper]);
  initializer.kind = CompiledDefKind::LazyValue;
  write_compiled_def("tests.initializer", "value", initializer);

  assert!(compiled_initializer_has_pending_dependency(
    "tests.initializer",
    "value",
    &HashSet::from([pending])
  ));
  assert!(!compiled_initializer_has_pending_dependency(
    "tests.initializer",
    "helper",
    &HashSet::from([pending])
  ));
  assert!(!compiled_initializer_has_pending_dependency(
    "tests.initializer",
    "value",
    &HashSet::from([unrelated])
  ));
  assert!(!compiled_initializer_has_pending_dependency(
    "tests.initializer",
    "missing",
    &HashSet::from([pending])
  ));
  assert!(lookup_runtime_cell("tests.initializer", "value").is_none());

  // A completed recursive dependency graph must terminate without becoming pending.
  write_compiled_def("tests.initializer", "pending", compiled_def_for_test(pending, vec![value]));
  assert!(!compiled_initializer_has_pending_dependency(
    "tests.initializer",
    "value",
    &HashSet::new()
  ));
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
fn write_runtime_ready_attaches_trait_definition_identity() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let trait_value = crate::calcit::CalcitTrait::new_runtime(cirru_edn::EdnTag::new("Show"), vec![], vec![]);
  write_runtime_ready("app.traits", "Show", Calcit::Trait(Arc::new(trait_value))).expect("store runtime trait");

  let Calcit::Trait(stored) = lookup_runtime_ready("app.traits", "Show").expect("stored trait") else {
    panic!("expected stored trait");
  };
  assert_eq!(stored.definition_ref.as_deref(), Some("app.traits/Show"));
}

#[test]
fn write_runtime_ready_attaches_struct_and_enum_definition_identity() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let struct_value = crate::calcit::CalcitStructDef::from_fields(cirru_edn::EdnTag::new("User"), vec![]);
  write_runtime_ready("app.models", "User", Calcit::StructDef(Arc::new(struct_value))).expect("store runtime struct");
  let Calcit::StructDef(stored_struct) = lookup_runtime_ready("app.models", "User").expect("stored struct") else {
    panic!("expected stored struct");
  };
  assert_eq!(stored_struct.definition_ref.as_deref(), Some("app.models/User"));

  let enum_struct = crate::calcit::CalcitStructValue {
    struct_ref: Arc::new(crate::calcit::CalcitStructDef::from_fields(
      cirru_edn::EdnTag::new("Status"),
      vec![cirru_edn::EdnTag::new("ready")],
    )),
    values: Arc::new(vec![Calcit::Nil]),
  };
  let enum_value = crate::calcit::CalcitEnumDef::from_struct(enum_struct).expect("build runtime enum");
  write_runtime_ready("app.models", "Status", Calcit::EnumDef(Arc::new(enum_value))).expect("store runtime enum");
  let Calcit::EnumDef(stored_enum) = lookup_runtime_ready("app.models", "Status").expect("stored enum") else {
    panic!("expected stored enum");
  };
  assert_eq!(stored_enum.definition_ref().map(AsRef::as_ref), Some("app.models/Status"));
}

#[test]
fn clear_runtime_caches_for_changes_clears_transitive_dependents() {
  let _guard = lock_program_test_state();
  for entry_slot in [false, true] {
    reset_program_test_state();
    calcit::clear_type_slots();
    struct ClearSlots;
    impl Drop for ClearSlots {
      fn drop(&mut self) {
        calcit::clear_type_slots();
      }
    }
    let _slots = ClearSlots;
    if entry_slot {
      calcit::install_entry_type_slots(HashMap::from([(Arc::from("test-slot"), Arc::new(CalcitTypeAnnotation::Number))]));
      PROGRAM_CODE_DATA.write().unwrap().insert(
        Arc::from("app.main"),
        ProgramFileData {
          import_map: HashMap::new(),
          defs: HashMap::new(),
        },
      );
    }

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
        changed_defs: HashMap::from([(String::from("a"), Cirru::Leaf(Arc::from("1")).into())]),
      },
    );

    if entry_slot {
      apply_code_changes(&changes).expect("apply changes with entry slots");
    }
    clear_runtime_caches_for_changes(&changes, false).expect("clear runtime caches for changes");

    assert_eq!(lookup_runtime_cell("app.main", "a"), Some(RuntimeCell::Cold));
    assert_eq!(lookup_runtime_cell("app.main", "b"), Some(RuntimeCell::Cold));
    assert_eq!(lookup_runtime_cell("app.main", "c"), Some(RuntimeCell::Cold));
    assert_eq!(lookup_runtime_ready("app.main", "d"), Some(Calcit::Number(4.0)));

    let compiled = PROGRAM_COMPILED_DATA_STATE.read().expect("read compiled data");
    if entry_slot {
      assert!(compiled.is_empty(), "slot consumers require fresh type evidence");
      continue;
    }
    let compiled_file = compiled.get("app.main").expect("compiled file should remain for unaffected defs");
    assert!(!compiled_file.defs.contains_key("a"));
    assert!(!compiled_file.defs.contains_key("b"));
    assert!(!compiled_file.defs.contains_key("c"));
    assert!(compiled_file.defs.contains_key("d"));
  }
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
fn clear_runtime_caches_for_reload_clears_selected_packages_and_dependents() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let app_main = ensure_def_id("app.main", "entry");
  let app_extra = ensure_def_id("app.extra", "helper");
  let demo_reload = ensure_def_id("demo.feature", "reload");
  let util_consumer = ensure_def_id("util.consumer", "use-app");
  let util_keep = ensure_def_id("util.keep", "value");

  {
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("seed compiled data");
    compiled.insert(
      Arc::from("app.main"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("entry"), compiled_def_for_test(app_main, vec![]))]),
      },
    );
    compiled.insert(
      Arc::from("app.extra"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("helper"), compiled_def_for_test(app_extra, vec![]))]),
      },
    );
    compiled.insert(
      Arc::from("demo.feature"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("reload"), compiled_def_for_test(demo_reload, vec![]))]),
      },
    );
    compiled.insert(
      Arc::from("util.consumer"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("use-app"), compiled_def_for_test(util_consumer, vec![app_main]))]),
      },
    );
    compiled.insert(
      Arc::from("util.keep"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("value"), compiled_def_for_test(util_keep, vec![]))]),
      },
    );
  }

  write_runtime_ready("app.main", "entry", Calcit::Number(1.0)).expect("seed runtime app.main/entry");
  write_runtime_ready("app.extra", "helper", Calcit::Number(2.0)).expect("seed runtime app.extra/helper");
  write_runtime_ready("demo.feature", "reload", Calcit::Number(3.0)).expect("seed runtime demo.feature/reload");
  write_runtime_ready("util.consumer", "use-app", Calcit::Number(4.0)).expect("seed runtime util.consumer/use-app");
  write_runtime_ready("util.keep", "value", Calcit::Number(9.0)).expect("seed runtime util.keep/value");

  clear_runtime_caches_for_reload(Arc::from("app.main"), Arc::from("demo.feature"), false)
    .expect("clear runtime caches for reload packages");

  assert_eq!(lookup_runtime_cell("app.main", "entry"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_runtime_cell("app.extra", "helper"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_runtime_cell("demo.feature", "reload"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_runtime_cell("util.consumer", "use-app"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_runtime_ready("util.keep", "value"), Some(Calcit::Number(9.0)));

  let compiled = PROGRAM_COMPILED_DATA_STATE.read().expect("read compiled data");
  assert!(!compiled.contains_key("app.main"));
  assert!(!compiled.contains_key("app.extra"));
  assert!(!compiled.contains_key("demo.feature"));
  assert!(!compiled.get("util.consumer").is_some_and(|file| file.defs.contains_key("use-app")));
  assert!(compiled.get("util.keep").is_some_and(|file| file.defs.contains_key("value")));
}

#[test]
fn clear_runtime_caches_for_reload_with_reload_libs_clears_all_namespaces() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let app_main = ensure_def_id("app.main", "entry");
  let util_keep = ensure_def_id("util.keep", "value");

  {
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("seed compiled data");
    compiled.insert(
      Arc::from("app.main"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("entry"), compiled_def_for_test(app_main, vec![]))]),
      },
    );
    compiled.insert(
      Arc::from("util.keep"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("value"), compiled_def_for_test(util_keep, vec![]))]),
      },
    );
  }

  write_runtime_ready("app.main", "entry", Calcit::Number(1.0)).expect("seed runtime app.main/entry");
  write_runtime_ready("util.keep", "value", Calcit::Number(9.0)).expect("seed runtime util.keep/value");

  clear_runtime_caches_for_reload(Arc::from("app.main"), Arc::from("demo.feature"), true)
    .expect("clear all runtime caches for reload libs");

  assert_eq!(lookup_runtime_cell("app.main", "entry"), None);
  assert_eq!(lookup_runtime_cell("util.keep", "value"), None);

  let compiled = PROGRAM_COMPILED_DATA_STATE.read().expect("read compiled data");
  assert!(compiled.is_empty());
}

#[test]
fn entry_slot_inference_rechecks_consumers_after_transitive_source_changes() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  struct ClearSlots;
  impl Drop for ClearSlots {
    fn drop(&mut self) {
      calcit::clear_type_slots();
    }
  }
  let _slots = ClearSlots;
  let path = "tests/fixtures/enum-slot-alias.cirru";
  let source = std::fs::read_to_string(path).expect("shared Calcit fixture");
  let data = cirru_edn::parse(&source).expect("parse shared fixture");
  let mut snapshot = snapshot::load_snapshot_data(&data, path).expect("load shared fixture");
  snapshot.select_entry(Some("inferred")).expect("inferred entry");
  snapshot.files.extend(crate::load_core_snapshot().expect("core source").files);
  let mut source = extract_program_data(&snapshot).expect("program source");
  // This in-process regression needs only the callback contract; the shared
  // Calcit tests exercise its assertion through the fully initialized CLI.
  source.get_mut("app.main").unwrap().defs.get_mut("callback").unwrap().code = code_to_calcit(
    &Cirru::List(vec![
      Cirru::leaf("defn"),
      Cirru::leaf("callback"),
      Cirru::List(vec![Cirru::leaf("op")]),
      Cirru::leaf("&unit"),
    ]),
    "app.main",
    "callback",
    vec![],
  )
  .expect("callback without CLI host logging");
  // Inventory may request the representative itself before any entry code.
  // Its compile-time declaration must be processed exactly once.
  let representative = cirru_parser::parse("def Representative $ do (deftype-slot :inventory-once) (:: :sample)")
    .expect("parse representative source")
    .remove(0);
  source.get_mut("app.ops").unwrap().defs.get_mut("Representative").unwrap().code =
    code_to_calcit(&representative, "app.ops", "Representative", vec![]).expect("representative code");
  *PROGRAM_CODE_DATA.write().expect("install source") = source;
  let warnings = RefCell::new(vec![]);
  runner::preprocess::compile_source_def_for_snapshot("app.ops", "Representative", &warnings, &CallStackList::default())
    .expect("compile uncached representative once for inventory");
  runner::preprocess::ensure_ns_def_compiled("app.main", "main!", &warnings, &CallStackList::default()).expect("initial compile");
  assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
  assert!(matches!(
    calcit::resolve_type_slot("dispatch-op").as_deref(),
    Some(CalcitTypeAnnotation::AnonymousEnum)
  ));

  let mut changes = snapshot::ChangesDict::default();
  changes.changed.insert(
    Arc::from("app.ops"),
    snapshot::FileChangeInfo {
      ns: None,
      added_defs: HashMap::new(),
      removed_defs: HashSet::new(),
      changed_defs: HashMap::from([("Representative".to_owned(), Cirru::leaf("|changed-kind").into())]),
    },
  );
  apply_code_changes(&changes).expect("change aliased representative");
  let result = runner::preprocess::ensure_ns_def_compiled("app.main", "main!", &warnings, &CallStackList::default());
  assert!(
    result.is_err() || !warnings.borrow().is_empty(),
    "the old Enum callback proof must be invalidated"
  );
  assert!(matches!(
    calcit::resolve_type_slot("dispatch-op").as_deref(),
    Some(CalcitTypeAnnotation::String)
  ));
}

#[test]
fn predicate_guard_proof_is_invalidated_when_body_changes() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let path = "tests/fixtures/def-value-schema.cirru";
  let data = cirru_edn::parse(&std::fs::read_to_string(path).expect("shared Calcit fixture")).expect("parse shared fixture");
  let mut snapshot = snapshot::load_snapshot_data(&data, path).expect("load shared fixture");
  snapshot.files.extend(crate::load_core_snapshot().expect("core source").files);
  let ns = "app.short-circuit";
  let predicate = "guarded-number?";
  let caller = "add-with-user-guard";
  *PROGRAM_CODE_DATA.write().expect("install source") = extract_program_data(&snapshot).expect("program source");

  let warnings = RefCell::new(vec![]);
  let stack = CallStackList::default();
  runner::preprocess::ensure_ns_def_compiled(ns, caller, &warnings, &stack).expect("compile guarded caller");
  assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
  assert_eq!(lookup_proven_guard(ns, predicate).as_deref(), Some(&CalcitTypeAnnotation::Number));
  assert!(lookup_compiled_def(ns, caller).is_some());

  for replacement in [
    "defn guarded-number? (x) true",
    "defn guarded-number? (x) (hint-fn ({} (:async true))) (number? x)",
    "defn guarded-number? (x) (&= x 1)",
    "defn guarded-number? (x) (if (number? x) true (string? x))",
    "defn guarded-number? (x) (&let (x 7) (number? x))",
  ] {
    let body = cirru_parser::parse(replacement).expect("parse replacement body").remove(0);
    let mut changes = snapshot::ChangesDict::default();
    changes.changed.insert(
      Arc::from(ns),
      snapshot::FileChangeInfo {
        ns: None,
        added_defs: HashMap::new(),
        removed_defs: HashSet::new(),
        changed_defs: HashMap::from([(String::from(predicate), body.into())]),
      },
    );
    apply_code_changes(&changes).expect("apply predicate body change");
    clear_runtime_caches_for_changes(&changes, false).expect("invalidate predicate and dependent");
    assert!(lookup_proven_guard(ns, predicate).is_none());
    assert!(lookup_compiled_def(ns, caller).is_none());

    let error = runner::preprocess::ensure_ns_def_compiled(ns, caller, &warnings, &stack).expect_err("reject unproven guard");
    assert_eq!(
      error.code.as_deref(),
      Some("E_PREDICATE_GUARD_UNPROVEN"),
      "{replacement}: {error:?}"
    );
    assert!(lookup_proven_guard(ns, predicate).is_none());
  }
}

#[test]
fn snapshot_rebuilds_changed_source_backed_def_after_reload_changes() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let old_code = code_to_calcit(&Cirru::Leaf(Arc::from("1")), "app.reload", "demo", vec![]).expect("build initial source-backed code");

  PROGRAM_CODE_DATA.write().expect("seed program code").insert(
    Arc::from("app.reload"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::from([(
        Arc::from("demo"),
        ProgramDefEntry {
          tags: Default::default(),
          code: old_code.clone(),
          schema: DYNAMIC_TYPE.clone(),
          doc: Arc::from(""),
          examples: vec![],
          ffi: None,
        },
      )]),
    },
  );

  let _ = ensure_def_id("app.reload", "demo");

  store_compiled_output(
    "app.reload",
    "demo",
    CompiledDefPayload {
      proven_guard: None,
      version_id: 0,
      preprocessed_code: Calcit::Number(1.0),
      codegen_form: Calcit::Number(1.0),
      deps: vec![],
      type_summary: None,
      source_code: Some(old_code),
      schema: DYNAMIC_TYPE.clone(),
      doc: Arc::from(""),
      examples: vec![],
    },
  );
  write_runtime_ready("app.reload", "demo", Calcit::Number(1.0)).expect("seed stale runtime value");

  let mut changes = snapshot::ChangesDict::default();
  changes.changed.insert(
    Arc::from("app.reload"),
    snapshot::FileChangeInfo {
      ns: None,
      added_defs: HashMap::new(),
      removed_defs: HashSet::new(),
      changed_defs: HashMap::from([(String::from("demo"), Cirru::Leaf(Arc::from("2")).into())]),
    },
  );

  apply_code_changes(&changes).expect("apply source changes");
  clear_runtime_caches_for_changes(&changes, false).expect("clear runtime caches for source change");

  assert_eq!(lookup_runtime_cell("app.reload", "demo"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_compiled_def("app.reload", "demo"), None);

  let snapshot = clone_compiled_program_snapshot().expect("clone compiled snapshot after reload changes");
  let rebuilt = snapshot
    .get("app.reload")
    .and_then(|file| file.defs.get("demo"))
    .expect("snapshot should rebuild changed source-backed def");

  assert_eq!(rebuilt.codegen_form, Calcit::Number(2.0));
  assert_eq!(rebuilt.preprocessed_code, Calcit::Number(2.0));
  assert_eq!(rebuilt.source_code, Some(Calcit::Number(2.0)));
}

#[test]
fn removed_source_def_changes_still_invalidate_transitive_dependents() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let shared_code =
    code_to_calcit(&Cirru::Leaf(Arc::from("1")), "app.main", "shared", vec![]).expect("build shared source-backed code");
  let consumer_code =
    code_to_calcit(&Cirru::Leaf(Arc::from("2")), "app.consumer", "use-shared", vec![]).expect("build consumer source-backed code");
  let helper_code =
    code_to_calcit(&Cirru::Leaf(Arc::from("3")), "app.helper", "keep", vec![]).expect("build helper source-backed code");

  PROGRAM_CODE_DATA.write().expect("seed program code").extend(HashMap::from([
    (
      Arc::from("app.main"),
      ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([(
          Arc::from("shared"),
          ProgramDefEntry {
            tags: Default::default(),
            code: shared_code,
            schema: DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: None,
          },
        )]),
      },
    ),
    (
      Arc::from("app.consumer"),
      ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([(
          Arc::from("use-shared"),
          ProgramDefEntry {
            tags: Default::default(),
            code: consumer_code,
            schema: DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: None,
          },
        )]),
      },
    ),
    (
      Arc::from("app.helper"),
      ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([(
          Arc::from("keep"),
          ProgramDefEntry {
            tags: Default::default(),
            code: helper_code,
            schema: DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: None,
          },
        )]),
      },
    ),
  ]));

  let shared_def = ensure_def_id("app.main", "shared");
  let consumer_def = ensure_def_id("app.consumer", "use-shared");
  let helper_def = ensure_def_id("app.helper", "keep");

  {
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("seed compiled data");
    compiled.extend(HashMap::from([
      (
        Arc::from("app.main"),
        CompiledFileData {
          defs: HashMap::from([(Arc::from("shared"), compiled_def_for_test(shared_def, vec![]))]),
        },
      ),
      (
        Arc::from("app.consumer"),
        CompiledFileData {
          defs: HashMap::from([(Arc::from("use-shared"), compiled_def_for_test(consumer_def, vec![shared_def]))]),
        },
      ),
      (
        Arc::from("app.helper"),
        CompiledFileData {
          defs: HashMap::from([(Arc::from("keep"), compiled_def_for_test(helper_def, vec![]))]),
        },
      ),
    ]));
  }

  write_runtime_ready("app.main", "shared", Calcit::Number(1.0)).expect("seed runtime shared");
  write_runtime_ready("app.consumer", "use-shared", Calcit::Number(2.0)).expect("seed runtime use-shared");
  write_runtime_ready("app.helper", "keep", Calcit::Number(3.0)).expect("seed runtime keep");

  let mut changes = snapshot::ChangesDict::default();
  changes.changed.insert(
    Arc::from("app.main"),
    snapshot::FileChangeInfo {
      ns: None,
      added_defs: HashMap::new(),
      removed_defs: HashSet::from([String::from("shared")]),
      changed_defs: HashMap::new(),
    },
  );

  apply_code_changes(&changes).expect("apply removed source changes");
  clear_runtime_caches_for_changes(&changes, false).expect("clear runtime caches for removed source change");

  assert!(
    PROGRAM_CODE_DATA
      .read()
      .expect("read program code")
      .get("app.main")
      .is_some_and(|file| !file.defs.contains_key("shared"))
  );

  assert_eq!(lookup_runtime_cell("app.main", "shared"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_runtime_cell("app.consumer", "use-shared"), Some(RuntimeCell::Cold));
  assert_eq!(lookup_runtime_ready("app.helper", "keep"), Some(Calcit::Number(3.0)));

  let compiled = PROGRAM_COMPILED_DATA_STATE.read().expect("read compiled data");
  assert!(!compiled.get("app.main").is_some_and(|file| file.defs.contains_key("shared")));
  assert!(
    !compiled
      .get("app.consumer")
      .is_some_and(|file| file.defs.contains_key("use-shared"))
  );
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
          tags: Default::default(),
          code: warn_code.clone(),
          schema: DYNAMIC_TYPE.clone(),
          doc: Arc::from(""),
          examples: vec![],
          ffi: None,
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
fn snapshot_skips_empty_namespace_when_source_backed_rebuild_fails() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let failing_code = code_to_calcit(
    &Cirru::List(vec![
      Cirru::leaf("if"),
      Cirru::leaf("true"),
      Cirru::leaf("1"),
      Cirru::leaf("2"),
      Cirru::leaf("3"),
    ]),
    "app.fail",
    "broken",
    vec![],
  )
  .expect("build failing source-backed code");

  PROGRAM_CODE_DATA.write().expect("seed program code").insert(
    Arc::from("app.fail"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::from([(
        Arc::from("broken"),
        ProgramDefEntry {
          tags: Default::default(),
          code: failing_code,
          schema: DYNAMIC_TYPE.clone(),
          doc: Arc::from(""),
          examples: vec![],
          ffi: None,
        },
      )]),
    },
  );
  let _ = ensure_def_id("app.fail", "broken");

  let snapshot = clone_compiled_program_snapshot().expect("clone compiled snapshot");

  assert!(!snapshot.contains_key("app.fail"));
}

#[test]
fn snapshot_skips_unreferenced_runtime_only_defs() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let _ = ensure_def_id("app.runtime", "unused");
  write_runtime_ready("app.runtime", "unused", Calcit::Number(42.0)).expect("seed unreferenced runtime-only value");

  let snapshot = clone_compiled_program_snapshot().expect("clone compiled snapshot");
  assert!(!snapshot.contains_key("app.runtime"));
}

#[test]
fn snapshot_keeps_referenced_runtime_only_defs() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let runtime_def = ensure_def_id("app.runtime", "shared");
  let consumer_def = ensure_def_id("app.consumer", "use-shared");

  {
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("seed compiled data");
    compiled.insert(
      Arc::from("app.consumer"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("use-shared"), compiled_def_for_test(consumer_def, vec![runtime_def]))]),
      },
    );
  }

  write_runtime_ready("app.runtime", "shared", Calcit::Number(42.0)).expect("seed referenced runtime-only value");

  let snapshot = clone_compiled_program_snapshot().expect("clone compiled snapshot");
  let compiled = snapshot
    .get("app.runtime")
    .and_then(|file| file.defs.get("shared"))
    .expect("referenced runtime-only def should be preserved in snapshot");

  assert_eq!(compiled.codegen_form, Calcit::Number(42.0));
  assert_eq!(compiled.source_code, None);
}

#[test]
fn snapshot_skips_unserializable_referenced_runtime_only_defs() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let runtime_def = ensure_def_id("app.runtime", "shared-atom");
  let consumer_def = ensure_def_id("app.consumer", "use-shared-atom");

  {
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("seed compiled data");
    compiled.insert(
      Arc::from("app.consumer"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("use-shared-atom"), compiled_def_for_test(consumer_def, vec![runtime_def]))]),
      },
    );
  }

  write_runtime_ready(
    "app.runtime",
    "shared-atom",
    crate::builtins::quick_build_atom(Calcit::Number(42.0)),
  )
  .expect("seed referenced runtime-only atom");

  let snapshot = clone_compiled_program_snapshot().expect("clone compiled snapshot");
  assert!(!snapshot.contains_key("app.runtime"));
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
      proven_guard: None,
      version_id: 0,
      preprocessed_code: Calcit::Nil,
      codegen_form: Calcit::Nil,
      deps: vec![],
      type_summary: None,
      source_code: None,
      schema,
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
fn lenient_compiled_fallback_backfills_runtime_cache() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let _ = ensure_def_id("app.compiled", "callable");
  write_runtime_ready("app.compiled", "callable", Calcit::Number(0.0)).expect("seed runtime slot");
  mark_runtime_def_cold("app.compiled", "callable");

  store_compiled_output(
    "app.compiled",
    "callable",
    CompiledDefPayload {
      proven_guard: None,
      version_id: 0,
      preprocessed_code: Calcit::Number(7.0),
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

  let value = resolve_runtime_or_compiled_def(
    "app.compiled",
    "callable",
    None,
    RuntimeResolveMode::Lenient,
    &CallStackList::default(),
  )
  .expect("lenient compiled fallback should succeed");
  assert_eq!(value, Some(Calcit::Number(7.0)));
  assert_eq!(
    lookup_runtime_cell("app.compiled", "callable"),
    Some(RuntimeCell::Ready(Calcit::Number(7.0)))
  );
}

#[test]
fn preprocess_ns_def_materializes_compiled_function_with_runtime_backfill() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let fn_code = code_to_calcit(
    &Cirru::List(vec![
      Cirru::leaf("defn"),
      Cirru::leaf("callable"),
      Cirru::List(vec![Cirru::leaf("x")]),
      Cirru::leaf("x"),
    ]),
    "app.preprocess",
    "callable",
    vec![],
  )
  .expect("parse fn payload");

  let def_id = ensure_def_id("app.preprocess", "callable");
  store_compiled_output(
    "app.preprocess",
    "callable",
    CompiledDefPayload {
      proven_guard: None,
      version_id: 0,
      preprocessed_code: fn_code,
      codegen_form: Calcit::Nil,
      deps: vec![],
      type_summary: None,
      source_code: None,
      schema: DYNAMIC_TYPE.clone(),
      doc: Arc::from(""),
      examples: vec![],
    },
  );
  write_runtime_ready("app.preprocess", "callable", Calcit::Number(0.0)).expect("seed runtime slot");
  mark_runtime_def_cold("app.preprocess", "callable");

  let warnings = RefCell::new(vec![]);
  crate::runner::preprocess::ensure_ns_def_compiled("app.preprocess", "callable", &warnings, &CallStackList::default())
    .expect("compiled function should materialize for preprocess");
  let value = resolve_compiled_executable_def("app.preprocess", "callable", &CallStackList::default())
    .expect("lookup compiled function after ensure");

  assert!(matches!(value, Some(Calcit::Fn { .. })));
  assert!(matches!(
    lookup_runtime_cell_by_id(def_id),
    Some(RuntimeCell::Ready(Calcit::Fn { .. }))
  ));
}

#[test]
fn lazy_runtime_resolution_seeds_from_compiled_when_runtime_slot_is_missing() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let def_id = ensure_def_id("app.preprocess", "lazy-value");
  store_compiled_output(
    "app.preprocess",
    "lazy-value",
    CompiledDefPayload {
      proven_guard: None,
      version_id: 0,
      preprocessed_code: Calcit::Number(7.0),
      codegen_form: Calcit::Number(7.0),
      deps: vec![],
      type_summary: None,
      source_code: None,
      schema: DYNAMIC_TYPE.clone(),
      doc: Arc::from(""),
      examples: vec![],
    },
  );

  let value = resolve_runtime_or_compiled_def(
    "app.preprocess",
    "lazy-value",
    None,
    RuntimeResolveMode::Lenient,
    &CallStackList::default(),
  )
  .expect("resolve compiled lazy value");

  assert_eq!(value, Some(Calcit::Number(7.0)));
  assert_eq!(lookup_runtime_cell_by_id(def_id), Some(RuntimeCell::Ready(Calcit::Number(7.0))));
}

#[test]
fn run_program_compiles_then_executes_with_runtime_backfill() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let fn_code = code_to_calcit(
    &Cirru::List(vec![
      Cirru::leaf("defn"),
      Cirru::leaf("main"),
      Cirru::List(vec![]),
      Cirru::leaf("7"),
    ]),
    "app.main",
    "main",
    vec![],
  )
  .expect("parse main fn");

  PROGRAM_CODE_DATA.write().expect("seed program code").insert(
    Arc::from("app.main"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::from([(
        Arc::from("main"),
        ProgramDefEntry {
          tags: Default::default(),
          code: fn_code,
          schema: DYNAMIC_TYPE.clone(),
          doc: Arc::from(""),
          examples: vec![],
          ffi: None,
        },
      )]),
    },
  );

  let result = run_program_with_docs(Arc::from("app.main"), Arc::from("main"), &[]).expect("run compiled main");

  assert_eq!(result, Calcit::Number(7.0));
  assert!(lookup_compiled_def("app.main", "main").is_some());
  assert!(matches!(
    lookup_runtime_cell("app.main", "main"),
    Some(RuntimeCell::Ready(Calcit::Fn { .. }))
  ));
}

#[test]
fn runtime_resolve_mode_handles_resolving_cell_differently() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  mark_runtime_def_resolving("app.runtime", "pending");

  let strict = resolve_runtime_or_compiled_def(
    "app.runtime",
    "pending",
    None,
    RuntimeResolveMode::Strict,
    &CallStackList::default(),
  );
  assert!(matches!(strict, Err(RuntimeResolveError::RuntimeCell(RuntimeCell::Resolving))));

  let lenient = resolve_runtime_or_compiled_def(
    "app.runtime",
    "pending",
    None,
    RuntimeResolveMode::Lenient,
    &CallStackList::default(),
  );
  assert_eq!(lenient.expect("lenient resolving lookup"), None);
}

#[test]
fn runtime_resolve_mode_handles_errored_cell_differently() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  mark_runtime_def_errored("app.runtime", "broken", Arc::from("boom"));

  let strict = resolve_runtime_or_compiled_def("app.runtime", "broken", None, RuntimeResolveMode::Strict, &CallStackList::default());
  assert!(matches!(strict, Err(RuntimeResolveError::RuntimeCell(RuntimeCell::Errored(message))) if message.as_ref() == "boom"));

  let lenient = resolve_runtime_or_compiled_def(
    "app.runtime",
    "broken",
    None,
    RuntimeResolveMode::Lenient,
    &CallStackList::default(),
  );
  assert_eq!(lenient.expect("lenient errored lookup"), None);
}

#[test]
fn compiled_executable_code_only_exposes_executable_kinds() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  store_compiled_output(
    "app.compiled",
    "callable",
    CompiledDefPayload {
      proven_guard: None,
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

#[test]
fn hot_reload_keeps_tags_of_changed_def_and_leaves_new_def_untagged() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let old_code = code_to_calcit(&Cirru::Leaf(Arc::from("1")), "app.tags", "demo", vec![]).expect("build initial code");
  let tags: HashSet<cirru_edn::EdnTag> = HashSet::from([cirru_edn::EdnTag::new("effect")]);

  PROGRAM_CODE_DATA.write().expect("seed program code").insert(
    Arc::from("app.tags"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::from([(
        Arc::from("demo"),
        ProgramDefEntry {
          tags: tags.clone(),
          code: old_code,
          schema: DYNAMIC_TYPE.clone(),
          doc: Arc::from(""),
          examples: vec![],
          ffi: None,
        },
      )]),
    },
  );

  let mut changes = snapshot::ChangesDict::default();
  changes.changed.insert(
    Arc::from("app.tags"),
    snapshot::FileChangeInfo {
      ns: None,
      added_defs: HashMap::from([(String::from("fresh"), Cirru::Leaf(Arc::from("3")).into())]),
      removed_defs: HashSet::new(),
      changed_defs: HashMap::from([(String::from("demo"), Cirru::Leaf(Arc::from("2")).into())]),
    },
  );
  apply_code_changes(&changes).expect("apply source changes");

  let program = PROGRAM_CODE_DATA.read().expect("read program code");
  let file = program.get("app.tags").expect("file stays loaded");
  assert_eq!(file.defs["demo"].tags, tags);
  assert_eq!(file.defs["demo"].code, Calcit::Number(2.0));
  assert!(file.defs["fresh"].tags.is_empty());
}

fn parse_changes_for_test(text: &str) -> snapshot::ChangesDict {
  cirru_edn::parse(text)
    .expect("parse incremental change EDN")
    .try_into()
    .expect("convert incremental change EDN")
}

fn parse_code_entry_for_test(text: &str) -> snapshot::CodeEntry {
  cirru_edn::parse(text)
    .expect("parse CodeEntry EDN")
    .try_into()
    .expect("convert CodeEntry EDN")
}

const INC_FN_ENTRY: &str = r#"%{} :CodeEntry (:doc |fresh-doc)
  :code $ quote $ defn fresh (x) x
  :examples $ [] (quote $ fresh 1)
  :tags $ #{} :effect
  :ffi $ {} (:target :node)
  :schema $ quote $ Fn Number (-> Number)"#;

#[test]
fn incremental_code_entry_for_new_def_matches_cold_start() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  PROGRAM_CODE_DATA.write().expect("seed program code").insert(
    Arc::from("app.inc"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::new(),
    },
  );

  let indented = INC_FN_ENTRY.replace('\n', "\n        ");
  let changes = parse_changes_for_test(&format!(
    "{{}}\n  :changed $ {{}}\n    |app.inc $ {{}}\n      :added-defs $ {{}}\n        |fresh $ {indented}\n"
  ));
  let change = &changes.changed["app.inc"].added_defs["fresh"];
  assert!(!change.is_code_only(), "a CodeEntry payload keeps its metadata: {change:?}");

  apply_code_changes(&changes).expect("apply CodeEntry change");
  clear_runtime_caches_for_changes(&changes, false).expect("clear caches");

  let cold_entry = parse_code_entry_for_test(INC_FN_ENTRY);
  let cold_file = snapshot::FileInSnapShot {
    ns: snapshot::NsEntry {
      doc: String::new(),
      code: Cirru::List(vec![Cirru::leaf("ns"), Cirru::leaf("app.inc")]),
    },
    defs: HashMap::from([(String::from("fresh"), cold_entry)]),
  };
  let cold = extract_file_data(&cold_file, Arc::from("app.inc")).expect("cold start file data");

  let program = PROGRAM_CODE_DATA.read().expect("read program code");
  let incremental = &program["app.inc"].defs["fresh"];
  assert_eq!(incremental, &cold.defs["fresh"]);
  assert!(
    matches!(incremental.schema.as_ref(), CalcitTypeAnnotation::Fn(_)),
    "{:?}",
    incremental.schema
  );
  assert!(incremental.ffi.is_some());
  assert_eq!(incremental.doc.as_ref(), "fresh-doc");
  assert_eq!(incremental.examples.len(), 1);
  assert!(incremental.tags.contains(&EdnTag::new("effect")));
  drop(program);
  assert!(matches!(
    lookup_def_schema("app.inc", "fresh").as_ref(),
    CalcitTypeAnnotation::Fn(_)
  ));
}

#[test]
fn incremental_metadata_only_change_replaces_metadata_and_invalidates_dependents() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let code = Cirru::List(vec![
    Cirru::leaf("defn"),
    Cirru::leaf("fresh"),
    Cirru::List(vec![Cirru::leaf("x")]),
    Cirru::leaf("x"),
  ]);
  let old_code = code_to_calcit(&code, "app.inc", "fresh", vec![]).expect("build old code");
  let consumer_code = code_to_calcit(&Cirru::leaf("2"), "app.consumer", "use-fresh", vec![]).expect("build consumer code");
  let keep_code = code_to_calcit(&Cirru::leaf("3"), "app.consumer", "keep", vec![]).expect("build unrelated code");
  let plain = |code: Calcit| ProgramDefEntry {
    tags: HashSet::from([EdnTag::new("stale")]),
    code,
    schema: DYNAMIC_TYPE.clone(),
    doc: Arc::from("stale doc"),
    examples: vec![],
    ffi: None,
  };
  PROGRAM_CODE_DATA.write().expect("seed program code").extend(HashMap::from([
    (
      Arc::from("app.inc"),
      ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([(Arc::from("fresh"), plain(old_code))]),
      },
    ),
    (
      Arc::from("app.consumer"),
      ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([
          (Arc::from("use-fresh"), plain(consumer_code)),
          (Arc::from("keep"), plain(keep_code)),
        ]),
      },
    ),
  ]));

  let fresh_def = ensure_def_id("app.inc", "fresh");
  let consumer_def = ensure_def_id("app.consumer", "use-fresh");
  let keep_def = ensure_def_id("app.consumer", "keep");
  {
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("seed compiled data");
    compiled.extend(HashMap::from([
      (
        Arc::from("app.inc"),
        CompiledFileData {
          defs: HashMap::from([(Arc::from("fresh"), compiled_def_for_test(fresh_def, vec![]))]),
        },
      ),
      (
        Arc::from("app.consumer"),
        CompiledFileData {
          defs: HashMap::from([
            (Arc::from("use-fresh"), compiled_def_for_test(consumer_def, vec![fresh_def])),
            (Arc::from("keep"), compiled_def_for_test(keep_def, vec![])),
          ]),
        },
      ),
    ]));
  }
  write_runtime_ready("app.consumer", "use-fresh", Calcit::Number(2.0)).expect("seed consumer runtime");
  write_runtime_ready("app.consumer", "keep", Calcit::Number(3.0)).expect("seed unrelated runtime");

  // Same code as the loaded definition: only metadata differs.
  let entry = parse_code_entry_for_test(INC_FN_ENTRY);
  assert_eq!(entry.code, code);
  let mut changes = snapshot::ChangesDict::default();
  changes.changed.insert(
    Arc::from("app.inc"),
    snapshot::FileChangeInfo {
      ns: None,
      added_defs: HashMap::new(),
      removed_defs: HashSet::new(),
      changed_defs: HashMap::from([(String::from("fresh"), entry.clone().into())]),
    },
  );

  // The change dictionary survives the `.compact-inc.cirru` text round trip.
  let text = cirru_edn::format(&cirru_edn::Edn::try_from(changes.clone()).expect("serialize changes"), true).expect("format changes");
  assert_eq!(parse_changes_for_test(&text), changes);

  apply_code_changes(&changes).expect("apply metadata-only change");
  clear_runtime_caches_for_changes(&changes, false).expect("clear caches");

  {
    let program = PROGRAM_CODE_DATA.read().expect("read program code");
    let applied = &program["app.inc"].defs["fresh"];
    assert!(
      matches!(applied.schema.as_ref(), CalcitTypeAnnotation::Fn(_)),
      "{:?}",
      applied.schema
    );
    assert_eq!(applied.doc.as_ref(), "fresh-doc");
    assert_eq!(applied.tags, HashSet::from([EdnTag::new("effect")]));
    assert_eq!(applied.ffi, entry.ffi);
    assert_eq!(applied.examples, entry.examples);
  }
  assert!(matches!(
    lookup_def_schema("app.inc", "fresh").as_ref(),
    CalcitTypeAnnotation::Fn(_)
  ));
  assert_eq!(lookup_compiled_def("app.inc", "fresh"), None);
  assert_eq!(lookup_compiled_def("app.consumer", "use-fresh"), None);
  assert_eq!(lookup_runtime_cell("app.consumer", "use-fresh"), Some(RuntimeCell::Cold));
  assert!(lookup_compiled_def("app.consumer", "keep").is_some());
  assert_eq!(lookup_runtime_ready("app.consumer", "keep"), Some(Calcit::Number(3.0)));
}

#[test]
fn legacy_code_only_incremental_entries_keep_documented_metadata_behaviour() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let schema = parse_code_entry_for_test(INC_FN_ENTRY).schema;
  let old_code = code_to_calcit(&Cirru::leaf("1"), "app.legacy", "demo", vec![]).expect("build old code");
  let loaded = ProgramDefEntry {
    tags: HashSet::from([EdnTag::new("effect")]),
    code: old_code,
    schema,
    doc: Arc::from("kept doc"),
    examples: vec![Cirru::leaf("1")],
    ffi: Some(cirru_edn::Edn::map_from_iter([(
      cirru_edn::Edn::tag("target"),
      cirru_edn::Edn::tag("node"),
    )])),
  };
  PROGRAM_CODE_DATA.write().expect("seed program code").insert(
    Arc::from("app.legacy"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::from([(Arc::from("demo"), loaded.clone())]),
    },
  );

  // Pre-#2178 producers write bare quoted code.
  let changes = parse_changes_for_test(
    "{}\n  :changed $ {}\n    |app.legacy $ {}\n      :added-defs $ {}\n        |fresh $ quote 3\n      :changed-defs $ {}\n        |demo $ quote 2\n",
  );
  let info = &changes.changed["app.legacy"];
  assert_eq!(info.code_only_def_count(), 2);
  assert_eq!(info.changed_defs["demo"], snapshot::DefinitionChange::Code(Cirru::leaf("2")));

  apply_code_changes(&changes).expect("apply legacy change");

  let program = PROGRAM_CODE_DATA.read().expect("read program code");
  let file = &program["app.legacy"];
  assert_eq!(
    file.defs["demo"],
    ProgramDefEntry {
      code: Calcit::Number(2.0),
      ..loaded
    },
    "legacy changed-defs only replace code"
  );
  let fresh = &file.defs["fresh"];
  assert_eq!(fresh.code, Calcit::Number(3.0));
  assert!(matches!(fresh.schema.as_ref(), CalcitTypeAnnotation::Dynamic));
  assert!(fresh.doc.is_empty() && fresh.examples.is_empty() && fresh.ffi.is_none() && fresh.tags.is_empty());
}

#[test]
fn incremental_code_entry_rejects_invalid_metadata_without_partial_apply() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  PROGRAM_CODE_DATA.write().expect("seed program code").insert(
    Arc::from("app.inc"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::new(),
    },
  );
  let error = snapshot::ChangesDict::try_from(
    cirru_edn::parse("{}\n  :changed $ {}\n    |app.inc $ {}\n      :added-defs $ {}\n        |bad $ []\n").expect("parse"),
  )
  .expect_err("a list is neither quoted code nor a CodeEntry");
  assert!(error.contains("added-defs/bad"), "error: {error}");

  // `:ffi :js` needs a `:js-ffi` Fn schema; the cold-start validation applies too.
  let mut entry = parse_code_entry_for_test(INC_FN_ENTRY);
  entry.ffi = Some(cirru_edn::parse("{} (:target :node) (:js $ {} (:code |x))").expect("parse js ffi"));
  let mut changes = snapshot::ChangesDict::default();
  changes.changed.insert(
    Arc::from("app.inc"),
    snapshot::FileChangeInfo {
      ns: None,
      added_defs: HashMap::from([(String::from("ok"), Cirru::leaf("1").into()), (String::from("bad"), entry.into())]),
      removed_defs: HashSet::new(),
      changed_defs: HashMap::new(),
    },
  );
  let error = apply_code_changes(&changes).expect_err("invalid JS FFI metadata");
  assert!(error.contains("app.inc/bad") && error.contains(":ffi :js"), "error: {error}");
  assert!(PROGRAM_CODE_DATA.read().expect("read program code")["app.inc"].defs.is_empty());
}

#[test]
fn incremental_code_entry_rejects_defmacro_without_strict_macro_contract() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  PROGRAM_CODE_DATA.write().expect("seed program code").insert(
    Arc::from("app.inc"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::new(),
    },
  );
  // Cold start rejects this entry (`validate_strict_macro_schema`), so the
  // watcher must not accept it with a Dynamic schema either.
  let changes = parse_changes_for_test(
    "{}\n  :changed $ {}\n    |app.inc $ {}\n      :added-defs $ {}\n        |m $ %{} :CodeEntry (:doc |)\n          :code $ quote $ defmacro m (x) x\n",
  );
  let error = apply_code_changes(&changes).expect_err("defmacro without a Macro contract");
  assert!(error.contains("`defmacro` requires a strict `Macro` contract"), "error: {error}");
  assert!(PROGRAM_CODE_DATA.read().expect("read program code")["app.inc"].defs.is_empty());

  // The same definition inside an added namespace is checked as well.
  let mut added = snapshot::ChangesDict::default();
  added.added.insert(
    Arc::from("app.macros"),
    snapshot::FileInSnapShot {
      ns: snapshot::NsEntry {
        doc: String::new(),
        code: Cirru::List(vec![Cirru::leaf("ns"), Cirru::leaf("app.macros")]),
      },
      defs: HashMap::from([(
        String::from("m"),
        snapshot::CodeEntry::from_code(Cirru::List(vec![
          Cirru::leaf("defmacro"),
          Cirru::leaf("m"),
          Cirru::List(vec![Cirru::leaf("x")]),
          Cirru::leaf("x"),
        ])),
      )]),
    },
  );
  assert!(apply_code_changes(&added).is_err());
  assert!(!PROGRAM_CODE_DATA.read().expect("read program code").contains_key("app.macros"));
}

#[test]
fn incremental_changes_apply_all_namespaces_or_none() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let old_code = code_to_calcit(&Cirru::leaf("1"), "app.a", "demo", vec![]).expect("build old code");
  PROGRAM_CODE_DATA.write().expect("seed program code").extend(HashMap::from([
    (
      Arc::from("app.a"),
      ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([(
          Arc::from("demo"),
          ProgramDefEntry {
            tags: Default::default(),
            code: old_code,
            schema: DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: None,
          },
        )]),
      },
    ),
    (
      Arc::from("app.gone"),
      ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::new(),
      },
    ),
  ]));
  let demo_def = ensure_def_id("app.a", "demo");
  {
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("seed compiled data");
    compiled.insert(
      Arc::from("app.a"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("demo"), compiled_def_for_test(demo_def, vec![]))]),
      },
    );
  }
  write_runtime_ready("app.a", "demo", Calcit::Number(1.0)).expect("seed runtime");
  let before = PROGRAM_CODE_DATA.read().expect("read program code").clone();

  // `app.a` and the namespace edits are valid, `app.b` is not loaded.
  let changes = parse_changes_for_test(
    "{}\n  :added $ {}\n    |app.new $ {}\n      :ns $ %{} :NsEntry (:doc |) (:code $ quote $ ns app.new)\n      :defs $ {}\n  :removed $ #{} |app.gone\n  :changed $ {}\n    |app.a $ {}\n      :changed-defs $ {}\n        |demo $ quote 2\n    |app.b $ {}\n      :changed-defs $ {}\n        |x $ quote 3\n",
  );
  let error = apply_code_changes(&changes).expect_err("app.b is not loaded");
  assert!(error.contains("can not load ns: app.b"), "error: {error}");

  assert_eq!(*PROGRAM_CODE_DATA.read().expect("read program code"), before);
  assert!(lookup_compiled_def("app.a", "demo").is_some());
  assert_eq!(lookup_runtime_ready("app.a", "demo"), Some(Calcit::Number(1.0)));
}

#[test]
fn legacy_changed_entry_inherits_metadata_added_in_the_same_message() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  PROGRAM_CODE_DATA.write().expect("seed program code").insert(
    Arc::from("app.inc"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::new(),
    },
  );
  let entry = parse_code_entry_for_test(INC_FN_ENTRY);
  let mut changes = snapshot::ChangesDict::default();
  changes.changed.insert(
    Arc::from("app.inc"),
    snapshot::FileChangeInfo {
      ns: None,
      added_defs: HashMap::from([(String::from("fresh"), entry.clone().into())]),
      removed_defs: HashSet::new(),
      changed_defs: HashMap::from([(String::from("fresh"), Cirru::leaf("7").into())]),
    },
  );
  apply_code_changes(&changes).expect("apply added then changed");

  let program = PROGRAM_CODE_DATA.read().expect("read program code");
  let applied = &program["app.inc"].defs["fresh"];
  assert_eq!(applied.code, Calcit::Number(7.0));
  assert!(
    matches!(applied.schema.as_ref(), CalcitTypeAnnotation::Fn(_)),
    "{:?}",
    applied.schema
  );
  assert_eq!(applied.ffi, entry.ffi);
  assert_eq!(applied.doc.as_ref(), "fresh-doc");
}

#[test]
fn incremental_definition_names_follow_snapshot_identifier_rules() {
  let duplicate = snapshot::ChangesDict::try_from(
    cirru_edn::parse("{}\n  :changed $ {}\n    |app.inc $ {}\n      :changed-defs $ {}\n        |x $ quote 1\n        'x $ quote 2\n")
      .expect("parse"),
  )
  .expect_err("String and Symbol spellings of one name collide");
  assert!(duplicate.contains("duplicate snapshot identifier `x`"), "error: {duplicate}");

  let tag = snapshot::ChangesDict::try_from(
    cirru_edn::parse("{}\n  :changed $ {}\n    |app.inc $ {}\n      :added-defs $ {}\n        :x $ quote 1\n").expect("parse"),
  )
  .expect_err("tags are not definition identifiers");
  assert!(tag.contains("must be a String or Symbol"), "error: {tag}");

  let symbol = snapshot::ChangesDict::try_from(
    cirru_edn::parse("{}\n  :changed $ {}\n    |app.inc $ {}\n      :added-defs $ {}\n        'x $ quote 1\n").expect("parse"),
  )
  .expect("Symbol keys are accepted like in :defs");
  assert!(symbol.changed["app.inc"].added_defs.contains_key("x"));
}

fn impl_attach_file(ns: &str, imports: &[(&str, &str)], defs: &[(&str, &str)]) -> ProgramFileData {
  ProgramFileData {
    import_map: imports
      .iter()
      .map(|(alias, target)| (Arc::from(*alias), Arc::new(ImportRule::NsAs(Arc::from(*target)))))
      .collect(),
    defs: defs
      .iter()
      .map(|(def, source)| {
        let cirru = cirru_parser::parse(source).expect("parse test source").remove(0);
        let code = code_to_calcit(&cirru, ns, def, vec![]).expect("build test code");
        (
          Arc::from(*def),
          ProgramDefEntry {
            tags: Default::default(),
            code,
            schema: DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: None,
          },
        )
      })
      .collect(),
  }
}

/// `app.t` has the trait, `app.m` the type, `app.o` is unrelated.
fn impl_attach_program(type_source: &str, type_ns_impls: &[(&str, &str)], other_impls: &[(&str, &str)]) -> ProgramCodeData {
  let mut main_defs = vec![("P", type_source), ("Old", "defimpl Old t/Labeled (.label f)")];
  main_defs.extend_from_slice(type_ns_impls);
  HashMap::from([
    (
      Arc::from("app.t"),
      impl_attach_file(
        "app.t",
        &[("m", "app.m")],
        &[("Labeled", "deftrait Labeled (.label (Fn Self (-> String)))")],
      ),
    ),
    (Arc::from("app.m"), impl_attach_file("app.m", &[("t", "app.t")], &main_defs)),
    (
      Arc::from("app.o"),
      impl_attach_file("app.o", &[("t", "app.t"), ("m", "app.m")], other_impls),
    ),
  ])
}

const LISTED_TYPE: &str = "defstruct P (:name String) (:impls t/Labeled)";

#[test]
fn defimpl_type_trait_attaches_through_the_impls_clause() {
  let _guard = lock_program_test_state();
  let mut program = impl_attach_program(LISTED_TYPE, &[("P-Labeled", "defimpl P t/Labeled (.label f)")], &[]);
  let attached = impl_attach::attach_source_impls(&mut program).expect("attach impls");
  assert_eq!(attached, vec![(Arc::from("app.m"), Arc::from("P"))]);
  let first = program["app.m"].defs["P"].code.to_string();
  assert_eq!(
    first,
    "([] 'calcit.core/impl-traits ([] 'defstruct 'P ([] :name 'String)) 'P-Labeled)"
  );

  impl_attach::attach_source_impls(&mut program).expect("attach again");
  assert_eq!(program["app.m"].defs["P"].code.to_string(), first, "attaching is idempotent");

  // A type that does not list the trait gets nothing, even with a `defimpl` next to it.
  let mut unlisted = impl_attach_program(
    "defstruct P (:name String)",
    &[("P-Labeled", "defimpl P t/Labeled (.label f)")],
    &[],
  );
  assert!(impl_attach::attach_source_impls(&mut unlisted).expect("no clause").is_empty());
}

#[test]
fn defimpl_type_trait_is_looked_up_only_in_the_type_and_trait_namespaces() {
  let _guard = lock_program_test_state();
  let mut in_trait_ns = impl_attach_program(LISTED_TYPE, &[], &[]);
  in_trait_ns.insert(
    Arc::from("app.t"),
    impl_attach_file(
      "app.t",
      &[("m", "app.m")],
      &[
        ("Labeled", "deftrait Labeled (.label (Fn Self (-> String)))"),
        ("P-Labeled", "defimpl m/P Labeled (.label f)"),
      ],
    ),
  );
  impl_attach::attach_source_impls(&mut in_trait_ns).expect("impl in the trait namespace");
  assert!(in_trait_ns["app.m"].defs["P"].code.to_string().ends_with("'app.t/P-Labeled)"));

  let mut elsewhere = impl_attach_program(LISTED_TYPE, &[], &[("P-Labeled", "defimpl m/P t/Labeled (.label f)")]);
  let error = impl_attach::attach_source_impls(&mut elsewhere).expect_err("impl in a third namespace is not found");
  assert!(error.starts_with("[E_MISSING_TRAIT_IMPL] app.m/P"), "{error}");

  let mut duplicate = impl_attach_program(
    LISTED_TYPE,
    &[
      ("P-Labeled", "defimpl P t/Labeled (.label f)"),
      ("P-Labeled2", "defimpl P t/Labeled (.label g)"),
    ],
    &[],
  );
  let error = impl_attach::attach_source_impls(&mut duplicate).expect_err("duplicate impl");
  assert!(error.starts_with("[E_DUPLICATE_TRAIT_IMPL] app.m/P"), "{error}");

  let mut unknown = impl_attach_program("defstruct P (:name String) (:impls Missing)", &[], &[]);
  let error = impl_attach::attach_source_impls(&mut unknown).expect_err("unknown trait");
  assert!(error.starts_with("[E_UNKNOWN_IMPLS_TRAIT] app.m/P"), "{error}");

  for repeated_source in [
    "defstruct P (:name String) (:impls t/Labeled t/Labeled)",
    "defstruct P (:name String) (:impls t/Labeled app.t/Labeled)",
    "defstruct P (:name String) (:impls t/Labeled LabeledAlias)",
    "defstruct P (:name String) (:impls LabeledAlias t/Labeled)",
  ] {
    let mut repeated = impl_attach_program(
      repeated_source,
      &[
        ("LabeledAlias", "def LabeledAlias t/Labeled"),
        ("P-Labeled", "defimpl P t/Labeled (.label f)"),
      ],
      &[],
    );
    let error = impl_attach::attach_source_impls(&mut repeated).expect_err("repeated trait in (:impls ...)");
    assert!(error.starts_with("[E_DUPLICATE_TRAIT_IMPL] app.m/P"), "{repeated_source}: {error}");
  }
}

#[test]
fn reload_preserves_program_and_caches_when_impls_validation_fails() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let mut program = impl_attach_program(LISTED_TYPE, &[("P-Labeled", "defimpl P t/Labeled (.label f)")], &[]);
  impl_attach::attach_source_impls(&mut program).expect("initial attach");
  PROGRAM_CODE_DATA.write().expect("seed program code").extend(program);
  let p_id = register_program_def_id("app.m", "P");
  write_runtime_ready("app.m", "P", Calcit::Number(42.0)).expect("seed runtime value for P");
  {
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("seed compiled cache");
    compiled.insert(
      Arc::from("app.m"),
      CompiledFileData {
        defs: HashMap::from([(Arc::from("P"), compiled_def_for_test(p_id, vec![]))]),
      },
    );
  }
  let code = |def: &str| {
    PROGRAM_CODE_DATA
      .read()
      .expect("read code")
      .get("app.m")
      .and_then(|file| file.defs.get(def))
      .map(|entry| entry.code.to_string())
  };
  let initial_p = code("P").expect("initial P");

  let mut invalid_changes = snapshot::ChangesDict::default();
  let mut invalid_info = snapshot::FileChangeInfo {
    ns: None,
    added_defs: HashMap::new(),
    removed_defs: HashSet::new(),
    changed_defs: HashMap::new(),
  };
  let invalid_q = cirru_parser::parse("defstruct Q (:x Number) (:impls t/Labeled t/Labeled)")
    .expect("parse invalid Q")
    .remove(0);
  invalid_info.added_defs.insert(String::from("Q"), invalid_q.into());
  invalid_changes.changed.insert(Arc::from("app.m"), invalid_info);

  let error = apply_code_changes(&invalid_changes).expect_err("duplicate (:impls ...) must fail reload");
  assert!(error.contains("E_DUPLICATE_TRAIT_IMPL"), "{error}");
  assert_eq!(code("Q"), None, "rejected definition must not be installed");
  assert_eq!(
    code("P").as_deref(),
    Some(initial_p.as_str()),
    "untouched attached type stays attached"
  );
  assert_eq!(
    lookup_runtime_cell("app.m", "P"),
    Some(RuntimeCell::Ready(Calcit::Number(42.0))),
    "failed reload must leave runtime value intact"
  );
  assert!(
    lookup_compiled_def("app.m", "P").is_some(),
    "failed reload must leave compiled cache intact"
  );

  let mut valid_changes = snapshot::ChangesDict::default();
  let mut valid_info = snapshot::FileChangeInfo {
    ns: None,
    added_defs: HashMap::new(),
    removed_defs: HashSet::new(),
    changed_defs: HashMap::new(),
  };
  let valid_q = cirru_parser::parse("defstruct Q (:x Number) (:impls t/Labeled)")
    .expect("parse valid Q")
    .remove(0);
  let valid_impl = cirru_parser::parse("defimpl Q t/Labeled (.label f)")
    .expect("parse valid impl")
    .remove(0);
  valid_info.added_defs.insert(String::from("Q"), valid_q.into());
  valid_info.added_defs.insert(String::from("Q-Labeled"), valid_impl.into());
  valid_changes.changed.insert(Arc::from("app.m"), valid_info);

  apply_code_changes(&valid_changes).expect("corrected reload should succeed");
  assert_eq!(
    code("Q").as_deref(),
    Some("([] 'calcit.core/impl-traits ([] 'defstruct 'Q ([] :x 'Number)) 'Q-Labeled)")
  );
  assert_eq!(
    code("P").as_deref(),
    Some(initial_p.as_str()),
    "existing attached type remains attached"
  );
}

#[test]
fn reload_reattaches_when_a_defimpl_changes_in_another_namespace() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let mut program = impl_attach_program(LISTED_TYPE, &[], &[]);
  program.insert(
    Arc::from("app.t"),
    impl_attach_file(
      "app.t",
      &[("m", "app.m")],
      &[
        ("Labeled", "deftrait Labeled (.label (Fn Self (-> String)))"),
        ("P-Labeled", "defimpl m/P Labeled (.label f)"),
      ],
    ),
  );
  impl_attach::attach_source_impls(&mut program).expect("initial attach");
  PROGRAM_CODE_DATA.write().expect("seed program code").extend(program);
  let _ = register_program_def_id("app.m", "P");
  write_runtime_ready("app.m", "P", Calcit::Nil).expect("seed stale type value");
  let code = |def: &str| PROGRAM_CODE_DATA.read().expect("read code")["app.m"].defs[def].code.to_string();
  let attached = code("P");

  let rename_impl = |from: &str, to: &str| {
    let mut changes = snapshot::ChangesDict::default();
    let mut info = snapshot::FileChangeInfo {
      ns: None,
      added_defs: HashMap::new(),
      removed_defs: HashSet::from([String::from(from)]),
      changed_defs: HashMap::new(),
    };
    let source = cirru_parser::parse("defimpl m/P Labeled (.label f)").expect("parse").remove(0);
    info.added_defs.insert(String::from(to), source.into());
    changes.changed.insert(Arc::from("app.t"), info);
    changes
  };

  apply_code_changes(&rename_impl("P-Labeled", "P-Labeled-v2")).expect("move impl to a new name");
  assert_eq!(
    code("P"),
    "([] 'calcit.core/impl-traits ([] 'defstruct 'P ([] :name 'String)) 'app.t/P-Labeled-v2)"
  );
  assert_ne!(code("P"), attached);
  assert_eq!(lookup_runtime_cell("app.m", "P"), Some(RuntimeCell::Cold), "the type is rebuilt");

  let mut changes = snapshot::ChangesDict::default();
  changes.changed.insert(
    Arc::from("app.t"),
    snapshot::FileChangeInfo {
      ns: None,
      added_defs: HashMap::new(),
      removed_defs: HashSet::from([String::from("P-Labeled-v2")]),
      changed_defs: HashMap::new(),
    },
  );
  let error = apply_code_changes(&changes).expect_err("a listed trait without impl");
  assert!(error.contains("E_MISSING_TRAIT_IMPL"), "{error}");
}

#[test]
fn list_source_def_names_returns_sorted_definition_names() {
  let _guard = lock_program_test_state();
  reset_program_test_state();

  let empty_entry = |name: &str| {
    (
      Arc::from(name),
      ProgramDefEntry {
        tags: Default::default(),
        code: Calcit::Nil,
        schema: DYNAMIC_TYPE.clone(),
        doc: Arc::from(""),
        examples: vec![],
        ffi: None,
      },
    )
  };
  PROGRAM_CODE_DATA.write().expect("seed sorted def list").insert(
    Arc::from("tests.sorted-defs"),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: HashMap::from([empty_entry("zeta"), empty_entry("alpha"), empty_entry("mid")]),
    },
  );

  assert_eq!(
    list_source_def_names("tests.sorted-defs"),
    vec![Arc::from("alpha"), Arc::from("mid"), Arc::from("zeta")]
  );
  assert!(list_source_def_names("tests.missing-defs").is_empty());
}

#[test]
fn coherent_nominal_types_reject_duplicate_top_level_defimpl_and_collect_non_coherent_forms() {
  let _guard = lock_program_test_state();

  // Plain defstruct without (:impls ...) is coherent, so two top-level defimpls for (P, Labeled) fail.
  let mut same_ns_dup = impl_attach_program(
    "defstruct P (:name String)",
    &[
      ("P-Labeled-1", "defimpl P t/Labeled (.label f)"),
      ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
    ],
    &[],
  );
  let err = impl_attach::attach_source_impls(&mut same_ns_dup).expect_err("duplicate top-level defimpl must fail");
  assert!(err.starts_with("[E_DUPLICATE_TRAIT_IMPL] app.m/P"), "{err}");

  // Duplicate across the type namespace and trait namespace also fails on a coherent nominal type.
  let mut cross_ns_dup = impl_attach_program(
    "defstruct P (:name String)",
    &[("P-Labeled-1", "defimpl P t/Labeled (.label f)")],
    &[],
  );
  cross_ns_dup.insert(
    Arc::from("app.t"),
    impl_attach_file(
      "app.t",
      &[("m", "app.m")],
      &[
        ("Labeled", "deftrait Labeled (.label (Fn Self (-> String)))"),
        ("P-Labeled-2", "defimpl m/P Labeled (.label g)"),
      ],
    ),
  );
  let err = impl_attach::attach_source_impls(&mut cross_ns_dup).expect_err("cross-ns duplicate top-level defimpl must fail");
  assert!(err.starts_with("[E_DUPLICATE_TRAIT_IMPL] app.m/P"), "{err}");

  // Every runtime/local impl attachment primitive marks the target nominal definition non-coherent.
  for (marker_expr, expect_non_coherent) in [
    ("def LocalType $ defstruct P (:name String)", false),
    ("def LocalEnum $ defenum P (:none)", false),
    ("def p-v $ impl-traits (%{} P :name |a) P-Labeled", true),
    ("def p-v $ &struct:impl-traits (%{} P :name |a) P-Labeled", true),
    ("def p-v $ &enum:impl-traits (%:: P :none) P-Labeled", true),
    ("def p-d $ &struct-def:impl-traits P P-Labeled", true),
    ("def p-d $ &enum-def:impl-traits P P-Labeled", true),
    ("defn helper (x) (hint-fn $ Fn P (-> Any)) (&struct:impl-traits x P-Labeled)", true),
    ("defn helper (x) (assert-type x P) (&struct:impl-traits x P-Labeled)", true),
  ] {
    let program = impl_attach_program(
      "defstruct P (:name String)",
      &[
        ("P-Labeled", "defimpl P t/Labeled (.label f)"),
        ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
        ("UseP", marker_expr),
      ],
      &[],
    );
    let non_coherent = impl_attach::collect_non_coherent_nominal_defs(&program);
    assert_eq!(
      non_coherent.contains("app.m/P"),
      expect_non_coherent,
      "unexpected coherence detection for `{marker_expr}`: {non_coherent:?}"
    );
    if expect_non_coherent {
      let mut detached = program;
      impl_attach::attach_source_impls(&mut detached).expect("non-coherent nominal types preserve detached defimpls");
    }
  }

  // Hand-written top-level `def P (impl-traits (defstruct P ...) ...)` outside calcit.core also marks P non-coherent.
  let mut hand_wrapped = impl_attach_program(
    "impl-traits (defstruct P (:name String)) P-Labeled",
    &[
      ("P-Labeled", "defimpl P t/Labeled (.label f)"),
      ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
    ],
    &[],
  );
  assert!(
    impl_attach::collect_non_coherent_nominal_defs(&hand_wrapped).contains("app.m/P"),
    "hand-written top-level impl-traits wrapper must mark nominal def non-coherent"
  );
  impl_attach::attach_source_impls(&mut hand_wrapped).expect("hand-wrapped non-coherent def preserves detached defimpls");

  // Factory calls `(make-p)` returning a nominal type must mark that type non-coherent when passed to `impl-traits`.
  for factory_def in [
    "defn make-p () (hint-fn $ Fn (-> P)) (%{} P :name |a)",
    "defn make-p () (%{} P :name |a)",
  ] {
    let mut factory_program = impl_attach_program(
      "defstruct P (:name String)",
      &[
        ("P-Labeled", "defimpl P t/Labeled (.label f)"),
        ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
        ("make-p", factory_def),
        ("UseFactory", "def p-v $ impl-traits (make-p) P-Labeled"),
      ],
      &[],
    );
    assert!(
      impl_attach::collect_non_coherent_nominal_defs(&factory_program).contains("app.m/P"),
      "factory call `{factory_def}` must mark returned nominal def non-coherent"
    );
    impl_attach::attach_source_impls(&mut factory_program).expect("factory-attached non-coherent def preserves detached defimpls");
  }

  // Shadowing `let` binding `(let ((P (make-q))) (impl-traits P ...))` must NOT fall through to top-level `P`.
  let mut shadowed = impl_attach_program(
    "defstruct P (:name String)",
    &[
      ("P-Labeled", "defimpl P t/Labeled (.label f)"),
      ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
      ("make-q", "defn make-q () 42"),
      ("UseShadow", "def v $ let ((P (make-q))) (impl-traits P P-Labeled)"),
    ],
    &[],
  );
  assert!(
    !impl_attach::collect_non_coherent_nominal_defs(&shadowed).contains("app.m/P"),
    "shadowed local binding without a nominal target must not mark top-level P non-coherent"
  );
  let err = impl_attach::attach_source_impls(&mut shadowed).expect_err("top-level P stays coherent and rejects duplicate defimpl");
  assert!(err.starts_with("[E_DUPLICATE_TRAIT_IMPL] app.m/P"), "{err}");

  // Deep branching factory DAG (26 layers of `(if flag (prev) (prev))`) must finish linearly without exponential blowup.
  let mut dag_defs: Vec<(String, String)> = vec![
    ("P-Labeled".to_owned(), "defimpl P t/Labeled (.label f)".to_owned()),
    ("P-Labeled-2".to_owned(), "defimpl P t/Labeled (.label g)".to_owned()),
    ("f0".to_owned(), "defn f0 () (%{} P :name |a)".to_owned()),
  ];
  let mut prev = String::from("f0");
  for layer in 1..=26 {
    let name = format!("f{layer}");
    dag_defs.push((name.clone(), format!("defn {name} (flag) (if flag ({prev}) ({prev}))")));
    prev = name;
  }
  dag_defs.push(("UseDag".to_owned(), format!("def v $ impl-traits ({prev} true) P-Labeled")));
  let dag_refs: Vec<(&str, &str)> = dag_defs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
  let mut dag_program = impl_attach_program("defstruct P (:name String)", &dag_refs, &[]);
  assert!(
    impl_attach::collect_non_coherent_nominal_defs(&dag_program).contains("app.m/P"),
    "26-layer branching factory DAG must mark P non-coherent quickly"
  );
  impl_attach::attach_source_impls(&mut dag_program).expect("DAG-attached non-coherent def preserves detached defimpls");

  // Unannotated legacy attachment helper `(defn decorate (x) (impl-traits x P-Labeled))` propagates caller argument types.
  let mut unannotated_helper = impl_attach_program(
    "defstruct P (:name String)",
    &[
      ("P-Labeled", "defimpl P t/Labeled (.label f)"),
      ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
      ("decorate", "defn decorate (x) (impl-traits x P-Labeled)"),
      ("wrap", "defn wrap (y) (decorate y)"),
      ("UseWrap", "def v $ wrap (%{} P :name |a)"),
    ],
    &[],
  );
  assert!(
    impl_attach::collect_non_coherent_nominal_defs(&unannotated_helper).contains("app.m/P"),
    "unannotated attachment helper must propagate caller nominal argument into non_coherent set"
  );
  impl_attach::attach_source_impls(&mut unannotated_helper).expect("unannotated helper keeps detached defimpls on passed nominal type");

  // Unannotated attachment helper with NO nominal callers must NOT mark unrelated top-level P non-coherent.
  let mut unused_helper = impl_attach_program(
    "defstruct P (:name String)",
    &[
      ("P-Labeled", "defimpl P t/Labeled (.label f)"),
      ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
      ("decorate", "defn decorate (x) (impl-traits x P-Labeled)"),
    ],
    &[],
  );
  assert!(
    !impl_attach::collect_non_coherent_nominal_defs(&unused_helper).contains("app.m/P"),
    "uncalled unannotated helper must not mark unrelated P non-coherent"
  );
  let err = impl_attach::attach_source_impls(&mut unused_helper).expect_err("unrelated P must still reject duplicate defimpl");
  assert!(err.starts_with("[E_DUPLICATE_TRAIT_IMPL] app.m/P"), "{err}");
}

#[test]
fn impl_attach_normalizes_trait_aliases_and_tracks_match_coherence() {
  let _guard = lock_program_test_state();

  // `(:impls LabeledAlias)` in `app.m` finds `defimpl m/P Labeled` in origin `app.t`.
  let mut alias_in_impls = impl_attach_program(
    "defstruct P (:name String) (:impls LabeledAlias)",
    &[("LabeledAlias", "def LabeledAlias t/Labeled")],
    &[],
  );
  alias_in_impls.insert(
    Arc::from("app.t"),
    impl_attach_file(
      "app.t",
      &[("m", "app.m")],
      &[
        ("Labeled", "deftrait Labeled (.label (Fn Self (-> String)))"),
        ("P-Labeled", "defimpl m/P Labeled (.label f)"),
      ],
    ),
  );
  impl_attach::attach_source_impls(&mut alias_in_impls).expect("impls trait alias must match origin defimpl in trait ns");
  assert!(alias_in_impls["app.m"].defs["P"].code.to_string().ends_with("'app.t/P-Labeled)"));

  // `(:impls t/Labeled)` finds `defimpl P LabeledAlias` when alias is in type ns.
  let mut alias_in_defimpl = impl_attach_program(
    LISTED_TYPE,
    &[
      ("LabeledAlias", "def LabeledAlias t/Labeled"),
      ("P-Labeled", "defimpl P LabeledAlias (.label f)"),
    ],
    &[],
  );
  impl_attach::attach_source_impls(&mut alias_in_defimpl).expect("defimpl using trait alias must match origin (:impls ...)");
  assert!(alias_in_defimpl["app.m"].defs["P"].code.to_string().ends_with("'P-Labeled)"));

  // Duplicate impl mixing origin and alias is rejected.
  let mut dup_alias_and_origin = impl_attach_program(
    LISTED_TYPE,
    &[
      ("LabeledAlias", "def LabeledAlias t/Labeled"),
      ("P-Labeled-1", "defimpl P LabeledAlias (.label f)"),
      ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
    ],
    &[],
  );
  let dup_err = impl_attach::attach_source_impls(&mut dup_alias_and_origin).expect_err("duplicate impl via alias must fail");
  assert!(dup_err.starts_with("[E_DUPLICATE_TRAIT_IMPL] app.m/P"), "{dup_err}");

  // `(impl-traits (match ...) ...)` marks branch nominal targets non-coherent.
  let mut match_branch_attach = impl_attach_program(
    "defstruct P (:name String)",
    &[
      ("P-Labeled-1", "defimpl P t/Labeled (.label f)"),
      ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
      (
        "UseMatch",
        "defn use-match (flag) (impl-traits (match flag (true (%{} P :name |a)) (_ (%{} P :name |b))) P-Labeled-1)",
      ),
    ],
    &[],
  );
  assert!(
    impl_attach::collect_non_coherent_nominal_defs(&match_branch_attach).contains("app.m/P"),
    "match branch returning nominal struct inside impl-traits must mark P non-coherent"
  );
  impl_attach::attach_source_impls(&mut match_branch_attach).expect("match-attached non-coherent type preserves detached defimpls");

  // `(impl-traits (match flag (true x) (_ ...)) ...)` propagates parameter sink through match.
  let mut match_param_sink = impl_attach_program(
    "defstruct P (:name String)",
    &[
      ("P-Labeled-1", "defimpl P t/Labeled (.label f)"),
      ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
      (
        "decorate",
        "defn decorate (flag x fallback) (impl-traits (match flag (true x) (_ fallback)) P-Labeled-1)",
      ),
      ("UseDecorate", "def v $ decorate true (%{} P :name |a) 0"),
    ],
    &[],
  );
  assert!(
    impl_attach::collect_non_coherent_nominal_defs(&match_param_sink).contains("app.m/P"),
    "match returning parameter inside impl-traits must propagate param sink to caller nominal type"
  );
  impl_attach::attach_source_impls(&mut match_param_sink).expect("match param sink keeps detached defimpls");

  // Pattern-bound local `P` inside `match` must not leak to top-level `P`.
  let match_shadowed = impl_attach_program(
    "defstruct P (:name String)",
    &[
      ("P-Labeled-1", "defimpl P t/Labeled (.label f)"),
      ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
      (
        "UseShadowMatch",
        "defn use-shadow (opt) (impl-traits (match opt ((:some P) P) ((:none) 0)) P-Labeled-1)",
      ),
    ],
    &[],
  );
  assert!(
    !impl_attach::collect_non_coherent_nominal_defs(&match_shadowed).contains("app.m/P"),
    "pattern-bound local P in match must not mark top-level P non-coherent"
  );

  // Nested `match` inside pattern-bound branch must keep outer pattern shadowing in both walker and param sink collector.
  let nested_match_shadowed = impl_attach_program(
    "defstruct P (:name String)",
    &[
      ("P-Labeled-1", "defimpl P t/Labeled (.label f)"),
      ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
      (
        "UseNestedShadowMatch",
        "defn use-nested-shadow (opt) (match opt ((:some P) (impl-traits (match true (true P) (_ P)) P-Labeled-1)) ((:none) 0))",
      ),
      (
        "decorate-shadow",
        "defn decorate-shadow (x opt) (match opt ((:some x) (impl-traits (match true (true x) (_ x)) P-Labeled-1)) ((:none) 0))",
      ),
      ("CallDecorateShadow", "def v $ decorate-shadow (%{} P :name |a) (Some 1)"),
    ],
    &[],
  );
  assert!(
    !impl_attach::collect_non_coherent_nominal_defs(&nested_match_shadowed).contains("app.m/P"),
    "nested match inside shadowing match branch must not leak outer param or top-level P"
  );

  // Pattern-bound name shadowing both an outer local helper and a global attachment helper `decorate`
  // must keep an empty sink entry so `(decorate ...)` does not fall through to the global helper.
  let shadowed_global_fn = impl_attach_program(
    "defstruct P (:name String)",
    &[
      ("P-Labeled-1", "defimpl P t/Labeled (.label f)"),
      ("P-Labeled-2", "defimpl P t/Labeled (.label g)"),
      ("decorate", "defn decorate (x) (impl-traits x P-Labeled-1)"),
      (
        "UseMatchShadowGlobalFn",
        "defn use-shadow-fn (opt) (let ((decorate (fn (x) x))) (match opt ((:some decorate) (decorate (%{} P :name |a))) ((:none) 0)))",
      ),
      (
        "WrapShadowGlobalFn",
        "defn wrap-shadow-fn (arg opt) (let ((decorate (fn (x) x))) (match opt ((:some decorate) (decorate arg)) ((:none) 0)))",
      ),
      ("CallWrapShadowGlobalFn", "def v $ wrap-shadow-fn (%{} P :name |a) (Some 1)"),
    ],
    &[],
  );
  assert!(
    !impl_attach::collect_non_coherent_nominal_defs(&shadowed_global_fn).contains("app.m/P"),
    "match pattern binding shadowing global helper name must block fallthrough in both walker and param sink collector"
  );
}
