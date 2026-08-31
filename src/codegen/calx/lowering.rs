//! Typed lowering from a proven Calx-eligible Calcit call graph.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use calx_vm::{
  BodyBuilder, Calx as VmValue, CalxBuildError, CalxError, CalxHostBindings, CalxProgramError, CalxRunResult, CalxSyntax as VmSyntax,
  CalxVM, FunctionBuilder, ImportId, LocalId, ProgramBuilder, SourceSpan, ValidatedProgram,
};

use crate::calcit::{Calcit, CalcitFnArgs, CalcitLocal, CalcitProc, CalcitSyntax, CalcitTypeAnnotation, brief_type_of_value};
use crate::program::{CompiledProgram, DefId};

use super::{
  CalxDefinitionRef, CalxEligibleCallGraph, CalxFallbackReport, CalxHostImports, CalxImportContract, CalxScalarType,
  analyze_calx_eligibility_with_imports, extract_fn_parts, import_contract, lookup_compiled_def, source_path,
};

/// A mismatch discovered while converting a graph that already passed the
/// eligibility proof. This is a compiler integration error, not a fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxLoweringError {
  pub function: CalxDefinitionRef,
  pub source_path: Option<Vec<u16>>,
  pub message: String,
}

impl fmt::Display for CalxLoweringError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "failed to lower `{}`: {}", self.function.qualified(), self.message)
  }
}

impl Error for CalxLoweringError {}

/// Compile-time phases remain distinct so only eligibility failures may be
/// treated as a deliberate whole-kernel fallback by an embedding.
#[derive(Debug)]
pub enum CalxKernelCompileError {
  Eligibility(CalxFallbackReport),
  Lowering(CalxLoweringError),
  Build(CalxBuildError),
  Validation(CalxProgramError),
}

impl fmt::Display for CalxKernelCompileError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Eligibility(report) => write!(
        f,
        "Calx eligibility rejected `{}` with {} issue(s)",
        report.entry.qualified(),
        report.issues.len()
      ),
      Self::Lowering(error) => error.fmt(f),
      Self::Build(error) => write!(f, "Calx program build failed: {error}"),
      Self::Validation(error) => write!(f, "Calx program validation failed: {error}"),
    }
  }
}

impl Error for CalxKernelCompileError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      Self::Eligibility(_) => None,
      Self::Lowering(error) => Some(error),
      Self::Build(error) => Some(error),
      Self::Validation(error) => Some(error),
    }
  }
}

/// Opt-in wall-clock measurements for the Calx compilation stages.
///
/// These durations are evidence for benchmark reports, not stable performance
/// thresholds. Program construction includes declaration emission and
/// `ProgramBuilder::build`; validation includes strict validation and Calx
/// instruction lowering in `ValidatedProgram::try_from_program`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalxKernelCompileTimings {
  pub eligibility: Duration,
  pub planning: Duration,
  pub program_construction: Duration,
  pub validation_lowering: Duration,
  pub total: Duration,
}

#[derive(Debug, Default)]
pub(super) struct CalxKernelCompileStageTimings {
  eligibility: Duration,
  planning: Duration,
  program_construction: Duration,
  validation_lowering: Duration,
}

impl From<CalxLoweringError> for CalxKernelCompileError {
  fn from(error: CalxLoweringError) -> Self {
    Self::Lowering(error)
  }
}

impl From<CalxBuildError> for CalxKernelCompileError {
  fn from(error: CalxBuildError) -> Self {
    Self::Build(error)
  }
}

impl From<CalxProgramError> for CalxKernelCompileError {
  fn from(error: CalxProgramError) -> Self {
    Self::Validation(error)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalxKernelBoundaryErrorKind {
  Arity,
  ArgumentType,
  ResultType,
}

/// Failure at the strict Calcit/Calx value boundary. No conversion to Nil or
/// Dynamic is attempted when a value does not match the proven signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxKernelBoundaryError {
  pub kind: CalxKernelBoundaryErrorKind,
  pub message: String,
}

impl fmt::Display for CalxKernelBoundaryError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.message)
  }
}

impl Error for CalxKernelBoundaryError {}

#[derive(Debug)]
pub enum CalxKernelRunError {
  Boundary(CalxKernelBoundaryError),
  Instantiate(CalxProgramError),
  Runtime(CalxError),
}

impl fmt::Display for CalxKernelRunError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Boundary(error) => write!(f, "Calx kernel boundary rejected a value: {error}"),
      Self::Instantiate(error) => write!(f, "Calx kernel instantiation failed: {error}"),
      Self::Runtime(error) => write!(f, "Calx kernel trapped: {error}"),
    }
  }
}

impl Error for CalxKernelRunError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      Self::Boundary(error) => Some(error),
      Self::Instantiate(error) => Some(error),
      // calx_vm 0.3 keeps CalxError as a diagnostic value rather than a
      // std::error::Error implementation.
      Self::Runtime(_) => None,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CalxDefinitionStamp {
  pub definition: CalxDefinitionRef,
  pub def_id: DefId,
  pub preprocessed_code: Calcit,
  pub schema: Arc<CalcitTypeAnnotation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CalxImportSchemaStamp {
  pub definition: CalxDefinitionRef,
  pub schema: Arc<CalcitTypeAnnotation>,
}

/// Immutable source-derived output of eligibility, lowering, and validation.
///
/// Host callbacks, VM instances, runtime values, and capability state are
/// deliberately absent so this value can be shared by a revision-safe cache.
#[derive(Debug, Clone)]
pub struct CalxCompiledArtifact {
  pub(super) graph: CalxEligibleCallGraph,
  pub(super) params: Vec<CalxScalarType>,
  pub(super) result: Option<CalxScalarType>,
  pub(super) program: ValidatedProgram,
  pub(super) reachable_stamps: Vec<CalxDefinitionStamp>,
  pub(super) import_schema_stamps: Vec<CalxImportSchemaStamp>,
  pub(super) import_contract: Vec<CalxImportContract>,
}

impl CalxCompiledArtifact {
  pub fn graph(&self) -> &CalxEligibleCallGraph {
    &self.graph
  }

  pub fn params(&self) -> &[CalxScalarType] {
    &self.params
  }

  pub fn result(&self) -> Option<CalxScalarType> {
    self.result
  }

  pub fn validated_program(&self) -> &ValidatedProgram {
    &self.program
  }

  pub fn import_contract(&self) -> &[CalxImportContract] {
    &self.import_contract
  }

  pub fn reachable_definition_count(&self) -> usize {
    self.reachable_stamps.len()
  }

  pub fn syntax_instruction_count(&self) -> usize {
    self.program.functions().iter().map(|function| function.syntax.len()).sum()
  }

  pub fn lowered_instruction_count(&self) -> usize {
    self.program.functions().iter().map(|function| function.instrs.len()).sum()
  }

  /// Deterministic shallow estimate used for bounded-cache observability.
  ///
  /// Shared persistent Calcit storage is intentionally not walked recursively;
  /// the estimate counts owned vectors, declarations, instructions, and names.
  pub fn estimated_bytes(&self) -> usize {
    let mut bytes = std::mem::size_of::<Self>();
    bytes += self.params.len() * std::mem::size_of::<CalxScalarType>();
    bytes += self.reachable_stamps.len() * std::mem::size_of::<CalxDefinitionStamp>();
    bytes += self.import_schema_stamps.len() * std::mem::size_of::<CalxImportSchemaStamp>();
    bytes += self.import_contract.len() * std::mem::size_of::<CalxImportContract>();
    for stamp in &self.reachable_stamps {
      bytes += stamp.definition.namespace.len() + stamp.definition.definition.len();
      bytes += std::mem::size_of_val(&stamp.preprocessed_code) + std::mem::size_of_val(stamp.schema.as_ref());
    }
    for stamp in &self.import_schema_stamps {
      bytes += stamp.definition.namespace.len() + stamp.definition.definition.len();
      bytes += std::mem::size_of_val(stamp.schema.as_ref());
    }
    for contract in &self.import_contract {
      bytes += contract.definition.namespace.len() + contract.definition.definition.len() + contract.export_name.len();
      bytes += contract.params.len() * std::mem::size_of::<CalxScalarType>();
    }
    for function in self.program.functions() {
      bytes += function.name.len();
      bytes += std::mem::size_of_val(function.syntax.as_slice());
      bytes += std::mem::size_of_val(function.instrs.as_slice());
    }
    bytes
  }

  /// Deterministic generated-program report for repository golden fixtures.
  /// This is diagnostic text, not a serialized Calx program ABI.
  pub fn stable_program_summary(&self) -> String {
    let mut output = format!("abi {}\nentry {}\n", self.graph.abi_edition, self.graph.entry.qualified());
    for import in self.program.imports() {
      let params = import.params.iter().map(ToString::to_string).collect::<Vec<_>>().join(",");
      let result = import.result.map(|value| value.to_string()).unwrap_or_else(|| "Void".to_owned());
      output.push_str(&format!("import {} ({params})->{result}\n", import.name));
    }
    for function in self.program.functions() {
      let params = function
        .params_types
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(",");
      let result = function
        .ret_types
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(",");
      let result = if result.is_empty() { "Void" } else { result.as_str() };
      output.push_str(&format!("function {} ({params})->{result}\n", function.name));
      for (index, syntax) in function.syntax.iter().enumerate() {
        let origin = function
          .source_spans
          .get(index)
          .and_then(Option::as_ref)
          .map(|span| span.source.as_ref())
          .unwrap_or("-");
        output.push_str(&format!("  {index:03} {syntax:?} @ {origin}\n"));
      }
    }
    output
  }
}

/// Per-request executable kernel with freshly attached host capabilities.
#[derive(Debug, Clone)]
pub struct CalxPreparedKernel {
  artifact: Rc<CalxCompiledArtifact>,
  host_bindings: CalxHostBindings,
}

/// Compatibility name for the pre-cache API.
pub type CalxCompiledKernel = CalxPreparedKernel;

impl CalxPreparedKernel {
  pub fn artifact(&self) -> &Rc<CalxCompiledArtifact> {
    &self.artifact
  }

  pub fn graph(&self) -> &CalxEligibleCallGraph {
    self.artifact.graph()
  }

  pub fn params(&self) -> &[CalxScalarType] {
    self.artifact.params()
  }

  pub fn result(&self) -> Option<CalxScalarType> {
    self.artifact.result()
  }

  pub fn validated_program(&self) -> &ValidatedProgram {
    self.artifact.validated_program()
  }

  /// Deterministic generated-program report for repository golden fixtures.
  /// This is diagnostic text, not a serialized Calx program ABI.
  pub fn stable_program_summary(&self) -> String {
    self.artifact.stable_program_summary()
  }

  pub fn instantiate(&self) -> Result<CalxVM, CalxKernelRunError> {
    CalxVM::from_validated_program(self.artifact.program.clone(), self.host_bindings.clone()).map_err(CalxKernelRunError::Instantiate)
  }

  pub fn run(&self, args: &[Calcit]) -> Result<Calcit, CalxKernelRunError> {
    if args.len() != self.artifact.params.len() {
      return Err(CalxKernelRunError::Boundary(CalxKernelBoundaryError {
        kind: CalxKernelBoundaryErrorKind::Arity,
        message: format!("expected {} arguments, found {}", self.artifact.params.len(), args.len()),
      }));
    }
    let mut vm_args = Vec::with_capacity(args.len());
    for (index, (value, expected)) in args.iter().zip(&self.artifact.params).enumerate() {
      let converted = match (expected, value) {
        (CalxScalarType::F64, Calcit::Number(value)) => VmValue::F64(*value),
        (CalxScalarType::Bool, Calcit::Bool(value)) => VmValue::Bool(*value),
        (CalxScalarType::F64Buffer, Calcit::F64Buffer(values)) => VmValue::f64_buffer_copy_from_slice(values),
        _ => {
          return Err(CalxKernelRunError::Boundary(CalxKernelBoundaryError {
            kind: CalxKernelBoundaryErrorKind::ArgumentType,
            message: format!(
              "argument {index} expected {}, found {}",
              expected.as_str(),
              brief_type_of_value(value)
            ),
          }));
        }
      };
      vm_args.push(converted);
    }

    let mut vm = self.instantiate()?;
    let output = vm.run_typed(vm_args).map_err(CalxKernelRunError::Runtime)?;
    match (self.artifact.result, output) {
      (None, CalxRunResult::Void) => Ok(Calcit::Unit),
      (Some(CalxScalarType::F64), CalxRunResult::Value(VmValue::F64(value))) => Ok(Calcit::Number(value)),
      (Some(CalxScalarType::Bool), CalxRunResult::Value(VmValue::Bool(value))) => Ok(Calcit::Bool(value)),
      (Some(CalxScalarType::F64Buffer), CalxRunResult::Value(VmValue::F64Buffer(values))) => {
        Ok(Calcit::F64Buffer(std::sync::Arc::from(values.as_ref())))
      }
      (expected, actual) => Err(CalxKernelRunError::Boundary(CalxKernelBoundaryError {
        kind: CalxKernelBoundaryErrorKind::ResultType,
        message: format!("validated result contract {expected:?} produced {actual:?}"),
      })),
    }
  }
}

/// Prove, lower, build, and validate one closed scalar kernel call graph.
///
/// Only [`CalxKernelCompileError::Eligibility`] is a supported signal for
/// whole-kernel fallback. Later failures indicate a compiler/VM integration
/// problem and must not cause an automatic retry in the Calcit runtime.
pub fn compile_calx_kernel(
  program: &CompiledProgram,
  namespace: impl Into<Arc<str>>,
  definition: impl Into<Arc<str>>,
) -> Result<CalxCompiledKernel, CalxKernelCompileError> {
  compile_calx_kernel_with_imports(program, namespace, definition, &CalxHostImports::new())
}

/// Compile a closed scalar kernel and report each compilation-stage duration.
pub fn compile_calx_kernel_measured(
  program: &CompiledProgram,
  namespace: impl Into<Arc<str>>,
  definition: impl Into<Arc<str>>,
) -> Result<(CalxCompiledKernel, CalxKernelCompileTimings), CalxKernelCompileError> {
  compile_calx_kernel_with_imports_measured(program, namespace, definition, &CalxHostImports::new())
}

/// Compiles a kernel with an explicit set of typed scalar host capabilities.
pub fn compile_calx_kernel_with_imports(
  program: &CompiledProgram,
  namespace: impl Into<Arc<str>>,
  definition: impl Into<Arc<str>>,
  imports: &CalxHostImports,
) -> Result<CalxCompiledKernel, CalxKernelCompileError> {
  let artifact = compile_calx_artifact_with_imports_inner(program, namespace.into(), definition.into(), imports, None)?;
  prepare_calx_artifact(artifact, imports)
}

/// Compile with explicit typed imports and report each compilation-stage duration.
pub fn compile_calx_kernel_with_imports_measured(
  program: &CompiledProgram,
  namespace: impl Into<Arc<str>>,
  definition: impl Into<Arc<str>>,
  imports: &CalxHostImports,
) -> Result<(CalxCompiledKernel, CalxKernelCompileTimings), CalxKernelCompileError> {
  let total_started = Instant::now();
  let (artifact, mut timings) = compile_calx_artifact_with_imports_measured(program, namespace.into(), definition.into(), imports)?;
  let kernel = prepare_calx_artifact(artifact, imports)?;
  timings.total = total_started.elapsed();
  Ok((kernel, timings))
}

pub(super) fn compile_calx_artifact_with_imports_measured(
  program: &CompiledProgram,
  namespace: Arc<str>,
  definition: Arc<str>,
  imports: &CalxHostImports,
) -> Result<(Rc<CalxCompiledArtifact>, CalxKernelCompileTimings), CalxKernelCompileError> {
  let total_started = Instant::now();
  let mut stage_timings = CalxKernelCompileStageTimings::default();
  let artifact = compile_calx_artifact_with_imports_inner(program, namespace, definition, imports, Some(&mut stage_timings))?;
  Ok((
    artifact,
    CalxKernelCompileTimings {
      eligibility: stage_timings.eligibility,
      planning: stage_timings.planning,
      program_construction: stage_timings.program_construction,
      validation_lowering: stage_timings.validation_lowering,
      total: total_started.elapsed(),
    },
  ))
}

pub(super) fn compile_calx_artifact_with_imports_inner(
  program: &CompiledProgram,
  namespace: Arc<str>,
  definition: Arc<str>,
  imports: &CalxHostImports,
  mut stage_timings: Option<&mut CalxKernelCompileStageTimings>,
) -> Result<Rc<CalxCompiledArtifact>, CalxKernelCompileError> {
  let eligibility_started = stage_timings.as_ref().map(|_| Instant::now());
  let graph =
    analyze_calx_eligibility_with_imports(program, namespace, definition, imports).map_err(CalxKernelCompileError::Eligibility)?;
  if let (Some(timings), Some(started)) = (stage_timings.as_deref_mut(), eligibility_started) {
    timings.eligibility = started.elapsed();
  }
  let planning_started = stage_timings.as_ref().map(|_| Instant::now());
  let entry_signature = graph
    .functions
    .iter()
    .find(|function| function.definition == graph.entry)
    .ok_or_else(|| CalxLoweringError {
      function: graph.entry.clone(),
      source_path: None,
      message: "eligible graph does not contain its entry function".to_owned(),
    })?;
  let params = entry_signature.params.clone();
  let result = entry_signature.result;
  let names = function_names(&graph);
  let signatures = graph
    .functions
    .iter()
    .map(|function| (function.definition.clone(), (function.params.clone(), function.result)))
    .collect::<BTreeMap<_, _>>();

  let mut plans = Vec::with_capacity(graph.functions.len());
  for function in &graph.functions {
    plans.push(plan_function(
      program,
      function.definition.clone(),
      &function.params,
      function.result,
      &names,
      &signatures,
      imports,
    )?);
  }
  if let (Some(timings), Some(started)) = (stage_timings.as_deref_mut(), planning_started) {
    timings.planning = started.elapsed();
  }

  let construction_started = stage_timings.as_ref().map(|_| Instant::now());
  let mut builder = ProgramBuilder::new();
  let used_imports = graph
    .functions
    .iter()
    .flat_map(|function| function.host_imports.iter().cloned())
    .collect::<BTreeSet<_>>();
  let mut import_ids = BTreeMap::new();
  for target in &used_imports {
    let import = imports.get(target).ok_or_else(|| {
      CalxKernelCompileError::Lowering(lower_error(
        target,
        None,
        "eligible host import has no compile-time capability binding",
      ))
    })?;
    let id = builder.import_at(
      import.name(),
      import.params().iter().copied().map(CalxScalarType::vm_type).collect(),
      import.result().map(CalxScalarType::vm_type),
      Some(SourceSpan::synthetic(source_origin(target, None))),
    )?;
    import_ids.insert(target.clone(), id);
  }
  for plan in &plans {
    builder.function(emit_function(plan, &import_ids)?)?;
  }
  let built_program = builder.build()?;
  if let (Some(timings), Some(started)) = (stage_timings.as_deref_mut(), construction_started) {
    timings.program_construction = started.elapsed();
  }
  let validation_started = stage_timings.as_ref().map(|_| Instant::now());
  let validated_program = ValidatedProgram::try_from_program(built_program)?;
  if let (Some(timings), Some(started)) = (stage_timings, validation_started) {
    timings.validation_lowering = started.elapsed();
  }
  let reachable_stamps = graph
    .functions
    .iter()
    .map(|function| {
      let compiled = lookup_compiled_def(program, &function.definition).ok_or_else(|| {
        CalxKernelCompileError::Lowering(lower_error(
          &function.definition,
          None,
          "eligible definition disappeared before artifact stamping",
        ))
      })?;
      Ok(CalxDefinitionStamp {
        definition: function.definition.clone(),
        def_id: compiled.def_id,
        preprocessed_code: compiled.preprocessed_code.clone(),
        schema: compiled.schema.clone(),
      })
    })
    .collect::<Result<Vec<_>, CalxKernelCompileError>>()?;
  let import_schema_stamps = used_imports
    .iter()
    .map(|definition| {
      let compiled = lookup_compiled_def(program, definition).ok_or_else(|| {
        CalxKernelCompileError::Lowering(lower_error(
          definition,
          None,
          "eligible host import disappeared before artifact stamping",
        ))
      })?;
      Ok(CalxImportSchemaStamp {
        definition: definition.clone(),
        schema: compiled.schema.clone(),
      })
    })
    .collect::<Result<Vec<_>, CalxKernelCompileError>>()?;
  Ok(Rc::new(CalxCompiledArtifact {
    graph,
    params,
    result,
    program: validated_program,
    reachable_stamps,
    import_schema_stamps,
    import_contract: import_contract(imports),
  }))
}

pub(super) fn prepare_calx_artifact(
  artifact: Rc<CalxCompiledArtifact>,
  imports: &CalxHostImports,
) -> Result<CalxPreparedKernel, CalxKernelCompileError> {
  let actual_contract = import_contract(imports);
  if actual_contract != artifact.import_contract {
    return Err(CalxKernelCompileError::Lowering(lower_error(
      &artifact.graph.entry,
      None,
      "typed host import declarations changed while attaching a compiled artifact",
    )));
  }

  let used_imports = artifact
    .graph
    .functions
    .iter()
    .flat_map(|function| function.host_imports.iter())
    .collect::<BTreeSet<_>>();
  let mut host_bindings = CalxHostBindings::new();
  for target in used_imports {
    let import = imports.get(target).ok_or_else(|| {
      CalxKernelCompileError::Lowering(lower_error(target, None, "compiled host import has no current capability binding"))
    })?;
    host_bindings.insert(import.name().into(), import.binding().clone());
  }
  Ok(CalxPreparedKernel { artifact, host_bindings })
}

fn function_names(graph: &CalxEligibleCallGraph) -> BTreeMap<CalxDefinitionRef, String> {
  graph
    .functions
    .iter()
    .map(|function| {
      let name = if function.definition == graph.entry {
        "main".to_owned()
      } else {
        function.definition.qualified()
      };
      (function.definition.clone(), name)
    })
    .collect()
}

#[derive(Debug, Clone)]
struct PlannedLocal {
  name: Arc<str>,
  value_type: CalxScalarType,
  source_path: Option<Vec<u16>>,
}

#[derive(Debug, Clone)]
struct PlannedFunction {
  definition: CalxDefinitionRef,
  vm_name: String,
  params: Vec<(u16, PlannedLocal)>,
  locals: BTreeMap<u16, PlannedLocal>,
  result: Option<CalxScalarType>,
  body: PlannedExpression,
}

#[derive(Debug, Clone)]
struct PlannedExpression {
  result: Option<CalxScalarType>,
  source_path: Option<Vec<u16>>,
  kind: PlannedExpressionKind,
}

#[derive(Debug, Clone)]
enum PlannedExpressionKind {
  Number(f64),
  Bool(bool),
  Unit,
  Local(u16),
  Sequence(Vec<PlannedExpression>),
  Let {
    local: u16,
    value: Box<PlannedExpression>,
    body: Box<PlannedExpression>,
  },
  If {
    condition: Box<PlannedExpression>,
    then_branch: Box<PlannedExpression>,
    else_branch: Box<PlannedExpression>,
  },
  Operation {
    operation: PlannedOperation,
    args: Vec<PlannedExpression>,
  },
  Call {
    function: String,
    args: Vec<PlannedExpression>,
    tail: bool,
  },
  ImportCall {
    target: CalxDefinitionRef,
    args: Vec<PlannedExpression>,
  },
}

#[derive(Debug, Clone, Copy)]
enum PlannedOperation {
  Add,
  Subtract,
  Multiply,
  Divide,
  Negate,
  Equal,
  LessThan,
  GreaterThan,
  F64BufferLen,
  F64ToI64Index,
  F64BufferGet,
}

struct PlanningContext<'a> {
  function: &'a CalxDefinitionRef,
  function_result: Option<CalxScalarType>,
  names: &'a BTreeMap<CalxDefinitionRef, String>,
  signatures: &'a BTreeMap<CalxDefinitionRef, (Vec<CalxScalarType>, Option<CalxScalarType>)>,
  imports: &'a CalxHostImports,
  locals: BTreeMap<u16, PlannedLocal>,
}

fn plan_function(
  program: &CompiledProgram,
  definition: CalxDefinitionRef,
  param_types: &[CalxScalarType],
  result: Option<CalxScalarType>,
  names: &BTreeMap<CalxDefinitionRef, String>,
  signatures: &BTreeMap<CalxDefinitionRef, (Vec<CalxScalarType>, Option<CalxScalarType>)>,
  imports: &CalxHostImports,
) -> Result<PlannedFunction, CalxLoweringError> {
  let compiled =
    lookup_compiled_def(program, &definition).ok_or_else(|| lower_error(&definition, None, "compiled definition disappeared"))?;
  let (args, body) = extract_fn_parts(compiled, &definition, std::slice::from_ref(&definition), &mut vec![]).ok_or_else(|| {
    lower_error(
      &definition,
      source_path(&compiled.preprocessed_code),
      "eligible function shape could not be recovered",
    )
  })?;
  let CalcitFnArgs::Args(arg_ids) = args else {
    return Err(lower_error(
      &definition,
      source_path(&compiled.preprocessed_code),
      "eligible function has marked arguments",
    ));
  };
  if arg_ids.len() != param_types.len() {
    return Err(lower_error(
      &definition,
      source_path(&compiled.preprocessed_code),
      format!("parameter count changed from {} to {}", param_types.len(), arg_ids.len()),
    ));
  }

  let params = arg_ids
    .iter()
    .zip(param_types)
    .map(|(idx, value_type)| {
      (
        *idx,
        PlannedLocal {
          name: Arc::from(CalcitLocal::read_name(*idx)),
          value_type: *value_type,
          source_path: source_path(&compiled.preprocessed_code),
        },
      )
    })
    .collect::<Vec<_>>();
  let mut context = PlanningContext {
    function: &definition,
    function_result: result,
    names,
    signatures,
    imports,
    locals: BTreeMap::new(),
  };
  let body = plan_sequence(&body, true, &mut context)?;
  if body.result != result {
    return Err(lower_error(
      &definition,
      body.source_path.clone(),
      format!("planned result {:?} differs from eligible result {result:?}", body.result),
    ));
  }
  let vm_name = names
    .get(&definition)
    .cloned()
    .ok_or_else(|| lower_error(&definition, None, "eligible function has no Calx name"))?;
  Ok(PlannedFunction {
    definition: definition.clone(),
    vm_name,
    params,
    locals: context.locals,
    result,
    body,
  })
}

fn plan_sequence(
  expressions: &[Calcit],
  tail: bool,
  context: &mut PlanningContext<'_>,
) -> Result<PlannedExpression, CalxLoweringError> {
  let Some(last) = expressions.last() else {
    return Err(lower_error(context.function, None, "eligible sequence is empty"));
  };
  if expressions.len() == 1 {
    return plan_expression(last, tail, context);
  }
  let mut planned = Vec::with_capacity(expressions.len());
  for (index, expression) in expressions.iter().enumerate() {
    planned.push(plan_expression(expression, tail && index + 1 == expressions.len(), context)?);
  }
  let result = planned.last().and_then(|expression| expression.result);
  Ok(PlannedExpression {
    result,
    source_path: expressions.iter().find_map(source_path),
    kind: PlannedExpressionKind::Sequence(planned),
  })
}

fn plan_expression(expression: &Calcit, tail: bool, context: &mut PlanningContext<'_>) -> Result<PlannedExpression, CalxLoweringError> {
  let path = source_path(expression);
  match expression {
    Calcit::Number(value) => Ok(planned(Some(CalxScalarType::F64), path, PlannedExpressionKind::Number(*value))),
    Calcit::Bool(value) => Ok(planned(Some(CalxScalarType::Bool), path, PlannedExpressionKind::Bool(*value))),
    Calcit::Unit => Ok(planned(None, path, PlannedExpressionKind::Unit)),
    Calcit::Local(local) => Ok(planned(
      scalar_local_type(local, context.function)?,
      path,
      PlannedExpressionKind::Local(local.idx),
    )),
    Calcit::List(items) if !items.is_empty() => plan_call(items, tail, context),
    other => Err(lower_error(
      context.function,
      path,
      format!("eligibility admitted unsupported expression `{other}`"),
    )),
  }
}

fn plan_call(
  items: &crate::calcit::CalcitList,
  tail: bool,
  context: &mut PlanningContext<'_>,
) -> Result<PlannedExpression, CalxLoweringError> {
  let operator = &items[0];
  let args = items.drop_left().to_vec();
  match operator {
    Calcit::Syntax(CalcitSyntax::If, _) => plan_if(&args, tail, context),
    Calcit::Syntax(CalcitSyntax::CoreLet, _) => plan_let(&args, tail, context),
    Calcit::Syntax(CalcitSyntax::AssertType, _) => args
      .first()
      .ok_or_else(|| lower_error(context.function, source_path(operator), "eligible assert-type has no value"))
      .and_then(|value| plan_expression(value, tail, context)),
    Calcit::Syntax(CalcitSyntax::HintFn, _) => Ok(planned(None, source_path(operator), PlannedExpressionKind::Unit)),
    Calcit::Proc(proc) => plan_proc(*proc, &args, tail, context),
    Calcit::Import(import) if import.ns.as_ref() == "calcit.core" && import.def.as_ref() == "do" => plan_sequence(&args, tail, context),
    Calcit::Import(import) => plan_direct_call(CalxDefinitionRef::new(import.ns.clone(), import.def.clone()), &args, tail, context),
    Calcit::Fn { info, .. } if info.def_ref.is_some() => {
      let def_ref = info.def_ref.as_ref().ok_or_else(|| {
        lower_error(
          context.function,
          source_path(operator),
          "eligible direct function lost its definition reference",
        )
      })?;
      plan_direct_call(
        CalxDefinitionRef::new(def_ref.def_ns.clone(), def_ref.def_name.clone()),
        &args,
        tail,
        context,
      )
    }
    other => Err(lower_error(
      context.function,
      source_path(other),
      format!("eligibility admitted unsupported operator `{other}`"),
    )),
  }
}

fn plan_if(args: &[Calcit], tail: bool, context: &mut PlanningContext<'_>) -> Result<PlannedExpression, CalxLoweringError> {
  if args.len() != 3 {
    return Err(lower_error(
      context.function,
      args.first().and_then(source_path),
      "eligible if is not ternary",
    ));
  }
  let condition = plan_expression(&args[0], false, context)?;
  let then_branch = plan_expression(&args[1], tail, context)?;
  let else_branch = plan_expression(&args[2], tail, context)?;
  if condition.result != Some(CalxScalarType::Bool) || then_branch.result != else_branch.result {
    return Err(lower_error(
      context.function,
      source_path(&args[0]),
      "eligible if lost its Bool condition or equal branch types",
    ));
  }
  let result = then_branch.result;
  Ok(planned(
    result,
    source_path(&args[0]),
    PlannedExpressionKind::If {
      condition: Box::new(condition),
      then_branch: Box::new(then_branch),
      else_branch: Box::new(else_branch),
    },
  ))
}

fn plan_let(args: &[Calcit], tail: bool, context: &mut PlanningContext<'_>) -> Result<PlannedExpression, CalxLoweringError> {
  let Some((binding, body)) = args.split_first() else {
    return Err(lower_error(context.function, None, "eligible &let has no binding"));
  };
  match binding {
    Calcit::Nil | Calcit::Unit => plan_sequence(body, tail, context),
    Calcit::List(pair) if pair.is_empty() => plan_sequence(body, tail, context),
    Calcit::List(pair) if pair.len() == 2 => {
      let Calcit::Local(local) = &pair[0] else {
        return Err(lower_error(
          context.function,
          source_path(&pair[0]),
          "eligible &let name is not a local",
        ));
      };
      let value_type = scalar_local_type(local, context.function)?;
      let value = plan_expression(&pair[1], false, context)?;
      if value.result != value_type {
        return Err(lower_error(
          context.function,
          source_path(&pair[1]),
          "eligible &let value changed type before lowering",
        ));
      }
      let local_plan = PlannedLocal {
        name: local.sym.clone(),
        value_type: value_type.ok_or_else(|| lower_error(context.function, source_path(&pair[0]), "&let local cannot be void"))?,
        source_path: local.location.as_ref().map(|path| path.as_ref().clone()),
      };
      if let Some(previous) = context.locals.insert(local.idx, local_plan.clone())
        && (previous.name != local_plan.name || previous.value_type != local_plan.value_type)
      {
        return Err(lower_error(
          context.function,
          source_path(&pair[0]),
          format!("local index {} was reused with an incompatible declaration", local.idx),
        ));
      }
      let body = plan_sequence(body, tail, context)?;
      let result = body.result;
      Ok(planned(
        result,
        source_path(binding),
        PlannedExpressionKind::Let {
          local: local.idx,
          value: Box::new(value),
          body: Box::new(body),
        },
      ))
    }
    other => Err(lower_error(
      context.function,
      source_path(other),
      "eligible &let binding has an unsupported shape",
    )),
  }
}

fn plan_proc(
  proc: CalcitProc,
  args: &[Calcit],
  tail: bool,
  context: &mut PlanningContext<'_>,
) -> Result<PlannedExpression, CalxLoweringError> {
  let (operation, result) = match proc {
    CalcitProc::NativeAdd => (Some(PlannedOperation::Add), Some(CalxScalarType::F64)),
    CalcitProc::NativeMinus if args.len() == 1 => (Some(PlannedOperation::Negate), Some(CalxScalarType::F64)),
    CalcitProc::NativeMinus => (Some(PlannedOperation::Subtract), Some(CalxScalarType::F64)),
    CalcitProc::NativeMultiply => (Some(PlannedOperation::Multiply), Some(CalxScalarType::F64)),
    CalcitProc::NativeDivide => (Some(PlannedOperation::Divide), Some(CalxScalarType::F64)),
    CalcitProc::NativeEquals => (Some(PlannedOperation::Equal), Some(CalxScalarType::Bool)),
    CalcitProc::NativeLessThan => (Some(PlannedOperation::LessThan), Some(CalxScalarType::Bool)),
    CalcitProc::NativeGreaterThan => (Some(PlannedOperation::GreaterThan), Some(CalxScalarType::Bool)),
    CalcitProc::NativeF64BufferLen => (Some(PlannedOperation::F64BufferLen), Some(CalxScalarType::F64)),
    CalcitProc::NativeF64ToI64Index => (Some(PlannedOperation::F64ToI64Index), Some(CalxScalarType::F64)),
    CalcitProc::NativeF64BufferGet => (Some(PlannedOperation::F64BufferGet), Some(CalxScalarType::F64)),
    CalcitProc::Recur => {
      let name = context
        .names
        .get(context.function)
        .cloned()
        .ok_or_else(|| lower_error(context.function, None, "recursive function has no Calx name"))?;
      let args = plan_args(args, context)?;
      return Ok(planned(
        context.function_result,
        args.first().and_then(|value| value.source_path.clone()),
        PlannedExpressionKind::Call {
          function: name,
          args,
          tail,
        },
      ));
    }
    _ => {
      return Err(lower_error(
        context.function,
        args.first().and_then(source_path),
        format!("eligibility admitted unsupported proc `{proc}`"),
      ));
    }
  };
  let args = plan_args(args, context)?;
  Ok(planned(
    result,
    args.first().and_then(|value| value.source_path.clone()),
    PlannedExpressionKind::Operation {
      operation: operation.ok_or_else(|| lower_error(context.function, None, "operation plan is missing"))?,
      args,
    },
  ))
}

fn plan_direct_call(
  target: CalxDefinitionRef,
  args: &[Calcit],
  tail: bool,
  context: &mut PlanningContext<'_>,
) -> Result<PlannedExpression, CalxLoweringError> {
  if let Some(import) = context.imports.get(&target) {
    let result = import.result();
    let args = plan_args(args, context)?;
    return Ok(planned(
      result,
      args.first().and_then(|value| value.source_path.clone()),
      PlannedExpressionKind::ImportCall { target, args },
    ));
  }
  let (_, result) = context.signatures.get(&target).ok_or_else(|| {
    lower_error(
      context.function,
      args.first().and_then(source_path),
      format!("callee `{}` is outside the graph", target.qualified()),
    )
  })?;
  let function = context.names.get(&target).cloned().ok_or_else(|| {
    lower_error(
      context.function,
      args.first().and_then(source_path),
      format!("callee `{}` has no Calx name", target.qualified()),
    )
  })?;
  let args = plan_args(args, context)?;
  Ok(planned(
    *result,
    args.first().and_then(|value| value.source_path.clone()),
    PlannedExpressionKind::Call { function, args, tail },
  ))
}

fn plan_args(args: &[Calcit], context: &mut PlanningContext<'_>) -> Result<Vec<PlannedExpression>, CalxLoweringError> {
  args.iter().map(|argument| plan_expression(argument, false, context)).collect()
}

fn scalar_local_type(local: &CalcitLocal, function: &CalxDefinitionRef) -> Result<Option<CalxScalarType>, CalxLoweringError> {
  match local.type_info.as_ref() {
    crate::calcit::CalcitTypeAnnotation::Number => Ok(Some(CalxScalarType::F64)),
    crate::calcit::CalcitTypeAnnotation::Bool => Ok(Some(CalxScalarType::Bool)),
    crate::calcit::CalcitTypeAnnotation::F64Buffer => Ok(Some(CalxScalarType::F64Buffer)),
    other => Err(lower_error(
      function,
      local.location.as_ref().map(|path| path.as_ref().clone()),
      format!("eligible local `{}` has non-scalar type `{other}`", local.sym),
    )),
  }
}

fn planned(result: Option<CalxScalarType>, source_path: Option<Vec<u16>>, kind: PlannedExpressionKind) -> PlannedExpression {
  PlannedExpression { result, source_path, kind }
}

fn lower_error(function: &CalxDefinitionRef, source_path: Option<Vec<u16>>, message: impl Into<String>) -> CalxLoweringError {
  CalxLoweringError {
    function: function.clone(),
    source_path,
    message: message.into(),
  }
}

fn emit_function(
  plan: &PlannedFunction,
  imports: &BTreeMap<CalxDefinitionRef, ImportId>,
) -> Result<FunctionBuilder, CalxKernelCompileError> {
  let source = source_origin(&plan.definition, None);
  let results = plan.result.into_iter().map(CalxScalarType::vm_type).collect();
  let mut builder = FunctionBuilder::synthetic(plan.vm_name.clone(), results, source)?;
  let mut local_ids = BTreeMap::new();
  for (idx, local) in &plan.params {
    let id = builder.parameter(local.name.as_ref(), local.value_type.vm_type())?;
    local_ids.insert(*idx, id);
  }
  for (idx, local) in &plan.locals {
    if local_ids.contains_key(idx) {
      continue;
    }
    let span = Some(SourceSpan::synthetic(source_origin(&plan.definition, local.source_path.as_deref())));
    let id = builder.local_at(local.name.as_ref(), local.value_type.vm_type(), span)?;
    local_ids.insert(*idx, id);
  }
  let context = EmissionContext {
    function: &plan.definition,
    locals: &local_ids,
    imports,
    lowering_error: RefCell::new(None),
  };
  emit_expression(&plan.body, builder.body(), &context)?;
  if let Some(error) = context.lowering_error.into_inner() {
    return Err(CalxKernelCompileError::Lowering(error));
  }
  Ok(builder)
}

struct EmissionContext<'a> {
  function: &'a CalxDefinitionRef,
  locals: &'a BTreeMap<u16, LocalId>,
  imports: &'a BTreeMap<CalxDefinitionRef, ImportId>,
  lowering_error: RefCell<Option<CalxLoweringError>>,
}

impl EmissionContext<'_> {
  fn record_missing_local(&self, idx: u16, source_path: Option<Vec<u16>>) {
    let mut error = self.lowering_error.borrow_mut();
    if error.is_none() {
      *error = Some(lower_error(
        self.function,
        source_path,
        format!("typed local index {idx} has no emitted Calx LocalId"),
      ));
    }
  }
}

fn emit_expression(
  expression: &PlannedExpression,
  body: &mut BodyBuilder,
  context: &EmissionContext<'_>,
) -> Result<(), CalxBuildError> {
  body.set_default_span(Some(SourceSpan::synthetic(source_origin(
    context.function,
    expression.source_path.as_deref(),
  ))));
  match &expression.kind {
    PlannedExpressionKind::Number(value) => {
      body.constant(VmValue::F64(*value))?;
    }
    PlannedExpressionKind::Bool(value) => {
      body.constant(VmValue::Bool(*value))?;
    }
    PlannedExpressionKind::Unit => {}
    PlannedExpressionKind::Local(idx) => {
      if let Some(local) = context.locals.get(idx) {
        body.local_get(local)?;
      } else {
        context.record_missing_local(*idx, expression.source_path.clone());
      }
    }
    PlannedExpressionKind::Sequence(expressions) => emit_sequence(expressions, body, context)?,
    PlannedExpressionKind::Let {
      local,
      value,
      body: let_body,
    } => {
      emit_expression(value, body, context)?;
      if let Some(local_id) = context.locals.get(local) {
        body.local_set(local_id)?;
      } else {
        context.record_missing_local(*local, expression.source_path.clone());
      }
      emit_expression(let_body, body, context)?;
    }
    PlannedExpressionKind::If {
      condition,
      then_branch,
      else_branch,
    } => {
      emit_expression(condition, body, context)?;
      let results = expression.result.into_iter().map(CalxScalarType::vm_type).collect();
      body.if_else(
        results,
        |then_body| emit_expression(then_branch, then_body, context),
        |else_body| emit_expression(else_branch, else_body, context),
      )?;
    }
    PlannedExpressionKind::Operation { operation, args } => {
      for argument in args {
        emit_expression(argument, body, context)?;
      }
      match operation {
        PlannedOperation::Add => body.emit(VmSyntax::Add)?,
        PlannedOperation::Subtract => body.emit(VmSyntax::Neg)?.emit(VmSyntax::Add)?,
        PlannedOperation::Multiply => body.emit(VmSyntax::Mul)?,
        PlannedOperation::Divide => body.emit(VmSyntax::Div)?,
        PlannedOperation::Negate => body.emit(VmSyntax::Neg)?,
        PlannedOperation::Equal => body.emit(VmSyntax::F64Eq)?,
        PlannedOperation::LessThan => body.emit(VmSyntax::F64Lt)?,
        PlannedOperation::GreaterThan => body.emit(VmSyntax::F64Gt)?,
        PlannedOperation::F64BufferLen => body.f64_buffer_len()?,
        PlannedOperation::F64ToI64Index => body.f64_to_i64_index()?,
        PlannedOperation::F64BufferGet => body.f64_buffer_get()?,
      };
    }
    PlannedExpressionKind::Call {
      function: callee,
      args,
      tail,
    } => {
      for argument in args {
        emit_expression(argument, body, context)?;
      }
      if *tail {
        body.return_call(callee.as_str())?;
      } else {
        body.call(callee.as_str())?;
      }
    }
    PlannedExpressionKind::ImportCall { target, args } => {
      for argument in args {
        emit_expression(argument, body, context)?;
      }
      if let Some(import) = context.imports.get(target) {
        body.call_import(import)?;
      } else {
        let mut error = context.lowering_error.borrow_mut();
        if error.is_none() {
          *error = Some(lower_error(
            context.function,
            expression.source_path.clone(),
            format!("host import `{}` has no emitted Calx ImportId", target.qualified()),
          ));
        }
      }
    }
  }
  Ok(())
}

fn emit_sequence(
  expressions: &[PlannedExpression],
  body: &mut BodyBuilder,
  context: &EmissionContext<'_>,
) -> Result<(), CalxBuildError> {
  for (index, expression) in expressions.iter().enumerate() {
    emit_expression(expression, body, context)?;
    if index + 1 != expressions.len() && expression.result.is_some() {
      body.emit(VmSyntax::Drop)?;
    }
  }
  Ok(())
}

fn source_origin(function: &CalxDefinitionRef, path: Option<&[u16]>) -> String {
  let path = path
    .map(|parts| parts.iter().map(u16::to_string).collect::<Vec<_>>().join("."))
    .unwrap_or_else(|| "-".to_owned());
  format!("calcit:{}@{path}", function.qualified())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn missing_planned_local_returns_lowering_error() {
    let definition = CalxDefinitionRef::new("tests.calx", "missing-local");
    let plan = PlannedFunction {
      definition: definition.clone(),
      vm_name: "main".to_owned(),
      params: vec![],
      locals: BTreeMap::new(),
      result: Some(CalxScalarType::F64),
      body: planned(Some(CalxScalarType::F64), Some(vec![3, 1]), PlannedExpressionKind::Local(42)),
    };

    let error = emit_function(&plan, &BTreeMap::new()).expect_err("an unplanned typed local must not panic");
    let CalxKernelCompileError::Lowering(error) = error else {
      panic!("expected a lowering error, got {error}");
    };
    assert_eq!(error.function, definition);
    assert_eq!(error.source_path, Some(vec![3, 1]));
    assert!(error.message.contains("local index 42"));
  }
}
