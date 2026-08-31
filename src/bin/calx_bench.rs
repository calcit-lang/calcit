//! Reproducible single-process measurement for one Calcit-to-Calx scalar case.
//!
//! The orchestration script runs this binary in fresh processes to measure
//! process-level cold cost without mixing benchmark policy into the compiler.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use argh::FromArgs;
use calcit::calcit::{CalcitFn, CalcitFnTypeAnnotation, CalcitTypeAnnotation, SchemaKind};
use calcit::call_stack::CallStackList;
use calcit::codegen::calx::{CalxCompiledKernel, CalxScalarType, CalxValue, compile_calx_kernel, compile_calx_kernel_measured};
use calcit::data::cirru::code_to_calcit;
use calcit::program::{PROGRAM_CODE_DATA, ProgramDefEntry, ProgramFileData, clone_existing_compiled_program, ensure_def_id};
use calcit::{Calcit, run_program_with_docs};
use calx_vm::CalxRunResult;
use cirru_parser::Cirru;
use serde::Serialize;

const FIXTURE_NAMESPACE: &str = "bench.calx-kernels";
const CARGO_LOCK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"));

struct ProfileAllocator;

static COUNT_PROFILE_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static PROFILE_ALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static PROFILE_REALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static PROFILE_DEALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static PROFILE_REQUESTED_BYTES: AtomicU64 = AtomicU64::new(0);
static PROFILE_DEALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static PROFILE_ALLOCATION_WINDOW: Mutex<()> = Mutex::new(());

struct ProfileAllocationWindow {
  _guard: MutexGuard<'static, ()>,
}

impl ProfileAllocationWindow {
  /// Start one exclusive allocation-measurement window and reset its counters.
  fn begin() -> Result<Self, String> {
    let guard = PROFILE_ALLOCATION_WINDOW
      .lock()
      .map_err(|error| format!("Calx compile profile allocation window is poisoned: {error}"))?;
    reset_profile_allocations();
    COUNT_PROFILE_ALLOCATIONS.store(true, Ordering::Relaxed);
    Ok(Self { _guard: guard })
  }

  /// Stop counting before exposing the counters to report construction.
  fn finish(self) -> AllocationReport {
    COUNT_PROFILE_ALLOCATIONS.store(false, Ordering::Relaxed);
    read_profile_allocations()
  }
}

impl Drop for ProfileAllocationWindow {
  fn drop(&mut self) {
    COUNT_PROFILE_ALLOCATIONS.store(false, Ordering::Relaxed);
  }
}

fn record_profile_allocation(bytes: usize) {
  if COUNT_PROFILE_ALLOCATIONS.load(Ordering::Relaxed) {
    PROFILE_ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
    PROFILE_REQUESTED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
  }
}

unsafe impl GlobalAlloc for ProfileAllocator {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let value = unsafe { System.alloc(layout) };
    if !value.is_null() {
      record_profile_allocation(layout.size());
    }
    value
  }

  unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    let value = unsafe { System.alloc_zeroed(layout) };
    if !value.is_null() {
      record_profile_allocation(layout.size());
    }
    value
  }

  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    if COUNT_PROFILE_ALLOCATIONS.load(Ordering::Relaxed) {
      PROFILE_DEALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
      PROFILE_DEALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
    }
    unsafe { System.dealloc(ptr, layout) };
  }

  unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    let value = unsafe { System.realloc(ptr, layout, new_size) };
    if !value.is_null() && COUNT_PROFILE_ALLOCATIONS.load(Ordering::Relaxed) {
      PROFILE_REALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
      PROFILE_REQUESTED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
    }
    value
  }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: ProfileAllocator = ProfileAllocator;

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

  /// repeat complete Calx compilation for profiler sampling; zero runs the normal benchmark
  #[argh(option, default = "0")]
  compile_profile_iterations: u32,

  /// complete Calx compilations discarded before profiler sampling
  #[argh(option, default = "100")]
  compile_profile_warmup: u32,

  /// compilations used for aggregate per-stage timings in profile mode
  #[argh(option, default = "10000")]
  compile_profile_stage_iterations: u32,

  /// compilations used for allocation counters in profile mode
  #[argh(option, default = "10000")]
  compile_profile_allocation_iterations: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvironmentReport {
  package_version: &'static str,
  calx_vm_version: String,
  profile: &'static str,
  os: &'static str,
  architecture: &'static str,
}

#[derive(Debug, Default, Serialize)]
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
  cached_native_resolution_ns: u64,
  cached_native_execution_total_ns: u64,
  cached_native_execution_per_call_ns: u64,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompileProfileReport {
  schema: &'static str,
  environment: EnvironmentReport,
  kernel: String,
  workload: &'static str,
  warmup_iterations: u32,
  measured_iterations: u32,
  fixture_install_ns: u64,
  calcit_frontend_ns: u64,
  snapshot_clone_ns: u64,
  compile_total_ns: u64,
  compile_per_iteration_ns: u64,
  stage_timing_iterations: u32,
  stage_timing_total: CompileReport,
  stage_timing_per_iteration: CompileReport,
  allocation_iterations: u32,
  allocations: AllocationReport,
  allocations_per_iteration: AllocationReport,
  compilation_succeeded: bool,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct AllocationReport {
  allocation_calls: u64,
  reallocation_calls: u64,
  deallocation_calls: u64,
  requested_bytes: u64,
  explicitly_deallocated_bytes: u64,
}

impl AllocationReport {
  fn divided_by(&self, iterations: u32) -> Self {
    let divisor = u64::from(iterations);
    Self {
      allocation_calls: self.allocation_calls / divisor,
      reallocation_calls: self.reallocation_calls / divisor,
      deallocation_calls: self.deallocation_calls / divisor,
      requested_bytes: self.requested_bytes / divisor,
      explicitly_deallocated_bytes: self.explicitly_deallocated_bytes / divisor,
    }
  }
}

impl CompileReport {
  fn divided_by(&self, iterations: u32) -> Self {
    let divisor = u64::from(iterations);
    Self {
      eligibility_ns: self.eligibility_ns / divisor,
      planning_ns: self.planning_ns / divisor,
      program_construction_ns: self.program_construction_ns / divisor,
      validation_lowering_ns: self.validation_lowering_ns / divisor,
      total_ns: self.total_ns / divisor,
    }
  }
}

fn nanos(duration: Duration) -> u64 {
  u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn environment_report() -> Result<EnvironmentReport, String> {
  Ok(EnvironmentReport {
    package_version: env!("CARGO_PKG_VERSION"),
    calx_vm_version: resolved_dependency_version(CARGO_LOCK, "calx_vm")?.to_owned(),
    profile: if cfg!(debug_assertions) { "debug" } else { "release" },
    os: std::env::consts::OS,
    architecture: std::env::consts::ARCH,
  })
}

/// Resolve one uniquely-versioned package from the lockfile embedded at build time.
fn resolved_dependency_version<'a>(lockfile: &'a str, package_name: &str) -> Result<&'a str, String> {
  let versions = lockfile
    .split("[[package]]")
    .skip(1)
    .filter_map(|package| {
      let name = package
        .lines()
        .find_map(|line| line.strip_prefix("name = \"").and_then(|value| value.strip_suffix('"')))?;
      let version = package
        .lines()
        .find_map(|line| line.strip_prefix("version = \"").and_then(|value| value.strip_suffix('"')))?;
      (name == package_name).then_some(version)
    })
    .collect::<Vec<_>>();
  match versions.as_slice() {
    [version] => Ok(version),
    [] => Err(format!("Cargo.lock has no resolved `{package_name}` package")),
    _ => Err(format!("Cargo.lock resolves `{package_name}` more than once: {versions:?}")),
  }
}

/// Build the fixed Number-only function schema for one benchmark definition.
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

/// Parse and install the source-backed scalar corpus in a fresh benchmark process.
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

/// Map a named corpus case and its input size to concrete Calcit arguments.
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

/// Convert proven scalar arguments without admitting Nil or Dynamic values.
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

/// Convert one strict Calx result back through the proven scalar boundary.
fn convert_result(kernel: &CalxCompiledKernel, result: CalxRunResult) -> Result<Calcit, String> {
  match (kernel.result(), result) {
    (None, CalxRunResult::Void) => Ok(Calcit::Unit),
    (Some(CalxScalarType::F64), CalxRunResult::Value(CalxValue::F64(value))) => Ok(Calcit::Number(value)),
    (Some(CalxScalarType::Bool), CalxRunResult::Value(CalxValue::Bool(value))) => Ok(Calcit::Bool(value)),
    (expected, actual) => Err(format!("validated result contract {expected:?} produced {actual:?}")),
  }
}

/// Resolve the already-preprocessed Calcit entry once for a fair repeated-call baseline.
fn resolve_cached_calcit_callable(kernel: &str, call_stack: &CallStackList) -> Result<Arc<CalcitFn>, String> {
  match calcit::runner::evaluate_symbol_from_program(kernel, FIXTURE_NAMESPACE, None, call_stack).map_err(|error| error.to_string())? {
    Calcit::Fn { info, .. } => Ok(info),
    value => Err(format!("expected cached benchmark callable, found {value}")),
  }
}

/// Reset counters while holding the exclusive profile allocation window.
fn reset_profile_allocations() {
  PROFILE_ALLOCATION_CALLS.store(0, Ordering::Relaxed);
  PROFILE_REALLOCATION_CALLS.store(0, Ordering::Relaxed);
  PROFILE_DEALLOCATION_CALLS.store(0, Ordering::Relaxed);
  PROFILE_REQUESTED_BYTES.store(0, Ordering::Relaxed);
  PROFILE_DEALLOCATED_BYTES.store(0, Ordering::Relaxed);
}

/// Snapshot counters before releasing the exclusive profile allocation window.
fn read_profile_allocations() -> AllocationReport {
  AllocationReport {
    allocation_calls: PROFILE_ALLOCATION_CALLS.load(Ordering::Relaxed),
    reallocation_calls: PROFILE_REALLOCATION_CALLS.load(Ordering::Relaxed),
    deallocation_calls: PROFILE_DEALLOCATION_CALLS.load(Ordering::Relaxed),
    requested_bytes: PROFILE_REQUESTED_BYTES.load(Ordering::Relaxed),
    explicitly_deallocated_bytes: PROFILE_DEALLOCATED_BYTES.load(Ordering::Relaxed),
  }
}

/// Accumulate one measured compilation into an aggregate stage report.
fn add_compile_timings(report: &mut CompileReport, timings: calcit::codegen::calx::CalxKernelCompileTimings) {
  report.eligibility_ns = report.eligibility_ns.saturating_add(nanos(timings.eligibility));
  report.planning_ns = report.planning_ns.saturating_add(nanos(timings.planning));
  report.program_construction_ns = report.program_construction_ns.saturating_add(nanos(timings.program_construction));
  report.validation_lowering_ns = report.validation_lowering_ns.saturating_add(nanos(timings.validation_lowering));
  report.total_ns = report.total_ns.saturating_add(nanos(timings.total));
}

/// Amplify the complete compile pipeline so sampling and allocation profilers
/// can distinguish its stages from process/frontend setup.
fn measure_compile_profile(args: &Args) -> Result<CompileProfileReport, String> {
  if args.compile_profile_iterations == 0 {
    return Err("--compile-profile-iterations must be greater than zero in compile profile mode".to_owned());
  }
  if args.compile_profile_stage_iterations == 0 {
    return Err("--compile-profile-stage-iterations must be greater than zero in compile profile mode".to_owned());
  }
  if args.compile_profile_allocation_iterations == 0 {
    return Err("--compile-profile-allocation-iterations must be greater than zero in compile profile mode".to_owned());
  }

  let fixture_started = Instant::now();
  install_fixture()?;
  let fixture_install_ns = nanos(fixture_started.elapsed());

  let frontend_started = Instant::now();
  let warnings = RefCell::new(vec![]);
  calcit::runner::preprocess::ensure_ns_def_compiled(FIXTURE_NAMESPACE, &args.kernel, &warnings, &CallStackList::default())
    .map_err(|error| error.to_string())?;
  if !warnings.borrow().is_empty() {
    return Err(format!("Calx compile profile frontend produced warnings: {:#?}", warnings.borrow()));
  }
  let calcit_frontend_ns = nanos(frontend_started.elapsed());

  let snapshot_started = Instant::now();
  let snapshot = clone_existing_compiled_program();
  let snapshot_clone_ns = nanos(snapshot_started.elapsed());

  for _ in 0..args.compile_profile_warmup {
    black_box(compile_calx_kernel(&snapshot, FIXTURE_NAMESPACE, args.kernel.as_str()).map_err(|error| error.to_string())?);
  }

  let mut stage_timing_total = CompileReport::default();
  for _ in 0..args.compile_profile_stage_iterations {
    let (kernel, timings) =
      compile_calx_kernel_measured(&snapshot, FIXTURE_NAMESPACE, args.kernel.as_str()).map_err(|error| error.to_string())?;
    add_compile_timings(&mut stage_timing_total, timings);
    black_box(kernel);
  }
  let stage_timing_per_iteration = stage_timing_total.divided_by(args.compile_profile_stage_iterations);

  let allocation_window = ProfileAllocationWindow::begin()?;
  let allocation_result = (|| {
    for _ in 0..args.compile_profile_allocation_iterations {
      black_box(compile_calx_kernel(&snapshot, FIXTURE_NAMESPACE, args.kernel.as_str()).map_err(|error| error.to_string())?);
    }
    Ok::<(), String>(())
  })();
  allocation_result?;
  let allocations = allocation_window.finish();
  let allocations_per_iteration = allocations.divided_by(args.compile_profile_allocation_iterations);

  let compile_started = Instant::now();
  for _ in 0..args.compile_profile_iterations {
    black_box(compile_calx_kernel(&snapshot, FIXTURE_NAMESPACE, args.kernel.as_str()).map_err(|error| error.to_string())?);
  }
  let compile_total_ns = nanos(compile_started.elapsed());

  Ok(CompileProfileReport {
    schema: "calcit-calx-compile-profile/1",
    environment: environment_report()?,
    kernel: args.kernel.clone(),
    workload: "complete-uncached-scalar-compilation",
    warmup_iterations: args.compile_profile_warmup,
    measured_iterations: args.compile_profile_iterations,
    fixture_install_ns,
    calcit_frontend_ns,
    snapshot_clone_ns,
    compile_total_ns,
    compile_per_iteration_ns: compile_total_ns / u64::from(args.compile_profile_iterations),
    stage_timing_iterations: args.compile_profile_stage_iterations,
    stage_timing_total,
    stage_timing_per_iteration,
    allocation_iterations: args.compile_profile_allocation_iterations,
    allocations,
    allocations_per_iteration,
    compilation_succeeded: true,
  })
}

/// Run correctness first, then collect one process-local staged measurement.
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

  let native_call_stack = CallStackList::default();
  let cached_native_resolution_started = Instant::now();
  let cached_native_callable = resolve_cached_calcit_callable(&args.kernel, &native_call_stack)?;
  let cached_native_resolution_ns = nanos(cached_native_resolution_started.elapsed());
  let cached_native_result =
    calcit::runner::run_fn(&calcit_args, &cached_native_callable, &native_call_stack).map_err(|error| error.to_string())?;
  if cached_native_result != native_result {
    return Err(format!(
      "correctness mismatch for {}/{}: Calcit lookup={native_result}, Calcit cached={cached_native_result}",
      FIXTURE_NAMESPACE, args.kernel
    ));
  }

  for _ in 0..args.vm_warmup {
    black_box(calcit::runner::run_fn(&calcit_args, &cached_native_callable, &native_call_stack).map_err(|error| error.to_string())?);
  }
  let cached_native_inputs = (0..args.hot_iterations).map(|_| calcit_args.clone()).collect::<Vec<_>>();
  let cached_native_started = Instant::now();
  for input in cached_native_inputs {
    black_box(calcit::runner::run_fn(&input, &cached_native_callable, &native_call_stack).map_err(|error| error.to_string())?);
  }
  let cached_native_execution_total_ns = nanos(cached_native_started.elapsed());
  let cached_native_execution_per_call_ns = cached_native_execution_total_ns / u64::from(args.hot_iterations);

  let calx_one_shot_started = Instant::now();
  let boundary_arguments_started = Instant::now();
  let vm_args = convert_arguments(&kernel, &calcit_args)?;
  let boundary_arguments_ns = nanos(boundary_arguments_started.elapsed());

  let setup_started = Instant::now();
  let mut vm = kernel.instantiate().map_err(|error| error.to_string())?;
  let vm_setup_ns = nanos(setup_started.elapsed());

  let one_shot_input = vm_args.clone();
  let execution_started = Instant::now();
  let vm_result = vm.run_typed(one_shot_input).map_err(|error| error.to_string())?;
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
    schema: "calcit-calx-benchmark/2",
    environment: environment_report()?,
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
      cached_native_resolution_ns,
      cached_native_execution_total_ns,
      cached_native_execution_per_call_ns,
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

/// Emit exactly one JSON report on success and keep failures on stderr.
fn main() {
  let args: Args = argh::from_env();
  let report = if args.compile_profile_iterations == 0 {
    measure(&args).and_then(|report| serde_json::to_string(&report).map_err(|error| error.to_string()))
  } else {
    measure_compile_profile(&args).and_then(|report| serde_json::to_string(&report).map_err(|error| error.to_string()))
  };
  match report {
    Ok(json) => println!("{json}"),
    Err(error) => {
      eprintln!("calcit-calx-bench: {error}");
      std::process::exit(1);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn embedded_calx_vm_version_comes_from_the_resolved_lockfile() {
    let version = resolved_dependency_version(CARGO_LOCK, "calx_vm").expect("one resolved calx_vm package");
    assert_eq!(version.split('.').count(), 3);
    assert!(version.split('.').all(|part| part.parse::<u64>().is_ok()));
  }

  #[test]
  fn dependency_version_requires_one_unique_package() {
    let duplicate = "[[package]]\nname = \"demo\"\nversion = \"1.0.0\"\n[[package]]\nname = \"demo\"\nversion = \"2.0.0\"\n";
    assert!(resolved_dependency_version(duplicate, "demo").is_err());
    assert!(resolved_dependency_version(duplicate, "missing").is_err());
  }

  #[test]
  fn cached_callable_matches_lookup_execution_and_rejects_missing_entries() {
    install_fixture().expect("install benchmark fixture");
    let args = kernel_arguments("range-sum", 10).expect("range-sum arguments");
    let lookup_result =
      run_program_with_docs(Arc::from(FIXTURE_NAMESPACE), Arc::from("range-sum"), &args).expect("run lookup baseline");
    let call_stack = CallStackList::default();
    let callable = resolve_cached_calcit_callable("range-sum", &call_stack).expect("resolve cached callable");
    let cached_result = calcit::runner::run_fn(&args, &callable, &call_stack).expect("run cached callable");
    assert_eq!(cached_result, lookup_result);

    let error = resolve_cached_calcit_callable("missing-kernel", &call_stack).expect_err("missing entries must fail");
    assert!(error.contains("missing-kernel"));
  }

  #[test]
  fn compile_profile_mode_repeats_complete_uncached_compilation() {
    let report = measure_compile_profile(&Args {
      kernel: "affine".to_owned(),
      size: 10,
      vm_warmup: 0,
      hot_iterations: 1,
      compile_profile_iterations: 2,
      compile_profile_warmup: 1,
      compile_profile_stage_iterations: 2,
      compile_profile_allocation_iterations: 2,
    })
    .expect("measure repeated complete compilation");

    assert_eq!(report.schema, "calcit-calx-compile-profile/1");
    assert_eq!(report.workload, "complete-uncached-scalar-compilation");
    assert_eq!(report.warmup_iterations, 1);
    assert_eq!(report.measured_iterations, 2);
    assert_eq!(report.stage_timing_iterations, 2);
    assert_eq!(report.allocation_iterations, 2);
    assert!(report.compile_total_ns > 0);
    assert!(report.compile_per_iteration_ns > 0);
    assert!(report.stage_timing_total.total_ns > 0);
    assert!(report.stage_timing_per_iteration.total_ns > 0);
    assert!(report.allocations.allocation_calls > 0);
    assert!(report.allocations.requested_bytes > 0);
    assert!(report.allocations_per_iteration.allocation_calls > 0);
    assert!(report.allocations_per_iteration.requested_bytes > 0);
    assert!(report.compilation_succeeded);
    let serialized = serde_json::to_value(&report).expect("serialize compile profile report");
    assert_eq!(serialized["compilationSucceeded"], true);
    assert!(serialized.get("correctness").is_none());
  }
}
