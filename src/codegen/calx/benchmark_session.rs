//! Revision-pinned internal adapter for the standalone Calx benchmark harness.
//!
//! This API intentionally has no SemVer stability promise. A consumer must pin the
//! exact Calcit revision it compiles and validate its quick benchmark matrix before
//! advancing that pin. The adapter hides process-wide compiler registries and exposes
//! only one isolated source/preprocess/compile/run session.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use calx_vm::{CalxRunResult, CalxVM};
use cirru_parser::Cirru;

use crate::calcit::{Calcit, CalcitFn, CalcitTypeAnnotation};
use crate::call_stack::CallStackList;
use crate::data::cirru::code_to_calcit;
use crate::program::{self, CompiledProgram, ProgramDefEntry, ProgramFileData};
use crate::runner;

use super::{
  CalxCachePreparation, CalxCompileCache, CalxHostImports, CalxKernelCompileError, CalxKernelCompileTimings, CalxKernelRunError,
  CalxPreparedKernel, CalxScalarType, CalxValue, compile_calx_kernel_with_imports_measured,
};

/// Contract identifier recorded by revision-pinned harness consumers.
pub const CALX_BENCHMARK_SESSION_EDITION: &str = "calcit-calx-benchmark-session/1";

static BENCHMARK_SESSION_LOCK: Mutex<()> = Mutex::new(());

/// One explicitly named source definition and its fixed type schema.
#[derive(Debug, Clone)]
pub struct CalxBenchmarkDefinition {
  name: Arc<str>,
  schema: Arc<CalcitTypeAnnotation>,
}

impl CalxBenchmarkDefinition {
  /// Declare one definition that must occur exactly once in the source corpus.
  pub fn new(name: impl Into<Arc<str>>, schema: Arc<CalcitTypeAnnotation>) -> Self {
    Self { name: name.into(), schema }
  }

  /// Declared definition name.
  pub fn name(&self) -> &str {
    &self.name
  }

  /// Fixed schema supplied before preprocessing.
  pub fn schema(&self) -> &Arc<CalcitTypeAnnotation> {
    &self.schema
  }
}

/// Explicit source corpus accepted by the internal benchmark adapter.
#[derive(Debug, Clone)]
pub struct CalxBenchmarkCorpus {
  namespace: Arc<str>,
  source: Arc<str>,
  definitions: BTreeMap<Arc<str>, Arc<CalcitTypeAnnotation>>,
}

impl CalxBenchmarkCorpus {
  /// Create a corpus with complete schemas for every top-level source definition.
  pub fn new(
    namespace: impl Into<Arc<str>>,
    source: impl Into<Arc<str>>,
    definitions: impl IntoIterator<Item = CalxBenchmarkDefinition>,
  ) -> Result<Self, String> {
    let namespace = namespace.into();
    if namespace.is_empty() {
      return Err("Calx benchmark corpus namespace must not be empty".to_owned());
    }
    let mut schemas = BTreeMap::new();
    for definition in definitions {
      if definition.name.is_empty() {
        return Err("Calx benchmark definition name must not be empty".to_owned());
      }
      if schemas.insert(definition.name.clone(), definition.schema).is_some() {
        return Err(format!("duplicate Calx benchmark schema for {}/{}", namespace, definition.name));
      }
    }
    if schemas.is_empty() {
      return Err("Calx benchmark corpus must declare at least one definition".to_owned());
    }
    Ok(Self {
      namespace,
      source: source.into(),
      definitions: schemas,
    })
  }

  /// Corpus namespace installed for the lifetime of one session.
  pub fn namespace(&self) -> &str {
    &self.namespace
  }

  fn install(&self) -> Result<(), String> {
    let mut remaining = self.definitions.clone();
    let mut source_names: HashSet<Arc<str>> = HashSet::new();
    let mut definitions = HashMap::new();
    for node in cirru_parser::parse(&self.source)? {
      let Cirru::List(items) = &node else {
        return Err(format!("Calx benchmark source definition must be a list: {node}"));
      };
      let Some(Cirru::Leaf(definition)) = items.get(1) else {
        return Err(format!("Calx benchmark source definition must have a name: {node}"));
      };
      if !source_names.insert(Arc::from(definition.as_ref())) {
        return Err(format!("duplicate Calx benchmark source definition: {definition}"));
      }
      let Some(schema) = remaining.remove(definition.as_ref()) else {
        return Err(format!(
          "Calx benchmark source {}/{} has no explicit schema",
          self.namespace, definition
        ));
      };
      let code = code_to_calcit(&node, &self.namespace, definition, vec![])?;
      definitions.insert(
        Arc::from(definition.as_ref()),
        ProgramDefEntry {
          code,
          schema,
          doc: Arc::from(""),
          examples: vec![],
          ffi: None,
        },
      );
    }
    if !remaining.is_empty() {
      let names = remaining.keys().map(AsRef::as_ref).collect::<Vec<&str>>().join(", ");
      return Err(format!("Calx benchmark schemas have no matching source definitions: {names}"));
    }
    program::install_internal_source_namespace(
      self.namespace.clone(),
      ProgramFileData {
        import_map: HashMap::new(),
        defs: definitions,
      },
    )
  }
}

/// Frontend stages measured while creating one immutable program session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalxBenchmarkSessionTimings {
  /// Parse, schema validation, and explicit source installation.
  pub source_install: Duration,
  /// Preprocessing of the selected entry and reachable definitions.
  pub frontend: Duration,
  /// Clone of the already-preprocessed immutable program handle.
  pub program_snapshot: Duration,
  /// Resolution of the cached Calcit callable stored by the session.
  pub cached_calcit_resolution: Duration,
}

/// Stable program-size evidence derived from one compiled Calx kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalxBenchmarkProgramCounts {
  /// Validated Calx function count.
  pub functions: usize,
  /// Validated host import count.
  pub imports: usize,
  /// Source-level validated syntax instruction count.
  pub syntax_nodes: usize,
  /// Lowered Calx instruction count.
  pub instructions: usize,
  /// Byte length of the deterministic diagnostic summary.
  pub diagnostic_bytes: usize,
}

/// Raw strict result returned before the adapter applies the proven result boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum CalxBenchmarkRawResult {
  /// A function with no result values completed.
  Void,
  /// One strict floating-point result.
  F64(f64),
  /// One strict boolean result.
  Bool(bool),
}

/// Reusable Calx VM owned by a benchmark call path.
pub struct CalxBenchmarkVm {
  vm: CalxVM,
}

impl CalxBenchmarkVm {
  /// Execute already-validated strict values without rebuilding the VM.
  pub fn run_values(&mut self, args: Vec<CalxValue>) -> Result<CalxBenchmarkRawResult, String> {
    match self.vm.run_typed(args).map_err(|error| error.to_string())? {
      CalxRunResult::Void => Ok(CalxBenchmarkRawResult::Void),
      CalxRunResult::Value(CalxValue::F64(value)) => Ok(CalxBenchmarkRawResult::F64(value)),
      CalxRunResult::Value(CalxValue::Bool(value)) => Ok(CalxBenchmarkRawResult::Bool(value)),
      CalxRunResult::Value(value) => Err(format!("validated scalar kernel produced unsupported VM value: {value:?}")),
    }
  }
}

/// Immutable benchmark program handle plus one cached Calcit callable.
///
/// The session owns the process-local adapter lock for its lifetime. Consumers can
/// compile and execute through methods here but cannot access compiler registries or
/// the underlying mutable source state.
pub struct CalxBenchmarkSession {
  _guard: MutexGuard<'static, ()>,
  namespace: Arc<str>,
  definition: Arc<str>,
  program: CompiledProgram,
  calcit_callable: Arc<CalcitFn>,
  timings: CalxBenchmarkSessionTimings,
}

impl CalxBenchmarkSession {
  /// Install a corpus, preprocess one entry, and freeze an immutable program handle.
  pub fn prepare(corpus: &CalxBenchmarkCorpus, definition: impl Into<Arc<str>>) -> Result<Self, String> {
    let definition = definition.into();
    if !corpus.definitions.contains_key(&definition) {
      return Err(format!(
        "Calx benchmark entry {}/{} has no explicit schema",
        corpus.namespace, definition
      ));
    }
    let guard = BENCHMARK_SESSION_LOCK
      .lock()
      .map_err(|error| format!("Calx benchmark session lock is poisoned: {error}"))?;

    let install_started = Instant::now();
    corpus.install()?;
    let source_install = install_started.elapsed();
    let result = (|| {
      let frontend_started = Instant::now();
      let warnings = RefCell::new(vec![]);
      runner::preprocess::ensure_ns_def_compiled(&corpus.namespace, &definition, &warnings, &CallStackList::default())
        .map_err(|error| error.to_string())?;
      if !warnings.borrow().is_empty() {
        return Err(format!("Calx benchmark frontend produced warnings: {:#?}", warnings.borrow()));
      }
      let frontend = frontend_started.elapsed();

      let snapshot_started = Instant::now();
      let program = program::clone_existing_compiled_program();
      let program_snapshot = snapshot_started.elapsed();

      let resolution_started = Instant::now();
      let calcit_callable = match runner::evaluate_symbol_from_program(&definition, &corpus.namespace, None, &CallStackList::default())
        .map_err(|error| error.to_string())?
      {
        Calcit::Fn { info, .. } => info,
        value => return Err(format!("expected cached benchmark callable, found {value}")),
      };
      let cached_calcit_resolution = resolution_started.elapsed();

      Ok((
        program,
        calcit_callable,
        CalxBenchmarkSessionTimings {
          source_install,
          frontend,
          program_snapshot,
          cached_calcit_resolution,
        },
      ))
    })();

    match result {
      Ok((program, calcit_callable, timings)) => Ok(Self {
        _guard: guard,
        namespace: corpus.namespace.clone(),
        definition,
        program,
        calcit_callable,
        timings,
      }),
      Err(error) => {
        program::remove_internal_source_namespace(&corpus.namespace);
        Err(error)
      }
    }
  }

  /// Adapter contract identifier; consumers record this with their pinned revision.
  pub const fn edition(&self) -> &'static str {
    CALX_BENCHMARK_SESSION_EDITION
  }

  /// Prepared entry namespace.
  pub fn namespace(&self) -> &str {
    &self.namespace
  }

  /// Prepared entry definition.
  pub fn definition(&self) -> &str {
    &self.definition
  }

  /// Measured source, frontend, snapshot, and cached-callable stages.
  pub const fn timings(&self) -> CalxBenchmarkSessionTimings {
    self.timings
  }

  /// Execute through normal symbol lookup for one correctness baseline.
  pub fn run_calcit_lookup(&self, args: &[Calcit]) -> Result<Calcit, String> {
    crate::run_program_with_docs(self.namespace.clone(), self.definition.clone(), args).map_err(|error| error.to_string())
  }

  /// Execute the callable resolved once when the session was prepared.
  pub fn run_calcit_cached(&self, args: &[Calcit]) -> Result<Calcit, String> {
    runner::run_fn(args, &self.calcit_callable, &CallStackList::default()).map_err(|error| error.to_string())
  }

  /// Compile one strict Calx kernel and return complete compile-stage timings.
  pub fn compile_calx_measured(
    &self,
    imports: &CalxHostImports,
  ) -> Result<(CalxPreparedKernel, CalxKernelCompileTimings), CalxKernelCompileError> {
    compile_calx_kernel_with_imports_measured(&self.program, self.namespace.clone(), self.definition.clone(), imports)
  }

  /// Compile one strict Calx kernel without retaining measurement stages.
  pub fn compile_calx(&self, imports: &CalxHostImports) -> Result<CalxPreparedKernel, CalxKernelCompileError> {
    self.compile_calx_measured(imports).map(|(kernel, _)| kernel)
  }

  /// Validate or populate an embedding-owned revision-safe artifact cache.
  pub fn prepare_cached_calx(
    &self,
    cache: &mut CalxCompileCache,
    imports: &CalxHostImports,
  ) -> Result<CalxCachePreparation, CalxKernelCompileError> {
    cache.prepare(&self.program, self.namespace.clone(), self.definition.clone(), imports)
  }

  /// Convert Calcit arguments through one compiled kernel's strict scalar boundary.
  pub fn encode_calx_arguments(kernel: &CalxPreparedKernel, args: &[Calcit]) -> Result<Vec<CalxValue>, String> {
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

  /// Create a fresh VM while keeping the underlying Calx program private to the adapter.
  pub fn instantiate_calx(kernel: &CalxPreparedKernel) -> Result<CalxBenchmarkVm, CalxKernelRunError> {
    kernel.instantiate().map(|vm| CalxBenchmarkVm { vm })
  }

  /// Decode a raw VM result through one compiled kernel's proven result boundary.
  pub fn decode_calx_result(kernel: &CalxPreparedKernel, result: CalxBenchmarkRawResult) -> Result<Calcit, String> {
    match (kernel.result(), result) {
      (None, CalxBenchmarkRawResult::Void) => Ok(Calcit::Unit),
      (Some(CalxScalarType::F64), CalxBenchmarkRawResult::F64(value)) => Ok(Calcit::Number(value)),
      (Some(CalxScalarType::Bool), CalxBenchmarkRawResult::Bool(value)) => Ok(Calcit::Bool(value)),
      (expected, actual) => Err(format!("validated result contract {expected:?} produced {actual:?}")),
    }
  }

  /// Derive stable size counts without exposing the immutable program handle.
  pub fn program_counts(kernel: &CalxPreparedKernel) -> CalxBenchmarkProgramCounts {
    let validated = kernel.validated_program();
    CalxBenchmarkProgramCounts {
      functions: validated.functions().len(),
      imports: validated.imports().len(),
      syntax_nodes: validated.functions().iter().map(|function| function.syntax.len()).sum(),
      instructions: validated.functions().iter().map(|function| function.instrs.len()).sum(),
      diagnostic_bytes: kernel.stable_program_summary().len(),
    }
  }
}

impl Drop for CalxBenchmarkSession {
  fn drop(&mut self) {
    program::remove_internal_source_namespace(&self.namespace);
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashSet;

  use crate::calcit::{CalcitFnTypeAnnotation, SchemaKind};

  use super::*;

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

  fn scalar_corpus() -> CalxBenchmarkCorpus {
    CalxBenchmarkCorpus::new(
      "test.calx-benchmark-session",
      "defn add-one (x)\n  &+ x 1",
      [CalxBenchmarkDefinition::new("add-one", number_fn_schema(1))],
    )
    .expect("declare scalar corpus")
  }

  #[test]
  fn pinned_session_runs_equivalent_calcit_and_calx_paths() {
    let session = CalxBenchmarkSession::prepare(&scalar_corpus(), "add-one").expect("prepare benchmark session");
    assert_eq!(session.edition(), CALX_BENCHMARK_SESSION_EDITION);
    assert_eq!(session.namespace(), "test.calx-benchmark-session");
    assert_eq!(session.definition(), "add-one");

    let calcit_args = vec![Calcit::Number(4.0)];
    let lookup = session.run_calcit_lookup(&calcit_args).expect("run lookup path");
    let cached = session.run_calcit_cached(&calcit_args).expect("run cached path");
    assert_eq!(lookup, Calcit::Number(5.0));
    assert_eq!(cached, lookup);

    let imports = CalxHostImports::new();
    let (kernel, compile_timings) = session.compile_calx_measured(&imports).expect("compile strict kernel");
    assert!(compile_timings.total > Duration::ZERO);
    let calx_args = CalxBenchmarkSession::encode_calx_arguments(&kernel, &calcit_args).expect("encode strict arguments");
    let mut vm = CalxBenchmarkSession::instantiate_calx(&kernel).expect("create benchmark VM");
    let raw = vm.run_values(calx_args).expect("execute strict kernel");
    let calx = CalxBenchmarkSession::decode_calx_result(&kernel, raw).expect("decode strict result");
    assert_eq!(calx, lookup);

    let counts = CalxBenchmarkSession::program_counts(&kernel);
    assert_eq!(counts.functions, 1);
    assert!(counts.syntax_nodes > 0);
    assert!(counts.instructions > 0);

    let mut cache = CalxCompileCache::new(1);
    let miss = session.prepare_cached_calx(&mut cache, &imports).expect("populate artifact cache");
    assert!(!miss.report().cache_hit);
    let hit = session.prepare_cached_calx(&mut cache, &imports).expect("reuse artifact cache");
    assert!(hit.report().cache_hit);
  }

  #[test]
  fn corpus_rejects_incomplete_schema_sets_without_installing_source() {
    let corpus = CalxBenchmarkCorpus::new(
      "test.calx-benchmark-session-invalid",
      "defn add-one (x)\n  &+ x 1",
      [
        CalxBenchmarkDefinition::new("add-one", number_fn_schema(1)),
        CalxBenchmarkDefinition::new("missing", number_fn_schema(1)),
      ],
    )
    .expect("declare intentionally mismatched corpus");
    let error = CalxBenchmarkSession::prepare(&corpus, "add-one")
      .err()
      .expect("missing source definition must fail");
    assert!(error.contains("missing"));
    assert!(
      !program::PROGRAM_CODE_DATA
        .read()
        .expect("read source registry")
        .contains_key(corpus.namespace())
    );
  }

  #[test]
  fn session_drop_releases_its_source_namespace() {
    let corpus = scalar_corpus();
    {
      let _session = CalxBenchmarkSession::prepare(&corpus, "add-one").expect("prepare benchmark session");
      assert!(
        program::PROGRAM_CODE_DATA
          .read()
          .expect("read source registry")
          .contains_key(corpus.namespace())
      );
    }
    assert!(
      !program::PROGRAM_CODE_DATA
        .read()
        .expect("read source registry")
        .contains_key(corpus.namespace())
    );
  }
}
