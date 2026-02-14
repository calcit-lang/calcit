use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::calcit::{Calcit, CalcitImport, CalcitLocal};

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

  def_names.sort();

  let mut result: Vec<Arc<str>> = Vec::with_capacity(def_names.len());
  'outer: for name in def_names {
    for (idx, existing) in result.iter().enumerate() {
      if depends_on(existing, &name, &deps_graph, 3) {
        result.insert(idx, name.to_owned());
        continue 'outer;
      }
    }
    result.push(name.to_owned());
  }

  result
}

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
}
