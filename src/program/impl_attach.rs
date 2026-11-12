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

use crate::calcit::{self, Calcit};

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

/// Put back the code as written for definitions this module attached.
fn restore_written_code(program: &mut ProgramCodeData) {
  let mut written = WRITTEN_CODE.lock().expect("lock written type code");
  written.retain(|(ns, def), (original, generated)| {
    if let Some(entry) = program.get_mut(ns).and_then(|file| file.defs.get_mut(def))
      && entry.code == *generated
    {
      entry.code = original.to_owned();
    }
    false
  });
}

/// Attach the implementations named by each type's `(:impls …)` clause.
/// Returns the definitions whose code was rewritten.
pub(crate) fn attach_source_impls(program: &mut ProgramCodeData) -> Result<Vec<DefKey>, String> {
  restore_written_code(program);

  let mut rewrites: Vec<(DefKey, Calcit)> = vec![];
  let mut namespaces: Vec<&Arc<str>> = program.keys().collect();
  namespaces.sort();
  for ns in namespaces {
    let file = &program[ns];
    let mut defs: Vec<&Arc<str>> = file.defs.keys().collect();
    defs.sort();
    for def in defs {
      let code = &file.defs[def].code;
      if nominal_form(code).is_none() {
        continue;
      }
      let Some((traits, without_clause)) = take_impls_clause(code) else {
        continue;
      };
      let type_key: DefKey = (ns.to_owned(), def.to_owned());
      let mut refs = Vec::with_capacity(traits.len());
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
      rewrites.push((type_key, generated));
    }
  }

  let mut written = WRITTEN_CODE.lock().expect("lock written type code");
  let mut attached = Vec::with_capacity(rewrites.len());
  for (key, generated) in rewrites {
    let Some(entry) = program.get_mut(&key.0).and_then(|file| file.defs.get_mut(&key.1)) else {
      continue;
    };
    let original = std::mem::replace(&mut entry.code, generated.to_owned());
    written.insert(key.clone(), (original, generated));
    attached.push(key);
  }
  attached.sort();
  Ok(attached)
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
