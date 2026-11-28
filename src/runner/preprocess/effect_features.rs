//! Effect boundaries for `:state` and `:io` (#2710).
//!
//! A function that reads or writes a Ref it did not create, prints, touches
//! files, the environment, the clock or the process has to declare the matching
//! feature in its `Fn` schema. The rule is "occurrence is use": a primitive
//! operation, a call to or a reference of a definition that declares the
//! feature, or a call of a local `Fn` value whose type carries it, all count
//! for the enclosing definition. A nested lambda without its own `hint-fn`
//! features belongs to the enclosing definition; a lambda that declares
//! `:state` / `:io` itself is checked against its own declaration instead, and
//! creating it is not a use.
//!
//! Unlike `:js-ffi`, which is lexical, these features propagate to callers:
//! they describe behaviour a caller can observe.
//!
//! Core functions are read from their source (`read-file` is an alias of the
//! `&read-file` proc, `deref` wraps `&atom:deref`), so their frozen schemas do
//! not need to list features.
//!
//! `dbg` expands to `calcit.core/&dbg-print`, which is not an effect; the entry
//! policy `:dbg` decides whether such debug output is allowed. `eprintln` in a
//! block that ends by raising is failure reporting (assertion macros expand to
//! this shape), not an effect.
//!
//! The check runs on the preprocessed definition, so macro expansions are
//! already resolved to `Proc`, `Syntax`, `Registered` and `Import` heads.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

use crate::calcit::{
  Calcit, CalcitCallKind, CalcitFnTypeAnnotation, CalcitImport, CalcitLocal, CalcitProc, CalcitSyntax, CalcitTrait,
  CalcitTypeAnnotation, LocatedWarning, MethodKind, NodeLocation,
};
use crate::program;
use crate::snapshot::FeaturePolicy;

pub(crate) const STATE_FEATURE: &str = "state";
pub(crate) const IO_FEATURE: &str = "io";
pub(crate) const DBG_POLICY: &str = "dbg";

pub const STATE_FEATURE_WARNING: &str = "W_STATE_FEATURE_REQUIRED";
pub const STATE_FEATURE_ERROR: &str = "E_STATE_FEATURE_REQUIRED";
pub const IO_FEATURE_WARNING: &str = "W_IO_FEATURE_REQUIRED";
pub const IO_FEATURE_ERROR: &str = "E_IO_FEATURE_REQUIRED";
pub const DBG_IN_ENTRY_WARNING: &str = "W_DBG_IN_ENTRY";
pub const DBG_IN_ENTRY_ERROR: &str = "E_DBG_IN_ENTRY";

const DBG_PRINT_DEF: &str = "&dbg-print";

/// Policy for `:state` / `:io` when the entry does not mention them.
/// Since 0.43.0 a missing declaration fails the check; an entry can still
/// opt back into `:warn` or `:allow` through `:feature-policy`.
const DEFAULT_EFFECT_POLICY: FeaturePolicy = FeaturePolicy::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Effect {
  State,
  Io,
}

impl Effect {
  fn feature(self) -> &'static str {
    match self {
      Self::State => STATE_FEATURE,
      Self::Io => IO_FEATURE,
    }
  }

  fn codes(self) -> (&'static str, &'static str) {
    match self {
      Self::State => (STATE_FEATURE_WARNING, STATE_FEATURE_ERROR),
      Self::Io => (IO_FEATURE_WARNING, IO_FEATURE_ERROR),
    }
  }

  fn policy(self) -> FeaturePolicy {
    program::configured_feature_policy(self.feature()).unwrap_or(DEFAULT_EFFECT_POLICY)
  }
}

/// Effect of a runtime proc used directly in a function body.
pub(crate) fn proc_effect(proc: &CalcitProc) -> Option<Effect> {
  use CalcitProc::*;
  match proc {
    AtomDeref | AddWatch | RemoveWatch => Some(Effect::State),
    GenerateId | NativeResetGenSymIndex => Some(Effect::State),
    NativeDisplayStack
    | GetEnv
    | GetArgs
    | ReadFile
    | NativeFsReadText
    | NativeReadStdinText
    | NativeFsReadDir
    | ReadDir
    | WriteFile
    | NativeFsWriteText
    | UnixTimeMs
    | MonotonicTimeMs
    | NativeWaitMs
    | NativeSecureRandomBytes
    | Quit => Some(Effect::Io),
    _ => None,
  }
}

/// Console output registered by the host (`println`, `eprintln`, `echo`).
pub(crate) fn registered_effect(name: &str) -> Option<Effect> {
  matches!(name, "println" | "eprintln" | "echo").then_some(Effect::Io)
}

fn declared_effects(features: &HashSet<cirru_edn::EdnTag>) -> HashSet<Effect> {
  let mut effects = HashSet::new();
  for feature in features {
    match feature.ref_str() {
      STATE_FEATURE => {
        effects.insert(Effect::State);
      }
      IO_FEATURE => {
        effects.insert(Effect::Io);
      }
      _ => {}
    }
  }
  effects
}

fn resolve_fn_in_ns(ns: &str, schema: &Arc<CalcitTypeAnnotation>) -> Option<Arc<CalcitFnTypeAnnotation>> {
  schema
    .resolve_to_fn()
    .or_else(|| super::resolve_namespace_type_refs_for_body(schema.clone(), ns).resolve_to_fn())
}

fn lookup_non_core_import_schema(ns: &str, def: &str) -> Arc<CalcitTypeAnnotation> {
  let source_schema = program::lookup_def_schema(ns, def);
  let schema = if !matches!(source_schema.as_ref(), CalcitTypeAnnotation::Dynamic) {
    source_schema
  } else {
    program::lookup_compiled_def(ns, def)
      .map(|compiled| compiled.schema)
      .unwrap_or(source_schema)
  };
  super::resolve_namespace_type_refs_for_body(schema, ns)
}

fn schema_effects(schema: &CalcitTypeAnnotation) -> HashSet<Effect> {
  schema
    .resolve_to_fn()
    .map(|signature| declared_effects(&signature.features))
    .unwrap_or_default()
}

/// One function scope whose declared features are checked.
struct Scope {
  owner: String,
  declared: HashSet<Effect>,
  /// The definition's own scope, as opposed to a lambda with its own hint.
  top: bool,
}

type BoundCallbackEffects = HashMap<(Arc<str>, u16), Vec<(Effect, String)>>;

/// What a definition's own scope needs, before its schema is consulted:
/// effects it causes itself, and the project definitions it uses whose
/// features it inherits. `calcit fix --rule effect-features-v1` solves these
/// to a fixpoint.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectSummary {
  pub state: bool,
  pub io: bool,
  pub deps: BTreeSet<(Arc<str>, Arc<str>)>,
}

/// Summaries keyed by `(namespace, definition)`.
pub type EffectSummaries = HashMap<(Arc<str>, Arc<str>), EffectSummary>;

static RECORDING: Mutex<Option<EffectSummaries>> = Mutex::new(None);

/// Start collecting [`EffectSummary`] values for every checked definition.
pub fn start_effect_summaries() {
  *RECORDING.lock().expect("effect summaries") = Some(HashMap::new());
}

/// Stop collecting and return the summaries gathered since the last start.
pub fn take_effect_summaries() -> EffectSummaries {
  RECORDING.lock().expect("effect summaries").take().unwrap_or_default()
}

fn recording() -> bool {
  RECORDING.lock().expect("effect summaries").is_some()
}

struct Checker<'a> {
  ns: &'a str,
  def: &'a str,
  /// Locals bound to a Ref created by `ref` inside this definition.
  local_refs: HashSet<(Arc<str>, u16)>,
  /// Locals bound to core functions with open `:pass`-like callback slots.
  local_open_callback_slots: HashMap<(Arc<str>, u16), &'static [usize]>,
  /// Static types active in the current lexical scope for method/trait lookup.
  scope_types: super::ScopeTypes,
  /// `(owner, effect)` pairs already reported, one diagnostic each.
  reported: HashSet<(String, Effect)>,
  dbg_reported: bool,
  /// Inside a block that ends with `raise`, where `eprintln` reports the failure.
  failure_report: bool,
  advisories: Vec<LocatedWarning>,
  /// Collected for `calcit fix` when recording is on.
  summary: Option<EffectSummary>,
  /// Set while reporting a project definition's features: that need comes
  /// from a dependency, not from this definition's own body.
  via_dependency: bool,
}

/// Check one preprocessed top-level definition. Only `defn` roots are
/// functions; values, macros and Refs declared with `defref` are skipped.
pub(crate) fn check_definition(ns: &str, def: &str, resolved: &Calcit) -> Vec<LocatedWarning> {
  let Calcit::List(items) = resolved else {
    return vec![];
  };
  if !matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) {
    return vec![];
  }
  let state_policy = Effect::State.policy();
  let io_policy = Effect::Io.policy();
  let dbg_policy = program::configured_feature_policy(DBG_POLICY).unwrap_or(FeaturePolicy::Allow);
  let record = recording();
  if !record && state_policy == FeaturePolicy::Allow && io_policy == FeaturePolicy::Allow && dbg_policy == FeaturePolicy::Allow {
    return vec![];
  }
  // Only a structured `Fn` schema can declare features; eval snippets and
  // untyped helpers have none and are not checked.
  let schema = super::resolve_namespace_type_refs_for_body(program::lookup_def_schema(ns, def), ns);
  let Some(signature) = schema.resolve_to_fn() else {
    return vec![];
  };
  let mut scope_types = super::ScopeTypes::new();
  scope_types.bind_function_generics(signature.as_ref(), ns);
  if let Some(Calcit::List(params)) = items.get(2) {
    for param in params.iter() {
      if let Calcit::Local(local) = param {
        scope_types.bind_parameter(local.sym.clone(), local.type_info.clone());
      }
    }
  }
  let mut checker = Checker {
    ns,
    def,
    local_refs: HashSet::new(),
    local_open_callback_slots: HashMap::new(),
    scope_types,
    reported: HashSet::new(),
    dbg_reported: false,
    failure_report: false,
    advisories: vec![],
    summary: record.then(EffectSummary::default),
    via_dependency: false,
  };
  let scope = Scope {
    owner: format!("{ns}/{def}"),
    declared: declared_effects(&signature.features),
    top: true,
  };
  for item in items.iter().skip(3) {
    checker.walk(item, &scope, None);
  }
  if let Some(summary) = checker.summary.take()
    && let Some(store) = RECORDING.lock().expect("effect summaries").as_mut()
  {
    store.insert((Arc::from(ns), Arc::from(def)), summary);
  }
  checker.advisories
}

impl Checker<'_> {
  fn fallback_location(&self) -> NodeLocation {
    NodeLocation::new(Arc::from(self.ns), Arc::from(self.def), Arc::new(vec![]))
  }

  fn walk(&mut self, form: &Calcit, scope: &Scope, near: Option<&NodeLocation>) {
    match form {
      Calcit::Proc(proc) => {
        if let Some(effect) = proc_effect(proc) {
          self.require(scope, effect, &format!("uses `{proc}`"), near);
        }
      }
      Calcit::Registered(name) => {
        if self.failure_report && name.as_ref() == "eprintln" {
          return;
        }
        if let Some(effect) = registered_effect(name) {
          self.require(scope, effect, &format!("uses `{name}`"), near);
        }
      }
      Calcit::Import(import) => {
        if import.ns.as_ref() == crate::calcit::CORE_NS && import.def.as_ref() == DBG_PRINT_DEF {
          self.note_dbg(near);
          return;
        }
        self.charge_import_effects(import, scope, near);
      }
      Calcit::List(items) => self.walk_list(items, scope, near),
      _ => {}
    }
  }

  fn walk_list(&mut self, items: &crate::calcit::CalcitList, scope: &Scope, near: Option<&NodeLocation>) {
    let located = items
      .iter()
      .find_map(Calcit::get_location)
      .filter(|location| !location.coord.is_empty());
    let near = located.as_ref().or(near);
    match items.first() {
      // Type annotations and quoted code are data, not behaviour.
      Some(Calcit::Syntax(CalcitSyntax::HintFn | CalcitSyntax::Quote, _)) => {}
      Some(Calcit::Syntax(CalcitSyntax::Defn, _)) => self.walk_lambda(items, scope, near),
      Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) => {
        // Local Ref, HOF-alias, and type bindings only belong inside their
        // own lexical body; restore the outer state on exit so inner defs do
        // not leak into outer same-named locals.
        let saved_refs = self.local_refs.clone();
        let saved_slots = self.local_open_callback_slots.clone();
        let saved_types = self.scope_types.clone();
        if let Some(Calcit::List(binding)) = items.get(1)
          && let (Some(Calcit::Local(local)), Some(init)) = (binding.first(), binding.get(1))
        {
          self.walk(init, scope, near);
          // Read open-slot provenance before `shadow` clears same-named outer bindings.
          let init_slots = self.open_callback_slots_for_expr(init);
          self.shadow(&local.sym);
          if is_ref_constructor(init) {
            self.local_refs.insert((local.sym.clone(), local.idx));
          }
          if let Some(slots) = init_slots {
            self.local_open_callback_slots.insert((local.sym.clone(), local.idx), slots);
          }
          let init_type = if !matches!(local.type_info.as_ref(), CalcitTypeAnnotation::Dynamic) {
            local.type_info.clone()
          } else {
            super::type_inference::resolve_type_value(init, &self.scope_types).unwrap_or_else(|| local.type_info.clone())
          };
          self.scope_types.insert(local.sym.clone(), init_type);
        }
        let body = items.iter().skip(2).collect::<Vec<_>>();
        let ends_with_raise = matches!(body.last(), Some(Calcit::List(last)) if matches!(last.first(), Some(Calcit::Proc(CalcitProc::Raise | CalcitProc::Unreachable | CalcitProc::NativeUnreachable))));
        let previous = self.failure_report;
        if ends_with_raise {
          self.failure_report = true;
        }
        for item in body {
          self.walk(item, scope, near);
        }
        self.failure_report = previous;
        self.local_refs = saved_refs;
        self.local_open_callback_slots = saved_slots;
        self.scope_types = saved_types;
      }
      Some(Calcit::Syntax(CalcitSyntax::Match, _)) => {
        if let Some(scrutinee) = items.get(1) {
          self.walk(scrutinee, scope, near);
        }
        if let Some(branches) = super::type_inference::preprocessed_match_branches(items) {
          for (pattern, body) in branches {
            let saved_refs = self.local_refs.clone();
            let saved_slots = self.local_open_callback_slots.clone();
            let saved_types = self.scope_types.clone();
            self.bind_match_pattern_locals(pattern);
            self.walk(body, scope, near);
            self.local_refs = saved_refs;
            self.local_open_callback_slots = saved_slots;
            self.scope_types = saved_types;
          }
        } else {
          for item in items.iter().skip(2) {
            self.walk(item, scope, near);
          }
        }
      }
      Some(Calcit::Syntax(CalcitSyntax::Reset, _)) => {
        self.check_ref_target(items.get(1), "reset!", scope, near);
        for item in items.iter().skip(2) {
          self.walk(item, scope, near);
        }
      }
      Some(Calcit::Syntax(CalcitSyntax::CallSpread, _)) => {
        if let Some(callee) = items.get(1) {
          let call_args: Vec<&Calcit> = items
            .iter()
            .skip(2)
            .take_while(|arg| !matches!(arg, Calcit::Syntax(CalcitSyntax::ArgSpread, _)))
            .collect();
          self.charge_call_effects(Some(callee), &call_args, true, scope, near);
        }
        for item in items.iter().skip(1) {
          self.walk(item, scope, near);
        }
      }
      Some(Calcit::Proc(proc @ (CalcitProc::AtomDeref | CalcitProc::AddWatch | CalcitProc::RemoveWatch))) => {
        let label = proc.to_string();
        self.check_ref_target(items.get(1), &label, scope, near);
        for item in items.iter().skip(2) {
          self.walk(item, scope, near);
        }
      }
      Some(Calcit::Import(import)) if import.ns.as_ref() == crate::calcit::CORE_NS && import.def.as_ref() == "deref" => {
        self.check_ref_target(items.get(1), "deref", scope, near);
        for item in items.iter().skip(2) {
          self.walk(item, scope, near);
        }
      }
      Some(Calcit::Proc(CalcitProc::NativeTraitCall)) => {
        let trait_args: Vec<&Calcit> = items.iter().skip(1).collect();
        self.charge_trait_call_effects(&trait_args, scope, near);
        for item in items.iter().skip(1) {
          self.walk(item, scope, near);
        }
      }
      Some(Calcit::Method(method_name, MethodKind::Invoke(receiver_hint))) => {
        let call_args: Vec<&Calcit> = items.iter().skip(1).collect();
        let handled_receiver = self.charge_method_call_effects(method_name.as_ref(), receiver_hint, &call_args, scope, near);
        let skip_count = if handled_receiver { 2 } else { 1 };
        for item in items.iter().skip(skip_count) {
          self.walk(item, scope, near);
        }
      }
      Some(Calcit::Local(_)) => {
        let call_args: Vec<&Calcit> = items.iter().skip(1).collect();
        self.charge_call_effects(items.first(), &call_args, false, scope, near);
        for item in items.iter().skip(1) {
          self.walk(item, scope, near);
        }
      }
      head => {
        let call_args: Vec<&Calcit> = items.iter().skip(1).collect();
        self.charge_call_effects(head, &call_args, false, scope, near);
        for item in items.iter() {
          self.walk(item, scope, near);
        }
      }
    }
  }

  fn open_callback_slots_for_expr(&self, expr: &Calcit) -> Option<&'static [usize]> {
    match expr {
      Calcit::Local(local) => self.local_open_callback_slots.get(&(local.sym.clone(), local.idx)).copied(),
      other => {
        let slots = core_open_callback_slots(Some(other));
        (!slots.is_empty()).then_some(slots)
      }
    }
  }

  fn charge_call_effects(
    &mut self,
    head: Option<&Calcit>,
    args: &[&Calcit],
    from_spread: bool,
    scope: &Scope,
    near: Option<&NodeLocation>,
  ) {
    match head {
      Some(Calcit::Local(local)) => {
        self.check_local_call(local, scope, near);
      }
      // Calling an inline lambda uses the effects its own hint declares;
      // building one without calling it does not.
      Some(Calcit::List(callee)) if matches!(callee.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) => {
        for effect in lambda_declared_effects(callee) {
          self.require(
            scope,
            effect,
            &format!("calls an inline lambda that declares `:{}`", effect.feature()),
            near,
          );
        }
      }
      Some(Calcit::Proc(CalcitProc::NativeTraitCall)) if from_spread => {
        self.charge_trait_call_effects(args, scope, near);
        return;
      }
      Some(Calcit::Method(method_name, MethodKind::Invoke(receiver_hint))) if from_spread => {
        self.charge_method_call_effects(method_name.as_ref(), receiver_hint, args, scope, near);
        return;
      }
      Some(callee_expr @ Calcit::List(_)) => {
        let mut collected = Vec::new();
        self.collect_callback_effects(callee_expr, &HashMap::new(), &mut collected);
        for (effect, label) in collected {
          self.require(scope, effect, &format!("calls {label} that declares `:{}`", effect.feature()), near);
        }
      }
      _ => {}
    }
    self.charge_pass_slot_callbacks(head, args, scope, near);
  }

  /// A callback slot marked `:pass` (core `map`, `each`, `foldl`, …) calls
  /// the callback before the call returns, so the effects of a callback that
  /// declares them belong to this caller (#2726). Named definitions are
  /// already charged where they are referenced; this covers inline lambdas
  /// with their own hint and local `Fn` values whose type carries features.
  fn charge_pass_slot_callbacks(&mut self, head: Option<&Calcit>, args: &[&Calcit], scope: &Scope, near: Option<&NodeLocation>) {
    let signature = self.callee_signature(head);
    let open_callback_slots = head.and_then(|expr| self.open_callback_slots_for_expr(expr)).unwrap_or(&[]);
    self.charge_pass_signature_callbacks(signature.as_deref(), open_callback_slots, args, scope, near);
  }

  fn charge_pass_signature_callbacks(
    &mut self,
    signature: Option<&CalcitFnTypeAnnotation>,
    open_callback_slots: &[usize],
    args: &[&Calcit],
    scope: &Scope,
    near: Option<&NodeLocation>,
  ) {
    if signature.is_none() && open_callback_slots.is_empty() {
      return;
    }
    let empty_bindings = HashMap::new();
    for (idx, arg) in args.iter().enumerate() {
      if matches!(arg, Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
        break;
      }
      let slot = signature.and_then(|sig| sig.arg_types.get(idx).or(sig.rest_type.as_ref()));
      if !slot.is_some_and(|slot| is_pass_slot(slot)) && !open_callback_slots.contains(&idx) {
        continue;
      }
      let mut collected = Vec::new();
      self.collect_callback_effects(arg, &empty_bindings, &mut collected);
      for (effect, label) in collected {
        self.require(
          scope,
          effect,
          &format!(
            "passes {label}, which declares `:{}`, to a callback slot that runs it",
            effect.feature()
          ),
          near,
        );
      }
    }
  }

  fn local_schema_effects(&self, local: &CalcitLocal) -> HashSet<Effect> {
    let type_info = if matches!(local.type_info.as_ref(), CalcitTypeAnnotation::Dynamic) {
      self.scope_types.get(&local.sym).unwrap_or(&local.type_info)
    } else {
      &local.type_info
    };
    resolve_fn_in_ns(self.ns, type_info)
      .map(|signature| declared_effects(&signature.features))
      .unwrap_or_default()
  }

  fn collect_callback_effects(&self, expr: &Calcit, let_effects: &BoundCallbackEffects, out: &mut Vec<(Effect, String)>) {
    match expr {
      Calcit::List(lambda) if matches!(lambda.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) => {
        for effect in lambda_declared_effects(lambda) {
          out.push((effect, String::from("an inline lambda")));
        }
      }
      Calcit::Local(local) => {
        if let Some(bound) = let_effects.get(&(local.sym.clone(), local.idx)) {
          out.extend(bound.iter().cloned());
        } else {
          let label = format!("`{}`", local.sym);
          for effect in self.local_schema_effects(local) {
            out.push((effect, label.clone()));
          }
        }
      }
      Calcit::List(items) => match items.first() {
        Some(Calcit::Syntax(CalcitSyntax::If, _)) => {
          if let Some(then_branch) = items.get(2) {
            self.collect_callback_effects(then_branch, let_effects, out);
          }
          if let Some(else_branch) = items.get(3) {
            self.collect_callback_effects(else_branch, let_effects, out);
          }
        }
        Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) if items.len() >= 3 => {
          let mut inner = let_effects.clone();
          if let Some(Calcit::List(binding)) = items.get(1)
            && let (Some(Calcit::Local(local)), Some(init)) = (binding.first(), binding.get(1))
          {
            let mut bound = Vec::new();
            let label = format!("`{}`", local.sym);
            for effect in resolve_fn_in_ns(self.ns, &local.type_info)
              .map(|signature| declared_effects(&signature.features))
              .unwrap_or_default()
            {
              bound.push((effect, label.clone()));
            }
            self.collect_callback_effects(init, let_effects, &mut bound);
            inner.insert((local.sym.clone(), local.idx), bound);
          }
          if let Some(tail) = items.iter().last() {
            self.collect_callback_effects(tail, &inner, out);
          }
        }
        Some(Calcit::Import(import)) if import.ns.as_ref() == crate::calcit::CORE_NS && import.def.as_ref() == "do" => {
          if items.len() >= 2
            && let Some(tail) = items.iter().last()
          {
            self.collect_callback_effects(tail, let_effects, out);
          }
        }
        _ => {
          let call_head = if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::CallSpread, _))) {
            items.get(1)
          } else {
            items.first()
          };
          let result_type = super::type_inference::resolve_type_value(expr, &self.scope_types)
            .filter(|resolved| !matches!(resolved.as_ref(), CalcitTypeAnnotation::Dynamic))
            .or_else(|| self.callee_signature(call_head).map(|signature| signature.return_type.clone()));
          if let Some(result_type) = result_type
            && let Some(signature) = resolve_fn_in_ns(self.ns, &result_type)
          {
            for effect in declared_effects(&signature.features) {
              out.push((effect, String::from("a callback returned by a call")));
            }
          }
        }
      },
      _ => {}
    }
  }

  fn resolve_trait_for_call(&self, form: &Calcit) -> Option<Arc<CalcitTrait>> {
    match form {
      Calcit::Trait(trait_def) => Some(trait_def.clone()),
      Calcit::Import(import) => match program::lookup_runtime_ready(&import.ns, &import.def) {
        Some(Calcit::Trait(trait_def)) => Some(trait_def),
        Some(_) => None,
        None => match super::type_inference::resolve_program_value_for_preprocess(&import.ns, &import.def, import.def_id) {
          Some(Calcit::Trait(trait_def)) => Some(trait_def),
          Some(_) => None,
          None => super::lookup_source_backed_trait_def(&import.ns, &import.def)
            .map(|trait_def| Arc::new(trait_def.with_definition_ref(&import.ns, &import.def))),
        },
      },
      Calcit::Local(local) => self
        .scope_types
        .get(&local.sym)
        .and_then(|t| t.as_trait_definition())
        .or_else(|| local.type_info.as_trait_definition())
        .cloned()
        .map(Arc::new),
      _ => None,
    }
  }

  fn charge_trait_call_effects(&mut self, trait_args: &[&Calcit], scope: &Scope, near: Option<&NodeLocation>) {
    let Some(trait_arg) = trait_args.first().copied() else {
      return;
    };
    let Some(method_arg) = trait_args.get(1).copied() else {
      return;
    };
    let Some(trait_def) = self.resolve_trait_for_call(trait_arg) else {
      return;
    };
    let method_name = match method_arg {
      Calcit::Tag(name) => name.ref_str(),
      Calcit::Symbol { sym, .. } | Calcit::Str(sym) => sym.as_ref(),
      _ => return,
    };
    let Ok(reachable) = trait_def.normalized_reachable_traits() else {
      return;
    };
    for declaring in reachable {
      let Some(method_idx) = declaring.method_index(method_name) else {
        continue;
      };
      let Some(method_type) = declaring.method_types.get(method_idx) else {
        break;
      };
      let call_args: &[&Calcit] = if trait_args.len() >= 2 { &trait_args[2..] } else { &[] };
      self.charge_trait_declaring_effects(declaring.as_ref(), method_name, method_type, call_args, scope, near);
      break;
    }
  }

  fn charge_trait_declaring_effects(
    &mut self,
    declaring: &CalcitTrait,
    method_name: &str,
    method_type: &Arc<CalcitTypeAnnotation>,
    args: &[&Calcit],
    scope: &Scope,
    near: Option<&NodeLocation>,
  ) {
    if declaring.definition_ref.as_deref() == Some("calcit.core/FsPathOps")
      && matches!(method_name, "read-text" | "write-text!" | "read-dir" | "walk-dir")
    {
      self.require(scope, Effect::Io, &format!("uses FsPath .{method_name}"), near);
    }
    let resolved = super::resolve_trait_method_type(declaring, method_type.clone());
    let Some(signature) = resolved.resolve_to_nonoptional_fn() else {
      return;
    };
    for effect in declared_effects(&signature.features) {
      self.require(
        scope,
        effect,
        &format!(
          "calls trait method {} .{method_name}, which declares `:{}`",
          declaring.origin_label(),
          effect.feature()
        ),
        near,
      );
    }
    self.charge_pass_signature_callbacks(Some(signature.as_ref()), &[], args, scope, near);
  }

  fn charge_import_effects(&mut self, import: &CalcitImport, scope: &Scope, near: Option<&NodeLocation>) {
    let core = import.ns.as_ref() == crate::calcit::CORE_NS;
    let has_source = !core && program::has_def_code(&import.ns, &import.def);
    let declared = if core {
      core_def_effects(&import.def)
    } else {
      if scope.top
        && has_source
        && let Some(summary) = self.summary.as_mut()
      {
        summary.deps.insert((import.ns.clone(), import.def.clone()));
      }
      schema_effects(lookup_non_core_import_schema(&import.ns, &import.def).as_ref())
    };
    self.via_dependency = has_source;
    for effect in [Effect::State, Effect::Io] {
      if declared.contains(&effect) {
        self.require(
          scope,
          effect,
          &format!("uses {}/{}, which needs `:{}`", import.ns, import.def, effect.feature()),
          near,
        );
      }
    }
    self.via_dependency = false;
  }

  fn charge_resolved_entry(&mut self, entry: &Calcit, args: &[&Calcit], scope: &Scope, near: Option<&NodeLocation>) {
    match entry {
      Calcit::Proc(proc) => {
        if let Some(effect) = proc_effect(proc) {
          self.require(scope, effect, &format!("uses `{proc}`"), near);
        }
      }
      Calcit::Registered(name) => {
        if let Some(effect) = registered_effect(name) {
          self.require(scope, effect, &format!("uses `{name}`"), near);
        }
      }
      Calcit::Import(import) => {
        self.charge_import_effects(import, scope, near);
      }
      Calcit::Fn { info, .. } => {
        let core = info.def_ns.as_ref() == crate::calcit::CORE_NS;
        let declared = if core {
          core_def_effects(&info.name)
        } else {
          if scope.top
            && program::has_def_code(&info.def_ns, &info.name)
            && let Some(summary) = self.summary.as_mut()
          {
            summary.deps.insert((info.def_ns.clone(), info.name.clone()));
          }
          let mut effects = schema_effects(lookup_non_core_import_schema(&info.def_ns, &info.name).as_ref());
          if let Some(hint_sig) = body_hint_signature(info.body.iter()) {
            effects.extend(declared_effects(&hint_sig.features));
          }
          effects
        };
        self.via_dependency = !core && program::has_def_code(&info.def_ns, &info.name);
        for effect in [Effect::State, Effect::Io] {
          if declared.contains(&effect) {
            self.require(
              scope,
              effect,
              &format!("uses {}/{}, which needs `:{}`", info.def_ns, info.name, effect.feature()),
              near,
            );
          }
        }
        self.via_dependency = false;
      }
      _ => {}
    }
    self.charge_pass_slot_callbacks(Some(entry), args, scope, near);
  }

  fn charge_method_call_effects(
    &mut self,
    method_name: &str,
    receiver_hint: &Arc<CalcitTypeAnnotation>,
    args: &[&Calcit],
    scope: &Scope,
    near: Option<&NodeLocation>,
  ) -> bool {
    let receiver_type = args
      .first()
      .and_then(|receiver| super::type_inference::resolve_type_value(receiver, &self.scope_types))
      .filter(|resolved| !matches!(resolved.as_ref(), CalcitTypeAnnotation::Dynamic))
      .unwrap_or_else(|| receiver_hint.clone());
    if method_name == "deref" && matches!(receiver_type.as_ref(), CalcitTypeAnnotation::Ref(_)) {
      self.check_ref_target(args.first().copied(), ".deref", scope, near);
      return true;
    }
    if matches!(receiver_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
      return false;
    }
    if let Some(impls) = super::get_impls_from_type(receiver_type.as_ref()) {
      if let super::ImplMethodResolution::Selected(candidate) = super::resolve_impl_method(receiver_type.as_ref(), &impls, method_name)
      {
        self.charge_resolved_entry(candidate.entry, args, scope, near);
        if let Calcit::Fn { info, .. } = candidate.entry
          && info.def_ns.as_ref() != crate::calcit::CORE_NS
          && !program::has_def_code(&info.def_ns, &info.name)
          && body_hint_signature(info.body.iter()).is_none()
          && let Some(origin) = candidate.impl_value.origin()
          && let Some(idx) = origin.method_index(method_name)
          && let Some(method_type) = origin.method_types.get(idx)
        {
          self.charge_trait_declaring_effects(origin.as_ref(), method_name, method_type, args, scope, near);
        }
      }
      return false;
    }
    if let Some(traits) = super::trait_list_for_receiver(receiver_type.as_ref(), &self.scope_types)
      && let super::TraitMethodResolution::Selected(candidate) = super::resolve_trait_method(&traits, method_name)
    {
      self.charge_trait_declaring_effects(candidate.trait_def.as_ref(), method_name, &candidate.method_type, args, scope, near);
    }
    false
  }

  fn bind_match_pattern_locals(&mut self, pattern: &Calcit) {
    match pattern {
      Calcit::Local(local) => {
        self.shadow(&local.sym);
        self.scope_types.insert(local.sym.clone(), local.type_info.clone());
      }
      Calcit::List(parts) => {
        for part in parts.iter() {
          self.bind_match_pattern_locals(part);
        }
      }
      _ => {}
    }
  }

  /// Forget local Ref and HOF-slot provenance for every binding named `sym`.
  fn shadow(&mut self, sym: &Arc<str>) {
    self.local_refs.retain(|(name, _)| name != sym);
    self.local_open_callback_slots.retain(|(name, _), _| name != sym);
  }

  /// A nested function keeps the enclosing scope unless its own hint declares
  /// `:state` or `:io`; then its body is checked against that declaration.
  fn walk_lambda(&mut self, items: &crate::calcit::CalcitList, scope: &Scope, near: Option<&NodeLocation>) {
    // Parameters shadow outer local Refs and types of the same name inside the body.
    let saved_refs = self.local_refs.clone();
    let saved_slots = self.local_open_callback_slots.clone();
    let saved_types = self.scope_types.clone();
    if let Some(lambda_sig) = lambda_declared_signature(items) {
      self.scope_types.bind_function_generics(&lambda_sig, self.ns);
    }
    if let Some(Calcit::List(params)) = items.get(2) {
      for param in params.iter() {
        match param {
          Calcit::Local(local) => {
            self.shadow(&local.sym);
            self.scope_types.bind_parameter(local.sym.clone(), local.type_info.clone());
          }
          Calcit::Symbol { sym, .. } => {
            self.shadow(sym);
            self.scope_types.shadow_parameter(sym);
          }
          _ => {}
        }
      }
    }
    let own = lambda_declared_effects(items);
    let body = items.iter().skip(3);
    match Some(own).filter(|effects| !effects.is_empty()) {
      Some(declared) => {
        let name = match items.get(1) {
          Some(Calcit::Symbol { sym, .. }) => sym.to_string(),
          _ => String::from("fn"),
        };
        let inner = Scope {
          owner: format!("{} (lambda `{name}`)", scope.owner),
          declared,
          top: false,
        };
        for item in body {
          self.walk(item, &inner, near);
        }
      }
      None => {
        for item in body {
          self.walk(item, scope, near);
        }
      }
    }
    self.local_refs = saved_refs;
    self.local_open_callback_slots = saved_slots;
    self.scope_types = saved_types;
  }

  fn check_ref_target(&mut self, target: Option<&Calcit>, operation: &str, scope: &Scope, near: Option<&NodeLocation>) {
    match target {
      Some(Calcit::Local(local)) if self.local_refs.contains(&(local.sym.clone(), local.idx)) => {}
      Some(Calcit::Import(import)) => self.require(
        scope,
        Effect::State,
        &format!("uses `{operation}` on global Ref `{}/{}`", import.ns, import.def),
        near,
      ),
      Some(Calcit::Local(local)) => self.require(
        scope,
        Effect::State,
        &format!("uses `{operation}` on `{}`, a Ref not created in this function", local.sym),
        near,
      ),
      // A Ref built right here, e.g. `(deref (ref 0))`, is local to this function.
      Some(other) if is_ref_constructor(other) => self.walk(other, scope, near),
      Some(other) => {
        self.require(scope, Effect::State, &format!("uses `{operation}`"), near);
        self.walk(other, scope, near);
      }
      None => {}
    }
  }

  fn check_local_call(&mut self, local: &CalcitLocal, scope: &Scope, near: Option<&NodeLocation>) {
    for effect in self.local_schema_effects(local) {
      self.require(
        scope,
        effect,
        &format!("calls `{}` whose Fn type declares `:{}`", local.sym, effect.feature()),
        near,
      );
    }
  }

  fn require(&mut self, scope: &Scope, effect: Effect, trigger: &str, near: Option<&NodeLocation>) {
    if scope.top
      && !self.via_dependency
      && let Some(summary) = self.summary.as_mut()
    {
      match effect {
        Effect::State => summary.state = true,
        Effect::Io => summary.io = true,
      }
    }
    if scope.declared.contains(&effect) || !self.reported.insert((scope.owner.clone(), effect)) {
      return;
    }
    let policy = effect.policy();
    if policy == FeaturePolicy::Allow {
      return;
    }
    let feature = effect.feature();
    let (warning, error) = effect.codes();
    let (label, code) = if policy == FeaturePolicy::Error {
      ("[Error]", error)
    } else {
      ("[Warn]", warning)
    };
    let alternative = match effect {
      Effect::State => "pass the value in as an argument",
      Effect::Io => "use `dbg` for temporary debug output",
    };
    let message = format!(
      "{label} {} {trigger} but its Fn schema does not declare `:{feature}`; {alternative}, or add the feature: calcit edit schema '{}/{}' --add-feature {feature}",
      scope.owner, self.ns, self.def
    );
    let location = near.cloned().unwrap_or_else(|| self.fallback_location());
    self.advisories.push(LocatedWarning::new_with_detail(
      message,
      location,
      Some(code.to_owned()),
      Some(format!(
        "`:state` / `:io` propagate to callers; `calcit fix --rule effect-features-v1` adds every missing one at once; set `:feature-policy` `:{feature}` on the entry to control this check"
      )),
    ));
  }

  fn note_dbg(&mut self, near: Option<&NodeLocation>) {
    let policy = program::configured_feature_policy(DBG_POLICY).unwrap_or(FeaturePolicy::Allow);
    if policy == FeaturePolicy::Allow || self.dbg_reported {
      return;
    }
    self.dbg_reported = true;
    let (label, code) = if policy == FeaturePolicy::Error {
      ("[Error]", DBG_IN_ENTRY_ERROR)
    } else {
      ("[Warn]", DBG_IN_ENTRY_WARNING)
    };
    let location = near.cloned().unwrap_or_else(|| self.fallback_location());
    self.advisories.push(LocatedWarning::new_with_detail(
      format!(
        "{label} {}/{} uses `dbg`, which the active entry's `:feature-policy` `:dbg` does not allow; remove the debug output before release",
        self.ns, self.def
      ),
      location,
      Some(code.to_owned()),
      None,
    ));
  }
}

/// Effects of a core function, read from its source: core wrappers such as
/// `read-file`, `get-env` or `deref` are thin layers over effect procs and do
/// not all declare features in their schemas. Macros are skipped, their
/// expansions are checked where they are used.
fn core_def_effects(def: &str) -> HashSet<Effect> {
  static CACHE: LazyLock<Mutex<HashMap<Arc<str>, HashSet<Effect>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
  if let Some(found) = CACHE.lock().expect("core effects cache").get(def) {
    return found.clone();
  }
  let mut visiting = HashSet::new();
  let found = scan_core_def(def, &mut visiting);
  CACHE.lock().expect("core effects cache").insert(Arc::from(def), found.clone());
  found
}

fn scan_core_def(def: &str, visiting: &mut HashSet<String>) -> HashSet<Effect> {
  let mut found = HashSet::new();
  if !visiting.insert(def.to_owned()) {
    return found;
  }
  if let CalcitTypeAnnotation::Fn(signature) = program::lookup_def_schema(crate::calcit::CORE_NS, def).as_ref() {
    found.extend(declared_effects(&signature.features));
  }
  let Some(code) = program::lookup_def_code(crate::calcit::CORE_NS, def) else {
    return found;
  };
  // Only functions and plain aliases (`read-file` is `&read-file`) carry
  // effects; macros expand where they are used, and values such as structs or
  // impl tables only hold method references.
  if matches!(&code, Calcit::List(items) if !matches!(items.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "defn")) {
    return found;
  }
  scan_core_code(&code, &mut found, visiting);
  found
}

fn scan_core_code(code: &Calcit, found: &mut HashSet<Effect>, visiting: &mut HashSet<String>) {
  match code {
    Calcit::Symbol { sym, .. } => {
      if let Some(proc) = CalcitProc::parse_source_proc(sym) {
        found.extend(proc_effect(&proc));
      } else if let Some(effect) = registered_effect(sym) {
        found.insert(effect);
      } else if sym.as_ref() == "reset!" {
        found.insert(Effect::State);
      } else if program::has_def_code(crate::calcit::CORE_NS, sym) {
        found.extend(scan_core_def(sym, visiting));
      }
    }
    Calcit::Proc(proc) => found.extend(proc_effect(proc)),
    Calcit::Registered(name) => found.extend(registered_effect(name)),
    Calcit::Syntax(CalcitSyntax::Reset, _) => {
      found.insert(Effect::State);
    }
    Calcit::List(items) => {
      if matches!(
        items.first(),
        Some(Calcit::Syntax(
          CalcitSyntax::Quote | CalcitSyntax::HintFn | CalcitSyntax::AssertType,
          _
        ))
      ) || matches!(items.first(), Some(Calcit::Symbol { sym, .. }) if matches!(sym.as_ref(), "quote" | "hint-fn" | "assert-type"))
      {
        return;
      }
      for item in items.iter() {
        scan_core_code(item, found, visiting);
      }
    }
    _ => {}
  }
}

/// The schema of a core function called at `head`, when it has one.
/// Core functions whose callback is declared `Dynamic` but called before
/// they return; type checking narrows these slots to a callback with the
/// same pass-through treatment (`carry_callback_slot_features`).
fn core_open_callback_slots(head: Option<&Calcit>) -> &'static [usize] {
  let name = match head {
    Some(Calcit::Import(import)) if import.ns.as_ref() == crate::calcit::CORE_NS => import.def.as_ref(),
    Some(Calcit::Proc(proc)) => proc.as_ref(),
    Some(Calcit::Fn { info, .. }) if info.def_ns.as_ref() == crate::calcit::CORE_NS => info.name.as_ref(),
    _ => return &[],
  };
  match name {
    "&list:sort-by" => &[1],
    _ => &[],
  }
}

impl Checker<'_> {
  /// The declared signature of the callable at `head`: a core function or proc,
  /// project definition, local binding, inline lambda, or direct `Fn`.
  fn callee_signature(&self, head: Option<&Calcit>) -> Option<Arc<CalcitFnTypeAnnotation>> {
    let def = match head? {
      Calcit::Import(import) if import.ns.as_ref() != crate::calcit::CORE_NS => {
        return lookup_non_core_import_schema(&import.ns, &import.def).resolve_to_fn();
      }
      Calcit::Import(import) => import.def.to_string(),
      Calcit::Local(local) => {
        let type_info = if matches!(local.type_info.as_ref(), CalcitTypeAnnotation::Dynamic) {
          self.scope_types.get(&local.sym).unwrap_or(&local.type_info)
        } else {
          &local.type_info
        };
        return resolve_fn_in_ns(self.ns, type_info);
      }
      Calcit::List(lambda) if matches!(lambda.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) => {
        return lambda_declared_signature(lambda);
      }
      Calcit::List(items) => {
        if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::CoreLet, _))) && items.len() >= 3 {
          let expr = head.expect("matched Some");
          return super::type_inference::resolve_type_value(expr, &self.scope_types)
            .filter(|ty| !matches!(ty.as_ref(), CalcitTypeAnnotation::Dynamic))
            .and_then(|ty| resolve_fn_in_ns(self.ns, &ty));
        }
        if let Some(Calcit::Import(import)) = items.first()
          && import.ns.as_ref() == crate::calcit::CORE_NS
          && import.def.as_ref() == "do"
        {
          let tail = (items.len() >= 2).then(|| &items[items.len() - 1]);
          return tail.and_then(|expr| {
            super::type_inference::resolve_type_value(expr, &self.scope_types)
              .filter(|ty| !matches!(ty.as_ref(), CalcitTypeAnnotation::Dynamic))
              .and_then(|ty| resolve_fn_in_ns(self.ns, &ty))
              .or_else(|| self.callee_signature(Some(expr)))
          });
        }
        let expr = head.expect("matched Some");
        if let Some(ty) = super::type_inference::resolve_type_value(expr, &self.scope_types)
          .filter(|ty| !matches!(ty.as_ref(), CalcitTypeAnnotation::Dynamic))
          && let Some(sig) = resolve_fn_in_ns(self.ns, &ty)
        {
          return Some(sig);
        }
        let call_head = if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::CallSpread, _))) {
          items.get(1)
        } else {
          items.first()
        };
        let return_type = self.callee_signature(call_head)?.return_type.clone();
        return resolve_fn_in_ns(self.ns, &return_type);
      }
      Calcit::Fn { info, .. } => {
        return if info.def_ns.as_ref() == crate::calcit::CORE_NS {
          program::lookup_def_schema(crate::calcit::CORE_NS, &info.name).resolve_to_fn()
        } else {
          lookup_non_core_import_schema(&info.def_ns, &info.name)
            .resolve_to_fn()
            .or_else(|| body_hint_signature(info.body.iter()))
            .or_else(|| CalcitTypeAnnotation::from_calcit_fn(info).resolve_to_fn())
        };
      }
      Calcit::Proc(proc) => {
        if let Some(found) = program::lookup_def_schema(crate::calcit::CORE_NS, proc.as_ref()).resolve_to_fn() {
          return Some(found);
        }
        let contract = proc.get_type_signature()?;
        return CalcitTypeAnnotation::from_proc_parts(contract.arg_types.clone(), contract.return_type.clone(), proc.arity())
          .resolve_to_fn();
      }
      _ => return None,
    };
    program::lookup_def_schema(crate::calcit::CORE_NS, &def).resolve_to_fn()
  }
}

/// Core callback slots that run the callback before returning: marked
/// `:pass`, or a bare `Fn` (`any?`, `filter`, `every?`, ...).
fn is_pass_slot(slot: &CalcitTypeAnnotation) -> bool {
  matches!(slot, CalcitTypeAnnotation::DynFn)
    || slot.resolve_to_fn().is_some_and(|signature| {
      signature
        .features
        .iter()
        .any(|feature| feature.ref_str() == crate::calcit::PASS_EFFECTS_FEATURE)
    })
}

fn body_hint_signature<'a>(mut body: impl Iterator<Item = &'a Calcit>) -> Option<Arc<CalcitFnTypeAnnotation>> {
  body.find_map(|item| match item {
    Calcit::List(hint) => match hint.call_kind() {
      CalcitCallKind::TypedFnHint(signature, _) => Some(signature.clone()),
      _ => None,
    },
    _ => None,
  })
}

fn lambda_declared_signature(items: &crate::calcit::CalcitList) -> Option<Arc<CalcitFnTypeAnnotation>> {
  body_hint_signature(items.iter().skip(3))
}

/// Effects declared by a nested `defn`'s own `hint-fn`, if any.
fn lambda_declared_effects(items: &crate::calcit::CalcitList) -> HashSet<Effect> {
  lambda_declared_signature(items)
    .map(|signature| declared_effects(&signature.features))
    .unwrap_or_default()
}

fn is_ref_constructor(form: &Calcit) -> bool {
  matches!(form, Calcit::List(items) if matches!(items.first(), Some(Calcit::Proc(CalcitProc::Ref))))
}

/// Blocking codes among this module's diagnostics.
pub(crate) fn is_blocking_code(code: &str) -> bool {
  matches!(code, STATE_FEATURE_ERROR | IO_FEATURE_ERROR | DBG_IN_ENTRY_ERROR)
}
