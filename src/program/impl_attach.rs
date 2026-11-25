//! Compile-time attachment of `defimpl Type Trait` (#2727).
//!
//! A struct or enum names the traits it implements with an `(:impls …)`
//! clause, and each implementation is a top-level `defimpl Type Trait …`:
//!
//! ```cirru
//! defstruct Coalescer (:delay-ms Number) (:impls CoalescerOps)
//! defimpl Coalescer CoalescerOps (.flush coalescer:flush)
//! ```
//!
//! Definitions are reached by reference, never by scanning the snapshot, so an
//! implementation is found the same way: the type refers to the trait, and the
//! `defimpl` is looked up only in the namespace of the type and the namespace
//! of the trait. When the program is loaded, the type's code is rewritten into
//! the `impl-traits` shape the checker and every backend already understand:
//!
//! ```cirru
//! calcit.core/impl-traits (defstruct Coalescer (:delay-ms Number)) Coalescer-CoalescerOps
//! ```
//!
//! The generated head is the qualified `calcit.core/impl-traits` symbol, so it
//! can be told apart from attachment written by hand. The code as written is
//! kept aside and restored before attaching again on reload.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

use crate::calcit::{self, Calcit, CalcitImport, CalcitList, CalcitProc, CalcitSyntax, CalcitTypeAnnotation};

use super::{ImportRule, ProgramCodeData, ProgramFileData};

const MISSING_TRAIT_IMPL_ERROR: &str = "E_MISSING_TRAIT_IMPL";
const DUPLICATE_TRAIT_IMPL_ERROR: &str = "E_DUPLICATE_TRAIT_IMPL";
const UNKNOWN_IMPLS_TRAIT_ERROR: &str = "E_UNKNOWN_IMPLS_TRAIT";
/// Head of a generated attachment. Hand-written attachment uses `impl-traits`.
pub(crate) const GENERATED_ATTACH_HEAD: &str = "calcit.core/impl-traits";
const IMPLS_CLAUSE: &str = "impls";

pub(crate) type DefKey = (Arc<str>, Arc<str>);

/// For each attached definition: the code as written, and the code generated
/// from it. A definition whose current code is no longer the generated one
/// was replaced since (by a reload) and keeps its new code.
static WRITTEN_CODE: LazyLock<Mutex<HashMap<DefKey, (Calcit, Calcit)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn symbol_name(form: &Calcit) -> Option<&str> {
  match form {
    Calcit::Symbol { sym, .. } => Some(sym.as_ref()),
    _ => None,
  }
}

fn is_impls_clause(form: &Calcit) -> bool {
  matches!(form, Calcit::List(items) if matches!(items.first(), Some(Calcit::Tag(tag)) if tag.ref_str() == IMPLS_CLAUSE))
}

/// `(defimpl Type Trait …)`: the type and trait symbols.
fn defimpl_type_and_trait(code: &Calcit) -> Option<(&str, &str)> {
  let Calcit::List(items) = code else { return None };
  if items.first().and_then(symbol_name) != Some("defimpl") {
    return None;
  }
  Some((items.get(1).and_then(symbol_name)?, items.get(2).and_then(symbol_name)?))
}

/// Resolve a symbol written in `ns` to the definition it names.
fn resolve_in(program: &ProgramCodeData, ns: &str, file: &ProgramFileData, name: &str) -> Option<DefKey> {
  if let Some((prefix, def)) = name.split_once('/') {
    let target_ns = match file.import_map.get(prefix).map(AsRef::as_ref) {
      Some(ImportRule::NsAs(target)) => target.to_owned(),
      _ => Arc::from(prefix),
    };
    return program
      .get(&target_ns)
      .is_some_and(|target| target.defs.contains_key(def))
      .then(|| (target_ns, Arc::from(def)));
  }
  if file.defs.contains_key(name) {
    return Some((Arc::from(ns), Arc::from(name)));
  }
  match file.import_map.get(name).map(AsRef::as_ref) {
    Some(ImportRule::NsReferDef(target_ns, target_def)) => Some((target_ns.to_owned(), target_def.to_owned())),
    _ => program
      .get(calcit::CORE_NS)
      .is_some_and(|core| core.defs.contains_key(name))
      .then(|| (Arc::from(calcit::CORE_NS), Arc::from(name))),
  }
}

/// The `defstruct`/`defenum` form of a definition: the code itself, or the
/// value of `def X (…)`.
fn nominal_form(code: &Calcit) -> Option<&Calcit> {
  let Calcit::List(items) = code else { return None };
  match items.first().and_then(symbol_name) {
    Some("defstruct" | "defenum") => Some(code),
    Some("def") => items.get(2).and_then(nominal_form),
    _ => None,
  }
}

/// The trait symbols of the `(:impls …)` clause, and the code without it.
fn take_impls_clause(code: &Calcit) -> Option<(Vec<Calcit>, Calcit)> {
  let Calcit::List(items) = code else { return None };
  match items.first().and_then(symbol_name) {
    Some("defstruct" | "defenum") => {
      let index = items.iter().position(is_impls_clause)?;
      let Some(Calcit::List(clause)) = items.get(index) else {
        return None;
      };
      let traits = clause.iter().skip(1).cloned().collect();
      let rest: Vec<Calcit> = items
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .map(|(_, x)| x.to_owned())
        .collect();
      Some((traits, Calcit::from(rest)))
    }
    Some("def") => {
      let (traits, value) = take_impls_clause(items.get(2)?)?;
      let mut rebuilt: Vec<Calcit> = items.iter().cloned().collect();
      rebuilt[2] = value;
      Some((traits, Calcit::from(rebuilt)))
    }
    _ => None,
  }
}

fn attach(code: &Calcit, impls: &[Calcit]) -> Calcit {
  let wrap = |value: &Calcit| {
    let info = match value {
      Calcit::List(xs) => match xs.first() {
        Some(Calcit::Symbol { info, .. }) => Some(info.to_owned()),
        _ => None,
      },
      _ => None,
    };
    let mut items = vec![Calcit::Symbol {
      sym: Arc::from(GENERATED_ATTACH_HEAD),
      info: info.unwrap_or_else(|| {
        Arc::new(calcit::CalcitSymbolInfo {
          at_ns: Arc::from(""),
          at_def: Arc::from(""),
        })
      }),
      location: None,
    }];
    items.push(value.to_owned());
    items.extend(impls.iter().cloned());
    Calcit::from(items)
  };
  if let Calcit::List(items) = code
    && items.first().and_then(symbol_name) == Some("def")
    && let Some(value) = items.get(2)
  {
    let mut rebuilt: Vec<Calcit> = items.iter().cloned().collect();
    rebuilt[2] = wrap(value);
    return Calcit::from(rebuilt);
  }
  wrap(code)
}

/// A reference to the impl definition, written from the type's namespace.
fn impl_reference(code: &Calcit, impl_ns: &str, impl_def: &str, type_ns: &str) -> Calcit {
  let info = match code {
    Calcit::List(items) => match items.first() {
      Some(Calcit::Symbol { info, .. }) => Some(info.to_owned()),
      _ => None,
    },
    _ => None,
  }
  .unwrap_or_else(|| {
    Arc::new(calcit::CalcitSymbolInfo {
      at_ns: Arc::from(type_ns),
      at_def: Arc::from(""),
    })
  });
  let sym = if impl_ns == type_ns {
    Arc::from(impl_def)
  } else {
    Arc::from(format!("{impl_ns}/{impl_def}"))
  };
  Calcit::Symbol { sym, info, location: None }
}

/// The `defimpl` of `type_key` for `trait_key`, looked up only in the
/// namespaces of the type and of the trait.
fn find_impl(program: &ProgramCodeData, type_key: &DefKey, trait_key: &DefKey) -> Vec<DefKey> {
  let mut namespaces = vec![&type_key.0];
  if trait_key.0 != type_key.0 {
    namespaces.push(&trait_key.0);
  }
  let mut found = vec![];
  for ns in namespaces {
    let Some(file) = program.get(ns) else { continue };
    let mut defs: Vec<&Arc<str>> = file.defs.keys().collect();
    defs.sort();
    for def in defs {
      let Some((type_name, trait_name)) = defimpl_type_and_trait(&file.defs[def].code) else {
        continue;
      };
      if resolve_in(program, ns, file, type_name).as_ref() == Some(type_key)
        && resolve_in(program, ns, file, trait_name).as_ref() == Some(trait_key)
      {
        found.push((ns.to_owned(), def.to_owned()));
      }
    }
  }
  found
}

/// Follow simple top-level alias definitions (`def Alias Target`) to the underlying nominal trait key.
fn resolve_trait_origin(program: &ProgramCodeData, start: &DefKey) -> DefKey {
  let mut current = start.clone();
  let mut visited = vec![start.clone()];
  while let Some(file) = program.get(&current.0)
    && let Some(entry) = file.defs.get(&current.1)
  {
    let target_form = match &entry.code {
      Calcit::List(items) if items.len() == 3 && items.first().and_then(symbol_name) == Some("def") => items.get(2),
      other => Some(other),
    };
    let Some(target) = target_form else { break };
    let next = match target {
      Calcit::Symbol { sym, .. } => resolve_in(program, &current.0, file, sym.as_ref()),
      Calcit::Import(import) => Some((import.ns.clone(), import.def.clone())),
      _ => None,
    };
    let Some(next_key) = next else { break };
    if visited.contains(&next_key) {
      break;
    }
    visited.push(next_key.clone());
    current = next_key;
  }
  current
}

/// Follow simple top-level alias definitions (`def Alias Target`) to the underlying nominal definition key.
fn resolve_nominal_target(
  program: &ProgramCodeData,
  ns: &str,
  file: &ProgramFileData,
  name: &str,
  written: &HashMap<DefKey, (Calcit, Calcit)>,
) -> Option<DefKey> {
  let start = resolve_in(program, ns, file, name)?;
  let mut current = start.clone();
  let mut visited = vec![start];
  while let Some(target_file) = program.get(&current.0)
    && let Some(entry) = target_file.defs.get(&current.1)
  {
    let code = effective_written_code(&current, &entry.code, written);
    if nominal_form(code).is_some() || code_defines_nominal_type(code) {
      return Some(current);
    }
    let target_form = match code {
      Calcit::List(items) if items.len() == 3 && items.first().and_then(symbol_name) == Some("def") => items.get(2),
      _ => None,
    };
    let Some(target) = target_form else { break };
    let next = match target {
      Calcit::Symbol { sym, .. } => resolve_in(program, &current.0, target_file, sym.as_ref()),
      Calcit::Import(import) => Some((import.ns.clone(), import.def.clone())),
      _ => None,
    };
    let Some(next_key) = next else { break };
    if visited.contains(&next_key) {
      break;
    }
    visited.push(next_key.clone());
    current = next_key;
  }
  None
}

fn is_impl_attach_head(program: &ProgramCodeData, ns: &str, file: &ProgramFileData, head: &Calcit) -> bool {
  match head {
    Calcit::Proc(
      CalcitProc::NativeStructImplTraits
      | CalcitProc::NativeEnumImplTraits
      | CalcitProc::NativeStructValueImplTraits
      | CalcitProc::NativeEnumValueImplTraits,
    ) => true,
    Calcit::Import(CalcitImport { ns: head_ns, def, .. }) => {
      head_ns.as_ref() == calcit::CORE_NS
        && matches!(
          def.as_ref(),
          "impl-traits" | "&struct:impl-traits" | "&enum:impl-traits" | "&struct-def:impl-traits" | "&enum-def:impl-traits"
        )
    }
    Calcit::Fn { info, .. } => info.def_ns.as_ref() == calcit::CORE_NS && info.name.as_ref() == "impl-traits",
    Calcit::Symbol { sym, .. } => {
      if matches!(
        sym.as_ref(),
        "impl-traits"
          | GENERATED_ATTACH_HEAD
          | "&struct:impl-traits"
          | "&enum:impl-traits"
          | "&struct-def:impl-traits"
          | "&enum-def:impl-traits"
      ) {
        return true;
      }
      resolve_in(program, ns, file, sym.as_ref()).is_some_and(|(target_ns, target_def)| {
        target_ns.as_ref() == calcit::CORE_NS
          && matches!(
            target_def.as_ref(),
            "impl-traits" | "&struct:impl-traits" | "&enum:impl-traits" | "&struct-def:impl-traits" | "&enum-def:impl-traits"
          )
      })
    }
    _ => false,
  }
}

fn is_core_symbol_or_import(program: &ProgramCodeData, ns: &str, file: &ProgramFileData, head: &Calcit, expected: &str) -> bool {
  match head {
    Calcit::Import(CalcitImport { ns: head_ns, def, .. }) => head_ns.as_ref() == calcit::CORE_NS && def.as_ref() == expected,
    Calcit::Symbol { sym, .. } => {
      sym.as_ref() == expected
        || resolve_in(program, ns, file, sym.as_ref())
          .is_some_and(|(target_ns, target_def)| target_ns.as_ref() == calcit::CORE_NS && target_def.as_ref() == expected)
    }
    _ => false,
  }
}

fn nominal_key_from_annotation(
  program: &ProgramCodeData,
  ns: &str,
  file: &ProgramFileData,
  annotation: &CalcitTypeAnnotation,
  written: &HashMap<DefKey, (Calcit, Calcit)>,
) -> Option<DefKey> {
  let from_def_ref = |def_ref: Option<&str>| -> Option<DefKey> {
    let (target_ns, target_def) = def_ref?.rsplit_once('/')?;
    Some((Arc::from(target_ns), Arc::from(target_def)))
  };
  match annotation {
    CalcitTypeAnnotation::StructDef(def) | CalcitTypeAnnotation::Struct(def, _) | CalcitTypeAnnotation::StructValue(def) => {
      from_def_ref(def.definition_ref.as_deref())
    }
    CalcitTypeAnnotation::EnumDef(def) | CalcitTypeAnnotation::Enum(def, _) | CalcitTypeAnnotation::EnumValue(def) => {
      from_def_ref(def.definition_ref().map(AsRef::as_ref))
    }
    CalcitTypeAnnotation::TypeRef(name, _) => {
      let stripped = name.trim_start_matches('\'').trim_start_matches(':');
      resolve_nominal_target(program, ns, file, stripped, written)
    }
    _ => None,
  }
}

fn fn_param_names_from_list(params: &CalcitList) -> Vec<Arc<str>> {
  params
    .iter()
    .filter_map(|item| match item {
      Calcit::Symbol { sym, .. } if !matches!(sym.as_ref(), "?" | "&") => Some(sym.clone()),
      Calcit::Local(local) => Some(local.sym.clone()),
      _ => None,
    })
    .collect()
}

#[derive(Clone, Copy)]
struct NonCoherentScanCtx<'a> {
  program: &'a ProgramCodeData,
  ns: &'a str,
  file: &'a ProgramFileData,
  written: &'a HashMap<DefKey, (Calcit, Calcit)>,
}

fn type_node_nominal_target(scan: NonCoherentScanCtx<'_>, node: &Calcit) -> Option<DefKey> {
  let from_def_ref = |def_ref: Option<&str>| -> Option<DefKey> {
    let (target_ns, target_def) = def_ref?.rsplit_once('/')?;
    Some((Arc::from(target_ns), Arc::from(target_def)))
  };
  match node {
    Calcit::Symbol { sym, .. } => resolve_nominal_target(
      scan.program,
      scan.ns,
      scan.file,
      sym.trim_start_matches('\'').trim_start_matches(':'),
      scan.written,
    ),
    Calcit::Import(import) => resolve_nominal_target(
      scan.program,
      scan.ns,
      scan.file,
      &format!("{}/{}", import.ns, import.def),
      scan.written,
    ),
    Calcit::StructDef(def) => from_def_ref(def.definition_ref.as_deref()),
    Calcit::EnumDef(def) => from_def_ref(def.definition_ref().map(AsRef::as_ref)),
    Calcit::Struct(def) => from_def_ref(def.struct_ref.definition_ref.as_deref()),
    Calcit::Enum(def) => from_def_ref(def.sum_type.as_ref().and_then(|d| d.definition_ref()).map(AsRef::as_ref)),
    Calcit::List(items) => {
      let head = items.first()?;
      if (matches!(head, Calcit::Syntax(CalcitSyntax::Quote, _))
        || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "quote"))
        && let Some(inner) = items.get(1)
      {
        return type_node_nominal_target(scan, inner);
      }
      if (matches!(head, Calcit::Proc(CalcitProc::NativeEnum))
        || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "::"))
        && let Some(target) = items.get(1)
      {
        return type_node_nominal_target(scan, target);
      }
      type_node_nominal_target(scan, head)
    }
    _ => None,
  }
}

fn hint_fn_arg_nodes(scan: NonCoherentScanCtx<'_>, body: &[Calcit]) -> Option<Vec<Calcit>> {
  let spec = body.iter().find_map(|item| {
    let Calcit::List(parts) = item else { return None };
    let head = parts.first()?;
    if matches!(head, Calcit::Syntax(CalcitSyntax::HintFn, _))
      || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "hint-fn")
    {
      parts.get(1)
    } else {
      None
    }
  })?;
  let Calcit::List(items) = spec else { return None };
  let head = items.first()?;
  if matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "Fn")
    || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "Fn")
  {
    let mut args = Vec::new();
    for item in items.iter().skip(1) {
      if let Calcit::List(clause) = item
        && matches!(clause.first(), Some(Calcit::Tag(_)))
      {
        continue;
      }
      if let Calcit::List(clause) = item
        && matches!(clause.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "->")
      {
        continue;
      }
      args.push(item.clone());
    }
    return Some(args);
  }
  if matches!(head, Calcit::Proc(CalcitProc::NativeMap)) || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "{}") {
    for pair in items.iter().skip(1) {
      if let Calcit::List(kv) = pair
        && matches!(kv.first(), Some(Calcit::Tag(tag)) if tag.ref_str() == "args")
        && let Some(Calcit::List(arg_list)) = kv.get(1)
      {
        let skip = usize::from(
          matches!(arg_list.first(), Some(Calcit::Proc(CalcitProc::List)))
            || arg_list
              .first()
              .is_some_and(|h| is_core_symbol_or_import(scan.program, scan.ns, scan.file, h, "[]")),
        );
        return Some(arg_list.iter().skip(skip).cloned().collect());
      }
    }
  }
  None
}

fn hint_fn_return_node(scan: NonCoherentScanCtx<'_>, body: &[Calcit]) -> Option<Calcit> {
  let spec = body.iter().find_map(|item| {
    let Calcit::List(parts) = item else { return None };
    let head = parts.first()?;
    if matches!(head, Calcit::Syntax(CalcitSyntax::HintFn, _))
      || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "hint-fn")
    {
      parts.get(1)
    } else {
      None
    }
  })?;
  let Calcit::List(items) = spec else { return None };
  let head = items.first()?;
  if matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "Fn")
    || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "Fn")
  {
    for item in items.iter().skip(1) {
      if let Calcit::List(clause) = item
        && matches!(clause.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "->")
      {
        return clause.get(1).cloned();
      }
    }
  }
  if matches!(head, Calcit::Proc(CalcitProc::NativeMap)) || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "{}") {
    for pair in items.iter().skip(1) {
      if let Calcit::List(kv) = pair
        && matches!(kv.first(), Some(Calcit::Tag(tag)) if tag.ref_str() == "return")
      {
        return kv.get(1).cloned();
      }
    }
  }
  None
}

fn bind_fn_param_nominals(
  scan: NonCoherentScanCtx<'_>,
  params: &CalcitList,
  schema: Option<&CalcitTypeAnnotation>,
  body: &[Calcit],
  local_bindings: &mut HashMap<Arc<str>, Vec<DefKey>>,
) {
  let param_names = fn_param_names_from_list(params);
  for param in &param_names {
    local_bindings.insert(param.clone(), Vec::new());
  }
  if let Some(arg_nodes) = hint_fn_arg_nodes(scan, body) {
    for (param, arg_node) in param_names.iter().zip(arg_nodes.iter()) {
      if let Some(key) = type_node_nominal_target(scan, arg_node) {
        local_bindings.insert(param.clone(), vec![key]);
      }
    }
  } else if let Some(fn_ann) = schema.and_then(|ann| ann.resolve_to_fn()) {
    for (param, arg_ty) in param_names.iter().zip(fn_ann.arg_types.iter()) {
      if let Some(key) = nominal_key_from_annotation(scan.program, scan.ns, scan.file, arg_ty.as_ref(), scan.written) {
        local_bindings.insert(param.clone(), vec![key]);
      }
    }
  }
  for item in body {
    let Calcit::List(parts) = item else { continue };
    let Some(head) = parts.first() else { continue };
    if (matches!(head, Calcit::Syntax(CalcitSyntax::AssertType, _))
      || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "assert-type"))
      && let (Some(target), Some(ty_expr)) = (parts.get(1), parts.get(2))
      && let Some(param) = match target {
        Calcit::Symbol { sym, .. } => Some(sym),
        Calcit::Local(local) => Some(&local.sym),
        _ => None,
      }
      && param_names.contains(param)
      && let Some(key) = type_node_nominal_target(scan, ty_expr)
    {
      local_bindings.insert(param.clone(), vec![key]);
    }
  }
}

fn resolve_call_return_nominal_targets(
  scan: NonCoherentScanCtx<'_>,
  head: &Calcit,
  local_bindings: &HashMap<Arc<str>, Vec<DefKey>>,
  visited_calls: &mut HashSet<DefKey>,
) -> Vec<DefKey> {
  if let Calcit::Symbol { sym, .. } = head
    && let Some(targets) = local_bindings.get(sym.as_ref())
  {
    return targets.clone();
  }
  let direct_nominal = match head {
    Calcit::Symbol { sym, .. } => resolve_nominal_target(scan.program, scan.ns, scan.file, sym.as_ref(), scan.written),
    Calcit::Import(import) => resolve_nominal_target(
      scan.program,
      scan.ns,
      scan.file,
      &format!("{}/{}", import.ns, import.def),
      scan.written,
    ),
    _ => None,
  };
  if let Some(key) = direct_nominal {
    return vec![key];
  }
  if let Calcit::Fn { info, .. } = head
    && let Some(target_file) = scan.program.get(info.def_ns.as_ref())
    && let Some(key) = nominal_key_from_annotation(scan.program, &info.def_ns, target_file, info.return_type.as_ref(), scan.written)
  {
    return vec![key];
  }
  let callee_key = match head {
    Calcit::Symbol { sym, .. } => resolve_in(scan.program, scan.ns, scan.file, sym.as_ref()),
    Calcit::Import(import) => Some((import.ns.clone(), import.def.clone())),
    _ => None,
  };
  let Some(callee_key) = callee_key else {
    return Vec::new();
  };
  if !visited_calls.insert(callee_key.clone()) {
    return Vec::new();
  }
  let result = (|| {
    let target_file = scan.program.get(&callee_key.0)?;
    let entry = target_file.defs.get(&callee_key.1)?;
    let callee_scan = NonCoherentScanCtx {
      program: scan.program,
      ns: &callee_key.0,
      file: target_file,
      written: scan.written,
    };
    if let Some(fn_ann) = entry.schema.resolve_to_fn()
      && let Some(key) =
        nominal_key_from_annotation(scan.program, &callee_key.0, target_file, fn_ann.return_type.as_ref(), scan.written)
    {
      return Some(vec![key]);
    }
    let code = effective_written_code(&callee_key, &entry.code, scan.written);
    let Calcit::List(items) = code else {
      return None;
    };
    let def_head = items.first()?;
    if (matches!(
      def_head,
      Calcit::Syntax(CalcitSyntax::Defn | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport, _)
    ) || is_core_symbol_or_import(scan.program, &callee_key.0, target_file, def_head, "defn"))
      && let Some(Calcit::List(params)) = items.get(2)
    {
      let body: Vec<Calcit> = items.iter().skip(3).cloned().collect();
      if let Some(ret_node) = hint_fn_return_node(callee_scan, &body)
        && let Some(key) = type_node_nominal_target(callee_scan, &ret_node)
      {
        return Some(vec![key]);
      }
      if let Some(tail) = body.last() {
        let mut callee_bindings = HashMap::new();
        bind_fn_param_nominals(callee_scan, params, Some(entry.schema.as_ref()), &body, &mut callee_bindings);
        return Some(expr_nominal_targets_inner(callee_scan, tail, &callee_bindings, visited_calls));
      }
    }
    None
  })();
  visited_calls.remove(&callee_key);
  result.unwrap_or_default()
}

fn expr_nominal_targets(
  program: &ProgramCodeData,
  ns: &str,
  file: &ProgramFileData,
  expr: &Calcit,
  local_bindings: &HashMap<Arc<str>, Vec<DefKey>>,
  written: &HashMap<DefKey, (Calcit, Calcit)>,
) -> Vec<DefKey> {
  expr_nominal_targets_inner(
    NonCoherentScanCtx {
      program,
      ns,
      file,
      written,
    },
    expr,
    local_bindings,
    &mut HashSet::new(),
  )
}

fn expr_nominal_targets_inner(
  scan: NonCoherentScanCtx<'_>,
  expr: &Calcit,
  local_bindings: &HashMap<Arc<str>, Vec<DefKey>>,
  visited_calls: &mut HashSet<DefKey>,
) -> Vec<DefKey> {
  let program = scan.program;
  let ns = scan.ns;
  let file = scan.file;
  let written = scan.written;
  match expr {
    Calcit::Symbol { sym, .. } => {
      if let Some(targets) = local_bindings.get(sym.as_ref()) {
        return targets.clone();
      }
      resolve_nominal_target(program, ns, file, sym.as_ref(), written)
        .into_iter()
        .collect()
    }
    Calcit::Local(local) => {
      if let Some(targets) = local_bindings.get(&local.sym) {
        return targets.clone();
      }
      nominal_key_from_annotation(program, ns, file, local.type_info.as_ref(), written)
        .into_iter()
        .collect()
    }
    Calcit::Import(import) => resolve_nominal_target(program, ns, file, &format!("{}/{}", import.ns, import.def), written)
      .or_else(|| {
        program
          .get(&import.ns)
          .and_then(|target_file| target_file.defs.get(&import.def))
          .and_then(|entry| {
            let key = (import.ns.clone(), import.def.clone());
            let code = effective_written_code(&key, &entry.code, written);
            (nominal_form(code).is_some() || code_defines_nominal_type(code)).then_some(key)
          })
      })
      .into_iter()
      .collect(),
    Calcit::Struct(s) => s
      .struct_ref
      .definition_ref
      .as_deref()
      .and_then(|r| r.rsplit_once('/'))
      .map(|(n, d)| (Arc::from(n), Arc::from(d)))
      .into_iter()
      .collect(),
    Calcit::StructDef(s) => s
      .definition_ref
      .as_deref()
      .and_then(|r| r.rsplit_once('/'))
      .map(|(n, d)| (Arc::from(n), Arc::from(d)))
      .into_iter()
      .collect(),
    Calcit::Enum(e) => e
      .sum_type
      .as_ref()
      .and_then(|d| d.definition_ref())
      .and_then(|r| r.rsplit_once('/'))
      .map(|(n, d)| (Arc::from(n), Arc::from(d)))
      .into_iter()
      .collect(),
    Calcit::EnumDef(e) => e
      .definition_ref()
      .and_then(|r| r.rsplit_once('/'))
      .map(|(n, d)| (Arc::from(n), Arc::from(d)))
      .into_iter()
      .collect(),
    Calcit::List(items) => {
      let Some(head) = items.first() else {
        return Vec::new();
      };
      if is_impl_attach_head(program, ns, file, head) {
        return items
          .get(1)
          .map(|base| expr_nominal_targets_inner(scan, base, local_bindings, visited_calls))
          .unwrap_or_default();
      }
      if matches!(head, Calcit::Syntax(CalcitSyntax::If, _)) || is_core_symbol_or_import(program, ns, file, head, "if") {
        let mut out = Vec::new();
        for branch in [items.get(2), items.get(3)].into_iter().flatten() {
          out.extend(expr_nominal_targets_inner(scan, branch, local_bindings, visited_calls));
        }
        return out;
      }
      if is_core_symbol_or_import(program, ns, file, head, "do") {
        return items
          .iter()
          .last()
          .map(|tail| expr_nominal_targets_inner(scan, tail, local_bindings, visited_calls))
          .unwrap_or_default();
      }
      if matches!(head, Calcit::Syntax(CalcitSyntax::CoreLet, _))
        || is_core_symbol_or_import(program, ns, file, head, "let")
        || is_core_symbol_or_import(program, ns, file, head, "&let")
      {
        let mut scoped = local_bindings.clone();
        if let Some(Calcit::List(bindings)) = items.get(1) {
          let mut apply_pair = |pair: &CalcitList| {
            if let (Some(lhs), Some(rhs)) = (pair.first(), pair.get(1))
              && let Some(name) = match lhs {
                Calcit::Symbol { sym, .. } => Some(sym.clone()),
                Calcit::Local(local) => Some(local.sym.clone()),
                _ => None,
              }
            {
              let targets = expr_nominal_targets_inner(scan, rhs, &scoped, visited_calls);
              scoped.insert(name, targets);
            }
          };
          if bindings.len() == 2 && !matches!(bindings.first(), Some(Calcit::List(_))) {
            apply_pair(bindings);
          } else {
            for entry in bindings.iter() {
              if let Calcit::List(pair) = entry {
                apply_pair(pair);
              }
            }
          }
        }
        return items
          .iter()
          .last()
          .map(|tail| expr_nominal_targets_inner(scan, tail, &scoped, visited_calls))
          .unwrap_or_default();
      }
      if matches!(
        head,
        Calcit::Proc(
          CalcitProc::NativeStruct
            | CalcitProc::NativeStructFromMap
            | CalcitProc::NativeNamedEnumNew
            | CalcitProc::NativeStructAssoc
            | CalcitProc::NativeStructAssocAt
            | CalcitProc::NativeStructWith
            | CalcitProc::NativeStructWithAt
        )
      ) || is_core_symbol_or_import(program, ns, file, head, "%{}")
        || is_core_symbol_or_import(program, ns, file, head, "%::")
        || is_core_symbol_or_import(program, ns, file, head, "&*::new")
        || is_core_symbol_or_import(program, ns, file, head, "&struct:new")
        || is_core_symbol_or_import(program, ns, file, head, "&struct:from-map")
        || is_core_symbol_or_import(program, ns, file, head, "&struct:assoc")
        || is_core_symbol_or_import(program, ns, file, head, "&struct:with")
        || is_core_symbol_or_import(program, ns, file, head, "&enum:new")
        || is_core_symbol_or_import(program, ns, file, head, "update")
      {
        return items
          .get(1)
          .map(|base| expr_nominal_targets_inner(scan, base, local_bindings, visited_calls))
          .unwrap_or_default();
      }
      // Constructor call `(Point :x 1)` / `(Option :some 1)` or factory call `(make-p)`.
      resolve_call_return_nominal_targets(scan, head, local_bindings, visited_calls)
    }
    _ => Vec::new(),
  }
}

fn walk_expr_for_non_coherent(
  scan: NonCoherentScanCtx<'_>,
  expr: &Calcit,
  local_bindings: &HashMap<Arc<str>, Vec<DefKey>>,
  fn_schema: Option<&CalcitTypeAnnotation>,
  out: &mut HashSet<String>,
) {
  let Calcit::List(items) = expr else { return };
  let Some(head) = items.first() else { return };
  if matches!(head, Calcit::Syntax(CalcitSyntax::Quote, _)) || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "quote")
  {
    return;
  }
  if is_impl_attach_head(scan.program, scan.ns, scan.file, head)
    && let Some(base) = items.get(1)
  {
    for (target_ns, target_def) in expr_nominal_targets(scan.program, scan.ns, scan.file, base, local_bindings, scan.written) {
      out.insert(format!("{target_ns}/{target_def}"));
    }
  }
  if (matches!(
    head,
    Calcit::Syntax(CalcitSyntax::Defn | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport, _)
  ) || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "defn"))
    && let Some(Calcit::List(params)) = items.get(2)
  {
    let mut scoped = local_bindings.clone();
    let body: Vec<Calcit> = items.iter().skip(3).cloned().collect();
    bind_fn_param_nominals(scan, params, fn_schema, &body, &mut scoped);
    for item in &body {
      walk_expr_for_non_coherent(scan, item, &scoped, None, out);
    }
    return;
  }
  if matches!(head, Calcit::Syntax(CalcitSyntax::CoreLet, _))
    || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "let")
    || is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "&let")
  {
    let mut scoped = local_bindings.clone();
    if let Some(Calcit::List(bindings)) = items.get(1) {
      let visit_pair = |pair: &CalcitList, scoped: &mut HashMap<Arc<str>, Vec<DefKey>>, out: &mut HashSet<String>| {
        if let Some(rhs) = pair.get(1) {
          walk_expr_for_non_coherent(scan, rhs, scoped, None, out);
          if let Some(name) = match pair.first() {
            Some(Calcit::Symbol { sym, .. }) => Some(sym.clone()),
            Some(Calcit::Local(local)) => Some(local.sym.clone()),
            _ => None,
          } {
            let targets = expr_nominal_targets(scan.program, scan.ns, scan.file, rhs, scoped, scan.written);
            scoped.insert(name, targets);
          }
        }
      };
      if bindings.len() == 2 && !matches!(bindings.first(), Some(Calcit::List(_))) {
        visit_pair(bindings, &mut scoped, out);
      } else {
        for entry in bindings.iter() {
          if let Calcit::List(pair) = entry {
            visit_pair(pair, &mut scoped, out);
          } else {
            walk_expr_for_non_coherent(scan, entry, &scoped, None, out);
          }
        }
      }
    }
    for item in items.iter().skip(2) {
      walk_expr_for_non_coherent(scan, item, &scoped, None, out);
    }
    return;
  }
  if matches!(head, Calcit::Fn { .. }) {
    // Nested fn expression? Usually `(fn (args) body...)` is `Syntax::Defn` or `Symbol("fn")`.
  }
  if is_core_symbol_or_import(scan.program, scan.ns, scan.file, head, "fn")
    && let Some(Calcit::List(params)) = items.get(1)
  {
    let mut scoped = local_bindings.clone();
    let body: Vec<Calcit> = items.iter().skip(2).cloned().collect();
    bind_fn_param_nominals(scan, params, None, &body, &mut scoped);
    for item in &body {
      walk_expr_for_non_coherent(scan, item, &scoped, None, out);
    }
    return;
  }
  for item in items.iter() {
    walk_expr_for_non_coherent(scan, item, local_bindings, None, out);
  }
}

fn collect_non_coherent_nominal_defs_inner(
  program: &ProgramCodeData,
  written: &HashMap<DefKey, (Calcit, Calcit)>,
  rewrites: &HashMap<DefKey, (Calcit, Calcit)>,
) -> HashSet<String> {
  let mut out = HashSet::new();
  for (ns, file) in program {
    for (def_name, entry) in &file.defs {
      let key: DefKey = (ns.to_owned(), def_name.to_owned());
      let generated_match = rewrites.get(&key).is_some_and(|(_, generated)| *generated == entry.code)
        || written.get(&key).is_some_and(|(_, generated)| *generated == entry.code);
      let core_top_level_wrapper =
        calcit::is_bundled_core_ns(ns) && code_is_top_level_impl_traits(&entry.code) && code_defines_nominal_type(&entry.code);
      if !generated_match
        && !core_top_level_wrapper
        && code_is_top_level_impl_traits(&entry.code)
        && code_defines_nominal_type(&entry.code)
      {
        out.insert(format!("{ns}/{def_name}"));
      }
      let root = if generated_match {
        rewrites
          .get(&key)
          .or_else(|| written.get(&key))
          .map(|(original, _)| original)
          .unwrap_or(&entry.code)
      } else if core_top_level_wrapper {
        // Core wraps top-level definitions such as Option and Result with `def Name (impl-traits (defenum ...) ...)`.
        // Skip that top-level attachment node itself while still scanning any inner expressions.
        if let Calcit::List(items) = &entry.code
          && items.first().and_then(symbol_name) == Some("def")
          && let Some(Calcit::List(wrap)) = items.get(2)
          && let Some(inner) = wrap.get(1)
        {
          inner
        } else {
          &entry.code
        }
      } else {
        &entry.code
      };
      walk_expr_for_non_coherent(
        NonCoherentScanCtx {
          program,
          ns,
          file,
          written,
        },
        root,
        &HashMap::new(),
        Some(entry.schema.as_ref()),
        &mut out,
      );
    }
  }
  out
}

/// Collect nominal `ns/def` keys that appear as the base of any non-generated
/// `impl-traits` / `&struct:impl-traits` / `&enum:impl-traits` / `&struct-def:impl-traits` / `&enum-def:impl-traits` call.
pub(crate) fn collect_non_coherent_nominal_defs(program: &ProgramCodeData) -> HashSet<String> {
  let written = WRITTEN_CODE.lock().expect("lock written type code").clone();
  collect_non_coherent_nominal_defs_inner(program, &written, &HashMap::new())
}

/// View the written form of a definition without mutating `program` before validation succeeds.
fn effective_written_code<'a>(key: &DefKey, current: &'a Calcit, written: &'a HashMap<DefKey, (Calcit, Calcit)>) -> &'a Calcit {
  match written.get(key) {
    Some((original, generated)) if current == generated => original,
    _ => current,
  }
}

/// Validated `(:impls …)` rewrites waiting to be committed to live program state.
pub(crate) struct StagedSourceImpls {
  rewrites: HashMap<DefKey, (Calcit, Calcit)>,
}

/// Validate and stage all `(:impls …)` attachments on `program` without mutating
/// `program` or global attachment state if any check fails.
pub(crate) fn stage_source_impls(program: &ProgramCodeData) -> Result<StagedSourceImpls, String> {
  let written = WRITTEN_CODE.lock().expect("lock written type code").clone();

  let mut rewrites: HashMap<DefKey, (Calcit, Calcit)> = HashMap::new();
  let mut namespaces: Vec<&Arc<str>> = program.keys().collect();
  namespaces.sort();
  for &ns in &namespaces {
    let file = &program[ns];
    let mut defs: Vec<&Arc<str>> = file.defs.keys().collect();
    defs.sort();
    for def in defs {
      let type_key: DefKey = (ns.to_owned(), def.to_owned());
      let code = effective_written_code(&type_key, &file.defs[def].code, &written);
      if nominal_form(code).is_none() {
        continue;
      }
      let Some((traits, without_clause)) = take_impls_clause(code) else {
        continue;
      };
      // Check all listed entries for visibility and origin uniqueness before any
      // `defimpl` lookup so errors stay independent of entry order.
      let mut resolved_traits: Vec<(&str, DefKey)> = Vec::with_capacity(traits.len());
      let mut seen_origins: Vec<DefKey> = Vec::with_capacity(traits.len());
      for trait_form in &traits {
        let Some(trait_name) = symbol_name(trait_form) else {
          return Err(format!(
            "[{UNKNOWN_IMPLS_TRAIT_ERROR}] {ns}/{def}: `(:impls …)` lists trait names, got `{trait_form}`"
          ));
        };
        let Some(trait_key) = resolve_in(program, ns, file, trait_name) else {
          return Err(format!(
            "[{UNKNOWN_IMPLS_TRAIT_ERROR}] {ns}/{def}: `(:impls {trait_name})` does not name a trait visible in `{ns}`; import it or write `namespace/Trait`"
          ));
        };
        let origin_key = resolve_trait_origin(program, &trait_key);
        if seen_origins.contains(&origin_key) {
          return Err(format!(
            "[{DUPLICATE_TRAIT_IMPL_ERROR}] {ns}/{def} lists {}/{} more than once in `(:impls …)`; remove the duplicate trait entry",
            origin_key.0, origin_key.1
          ));
        }
        seen_origins.push(origin_key);
        resolved_traits.push((trait_name, trait_key));
      }

      let mut refs = Vec::with_capacity(resolved_traits.len());
      for (trait_name, trait_key) in resolved_traits {
        let found = find_impl(program, &type_key, &trait_key);
        match found.as_slice() {
          [] => {
            return Err(format!(
              "[{MISSING_TRAIT_IMPL_ERROR}] {ns}/{def} lists `{trait_name}` in `(:impls …)`, but neither `{}` nor `{}` has a `defimpl {def} {trait_name} …`; write it in one of these namespaces",
              type_key.0, trait_key.0
            ));
          }
          [(impl_ns, impl_def)] => refs.push(impl_reference(code, impl_ns, impl_def, ns)),
          [(a_ns, a_def), (b_ns, b_def), ..] => {
            return Err(format!(
              "[{DUPLICATE_TRAIT_IMPL_ERROR}] {ns}/{def} implements {}/{} twice: {a_ns}/{a_def} and {b_ns}/{b_def}; a type has at most one implementation of a trait, remove one of them",
              trait_key.0, trait_key.1
            ));
          }
        }
      }
      let generated = if refs.is_empty() {
        without_clause
      } else {
        attach(&without_clause, &refs)
      };
      rewrites.insert(type_key, (code.to_owned(), generated));
    }
  }

  let non_coherent = collect_non_coherent_nominal_defs_inner(program, &written, &rewrites);
  let mut coherent_impls_by_pair: HashMap<(DefKey, DefKey), Vec<DefKey>> = HashMap::new();
  for ns in &namespaces {
    let file = &program[*ns];
    let mut defs: Vec<&Arc<str>> = file.defs.keys().collect();
    defs.sort();
    for def in defs {
      let impl_key: DefKey = ((*ns).to_owned(), def.to_owned());
      let code = effective_written_code(&impl_key, &file.defs[def].code, &written);
      let Some((type_name, trait_name)) = defimpl_type_and_trait(code) else {
        continue;
      };
      let Some(type_key) = resolve_nominal_target(program, ns, file, type_name, &written) else {
        continue;
      };
      if non_coherent.contains(&format!("{}/{}", type_key.0, type_key.1)) {
        continue;
      }
      let Some(trait_key) = resolve_in(program, ns, file, trait_name) else {
        continue;
      };
      let origin_key = resolve_trait_origin(program, &trait_key);
      coherent_impls_by_pair.entry((type_key, origin_key)).or_default().push(impl_key);
    }
  }
  let mut duplicate_pairs: Vec<_> = coherent_impls_by_pair.into_iter().filter(|(_, impls)| impls.len() >= 2).collect();
  duplicate_pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
  if let Some(((type_key, origin_key), impls)) = duplicate_pairs.into_iter().next() {
    let (a_ns, a_def) = &impls[0];
    let (b_ns, b_def) = &impls[1];
    return Err(format!(
      "[{DUPLICATE_TRAIT_IMPL_ERROR}] {}/{} implements {}/{} twice: {a_ns}/{a_def} and {b_ns}/{b_def}; a coherent type has at most one implementation of a trait, remove one of them",
      type_key.0, type_key.1, origin_key.0, origin_key.1
    ));
  }

  Ok(StagedSourceImpls { rewrites })
}

/// Commit staged `(:impls …)` rewrites to `program` and record the active
/// written-source map. Previously attached definitions that no longer have an
/// `(:impls …)` clause are restored to their written code unless already edited.
pub(crate) fn commit_staged_impls(program: &mut ProgramCodeData, staged: StagedSourceImpls) -> Vec<DefKey> {
  let mut written = WRITTEN_CODE.lock().expect("lock written type code");
  for ((ns, def), (original, old_generated)) in written.iter() {
    if !staged.rewrites.contains_key(&(ns.to_owned(), def.to_owned()))
      && let Some(entry) = program.get_mut(ns).and_then(|file| file.defs.get_mut(def))
      && entry.code == *old_generated
    {
      original.clone_into(&mut entry.code);
    }
  }
  let mut attached = Vec::with_capacity(staged.rewrites.len());
  for (key, (_, generated)) in &staged.rewrites {
    let Some(entry) = program.get_mut(&key.0).and_then(|file| file.defs.get_mut(&key.1)) else {
      continue;
    };
    generated.clone_into(&mut entry.code);
    attached.push(key.to_owned());
  }
  *written = staged.rewrites;
  attached.sort();
  attached
}

/// Attach the implementations named by each type's `(:impls …)` clause.
/// Returns the definitions whose code was rewritten.
pub(crate) fn attach_source_impls(program: &mut ProgramCodeData) -> Result<Vec<DefKey>, String> {
  let staged = stage_source_impls(program)?;
  Ok(commit_staged_impls(program, staged))
}

/// Definitions currently carrying generated code, with that code.
pub(crate) fn generated_attachments() -> HashMap<DefKey, Calcit> {
  WRITTEN_CODE
    .lock()
    .expect("lock written type code")
    .iter()
    .map(|(key, (_, generated))| (key.to_owned(), generated.to_owned()))
    .collect()
}

/// Whether the code, as written or attached, defines a struct or enum.
pub(crate) fn code_defines_nominal_type(code: &Calcit) -> bool {
  let Calcit::List(items) = code else { return false };
  match items.first().and_then(symbol_name) {
    Some("defstruct" | "defenum") => true,
    Some("def") => items.get(2).is_some_and(code_defines_nominal_type),
    Some("impl-traits" | GENERATED_ATTACH_HEAD) => items.get(1).is_some_and(code_defines_nominal_type),
    _ => false,
  }
}

/// Whether the code is attachment at the top of a definition, `impl-traits …`
/// or `def X (impl-traits …)`, with the plain or the qualified head.
pub(crate) fn code_is_top_level_impl_traits(code: &Calcit) -> bool {
  let Calcit::List(items) = code else { return false };
  match items.first().and_then(symbol_name) {
    Some("impl-traits" | GENERATED_ATTACH_HEAD) => true,
    Some("def") => items.get(2).is_some_and(code_is_top_level_impl_traits),
    _ => false,
  }
}

/// Whether the definition's current code is attachment written by hand: an
/// `impl-traits` head that this module did not generate from `(:impls …)`.
pub(crate) fn is_hand_written_top_level_impl_traits(ns: &str, def: &str, code: &Calcit) -> bool {
  if !code_is_top_level_impl_traits(code) {
    return false;
  }
  let key: DefKey = (Arc::from(ns), Arc::from(def));
  let generated = WRITTEN_CODE
    .lock()
    .expect("lock written type code")
    .get(&key)
    .is_some_and(|(_, generated)| generated == code);
  !generated
}
