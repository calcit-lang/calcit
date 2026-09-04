use super::*;
use crate::calcit::data_shape::{DataShapeGraph, DataShapeNode};
use crate::calcit::{CalcitFnTypeAnnotation, CalcitImpl, CalcitImport, CalcitStructDef, CalcitSyntax, ImportInfo, SchemaKind};
use crate::call_stack::CallStackList;
use crate::codegen::calx::{
  CalxCacheMissReason, CalxCompileCache, CalxDefinitionRef, CalxError, CalxFallbackCode, CalxHostImport, CalxHostImports,
  CalxKernelBoundaryErrorKind, CalxKernelCompileError, CalxKernelRunError, CalxScalarType, CalxValue, analyze_calx_eligibility,
  analyze_calx_eligibility_with_imports, compile_calx_kernel, compile_calx_kernel_measured, compile_calx_kernel_with_imports,
};
use crate::data::cirru::code_to_calcit;
use crate::run_program_with_docs;
use cirru_edn::EdnTag;
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
static CALX_VOID_IMPORT_CALLS: AtomicUsize = AtomicUsize::new(0);

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
      nominal_path: Some((ns.clone(), def.clone())),
      type_args: Arc::new(vec![]),
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
  let tagged_impl = Calcit::Impl(CalcitImpl {
    name: EdnTag::new("z-owner"),
    origin: None,
    fields: Arc::new(vec![]),
    values: Arc::new(vec![]),
  });
  assert_eq!(
    find_source_def_for_runtime_value("tests.runtime-provenance", &tagged_impl).as_deref(),
    Some("z-owner"),
    "an impl tag identifies its source owner even before that owner is Ready"
  );
  assert_eq!(find_source_def_for_runtime_value("tests.missing", &target), None);
}

#[test]
fn nominal_impl_wrapper_type_ref_matches_its_concrete_enum_value() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  calcit::register_program_lookups(lookup_runtime_ready, lookup_def_code, lookup_def_schema);

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

  assert!(wrapper_ref.matches_annotation(&concrete));
  assert!(concrete.matches_annotation(&wrapper_ref));
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
    cirru_list(vec![cirru_leaf("[]"), cirru_leaf("add"), cirru_leaf("add")]),
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
    import_rule(
      "audit.math",
      ":refer",
      cirru_list(vec![cirru_leaf("[]"), cirru_leaf("add"), cirru_leaf("subtract")]),
    ),
    import_rule("|chalk", ":default", cirru_leaf("chalk")),
  ];

  assert!(
    validate_import_rules(&rules)
      .expect("supported import rules should pass validation")
      .is_empty()
  );
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
fn namespace_validation_accepts_legacy_colon_ns_form() {
  let ns_form = cirru_list(vec![cirru_leaf(":ns"), cirru_leaf("app.main")]);

  let imports = extract_import_map(&ns_form, "app.main").expect("legacy :ns form should remain compatible");
  assert!(imports.is_empty());
}

fn lock_program_test_state() -> ProgramTestStateGuard {
  super::lock_program_test_state()
}

fn reset_program_test_state() {
  PROGRAM_RUNTIME_DATA_STATE.write().expect("reset runtime data").clear();
  PROGRAM_COMPILED_DATA_STATE.write().expect("reset compiled data").clear();
  PROGRAM_CODE_DATA.write().expect("reset program code").clear();
  *PROGRAM_DEF_ID_INDEX.write().expect("reset def id index") = ProgramDefIdIndex::default();
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

fn calx_test_fn_schema(arg_types: Vec<CalcitTypeAnnotation>, return_type: CalcitTypeAnnotation) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
    generics: Arc::new(vec![]),
    where_bounds: Arc::new(vec![]),
    arg_types: arg_types.into_iter().map(Arc::new).collect(),
    return_type: Arc::new(return_type),
    fn_kind: SchemaKind::Fn,
    rest_type: None,
    features: Arc::new(HashSet::new()),
  })))
}

fn calx_test_defs_from_source(namespace: &str, source: &str) -> HashMap<String, Calcit> {
  cirru_parser::parse(source)
    .expect("parse Calx source fixture")
    .into_iter()
    .map(|node| {
      let Cirru::List(items) = &node else {
        panic!("Calx fixture definition must be a list: {node}");
      };
      let Some(Cirru::Leaf(definition)) = items.get(1) else {
        panic!("Calx fixture definition must have a name: {node}");
      };
      let code = code_to_calcit(&node, namespace, definition, vec![]).expect("convert Calx fixture source");
      (definition.to_string(), code)
    })
    .collect()
}

fn install_calx_test_defs(namespace: &str, defs: Vec<(&str, Calcit, Arc<CalcitTypeAnnotation>)>) {
  let mut entries = HashMap::new();
  for (definition, code, schema) in defs {
    let _ = ensure_def_id(namespace, definition);
    entries.insert(
      Arc::from(definition),
      ProgramDefEntry {
        code,
        schema,
        doc: Arc::from(""),
        examples: vec![],
        ffi: None,
      },
    );
  }
  PROGRAM_CODE_DATA.write().expect("install Calx eligibility fixtures").insert(
    Arc::from(namespace),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: entries,
    },
  );
}

fn compile_calx_test_entry(namespace: &str, definition: &str) {
  let warnings = RefCell::new(vec![]);
  crate::runner::preprocess::ensure_ns_def_compiled(namespace, definition, &warnings, &CallStackList::default())
    .unwrap_or_else(|error| panic!("preprocess Calx fixture {namespace}/{definition}: {error}"));
  assert!(
    warnings.borrow().is_empty(),
    "unexpected fixture warnings: {:#?}",
    warnings.borrow()
  );
}

fn install_calx_scalar_kernel_fixture(namespace: &str) {
  let number = || CalcitTypeAnnotation::Number;
  let mut source_defs = calx_test_defs_from_source(namespace, include_str!("../../tests/fixtures/calx/scalar-kernels.cirru"));
  install_calx_test_defs(
    namespace,
    vec![
      (
        "range-sum",
        source_defs.remove("range-sum").expect("range-sum source"),
        calx_test_fn_schema(vec![number(), number()], number()),
      ),
      (
        "fibonacci",
        source_defs.remove("fibonacci").expect("fibonacci source"),
        calx_test_fn_schema(vec![number()], number()),
      ),
      (
        "affine-helper",
        source_defs.remove("affine-helper").expect("affine-helper source"),
        calx_test_fn_schema(vec![number(), number(), number()], number()),
      ),
      (
        "affine",
        source_defs.remove("affine").expect("affine source"),
        calx_test_fn_schema(vec![number(), number(), number()], number()),
      ),
      (
        "polynomial",
        source_defs.remove("polynomial").expect("polynomial source"),
        calx_test_fn_schema(vec![number()], number()),
      ),
      (
        "bounded-simulation",
        source_defs.remove("bounded-simulation").expect("bounded-simulation source"),
        calx_test_fn_schema(vec![number(), number(), number()], number()),
      ),
    ],
  );
  for definition in ["range-sum", "fibonacci", "affine", "polynomial", "bounded-simulation"] {
    compile_calx_test_entry(namespace, definition);
  }
}

#[test]
fn calx_eligibility_accepts_five_real_preprocessed_scalar_kernels() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let namespace = "tests.calx-kernels";
  install_calx_scalar_kernel_fixture(namespace);
  let snapshot = clone_compiled_program_snapshot().expect("clone typed preprocessed Calx fixtures");

  let range = analyze_calx_eligibility(&snapshot, namespace, "range-sum").expect("range-sum should be eligible");
  assert_eq!(range.functions.len(), 1);
  assert_eq!(range.functions[0].params, vec![CalxScalarType::F64, CalxScalarType::F64]);
  assert_eq!(range.functions[0].result, Some(CalxScalarType::F64));

  let fibonacci = analyze_calx_eligibility(&snapshot, namespace, "fibonacci").expect("fibonacci should be eligible");
  assert_eq!(fibonacci.functions.len(), 1);
  assert_eq!(
    fibonacci.functions[0].direct_calls,
    vec![CalxDefinitionRef::new(namespace, "fibonacci")]
  );

  let affine = analyze_calx_eligibility(&snapshot, namespace, "affine").expect("affine should be eligible");
  assert_eq!(
    affine
      .functions
      .iter()
      .map(|function| function.definition.definition.as_ref())
      .collect::<Vec<_>>(),
    vec!["affine", "affine-helper"]
  );
  let polynomial = analyze_calx_eligibility(&snapshot, namespace, "polynomial").expect("polynomial should be eligible");
  assert_eq!(polynomial.functions.len(), 1);

  let bounded_simulation =
    analyze_calx_eligibility(&snapshot, namespace, "bounded-simulation").expect("bounded-simulation should be eligible");
  assert_eq!(bounded_simulation.functions.len(), 1);
  let summary = format!(
    "## range-sum\n{}## fibonacci\n{}## affine\n{}## polynomial\n{}## bounded-simulation\n{}",
    range.stable_summary(),
    fibonacci.stable_summary(),
    affine.stable_summary(),
    polynomial.stable_summary(),
    bounded_simulation.stable_summary()
  );
  assert_eq!(summary, include_str!("../../tests/fixtures/calx/scalar-kernels.golden.txt"));
}

#[test]
fn calx_lowering_executes_five_source_kernels_like_native_calcit() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let namespace = "tests.calx-kernels";
  install_calx_scalar_kernel_fixture(namespace);
  let snapshot = clone_compiled_program_snapshot().expect("clone typed preprocessed Calx fixtures");

  let cases = [
    ("range-sum", vec![Calcit::Number(10.0), Calcit::Number(0.0)]),
    ("fibonacci", vec![Calcit::Number(10.0)]),
    ("affine", vec![Calcit::Number(3.0), Calcit::Number(4.0), Calcit::Number(5.0)]),
    ("polynomial", vec![Calcit::Number(3.0)]),
    (
      "bounded-simulation",
      vec![Calcit::Number(10.0), Calcit::Number(0.5), Calcit::Number(0.99)],
    ),
  ];
  for (definition, args) in cases {
    let kernel = compile_calx_kernel(&snapshot, namespace, definition)
      .unwrap_or_else(|error| panic!("compile Calx kernel {namespace}/{definition}: {error}"));
    assert_eq!(kernel.validated_program().functions()[0].name.as_ref(), "main");
    assert!(
      kernel
        .validated_program()
        .functions()
        .iter()
        .all(|function| !function.instrs.is_empty())
    );

    let calx_result = kernel
      .run(&args)
      .unwrap_or_else(|error| panic!("run Calx kernel {namespace}/{definition}: {error}"));
    let native_result = run_program_with_docs(Arc::from(namespace), Arc::from(definition), &args)
      .unwrap_or_else(|error| panic!("run native Calcit kernel {namespace}/{definition}: {error}"));
    assert_eq!(calx_result, native_result, "Calx/native mismatch for {definition}");
  }

  let range = compile_calx_kernel(&snapshot, namespace, "range-sum").expect("compile range-sum boundary fixture");
  let error = range
    .run(&[Calcit::Nil, Calcit::Number(0.0)])
    .expect_err("Nil must not cross the strict Calx boundary");
  assert!(matches!(
    error,
    CalxKernelRunError::Boundary(ref boundary) if boundary.kind == CalxKernelBoundaryErrorKind::ArgumentType
  ));
}

fn install_calx_f64_buffer_kernel_fixture(namespace: &str) {
  let mut source_defs = calx_test_defs_from_source(namespace, include_str!("../../tests/fixtures/calx/f64-buffer-kernel.cirru"));
  install_calx_test_defs(
    namespace,
    vec![(
      "dot-product",
      source_defs.remove("dot-product").expect("dot-product source"),
      calx_test_fn_schema(
        vec![
          CalcitTypeAnnotation::F64Buffer,
          CalcitTypeAnnotation::F64Buffer,
          CalcitTypeAnnotation::Number,
          CalcitTypeAnnotation::Number,
        ],
        CalcitTypeAnnotation::Number,
      ),
    )],
  );
  compile_calx_test_entry(namespace, "dot-product");
}

#[test]
fn calx_f64_buffer_dot_product_is_source_backed_strict_and_differential() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let namespace = "tests.calx-f64-buffer";
  install_calx_f64_buffer_kernel_fixture(namespace);
  let snapshot = clone_compiled_program_snapshot().expect("clone typed F64Buffer fixture");

  let graph = analyze_calx_eligibility(&snapshot, namespace, "dot-product").expect("F64Buffer kernel should be eligible");
  assert_eq!(graph.abi_edition.as_ref(), "calcit-calx-kernel/2");
  assert_eq!(
    graph.functions[0].params,
    vec![
      CalxScalarType::F64Buffer,
      CalxScalarType::F64Buffer,
      CalxScalarType::F64,
      CalxScalarType::F64,
    ]
  );

  let args = vec![
    Calcit::F64Buffer(Arc::from([1.0, 2.0, 3.0])),
    Calcit::F64Buffer(Arc::from([4.0, 5.0, 6.0])),
    Calcit::Number(2.0),
    Calcit::Number(0.0),
  ];
  let kernel = compile_calx_kernel(&snapshot, namespace, "dot-product").expect("compile strict F64Buffer kernel");
  assert_eq!(
    kernel.stable_program_summary(),
    include_str!("../../tests/fixtures/calx/f64-buffer-kernel.golden.txt")
  );
  let calx_result = kernel.run(&args).expect("run strict F64Buffer kernel");
  let native_result =
    run_program_with_docs(Arc::from(namespace), Arc::from("dot-product"), &args).expect("run native F64Buffer kernel");
  assert_eq!(calx_result, Calcit::Number(32.0));
  assert_eq!(calx_result, native_result);
  assert!(kernel.stable_program_summary().contains("F64BufferGet"));

  for invalid in [Calcit::Nil, Calcit::from(vec![Calcit::Number(1.0)]), Calcit::Buffer(vec![0, 1])] {
    let error = kernel
      .run(&[invalid, args[1].clone(), Calcit::Number(2.0), Calcit::Number(0.0)])
      .expect_err("only concrete F64Buffer crosses the strict boundary");
    assert!(matches!(error, CalxKernelRunError::Boundary(ref boundary) if boundary.kind == CalxKernelBoundaryErrorKind::ArgumentType));
  }

  let trap = kernel
    .run(&[args[0].clone(), args[1].clone(), Calcit::Number(3.0), Calcit::Number(0.0)])
    .expect_err("out-of-bounds buffer access must trap without native retry");
  assert!(matches!(trap, CalxKernelRunError::Runtime(_)));
}

#[test]
fn calx_measured_compile_reports_non_overlapping_stages() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let namespace = "tests.calx-kernels";
  install_calx_scalar_kernel_fixture(namespace);
  let snapshot = clone_compiled_program_snapshot().expect("clone typed preprocessed Calx fixtures");

  let (kernel, timings) = compile_calx_kernel_measured(&snapshot, namespace, "range-sum").expect("measure range-sum compilation");
  let measured_stages = timings.eligibility + timings.planning + timings.program_construction + timings.validation_lowering;
  assert!(timings.total >= measured_stages);
  assert_eq!(kernel.graph().entry, CalxDefinitionRef::new(namespace, "range-sum"));
}

fn install_calx_typed_import_fixture(namespace: &str) {
  let number = || CalcitTypeAnnotation::Number;
  let mut source_defs = calx_test_defs_from_source(namespace, include_str!("../../tests/fixtures/calx/typed-imports.cirru"));
  install_calx_test_defs(
    namespace,
    vec![
      (
        "host-scale",
        source_defs.remove("host-scale").expect("host-scale source"),
        calx_test_fn_schema(vec![number()], number()),
      ),
      (
        "host-observe",
        source_defs.remove("host-observe").expect("host-observe source"),
        calx_test_fn_schema(vec![number()], CalcitTypeAnnotation::Unit),
      ),
      (
        "host-trap",
        source_defs.remove("host-trap").expect("host-trap source"),
        calx_test_fn_schema(vec![number()], number()),
      ),
      (
        "imported-pipeline",
        source_defs.remove("imported-pipeline").expect("imported-pipeline source"),
        calx_test_fn_schema(vec![number()], number()),
      ),
      (
        "imported-trap",
        source_defs.remove("imported-trap").expect("imported-trap source"),
        calx_test_fn_schema(vec![number()], number()),
      ),
    ],
  );
  for definition in ["imported-pipeline", "imported-trap"] {
    compile_calx_test_entry(namespace, definition);
  }
}

fn calx_test_scale(args: &[CalxValue]) -> Result<CalxValue, CalxError> {
  let [CalxValue::F64(value)] = args else {
    return Err(CalxError::new_raw("fixture scale expected one F64".to_owned()));
  };
  Ok(CalxValue::F64(value * 2.0))
}

fn calx_test_scale_three(args: &[CalxValue]) -> Result<CalxValue, CalxError> {
  let [CalxValue::F64(value)] = args else {
    return Err(CalxError::new_raw("fixture scale expected one F64".to_owned()));
  };
  Ok(CalxValue::F64(value * 3.0))
}

fn calx_test_observe(args: &[CalxValue]) -> Result<(), CalxError> {
  let [CalxValue::F64(_)] = args else {
    return Err(CalxError::new_raw("fixture observe expected one F64".to_owned()));
  };
  CALX_VOID_IMPORT_CALLS.fetch_add(1, Ordering::SeqCst);
  Ok(())
}

fn calx_test_trap(_args: &[CalxValue]) -> Result<CalxValue, CalxError> {
  Err(CalxError::new_raw("fixture host trap".to_owned()))
}

fn calx_test_host_imports(namespace: &str) -> CalxHostImports {
  calx_test_host_imports_with_scale(namespace, "fixture.scale", calx_test_scale)
}

fn calx_test_host_imports_with_scale(
  namespace: &str,
  export_name: &str,
  callback: fn(&[CalxValue]) -> Result<CalxValue, CalxError>,
) -> CalxHostImports {
  let mut imports = CalxHostImports::new();
  imports.insert(
    CalxDefinitionRef::new(namespace, "host-scale"),
    CalxHostImport::value(export_name, vec![CalxScalarType::F64], CalxScalarType::F64, callback).expect("valid value host import"),
  );
  imports.insert(
    CalxDefinitionRef::new(namespace, "host-observe"),
    CalxHostImport::void("fixture.observe", vec![CalxScalarType::F64], calx_test_observe).expect("valid void host import"),
  );
  imports.insert(
    CalxDefinitionRef::new(namespace, "host-trap"),
    CalxHostImport::value("fixture.trap", vec![CalxScalarType::F64], CalxScalarType::F64, calx_test_trap)
      .expect("valid trapping host import"),
  );
  imports
}

#[test]
fn calx_compile_cache_hits_reuse_artifacts_but_reattach_current_callbacks() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let namespace = "tests.calx-cache-imports";
  install_calx_typed_import_fixture(namespace);
  let snapshot = clone_compiled_program_snapshot().expect("clone typed import cache fixture");
  let imports_a = calx_test_host_imports_with_scale(namespace, "fixture.scale", calx_test_scale);
  let imports_b = calx_test_host_imports_with_scale(namespace, "fixture.scale", calx_test_scale_three);
  let mut cache = CalxCompileCache::new(2);

  let first = cache
    .prepare(&snapshot, namespace, "imported-pipeline", &imports_a)
    .expect("compile initial cached artifact");
  assert_eq!(first.report().miss_reason, Some(CalxCacheMissReason::Empty));
  assert!(!first.report().cache_hit);
  assert_eq!(
    first.kernel().run(&[Calcit::Number(3.0)]).expect("run callback A"),
    Calcit::Number(7.0)
  );
  let first_artifact = first.kernel().artifact().clone();

  let second = cache
    .prepare(&snapshot, namespace, "imported-pipeline", &imports_b)
    .expect("hit artifact with callback B");
  assert!(second.report().cache_hit);
  assert!(second.report().miss_reason.is_none());
  assert!(second.report().skipped_eligibility);
  assert!(second.report().skipped_planning);
  assert!(second.report().skipped_program_construction);
  assert!(second.report().skipped_validation_lowering);
  assert!(std::rc::Rc::ptr_eq(&first_artifact, second.kernel().artifact()));
  assert_eq!(
    second.kernel().run(&[Calcit::Number(3.0)]).expect("run callback B"),
    Calcit::Number(10.0),
    "cache hit must use the current callback rather than stale capability state"
  );

  let mut host_schema_changed = snapshot.clone();
  host_schema_changed
    .get_mut(namespace)
    .expect("fixture namespace")
    .defs
    .get_mut("host-scale")
    .expect("host import definition")
    .schema = calx_test_fn_schema(vec![CalcitTypeAnnotation::Bool], CalcitTypeAnnotation::Number);
  assert!(matches!(
    cache.prepare(&host_schema_changed, namespace, "imported-pipeline", &imports_b),
    Err(CalxKernelCompileError::Eligibility(_))
  ));

  let changed_contract = calx_test_host_imports_with_scale(namespace, "fixture.scale.v2", calx_test_scale_three);
  let third = cache
    .prepare(&snapshot, namespace, "imported-pipeline", &changed_contract)
    .expect("changed import declaration recompiles");
  assert_eq!(third.report().miss_reason, Some(CalxCacheMissReason::ImportContractChanged));
  assert!(!std::rc::Rc::ptr_eq(&first_artifact, third.kernel().artifact()));

  let stats = cache.stats();
  assert_eq!(stats.hits, 1);
  assert_eq!(stats.misses, 3);
  assert_eq!(stats.miss_count(CalxCacheMissReason::Empty), 1);
  assert_eq!(stats.miss_count(CalxCacheMissReason::SchemaChanged), 1);
  assert_eq!(stats.miss_count(CalxCacheMissReason::ImportContractChanged), 1);
  assert_eq!(stats.entry_count, 2);
  assert!(stats.reachable_function_count >= 2);
  assert!(stats.syntax_instruction_count > 0);
  assert!(stats.lowered_instruction_count > 0);
  assert!(stats.estimated_bytes > 0);
}

#[test]
fn calx_compile_cache_validates_reachable_revisions_without_global_invalidation() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let namespace = "tests.calx-cache-revisions";
  install_calx_scalar_kernel_fixture(namespace);
  let snapshot = clone_compiled_program_snapshot().expect("clone revision cache fixture");
  let imports = CalxHostImports::new();
  let mut cache = CalxCompileCache::new(2);

  let initial = cache.prepare(&snapshot, namespace, "affine", &imports).expect("cache affine");
  assert_eq!(initial.report().miss_reason, Some(CalxCacheMissReason::Empty));
  let initial_artifact = initial.kernel().artifact().clone();

  let mut unrelated = snapshot.clone();
  unrelated
    .get_mut(namespace)
    .expect("fixture namespace")
    .defs
    .get_mut("polynomial")
    .expect("unrelated definition")
    .def_id
    .0 += 10_000;
  let unrelated_hit = cache
    .prepare(&unrelated, namespace, "affine", &imports)
    .expect("unrelated change must preserve hit");
  assert!(unrelated_hit.report().cache_hit);
  assert!(std::rc::Rc::ptr_eq(&initial_artifact, unrelated_hit.kernel().artifact()));

  let mut entry_changed = unrelated.clone();
  entry_changed
    .get_mut(namespace)
    .expect("fixture namespace")
    .defs
    .get_mut("affine")
    .expect("entry definition")
    .def_id
    .0 += 20_000;
  let entry_miss = cache
    .prepare(&entry_changed, namespace, "affine", &imports)
    .expect("entry replacement recompiles");
  assert_eq!(entry_miss.report().miss_reason, Some(CalxCacheMissReason::EntryChanged));

  let mut callee_changed = entry_changed.clone();
  callee_changed
    .get_mut(namespace)
    .expect("fixture namespace")
    .defs
    .get_mut("affine-helper")
    .expect("callee definition")
    .def_id
    .0 += 30_000;
  let callee_miss = cache
    .prepare(&callee_changed, namespace, "affine", &imports)
    .expect("callee replacement recompiles");
  assert_eq!(callee_miss.report().miss_reason, Some(CalxCacheMissReason::CalleeChanged));

  let mut schema_changed = callee_changed.clone();
  schema_changed
    .get_mut(namespace)
    .expect("fixture namespace")
    .defs
    .get_mut("affine-helper")
    .expect("callee definition")
    .schema = DYNAMIC_TYPE.clone();
  assert!(matches!(
    cache.prepare(&schema_changed, namespace, "affine", &imports),
    Err(CalxKernelCompileError::Eligibility(_))
  ));

  let mut dependency_missing = callee_changed;
  dependency_missing
    .get_mut(namespace)
    .expect("fixture namespace")
    .defs
    .remove("affine-helper");
  assert!(matches!(
    cache.prepare(&dependency_missing, namespace, "affine", &imports),
    Err(CalxKernelCompileError::Eligibility(_))
  ));

  let stats = cache.stats();
  assert_eq!(stats.hits, 1);
  assert_eq!(stats.miss_count(CalxCacheMissReason::EntryChanged), 1);
  assert_eq!(stats.miss_count(CalxCacheMissReason::CalleeChanged), 1);
  assert_eq!(stats.miss_count(CalxCacheMissReason::SchemaChanged), 1);
  assert_eq!(stats.miss_count(CalxCacheMissReason::DependencyMissing), 1);
  assert_eq!(stats.entry_count, 1, "failed recompilation must not cache a partial artifact");
}

#[test]
fn calx_compile_cache_bounds_lru_and_eviction_provenance() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let namespace = "tests.calx-cache-lru";
  install_calx_scalar_kernel_fixture(namespace);
  let snapshot = clone_compiled_program_snapshot().expect("clone LRU cache fixture");
  let imports = CalxHostImports::new();
  let mut cache = CalxCompileCache::new(1);

  cache.prepare(&snapshot, namespace, "range-sum", &imports).expect("insert A");
  cache
    .prepare(&snapshot, namespace, "fibonacci", &imports)
    .expect("insert B and evict A");
  assert_eq!(cache.stats().entry_count, 1);
  assert_eq!(cache.stats().recently_evicted_count, 1);
  assert_eq!(cache.stats().evictions, 1);

  cache
    .prepare(&snapshot, namespace, "polynomial", &imports)
    .expect("insert C and overflow ledger");
  let forgotten = cache
    .prepare(&snapshot, namespace, "range-sum", &imports)
    .expect("oldest tombstone must become empty");
  assert_eq!(forgotten.report().miss_reason, Some(CalxCacheMissReason::Empty));

  let evicted = cache
    .prepare(&snapshot, namespace, "polynomial", &imports)
    .expect("most recent tombstone must remain observable");
  assert_eq!(evicted.report().miss_reason, Some(CalxCacheMissReason::Evicted));
  assert_eq!(cache.stats().entry_count, 1);
  assert!(cache.stats().evictions >= 4);
  assert_eq!(cache.stats().miss_count(CalxCacheMissReason::Evicted), 1);

  cache.clear();
  let after_clear = cache
    .prepare(&snapshot, namespace, "range-sum", &imports)
    .expect("clear removes artifacts and tombstones");
  assert_eq!(after_clear.report().miss_reason, Some(CalxCacheMissReason::Empty));
  assert_eq!(cache.stats().clears, 1);

  let mut disabled = CalxCompileCache::new(0);
  for _ in 0..2 {
    let preparation = disabled
      .prepare(&snapshot, namespace, "range-sum", &imports)
      .expect("zero-capacity cache compiles without storing");
    assert_eq!(preparation.report().miss_reason, Some(CalxCacheMissReason::Empty));
  }
  assert_eq!(disabled.stats().entry_count, 0);
  assert_eq!(disabled.stats().misses, 2);

  let mut lru = CalxCompileCache::new(2);
  lru.prepare(&snapshot, namespace, "range-sum", &imports).expect("insert LRU A");
  lru.prepare(&snapshot, namespace, "fibonacci", &imports).expect("insert LRU B");
  assert!(
    lru
      .prepare(&snapshot, namespace, "range-sum", &imports)
      .expect("touch LRU A")
      .report()
      .cache_hit
  );
  lru.prepare(&snapshot, namespace, "polynomial", &imports).expect("insert LRU C");
  let evicted_b = lru
    .prepare(&snapshot, namespace, "fibonacci", &imports)
    .expect("least recently used B was evicted");
  assert_eq!(evicted_b.report().miss_reason, Some(CalxCacheMissReason::Evicted));
}

#[test]
fn calx_typed_imports_cover_void_value_and_generated_program_golden() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let namespace = "tests.calx-imports";
  install_calx_typed_import_fixture(namespace);
  let snapshot = clone_compiled_program_snapshot().expect("clone typed import fixtures");
  let imports = calx_test_host_imports(namespace);

  let graph = analyze_calx_eligibility_with_imports(&snapshot, namespace, "imported-pipeline", &imports)
    .expect("explicit typed imports should be eligible");
  assert_eq!(graph.functions.len(), 1, "approved imports must not expand the guest call graph");
  assert_eq!(
    graph.functions[0].host_imports,
    vec![
      CalxDefinitionRef::new(namespace, "host-observe"),
      CalxDefinitionRef::new(namespace, "host-scale"),
    ]
  );

  let kernel =
    compile_calx_kernel_with_imports(&snapshot, namespace, "imported-pipeline", &imports).expect("compile typed import fixture");
  assert_eq!(
    kernel.stable_program_summary(),
    include_str!("../../tests/fixtures/calx/generated-program.golden.txt")
  );
  assert_eq!(kernel.validated_program().imports().len(), 2);
  assert!(kernel.validated_program().imports().iter().any(|import| import.result.is_none()));
  assert!(kernel.validated_program().imports().iter().any(|import| import.result.is_some()));

  CALX_VOID_IMPORT_CALLS.store(0, Ordering::SeqCst);
  let args = [Calcit::Number(3.0)];
  let calx_result = kernel.run(&args).expect("run typed import fixture");
  let native_result =
    run_program_with_docs(Arc::from(namespace), Arc::from("imported-pipeline"), &args).expect("run native typed import fixture");
  assert_eq!(calx_result, native_result);
  assert_eq!(calx_result, Calcit::Number(7.0));
  assert_eq!(CALX_VOID_IMPORT_CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn calx_typed_import_trap_is_stable_and_never_becomes_fallback() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let namespace = "tests.calx-imports";
  install_calx_typed_import_fixture(namespace);
  let snapshot = clone_compiled_program_snapshot().expect("clone typed import trap fixture");
  let imports = calx_test_host_imports(namespace);
  let kernel =
    compile_calx_kernel_with_imports(&snapshot, namespace, "imported-trap", &imports).expect("compile trapping typed import fixture");

  let error = kernel
    .run(&[Calcit::Number(2.0)])
    .expect_err("host callback failure must remain a Calx runtime trap");
  let summary = format!("{error}\n");
  assert_eq!(summary, include_str!("../../tests/fixtures/calx/trap.golden.txt"));
  let CalxKernelRunError::Runtime(runtime) = error else {
    panic!("host callback failure must not become boundary/fallback: {error}");
  };
  assert_eq!(runtime.message, "fixture host trap");
  assert!(runtime.snapshot.is_none(), "host errors must not invent VM state");
}

#[test]
fn calx_typed_import_signature_mismatch_falls_back_before_lowering() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let namespace = "tests.calx-imports";
  install_calx_typed_import_fixture(namespace);
  let snapshot = clone_compiled_program_snapshot().expect("clone typed import mismatch fixture");
  let mut imports = calx_test_host_imports(namespace);
  imports.insert(
    CalxDefinitionRef::new(namespace, "host-scale"),
    CalxHostImport::value("fixture.scale", vec![CalxScalarType::Bool], CalxScalarType::F64, calx_test_scale)
      .expect("construct deliberately mismatched import"),
  );

  let report = analyze_calx_eligibility_with_imports(&snapshot, namespace, "imported-pipeline", &imports)
    .expect_err("typed import signature mismatch must reject the whole kernel");
  assert!(report.issues.iter().any(|issue| issue.code == CalxFallbackCode::HostCapability));
  assert!(matches!(
    compile_calx_kernel_with_imports(&snapshot, namespace, "imported-pipeline", &imports),
    Err(CalxKernelCompileError::Eligibility(_))
  ));
}

#[test]
fn calx_eligibility_falls_back_for_the_whole_reachable_closure() {
  let _guard = lock_program_test_state();
  reset_program_test_state();
  let namespace = "tests.calx-fallback";
  let mut source_defs = calx_test_defs_from_source(namespace, include_str!("../../tests/fixtures/calx/fallback.cirru"));
  install_calx_test_defs(
    namespace,
    vec![
      (
        "dynamic-helper",
        source_defs.remove("dynamic-helper").expect("dynamic-helper source"),
        DYNAMIC_TYPE.clone(),
      ),
      (
        "entry",
        source_defs.remove("entry").expect("entry source"),
        calx_test_fn_schema(vec![CalcitTypeAnnotation::Number], CalcitTypeAnnotation::Number),
      ),
    ],
  );
  compile_calx_test_entry(namespace, "entry");
  let snapshot = clone_compiled_program_snapshot().expect("clone fallback fixture");

  let report = analyze_calx_eligibility(&snapshot, namespace, "entry").expect_err("reachable Dynamic callee must reject closure");
  assert_eq!(
    report.stable_summary(),
    include_str!("../../tests/fixtures/calx/fallback.golden.txt")
  );
  assert!(report.issues.iter().any(|issue| issue.code == CalxFallbackCode::DynamicType));
  assert!(report.issues.iter().any(|issue| issue.code == CalxFallbackCode::CallClosure));
  let dynamic = report
    .issues
    .iter()
    .find(|issue| issue.code == CalxFallbackCode::DynamicType)
    .expect("Dynamic fallback detail");
  assert_eq!(dynamic.call_path.len(), 2);
  assert_eq!(dynamic.call_path[1], CalxDefinitionRef::new(namespace, "dynamic-helper"));
  assert!(matches!(
    compile_calx_kernel(&snapshot, namespace, "entry"),
    Err(CalxKernelCompileError::Eligibility(_))
  ));

  let mut cache = CalxCompileCache::new(2);
  for _ in 0..2 {
    assert!(matches!(
      cache.prepare(&snapshot, namespace, "entry", &CalxHostImports::new()),
      Err(CalxKernelCompileError::Eligibility(_))
    ));
  }
  let stats = cache.stats();
  assert_eq!(stats.entry_count, 0, "negative eligibility results must never be cached");
  assert_eq!(stats.misses, 2);
  assert_eq!(stats.miss_count(CalxCacheMissReason::Empty), 2);
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
  write_runtime_ready("app.traits", "Show", Calcit::Trait(trait_value)).expect("store runtime trait");

  let Calcit::Trait(stored) = lookup_runtime_ready("app.traits", "Show").expect("stored trait") else {
    panic!("expected stored trait");
  };
  assert_eq!(stored.definition_ref.as_deref(), Some("app.traits/Show"));
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
      changed_defs: HashMap::from([(String::from("demo"), Cirru::Leaf(Arc::from("2")))]),
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
