use calcit::Calcit;
use calcit::codegen::calx::benchmark::{
  CalxBenchmarkAdapterStage, CalxBenchmarkCorpus, CalxBenchmarkDefinition, CalxBenchmarkReturn, CalxBenchmarkSession,
};
use calcit::codegen::calx::{CalxHostImports, CalxScalarType};

const SOURCE: &str = r#"defn affine-helper (x scale offset)
  &+
    &* x scale
    , offset

defn affine (x scale offset)
  affine-helper x scale offset
"#;

fn scalar_definition(name: &str, arity: usize) -> CalxBenchmarkDefinition {
  CalxBenchmarkDefinition::new(
    name,
    vec![CalxScalarType::F64; arity],
    CalxBenchmarkReturn::Scalar(CalxScalarType::F64),
  )
  .expect("valid scalar definition")
}

#[test]
fn revision_pinned_session_encapsulates_source_runtime_and_strict_calx_paths() {
  let corpus = CalxBenchmarkCorpus::new(
    "bench.adapter-contract",
    SOURCE,
    vec![scalar_definition("affine-helper", 3), scalar_definition("affine", 3)],
  )
  .expect("valid explicit corpus");
  let preparation = CalxBenchmarkSession::prepare(corpus).expect("prepare immutable benchmark session");
  let timings = preparation.timings();
  assert!(timings.source_install <= timings.source_install + timings.preprocess + timings.snapshot_clone);
  let session = preparation.into_session();

  let args = vec![Calcit::Number(4.0), Calcit::Number(1.5), Calcit::Number(2.0)];
  let lookup = session.run_calcit_lookup("affine", &args).expect("run lookup path");
  let callable = session.resolve_calcit_callable("affine").expect("resolve cached callable");
  let cached = callable.run(&args).expect("run cached callable");
  assert_eq!(cached, lookup);

  let (kernel, compile_timings) = session
    .compile_calx_measured("affine", &CalxHostImports::new())
    .expect("compile measured Calx kernel");
  assert!(compile_timings.total >= compile_timings.eligibility);
  let vm_args = session
    .prepare_calx_arguments("affine", &kernel, &args)
    .expect("prepare strict scalar arguments");
  let mut vm = kernel.instantiate().expect("instantiate Calx VM");
  let vm_result = vm.run_typed(vm_args).expect("execute Calx kernel");
  let calx = session
    .finish_calx_result("affine", &kernel, vm_result)
    .expect("finish strict result boundary");
  assert_eq!(calx, lookup);

  let counts = session.program_counts("affine", &kernel).expect("stable program counts");
  assert_eq!(counts.functions, 2);
  assert_eq!(counts.imports, 0);
  assert!(counts.syntax_nodes > 0);
  assert!(counts.instructions > 0);
  assert!(counts.diagnostic_bytes > 0);

  let boundary_error = callable
    .run(&[Calcit::Nil, Calcit::Number(1.5), Calcit::Number(2.0)])
    .expect_err("Nil must not cross a strict scalar boundary");
  assert_eq!(boundary_error.stage(), CalxBenchmarkAdapterStage::Boundary);
  drop(session);

  let undeclared_source = CalxBenchmarkCorpus::new("bench.adapter-contract-invalid", SOURCE, vec![scalar_definition("affine", 3)])
    .expect("constructor accepts a schema table before parsing source");
  let corpus_error = CalxBenchmarkSession::prepare(undeclared_source)
    .err()
    .expect("every source definition must have an explicit schema");
  assert_eq!(corpus_error.stage(), CalxBenchmarkAdapterStage::Corpus);

  let duplicate_error = CalxBenchmarkCorpus::new(
    "bench.adapter-contract-duplicate",
    SOURCE,
    vec![scalar_definition("affine", 3), scalar_definition("affine", 3)],
  )
  .expect_err("duplicate schema declarations must fail");
  assert_eq!(duplicate_error.stage(), CalxBenchmarkAdapterStage::Corpus);
}

#[test]
fn benchmark_runner_consumes_only_the_session_adapter() {
  let runner = include_str!("../src/bin/calx_bench.rs");
  for forbidden in [
    "PROGRAM_CODE_DATA",
    "ProgramFileData",
    "ensure_def_id",
    "run_fn",
    "clone_existing_compiled_program",
    "run_program_with_docs",
  ] {
    assert!(
      !runner.contains(forbidden),
      "runner must not reach forbidden core symbol `{forbidden}`"
    );
  }
}
