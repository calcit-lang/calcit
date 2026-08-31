//! Reproducible single-process measurement for one Calcit-to-Calx scalar case.
//!
//! The orchestration script runs this binary in fresh processes to measure
//! process-level cold cost without mixing benchmark policy into the compiler.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashSet;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use argh::FromArgs;
use calcit::Calcit;
use calcit::calcit::{CalcitFnTypeAnnotation, CalcitTypeAnnotation, SchemaKind};
use calcit::codegen::calx::benchmark_session::{CalxBenchmarkCorpus, CalxBenchmarkDefinition, CalxBenchmarkSession};
use calcit::codegen::calx::{CalxCompileCache, CalxHostImports};
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

  /// cache hits measured with fresh binding attachment and VM setup; zero disables cache profile mode
  #[argh(option, default = "0")]
  cache_profile_iterations: u32,

  /// cache-hit preparations discarded before cache profile measurement
  #[argh(option, default = "100")]
  cache_profile_warmup: u32,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheProfileRuntimeReport {
  initial_miss_prepare_ns: u64,
  hit_prepare_total_ns: u64,
  hit_prepare_per_iteration_ns: u64,
  revision_validation_total_ns: u64,
  revision_validation_per_iteration_ns: u64,
  binding_attachment_total_ns: u64,
  binding_attachment_per_iteration_ns: u64,
  fresh_vm_setup_total_ns: u64,
  fresh_vm_setup_per_iteration_ns: u64,
  fresh_vm_execution_total_ns: u64,
  fresh_vm_execution_per_iteration_ns: u64,
  reused_vm_execution_total_ns: u64,
  reused_vm_execution_per_iteration_ns: u64,
  cached_native_execution_total_ns: u64,
  cached_native_execution_per_iteration_ns: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheProfileStateReport {
  capacity: usize,
  hits: u64,
  misses: u64,
  initial_miss_reason: &'static str,
  evictions: u64,
  entries: usize,
  reachable_functions: usize,
  syntax_instructions: usize,
  lowered_instructions: usize,
  estimated_bytes: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheProfileReport {
  schema: &'static str,
  environment: EnvironmentReport,
  kernel: String,
  workload: &'static str,
  warmup_iterations: u32,
  measured_iterations: u32,
  fixture_install_ns: u64,
  calcit_frontend_ns: u64,
  snapshot_clone_ns: u64,
  initial_compile: CompileReport,
  runtime: CacheProfileRuntimeReport,
  cache: CacheProfileStateReport,
  correctness: bool,
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
    package_version: calcit::cli_args::CALCIT_VERSION,
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

/// Declare the complete source-backed scalar corpus and fixed schemas.
fn benchmark_corpus() -> Result<CalxBenchmarkCorpus, String> {
  let definitions = [
    ("range-sum", 2),
    ("fibonacci", 1),
    ("affine-helper", 3),
    ("affine", 3),
    ("polynomial", 1),
    ("bounded-simulation", 3),
  ]
  .into_iter()
  .map(|(name, arity)| CalxBenchmarkDefinition::new(name, number_fn_schema(arity)));
  CalxBenchmarkCorpus::new(
    FIXTURE_NAMESPACE,
    include_str!("../../tests/fixtures/calx/scalar-kernels.cirru"),
    definitions,
  )
}

/// Create the only mutable setup phase, then retain the immutable pinned session.
fn prepare_session(kernel: &str) -> Result<CalxBenchmarkSession, String> {
  CalxBenchmarkSession::prepare(&benchmark_corpus()?, kernel)
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

fn compile_report(timings: calcit::codegen::calx::CalxKernelCompileTimings) -> CompileReport {
  CompileReport {
    eligibility_ns: nanos(timings.eligibility),
    planning_ns: nanos(timings.planning),
    program_construction_ns: nanos(timings.program_construction),
    validation_lowering_ns: nanos(timings.validation_lowering),
    total_ns: nanos(timings.total),
  }
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

  let session = prepare_session(&args.kernel)?;
  let session_timings = session.timings();
  let fixture_install_ns = nanos(session_timings.source_install);
  let calcit_frontend_ns = nanos(session_timings.frontend);
  let snapshot_clone_ns = nanos(session_timings.program_snapshot);
  let imports = CalxHostImports::new();

  for _ in 0..args.compile_profile_warmup {
    black_box(session.compile_calx(&imports).map_err(|error| error.to_string())?);
  }

  let mut stage_timing_total = CompileReport::default();
  for _ in 0..args.compile_profile_stage_iterations {
    let (kernel, timings) = session.compile_calx_measured(&imports).map_err(|error| error.to_string())?;
    add_compile_timings(&mut stage_timing_total, timings);
    black_box(kernel);
  }
  let stage_timing_per_iteration = stage_timing_total.divided_by(args.compile_profile_stage_iterations);

  let allocation_window = ProfileAllocationWindow::begin()?;
  let allocation_result = (|| {
    for _ in 0..args.compile_profile_allocation_iterations {
      black_box(session.compile_calx(&imports).map_err(|error| error.to_string())?);
    }
    Ok::<(), String>(())
  })();
  allocation_result?;
  let allocations = allocation_window.finish();
  let allocations_per_iteration = allocations.divided_by(args.compile_profile_allocation_iterations);

  let compile_started = Instant::now();
  for _ in 0..args.compile_profile_iterations {
    black_box(session.compile_calx(&imports).map_err(|error| error.to_string())?);
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

/// Measure cache-hit preparation separately from fresh-VM and reused-VM work.
fn measure_cache_profile(args: &Args) -> Result<CacheProfileReport, String> {
  if args.cache_profile_iterations == 0 {
    return Err("--cache-profile-iterations must be greater than zero in cache profile mode".to_owned());
  }

  let session = prepare_session(&args.kernel)?;
  let session_timings = session.timings();
  let fixture_install_ns = nanos(session_timings.source_install);
  let calcit_frontend_ns = nanos(session_timings.frontend);
  let snapshot_clone_ns = nanos(session_timings.program_snapshot);
  let imports = CalxHostImports::new();
  let mut cache = CalxCompileCache::new(1);
  let initial_started = Instant::now();
  let initial = session
    .prepare_cached_calx(&mut cache, &imports)
    .map_err(|error| error.to_string())?;
  let initial_miss_prepare_ns = nanos(initial_started.elapsed());
  let initial_miss_reason = initial
    .report()
    .miss_reason
    .ok_or_else(|| "initial cache preparation unexpectedly hit".to_owned())?;
  let initial_compile = compile_report(
    initial
      .report()
      .compilation
      .ok_or_else(|| "initial cache miss did not report compilation stages".to_owned())?,
  );
  let initial_kernel = initial.into_kernel();
  let calcit_args = kernel_arguments(&args.kernel, args.size)?;
  let vm_args = CalxBenchmarkSession::encode_calx_arguments(&initial_kernel, &calcit_args)?;

  let native_result = session.run_calcit_lookup(&calcit_args)?;

  for _ in 0..args.cache_profile_warmup {
    let preparation = session
      .prepare_cached_calx(&mut cache, &imports)
      .map_err(|error| error.to_string())?;
    if !preparation.report().cache_hit {
      return Err("cache profile warmup unexpectedly missed".to_owned());
    }
    let mut vm = CalxBenchmarkSession::instantiate_calx(preparation.kernel()).map_err(|error| error.to_string())?;
    black_box(vm.run_values(vm_args.clone())?);
    black_box(session.run_calcit_cached(&calcit_args)?);
  }

  let mut hit_prepare_total_ns = 0u64;
  let mut revision_validation_total_ns = 0u64;
  let mut binding_attachment_total_ns = 0u64;
  let mut fresh_vm_setup_total_ns = 0u64;
  let mut fresh_vm_execution_total_ns = 0u64;
  let mut last_fresh_result = None;
  for _ in 0..args.cache_profile_iterations {
    let prepare_started = Instant::now();
    let preparation = session
      .prepare_cached_calx(&mut cache, &imports)
      .map_err(|error| error.to_string())?;
    hit_prepare_total_ns = hit_prepare_total_ns.saturating_add(nanos(prepare_started.elapsed()));
    if !preparation.report().cache_hit {
      return Err("cache profile measured preparation unexpectedly missed".to_owned());
    }
    revision_validation_total_ns = revision_validation_total_ns.saturating_add(nanos(preparation.report().revision_validation));
    binding_attachment_total_ns = binding_attachment_total_ns.saturating_add(nanos(preparation.report().binding_attachment));

    let setup_started = Instant::now();
    let mut vm = CalxBenchmarkSession::instantiate_calx(preparation.kernel()).map_err(|error| error.to_string())?;
    fresh_vm_setup_total_ns = fresh_vm_setup_total_ns.saturating_add(nanos(setup_started.elapsed()));
    let execution_started = Instant::now();
    let result = vm.run_values(vm_args.clone())?;
    fresh_vm_execution_total_ns = fresh_vm_execution_total_ns.saturating_add(nanos(execution_started.elapsed()));
    last_fresh_result = Some(CalxBenchmarkSession::decode_calx_result(preparation.kernel(), result)?);
  }

  let mut reused_vm = CalxBenchmarkSession::instantiate_calx(&initial_kernel).map_err(|error| error.to_string())?;
  for _ in 0..args.cache_profile_warmup {
    black_box(reused_vm.run_values(vm_args.clone())?);
  }
  let reused_started = Instant::now();
  let mut last_reused_result = None;
  for _ in 0..args.cache_profile_iterations {
    last_reused_result = Some(CalxBenchmarkSession::decode_calx_result(
      &initial_kernel,
      reused_vm.run_values(vm_args.clone())?,
    )?);
  }
  let reused_vm_execution_total_ns = nanos(reused_started.elapsed());

  let cached_native_started = Instant::now();
  let mut last_cached_native_result = None;
  for _ in 0..args.cache_profile_iterations {
    last_cached_native_result = Some(session.run_calcit_cached(&calcit_args)?);
  }
  let cached_native_execution_total_ns = nanos(cached_native_started.elapsed());

  if last_fresh_result.as_ref() != Some(&native_result)
    || last_reused_result.as_ref() != Some(&native_result)
    || last_cached_native_result.as_ref() != Some(&native_result)
  {
    return Err(format!(
      "cache profile correctness mismatch for {}/{}",
      FIXTURE_NAMESPACE, args.kernel
    ));
  }

  let divisor = u64::from(args.cache_profile_iterations);
  let stats = cache.stats();
  Ok(CacheProfileReport {
    schema: "calcit-calx-cache-profile/1",
    environment: environment_report()?,
    kernel: args.kernel.clone(),
    workload: "revision-validated-cache-hit-plus-fresh-vm",
    warmup_iterations: args.cache_profile_warmup,
    measured_iterations: args.cache_profile_iterations,
    fixture_install_ns,
    calcit_frontend_ns,
    snapshot_clone_ns,
    initial_compile,
    runtime: CacheProfileRuntimeReport {
      initial_miss_prepare_ns,
      hit_prepare_total_ns,
      hit_prepare_per_iteration_ns: hit_prepare_total_ns / divisor,
      revision_validation_total_ns,
      revision_validation_per_iteration_ns: revision_validation_total_ns / divisor,
      binding_attachment_total_ns,
      binding_attachment_per_iteration_ns: binding_attachment_total_ns / divisor,
      fresh_vm_setup_total_ns,
      fresh_vm_setup_per_iteration_ns: fresh_vm_setup_total_ns / divisor,
      fresh_vm_execution_total_ns,
      fresh_vm_execution_per_iteration_ns: fresh_vm_execution_total_ns / divisor,
      reused_vm_execution_total_ns,
      reused_vm_execution_per_iteration_ns: reused_vm_execution_total_ns / divisor,
      cached_native_execution_total_ns,
      cached_native_execution_per_iteration_ns: cached_native_execution_total_ns / divisor,
    },
    cache: CacheProfileStateReport {
      capacity: cache.capacity(),
      hits: stats.hits,
      misses: stats.misses,
      initial_miss_reason: initial_miss_reason.as_str(),
      evictions: stats.evictions,
      entries: stats.entry_count,
      reachable_functions: stats.reachable_function_count,
      syntax_instructions: stats.syntax_instruction_count,
      lowered_instructions: stats.lowered_instruction_count,
      estimated_bytes: stats.estimated_bytes,
    },
    correctness: true,
  })
}

/// Run correctness first, then collect one process-local staged measurement.
fn measure(args: &Args) -> Result<BenchmarkReport, String> {
  if args.hot_iterations == 0 {
    return Err("--hot-iterations must be greater than zero".to_owned());
  }

  let session = prepare_session(&args.kernel)?;
  let session_timings = session.timings();
  let fixture_install_ns = nanos(session_timings.source_install);
  let calcit_frontend_ns = nanos(session_timings.frontend);
  let snapshot_clone_ns = nanos(session_timings.program_snapshot);

  let (kernel, compile_timings) = session
    .compile_calx_measured(&CalxHostImports::new())
    .map_err(|error| error.to_string())?;
  let calcit_args = kernel_arguments(&args.kernel, args.size)?;

  let native_started = Instant::now();
  let native_result = session.run_calcit_lookup(&calcit_args)?;
  let native_call_ns = nanos(native_started.elapsed());

  let cached_native_resolution_ns = nanos(session_timings.cached_calcit_resolution);
  let cached_native_result = session.run_calcit_cached(&calcit_args)?;
  if cached_native_result != native_result {
    return Err(format!(
      "correctness mismatch for {}/{}: Calcit lookup={native_result}, Calcit cached={cached_native_result}",
      FIXTURE_NAMESPACE, args.kernel
    ));
  }

  for _ in 0..args.vm_warmup {
    black_box(session.run_calcit_cached(&calcit_args)?);
  }
  let cached_native_inputs = (0..args.hot_iterations).map(|_| calcit_args.clone()).collect::<Vec<_>>();
  let cached_native_started = Instant::now();
  for input in cached_native_inputs {
    black_box(session.run_calcit_cached(&input)?);
  }
  let cached_native_execution_total_ns = nanos(cached_native_started.elapsed());
  let cached_native_execution_per_call_ns = cached_native_execution_total_ns / u64::from(args.hot_iterations);

  let calx_one_shot_started = Instant::now();
  let boundary_arguments_started = Instant::now();
  let vm_args = CalxBenchmarkSession::encode_calx_arguments(&kernel, &calcit_args)?;
  let boundary_arguments_ns = nanos(boundary_arguments_started.elapsed());

  let setup_started = Instant::now();
  let mut vm = CalxBenchmarkSession::instantiate_calx(&kernel).map_err(|error| error.to_string())?;
  let vm_setup_ns = nanos(setup_started.elapsed());

  let one_shot_input = vm_args.clone();
  let execution_started = Instant::now();
  let vm_result = vm.run_values(one_shot_input)?;
  let pure_execution_ns = nanos(execution_started.elapsed());

  let result_boundary_started = Instant::now();
  let calx_result = CalxBenchmarkSession::decode_calx_result(&kernel, vm_result)?;
  let boundary_result_ns = nanos(result_boundary_started.elapsed());
  let calx_one_shot_ns = nanos(calx_one_shot_started.elapsed());
  if calx_result != native_result {
    return Err(format!(
      "correctness mismatch for {}/{}: Calcit={native_result}, Calx={calx_result}",
      FIXTURE_NAMESPACE, args.kernel
    ));
  }

  let mut hot_vm = CalxBenchmarkSession::instantiate_calx(&kernel).map_err(|error| error.to_string())?;
  for _ in 0..args.vm_warmup {
    black_box(hot_vm.run_values(vm_args.clone())?);
  }
  let hot_inputs = (0..args.hot_iterations).map(|_| vm_args.clone()).collect::<Vec<_>>();
  let hot_started = Instant::now();
  for input in hot_inputs {
    black_box(hot_vm.run_values(input)?);
  }
  let hot_execution_total_ns = nanos(hot_started.elapsed());
  let hot_execution_per_call_ns = hot_execution_total_ns / u64::from(args.hot_iterations);

  let counts = CalxBenchmarkSession::program_counts(&kernel);

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
      functions: counts.functions,
      imports: counts.imports,
      syntax_nodes: counts.syntax_nodes,
      instructions: counts.instructions,
      diagnostic_bytes: counts.diagnostic_bytes,
      host_boundary_calls_per_execution: 0,
      reuses_vm_frames_and_stack: true,
    },
    correctness: true,
  })
}

/// Emit exactly one JSON report on success and keep failures on stderr.
fn main() {
  let args: Args = argh::from_env();
  let report = if args.compile_profile_iterations > 0 && args.cache_profile_iterations > 0 {
    Err("compile profile mode and cache profile mode are mutually exclusive".to_owned())
  } else if args.compile_profile_iterations > 0 {
    measure_compile_profile(&args).and_then(|report| serde_json::to_string(&report).map_err(|error| error.to_string()))
  } else if args.cache_profile_iterations > 0 {
    measure_cache_profile(&args).and_then(|report| serde_json::to_string(&report).map_err(|error| error.to_string()))
  } else {
    measure(&args).and_then(|report| serde_json::to_string(&report).map_err(|error| error.to_string()))
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
    let args = kernel_arguments("range-sum", 10).expect("range-sum arguments");
    {
      let session = prepare_session("range-sum").expect("prepare benchmark session");
      let lookup_result = session.run_calcit_lookup(&args).expect("run lookup baseline");
      let cached_result = session.run_calcit_cached(&args).expect("run cached callable");
      assert_eq!(cached_result, lookup_result);
    }

    let error = prepare_session("missing-kernel").err().expect("missing entries must fail");
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
      cache_profile_iterations: 0,
      cache_profile_warmup: 0,
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

  #[test]
  fn cache_profile_separates_hit_validation_binding_and_fresh_vm_costs() {
    let report = measure_cache_profile(&Args {
      kernel: "affine".to_owned(),
      size: 10,
      vm_warmup: 0,
      hot_iterations: 1,
      compile_profile_iterations: 0,
      compile_profile_warmup: 0,
      compile_profile_stage_iterations: 1,
      compile_profile_allocation_iterations: 1,
      cache_profile_iterations: 2,
      cache_profile_warmup: 1,
    })
    .expect("measure revision-safe cache hits");

    assert_eq!(report.schema, "calcit-calx-cache-profile/1");
    assert_eq!(report.workload, "revision-validated-cache-hit-plus-fresh-vm");
    assert_eq!(report.cache.capacity, 1);
    assert_eq!(report.cache.misses, 1);
    assert_eq!(report.cache.initial_miss_reason, "empty");
    assert_eq!(report.cache.evictions, 0);
    assert_eq!(report.cache.entries, 1);
    assert_eq!(report.cache.hits, 3);
    assert!(report.cache.estimated_bytes > 0);
    assert!(report.initial_compile.total_ns > 0);
    assert!(report.runtime.hit_prepare_total_ns > 0);
    assert!(report.runtime.fresh_vm_setup_total_ns > 0);
    assert!(report.runtime.fresh_vm_execution_total_ns > 0);
    assert!(report.runtime.reused_vm_execution_total_ns > 0);
    assert!(report.runtime.cached_native_execution_total_ns > 0);
    assert!(report.correctness);
  }
}
