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
  Calcit, CalcitCallKind, CalcitLocal, CalcitProc, CalcitSyntax, CalcitTypeAnnotation, LocatedWarning, NodeLocation,
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

fn schema_effects(schema: &CalcitTypeAnnotation) -> HashSet<Effect> {
  match schema {
    CalcitTypeAnnotation::Fn(signature) => declared_effects(&signature.features),
    _ => HashSet::new(),
  }
}

/// One function scope whose declared features are checked.
struct Scope {
  owner: String,
  declared: HashSet<Effect>,
  /// The definition's own scope, as opposed to a lambda with its own hint.
  top: bool,
}

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
  let mut checker = Checker {
    ns,
    def,
    local_refs: HashSet::new(),
    reported: HashSet::new(),
    dbg_reported: false,
    failure_report: false,
    advisories: vec![],
    summary: record.then(EffectSummary::default),
    via_dependency: false,
  };
  // Only a structured `Fn` schema can declare features; eval snippets and
  // untyped helpers have none and are not checked.
  let schema = program::lookup_def_schema(ns, def);
  let CalcitTypeAnnotation::Fn(signature) = schema.as_ref() else {
    return vec![];
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
        let core = import.ns.as_ref() == crate::calcit::CORE_NS;
        let declared = if core {
          core_def_effects(&import.def)
        } else {
          if scope.top
            && let Some(summary) = self.summary.as_mut()
          {
            summary.deps.insert((import.ns.clone(), import.def.clone()));
          }
          schema_effects(program::lookup_def_schema(&import.ns, &import.def).as_ref())
        };
        self.via_dependency = !core;
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
        // A local Ref is known only inside its own binding: restore the
        // previous provenance once the body ends, so an outer binding of the
        // same name is not exempted by an inner `(ref ...)`.
        let saved_refs = self.local_refs.clone();
        if let Some(Calcit::List(binding)) = items.get(1)
          && let (Some(Calcit::Local(local)), Some(init)) = (binding.first(), binding.get(1))
        {
          self.walk(init, scope, near);
          self.shadow(&local.sym);
          if is_ref_constructor(init) {
            self.local_refs.insert((local.sym.clone(), local.idx));
          }
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
      }
      Some(Calcit::Syntax(CalcitSyntax::Reset, _)) => {
        self.check_ref_target(items.get(1), "reset!", scope, near);
        for item in items.iter().skip(2) {
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
      Some(Calcit::Local(local)) => {
        self.check_local_call(local, scope, near);
        for item in items.iter().skip(1) {
          self.walk(item, scope, near);
        }
      }
      head => {
        // Calling an inline lambda uses the effects its own hint declares;
        // building one without calling it does not.
        if let Some(Calcit::List(callee)) = head
          && matches!(callee.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _)))
        {
          for effect in lambda_declared_effects(callee) {
            self.require(
              scope,
              effect,
              &format!("calls an inline lambda that declares `:{}`", effect.feature()),
              near,
            );
          }
        }
        self.charge_pass_slot_callbacks(head, items, scope, near);
        for item in items.iter() {
          self.walk(item, scope, near);
        }
      }
    }
  }

  /// A callback slot marked `:pass` (core `map`, `each`, `foldl`, …) calls
  /// the callback before the call returns, so the effects of a callback that
  /// declares them belong to this caller (#2726). Named definitions are
  /// already charged where they are referenced; this covers inline lambdas
  /// with their own hint and local `Fn` values whose type carries features.
  fn charge_pass_slot_callbacks(
    &mut self,
    head: Option<&Calcit>,
    items: &crate::calcit::CalcitList,
    scope: &Scope,
    near: Option<&NodeLocation>,
  ) {
    let Some(signature) = callee_signature(head) else {
      return;
    };
    let open_callback_slots = core_open_callback_slots(head);
    for (idx, arg) in items.iter().skip(1).enumerate() {
      let slot = signature.arg_types.get(idx).or(signature.rest_type.as_ref());
      if !slot.is_some_and(|slot| is_pass_slot(slot)) && !open_callback_slots.contains(&idx) {
        continue;
      }
      let (effects, label) = match arg {
        Calcit::List(lambda) if matches!(lambda.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) => {
          (lambda_declared_effects(lambda), String::from("an inline lambda"))
        }
        Calcit::Local(local) => (schema_effects(local.type_info.as_ref()), format!("`{}`", local.sym)),
        _ => continue,
      };
      for effect in effects {
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

  /// Forget local Ref provenance for every binding named `sym`.
  fn shadow(&mut self, sym: &Arc<str>) {
    self.local_refs.retain(|(name, _)| name != sym);
  }

  /// A nested function keeps the enclosing scope unless its own hint declares
  /// `:state` or `:io`; then its body is checked against that declaration.
  fn walk_lambda(&mut self, items: &crate::calcit::CalcitList, scope: &Scope, near: Option<&NodeLocation>) {
    // Parameters shadow outer local Refs of the same name inside the body.
    let saved_refs = self.local_refs.clone();
    if let Some(Calcit::List(params)) = items.get(2) {
      for param in params.iter() {
        match param {
          Calcit::Local(local) => self.shadow(&local.sym),
          Calcit::Symbol { sym, .. } => self.shadow(sym),
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
    for effect in schema_effects(local.type_info.as_ref()) {
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
      if let Ok(proc) = sym.parse::<CalcitProc>() {
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
    _ => return &[],
  };
  match name {
    "&list:sort-by" => &[1],
    _ => &[],
  }
}

/// The declared signature of the function called at `head`: a core
/// function or proc, or a project definition (its own `:pass` slots count too).
fn callee_signature(head: Option<&Calcit>) -> Option<Arc<crate::calcit::CalcitFnTypeAnnotation>> {
  let def = match head? {
    Calcit::Import(import) if import.ns.as_ref() != crate::calcit::CORE_NS => {
      return program::lookup_def_schema(&import.ns, &import.def).resolve_to_fn();
    }
    Calcit::Import(import) => import.def.to_string(),
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

/// Effects declared by a nested `defn`'s own `hint-fn`, if any.
fn lambda_declared_effects(items: &crate::calcit::CalcitList) -> HashSet<Effect> {
  items
    .iter()
    .skip(3)
    .find_map(|item| match item {
      Calcit::List(hint) => match hint.call_kind() {
        CalcitCallKind::TypedFnHint(signature, _) => Some(declared_effects(&signature.features)),
        _ => None,
      },
      _ => None,
    })
    .unwrap_or_default()
}

fn is_ref_constructor(form: &Calcit) -> bool {
  matches!(form, Calcit::List(items) if matches!(items.first(), Some(Calcit::Proc(CalcitProc::Ref))))
}

/// Blocking codes among this module's diagnostics.
pub(crate) fn is_blocking_code(code: &str) -> bool {
  matches!(code, STATE_FEATURE_ERROR | IO_FEATURE_ERROR | DBG_IN_ENTRY_ERROR)
}
