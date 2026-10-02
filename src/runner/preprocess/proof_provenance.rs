//! Bounded explanations for failed proof relations, never additional type evidence.
use super::*;
use crate::calcit::CalcitErrProvenance;

type BindingKey = (Arc<str>, Arc<str>, Arc<str>);
type Evidence = Vec<CalcitErrProvenance>;

thread_local! {
  static BINDINGS: RefCell<HashMap<BindingKey, Evidence>> = RefCell::new(HashMap::new());
}

/// Restore lexical bindings on success, rejection and unwinding.
pub(super) struct BindingScope(Vec<(BindingKey, Option<Evidence>)>);

impl BindingScope {
  pub(super) fn new() -> Self {
    Self(Vec::new())
  }

  pub(super) fn bind(&mut self, local: &Calcit, value: Option<&Calcit>, types: &ScopeTypes) {
    if !REQUIRE_ASSERTION_PROOF.with(Cell::get) {
      return;
    }
    let Calcit::Local(local) = local else {
      return;
    };
    let key = (local.info.at_ns.clone(), local.info.at_def.clone(), local.sym.clone());
    let node = Calcit::Local(local.clone());
    let mut evidence = explain(&node, types, false);
    if let Some(value) = value {
      evidence.extend(explain(value, types, true));
      evidence.truncate(16);
    }
    self.install(key, evidence);
  }

  fn install(&mut self, key: BindingKey, evidence: Evidence) {
    let previous = BINDINGS.with(|bindings| bindings.borrow_mut().insert(key.clone(), evidence));
    self.0.push((key, previous));
  }
}

impl Drop for BindingScope {
  fn drop(&mut self) {
    BINDINGS.with(|bindings| {
      let mut bindings = bindings.borrow_mut();
      for (key, previous) in self.0.drain(..).rev() {
        if let Some(previous) = previous {
          bindings.insert(key, previous);
        } else {
          bindings.remove(&key);
        }
      }
    });
  }
}

fn item(kind: &str, operation: String, location: Option<NodeLocation>, actual: String, flow: &str) -> CalcitErrProvenance {
  CalcitErrProvenance {
    kind: kind.to_owned(),
    operation,
    definition: location.as_ref().map(|location| format!("{}/{}", location.ns, location.def)),
    path: location.map(|location| {
      if location.coord.is_empty() {
        "code".to_owned()
      } else {
        format!("code@{}", location.coord.iter().map(u16::to_string).collect::<Vec<_>>().join("."))
      }
    }),
    r#type: actual.clone(),
    output_type: actual,
    flow: flow.to_owned(),
    migration: "Inspect the source contract; choose checked decoding at the real boundary, not another assertion or an unsafe cast."
      .to_owned(),
  }
}

fn explain(value: &Calcit, types: &ScopeTypes, follow_binding: bool) -> Evidence {
  let mut result = Vec::new();
  walk(value, types, follow_binding, 0, &mut HashSet::new(), &mut result);
  result
}

fn walk(
  value: &Calcit,
  types: &ScopeTypes,
  follow_binding: bool,
  depth: usize,
  seen: &mut HashSet<(Arc<str>, Arc<str>)>,
  out: &mut Evidence,
) {
  if out.len() >= 16 || depth >= 8 {
    return;
  }
  let actual = resolve_type_value(value, types)
    .map(|ty| ty.to_brief_string())
    .unwrap_or_else(|| "unknown".to_owned());
  match value {
    Calcit::Local(local) => {
      if follow_binding
        && let Some(evidence) = BINDINGS.with(|bindings| {
          bindings
            .borrow()
            .get(&(local.info.at_ns.clone(), local.info.at_def.clone(), local.sym.clone()))
            .cloned()
        })
      {
        out.extend(evidence.into_iter().take(16 - out.len()));
      } else {
        out.push(item(
          "source-binding",
          local.sym.to_string(),
          local.location.as_ref().and_then(|_| value.get_location()),
          actual,
          "Lexical binding before the failing proof relation; its annotation is not a runtime validation.",
        ));
      }
    }
    Calcit::List(forms) => {
      let Some(head) = forms.first() else {
        return;
      };
      if matches!(head, Calcit::Syntax(CalcitSyntax::CoreLet, _)) {
        let mut scope = types.clone();
        let mut binding = BindingScope::new();
        if let Some(Calcit::List(pair)) = forms.get(1)
          && let (Some(Calcit::Local(local)), Some(value)) = (pair.first(), pair.get(1))
        {
          scope.insert(
            local.sym.clone(),
            resolve_type_value(value, types).unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone()),
          );
          let mut evidence = vec![item(
            "source-binding",
            local.sym.to_string(),
            local.location.as_ref().and_then(|_| Calcit::Local(local.clone()).get_location()),
            scope[&local.sym].to_brief_string(),
            "Lexical initializer feeding the returned binding; not a runtime validation.",
          )];
          // Keep the same recursion budget and visited callees while explaining
          // nested lets; restarting a trace here would recurse forever on helpers.
          walk(value, types, true, depth + 1, seen, &mut evidence);
          binding.install((local.info.at_ns.clone(), local.info.at_def.clone(), local.sym.clone()), evidence);
        }
        if let Some(tail) = forms.iter().last() {
          walk(tail, &scope, true, depth + 1, seen, out);
        }
        return;
      }
      if let Calcit::Import(import) = head {
        let key = (import.ns.clone(), import.def.clone());
        let location = program::lookup_def_code(&import.ns, &import.def)
          .map(|_| NodeLocation::new(import.ns.clone(), import.def.clone(), Arc::new(vec![])));
        out.push(item(
          "resolved-producer",
          format!("{}/{}", import.ns, import.def),
          location,
          actual,
          "Compiler-resolved producer contract; a declared return type does not validate an open input.",
        ));
        if seen.insert(key.clone()) {
          if let Some(Calcit::Fn { info: function, .. }) = program::lookup_runtime_ready(&import.ns, &import.def) {
            let params = match function.args.as_ref() {
              CalcitFnArgs::Args(params) => params.clone(),
              CalcitFnArgs::MarkedArgs(params) => params
                .iter()
                .filter_map(|param| match param {
                  CalcitArgLabel::Idx(index) => Some(*index),
                  _ => None,
                })
                .collect(),
            };
            let body_types = params
              .iter()
              .zip(&function.arg_types)
              .map(|(index, ty)| (Arc::from(CalcitLocal::read_name(*index)), ty.clone()))
              .collect();
            if let Some(tail) = function
              .body
              .iter()
              .rev()
              .find(|form| !builtins::syntax::is_function_metadata_hint(form))
            {
              walk(tail, &body_types, true, depth + 1, seen, out);
            }
          }
          seen.remove(&key);
        }
      } else {
        out.push(item(
          "typed-operation",
          match head {
            Calcit::Fn { info: function, .. } => format!("{}/{}", function.def_ns, function.name),
            _ => head.to_string(),
          },
          derive_list_call_expr_location(forms),
          actual,
          "Processed expression feeding the failing relation; argument paths are possible contributors, not a selected runtime branch.",
        ));
      }
      for argument in forms.iter().skip(1) {
        walk(argument, types, true, depth + 1, seen, out);
      }
    }
    _ => {}
  }
}

pub(super) fn attach(mut error: CalcitErr, value: &Calcit, types: &ScopeTypes) -> CalcitErr {
  if REQUIRE_ASSERTION_PROOF.with(Cell::get) && error.provenance.is_empty() {
    *error.provenance = explain(value, types, true);
  }
  error
}
