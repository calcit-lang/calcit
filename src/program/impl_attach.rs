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

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use crate::calcit::{self, Calcit, CalcitImport, CalcitProc, CalcitSyntax};

use super::{ImportRule, ProgramCodeData, ProgramFileData};

const MISSING_TRAIT_IMPL_ERROR: &str = "E_MISSING_TRAIT_IMPL";
const DUPLICATE_TRAIT_IMPL_ERROR: &str = "E_DUPLICATE_TRAIT_IMPL";
const UNKNOWN_IMPLS_TRAIT_ERROR: &str = "E_UNKNOWN_IMPLS_TRAIT";
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
    let is_enum = matches!(
      value,
      Calcit::List(xs) if xs.first().and_then(symbol_name) == Some("defenum")
    );
    let head = Calcit::Proc(if is_enum {
      CalcitProc::NativeEnumImplTraits
    } else {
      CalcitProc::NativeStructImplTraits
    });
    let mut items = vec![head, value.to_owned()];
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
/// namespaces of the type, the trait reference, and the trait's resolved origin.
fn find_impl(program: &ProgramCodeData, type_key: &DefKey, trait_key: &DefKey) -> Vec<DefKey> {
  let trait_origin = resolve_trait_origin(program, trait_key);
  let mut namespaces = vec![&type_key.0];
  if trait_key.0 != type_key.0 {
    namespaces.push(&trait_key.0);
  }
  if trait_origin.0 != type_key.0 && trait_origin.0 != trait_key.0 {
    namespaces.push(&trait_origin.0);
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
        && resolve_in(program, ns, file, trait_name)
          .map(|key| resolve_trait_origin(program, &key))
          .as_ref()
          == Some(&trait_origin)
      {
        found.push((ns.to_owned(), def.to_owned()));
      }
    }
  }
  found
}

fn peel_trait_alias_target_form(code: &Calcit) -> Option<&Calcit> {
  let mut current = code;
  loop {
    match current {
      Calcit::Thunk(thunk) => current = thunk.get_code(),
      Calcit::List(items)
        if items.len() == 2
          && (matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Quote, _)))
            || matches!(items.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "quote")
            || matches!(
              items.first(),
              Some(Calcit::Import(CalcitImport { ns, def, .. })) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "quote"
            )) =>
      {
        current = items.get(1)?;
      }
      Calcit::List(items)
        if items.len() == 3
          && (items.first().and_then(symbol_name) == Some("def")
            || matches!(
              items.first(),
              Some(Calcit::Import(CalcitImport { ns, def, .. })) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "def"
            )) =>
      {
        return items.get(2);
      }
      other => return Some(other),
    }
  }
}

/// Follow simple top-level alias definitions (`def Alias Target`) to the underlying nominal trait key.
fn resolve_trait_origin(program: &ProgramCodeData, start: &DefKey) -> DefKey {
  let mut current = start.clone();
  let mut visited = vec![start.clone()];
  while let Some(file) = program.get(&current.0)
    && let Some(entry) = file.defs.get(&current.1)
  {
    let Some(target) = peel_trait_alias_target_form(&entry.code) else {
      break;
    };
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

/// Resolve a trait `(ns, def)` through any top-level `def Alias Target` chain
/// against the live program code table.
pub(crate) fn resolve_trait_origin_in_program(ns: &str, def: &str) -> DefKey {
  let program = super::PROGRAM_CODE_DATA.read().expect("read program code");
  resolve_trait_origin(&program, &(Arc::from(ns), Arc::from(def)))
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
  stage_source_impls_with_written(program, &written)
}

fn stage_source_impls_with_written(
  program: &ProgramCodeData,
  written: &HashMap<DefKey, (Calcit, Calcit)>,
) -> Result<StagedSourceImpls, String> {
  let mut rewrites: HashMap<DefKey, (Calcit, Calcit)> = HashMap::new();
  let mut namespaces: Vec<&Arc<str>> = program.keys().collect();
  namespaces.sort();
  for &ns in &namespaces {
    let file = &program[ns];
    let mut defs: Vec<&Arc<str>> = file.defs.keys().collect();
    defs.sort();
    for def in defs {
      let type_key: DefKey = (ns.to_owned(), def.to_owned());
      let code = effective_written_code(&type_key, &file.defs[def].code, written);
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

  let mut coherent_impls_by_pair: HashMap<(DefKey, DefKey), Vec<DefKey>> = HashMap::new();
  for ns in &namespaces {
    let file = &program[*ns];
    let mut defs: Vec<&Arc<str>> = file.defs.keys().collect();
    defs.sort();
    for def in defs {
      let impl_key: DefKey = ((*ns).to_owned(), def.to_owned());
      let code = effective_written_code(&impl_key, &file.defs[def].code, written);
      let Some((type_name, trait_name)) = defimpl_type_and_trait(code) else {
        continue;
      };
      let Some(type_key) = resolve_nominal_target(program, ns, file, type_name, written) else {
        continue;
      };
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

/// Attach `(:impls …)` directly on `program` without touching global `WRITTEN_CODE`.
pub(crate) fn attach_source_impls_detached(program: &mut ProgramCodeData) -> Result<(), String> {
  let empty = HashMap::new();
  let staged = stage_source_impls_with_written(program, &empty)?;
  for (key, (_, generated)) in &staged.rewrites {
    if let Some(entry) = program.get_mut(&key.0).and_then(|file| file.defs.get_mut(&key.1)) {
      generated.clone_into(&mut entry.code);
    }
  }
  Ok(())
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
  if matches!(
    items.first(),
    Some(Calcit::Proc(CalcitProc::NativeStructImplTraits | CalcitProc::NativeEnumImplTraits))
  ) {
    return items.get(1).is_some_and(code_defines_nominal_type);
  }
  match items.first().and_then(symbol_name) {
    Some("defstruct" | "defenum") => true,
    Some("def") => items.get(2).is_some_and(code_defines_nominal_type),
    _ => false,
  }
}

/// Return the pre-attachment definition code when `code` is a compiler-generated
/// `(:impls …)` wrapper recorded for `(ns, def)`.
pub(crate) fn written_source_code(ns: &str, def: &str, code: &Calcit) -> Option<Calcit> {
  let key: DefKey = (Arc::from(ns), Arc::from(def));
  WRITTEN_CODE
    .lock()
    .expect("lock written type code")
    .get(&key)
    .and_then(|(written, generated)| (generated == code).then(|| written.clone()))
}
