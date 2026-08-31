//! Eligibility analysis and lowering for the experimental Calcit-to-Calx scalar subset.
//!
//! This module consumes an immutable [`CompiledProgram`](crate::program::CompiledProgram)
//! snapshot. It never emits a partial program: callers receive either a closed eligible
//! call graph, or one stable fallback report. Eligible graphs can then be lowered through
//! `calx_vm::ProgramBuilder` and strict validation without admitting Nil or Dynamic values.

pub mod benchmark_session;
mod cache;
mod lowering;

pub use cache::{CalxCacheMissReason, CalxCachePreparation, CalxCachePrepareReport, CalxCompileCache, CalxCompileCacheStats};
pub use calx_vm::{Calx as CalxValue, CalxBuildError, CalxError, CalxProgramError};
pub use lowering::{
  CalxCompiledArtifact, CalxCompiledKernel, CalxKernelBoundaryError, CalxKernelBoundaryErrorKind, CalxKernelCompileError,
  CalxKernelCompileTimings, CalxKernelRunError, CalxLoweringError, CalxPreparedKernel, compile_calx_kernel,
  compile_calx_kernel_measured, compile_calx_kernel_with_imports, compile_calx_kernel_with_imports_measured,
};

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use calx_vm::{CalxHostBinding, CalxType as VmType};

use crate::builtins::syntax::get_raw_args_fn;
use crate::calcit::{Calcit, CalcitFnArgs, CalcitProc, CalcitSyntax, CalcitTypeAnnotation};
use crate::program::{CompiledDef, CompiledDefKind, CompiledProgram};

pub const CALX_KERNEL_ABI_EDITION: &str = "calcit-calx-kernel/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CalxScalarType {
  F64,
  Bool,
}

impl CalxScalarType {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::F64 => "F64",
      Self::Bool => "Bool",
    }
  }

  const fn vm_type(self) -> VmType {
    match self {
      Self::F64 => VmType::F64,
      Self::Bool => VmType::Bool,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalxDefinitionRef {
  pub namespace: Arc<str>,
  pub definition: Arc<str>,
}

impl CalxDefinitionRef {
  pub fn new(namespace: impl Into<Arc<str>>, definition: impl Into<Arc<str>>) -> Self {
    Self {
      namespace: namespace.into(),
      definition: definition.into(),
    }
  }

  pub fn qualified(&self) -> String {
    format!("{}/{}", self.namespace, self.definition)
  }
}

/// One explicitly approved scalar host capability for a Calx kernel.
///
/// The Calcit definition is supplied as the key in [`CalxHostImports`]. Its
/// typed snapshot signature must exactly match this declaration before the
/// function body may be replaced by the host callback.
#[derive(Debug, Clone)]
pub struct CalxHostImport {
  name: Arc<str>,
  params: Vec<CalxScalarType>,
  result: Option<CalxScalarType>,
  binding: CalxHostBinding,
}

/// Callback-free typed declaration used in revision-safe Calx artifact keys.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalxImportContract {
  pub definition: CalxDefinitionRef,
  pub export_name: Arc<str>,
  pub params: Vec<CalxScalarType>,
  pub result: Option<CalxScalarType>,
}

impl CalxHostImport {
  /// Declares a zero-result import backed by `Result<(), CalxError>`.
  pub fn void(
    name: impl Into<Arc<str>>,
    params: Vec<CalxScalarType>,
    callback: fn(&[CalxValue]) -> Result<(), CalxError>,
  ) -> Result<Self, CalxProgramError> {
    let binding = CalxHostBinding::void(params.iter().copied().map(CalxScalarType::vm_type).collect(), callback)?;
    Ok(Self {
      name: name.into(),
      params,
      result: None,
      binding,
    })
  }

  /// Declares a single-result import backed by `Result<CalxValue, CalxError>`.
  pub fn value(
    name: impl Into<Arc<str>>,
    params: Vec<CalxScalarType>,
    result: CalxScalarType,
    callback: fn(&[CalxValue]) -> Result<CalxValue, CalxError>,
  ) -> Result<Self, CalxProgramError> {
    let binding = CalxHostBinding::value(
      params.iter().copied().map(CalxScalarType::vm_type).collect(),
      result.vm_type(),
      callback,
    )?;
    Ok(Self {
      name: name.into(),
      params,
      result: Some(result),
      binding,
    })
  }

  /// Calx import declaration name used in the generated program.
  pub fn name(&self) -> &str {
    &self.name
  }

  /// Concrete non-nil, non-Dynamic scalar parameter contract.
  pub fn params(&self) -> &[CalxScalarType] {
    &self.params
  }

  /// Zero-result imports return `None`; single-result imports return one scalar type.
  pub fn result(&self) -> Option<CalxScalarType> {
    self.result
  }

  fn contract(&self, definition: CalxDefinitionRef) -> CalxImportContract {
    CalxImportContract {
      definition,
      export_name: self.name.clone(),
      params: self.params.clone(),
      result: self.result,
    }
  }

  pub(super) fn binding(&self) -> &CalxHostBinding {
    &self.binding
  }
}

/// Explicit Calcit-definition to Calx-host-capability mapping.
pub type CalxHostImports = BTreeMap<CalxDefinitionRef, CalxHostImport>;

fn import_contract(imports: &CalxHostImports) -> Vec<CalxImportContract> {
  imports
    .iter()
    .map(|(definition, import)| import.contract(definition.clone()))
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CalxFallbackCode {
  DynamicType,
  NilValue,
  UnsupportedType,
  UnsupportedForm,
  IndirectCall,
  Arity,
  NonBoolCondition,
  RecurNotTail,
  HostCapability,
  CallClosure,
  AbiEdition,
}

impl CalxFallbackCode {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::DynamicType => "CALX_SUBSET_DYNAMIC_TYPE",
      Self::NilValue => "CALX_SUBSET_NIL_VALUE",
      Self::UnsupportedType => "CALX_SUBSET_UNSUPPORTED_TYPE",
      Self::UnsupportedForm => "CALX_SUBSET_UNSUPPORTED_FORM",
      Self::IndirectCall => "CALX_SUBSET_INDIRECT_CALL",
      Self::Arity => "CALX_SUBSET_ARITY",
      Self::NonBoolCondition => "CALX_SUBSET_NON_BOOL_CONDITION",
      Self::RecurNotTail => "CALX_SUBSET_RECUR_NOT_TAIL",
      Self::HostCapability => "CALX_SUBSET_HOST_CAPABILITY",
      Self::CallClosure => "CALX_SUBSET_CALL_CLOSURE",
      Self::AbiEdition => "CALX_SUBSET_ABI_EDITION",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalxFallbackIssue {
  pub code: CalxFallbackCode,
  pub namespace: Arc<str>,
  pub definition: Arc<str>,
  pub source_path: Option<Vec<u16>>,
  pub call_path: Vec<CalxDefinitionRef>,
  pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxFallbackReport {
  pub abi_edition: Arc<str>,
  pub entry: CalxDefinitionRef,
  pub issues: Vec<CalxFallbackIssue>,
}

impl CalxFallbackReport {
  /// Deterministic text used by source-backed golden fixtures. This is an
  /// experimental report format, not a serialized compiler ABI.
  pub fn stable_summary(&self) -> String {
    let mut output = format!("abi {}\nentry {}\n", self.abi_edition, self.entry.qualified());
    for issue in &self.issues {
      let path = issue
        .source_path
        .as_ref()
        .map(|parts| parts.iter().map(u16::to_string).collect::<Vec<_>>().join("."))
        .unwrap_or_else(|| "-".to_owned());
      let call_path = issue
        .call_path
        .iter()
        .map(CalxDefinitionRef::qualified)
        .collect::<Vec<_>>()
        .join(" -> ");
      output.push_str(&format!(
        "issue {} {}/{} source={} call={} message={}\n",
        issue.code.as_str(),
        issue.namespace,
        issue.definition,
        path,
        call_path,
        issue.message
      ));
    }
    output
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxEligibleFunction {
  pub definition: CalxDefinitionRef,
  pub params: Vec<CalxScalarType>,
  pub result: Option<CalxScalarType>,
  pub direct_calls: Vec<CalxDefinitionRef>,
  pub host_imports: Vec<CalxDefinitionRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxEligibleCallGraph {
  pub abi_edition: Arc<str>,
  pub entry: CalxDefinitionRef,
  pub functions: Vec<CalxEligibleFunction>,
}

impl CalxEligibleCallGraph {
  /// Deterministic text used by source-backed golden fixtures. This is an
  /// experimental report format, not a serialized compiler ABI.
  pub fn stable_summary(&self) -> String {
    let mut output = format!("abi {}\nentry {}\n", self.abi_edition, self.entry.qualified());
    for function in &self.functions {
      let params = function.params.iter().map(|value| value.as_str()).collect::<Vec<_>>().join(",");
      let result = function.result.map(CalxScalarType::as_str).unwrap_or("Void");
      output.push_str(&format!("function {} ({params})->{result}\n", function.definition.qualified()));
      for callee in &function.direct_calls {
        output.push_str(&format!("  call {}\n", callee.qualified()));
      }
      for import in &function.host_imports {
        output.push_str(&format!("  import {}\n", import.qualified()));
      }
    }
    output
  }
}

/// Proves that one explicit entry and its reachable direct-call closure belong
/// to the first Calx scalar subset.
///
/// Any issue returns a sorted [`CalxFallbackReport`]. The eligible graph is
/// published only when every reachable definition succeeds.
pub fn analyze_calx_eligibility(
  program: &CompiledProgram,
  namespace: impl Into<Arc<str>>,
  definition: impl Into<Arc<str>>,
) -> Result<CalxEligibleCallGraph, CalxFallbackReport> {
  analyze_calx_eligibility_with_imports(program, namespace, definition, &CalxHostImports::new())
}

/// Proves eligibility while replacing only explicitly declared, signature-
/// matched Calcit definitions with strict typed host imports.
pub fn analyze_calx_eligibility_with_imports(
  program: &CompiledProgram,
  namespace: impl Into<Arc<str>>,
  definition: impl Into<Arc<str>>,
  imports: &CalxHostImports,
) -> Result<CalxEligibleCallGraph, CalxFallbackReport> {
  let entry = CalxDefinitionRef::new(namespace, definition);
  let mut analyzer = EligibilityAnalyzer::new(program, entry.clone(), imports);
  analyzer.visit(entry.clone(), vec![entry.clone()]);
  analyzer.finish()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionSignature {
  params: Vec<CalxScalarType>,
  result: Option<CalxScalarType>,
}

struct EligibilityAnalyzer<'a> {
  program: &'a CompiledProgram,
  imports: &'a CalxHostImports,
  entry: CalxDefinitionRef,
  visited: BTreeSet<CalxDefinitionRef>,
  functions: BTreeMap<CalxDefinitionRef, CalxEligibleFunction>,
  issues: Vec<CalxFallbackIssue>,
}

impl<'a> EligibilityAnalyzer<'a> {
  fn new(program: &'a CompiledProgram, entry: CalxDefinitionRef, imports: &'a CalxHostImports) -> Self {
    Self {
      program,
      imports,
      entry,
      visited: BTreeSet::new(),
      functions: BTreeMap::new(),
      issues: vec![],
    }
  }

  fn visit(&mut self, target: CalxDefinitionRef, call_path: Vec<CalxDefinitionRef>) {
    if !self.visited.insert(target.clone()) {
      return;
    }
    let Some(compiled) = lookup_compiled_def(self.program, &target) else {
      self.push_issue(
        CalxFallbackCode::UnsupportedForm,
        &target,
        None,
        &call_path,
        format!(
          "reachable definition `{}` is missing from the compiled snapshot",
          target.qualified()
        ),
      );
      return;
    };
    if compiled.kind != CompiledDefKind::Fn {
      self.push_issue(
        CalxFallbackCode::IndirectCall,
        &target,
        source_path(&compiled.preprocessed_code),
        &call_path,
        format!("reachable definition `{}` is not a top-level function", target.qualified()),
      );
      return;
    }

    let signature = analyze_signature(compiled, &target, &call_path, &mut self.issues);
    let parts = extract_fn_parts(compiled, &target, &call_path, &mut self.issues);
    let mut direct_calls = BTreeSet::new();
    let mut host_imports = BTreeSet::new();
    let mut body_result = None;
    if let Some((args, body)) = parts {
      if let Some(signature) = &signature
        && args.param_len() != signature.params.len()
      {
        self.push_issue(
          CalxFallbackCode::Arity,
          &target,
          source_path(&compiled.preprocessed_code),
          &call_path,
          format!(
            "function `{}` has {} preprocessed parameters but {} typed parameters",
            target.qualified(),
            args.param_len(),
            signature.params.len()
          ),
        );
      }
      if matches!(args, CalcitFnArgs::MarkedArgs(_)) {
        self.push_issue(
          CalxFallbackCode::Arity,
          &target,
          source_path(&compiled.preprocessed_code),
          &call_path,
          format!("function `{}` uses optional/rest argument markers", target.qualified()),
        );
      }
      let mut context = ExpressionContext {
        program: self.program,
        function: &target,
        signature: signature.as_ref(),
        call_path: &call_path,
        direct_calls: &mut direct_calls,
        host_imports: &mut host_imports,
        imports: self.imports,
        issues: &mut self.issues,
      };
      body_result = analyze_sequence(&body, true, &mut context);
    }
    if let (Some(signature), Some(actual_result)) = (&signature, body_result)
      && actual_result != signature.result
    {
      self.push_issue(
        CalxFallbackCode::UnsupportedType,
        &target,
        source_path(&compiled.preprocessed_code),
        &call_path,
        format!(
          "function `{}` body result {actual_result:?} does not match declared result {:?}",
          target.qualified(),
          signature.result
        ),
      );
    }

    if let Some(signature) = signature {
      self.functions.insert(
        target.clone(),
        CalxEligibleFunction {
          definition: target.clone(),
          params: signature.params,
          result: signature.result,
          direct_calls: direct_calls.iter().cloned().collect(),
          host_imports: host_imports.iter().cloned().collect(),
        },
      );
    }

    for callee in direct_calls {
      let mut callee_path = call_path.clone();
      callee_path.push(callee.clone());
      self.visit(callee, callee_path);
    }
  }

  fn push_issue(
    &mut self,
    code: CalxFallbackCode,
    target: &CalxDefinitionRef,
    source_path: Option<Vec<u16>>,
    call_path: &[CalxDefinitionRef],
    message: String,
  ) {
    self.issues.push(CalxFallbackIssue {
      code,
      namespace: target.namespace.clone(),
      definition: target.definition.clone(),
      source_path,
      call_path: call_path.to_vec(),
      message,
    });
  }

  fn finish(mut self) -> Result<CalxEligibleCallGraph, CalxFallbackReport> {
    if self.issues.iter().any(|issue| {
      issue.namespace.as_ref() != self.entry.namespace.as_ref() || issue.definition.as_ref() != self.entry.definition.as_ref()
    }) {
      self.issues.push(CalxFallbackIssue {
        code: CalxFallbackCode::CallClosure,
        namespace: self.entry.namespace.clone(),
        definition: self.entry.definition.clone(),
        source_path: None,
        call_path: vec![self.entry.clone()],
        message: format!("entry `{}` has an ineligible reachable call closure", self.entry.qualified()),
      });
    }
    self.issues.sort();
    self.issues.dedup();
    if self.issues.is_empty() {
      Ok(CalxEligibleCallGraph {
        abi_edition: Arc::from(CALX_KERNEL_ABI_EDITION),
        entry: self.entry,
        functions: self.functions.into_values().collect(),
      })
    } else {
      Err(CalxFallbackReport {
        abi_edition: Arc::from(CALX_KERNEL_ABI_EDITION),
        entry: self.entry,
        issues: self.issues,
      })
    }
  }
}

fn lookup_compiled_def<'a>(program: &'a CompiledProgram, target: &CalxDefinitionRef) -> Option<&'a CompiledDef> {
  program.get(target.namespace.as_ref())?.get(target.definition.as_ref())
}

fn analyze_signature(
  compiled: &CompiledDef,
  target: &CalxDefinitionRef,
  call_path: &[CalxDefinitionRef],
  issues: &mut Vec<CalxFallbackIssue>,
) -> Option<FunctionSignature> {
  let CalcitTypeAnnotation::Fn(signature) = compiled.schema.as_ref() else {
    let code = if matches!(compiled.schema.as_ref(), CalcitTypeAnnotation::Dynamic) {
      CalxFallbackCode::DynamicType
    } else {
      CalxFallbackCode::UnsupportedType
    };
    push_expression_issue(
      issues,
      code,
      target,
      source_path(&compiled.preprocessed_code),
      call_path,
      format!("function `{}` has non-function schema `{}`", target.qualified(), compiled.schema),
    );
    return None;
  };
  if !signature.generics.is_empty() || !signature.where_bounds.is_empty() {
    push_expression_issue(
      issues,
      CalxFallbackCode::UnsupportedType,
      target,
      source_path(&compiled.preprocessed_code),
      call_path,
      format!("function `{}` has unresolved generics or trait bounds", target.qualified()),
    );
  }
  if signature.rest_type.is_some() {
    push_expression_issue(
      issues,
      CalxFallbackCode::Arity,
      target,
      source_path(&compiled.preprocessed_code),
      call_path,
      format!("function `{}` declares a rest parameter", target.qualified()),
    );
  }

  let mut params = vec![];
  for (index, value_type) in signature.arg_types.iter().enumerate() {
    if let Some(value_type) = map_slot_type(
      value_type,
      &format!("parameter {index}"),
      target,
      source_path(&compiled.preprocessed_code),
      call_path,
      issues,
    ) {
      params.push(value_type);
    }
  }
  let result = map_result_type(
    &signature.return_type,
    "function result",
    target,
    source_path(&compiled.preprocessed_code),
    call_path,
    issues,
  );
  if params.len() == signature.arg_types.len() && result.is_some() {
    Some(FunctionSignature {
      params,
      result: result.flatten(),
    })
  } else {
    None
  }
}

fn signature_without_issues(compiled: &CompiledDef) -> Option<FunctionSignature> {
  let CalcitTypeAnnotation::Fn(signature) = compiled.schema.as_ref() else {
    return None;
  };
  if !signature.generics.is_empty() || !signature.where_bounds.is_empty() || signature.rest_type.is_some() {
    return None;
  }
  let params: Option<Vec<_>> = signature.arg_types.iter().map(|value| map_slot_type_quiet(value)).collect();
  Some(FunctionSignature {
    params: params?,
    result: map_result_type_quiet(&signature.return_type)?,
  })
}

fn extract_fn_parts(
  compiled: &CompiledDef,
  target: &CalxDefinitionRef,
  call_path: &[CalxDefinitionRef],
  issues: &mut Vec<CalxFallbackIssue>,
) -> Option<(CalcitFnArgs, Vec<Calcit>)> {
  let Calcit::List(items) = &compiled.preprocessed_code else {
    push_expression_issue(
      issues,
      CalxFallbackCode::UnsupportedForm,
      target,
      source_path(&compiled.preprocessed_code),
      call_path,
      format!("function `{}` is not a preprocessed defn form", target.qualified()),
    );
    return None;
  };
  let Some(Calcit::Syntax(CalcitSyntax::Defn, _)) = items.first() else {
    push_expression_issue(
      issues,
      CalxFallbackCode::UnsupportedForm,
      target,
      source_path(&compiled.preprocessed_code),
      call_path,
      format!("function `{}` is not a standard defn", target.qualified()),
    );
    return None;
  };
  let Some(Calcit::List(args)) = items.get(2) else {
    push_expression_issue(
      issues,
      CalxFallbackCode::Arity,
      target,
      source_path(&compiled.preprocessed_code),
      call_path,
      format!("function `{}` has no parameter list", target.qualified()),
    );
    return None;
  };
  let args = match get_raw_args_fn(args) {
    Ok(args) => args,
    Err(message) => {
      push_expression_issue(
        issues,
        CalxFallbackCode::Arity,
        target,
        source_path(&compiled.preprocessed_code),
        call_path,
        format!("function `{}` has invalid parameters: {message}", target.qualified()),
      );
      return None;
    }
  };
  let body = items
    .iter()
    .skip(3)
    .filter(|item| CalcitTypeAnnotation::extract_fn_annotation_from_hint_form(item).is_none())
    .cloned()
    .collect();
  Some((args, body))
}

struct ExpressionContext<'a> {
  program: &'a CompiledProgram,
  function: &'a CalxDefinitionRef,
  signature: Option<&'a FunctionSignature>,
  call_path: &'a [CalxDefinitionRef],
  direct_calls: &'a mut BTreeSet<CalxDefinitionRef>,
  host_imports: &'a mut BTreeSet<CalxDefinitionRef>,
  imports: &'a CalxHostImports,
  issues: &'a mut Vec<CalxFallbackIssue>,
}

fn analyze_sequence(expressions: &[Calcit], tail: bool, context: &mut ExpressionContext<'_>) -> Option<Option<CalxScalarType>> {
  if expressions.is_empty() {
    issue(
      context,
      CalxFallbackCode::NilValue,
      None,
      "an empty function/branch body would produce Nil".to_owned(),
    );
    return None;
  }
  let mut result = None;
  for (index, expression) in expressions.iter().enumerate() {
    result = analyze_expression(expression, tail && index + 1 == expressions.len(), context);
  }
  result
}

fn analyze_expression(expression: &Calcit, tail: bool, context: &mut ExpressionContext<'_>) -> Option<Option<CalxScalarType>> {
  match expression {
    Calcit::Number(_) => Some(Some(CalxScalarType::F64)),
    Calcit::Bool(_) => Some(Some(CalxScalarType::Bool)),
    Calcit::Unit => Some(None),
    Calcit::Nil => {
      issue(
        context,
        CalxFallbackCode::NilValue,
        source_path(expression),
        "Nil is not part of the Calx scalar subset".to_owned(),
      );
      None
    }
    Calcit::Local(local) => map_slot_type(
      &local.type_info,
      &format!("local `{}`", local.sym),
      context.function,
      local.location.as_ref().map(|path| path.as_ref().clone()),
      context.call_path,
      context.issues,
    )
    .map(Some),
    Calcit::List(items) if !items.is_empty() => analyze_call(items, tail, context),
    Calcit::Import(_) | Calcit::Fn { .. } | Calcit::Proc(_) | Calcit::Method(_, _) | Calcit::Symbol { .. } => {
      issue(
        context,
        CalxFallbackCode::IndirectCall,
        source_path(expression),
        format!("function/operator value `{expression}` is not a direct fixed-arity call"),
      );
      None
    }
    other => {
      issue(
        context,
        CalxFallbackCode::UnsupportedForm,
        source_path(other),
        format!("unsupported Calx scalar expression `{other}`"),
      );
      None
    }
  }
}

fn analyze_call(items: &crate::calcit::CalcitList, tail: bool, context: &mut ExpressionContext<'_>) -> Option<Option<CalxScalarType>> {
  let operator = &items[0];
  let args = items.drop_left().to_vec();
  match operator {
    Calcit::Syntax(CalcitSyntax::If, _) => analyze_if(&args, tail, context),
    Calcit::Syntax(CalcitSyntax::CoreLet, _) => analyze_let(&args, tail, context),
    Calcit::Syntax(CalcitSyntax::AssertType, _) => {
      if let Some(value) = args.first() {
        analyze_expression(value, tail, context)
      } else {
        issue(
          context,
          CalxFallbackCode::UnsupportedForm,
          source_path(operator),
          "assert-type has no value expression".to_owned(),
        );
        None
      }
    }
    Calcit::Syntax(CalcitSyntax::HintFn, _) => Some(None),
    Calcit::Syntax(syntax, _) => {
      issue(
        context,
        CalxFallbackCode::UnsupportedForm,
        source_path(operator),
        format!("syntax `{syntax}` is outside the Calx scalar subset"),
      );
      None
    }
    Calcit::Proc(proc) => analyze_proc(*proc, &args, tail, context),
    Calcit::Import(import) if import.ns.as_ref() == "calcit.core" && import.def.as_ref() == "do" => {
      analyze_sequence(&args, tail, context)
    }
    Calcit::Import(import) => analyze_direct_call(CalxDefinitionRef::new(import.ns.clone(), import.def.clone()), &args, context),
    Calcit::Fn { info, .. } if info.def_ref.is_some() => {
      let def_ref = info.def_ref.as_ref().expect("checked function definition reference");
      analyze_direct_call(
        CalxDefinitionRef::new(def_ref.def_ns.clone(), def_ref.def_name.clone()),
        &args,
        context,
      )
    }
    Calcit::Local(_) | Calcit::Fn { .. } | Calcit::Symbol { .. } => {
      analyze_unknown_args(&args, context);
      issue(
        context,
        CalxFallbackCode::IndirectCall,
        source_path(operator),
        format!("dynamic operator `{operator}` is not supported"),
      );
      None
    }
    other => {
      analyze_unknown_args(&args, context);
      issue(
        context,
        CalxFallbackCode::UnsupportedForm,
        source_path(other),
        format!("unsupported call operator `{other}`"),
      );
      None
    }
  }
}

fn analyze_if(args: &[Calcit], tail: bool, context: &mut ExpressionContext<'_>) -> Option<Option<CalxScalarType>> {
  if args.len() != 3 {
    issue(
      context,
      CalxFallbackCode::NilValue,
      args.first().and_then(source_path),
      format!("Calx scalar if requires condition, then, and else; found {} arguments", args.len()),
    );
    return None;
  }
  let condition = analyze_expression(&args[0], false, context);
  if condition != Some(Some(CalxScalarType::Bool)) {
    issue(
      context,
      CalxFallbackCode::NonBoolCondition,
      source_path(&args[0]),
      "Calx scalar conditions must be statically Bool".to_owned(),
    );
  }
  let then_type = analyze_expression(&args[1], tail, context);
  let else_type = analyze_expression(&args[2], tail, context);
  if then_type.is_some() && else_type.is_some() && then_type != else_type {
    issue(
      context,
      CalxFallbackCode::UnsupportedType,
      source_path(&args[1]),
      "if branches must have the same scalar/void result".to_owned(),
    );
    None
  } else {
    then_type.or(else_type)
  }
}

fn analyze_let(args: &[Calcit], tail: bool, context: &mut ExpressionContext<'_>) -> Option<Option<CalxScalarType>> {
  let Some((binding, body)) = args.split_first() else {
    issue(
      context,
      CalxFallbackCode::UnsupportedForm,
      None,
      "&let requires a binding and body".to_owned(),
    );
    return None;
  };
  match binding {
    Calcit::Nil | Calcit::Unit => {}
    Calcit::List(pair) if pair.len() == 2 => {
      let declared = match &pair[0] {
        Calcit::Local(local) => map_slot_type(
          &local.type_info,
          &format!("local `{}`", local.sym),
          context.function,
          local.location.as_ref().map(|path| path.as_ref().clone()),
          context.call_path,
          context.issues,
        )
        .map(Some),
        other => {
          issue(
            context,
            CalxFallbackCode::UnsupportedForm,
            source_path(other),
            format!("&let binding name `{other}` is not a resolved local"),
          );
          None
        }
      };
      let value = analyze_expression(&pair[1], false, context);
      if declared.is_some() && value.is_some() && declared != value {
        issue(
          context,
          CalxFallbackCode::UnsupportedType,
          source_path(&pair[1]),
          "&let binding value does not match its concrete local type".to_owned(),
        );
      }
    }
    other => {
      issue(
        context,
        CalxFallbackCode::UnsupportedForm,
        source_path(other),
        "&let requires exactly one resolved local binding".to_owned(),
      );
    }
  }
  analyze_sequence(body, tail, context)
}

fn analyze_proc(proc: CalcitProc, args: &[Calcit], tail: bool, context: &mut ExpressionContext<'_>) -> Option<Option<CalxScalarType>> {
  match proc {
    CalcitProc::NativeAdd | CalcitProc::NativeMultiply | CalcitProc::NativeDivide => {
      analyze_typed_args(proc, args, &[CalxScalarType::F64, CalxScalarType::F64], context)?;
      Some(Some(CalxScalarType::F64))
    }
    CalcitProc::NativeMinus if args.len() == 1 => {
      analyze_typed_args(proc, args, &[CalxScalarType::F64], context)?;
      Some(Some(CalxScalarType::F64))
    }
    CalcitProc::NativeMinus => {
      analyze_typed_args(proc, args, &[CalxScalarType::F64, CalxScalarType::F64], context)?;
      Some(Some(CalxScalarType::F64))
    }
    CalcitProc::NativeLessThan | CalcitProc::NativeGreaterThan | CalcitProc::NativeEquals => {
      analyze_typed_args(proc, args, &[CalxScalarType::F64, CalxScalarType::F64], context)?;
      Some(Some(CalxScalarType::Bool))
    }
    CalcitProc::Recur => {
      if !tail {
        issue(
          context,
          CalxFallbackCode::RecurNotTail,
          args.first().and_then(source_path),
          "recur is only eligible in tail position".to_owned(),
        );
      }
      let signature = context.signature?;
      analyze_typed_args(proc, args, &signature.params, context)?;
      Some(signature.result)
    }
    other => {
      issue(
        context,
        CalxFallbackCode::UnsupportedForm,
        args.first().and_then(source_path),
        format!("native proc `{other}` is outside the Calx scalar subset"),
      );
      None
    }
  }
}

fn analyze_typed_args(
  operator: CalcitProc,
  args: &[Calcit],
  expected: &[CalxScalarType],
  context: &mut ExpressionContext<'_>,
) -> Option<()> {
  if args.len() != expected.len() {
    issue(
      context,
      CalxFallbackCode::Arity,
      args.first().and_then(source_path),
      format!("`{operator}` expects {} arguments, found {}", expected.len(), args.len()),
    );
    return None;
  }
  let mut valid = true;
  for (argument, expected) in args.iter().zip(expected) {
    if analyze_expression(argument, false, context) != Some(Some(*expected)) {
      valid = false;
    }
  }
  valid.then_some(())
}

fn analyze_direct_call(
  target: CalxDefinitionRef,
  args: &[Calcit],
  context: &mut ExpressionContext<'_>,
) -> Option<Option<CalxScalarType>> {
  if let Some(import) = context.imports.get(&target) {
    return analyze_host_import_call(target, import, args, context);
  }
  let Some(compiled) = lookup_compiled_def(context.program, &target) else {
    analyze_unknown_args(args, context);
    issue(
      context,
      CalxFallbackCode::HostCapability,
      args.first().and_then(source_path),
      format!(
        "call target `{}` is not a declared compiled function or approved host capability",
        target.qualified()
      ),
    );
    return None;
  };
  if compiled.kind != CompiledDefKind::Fn {
    analyze_unknown_args(args, context);
    issue(
      context,
      CalxFallbackCode::IndirectCall,
      args.first().and_then(source_path),
      format!("call target `{}` is not a top-level function", target.qualified()),
    );
    return None;
  }
  context.direct_calls.insert(target.clone());
  let Some(signature) = signature_without_issues(compiled) else {
    analyze_unknown_args(args, context);
    return None;
  };
  if args.len() != signature.params.len() {
    issue(
      context,
      CalxFallbackCode::Arity,
      args.first().and_then(source_path),
      format!(
        "call target `{}` expects {} arguments, found {}",
        target.qualified(),
        signature.params.len(),
        args.len()
      ),
    );
    return None;
  }
  let mut valid = true;
  for (argument, expected) in args.iter().zip(&signature.params) {
    if analyze_expression(argument, false, context) != Some(Some(*expected)) {
      valid = false;
    }
  }
  valid.then_some(signature.result)
}

fn analyze_host_import_call(
  target: CalxDefinitionRef,
  import: &CalxHostImport,
  args: &[Calcit],
  context: &mut ExpressionContext<'_>,
) -> Option<Option<CalxScalarType>> {
  let Some(compiled) = lookup_compiled_def(context.program, &target) else {
    analyze_unknown_args(args, context);
    issue(
      context,
      CalxFallbackCode::HostCapability,
      args.first().and_then(source_path),
      format!("approved host import `{}` is missing from the typed snapshot", target.qualified()),
    );
    return None;
  };
  if compiled.kind != CompiledDefKind::Fn {
    analyze_unknown_args(args, context);
    issue(
      context,
      CalxFallbackCode::HostCapability,
      source_path(&compiled.preprocessed_code),
      format!("approved host import `{}` is not a top-level function", target.qualified()),
    );
    return None;
  }
  let declared = analyze_signature(compiled, &target, context.call_path, context.issues)?;
  if declared.params != import.params || declared.result != import.result {
    analyze_unknown_args(args, context);
    issue(
      context,
      CalxFallbackCode::HostCapability,
      source_path(&compiled.preprocessed_code),
      format!(
        "approved host import `{}` declares ({:?})->{:?}, but typed snapshot requires ({:?})->{:?}",
        target.qualified(),
        import.params,
        import.result,
        declared.params,
        declared.result
      ),
    );
    return None;
  }
  if args.len() != import.params.len() {
    issue(
      context,
      CalxFallbackCode::Arity,
      args.first().and_then(source_path),
      format!(
        "host import `{}` expects {} arguments, found {}",
        target.qualified(),
        import.params.len(),
        args.len()
      ),
    );
    return None;
  }
  let mut valid = true;
  for (argument, expected) in args.iter().zip(&import.params) {
    if analyze_expression(argument, false, context) != Some(Some(*expected)) {
      valid = false;
    }
  }
  if valid {
    context.host_imports.insert(target);
    Some(import.result)
  } else {
    None
  }
}

fn analyze_unknown_args(args: &[Calcit], context: &mut ExpressionContext<'_>) {
  for argument in args {
    analyze_expression(argument, false, context);
  }
}

fn map_slot_type(
  annotation: &CalcitTypeAnnotation,
  boundary: &str,
  target: &CalxDefinitionRef,
  source_path: Option<Vec<u16>>,
  call_path: &[CalxDefinitionRef],
  issues: &mut Vec<CalxFallbackIssue>,
) -> Option<CalxScalarType> {
  match annotation {
    CalcitTypeAnnotation::Number => Some(CalxScalarType::F64),
    CalcitTypeAnnotation::Bool => Some(CalxScalarType::Bool),
    CalcitTypeAnnotation::Dynamic => {
      push_expression_issue(
        issues,
        CalxFallbackCode::DynamicType,
        target,
        source_path,
        call_path,
        format!("{boundary} remains Dynamic"),
      );
      None
    }
    CalcitTypeAnnotation::Nil | CalcitTypeAnnotation::Optional(_) | CalcitTypeAnnotation::JsNullish(_) => {
      push_expression_issue(
        issues,
        CalxFallbackCode::NilValue,
        target,
        source_path,
        call_path,
        format!("{boundary} can contain Nil/absence"),
      );
      None
    }
    other => {
      push_expression_issue(
        issues,
        CalxFallbackCode::UnsupportedType,
        target,
        source_path,
        call_path,
        format!("{boundary} type `{other}` is outside the Calx scalar subset"),
      );
      None
    }
  }
}

fn map_result_type(
  annotation: &CalcitTypeAnnotation,
  boundary: &str,
  target: &CalxDefinitionRef,
  source_path: Option<Vec<u16>>,
  call_path: &[CalxDefinitionRef],
  issues: &mut Vec<CalxFallbackIssue>,
) -> Option<Option<CalxScalarType>> {
  if matches!(annotation, CalcitTypeAnnotation::Unit) {
    Some(None)
  } else {
    map_slot_type(annotation, boundary, target, source_path, call_path, issues).map(Some)
  }
}

fn map_slot_type_quiet(annotation: &CalcitTypeAnnotation) -> Option<CalxScalarType> {
  match annotation {
    CalcitTypeAnnotation::Number => Some(CalxScalarType::F64),
    CalcitTypeAnnotation::Bool => Some(CalxScalarType::Bool),
    _ => None,
  }
}

fn map_result_type_quiet(annotation: &CalcitTypeAnnotation) -> Option<Option<CalxScalarType>> {
  if matches!(annotation, CalcitTypeAnnotation::Unit) {
    Some(None)
  } else {
    map_slot_type_quiet(annotation).map(Some)
  }
}

fn issue(context: &mut ExpressionContext<'_>, code: CalxFallbackCode, source_path: Option<Vec<u16>>, message: String) {
  push_expression_issue(context.issues, code, context.function, source_path, context.call_path, message);
}

fn push_expression_issue(
  issues: &mut Vec<CalxFallbackIssue>,
  code: CalxFallbackCode,
  target: &CalxDefinitionRef,
  source_path: Option<Vec<u16>>,
  call_path: &[CalxDefinitionRef],
  message: String,
) {
  issues.push(CalxFallbackIssue {
    code,
    namespace: target.namespace.clone(),
    definition: target.definition.clone(),
    source_path,
    call_path: call_path.to_vec(),
    message,
  });
}

fn source_path(expression: &Calcit) -> Option<Vec<u16>> {
  match expression {
    Calcit::Local(local) => local.location.as_ref().map(|path| path.as_ref().clone()),
    Calcit::Symbol { location, .. } => location.as_ref().map(|path| path.as_ref().clone()),
    Calcit::List(items) => items.iter().find_map(source_path),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fallback_codes_keep_the_v1_abi_names() {
    assert_eq!(
      [
        CalxFallbackCode::DynamicType,
        CalxFallbackCode::NilValue,
        CalxFallbackCode::UnsupportedType,
        CalxFallbackCode::UnsupportedForm,
        CalxFallbackCode::IndirectCall,
        CalxFallbackCode::Arity,
        CalxFallbackCode::NonBoolCondition,
        CalxFallbackCode::RecurNotTail,
        CalxFallbackCode::HostCapability,
        CalxFallbackCode::CallClosure,
        CalxFallbackCode::AbiEdition,
      ]
      .map(CalxFallbackCode::as_str),
      [
        "CALX_SUBSET_DYNAMIC_TYPE",
        "CALX_SUBSET_NIL_VALUE",
        "CALX_SUBSET_UNSUPPORTED_TYPE",
        "CALX_SUBSET_UNSUPPORTED_FORM",
        "CALX_SUBSET_INDIRECT_CALL",
        "CALX_SUBSET_ARITY",
        "CALX_SUBSET_NON_BOOL_CONDITION",
        "CALX_SUBSET_RECUR_NOT_TAIL",
        "CALX_SUBSET_HOST_CAPABILITY",
        "CALX_SUBSET_CALL_CLOSURE",
        "CALX_SUBSET_ABI_EDITION",
      ]
    );
  }
}
