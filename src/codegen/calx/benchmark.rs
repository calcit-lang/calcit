//! Revision-pinned internal adapter for the standalone Calx benchmark harness.
//!
//! This API deliberately has no semver compatibility promise. It encapsulates
//! Calcit's process-level preprocessing registries behind one serialized
//! session, then exposes only an immutable compiled-program snapshot, cached
//! callable handles, strict scalar conversion, Calx compilation, and stable
//! report inputs. Benchmark iteration and statistics policy stay outside core.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use calx_vm::CalxRunResult;
use cirru_parser::Cirru;

use crate::calcit::{Calcit, CalcitFn, CalcitFnTypeAnnotation, CalcitTypeAnnotation, SchemaKind, brief_type_of_value};
use crate::call_stack::CallStackList;
use crate::data::cirru::code_to_calcit;
use crate::program::{CompiledProgram, ProgramDefEntry, ProgramFileData, clone_existing_compiled_program, replace_benchmark_namespace};
use crate::run_program_with_docs;
use crate::runner;
use crate::runner::preprocess::ensure_ns_def_compiled;

use super::{
  CalxCachePreparation, CalxCompileCache, CalxHostImports, CalxKernelCompileError, CalxKernelCompileTimings, CalxPreparedKernel,
  CalxScalarType, CalxValue, compile_calx_kernel_with_imports, compile_calx_kernel_with_imports_measured,
};

static BENCHMARK_SESSION_LOCK: Mutex<()> = Mutex::new(());

/// Stage assigned to one adapter failure without leaking registry details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalxBenchmarkAdapterStage {
  Corpus,
  Install,
  Preprocess,
  Snapshot,
  Resolve,
  Compile,
  NativeRun,
  Boundary,
}

impl CalxBenchmarkAdapterStage {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Corpus => "corpus",
      Self::Install => "install",
      Self::Preprocess => "preprocess",
      Self::Snapshot => "snapshot",
      Self::Resolve => "resolve",
      Self::Compile => "compile",
      Self::NativeRun => "native-run",
      Self::Boundary => "boundary",
    }
  }
}

/// Structured adapter failure suitable for a benchmark process diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxBenchmarkAdapterError {
  stage: CalxBenchmarkAdapterStage,
  message: String,
}

impl CalxBenchmarkAdapterError {
  fn new(stage: CalxBenchmarkAdapterStage, message: impl Into<String>) -> Self {
    Self {
      stage,
      message: message.into(),
    }
  }

  pub const fn stage(&self) -> CalxBenchmarkAdapterStage {
    self.stage
  }

  pub fn message(&self) -> &str {
    &self.message
  }
}

impl fmt::Display for CalxBenchmarkAdapterError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Calx benchmark adapter {} failed: {}", self.stage.as_str(), self.message)
  }
}

impl Error for CalxBenchmarkAdapterError {}

/// Explicit benchmark return contract; unit is never encoded as Nil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalxBenchmarkReturn {
  Unit,
  Scalar(CalxScalarType),
}

impl CalxBenchmarkReturn {
  const fn scalar(self) -> Option<CalxScalarType> {
    match self {
      Self::Unit => None,
      Self::Scalar(value) => Some(value),
    }
  }
}

/// One declared function in an explicit benchmark source corpus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxBenchmarkDefinition {
  name: Arc<str>,
  params: Vec<CalxScalarType>,
  result: CalxBenchmarkReturn,
}

impl CalxBenchmarkDefinition {
  pub fn new(
    name: impl Into<Arc<str>>,
    params: Vec<CalxScalarType>,
    result: CalxBenchmarkReturn,
  ) -> Result<Self, CalxBenchmarkAdapterError> {
    let name = name.into();
    if name.is_empty() {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Corpus,
        "definition name must not be empty",
      ));
    }
    Ok(Self { name, params, result })
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn params(&self) -> &[CalxScalarType] {
    &self.params
  }

  pub const fn result(&self) -> CalxBenchmarkReturn {
    self.result
  }
}

/// Named source corpus with a complete, fixed scalar schema table.
#[derive(Debug, Clone)]
pub struct CalxBenchmarkCorpus {
  namespace: Arc<str>,
  source: Arc<str>,
  definitions: BTreeMap<Arc<str>, CalxBenchmarkDefinition>,
}

impl CalxBenchmarkCorpus {
  pub fn new(
    namespace: impl Into<Arc<str>>,
    source: impl Into<Arc<str>>,
    definitions: Vec<CalxBenchmarkDefinition>,
  ) -> Result<Self, CalxBenchmarkAdapterError> {
    let namespace = namespace.into();
    if namespace.is_empty() {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Corpus,
        "namespace must not be empty",
      ));
    }
    let source = source.into();
    if source.trim().is_empty() {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Corpus,
        "source corpus must not be empty",
      ));
    }
    if definitions.is_empty() {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Corpus,
        "at least one explicit function schema is required",
      ));
    }

    let mut by_name = BTreeMap::new();
    for definition in definitions {
      let name = definition.name.clone();
      if by_name.insert(name.clone(), definition).is_some() {
        return Err(CalxBenchmarkAdapterError::new(
          CalxBenchmarkAdapterStage::Corpus,
          format!("duplicate schema declaration for `{name}`"),
        ));
      }
    }
    Ok(Self {
      namespace,
      source,
      definitions: by_name,
    })
  }
}

/// One-time source installation, preprocessing, and snapshot timings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalxBenchmarkSessionTimings {
  pub source_install: Duration,
  pub preprocess: Duration,
  pub snapshot_clone: Duration,
}

/// Prepared session plus its one-time stage timings.
pub struct CalxBenchmarkSessionPreparation {
  session: CalxBenchmarkSession,
  timings: CalxBenchmarkSessionTimings,
}

impl CalxBenchmarkSessionPreparation {
  pub const fn timings(&self) -> CalxBenchmarkSessionTimings {
    self.timings
  }

  pub fn into_session(self) -> CalxBenchmarkSession {
    self.session
  }

  pub fn into_parts(self) -> (CalxBenchmarkSession, CalxBenchmarkSessionTimings) {
    (self.session, self.timings)
  }
}

/// Stable program metrics consumed by the external report schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalxBenchmarkProgramCounts {
  pub functions: usize,
  pub imports: usize,
  pub syntax_nodes: usize,
  pub instructions: usize,
  pub diagnostic_bytes: usize,
}

/// Cached Calcit function bound to the session that resolved it.
pub struct CalxBenchmarkCalcitCallable<'session> {
  session: &'session CalxBenchmarkSession,
  definition: Arc<str>,
  callable: Arc<CalcitFn>,
}

impl CalxBenchmarkCalcitCallable<'_> {
  /// Run the cached function while its source session still owns the runtime corpus.
  pub fn run(&self, args: &[Calcit]) -> Result<Calcit, CalxBenchmarkAdapterError> {
    let contract = self.session.definition(&self.definition)?;
    let qualified_name = format!("{}/{}", self.session.namespace, self.definition);
    validate_calcit_arguments(&qualified_name, &contract.params, args)?;
    let result = runner::run_fn(args, &self.callable, &CallStackList::default())
      .map_err(|error| CalxBenchmarkAdapterError::new(CalxBenchmarkAdapterStage::NativeRun, error.to_string()))?;
    validate_calcit_result(&qualified_name, contract.result, &result)?;
    Ok(result)
  }
}

/// Serialized benchmark session backed by one immutable compiled snapshot.
///
/// The guard intentionally prevents multiple active adapters in one process.
/// A harness uses fresh processes for samples, so concurrent or nested corpus
/// installation would be an API misuse rather than a supported workload.
pub struct CalxBenchmarkSession {
  namespace: Arc<str>,
  definitions: BTreeMap<Arc<str>, CalxBenchmarkDefinition>,
  program: Arc<CompiledProgram>,
  _session_guard: MutexGuard<'static, ()>,
}

impl CalxBenchmarkSession {
  pub fn prepare(corpus: CalxBenchmarkCorpus) -> Result<CalxBenchmarkSessionPreparation, CalxBenchmarkAdapterError> {
    let session_guard = BENCHMARK_SESSION_LOCK
      .lock()
      .map_err(|_| CalxBenchmarkAdapterError::new(CalxBenchmarkAdapterStage::Install, "benchmark session lock is poisoned"))?;

    let install_started = Instant::now();
    let file = parse_explicit_corpus(&corpus)?;
    replace_benchmark_namespace(corpus.namespace.clone(), file);
    let source_install = install_started.elapsed();

    let preprocess_started = Instant::now();
    let warnings = RefCell::new(vec![]);
    let call_stack = CallStackList::default();
    for definition in corpus.definitions.keys() {
      ensure_ns_def_compiled(&corpus.namespace, definition, &warnings, &call_stack).map_err(|error| {
        CalxBenchmarkAdapterError::new(
          CalxBenchmarkAdapterStage::Preprocess,
          format!("failed to preprocess `{}/{definition}`: {error}", corpus.namespace),
        )
      })?;
    }
    if !warnings.borrow().is_empty() {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Preprocess,
        format!("source corpus produced warnings: {:#?}", warnings.borrow()),
      ));
    }
    let preprocess = preprocess_started.elapsed();

    let snapshot_started = Instant::now();
    let program = Arc::new(clone_existing_compiled_program());
    let snapshot_clone = snapshot_started.elapsed();
    let Some(file) = program.get(&corpus.namespace) else {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Snapshot,
        format!("compiled snapshot is missing namespace `{}`", corpus.namespace),
      ));
    };
    for definition in corpus.definitions.keys() {
      if file.get(definition).is_none() {
        return Err(CalxBenchmarkAdapterError::new(
          CalxBenchmarkAdapterStage::Snapshot,
          format!("compiled snapshot is missing `{}/{definition}`", corpus.namespace),
        ));
      }
    }

    Ok(CalxBenchmarkSessionPreparation {
      session: Self {
        namespace: corpus.namespace,
        definitions: corpus.definitions,
        program,
        _session_guard: session_guard,
      },
      timings: CalxBenchmarkSessionTimings {
        source_install,
        preprocess,
        snapshot_clone,
      },
    })
  }

  /// Resolve a cached callable whose lifetime cannot outlive this session.
  pub fn resolve_calcit_callable(&self, definition: &str) -> Result<CalxBenchmarkCalcitCallable<'_>, CalxBenchmarkAdapterError> {
    self.definition(definition)?;
    let value = runner::evaluate_symbol_from_program(definition, &self.namespace, None, &CallStackList::default())
      .map_err(|error| CalxBenchmarkAdapterError::new(CalxBenchmarkAdapterStage::Resolve, error.to_string()))?;
    let Calcit::Fn { info, .. } = value else {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Resolve,
        format!("expected `{}/{definition}` to resolve to a function, found {value}", self.namespace),
      ));
    };
    Ok(CalxBenchmarkCalcitCallable {
      session: self,
      definition: Arc::from(definition),
      callable: info,
    })
  }

  pub fn run_calcit_lookup(&self, definition: &str, args: &[Calcit]) -> Result<Calcit, CalxBenchmarkAdapterError> {
    let contract = self.definition(definition)?;
    let qualified = format!("{}/{definition}", self.namespace);
    validate_calcit_arguments(&qualified, &contract.params, args)?;
    let result = run_program_with_docs(self.namespace.clone(), Arc::from(definition), args)
      .map_err(|error| CalxBenchmarkAdapterError::new(CalxBenchmarkAdapterStage::NativeRun, error.to_string()))?;
    validate_calcit_result(&qualified, contract.result, &result)?;
    Ok(result)
  }

  pub fn compile_calx(&self, definition: &str, imports: &CalxHostImports) -> Result<CalxPreparedKernel, CalxBenchmarkAdapterError> {
    self.definition(definition)?;
    let kernel =
      compile_calx_kernel_with_imports(&self.program, self.namespace.clone(), Arc::from(definition), imports).map_err(compile_error)?;
    self.verify_kernel_contract(definition, &kernel)?;
    Ok(kernel)
  }

  pub fn compile_calx_measured(
    &self,
    definition: &str,
    imports: &CalxHostImports,
  ) -> Result<(CalxPreparedKernel, CalxKernelCompileTimings), CalxBenchmarkAdapterError> {
    self.definition(definition)?;
    let (kernel, timings) =
      compile_calx_kernel_with_imports_measured(&self.program, self.namespace.clone(), Arc::from(definition), imports)
        .map_err(compile_error)?;
    self.verify_kernel_contract(definition, &kernel)?;
    Ok((kernel, timings))
  }

  pub fn prepare_calx_cache(
    &self,
    cache: &mut CalxCompileCache,
    definition: &str,
    imports: &CalxHostImports,
  ) -> Result<CalxCachePreparation, CalxBenchmarkAdapterError> {
    self.definition(definition)?;
    let preparation = cache
      .prepare(&self.program, self.namespace.clone(), Arc::from(definition), imports)
      .map_err(compile_error)?;
    self.verify_kernel_contract(definition, preparation.kernel())?;
    Ok(preparation)
  }

  pub fn prepare_calx_arguments(
    &self,
    definition: &str,
    kernel: &CalxPreparedKernel,
    args: &[Calcit],
  ) -> Result<Vec<CalxValue>, CalxBenchmarkAdapterError> {
    let contract = self.definition(definition)?;
    self.verify_kernel_contract(definition, kernel)?;
    let qualified = format!("{}/{definition}", self.namespace);
    validate_calcit_arguments(&qualified, &contract.params, args)?;
    Ok(
      args
        .iter()
        .zip(&contract.params)
        .map(|(value, expected)| match (value, expected) {
          (Calcit::Number(value), CalxScalarType::F64) => CalxValue::F64(*value),
          (Calcit::Bool(value), CalxScalarType::Bool) => CalxValue::Bool(*value),
          _ => unreachable!("arguments were validated against the same scalar contract"),
        })
        .collect(),
    )
  }

  pub fn finish_calx_result(
    &self,
    definition: &str,
    kernel: &CalxPreparedKernel,
    result: CalxRunResult,
  ) -> Result<Calcit, CalxBenchmarkAdapterError> {
    let contract = self.definition(definition)?;
    self.verify_kernel_contract(definition, kernel)?;
    let value = match (contract.result, result) {
      (CalxBenchmarkReturn::Unit, CalxRunResult::Void) => Calcit::Unit,
      (CalxBenchmarkReturn::Scalar(CalxScalarType::F64), CalxRunResult::Value(CalxValue::F64(value))) => Calcit::Number(value),
      (CalxBenchmarkReturn::Scalar(CalxScalarType::Bool), CalxRunResult::Value(CalxValue::Bool(value))) => Calcit::Bool(value),
      (expected, actual) => {
        return Err(CalxBenchmarkAdapterError::new(
          CalxBenchmarkAdapterStage::Boundary,
          format!("validated result contract {expected:?} produced {actual:?}"),
        ));
      }
    };
    Ok(value)
  }

  pub fn program_counts(
    &self,
    definition: &str,
    kernel: &CalxPreparedKernel,
  ) -> Result<CalxBenchmarkProgramCounts, CalxBenchmarkAdapterError> {
    self.verify_kernel_contract(definition, kernel)?;
    let validated = kernel.validated_program();
    Ok(CalxBenchmarkProgramCounts {
      functions: validated.functions().len(),
      imports: validated.imports().len(),
      syntax_nodes: validated.functions().iter().map(|function| function.syntax.len()).sum(),
      instructions: validated.functions().iter().map(|function| function.instrs.len()).sum(),
      diagnostic_bytes: kernel.stable_program_summary().len(),
    })
  }

  fn definition(&self, definition: &str) -> Result<&CalxBenchmarkDefinition, CalxBenchmarkAdapterError> {
    self.definitions.get(definition).ok_or_else(|| {
      CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Resolve,
        format!("benchmark corpus `{}` does not declare `{definition}`", self.namespace),
      )
    })
  }

  fn verify_kernel_contract(&self, definition: &str, kernel: &CalxPreparedKernel) -> Result<(), CalxBenchmarkAdapterError> {
    let contract = self.definition(definition)?;
    if kernel.params() != contract.params || kernel.result() != contract.result.scalar() {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Boundary,
        format!(
          "compiled kernel `{}/{definition}` has {:?}->{:?}, expected {:?}->{:?}",
          self.namespace,
          kernel.params(),
          kernel.result(),
          contract.params,
          contract.result
        ),
      ));
    }
    Ok(())
  }
}

fn compile_error(error: CalxKernelCompileError) -> CalxBenchmarkAdapterError {
  CalxBenchmarkAdapterError::new(CalxBenchmarkAdapterStage::Compile, error.to_string())
}

fn parse_explicit_corpus(corpus: &CalxBenchmarkCorpus) -> Result<ProgramFileData, CalxBenchmarkAdapterError> {
  let nodes =
    cirru_parser::parse(&corpus.source).map_err(|error| CalxBenchmarkAdapterError::new(CalxBenchmarkAdapterStage::Corpus, error))?;
  let mut definitions = HashMap::new();
  for node in nodes {
    let Cirru::List(items) = &node else {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Corpus,
        format!("definition must be a list: {node}"),
      ));
    };
    let Some(Cirru::Leaf(name)) = items.get(1) else {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Corpus,
        format!("definition must have a name: {node}"),
      ));
    };
    let Some(contract) = corpus.definitions.get(name.as_ref()) else {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Corpus,
        format!("source definition `{name}` has no explicit schema"),
      ));
    };
    let code = code_to_calcit(&node, &corpus.namespace, name, vec![])
      .map_err(|error| CalxBenchmarkAdapterError::new(CalxBenchmarkAdapterStage::Corpus, error.to_string()))?;
    let entry = ProgramDefEntry {
      code,
      schema: function_schema(contract),
      doc: Arc::from(""),
      examples: vec![],
      ffi: None,
    };
    if definitions.insert(Arc::from(name.as_ref()), entry).is_some() {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Corpus,
        format!("source corpus defines `{name}` more than once"),
      ));
    }
  }
  for name in corpus.definitions.keys() {
    if !definitions.contains_key(name) {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Corpus,
        format!("schema declaration `{name}` has no source definition"),
      ));
    }
  }
  Ok(ProgramFileData {
    import_map: HashMap::new(),
    defs: definitions,
  })
}

fn function_schema(definition: &CalxBenchmarkDefinition) -> Arc<CalcitTypeAnnotation> {
  let arg_types = definition.params.iter().copied().map(scalar_annotation).collect();
  let return_type = match definition.result {
    CalxBenchmarkReturn::Unit => Arc::new(CalcitTypeAnnotation::Unit),
    CalxBenchmarkReturn::Scalar(value) => scalar_annotation(value),
  };
  Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
    generics: Arc::new(vec![]),
    where_bounds: Arc::new(vec![]),
    arg_types,
    return_type,
    fn_kind: SchemaKind::Fn,
    rest_type: None,
    features: Arc::new(HashSet::new()),
  })))
}

fn scalar_annotation(value: CalxScalarType) -> Arc<CalcitTypeAnnotation> {
  match value {
    CalxScalarType::F64 => Arc::new(CalcitTypeAnnotation::Number),
    CalxScalarType::Bool => Arc::new(CalcitTypeAnnotation::Bool),
  }
}

fn validate_calcit_arguments(qualified: &str, expected: &[CalxScalarType], args: &[Calcit]) -> Result<(), CalxBenchmarkAdapterError> {
  if args.len() != expected.len() {
    return Err(CalxBenchmarkAdapterError::new(
      CalxBenchmarkAdapterStage::Boundary,
      format!("`{qualified}` expected {} arguments, found {}", expected.len(), args.len()),
    ));
  }
  for (index, (value, scalar)) in args.iter().zip(expected).enumerate() {
    let matches = matches!(
      (value, scalar),
      (Calcit::Number(_), CalxScalarType::F64) | (Calcit::Bool(_), CalxScalarType::Bool)
    );
    if !matches {
      return Err(CalxBenchmarkAdapterError::new(
        CalxBenchmarkAdapterStage::Boundary,
        format!(
          "`{qualified}` argument {index} expected {}, found {}",
          scalar.as_str(),
          brief_type_of_value(value)
        ),
      ));
    }
  }
  Ok(())
}

fn validate_calcit_result(qualified: &str, expected: CalxBenchmarkReturn, value: &Calcit) -> Result<(), CalxBenchmarkAdapterError> {
  let matches = matches!(
    (expected, value),
    (CalxBenchmarkReturn::Unit, Calcit::Unit)
      | (CalxBenchmarkReturn::Scalar(CalxScalarType::F64), Calcit::Number(_))
      | (CalxBenchmarkReturn::Scalar(CalxScalarType::Bool), Calcit::Bool(_))
  );
  if matches {
    Ok(())
  } else {
    Err(CalxBenchmarkAdapterError::new(
      CalxBenchmarkAdapterStage::Boundary,
      format!("`{qualified}` expected result {expected:?}, found {}", brief_type_of_value(value)),
    ))
  }
}
