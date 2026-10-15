use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use crate::calcit::{Calcit, CalcitImport, CalcitLocal, CalcitProc, CalcitSyntax};
use crate::program::{CompiledDefKind, CompiledFileData, CompiledProgram, DefId};

/// Find the native layout helper without interpreting quoted symbol data.
/// Legacy JS warning stubs keep their existing behavior in this bugfix.
pub(super) fn find_js_unavailable_proc(code: &Calcit) -> Option<CalcitProc> {
  match code {
    Calcit::Proc(CalcitProc::NativeFormatTernaryTree) => Some(CalcitProc::NativeFormatTernaryTree),
    Calcit::Thunk(thunk) => find_js_unavailable_proc(thunk.get_code()),
    Calcit::Fn { info, .. } => info.body.iter().find_map(find_js_unavailable_proc),
    // Quoted procs are emitted as symbols, not runtime references.
    Calcit::List(items) if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Quote, _))) => None,
    Calcit::List(items) => items.iter().find_map(find_js_unavailable_proc),
    _ => None,
  }
}

/// Omit only unavailable definitions and their dependents, not ordinary unused code.
/// Macro bodies run on the native host and are never JavaScript runtime roots.
pub(super) fn js_unavailable_defs(program: &CompiledProgram) -> HashSet<DefId> {
  let mut unavailable = HashSet::new();
  let mut dependents: HashMap<DefId, Vec<DefId>> = HashMap::new();
  for file in program.values() {
    for compiled in file.defs.values() {
      if compiled.kind == CompiledDefKind::Macro {
        continue;
      }
      if find_js_unavailable_proc(&compiled.codegen_form).is_some() {
        unavailable.insert(compiled.def_id);
      }
      for dependency in &compiled.deps {
        dependents.entry(*dependency).or_default().push(compiled.def_id);
      }
    }
  }
  let mut pending: VecDeque<_> = unavailable.iter().copied().collect();
  while let Some(dependency) = pending.pop_front() {
    if let Some(definitions) = dependents.get(&dependency) {
      for definition in definitions {
        if unavailable.insert(*definition) {
          pending.push_back(*definition);
        }
      }
    }
  }
  unavailable
}

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
  fn unavailable_runtime_dependencies_propagate_without_rejecting_quoted_symbols_or_macros() {
    let native_proc = Calcit::Proc("&format-ternary-tree".parse().unwrap());
    let quoted = Calcit::List(Arc::new(CalcitList::Vector(vec![
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from("app.main")),
      native_proc.clone(),
    ])));
    assert!(find_js_unavailable_proc(&quoted).is_none());
    assert!(find_js_unavailable_proc(&Calcit::Proc("&reset-gensym-index!".parse().unwrap())).is_none());

    let mut macro_def = lazy_value(DefId(4), native_proc.clone(), vec![DefId(1)]);
    macro_def.kind = CompiledDefKind::Macro;
    let program = HashMap::from([(
      Arc::from("app.main"),
      CompiledFileData {
        defs: HashMap::from([
          (Arc::from("native"), lazy_value(DefId(1), native_proc, vec![])),
          (
            Arc::from("wrapper"),
            lazy_value(DefId(2), symbol("native"), vec![DefId(1), DefId(3)]),
          ),
          (Arc::from("cycle"), lazy_value(DefId(3), symbol("wrapper"), vec![DefId(2)])),
          (Arc::from("macro"), macro_def),
          (Arc::from("quoted"), lazy_value(DefId(5), quoted, vec![])),
          (Arc::from("safe"), lazy_value(DefId(6), Calcit::Number(3.0), vec![])),
        ]),
      },
    )]);
    assert_eq!(js_unavailable_defs(&program), HashSet::from([DefId(1), DefId(2), DefId(3)]));
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
