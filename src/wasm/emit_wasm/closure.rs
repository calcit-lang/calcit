use super::*;
use crate::wasm::host::calcit::CalcitFn;

/// Normalized parameter shape shared by top-level functions, lifted closures, and trampolines.
#[derive(Clone, Debug)]
pub(super) struct ClosureParamShape {
  pub(super) params: Vec<String>,
  pub(super) required: usize,
  pub(super) rest: bool,
  pub(super) option_types: Vec<Arc<CalcitTypeAnnotation>>,
}

impl ClosureParamShape {
  pub(super) fn positional(&self) -> usize {
    self.params.len() - usize::from(self.rest)
  }

  pub(super) fn accepts(&self, count: usize) -> bool {
    count >= self.required && (self.rest || count <= self.positional())
  }
}

/// Captured variable descriptor recorded for a lifted closure environment.
#[derive(Clone)]
pub(super) struct CaptureSpec {
  pub(super) name: String,
  pub(super) is_heap: bool,
  pub(super) evidence: Option<WasmTypeEvidence>,
}

/// Static zero-capture closure singleton emitted for a top-level function value.
#[derive(Clone, Debug)]
pub(super) struct StaticClosureSpec {
  pub(super) fn_idx: u32,
  pub(super) shape: ClosureParamShape,
  pub(super) has_rest_fn: bool,
}

/// Dynamic or static-fallback closure entry backed by a generated `__closure_body_<id>`
/// (or a direct top-level `fn_idx` when using fallback allocation).
#[derive(Clone)]
pub(super) struct LiftedClosureSpec {
  pub(super) id: u32,
  pub(super) body_fn_idx: u32,
  pub(super) name: String,
  pub(super) shape: ClosureParamShape,
  pub(super) arg_types: Vec<Arc<CalcitTypeAnnotation>>,
  pub(super) body: Vec<Calcit>,
  pub(super) captures: Vec<CaptureSpec>,
  pub(super) self_name: Option<String>,
  pub(super) source_ns: String,
}

#[derive(Clone)]
pub(super) enum ClosureEntry {
  Static(StaticClosureSpec),
  Lifted(LiftedClosureSpec),
}

impl ClosureEntry {
  fn accepts(&self, count: usize) -> bool {
    match self {
      ClosureEntry::Static(spec) => spec.shape.accepts(count),
      ClosureEntry::Lifted(spec) => spec.shape.accepts(count),
    }
  }
}

/// Compile-time registry for static closure singletons, lifted closure bodies, and
/// arity-indexed `BrTable` dispatch trampolines (`__closure_call_<arity>`).
#[derive(Clone, Default)]
pub(super) struct ClosureRegistry {
  next_dyn_fn_idx: Rc<RefCell<u32>>,
  pub(super) entries: Vec<ClosureEntry>,
  pub(super) static_by_qualified: HashMap<String, u32>,
  pub(super) static_ptrs: HashMap<String, u32>,
  static_fallbacks: HashMap<String, u32>,
  lifted_by_key: HashMap<String, (u32, u32)>,
  trampoline_fn_by_arity: HashMap<u32, u32>,
  trampoline_arities: Vec<(u32, u32)>,
}

impl ClosureRegistry {
  pub(super) fn new(next_dyn_fn_idx: Rc<RefCell<u32>>) -> Self {
    Self {
      next_dyn_fn_idx,
      ..Self::default()
    }
  }

  fn alloc_fn_idx(&self) -> u32 {
    let mut next = self.next_dyn_fn_idx.borrow_mut();
    let idx = *next;
    *next += 1;
    idx
  }

  /// Registers all pre-allocated static singletons in deterministic `0..S` ID order.
  pub(super) fn register_static_singletons(
    &mut self,
    singletons: &[StaticClosureSingleton],
    fn_index: &HashMap<String, u32>,
    fn_arity: &HashMap<String, u32>,
    fn_has_rest: &HashMap<String, u32>,
    fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
    fn_schemas: &HashMap<String, Arc<CalcitFnTypeAnnotation>>,
  ) {
    let mut def_args_by_qualified: HashMap<String, &CalcitFnArgs> = HashMap::with_capacity(fn_defs.len());
    for (ns, name, args, _) in fn_defs {
      def_args_by_qualified.insert(format!("{ns}/{name}"), args);
    }

    for singleton in singletons {
      // Keep slot numbering aligned even if the symbol did not map to a compiled function.
      let fn_idx = fn_index.get(&singleton.qualified).copied().unwrap_or(u32::MAX);
      let arity = fn_arity.get(&singleton.qualified).copied().unwrap_or(0);
      let has_rest_fn = fn_has_rest.contains_key(&singleton.qualified);
      let shape = if let Some(args) = def_args_by_qualified.get(&singleton.qualified) {
        let sig = fn_schemas.get(&singleton.qualified).map(|s| s.as_ref());
        shape_from_fn_args(args, sig)
      } else {
        ClosureParamShape {
          params: (0..arity).map(|i| format!("arg{i}")).collect(),
          required: arity as usize,
          rest: false,
          option_types: Vec::new(),
        }
      };

      let id = self.entries.len() as u32;
      debug_assert_eq!(id, singleton.closure_id);
      self.static_by_qualified.insert(singleton.qualified.clone(), id);
      self.static_ptrs.insert(singleton.qualified.clone(), singleton.static_ptr);
      self.entries.push(ClosureEntry::Static(StaticClosureSpec {
        fn_idx,
        shape,
        has_rest_fn,
      }));
    }
  }

  /// Ensures a `__closure_call_<arity>` `BrTable` trampoline exists and returns its WASM function index.
  pub(super) fn ensure_trampoline(&mut self, arity: u32) -> u32 {
    if let Some(&fn_idx) = self.trampoline_fn_by_arity.get(&arity) {
      return fn_idx;
    }
    let fn_idx = self.alloc_fn_idx();
    self.trampoline_fn_by_arity.insert(arity, fn_idx);
    self.trampoline_arities.push((arity, fn_idx));
    fn_idx
  }

  fn ensure_static_fallback_id(&mut self, qualified: &str, fn_idx: u32, shape: ClosureParamShape, has_rest_fn: bool) -> u32 {
    if let Some(&id) = self.static_fallbacks.get(qualified) {
      return id;
    }
    let id = self.entries.len() as u32;
    self.static_fallbacks.insert(qualified.to_owned(), id);
    self.entries.push(ClosureEntry::Static(StaticClosureSpec {
      fn_idx,
      shape,
      has_rest_fn,
    }));
    id
  }

  fn intern_lifted(&mut self, key: String, spec_builder: impl FnOnce(u32, u32) -> LiftedClosureSpec) -> (u32, u32) {
    if let Some(&(id, body_fn_idx)) = self.lifted_by_key.get(&key) {
      return (id, body_fn_idx);
    }
    let id = self.entries.len() as u32;
    let body_fn_idx = self.alloc_fn_idx();
    let spec = spec_builder(id, body_fn_idx);
    self.entries.push(ClosureEntry::Lifted(spec));
    self.lifted_by_key.insert(key, (id, body_fn_idx));
    (id, body_fn_idx)
  }
}

fn shape_from_fn_args(args: &CalcitFnArgs, signature: Option<&CalcitFnTypeAnnotation>) -> ClosureParamShape {
  let params = fn_param_names(args);
  let (required, rest) = match args {
    CalcitFnArgs::Args(_) => (params.len(), false),
    CalcitFnArgs::MarkedArgs(labels) => arg_labels_shape(labels).unwrap_or((params.len(), false)),
  };
  let option_types = if !rest && required == params.len() {
    if let Some(sig) = signature {
      let count = crate::wasm::host::calcit::trailing_option_arg_count(&sig.arg_types, params.len());
      sig.arg_types.iter().skip(params.len() - count).cloned().collect()
    } else {
      Vec::new()
    }
  } else {
    Vec::new()
  };
  ClosureParamShape {
    required: required - option_types.len(),
    params,
    rest,
    option_types,
  }
}

struct DefnClosureParts {
  self_name: Option<String>,
  shape: ClosureParamShape,
  arg_types: Vec<Arc<CalcitTypeAnnotation>>,
  body: Vec<Calcit>,
}

fn extract_defn_closure_parts(ctx: &WasmGenCtx, items: &[Calcit], source_expr: Option<&Calcit>) -> Result<DefnClosureParts, String> {
  let self_name = match items.get(1) {
    Some(Calcit::Symbol { sym, .. }) | Some(Calcit::Local(CalcitLocal { sym, .. })) => {
      let name = sym.as_ref();
      if name.is_empty() || name.starts_with("fn%") {
        None
      } else {
        Some(name.to_owned())
      }
    }
    _ => None,
  };
  let param_list = match items.get(2) {
    Some(Calcit::List(list)) => list,
    other => return Err(format!("expected parameter list in nested fn/defn closure, got: {other:?}")),
  };
  let mut params = Vec::new();
  let mut labels = Vec::new();
  for p in param_list.iter() {
    match p {
      Calcit::Local(CalcitLocal { sym, .. }) | Calcit::Symbol { sym, .. } => match sym.as_ref() {
        "&" => labels.push(CalcitArgLabel::RestMark),
        "?" => labels.push(CalcitArgLabel::OptionalMark),
        name => {
          labels.push(CalcitArgLabel::Idx(0));
          params.push(name.to_owned());
        }
      },
      Calcit::Syntax(CalcitSyntax::ArgSpread, _) => labels.push(CalcitArgLabel::RestMark),
      Calcit::Syntax(CalcitSyntax::ArgOptional, _) => labels.push(CalcitArgLabel::OptionalMark),
      other => return Err(format!("invalid closure parameter `{other}`")),
    }
  }
  let (required, rest) =
    arg_labels_shape(&labels).ok_or_else(|| "unsupported `?`/`&` parameter marker placement in closure".to_owned())?;
  let body: Vec<Calcit> = items.iter().skip(3).cloned().collect();
  let fn_sig = source_expr
    .and_then(|e| infer_wasm_static_type(ctx, e))
    .and_then(|t| t.resolve_to_fn());
  let option_types = if !rest && required == params.len() {
    if let Some(sig) = fn_sig.as_deref() {
      let count = crate::wasm::host::calcit::trailing_option_arg_count(&sig.arg_types, params.len());
      sig.arg_types.iter().skip(params.len() - count).cloned().collect()
    } else {
      Vec::new()
    }
  } else {
    Vec::new()
  };
  let arg_types = fn_sig.map(|sig| sig.arg_types.clone()).unwrap_or_default();
  Ok(DefnClosureParts {
    self_name,
    shape: ClosureParamShape {
      required: required - option_types.len(),
      params,
      rest,
      option_types,
    },
    arg_types,
    body,
  })
}

fn binding_names_in_let_pair(pair: &Calcit) -> Option<String> {
  if let Calcit::List(xs) = pair
    && xs.len() == 2
  {
    match &xs[0] {
      Calcit::Local(CalcitLocal { sym, .. }) | Calcit::Symbol { sym, .. } => Some(sym.to_string()),
      _ => None,
    }
  } else {
    None
  }
}

/// Collects free local/lambda/self-closure variable names referenced by `body` in deterministic order.
fn collect_free_vars(
  body: &[Calcit],
  initial_bound: &HashSet<String>,
  outer_locals: &HashMap<String, u32>,
  outer_lambdas: &HashMap<String, Arc<InlineClosure>>,
  outer_self: Option<&(String, u32, ClosureParamShape)>,
) -> Vec<String> {
  let mut free = Vec::new();
  let mut seen = HashSet::new();

  fn record_if_free(
    name: &str,
    bound: &HashSet<String>,
    outer_locals: &HashMap<String, u32>,
    outer_lambdas: &HashMap<String, Arc<InlineClosure>>,
    outer_self: Option<&(String, u32, ClosureParamShape)>,
    free: &mut Vec<String>,
    seen: &mut HashSet<String>,
  ) {
    if bound.contains(name) || seen.contains(name) {
      return;
    }
    let in_outer_self = outer_self.is_some_and(|(self_name, _, _)| self_name == name);
    if outer_locals.contains_key(name) || outer_lambdas.contains_key(name) || in_outer_self {
      seen.insert(name.to_owned());
      free.push(name.to_owned());
    }
  }

  fn visit_expr(
    expr: &Calcit,
    bound: &HashSet<String>,
    outer_locals: &HashMap<String, u32>,
    outer_lambdas: &HashMap<String, Arc<InlineClosure>>,
    outer_self: Option<&(String, u32, ClosureParamShape)>,
    free: &mut Vec<String>,
    seen: &mut HashSet<String>,
  ) {
    match expr {
      Calcit::Local(local) => record_if_free(local.sym.as_ref(), bound, outer_locals, outer_lambdas, outer_self, free, seen),
      Calcit::Symbol { sym, .. } => record_if_free(sym.as_ref(), bound, outer_locals, outer_lambdas, outer_self, free, seen),
      Calcit::List(xs) if !xs.is_empty() => match &xs[0] {
        Calcit::Syntax(CalcitSyntax::CoreLet, _) => {
          let mut inner_bound = bound.clone();
          if let Some(first_arg) = xs.get(1)
            && let Calcit::List(pair_or_list) = first_arg
          {
            let is_single_pair =
              pair_or_list.len() == 2 && matches!(pair_or_list.first(), Some(Calcit::Local(_) | Calcit::Symbol { .. }));
            if is_single_pair {
              if let Some(val) = pair_or_list.get(1) {
                visit_expr(val, &inner_bound, outer_locals, outer_lambdas, outer_self, free, seen);
              }
              if let Some(name) = binding_names_in_let_pair(first_arg) {
                inner_bound.insert(name);
              }
            } else {
              for pair in pair_or_list.iter() {
                if let Calcit::List(p) = pair
                  && p.len() == 2
                {
                  visit_expr(&p[1], &inner_bound, outer_locals, outer_lambdas, outer_self, free, seen);
                }
                if let Some(name) = binding_names_in_let_pair(pair) {
                  inner_bound.insert(name);
                }
              }
            }
          }
          for body_expr in xs.iter().skip(2) {
            visit_expr(body_expr, &inner_bound, outer_locals, outer_lambdas, outer_self, free, seen);
          }
        }
        Calcit::Syntax(CalcitSyntax::Defn, _) => {
          let mut inner_bound = bound.clone();
          if let Some(Calcit::Symbol { sym, .. } | Calcit::Local(CalcitLocal { sym, .. })) = xs.get(1) {
            inner_bound.insert(sym.to_string());
          }
          if let Some(Calcit::List(params)) = xs.get(2) {
            for p in params.iter() {
              if let Calcit::Symbol { sym, .. } | Calcit::Local(CalcitLocal { sym, .. }) = p
                && !matches!(sym.as_ref(), "&" | "?")
              {
                inner_bound.insert(sym.to_string());
              }
            }
          }
          for body_expr in xs.iter().skip(3) {
            visit_expr(body_expr, &inner_bound, outer_locals, outer_lambdas, outer_self, free, seen);
          }
        }
        Calcit::Syntax(CalcitSyntax::Match, _) => {
          if let Some(subject) = xs.get(1) {
            visit_expr(subject, bound, outer_locals, outer_lambdas, outer_self, free, seen);
          }
          let branches: Vec<&Calcit> = if xs.len() == 4
            && matches!(xs.get(2), Some(Calcit::EnumDef(_)))
            && let Some(Calcit::List(table)) = xs.get(3)
          {
            table.iter().filter(|b| !matches!(b, Calcit::Nil)).collect()
          } else {
            xs.iter().skip(2).collect()
          };
          for branch in branches {
            if let Calcit::List(pair) = branch
              && pair.len() == 2
            {
              let mut branch_bound = bound.clone();
              if let Calcit::List(pat) = &pair[0] {
                for item in pat.iter().skip(1) {
                  if let Calcit::Local(CalcitLocal { sym, .. }) | Calcit::Symbol { sym, .. } = item {
                    branch_bound.insert(sym.to_string());
                  }
                }
              }
              visit_expr(&pair[1], &branch_bound, outer_locals, outer_lambdas, outer_self, free, seen);
            } else {
              visit_expr(branch, bound, outer_locals, outer_lambdas, outer_self, free, seen);
            }
          }
        }
        Calcit::Syntax(CalcitSyntax::Quote | CalcitSyntax::Quasiquote, _) => {}
        _ => {
          for item in xs.iter() {
            visit_expr(item, bound, outer_locals, outer_lambdas, outer_self, free, seen);
          }
        }
      },
      Calcit::Fn { info, .. } => {
        let mut inner_bound = bound.clone();
        for param in fn_param_names(&info.args) {
          inner_bound.insert(param);
        }
        for body_expr in info.body.iter() {
          visit_expr(body_expr, &inner_bound, outer_locals, outer_lambdas, outer_self, free, seen);
        }
      }
      _ => {}
    }
  }

  for expr in body {
    visit_expr(expr, initial_bound, outer_locals, outer_lambdas, outer_self, &mut free, &mut seen);
  }
  free
}

fn is_inline_hof_head(head: &Calcit) -> bool {
  if let Calcit::Proc(proc) = head {
    return matches!(proc, CalcitProc::NativeListFoldl | CalcitProc::NativeListFoldlShortcut);
  }
  let name = match head {
    Calcit::Import(imp) if imp.ns.as_ref() == crate::wasm::host::calcit::CORE_NS => imp.def.as_ref(),
    Calcit::Symbol { sym, .. } => sym.as_ref(),
    Calcit::Fn { info, .. } => match &info.def_ref {
      Some(def_ref) if def_ref.def_ns.as_ref() == crate::wasm::host::calcit::CORE_NS => def_ref.def_name.as_ref(),
      _ => return false,
    },
    _ => return false,
  };
  matches!(
    name,
    "map"
      | "&list:map"
      | "filter"
      | "&list:filter"
      | "&set:filter"
      | "filter-not"
      | "each"
      | "any?"
      | "every?"
      | "find"
      | "find-index"
      | "map-indexed"
      | "&list:map-indexed"
      | "mapcat"
      | "fold"
      | "reduce"
      | "foldl"
      | "foldl-compare"
      | "foldl-shortcut"
      | "&list:foldl"
      | "&list:foldl-compare"
      | "map-kv"
      | "filter-map-kv"
      | "update"
  )
}

fn is_var_ref(expr: &Calcit, name: &str) -> bool {
  matches!(expr, Calcit::Local(local) if local.sym.as_ref() == name)
    || matches!(expr, Calcit::Symbol { sym, .. } if sym.as_ref() == name)
}

fn is_inline_lambda_expr(expr: &Calcit) -> bool {
  matches!(expr, Calcit::Fn { info, .. } if info.def_ref.is_none())
    || matches!(expr, Calcit::List(items) if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))))
}

fn inline_lambda_uses_recur(expr: &Calcit) -> bool {
  match expr {
    Calcit::Fn { info, .. } => info.body.iter().any(check_uses_recur),
    Calcit::List(items) if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) => {
      items.iter().skip(3).any(check_uses_recur)
    }
    _ => false,
  }
}

fn visit_closure_escape_expr(expr: &Calcit, name: &str, in_lifted_lambda: bool, treat_top_fn_as_inline: bool) -> bool {
  if is_var_ref(expr, name) {
    return true;
  }
  match expr {
    Calcit::List(xs) if !xs.is_empty() => match &xs[0] {
      Calcit::Syntax(CalcitSyntax::CoreLet, _) => {
        let rest_body: Vec<Calcit> = xs.iter().skip(2).cloned().collect();
        if let Some(first_arg) = xs.get(1)
          && let Calcit::List(pair_or_list) = first_arg
        {
          let is_single_pair =
            pair_or_list.len() == 2 && matches!(pair_or_list.first(), Some(Calcit::Local(_) | Calcit::Symbol { .. }));
          if is_single_pair {
            if let Some(val) = pair_or_list.get(1) {
              let rhs_stays_inline = is_inline_lambda_expr(val)
                && binding_names_in_let_pair(first_arg)
                  .as_deref()
                  .is_some_and(|lhs| !closure_name_may_escape(lhs, &rest_body));
              if visit_closure_escape_expr(val, name, in_lifted_lambda, rhs_stays_inline) {
                return true;
              }
            }
            if binding_names_in_let_pair(first_arg).as_deref() == Some(name) {
              return false;
            }
          } else {
            let pairs_vec = pair_or_list.to_vec();
            let mut shadowed = false;
            for (idx, pair) in pairs_vec.iter().enumerate() {
              if let Calcit::List(p) = pair
                && p.len() == 2
              {
                let rhs_stays_inline = is_inline_lambda_expr(&p[1])
                  && binding_names_in_let_pair(pair)
                    .as_deref()
                    .is_some_and(|lhs| !let_pairs_may_escape(lhs, &pairs_vec[idx + 1..], &rest_body));
                if visit_closure_escape_expr(&p[1], name, in_lifted_lambda, rhs_stays_inline) {
                  return true;
                }
              }
              if binding_names_in_let_pair(pair).as_deref() == Some(name) {
                shadowed = true;
                break;
              }
            }
            if shadowed {
              return false;
            }
          }
        }
        rest_body
          .iter()
          .any(|e| visit_closure_escape_expr(e, name, in_lifted_lambda, false))
      }
      Calcit::Syntax(CalcitSyntax::Defn, _) => {
        let mut shadowed = matches!(
          xs.get(1),
          Some(Calcit::Symbol { sym, .. } | Calcit::Local(CalcitLocal { sym, .. })) if sym.as_ref() == name
        );
        if let Some(Calcit::List(params)) = xs.get(2) {
          for p in params.iter() {
            if let Calcit::Symbol { sym, .. } | Calcit::Local(CalcitLocal { sym, .. }) = p
              && sym.as_ref() == name
            {
              shadowed = true;
            }
          }
        }
        if shadowed {
          return false;
        }
        let body_in_lifted = if treat_top_fn_as_inline { in_lifted_lambda } else { true };
        xs.iter().skip(3).any(|e| visit_closure_escape_expr(e, name, body_in_lifted, false))
      }
      Calcit::Syntax(CalcitSyntax::Quote | CalcitSyntax::Quasiquote, _) => false,
      Calcit::Syntax(CalcitSyntax::Match, _) => {
        if let Some(subject) = xs.get(1)
          && visit_closure_escape_expr(subject, name, in_lifted_lambda, false)
        {
          return true;
        }
        let branches: Vec<&Calcit> = if xs.len() == 4
          && matches!(xs.get(2), Some(Calcit::EnumDef(_)))
          && let Some(Calcit::List(table)) = xs.get(3)
        {
          table.iter().filter(|b| !matches!(b, Calcit::Nil)).collect()
        } else {
          xs.iter().skip(2).collect()
        };
        for branch in branches {
          if let Calcit::List(pair) = branch
            && pair.len() == 2
          {
            let shadowed = if let Calcit::List(pat) = &pair[0] {
              pat.iter().skip(1).any(|item| is_var_ref(item, name))
            } else {
              false
            };
            if !shadowed && visit_closure_escape_expr(&pair[1], name, in_lifted_lambda, false) {
              return true;
            }
          } else if visit_closure_escape_expr(branch, name, in_lifted_lambda, false) {
            return true;
          }
        }
        false
      }
      _ => {
        if is_var_ref(&xs[0], name) {
          return in_lifted_lambda
            || xs
              .iter()
              .skip(1)
              .any(|arg| visit_closure_escape_expr(arg, name, in_lifted_lambda, false));
        }
        if !in_lifted_lambda && is_inline_hof_head(&xs[0]) {
          return xs.iter().skip(1).any(|arg| {
            if is_var_ref(arg, name) {
              false
            } else {
              let arg_inline = is_inline_lambda_expr(arg) && !inline_lambda_uses_recur(arg);
              visit_closure_escape_expr(arg, name, in_lifted_lambda, arg_inline)
            }
          });
        }
        xs.iter().any(|item| visit_closure_escape_expr(item, name, in_lifted_lambda, false))
      }
    },
    Calcit::Fn { info, .. } => {
      if fn_param_names(&info.args).iter().any(|p| p == name) {
        return false;
      }
      let body_in_lifted = if treat_top_fn_as_inline { in_lifted_lambda } else { true };
      info.body.iter().any(|e| visit_closure_escape_expr(e, name, body_in_lifted, false))
    }
    _ => false,
  }
}

/// Returns true if a `let`-bound inline closure named `name` may escape direct call/HOF-callback
/// positions in `exprs` and therefore must be materialized into a heap closure pointer once at binding.
pub(super) fn closure_name_may_escape(name: &str, exprs: &[Calcit]) -> bool {
  exprs.iter().any(|e| visit_closure_escape_expr(e, name, false, false))
}

/// Checks if `let` pairs + trailing body may let `name` escape after its binding.
pub(super) fn let_pairs_may_escape(name: &str, remaining_pairs: &[Calcit], body: &[Calcit]) -> bool {
  for (idx, pair) in remaining_pairs.iter().enumerate() {
    if let Calcit::List(xs) = pair
      && xs.len() == 2
    {
      let rhs_stays_inline = is_inline_lambda_expr(&xs[1])
        && binding_names_in_let_pair(pair)
          .as_deref()
          .is_some_and(|lhs| !let_pairs_may_escape(lhs, &remaining_pairs[idx + 1..], body));
      if visit_closure_escape_expr(&xs[1], name, false, rhs_stays_inline) {
        return true;
      }
      if is_var_ref(&xs[0], name) {
        return false;
      }
    }
  }
  closure_name_may_escape(name, body)
}

fn capture_specs_from_scope(
  free_vars: Vec<String>,
  outer_local_types: &HashMap<String, WasmTypeEvidence>,
  outer_lambdas: &HashMap<String, Arc<InlineClosure>>,
) -> Result<Vec<CaptureSpec>, String> {
  if free_vars.len() > 64 {
    return Err(format!(
      "E_WASM_CLOSURE_SPECIALIZATION: closure captures {} variables (maximum supported is 64)",
      free_vars.len()
    ));
  }
  Ok(
    free_vars
      .into_iter()
      .map(|name| {
        let evidence = outer_local_types.get(&name).cloned().or_else(|| {
          if outer_lambdas.contains_key(&name) {
            Some(WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::DynFn)))
          } else {
            None
          }
        });
        let is_heap = if outer_lambdas.contains_key(&name) {
          true
        } else {
          evidence
            .as_ref()
            .is_none_or(|ev| super::dyn_trait::dyn_payload_is_heap(ev.scalar_origin.as_ref().unwrap_or(&ev.annotation)))
        };
        CaptureSpec { name, is_heap, evidence }
      })
      .collect(),
  )
}

fn build_lifted_dedup_key(
  source_ns: &str,
  self_name: Option<&str>,
  shape: &ClosureParamShape,
  body: &[Calcit],
  captures: &[CaptureSpec],
) -> String {
  let body_str = body.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>().join(";");
  let cap_str = captures
    .iter()
    .map(|c| {
      let ty_key = c
        .evidence
        .as_ref()
        .map_or_else(|| String::from("_"), |ev| ev.annotation.to_brief_string());
      format!("{}:{}:{ty_key}", c.name, usize::from(c.is_heap))
    })
    .collect::<Vec<_>>()
    .join(",");
  format!(
    "{source_ns}|{}|{}|{}|{}|[{body_str}]|[{cap_str}]",
    self_name.unwrap_or(""),
    shape.params.join(","),
    shape.required,
    usize::from(shape.rest)
  )
}

/// Emits instructions that allocate a closure object `[closure_id @0][env_count @4][heap_mask @8][cap_i @16+8*i]`
/// and leaves an `f64` closure pointer in a new local (returned).
#[allow(clippy::too_many_arguments)]
fn emit_lift_and_alloc_closure(
  ctx: &mut WasmGenCtx,
  name: String,
  self_name: Option<String>,
  shape: ClosureParamShape,
  arg_types: Vec<Arc<CalcitTypeAnnotation>>,
  body: Vec<Calcit>,
  outer_locals: &HashMap<String, u32>,
  outer_local_types: &HashMap<String, WasmTypeEvidence>,
  outer_lambdas: &HashMap<String, Arc<InlineClosure>>,
  outer_self: Option<&(String, u32, ClosureParamShape)>,
) -> Result<u32, String> {
  let mut initial_bound: HashSet<String> = shape.params.iter().cloned().collect();
  if let Some(ref sname) = self_name {
    initial_bound.insert(sname.clone());
  }
  let free_vars = collect_free_vars(&body, &initial_bound, outer_locals, outer_lambdas, outer_self);
  let captures = capture_specs_from_scope(free_vars, outer_local_types, outer_lambdas)?;

  // Evaluate / prepare each captured slot before allocating the closure object so nested
  // lambda materializations cannot invalidate heap allocation ordering.
  let mut cap_f64_locals = Vec::with_capacity(captures.len());
  let mut heap_mask: i64 = 0;
  for (i, cap) in captures.iter().enumerate() {
    if cap.is_heap {
      heap_mask |= 1i64 << i;
    }
    let val_local = ctx.alloc_local();
    if let Some(&local_idx) = outer_locals.get(&cap.name) {
      ctx.emit_local_as_f64(local_idx);
      ctx.emit(Instruction::LocalSet(val_local));
    } else if let Some(inner_closure) = outer_lambdas.get(&cap.name).cloned() {
      let inner_local = emit_materialize_inline_closure(ctx, &inner_closure, None)?;
      ctx.emit(Instruction::LocalGet(inner_local));
      ctx.emit(Instruction::LocalSet(val_local));
    } else if let Some((sname, _, _)) = outer_self
      && sname == &cap.name
    {
      // Inside a lifted self-recursive closure, param 0 IS the closure's env/self pointer.
      ctx.emit(Instruction::LocalGet(0));
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalSet(val_local));
    } else {
      return Err(format!("captured closure binding `{}` is missing", cap.name));
    }
    cap_f64_locals.push(val_local);
  }

  let dedup_key = build_lifted_dedup_key(&ctx.source_ns, self_name.as_deref(), &shape, &body, &captures);
  let source_ns = ctx.source_ns.clone();
  let (closure_id, body_fn_idx) = ctx
    .closures
    .borrow_mut()
    .intern_lifted(dedup_key, |id, body_fn_idx| LiftedClosureSpec {
      id,
      body_fn_idx,
      name: name.clone(),
      shape: shape.clone(),
      arg_types: arg_types.clone(),
      body: body.clone(),
      captures: captures.clone(),
      self_name: self_name.clone(),
      source_ns,
    });
  ctx.record_function_value_idx(body_fn_idx);

  let env_count = captures.len() as i32;
  let payload_bytes = 16 + env_count * 8;
  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, payload_bytes, ptr, "closure");

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(closure_id as i32));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(env_count));
  ctx.emit(Instruction::I32Store(mem_arg_i32(4)));

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I64Const(heap_mask));
  ctx.emit(Instruction::I64Store(mem_arg_i64(8)));

  for (i, val_local) in cap_f64_locals.into_iter().enumerate() {
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(Instruction::LocalGet(val_local));
    ctx.emit(Instruction::F64Store(mem_arg_f64((16 + i * 8) as u64)));
  }

  let out_local = ctx.alloc_local();
  ctx.ptr_to_f64(ptr);
  ctx.emit(Instruction::LocalSet(out_local));
  Ok(out_local)
}

/// Emits a zero-env heap closure fallback when a static function was not pre-allocated in static data.
fn emit_zero_env_heap_closure(ctx: &mut WasmGenCtx, closure_id: u32) {
  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, 16, ptr, "closure");
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(closure_id as i32));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32Store(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I64Const(0));
  ctx.emit(Instruction::I64Store(mem_arg_i64(8)));
  ctx.ptr_to_f64(ptr);
}

/// Emits the closure pointer (`f64`) for a top-level compiled function (`defn` / `import`).
pub(super) fn emit_static_closure_value(ctx: &mut WasmGenCtx, qualified: &str, short_name: &str) -> Result<(), String> {
  if function_requires_nil_specialization(ctx, qualified, short_name) {
    return Err(format!(
      "E_WASM_NIL_TYPE_EVIDENCE: `{qualified}` cannot be used as a first-class function before its generic argument type is known"
    ));
  }
  if function_requires_trait_specialization(ctx, qualified, short_name) {
    return Err(format!(
      "E_WASM_TRAIT_TYPE_EVIDENCE: `{qualified}` cannot be used as a first-class function before its generic argument type is known"
    ));
  }
  let target_key = if ctx.fn_index.contains_key(qualified) {
    qualified
  } else {
    short_name
  };
  let fn_idx = *ctx
    .fn_index
    .get(target_key)
    .ok_or_else(|| format!("unknown function value in WASM: {qualified}"))?;
  ctx.record_function_value(target_key);

  let static_ptr = {
    let closures = ctx.closures.borrow();
    closures
      .static_ptrs
      .get(qualified)
      .or_else(|| closures.static_ptrs.get(target_key))
      .copied()
  };
  if let Some(static_ptr) = static_ptr {
    ctx.emit(f64_const(static_ptr as f64));
    return Ok(());
  }

  let arity = ctx.fn_arity.get(target_key).copied().unwrap_or(0);
  let rest_fixed = ctx.fn_has_rest.get(target_key).copied();
  let shape = ClosureParamShape {
    params: (0..arity).map(|i| format!("arg{i}")).collect(),
    required: rest_fixed.map_or(arity as usize, |f| f as usize),
    rest: rest_fixed.is_some(),
    option_types: Vec::new(),
  };
  let closure_id = ctx
    .closures
    .borrow_mut()
    .ensure_static_fallback_id(target_key, fn_idx, shape, rest_fixed.is_some());
  emit_zero_env_heap_closure(ctx, closure_id);
  Ok(())
}

/// Lifts a `Calcit::Fn` without `def_ref` into a heap closure and pushes its `f64` pointer.
pub(super) fn emit_fn_value(ctx: &mut WasmGenCtx, info: &CalcitFn, expr: &Calcit) -> Result<(), String> {
  let shape = ClosureParamShape {
    params: fn_param_names(&info.args),
    required: match info.args.as_ref() {
      CalcitFnArgs::Args(idxs) => idxs.len(),
      CalcitFnArgs::MarkedArgs(labels) => arg_labels_shape(labels).map_or(0, |(req, _)| req),
    },
    rest: match info.args.as_ref() {
      CalcitFnArgs::Args(_) => false,
      CalcitFnArgs::MarkedArgs(labels) => arg_labels_shape(labels).is_some_and(|(_, rest)| rest),
    },
    option_types: Vec::new(),
  };
  let fn_sig = infer_wasm_static_type(ctx, expr).and_then(|t| t.resolve_to_fn());
  let mut shape = shape;
  let arg_types = if let Some(sig) = fn_sig.as_deref() {
    if !shape.rest && shape.required == shape.params.len() {
      let count = crate::wasm::host::calcit::trailing_option_arg_count(&sig.arg_types, shape.params.len());
      shape.option_types = sig.arg_types.iter().skip(shape.params.len() - count).cloned().collect();
      shape.required -= shape.option_types.len();
    }
    sig.arg_types.clone()
  } else {
    Vec::new()
  };
  let self_name = {
    let raw = info.name.as_ref();
    if raw.is_empty() || raw.starts_with("fn%") {
      None
    } else {
      Some(raw.to_owned())
    }
  };
  let display_name = format!("{}/{}", info.def_ns, info.name);
  let local = emit_lift_and_alloc_closure(
    ctx,
    display_name,
    self_name,
    shape,
    arg_types,
    info.body.to_vec(),
    &ctx.locals.clone(),
    &ctx.local_types.clone(),
    &ctx.lambda_locals.clone(),
    ctx.self_closure.as_ref().cloned().as_ref(),
  )?;
  ctx.emit(Instruction::LocalGet(local));
  Ok(())
}

/// Lifts a nested `CalcitSyntax::Defn` form in expression position into a heap closure.
pub(super) fn emit_defn_value(ctx: &mut WasmGenCtx, items: &[Calcit], expr: &Calcit) -> Result<(), String> {
  let DefnClosureParts {
    self_name,
    shape,
    arg_types,
    body,
  } = extract_defn_closure_parts(ctx, items, Some(expr))?;
  let display_name = self_name.clone().unwrap_or_else(|| format!("{}/<lambda>", ctx.source_ns));
  let local = emit_lift_and_alloc_closure(
    ctx,
    display_name,
    self_name,
    shape,
    arg_types,
    body,
    &ctx.locals.clone(),
    &ctx.local_types.clone(),
    &ctx.lambda_locals.clone(),
    ctx.self_closure.as_ref().cloned().as_ref(),
  )?;
  ctx.emit(Instruction::LocalGet(local));
  Ok(())
}

/// Materializes an `InlineClosure` once into a local `f64` closure pointer, reusing
/// `closure.materialized_local` when already materialized.
pub(super) fn emit_materialize_inline_closure(
  ctx: &mut WasmGenCtx,
  closure: &InlineClosure,
  expr_hint: Option<&Calcit>,
) -> Result<u32, String> {
  if let Some(existing) = closure.materialized_local {
    return Ok(existing);
  }
  let shape = ClosureParamShape {
    params: closure.params.clone(),
    required: closure.required,
    rest: closure.rest,
    option_types: closure.option_types.clone(),
  };
  let arg_types = expr_hint
    .and_then(|e| infer_wasm_static_type(ctx, e))
    .and_then(|t| t.resolve_to_fn())
    .map(|sig| sig.arg_types.clone())
    .unwrap_or_default();
  emit_lift_and_alloc_closure(
    ctx,
    closure
      .self_name
      .clone()
      .unwrap_or_else(|| format!("{}/<inline-lambda>", ctx.source_ns)),
    closure.self_name.clone(),
    shape,
    arg_types,
    closure.body.clone(),
    &closure.captured_locals,
    &closure.captured_local_types,
    &closure.captured_closures,
    closure.captured_self_closure.as_deref(),
  )
}

/// Pushes an `InlineClosure`'s closure pointer onto the WASM stack.
pub(super) fn emit_inline_closure_value(
  ctx: &mut WasmGenCtx,
  closure: &InlineClosure,
  expr_hint: Option<&Calcit>,
) -> Result<(), String> {
  let local = emit_materialize_inline_closure(ctx, closure, expr_hint)?;
  ctx.emit(Instruction::LocalGet(local));
  Ok(())
}

/// Emits a dynamic closure call through the `__closure_call_<arity>` `BrTable` trampoline.
pub(super) fn emit_closure_call_expr(
  ctx: &mut WasmGenCtx,
  emit_head: impl FnOnce(&mut WasmGenCtx) -> Result<(), String>,
  args_list: &[Calcit],
) -> Result<(), String> {
  let fn_local = ctx.alloc_local();
  emit_head(ctx)?;
  ctx.emit(Instruction::LocalSet(fn_local));
  let mut arg_locals = Vec::with_capacity(args_list.len());
  for arg in args_list {
    let local = ctx.alloc_local();
    emit_expr(ctx, arg)?;
    ctx.emit(Instruction::LocalSet(local));
    arg_locals.push(local);
  }
  emit_closure_call_from_locals(ctx, fn_local, &arg_locals)
}

/// Emits a dynamic closure call from already-evaluated `f64` locals.
pub(super) fn emit_closure_call_from_locals(ctx: &mut WasmGenCtx, fn_local: u32, arg_locals: &[u32]) -> Result<(), String> {
  let arity = arg_locals.len() as u32;
  let trampoline_idx = ctx.closures.borrow_mut().ensure_trampoline(arity);
  ctx.emit(Instruction::LocalGet(fn_local));
  for &arg_local in arg_locals {
    ctx.emit(Instruction::LocalGet(arg_local));
  }
  ctx.emit(Instruction::Call(trampoline_idx));
  Ok(())
}

/// Emits a direct self-recursive call to the current lifted closure's `__closure_body_<id>`
/// without going through `__closure_call_<arity>`.
pub(super) fn emit_self_closure_call(
  ctx: &mut WasmGenCtx,
  body_fn_idx: u32,
  shape: &ClosureParamShape,
  args_list: &[Calcit],
) -> Result<(), String> {
  if !shape.accepts(args_list.len()) {
    ctx.emit(Instruction::Unreachable);
    return Ok(());
  }
  let mut arg_locals = Vec::with_capacity(args_list.len());
  for arg in args_list {
    let local = ctx.alloc_local();
    emit_expr(ctx, arg)?;
    ctx.emit(Instruction::LocalSet(local));
    arg_locals.push(local);
  }
  let positional = shape.positional();
  ctx.emit(Instruction::LocalGet(0)); // env_ptr
  for &arg_local in arg_locals.iter().take(positional) {
    ctx.emit(Instruction::LocalGet(arg_local));
  }
  for i in arg_locals.len()..positional {
    if i >= positional - shape.option_types.len() {
      let none = nominal_option_none_expression()?;
      emit_expr(ctx, &none)?;
    } else {
      ctx.emit(f64_const(0.0));
    }
  }
  if shape.rest {
    let rest_list = emit_list_from_locals(ctx, &arg_locals[positional..]);
    ctx.emit(Instruction::LocalGet(rest_list));
  }
  ctx.emit(Instruction::Call(body_fn_idx));
  Ok(())
}

/// Compiles a lifted closure body `__closure_body_<id>(env_ptr: i32, p0: f64, ...) -> f64`.
fn compile_closure_body(spec: &LiftedClosureSpec, env: &WasmCompileEnv) -> Result<CompiledFn, String> {
  let arity = spec.shape.params.len();
  let mut ctx = WasmGenCtx::new(1 + arity as u32, env.clone());
  spec.source_ns.clone_into(&mut ctx.source_ns);
  ctx.source_fn_index = Some(spec.body_fn_idx);

  let mut params = Vec::with_capacity(1 + arity);
  params.push(ValType::I32); // env_ptr
  params.extend(std::iter::repeat_n(ValType::F64, arity));

  let option_start = spec.shape.positional().saturating_sub(spec.shape.option_types.len());
  for (i, pname) in spec.shape.params.iter().enumerate() {
    let local_idx = 1 + i as u32;
    ctx.locals.insert(pname.clone(), local_idx);
    ctx.arg_indices.push(local_idx);
    if i < spec.arg_types.len() {
      ctx
        .local_types
        .insert(pname.clone(), WasmTypeEvidence::contract(spec.arg_types[i].clone()));
    } else if !spec.shape.rest && i >= option_start {
      let opt_idx = i - option_start;
      ctx
        .local_types
        .insert(pname.clone(), WasmTypeEvidence::contract(spec.shape.option_types[opt_idx].clone()));
    } else if spec.shape.rest && i == arity - 1 {
      ctx.local_types.insert(
        pname.clone(),
        WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::List(DYNAMIC_TYPE.clone()))),
      );
    }
  }
  if spec.shape.rest || spec.shape.required != arity {
    ctx.recur_shape = Some((spec.shape.required, spec.shape.rest));
  }
  if let Some(ref sname) = spec.self_name
    && !spec.shape.params.iter().any(|p| p == sname)
  {
    ctx.self_closure = Some((sname.clone(), spec.body_fn_idx, spec.shape.clone()));
  }

  for (cap_idx, cap) in spec.captures.iter().enumerate() {
    if spec.shape.params.iter().any(|p| p == &cap.name) || spec.self_name.as_deref() == Some(cap.name.as_str()) {
      continue;
    }
    let cap_local = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(0)); // env_ptr
    ctx.emit(Instruction::F64Load(mem_arg_f64((16 + cap_idx * 8) as u64)));
    ctx.emit(Instruction::LocalSet(cap_local));
    ctx.locals.insert(cap.name.clone(), cap_local);
    if let Some(ev) = &cap.evidence {
      ctx.local_types.insert(cap.name.clone(), ev.clone());
    }
  }

  reject_non_tail_recur(&spec.body, &spec.name)?;
  ctx.uses_recur = spec.body.iter().any(check_uses_recur);
  if ctx.uses_recur {
    ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Result(ValType::F64)));
    emit_body(&mut ctx, &spec.body)?;
    ctx.emit(Instruction::End);
  } else {
    emit_body(&mut ctx, &spec.body)?;
  }

  Ok(CompiledFn {
    export_name: None,
    params,
    results: vec![ValType::F64],
    locals: ctx.extra_locals,
    instructions: ctx.instructions,
  })
}

/// Computes all function indices that can transitively reach any function in
/// `unsupported_dependencies` via direct `Instruction::Call` or recorded `function_values`
/// without routing through `__closure_call_*` trampolines.
fn compute_blocked_closure_targets(
  env: &WasmCompileEnv,
  compiled_fns: &[CompiledFn],
  num_imports: u32,
  dyn_slots: &[super::dyn_trait::CompiledTrampolineSlot],
  compiled_bodies: &HashMap<u32, CompiledClosureBodySlot>,
  trampoline_arities: &[(u32, u32)],
  unsupported_dependencies: &HashMap<u32, (String, String)>,
) -> HashSet<u32> {
  if unsupported_dependencies.is_empty() {
    return HashSet::new();
  }
  let trampoline_fn_indices: HashSet<u32> = trampoline_arities.iter().map(|&(_, idx)| idx).collect();
  let mut rev_adj: HashMap<u32, Vec<u32>> = HashMap::new();
  let function_values = env.function_value_dependencies.borrow();

  let mut record_fn_edges = |caller_idx: u32, instructions: &[Instruction]| {
    if trampoline_fn_indices.contains(&caller_idx) {
      return;
    }
    for instr in instructions {
      if let Instruction::Call(callee) = instr {
        rev_adj.entry(*callee).or_default().push(caller_idx);
      }
    }
  };

  for (offset, func) in compiled_fns.iter().enumerate() {
    record_fn_edges(num_imports + offset as u32, &func.instructions);
  }
  for (body_fn_idx, res) in compiled_bodies.values() {
    if let Ok(func) = res {
      record_fn_edges(*body_fn_idx, &func.instructions);
    }
  }
  {
    let dyn_reg = env.dyn_traits.borrow();
    for (slot, item) in dyn_slots.iter().enumerate() {
      if let Some(Ok(func)) = item
        && let Some(&tramp_idx) = dyn_reg.trampoline_fn_indices.get(slot)
      {
        record_fn_edges(tramp_idx, &func.instructions);
      }
    }
  }
  for (&caller_idx, refs) in function_values.iter() {
    if trampoline_fn_indices.contains(&caller_idx) {
      continue;
    }
    for &target in refs {
      rev_adj.entry(target).or_default().push(caller_idx);
    }
  }

  let mut blocked: HashSet<u32> = unsupported_dependencies.keys().copied().collect();
  let mut queue: Vec<u32> = blocked.iter().copied().collect();
  while let Some(node) = queue.pop() {
    if let Some(callers) = rev_adj.get(&node) {
      for &caller in callers {
        if blocked.insert(caller) {
          queue.push(caller);
        }
      }
    }
  }
  blocked
}

/// Compiles `__closure_call_<call_arity>(closure_f64, arg_0..arg_{n-1}) -> f64` using a `BrTable`
/// over all registered static and lifted `closure_id`s.
fn compile_closure_trampoline(
  env: &WasmCompileEnv,
  trampoline_fn_idx: u32,
  call_arity: u32,
  blocked_targets: &HashSet<u32>,
) -> Result<(CompiledFn, usize), String> {
  let entries = env.closures.borrow().entries.clone();
  let entry_count = entries.len();
  let mut ctx = WasmGenCtx::new(1 + call_arity, env.clone());
  ctx.source_fn_index = Some(trampoline_fn_idx);

  if entry_count == 0 {
    ctx.emit(Instruction::Unreachable);
  } else {
    let closure_ptr = ctx.alloc_local_typed(ValType::I32);
    let closure_id = ctx.alloc_local_typed(ValType::I32);

    ctx.emit(Instruction::LocalGet(0));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalTee(closure_ptr));
    ctx.emit(Instruction::I32Eqz);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    ctx.emit(Instruction::Unreachable);
    ctx.emit(Instruction::End);

    ctx.emit(Instruction::LocalGet(closure_ptr));
    ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
    ctx.emit(Instruction::LocalSet(closure_id));

    ctx.emit(Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64))); // $done
    ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty)); // $trap
    for _ in 0..entry_count {
      ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty)); // $arm_i
    }
    let targets: Vec<u32> = (0..entry_count as u32).collect();
    ctx.emit(Instruction::LocalGet(closure_id));
    ctx.emit(Instruction::BrTable(targets.into(), entry_count as u32));

    for (arm_idx, entry) in entries.iter().enumerate() {
      ctx.emit(Instruction::End); // close $arm_i

      let target_fn_idx = match entry {
        ClosureEntry::Static(spec) => spec.fn_idx,
        ClosureEntry::Lifted(spec) => spec.body_fn_idx,
      };
      if !entry.accepts(call_arity as usize) || target_fn_idx == u32::MAX || blocked_targets.contains(&target_fn_idx) {
        ctx.emit(Instruction::Unreachable);
      } else {
        match entry {
          ClosureEntry::Static(spec) => {
            let sig = env.fn_signatures.get(&spec.fn_idx).cloned();
            let positional = spec.shape.positional();
            let rest_list = if spec.shape.rest || spec.has_rest_fn {
              let trailing: Vec<u32> = ((1 + positional as u32)..=(call_arity)).collect();
              Some(emit_list_from_locals(&mut ctx, &trailing))
            } else {
              None
            };
            let arg_count = if spec.shape.rest || spec.has_rest_fn {
              positional
            } else {
              call_arity as usize
            };
            for i in 0..arg_count {
              let param_repr = sig.as_ref().and_then(|s| s.params.get(i).copied()).unwrap_or(NativeValRepr::F64);
              ctx.emit(Instruction::LocalGet(1 + i as u32));
              emit_unbox_f64_to_repr(&mut ctx, param_repr);
            }
            if !spec.shape.rest && !spec.has_rest_fn {
              let option_start = positional.saturating_sub(spec.shape.option_types.len());
              for i in arg_count..positional {
                let param_repr = sig.as_ref().and_then(|s| s.params.get(i).copied()).unwrap_or(NativeValRepr::F64);
                if i >= option_start {
                  let none = nominal_option_none_expression()?;
                  emit_expr_as_repr(&mut ctx, &none, param_repr)?;
                } else {
                  ctx.emit(f64_const(0.0));
                  emit_unbox_f64_to_repr(&mut ctx, param_repr);
                }
              }
            }
            if let Some(rest_local) = rest_list {
              ctx.emit(Instruction::LocalGet(rest_local));
            }
            ctx.emit(Instruction::Call(spec.fn_idx));
            let ret_repr = sig.as_ref().map_or(NativeValRepr::F64, |s| s.result);
            emit_box_repr_to_f64(&mut ctx, ret_repr);
          }
          ClosureEntry::Lifted(spec) => {
            let positional = spec.shape.positional();
            let rest_list = if spec.shape.rest {
              let trailing: Vec<u32> = ((1 + positional as u32)..=(call_arity)).collect();
              Some(emit_list_from_locals(&mut ctx, &trailing))
            } else {
              None
            };
            ctx.emit(Instruction::LocalGet(closure_ptr)); // env_ptr
            let provided = if spec.shape.rest { positional } else { call_arity as usize };
            for i in 0..provided {
              ctx.emit(Instruction::LocalGet(1 + i as u32));
            }
            if !spec.shape.rest {
              let option_start = positional.saturating_sub(spec.shape.option_types.len());
              for i in provided..positional {
                if i >= option_start {
                  let none = nominal_option_none_expression()?;
                  emit_expr(&mut ctx, &none)?;
                } else {
                  ctx.emit(f64_const(0.0));
                }
              }
            }
            if let Some(rest_local) = rest_list {
              ctx.emit(Instruction::LocalGet(rest_local));
            }
            ctx.emit(Instruction::Call(spec.body_fn_idx));
          }
        }
      }
      ctx.emit(Instruction::Br((entry_count - arm_idx) as u32));
    }

    ctx.emit(Instruction::End); // close $trap
    ctx.emit(Instruction::Unreachable);
    ctx.emit(Instruction::End); // close $done
  }

  Ok((
    CompiledFn {
      export_name: None,
      params: vec![ValType::F64; 1 + call_arity as usize],
      results: vec![ValType::F64],
      locals: std::mem::take(&mut ctx.extra_locals),
      instructions: std::mem::take(&mut ctx.instructions),
    },
    entry_count,
  ))
}

pub(super) type CompiledClosureBodySlot = (u32, Result<CompiledFn, Vec<ValType>>);
pub(super) type CompiledClosureTrampolineSlot = Option<(u32, u32, usize, usize, Result<CompiledFn, (String, String, u32)>)>;

/// Executes one step of lifted closure body and `BrTable` trampoline compilation inside
/// the joint fixed-point loop. Returns `true` if any body or trampoline changed.
pub(super) fn compile_closures_step(
  env: &WasmCompileEnv,
  compiled_fns: &[CompiledFn],
  num_imports: u32,
  dyn_slots: &[super::dyn_trait::CompiledTrampolineSlot],
  compiled_bodies: &mut HashMap<u32, CompiledClosureBodySlot>,
  compiled_trampolines: &mut Vec<CompiledClosureTrampolineSlot>,
  unsupported_dependencies: &mut HashMap<u32, (String, String)>,
) -> bool {
  let mut changed = false;

  // Snapshot current lifted specs because compiling a body may intern nested closures.
  let lifted_specs: Vec<LiftedClosureSpec> = env
    .closures
    .borrow()
    .entries
    .iter()
    .filter_map(|entry| match entry {
      ClosureEntry::Lifted(spec) => Some(spec.clone()),
      ClosureEntry::Static(_) => None,
    })
    .collect();

  for spec in lifted_specs {
    if compiled_bodies.contains_key(&spec.id) {
      continue;
    }
    changed = true;
    let params = {
      let mut p = Vec::with_capacity(1 + spec.shape.params.len());
      p.push(ValType::I32);
      p.extend(std::iter::repeat_n(ValType::F64, spec.shape.params.len()));
      p
    };
    let compiled = match compile_closure_body(&spec, env) {
      Ok(func) => Ok(func),
      Err(err) => {
        unsupported_dependencies.insert(spec.body_fn_idx, (spec.name.clone(), err));
        Err(params)
      }
    };
    compiled_bodies.insert(spec.id, (spec.body_fn_idx, compiled));
  }

  let trampoline_arities = env.closures.borrow().trampoline_arities.clone();
  if compiled_trampolines.len() < trampoline_arities.len() {
    compiled_trampolines.resize_with(trampoline_arities.len(), || None);
  }
  let entry_count = env.closures.borrow().entries.len();
  let unsupported_count = unsupported_dependencies.len();
  let blocked_targets = compute_blocked_closure_targets(
    env,
    compiled_fns,
    num_imports,
    dyn_slots,
    compiled_bodies,
    &trampoline_arities,
    unsupported_dependencies,
  );
  let blocked_count = blocked_targets.len();

  for (slot, &(arity, fn_idx)) in trampoline_arities.iter().enumerate() {
    if let Some((_, _, prev_entries, prev_blocked, _)) = compiled_trampolines[slot]
      && prev_entries == entry_count
      && prev_blocked == blocked_count
    {
      continue;
    }
    changed = true;
    let res = match compile_closure_trampoline(env, fn_idx, arity, &blocked_targets) {
      Ok((func, _)) => Ok(func),
      Err(err) => Err((format!("__closure_call_{arity}"), err, arity)),
    };
    compiled_trampolines[slot] = Some((arity, fn_idx, entry_count, blocked_count, res));
  }

  let has_new_lifted = env
    .closures
    .borrow()
    .entries
    .iter()
    .any(|entry| matches!(entry, ClosureEntry::Lifted(spec) if !compiled_bodies.contains_key(&spec.id)));
  let has_new_trampolines = env.closures.borrow().trampoline_arities.len() != compiled_trampolines.len();
  let new_unsupported = unsupported_dependencies.len() != unsupported_count;

  changed || has_new_lifted || has_new_trampolines || new_unsupported
}

/// Writes compiled closure bodies and trampolines into their assigned `dynamic_fns` slots.
pub(super) fn finalize_closures(
  compiled_bodies: HashMap<u32, CompiledClosureBodySlot>,
  compiled_trampolines: Vec<CompiledClosureTrampolineSlot>,
  dyn_base_fn_idx: u32,
  dynamic_fns: &mut [Option<CompiledFn>],
  unsupported_dependencies: &mut HashMap<u32, (String, String)>,
) {
  for (_, (fn_idx, entry)) in compiled_bodies {
    let slot = (fn_idx - dyn_base_fn_idx) as usize;
    dynamic_fns[slot] = Some(match entry {
      Ok(func) => func,
      Err(params) => CompiledFn {
        export_name: None,
        params,
        results: vec![ValType::F64],
        locals: Vec::new(),
        instructions: vec![Instruction::Unreachable],
      },
    });
  }
  for item in compiled_trampolines {
    let (arity, fn_idx, _, _, res) = item.expect("closure trampoline slot must be compiled");
    let slot = (fn_idx - dyn_base_fn_idx) as usize;
    dynamic_fns[slot] = Some(match res {
      Ok(func) => func,
      Err((label, err, _)) => {
        unsupported_dependencies.insert(fn_idx, (label, err));
        CompiledFn {
          export_name: None,
          params: vec![ValType::F64; 1 + arity as usize],
          results: vec![ValType::F64],
          locals: Vec::new(),
          instructions: vec![Instruction::Unreachable],
        }
      }
    });
  }
}
