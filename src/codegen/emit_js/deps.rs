use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::calcit::{Calcit, CalcitImport, CalcitLocal, CalcitSyntax, CalcitTypeAnnotation};
use crate::program::{CompiledDef, CompiledFileData, DefId};

pub(super) fn contains_symbol(xs: &Calcit, symbol: &str) -> bool {
  match xs {
    Calcit::Symbol { sym, .. } => &**sym == symbol,
    Calcit::Local(CalcitLocal { sym, .. }) => sym.as_ref() == symbol,
    Calcit::Import(CalcitImport { def, .. }) => &**def == symbol,
    Calcit::Thunk(thunk) => contains_symbol(thunk.get_code(), symbol),
    Calcit::Fn { info, .. } => info.body.iter().any(|x| contains_symbol(x, symbol)),
    Calcit::List(zs) => zs.iter().any(|z| contains_symbol(z, symbol)),
    _ => false,
  }
}

/// Whether `symbol` is read while the form itself is evaluated, outside the
/// bodies of the functions it defines, which only read it when called.
fn reads_symbol_eagerly(xs: &Calcit, symbol: &str) -> bool {
  match xs {
    Calcit::Fn { .. } => false,
    Calcit::List(zs) if matches!(zs.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) => false,
    Calcit::List(zs) => zs.iter().any(|z| reads_symbol_eagerly(z, symbol)),
    Calcit::Thunk(thunk) => reads_symbol_eagerly(thunk.get_code(), symbol),
    _ => contains_symbol(xs, symbol),
  }
}

/// Whether a compiled definition is a `defimpl` value, by its source head or its `Impl` schema.
fn is_impl_definition(compiled: &CompiledDef) -> bool {
  let source_is_defimpl = compiled.source_code.as_ref().is_some_and(|code| match code {
    Calcit::List(items) => match items.first() {
      Some(Calcit::Symbol { sym, .. }) => sym.as_ref() == "defimpl" || sym.as_ref() == "calcit.core/defimpl",
      Some(Calcit::Import(CalcitImport { def, .. })) => def.as_ref() == "defimpl",
      _ => false,
    },
    _ => false,
  });
  source_is_defimpl
    || matches!(compiled.schema.as_ref(), CalcitTypeAnnotation::Custom(value) if matches!(value.as_ref(), Calcit::Tag(tag) if tag.ref_str() == "impl"))
}

fn reaches(from: &Arc<str>, to: &Arc<str>, graph: &HashMap<Arc<str>, HashSet<Arc<str>>>) -> bool {
  let mut stack = vec![from.to_owned()];
  let mut seen: HashSet<Arc<str>> = HashSet::new();
  while let Some(name) = stack.pop() {
    if &name == to {
      return true;
    }
    if seen.insert(name.to_owned()) {
      stack.extend(graph.get(&name).into_iter().flatten().cloned());
    }
  }
  false
}

#[cfg(test)]
pub(super) fn sort_by_deps(deps: &HashMap<Arc<str>, Calcit>) -> Vec<Arc<str>> {
  let mut deps_graph: HashMap<Arc<str>, HashSet<Arc<str>>> = HashMap::new();
  let mut def_names: Vec<Arc<str>> = Vec::with_capacity(deps.len());

  for (name, expr) in deps {
    def_names.push(name.to_owned());
    let mut deps_info: HashSet<Arc<str>> = HashSet::new();
    for candidate in deps.keys() {
      if candidate == name {
        continue;
      }
      if contains_symbol(expr, candidate) {
        deps_info.insert(candidate.to_owned());
      }
    }
    deps_graph.insert(name.to_owned(), deps_info);
  }

  sort_names_by_graph(def_names, &deps_graph)
}

pub(super) fn sort_compiled_defs_by_deps(file: &CompiledFileData) -> Vec<Arc<str>> {
  let mut deps_graph: HashMap<Arc<str>, HashSet<Arc<str>>> = HashMap::new();
  let mut def_names: Vec<Arc<str>> = Vec::with_capacity(file.defs.len());
  let mut local_defs: HashMap<DefId, Arc<str>> = HashMap::with_capacity(file.defs.len());

  for (name, compiled) in &file.defs {
    def_names.push(name.to_owned());
    local_defs.insert(compiled.def_id, name.to_owned());
  }

  for (name, compiled) in &file.defs {
    let mut deps_info: HashSet<Arc<str>> = HashSet::new();
    for dep_id in &compiled.deps {
      if let Some(dep_name) = local_defs.get(dep_id)
        && dep_name != name
      {
        deps_info.insert(dep_name.to_owned());
      }
    }
    deps_graph.insert(name.to_owned(), deps_info);
  }

  // A type and the impl attached to it refer to each other: the type reads the
  // impl while it is built, and the impl's methods read the type only when
  // called. For an impl definition, drop a lazy edge into its own dependency
  // cycle so the impl is emitted before the types that attach it. Only impls
  // are pruned: a plain function may be called while another value is
  // initialized, so the values its body reads must keep their place.
  let full_graph = deps_graph.clone();
  for (name, deps) in deps_graph.iter_mut() {
    let compiled = &file.defs[name];
    if !is_impl_definition(compiled) {
      continue;
    }
    let form = &compiled.codegen_form;
    deps.retain(|dep| reads_symbol_eagerly(form, dep) || !reaches(dep, name, &full_graph));
  }

  sort_names_by_graph(def_names, &deps_graph)
}

fn sort_names_by_graph(mut def_names: Vec<Arc<str>>, deps_graph: &HashMap<Arc<str>, HashSet<Arc<str>>>) -> Vec<Arc<str>> {
  def_names.sort();

  let mut result: Vec<Arc<str>> = Vec::with_capacity(def_names.len());
  let mut visiting: HashSet<Arc<str>> = HashSet::new();
  let mut visited: HashSet<Arc<str>> = HashSet::new();
  for name in &def_names {
    visit_dependency(name, deps_graph, &mut visiting, &mut visited, &mut result);
  }

  result
}

fn visit_dependency(
  name: &Arc<str>,
  deps_graph: &HashMap<Arc<str>, HashSet<Arc<str>>>,
  visiting: &mut HashSet<Arc<str>>,
  visited: &mut HashSet<Arc<str>>,
  result: &mut Vec<Arc<str>>,
) {
  if visited.contains(name) || !visiting.insert(name.clone()) {
    return;
  }

  let mut dependencies: Vec<Arc<str>> = deps_graph
    .get(name)
    .into_iter()
    .flatten()
    .filter(|dependency| deps_graph.contains_key(*dependency))
    .cloned()
    .collect();
  dependencies.sort();
  for dependency in &dependencies {
    visit_dependency(dependency, deps_graph, visiting, visited, result);
  }

  visiting.remove(name);
  if visited.insert(name.clone()) {
    result.push(name.clone());
  }
}

#[cfg(test)]
fn depends_on(x: &str, y: &str, deps: &HashMap<Arc<str>, HashSet<Arc<str>>>, decay: usize) -> bool {
  if decay == 0 {
    return false;
  }

  if let Some(items) = deps.get(x) {
    for item in items {
      if &**item == y || depends_on(item, y, deps, decay - 1) {
        return true;
      }
    }
  }

  false
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitList, CalcitSymbolInfo, DYNAMIC_TYPE};
  use crate::program::{CompiledDef, CompiledDefKind};

  fn symbol(name: &str) -> Calcit {
    Calcit::Symbol {
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("app.main"),
        at_def: Arc::from("fixture"),
      }),
      location: None,
    }
  }

  fn lazy_value(def_id: DefId, codegen_form: Calcit, deps: Vec<DefId>) -> CompiledDef {
    CompiledDef {
      def_id,
      version_id: 0,
      kind: CompiledDefKind::LazyValue,
      preprocessed_code: codegen_form.clone(),
      codegen_form,
      deps,
      type_summary: None,
      source_code: None,
      schema: DYNAMIC_TYPE.clone(),
      doc: Arc::from(""),
      examples: vec![],
    }
  }

  #[test]
  fn depends_on_transitively() {
    let mut graph: HashMap<Arc<str>, HashSet<Arc<str>>> = HashMap::new();
    graph.insert(Arc::<str>::from("a"), HashSet::from([Arc::<str>::from("b")]));
    graph.insert(Arc::<str>::from("b"), HashSet::from([Arc::<str>::from("c")]));
    graph.insert(Arc::<str>::from("c"), HashSet::new());

    assert!(depends_on("a", "c", &graph, 3));
    assert!(!depends_on("c", "a", &graph, 3));
  }

  #[test]
  fn sort_without_edges_uses_lexical_order() {
    let mut deps: HashMap<Arc<str>, Calcit> = HashMap::new();
    deps.insert(Arc::<str>::from("b"), Calcit::Number(2.0));
    deps.insert(Arc::<str>::from("a"), Calcit::Number(1.0));

    let sorted = sort_by_deps(&deps);
    assert_eq!(sorted, vec![Arc::<str>::from("a"), Arc::<str>::from("b")]);
  }

  #[test]
  fn sort_compiled_lazy_values_recovers_codegen_dependencies() {
    let file = CompiledFileData {
      defs: HashMap::from([
        (
          Arc::from("%constructor"),
          lazy_value(
            DefId(4),
            Calcit::List(Arc::new(CalcitList::Vector(vec![symbol("attached-type")]))),
            vec![DefId(1)],
          ),
        ),
        (
          Arc::from("attached-type"),
          lazy_value(
            DefId(1),
            Calcit::List(Arc::new(CalcitList::Vector(vec![
              symbol("impl-traits"),
              symbol("base-type"),
              symbol("trait-impl"),
            ]))),
            vec![DefId(2), DefId(3)],
          ),
        ),
        (Arc::from("base-type"), lazy_value(DefId(2), Calcit::Number(1.0), vec![])),
        (Arc::from("trait-impl"), lazy_value(DefId(3), Calcit::Number(2.0), vec![])),
      ]),
    };

    let sorted = sort_compiled_defs_by_deps(&file);
    let attached_index = sorted.iter().position(|name| name.as_ref() == "attached-type").unwrap();
    let base_index = sorted.iter().position(|name| name.as_ref() == "base-type").unwrap();
    let impl_index = sorted.iter().position(|name| name.as_ref() == "trait-impl").unwrap();
    let constructor_index = sorted.iter().position(|name| name.as_ref() == "%constructor").unwrap();

    assert!(base_index < attached_index);
    assert!(impl_index < attached_index);
    assert!(attached_index < constructor_index);
  }
}
