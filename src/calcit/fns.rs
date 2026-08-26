use std::{fmt::Display, sync::Arc};

use crate::Calcit;

use super::{CalcitGenericBound, CalcitLocal, CalcitTypeAnnotation};

/// Counts the continuous `Option<T>` suffix of a fixed-arity parameter list.
/// Incomplete type metadata is not enough to make any parameter omittable.
pub(crate) fn trailing_option_arg_count(arg_types: &[Arc<CalcitTypeAnnotation>], param_len: usize) -> usize {
  if arg_types.len() != param_len {
    return 0;
  }
  arg_types.iter().rev().take_while(|arg| arg.is_option_type()).count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamShapeToken {
  Binding,
  OptionalMark,
  RestMark,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamShape {
  pub required: usize,
  pub optional: usize,
  pub has_rest: bool,
  pub errors: Vec<&'static str>,
}

impl ParamShape {
  pub fn from_tokens(tokens: impl IntoIterator<Item = ParamShapeToken>) -> Self {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Phase {
      Required,
      Optional,
      RestBinding,
      AfterRest,
    }

    let mut shape = Self {
      required: 0,
      optional: 0,
      has_rest: false,
      errors: vec![],
    };
    let mut phase = Phase::Required;
    let mut saw_optional_mark = false;

    for token in tokens {
      match token {
        ParamShapeToken::Binding => match phase {
          Phase::Required => shape.required += 1,
          Phase::Optional => shape.optional += 1,
          Phase::RestBinding => {
            shape.has_rest = true;
            phase = Phase::AfterRest;
          }
          Phase::AfterRest => shape.errors.push("binding appears after the rest parameter"),
        },
        ParamShapeToken::OptionalMark => match phase {
          Phase::Required => {
            saw_optional_mark = true;
            phase = Phase::Optional;
          }
          Phase::Optional => shape.errors.push("optional marker appears more than once"),
          Phase::RestBinding | Phase::AfterRest => shape.errors.push("optional marker appears after the rest marker"),
        },
        ParamShapeToken::RestMark => match phase {
          Phase::Required | Phase::Optional => phase = Phase::RestBinding,
          Phase::RestBinding | Phase::AfterRest => shape.errors.push("rest marker appears more than once"),
        },
      }
    }

    if saw_optional_mark && shape.optional == 0 {
      shape.errors.push("optional marker is missing an optional binding");
    }
    if phase == Phase::RestBinding {
      shape.errors.push("rest marker is missing its binding");
    }
    shape
  }

  pub fn from_schema(arg_types: &[Arc<CalcitTypeAnnotation>], has_rest: bool) -> Self {
    let optional = trailing_option_arg_count(arg_types, arg_types.len());
    Self {
      required: arg_types.len() - optional,
      optional,
      has_rest,
      errors: vec![],
    }
  }

  /// Preserve the established callable arity model for ordinary functions,
  /// whose schemas describe every fixed slot but do not encode the `?`
  /// marker separately. Macro diagnostics use the full shape instead.
  pub fn as_fixed_arity(&self) -> Self {
    Self {
      required: self.required + self.optional,
      optional: 0,
      has_rest: self.has_rest,
      errors: self.errors.clone(),
    }
  }
}

pub fn compare_param_shapes(owner: &str, code: &ParamShape, schema: &ParamShape) -> Vec<String> {
  let mut issues = code
    .errors
    .iter()
    .map(|detail| format!("[E_DEF_PARAM_SHAPE] {owner}: malformed parameter list: {detail}"))
    .collect::<Vec<_>>();
  if code.required != schema.required {
    issues.push(format!(
      "[E_SCHEMA_REQUIRED_ARGS] {owner}: schema has {} required arg(s) but code has {}",
      schema.required, code.required
    ));
  }
  if code.optional != schema.optional {
    issues.push(format!(
      "[E_SCHEMA_OPTIONAL_ARGS] {owner}: schema has {} optional arg(s) but code has {}",
      schema.optional, code.optional
    ));
  }
  if code.has_rest != schema.has_rest {
    issues.push(if code.has_rest {
      format!("[E_SCHEMA_REST_ARGS] {owner}: code has & rest param but schema has no :rest")
    } else {
      format!("[E_SCHEMA_REST_ARGS] {owner}: schema has :rest but code has no & param")
    });
  }
  issues
}

/// structure of a function arguments
#[derive(Debug, Clone)]
pub enum CalcitArgLabel {
  /// variable
  Idx(u16),
  /// `?``
  OptionalMark,
  /// `&`
  RestMark,
}

impl Display for CalcitArgLabel {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      CalcitArgLabel::Idx(s) => write!(f, "{}", CalcitLocal::read_name(*s)),
      CalcitArgLabel::OptionalMark => write!(f, "?"),
      CalcitArgLabel::RestMark => write!(f, "&"),
    }
  }
}

#[derive(Debug, Clone)]
pub enum CalcitFnArgs {
  MarkedArgs(Vec<CalcitArgLabel>),
  Args(Vec<u16>),
}

/// Execution-only callable shape cached after typed preprocessing.
///
/// Native calls should not have to walk argument annotations merely to decide
/// arity and trailing `Option` completion. The full annotations stay on
/// `CalcitFn` for codegen, queries, and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalcitFnCallShape {
  param_len: u16,
  trailing_optionals: u16,
  has_rest: bool,
}

impl CalcitFnCallShape {
  pub fn fixed(param_len: usize) -> Self {
    Self {
      param_len: u16::try_from(param_len).expect("function parameter count exceeds local index capacity"),
      trailing_optionals: 0,
      has_rest: false,
    }
  }

  pub fn from_parts(args: &CalcitFnArgs, arg_types: &[Arc<CalcitTypeAnnotation>], has_typed_rest: bool) -> Self {
    let param_len = args.param_len();
    let has_marked_rest =
      matches!(args, CalcitFnArgs::MarkedArgs(labels) if labels.iter().any(|label| matches!(label, CalcitArgLabel::RestMark)));
    Self {
      param_len: u16::try_from(param_len).expect("function parameter count exceeds local index capacity"),
      trailing_optionals: u16::try_from(trailing_option_arg_count(arg_types, param_len))
        .expect("optional parameter count exceeds local index capacity"),
      has_rest: has_typed_rest || has_marked_rest,
    }
  }

  pub fn param_len(self) -> usize {
    usize::from(self.param_len)
  }

  pub fn trailing_optionals(self) -> usize {
    usize::from(self.trailing_optionals)
  }

  pub fn has_rest(self) -> bool {
    self.has_rest
  }
}

impl CalcitFnArgs {
  /// Counts positional parameters(either indexed locals or symbols) while ignoring markers.
  pub fn param_len(&self) -> usize {
    match self {
      CalcitFnArgs::MarkedArgs(xs) => xs.iter().filter(|label| matches!(label, CalcitArgLabel::Idx(_))).count(),
      CalcitFnArgs::Args(xs) => xs.len(),
    }
  }

  /// Produce a Vec<Arc<...>> aligned with current parameter arity for storing type hints.
  pub fn empty_arg_types(&self) -> Vec<Arc<CalcitTypeAnnotation>> {
    vec![super::DYNAMIC_TYPE.clone(); self.param_len()]
  }
}

#[derive(Debug, Clone)]
pub struct CalcitFn {
  pub name: Arc<str>,
  /// where it was defined
  pub def_ns: Arc<str>,
  /// reference to a top-level defn when available
  pub def_ref: Option<CalcitFnDefRef>,
  /// usage metadata for codegen and diagnostics
  pub usage: CalcitFnUsageMeta,
  pub scope: Arc<CalcitScope>,
  pub args: Arc<CalcitFnArgs>,
  /// compact arity facts used by the native call path
  pub call_shape: CalcitFnCallShape,
  pub body: Vec<Calcit>,
  /// generics declared by hint-fn
  pub generics: Arc<Vec<Arc<str>>>,
  /// generic trait bounds declared by hint-fn/:schema :where
  pub where_bounds: Arc<Vec<CalcitGenericBound>>,
  /// return type declared by hint-fn
  pub return_type: Arc<CalcitTypeAnnotation>,
  /// argument types declared by assert-type
  pub arg_types: Vec<Arc<CalcitTypeAnnotation>>,
  /// element type accepted by a `&` rest parameter, when present
  pub rest_type: Option<Arc<CalcitTypeAnnotation>>,
}

#[derive(Debug, Clone)]
pub struct CalcitFnDefRef {
  pub def_ns: Arc<str>,
  pub def_name: Arc<str>,
  pub coord: Option<(u16, u16)>,
  pub is_defn: bool,
  pub is_macro_gen: bool,
}

impl Default for CalcitFnDefRef {
  fn default() -> Self {
    CalcitFnDefRef {
      def_ns: Arc::from(""),
      def_name: Arc::from(""),
      coord: None,
      is_defn: false,
      is_macro_gen: false,
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct CalcitFnUsageMeta {
  pub used_in_impl: bool,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn counts_plain_args() {
    let args = CalcitFnArgs::Args(vec![1, 2, 3]);
    assert_eq!(args.param_len(), 3);
    assert_eq!(args.empty_arg_types().len(), 3);
    assert!(
      args
        .empty_arg_types()
        .iter()
        .all(|item| matches!(**item, CalcitTypeAnnotation::Dynamic))
    );
  }

  #[test]
  fn counts_marked_args_only_on_locals() {
    let args = CalcitFnArgs::MarkedArgs(vec![
      CalcitArgLabel::Idx(1),
      CalcitArgLabel::OptionalMark,
      CalcitArgLabel::Idx(2),
      CalcitArgLabel::RestMark,
    ]);
    assert_eq!(args.param_len(), 2, "only locals should be counted toward arity");
    assert_eq!(args.empty_arg_types().len(), 2);
    assert!(
      args
        .empty_arg_types()
        .iter()
        .all(|item| matches!(**item, CalcitTypeAnnotation::Dynamic))
    );
  }

  #[test]
  fn counts_only_continuous_option_suffix() {
    let option_number = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("Option"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
    ));
    let types = vec![option_number.clone(), Arc::new(CalcitTypeAnnotation::String), option_number.clone()];
    assert_eq!(trailing_option_arg_count(&types, 3), 1);

    let types = vec![Arc::new(CalcitTypeAnnotation::Number), option_number.clone(), option_number];
    assert_eq!(trailing_option_arg_count(&types, 3), 2);
    assert_eq!(trailing_option_arg_count(&types, 4), 0, "partial metadata must keep exact arity");
  }

  #[test]
  fn caches_runtime_call_shape_from_typed_arguments() {
    let option_number = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("Option"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
    ));
    let args = CalcitFnArgs::Args(vec![1, 2, 3]);
    let shape = CalcitFnCallShape::from_parts(
      &args,
      &[Arc::new(CalcitTypeAnnotation::Number), option_number.clone(), option_number],
      false,
    );
    assert_eq!(shape.param_len(), 3);
    assert_eq!(shape.trailing_optionals(), 2);
    assert!(!shape.has_rest());
  }

  #[test]
  fn runtime_call_shape_combines_marked_and_typed_rest_evidence() {
    let marked = CalcitFnArgs::MarkedArgs(vec![CalcitArgLabel::Idx(1), CalcitArgLabel::RestMark, CalcitArgLabel::Idx(2)]);
    let marked_shape = CalcitFnCallShape::from_parts(&marked, &[], false);
    assert_eq!(marked_shape.param_len(), 2);
    assert!(marked_shape.has_rest());

    let typed = CalcitFnArgs::Args(vec![1]);
    assert!(CalcitFnCallShape::from_parts(&typed, &[crate::calcit::DYNAMIC_TYPE.clone()], true).has_rest());
  }

  #[test]
  fn restores_call_frame_without_dropping_captured_bindings() {
    let mut scope = CalcitScope::default();
    scope.insert_mut(1, Calcit::Number(42.0));
    let checkpoint = scope.frame_checkpoint();

    for value in 0..100_000 {
      scope.restore_frame(checkpoint);
      scope.insert_mut(2, Calcit::Number(value as f64));
      scope.insert_mut(3, Calcit::Number((value + 1) as f64));
    }

    assert_eq!(scope.0.len(), checkpoint + 2);
    assert_eq!(scope.get(1), Some(&Calcit::Number(42.0)));
    assert_eq!(scope.get(2), Some(&Calcit::Number(99_999.0)));
    assert_eq!(scope.get(3), Some(&Calcit::Number(100_000.0)));
  }

  #[test]
  fn parameter_shapes_track_required_optional_and_rest_bindings() {
    let shape = ParamShape::from_tokens([
      ParamShapeToken::Binding,
      ParamShapeToken::OptionalMark,
      ParamShapeToken::Binding,
      ParamShapeToken::RestMark,
      ParamShapeToken::Binding,
    ]);
    assert_eq!(shape.required, 1);
    assert_eq!(shape.optional, 1);
    assert!(shape.has_rest);
    assert!(shape.errors.is_empty());
  }

  #[test]
  fn parameter_shapes_report_malformed_marker_sequences() {
    let cases = [
      (vec![ParamShapeToken::OptionalMark], "optional marker is missing"),
      (vec![ParamShapeToken::RestMark], "rest marker is missing"),
      (
        vec![ParamShapeToken::RestMark, ParamShapeToken::Binding, ParamShapeToken::Binding],
        "binding appears after",
      ),
      (
        vec![ParamShapeToken::OptionalMark, ParamShapeToken::OptionalMark],
        "optional marker appears more than once",
      ),
      (
        vec![ParamShapeToken::OptionalMark, ParamShapeToken::RestMark, ParamShapeToken::Binding],
        "optional marker is missing",
      ),
    ];
    for (tokens, expected) in cases {
      let shape = ParamShape::from_tokens(tokens);
      assert!(shape.errors.iter().any(|error| error.contains(expected)), "{shape:?}");
    }
  }
}

/// Macro variant of Calcit data
#[derive(Debug, Clone)]
pub struct CalcitMacro {
  pub name: Arc<str>,
  /// where it was defined
  pub def_ns: Arc<str>,
  pub args: Arc<Vec<CalcitArgLabel>>,
  pub body: Arc<Vec<Calcit>>,
  pub signature: Arc<crate::calcit::MacroSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopePair {
  pub key: u16,
  pub value: Calcit,
}

impl Display for ScopePair {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}: {}", self.key, self.value)
  }
}

/// scope backed by a contiguous Vec for cache-friendly reverse linear scan
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CalcitScope(Vec<ScopePair>);

impl CalcitScope {
  /// Capture the current end of a lexical frame before adding call-local bindings.
  pub(crate) fn frame_checkpoint(&self) -> usize {
    self.0.len()
  }

  /// Drop bindings added after a frame checkpoint while preserving captured scope.
  pub(crate) fn restore_frame(&mut self, checkpoint: usize) {
    self.0.truncate(checkpoint);
  }

  /// load value of a symbol from the scope (reverse scan for shadowing)
  pub fn get(&self, key: u16) -> Option<&Calcit> {
    for pair in self.0.iter().rev() {
      if pair.key == key {
        return Some(&pair.value);
      }
    }
    None
  }

  pub fn get_by_name(&self, s: &str) -> Option<&Calcit> {
    let key = CalcitLocal::track_sym(&Arc::from(s));
    self.get(key)
  }

  /// mutable insertion of variable
  pub fn insert_mut(&mut self, key: u16, value: Calcit) {
    self.0.push(ScopePair { key, value });
  }

  pub fn get_names(&self) -> String {
    let mut vars = String::new();
    for (i, k) in self.0.iter().enumerate() {
      if i > 0 {
        vars.push(',');
      }
      let name = CalcitLocal::read_name(k.key);
      vars.push_str(&name);
    }
    vars
  }
}
