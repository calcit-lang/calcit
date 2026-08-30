//! Reproducible single-process measurement for one Calcit-to-Calx scalar case.
//!
//! The orchestration script runs this binary in fresh processes to measure
//! process-level cold cost without mixing benchmark policy into the compiler.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};

use argh::FromArgs;
use calcit::calcit::{CalcitFnTypeAnnotation, CalcitTypeAnnotation, SchemaKind};
use calcit::call_stack::CallStackList;
use calcit::codegen::calx::{CalxCompiledKernel, CalxScalarType, CalxValue, compile_calx_kernel_measured};
use calcit::data::cirru::code_to_calcit;
use calcit::program::{PROGRAM_CODE_DATA, ProgramDefEntry, ProgramFileData, clone_existing_compiled_program, ensure_def_id};
use calcit::{Calcit, run_program_with_docs};
use calx_vm::CalxRunResult;
use cirru_parser::Cirru;
use serde::Serialize;

const FIXTURE_NAMESPACE: &str = "bench.calx-kernels";
const CALX_VM_VERSION: &str = "0.3.0";

#[derive(Debug, FromArgs)]
/// measure one source-backed Calcit-to-Calx scalar kernel
struct Args {
  /// kernel name: range-sum, fibonacci, affine, polynomial, or bounded-simulation
  #[argh(option, default = "String::from(\"range-sum\")")]
  kernel: String,

  /// numeric input size for this case
  #[argh(option, default = "10")]
  size: u32,

  /// reused-VM calls discarded before hot measurement
  #[argh(option, default = "10")]
  vm_warmup: u32,

  /// reused-VM calls included in hot measurement
  #[argh(option, default = "100")]
  hot_iterations: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvironmentReport {
  package_version: &'static str,
  calx_vm_version: &'static str,
  profile: &'static str,
  os: &'static str,
  architecture: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompileReport {
  eligibility_ns: u64,
  planning_ns: u64,
  program_construction_ns: u64,
  validation_lowering_ns: u64,
  total_ns: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeReport {
  native_call_ns: u64,
  boundary_arguments_ns: u64,
  vm_setup_ns: u64,
  pure_execution_ns: u64,
  boundary_result_ns: u64,
  calx_one_shot_ns: u64,
  hot_execution_total_ns: u64,
  hot_execution_per_call_ns: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgramReport {
  functions: usize,
  imports: usize,
  syntax_nodes: usize,
  instructions: usize,
  diagnostic_bytes: usize,
  host_boundary_calls_per_execution: u32,
  reuses_vm_frames_and_stack: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkReport {
  schema: &'static str,
  environment: EnvironmentReport,
  kernel: String,
  workload: &'static str,
  size: u32,
  vm_warmup: u32,
  hot_iterations: u32,
  fixture_install_ns: u64,
  calcit_frontend_ns: u64,
  snapshot_clone_ns: u64,
  compile: CompileReport,
  runtime: RuntimeReport,
  program: ProgramReport,
  correctness: bool,
}

fn nanos(duration: Duration) -> u64 {
  u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn number_fn_schema(arity: usize) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
    generics: Arc::new(vec![]),
    where_bounds: Arc::new(vec![]),
    arg_types: (0..arity).map(|_| Arc::new(CalcitTypeAnnotation::Number)).collect(),
    return_type: Arc::new(CalcitTypeAnnotation::Number),
    fn_kind: SchemaKind::Fn,
    rest_type: None,
    features: Arc::new(HashSet::new()),
  })))
}

fn install_fixture() -> Result<(), String> {
  let arities = HashMap::from([
    ("range-sum", 2),
    ("fibonacci", 1),
    ("affine-helper", 3),
    ("affine", 3),
    ("polynomial", 1),
    ("bounded-simulation", 3),
  ]);
  let mut definitions = HashMap::new();
  for node in cirru_parser::parse(include_str!("../../tests/fixtures/calx/scalar-kernels.cirru"))? {
    let Cirru::List(items) = &node else {
      return Err(format!("Calx benchmark fixture definition must be a list: {node}"));
    };
    let Some(Cirru::Leaf(definition)) = items.get(1) else {
      return Err(format!("Calx benchmark fixture definition must have a name: {node}"));
    };
    let Some(arity) = arities.get(definition.as_ref()) else {
      return Err(format!("Calx benchmark fixture `{definition}` has no declared signature"));
    };
    let code = code_to_calcit(&node, FIXTURE_NAMESPACE, definition, vec![])?;
    let _ = ensure_def_id(FIXTURE_NAMESPACE, definition);
    definitions.insert(
      Arc::from(definition.as_ref()),
      ProgramDefEntry {
        code,
        schema: number_fn_schema(*arity),
        doc: Arc::from(""),
        examples: vec![],
        ffi: None,
      },
    );
  }
  PROGRAM_CODE_DATA.write().map_err(|error| error.to_string())?.insert(
    Arc::from(FIXTURE_NAMESPACE),
    ProgramFileData {
      import_map: HashMap::new(),
      defs: definitions,
    },
  );
  Ok(())
}

fn kernel_arguments(kernel: &str, size: u32) -> Result<Vec<Calcit>, String> {
  let n = f64::from(size);
  match kernel {
    "range-sum" => Ok(vec![Calcit::Number(n), Calcit::Number(0.0)]),
    "fibonacci" => Ok(vec![Calcit::Number(n)]),
    "affine" => Ok(vec![Calcit::Number(n), Calcit::Number(1.5), Calcit::Number(2.0)]),
    "polynomial" => Ok(vec![Calcit::Number(n)]),
    "bounded-simulation" => Ok(vec![Calcit::Number(n), Calcit::Number(0.5), Calcit::Number(0.999)]),
    _ => Err(format!("unknown Calx benchmark kernel `{kernel}`")),
  }
}

fn convert_arguments(kernel: &CalxCompiledKernel, args: &[Calcit]) -> Result<Vec<CalxValue>, String> {
  if kernel.params().len() != args.len() {
    return Err(format!("expected {} arguments, found {}", kernel.params().len(), args.len()));
  }
  args
    .iter()
    .zip(kernel.params())
    .enumerate()
    .map(|(index, (value, expected))| match (value, expected) {
      (Calcit::Number(value), CalxScalarType::F64) => Ok(CalxValue::F64(*value)),
      (Calcit::Bool(value), CalxScalarType::Bool) => Ok(CalxValue::Bool(*value)),
      _ => Err(format!("argument {index} does not match the proven Calx scalar type")),
    })
    .collect()
}

fn convert_result(kernel: &CalxCompiledKernel, result: CalxRunResult) -> Result<Calcit, String> {
  match (kernel.result(), result) {
    (None, CalxRunResult::Void) => Ok(Calcit::Unit),
    (Some(CalxScalarType::F64), CalxRunResult::Value(CalxValue::F64(value))) => Ok(Calcit::Number(value)),
    (Some(CalxScalarType::Bool), CalxRunResult::Value(CalxValue::Bool(value))) => Ok(Calcit::Bool(value)),
    (expected, actual) => Err(format!("validated result contract {expected:?} produced {actual:?}")),
  }
}

fn measure(args: &Args) -> Result<BenchmarkReport, String> {
  if args.hot_iterations == 0 {
    return Err("--hot-iterations must be greater than zero".to_owned());
  }

  let fixture_started = Instant::now();
  install_fixture()?;
  let fixture_install_ns = nanos(fixture_started.elapsed());

  let frontend_started = Instant::now();
  let warnings = RefCell::new(vec![]);
  calcit::runner::preprocess::ensure_ns_def_compiled(FIXTURE_NAMESPACE, &args.kernel, &warnings, &CallStackList::default())
    .map_err(|error| error.to_string())?;
  if !warnings.borrow().is_empty() {
    return Err(format!("Calx benchmark frontend produced warnings: {:#?}", warnings.borrow()));
  }
  let calcit_frontend_ns = nanos(frontend_started.elapsed());

  let snapshot_started = Instant::now();
  let snapshot = clone_existing_compiled_program();
  let snapshot_clone_ns = nanos(snapshot_started.elapsed());

  let (kernel, compile_timings) =
    compile_calx_kernel_measured(&snapshot, FIXTURE_NAMESPACE, args.kernel.as_str()).map_err(|error| error.to_string())?;
  let calcit_args = kernel_arguments(&args.kernel, args.size)?;

  let native_started = Instant::now();
  let native_result = run_program_with_docs(Arc::from(FIXTURE_NAMESPACE), Arc::from(args.kernel.as_str()), &calcit_args)
    .map_err(|error| error.to_string())?;
  let native_call_ns = nanos(native_started.elapsed());

  let calx_one_shot_started = Instant::now();
  let boundary_arguments_started = Instant::now();
  let vm_args = convert_arguments(&kernel, &calcit_args)?;
  let boundary_arguments_ns = nanos(boundary_arguments_started.elapsed());

  let setup_started = Instant::now();
  let mut vm = kernel.instantiate().map_err(|error| error.to_string())?;
  let vm_setup_ns = nanos(setup_started.elapsed());

  let execution_started = Instant::now();
  let vm_result = vm.run_typed(vm_args.clone()).map_err(|error| error.to_string())?;
  let pure_execution_ns = nanos(execution_started.elapsed());

  let result_boundary_started = Instant::now();
  let calx_result = convert_result(&kernel, vm_result)?;
  let boundary_result_ns = nanos(result_boundary_started.elapsed());
  let calx_one_shot_ns = nanos(calx_one_shot_started.elapsed());
  if calx_result != native_result {
    return Err(format!(
      "correctness mismatch for {}/{}: Calcit={native_result}, Calx={calx_result}",
      FIXTURE_NAMESPACE, args.kernel
    ));
  }

  let mut hot_vm = kernel.instantiate().map_err(|error| error.to_string())?;
  for _ in 0..args.vm_warmup {
    black_box(hot_vm.run_typed(vm_args.clone()).map_err(|error| error.to_string())?);
  }
  let hot_inputs = (0..args.hot_iterations).map(|_| vm_args.clone()).collect::<Vec<_>>();
  let hot_started = Instant::now();
  for input in hot_inputs {
    black_box(hot_vm.run_typed(input).map_err(|error| error.to_string())?);
  }
  let hot_execution_total_ns = nanos(hot_started.elapsed());
  let hot_execution_per_call_ns = hot_execution_total_ns / u64::from(args.hot_iterations);

  let validated = kernel.validated_program();
  let functions = validated.functions().len();
  let imports = validated.imports().len();
  let syntax_nodes = validated.functions().iter().map(|function| function.syntax.len()).sum();
  let instructions = validated.functions().iter().map(|function| function.instrs.len()).sum();
  let diagnostic_bytes = kernel.stable_program_summary().len();

  Ok(BenchmarkReport {
    schema: "calcit-calx-benchmark/1",
    environment: EnvironmentReport {
      package_version: env!("CARGO_PKG_VERSION"),
      calx_vm_version: CALX_VM_VERSION,
      profile: if cfg!(debug_assertions) { "debug" } else { "release" },
      os: std::env::consts::OS,
      architecture: std::env::consts::ARCH,
    },
    kernel: args.kernel.clone(),
    workload: "scalar-only",
    size: args.size,
    vm_warmup: args.vm_warmup,
    hot_iterations: args.hot_iterations,
    fixture_install_ns,
    calcit_frontend_ns,
    snapshot_clone_ns,
    compile: CompileReport {
      eligibility_ns: nanos(compile_timings.eligibility),
      planning_ns: nanos(compile_timings.planning),
      program_construction_ns: nanos(compile_timings.program_construction),
      validation_lowering_ns: nanos(compile_timings.validation_lowering),
      total_ns: nanos(compile_timings.total),
    },
    runtime: RuntimeReport {
      native_call_ns,
      boundary_arguments_ns,
      vm_setup_ns,
      pure_execution_ns,
      boundary_result_ns,
      calx_one_shot_ns,
      hot_execution_total_ns,
      hot_execution_per_call_ns,
    },
    program: ProgramReport {
      functions,
      imports,
      syntax_nodes,
      instructions,
      diagnostic_bytes,
      host_boundary_calls_per_execution: 0,
      reuses_vm_frames_and_stack: true,
    },
    correctness: true,
  })
}

fn main() {
  let args: Args = argh::from_env();
  match measure(&args).and_then(|report| serde_json::to_string(&report).map_err(|error| error.to_string())) {
    Ok(json) => println!("{json}"),
    Err(error) => {
      eprintln!("calcit-calx-bench: {error}");
      std::process::exit(1);
    }
  }
}
