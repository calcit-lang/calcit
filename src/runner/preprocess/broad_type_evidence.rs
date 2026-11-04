//! Candidate concrete types for bare `Enum` / `Struct` schema slots
//! (`W_BROAD_TYPE`, #2566), including those inside a callback parameter such
//! as `Fn Enum (-> Unit)`.
//!
//! While call sites are checked, the actual type passed into a broad slot of a
//! project function is recorded under that function and the slot's path
//! (`arg 1`, or `arg 1 → arg 1` for a callback parameter). After the check,
//! a slot whose calls all pass one concrete type names it in the note's hint;
//! calls that pass several concrete types are listed. A slot with no checked
//! call, or with a call whose type is itself broad or unknown, keeps the
//! original note: only a complete, unambiguous set of calls names a type.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, LazyLock, Mutex};

use crate::calcit::{CalcitTypeAnnotation, LocatedWarning};
use crate::snapshot::SchemaSlotStep;

pub(super) type SlotKey = (Arc<str>, Arc<str>, Vec<SchemaSlotStep>);

/// A broad-type note waiting for the check to supply call-site evidence.
struct PendingBroadType {
  advisory: LocatedWarning,
  slot: Option<SlotKey>,
}

static PENDING: LazyLock<Mutex<Vec<PendingBroadType>>> = LazyLock::new(|| Mutex::new(vec![]));

/// Per slot, the display of each concrete type passed, or `None` for a call
/// whose type proves no particular definition.
static EVIDENCE: LazyLock<Mutex<HashMap<SlotKey, BTreeSet<Option<String>>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

thread_local! {
  /// The function and slot prefix whose parameters the current call fills.
  static CALL_OWNER: RefCell<Option<SlotKey>> = const { RefCell::new(None) };
}

/// Queues a `W_BROAD_TYPE` note. `slot` names the function definition and
/// the path of the broad type among its parameters and results.
pub fn push_broad_type_advisory(advisory: LocatedWarning, slot: Option<(&str, &str, &[SchemaSlotStep])>) {
  let slot = slot.map(|(ns, def, path)| (Arc::from(ns), Arc::from(def), path.to_vec()));
  PENDING
    .lock()
    .expect("lock broad type notes")
    .push(PendingBroadType { advisory, slot });
}

/// Restores the previous call owner when a checked call ends.
pub(super) struct CallOwnerScope {
  previous: Option<SlotKey>,
}

impl CallOwnerScope {
  pub(super) fn enter(ns: &str, def: &str, prefix: Vec<SchemaSlotStep>) -> Option<Self> {
    if !super::should_emit_project_source_lint(ns) {
      return None;
    }
    let owner = (Arc::from(ns), Arc::from(def), prefix);
    Some(Self {
      previous: CALL_OWNER.with(|cell| cell.borrow_mut().replace(owner)),
    })
  }
}

impl Drop for CallOwnerScope {
  fn drop(&mut self) {
    CALL_OWNER.with(|cell| *cell.borrow_mut() = self.previous.take());
  }
}

/// Claims the owner set for the call about to be checked, so that calls
/// checked while inferring its arguments do not record under it.
pub(super) fn take_call_owner() -> Option<SlotKey> {
  CALL_OWNER.with(|cell| cell.borrow_mut().take())
}

/// The position of `param` among the parameters of the `defn` `ns/def`.
pub(super) fn defn_param_index(ns: &str, def: &str, param: &str) -> Option<usize> {
  use crate::calcit::Calcit;
  let Calcit::List(items) = crate::program::lookup_def_code(ns, def)? else {
    return None;
  };
  if !matches!(items.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "defn") {
    return None;
  }
  let Some(Calcit::List(params)) = items.get(2) else { return None };
  let names = params
    .iter()
    .map(|param| match param {
      Calcit::Symbol { sym, .. } => Some(sym.clone()),
      Calcit::Local(local) => Some(local.sym.clone()),
      _ => None,
    })
    .collect::<Option<Vec<_>>>()?;
  // A rest or optional marker shifts positions; leave those signatures alone.
  if names.iter().any(|name| matches!(name.as_ref(), "&" | "?")) {
    return None;
  }
  names.iter().position(|name| name.as_ref() == param)
}

/// Records what argument `index` of a call passes into each broad part of
/// its expected type, under the owner claimed for that call.
pub(super) fn record_argument(owner: Option<&SlotKey>, index: usize, expected: &CalcitTypeAnnotation, actual: &CalcitTypeAnnotation) {
  let Some((ns, def, path)) = owner else {
    return;
  };
  let mut path = path.clone();
  path.push(SchemaSlotStep::Arg(index));
  let mut found = vec![];
  collect_slot_evidence(expected, actual, &mut path, &mut found, false);
  if found.is_empty() {
    return;
  }
  let mut evidence = EVIDENCE.lock().expect("lock broad type evidence");
  for (path, candidate) in found {
    evidence.entry((ns.clone(), def.clone(), path)).or_default().insert(candidate);
  }
}

fn collect_slot_evidence(
  expected: &CalcitTypeAnnotation,
  actual: &CalcitTypeAnnotation,
  path: &mut Vec<SchemaSlotStep>,
  found: &mut Vec<(Vec<SchemaSlotStep>, Option<String>)>,
  accepted_by_actual: bool,
) {
  if let Some(family) = broad_family(expected) {
    // A passed function that itself accepts the whole family fits any
    // candidate, so it neither names one nor rules one out.
    if accepted_by_actual && broad_family(actual) == Some(family) {
      return;
    }
    found.push((path.clone(), concrete_candidate(family, actual)));
    return;
  }
  // A callback parameter: what the passed function accepts and returns.
  if let (CalcitTypeAnnotation::Fn(expected), CalcitTypeAnnotation::Fn(actual)) = (expected, actual) {
    for (index, (expected_arg, actual_arg)) in expected.arg_types.iter().zip(actual.arg_types.iter()).enumerate() {
      path.push(SchemaSlotStep::Arg(index));
      collect_slot_evidence(expected_arg, actual_arg, path, found, !accepted_by_actual);
      path.pop();
    }
    path.push(SchemaSlotStep::Return);
    collect_slot_evidence(&expected.return_type, &actual.return_type, path, found, accepted_by_actual);
    path.pop();
  }
}

/// Families whose members are named definitions. A bare `Fn` slot is left
/// out: one caller's full signature is often narrower than the slot means.
fn broad_family(annotation: &CalcitTypeAnnotation) -> Option<&'static str> {
  match annotation {
    CalcitTypeAnnotation::AnonymousEnum => Some("Enum"),
    CalcitTypeAnnotation::Custom(value) => match value.as_ref() {
      crate::calcit::Calcit::Tag(tag) if matches!(tag.ref_str(), "struct" | "record") => Some("Struct"),
      _ => None,
    },
    _ => None,
  }
}

/// The concrete type an actual value proves for a broad family, written as
/// a schema would name it.
fn concrete_candidate(family: &str, actual: &CalcitTypeAnnotation) -> Option<String> {
  let named = |definition: Option<(Arc<str>, Arc<str>)>, args: &[Arc<CalcitTypeAnnotation>]| -> Option<String> {
    if !args.is_empty() {
      return None;
    }
    let (ns, def) = definition?;
    Some(if ns.as_ref() == crate::calcit::CORE_NS {
      def.to_string()
    } else {
      format!("{ns}/{def}")
    })
  };
  match (family, actual) {
    ("Enum", CalcitTypeAnnotation::Enum(_, args)) => named(actual.resolve_to_enum_with_ref()?.1, args),
    ("Enum", CalcitTypeAnnotation::EnumValue(_) | CalcitTypeAnnotation::TypeRef(_, _)) => {
      named(actual.resolve_to_enum_with_ref()?.1, type_ref_args(actual))
    }
    ("Struct", CalcitTypeAnnotation::Struct(_, args)) => named(actual.resolve_to_struct_with_ref()?.1, args),
    ("Struct", CalcitTypeAnnotation::StructValue(_) | CalcitTypeAnnotation::TypeRef(_, _)) => {
      named(actual.resolve_to_struct_with_ref()?.1, type_ref_args(actual))
    }
    _ => None,
  }
}

fn type_ref_args(annotation: &CalcitTypeAnnotation) -> &[Arc<CalcitTypeAnnotation>] {
  match annotation {
    CalcitTypeAnnotation::TypeRef(_, args) => args.as_slice(),
    _ => &[],
  }
}

/// The queued notes, each told what its checked calls pass. Evidence is
/// cleared with them, so a later check starts afresh.
pub(super) fn take_broad_type_advisories() -> Vec<LocatedWarning> {
  let pending = std::mem::take(&mut *PENDING.lock().expect("lock broad type notes"));
  let evidence = std::mem::take(&mut *EVIDENCE.lock().expect("lock broad type evidence"));
  pending
    .into_iter()
    .map(|PendingBroadType { advisory, slot }| {
      let Some(slot) = slot else { return advisory };
      let Some(candidates) = evidence.get(&slot) else {
        return advisory;
      };
      // A call that proves no particular definition leaves the slot open.
      if candidates.contains(&None) {
        return advisory;
      }
      let names = candidates.iter().flatten().map(|name| format!("`{name}`")).collect::<Vec<_>>();
      let place = slot.2.iter().map(SchemaSlotStep::to_string).collect::<Vec<_>>().join(" → ");
      let (extra, hint) = match names.as_slice() {
        [] => return advisory,
        [name] => (
          format!(" Every call this check reached passes {name} at {place}."),
          format!(
            "write {name} at {place} of `{}/{}`, then run the definition `:tests` too: calls the check does not reach are not counted",
            slot.0, slot.1
          ),
        ),
        _ => (
          format!(" Calls this check reached pass {} at {place}.", names.join(", ")),
          format!(
            "calls pass {} at {place} of `{}/{}`; declare a union of them or a `:generics` type variable",
            names.join(", "),
            slot.0,
            slot.1
          ),
        ),
      };
      let mut advisory = advisory;
      advisory.append_message(&extra);
      advisory.set_hint(hint);
      advisory
    })
    .collect()
}
