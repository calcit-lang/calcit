//! Shared structural matching for source queries and reviewed project rewrites.

use std::collections::BTreeMap;

use cirru_parser::Cirru;

/// Match exact or prefix structure, optionally binding named subtree variables.
/// Bindings belong to one candidate only; a failed candidate returns no evidence.
pub(crate) fn match_structure(node: &Cirru, pattern: &Cirru, prefix: bool, bind_variables: bool) -> Option<BTreeMap<String, Cirru>> {
  fn visit(node: &Cirru, pattern: &Cirru, prefix: bool, bind_variables: bool, bindings: &mut BTreeMap<String, Cirru>) -> bool {
    if bind_variables
      && let Cirru::Leaf(name) = pattern
      && name.starts_with('?')
      && name.len() > 1
    {
      return match bindings.get(name.as_ref()) {
        Some(previous) => previous == node,
        None => {
          bindings.insert(name.to_string(), node.clone());
          true
        }
      };
    }
    match (node, pattern) {
      (Cirru::Leaf(actual), Cirru::Leaf(expected)) => actual == expected,
      (Cirru::List(actual), Cirru::List(expected)) => {
        (actual.len() == expected.len() || (prefix && actual.len() >= expected.len()))
          && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| visit(actual, expected, prefix, bind_variables, bindings))
      }
      _ => false,
    }
  }

  let mut bindings = BTreeMap::new();
  visit(node, pattern, prefix, bind_variables, &mut bindings).then_some(bindings)
}

/// Instantiate a template without evaluating, stringifying, or reparsing bindings.
/// Reject changes to variable occurrence counts, including repeated variables
/// in the pattern. Equal counts do not prove evaluation order or equivalence.
pub(crate) fn instantiate_reviewed_template(
  pattern: &Cirru,
  template: &Cirru,
  bindings: &BTreeMap<String, Cirru>,
) -> Result<Cirru, String> {
  fn visit(template: &Cirru, bindings: &BTreeMap<String, Cirru>, uses: &mut BTreeMap<String, usize>) -> Result<Cirru, String> {
    match template {
      Cirru::Leaf(name) if name.starts_with('?') && name.len() > 1 => {
        let bound = bindings
          .get(name.as_ref())
          .ok_or_else(|| format!("Replacement variable `{name}` is not bound by the pattern."))?;
        *uses.entry(name.to_string()).or_default() += 1;
        Ok(bound.clone())
      }
      Cirru::Leaf(_) => Ok(template.clone()),
      Cirru::List(items) => items
        .iter()
        .map(|item| visit(item, bindings, uses))
        .collect::<Result<Vec<_>, _>>()
        .map(Cirru::List),
    }
  }

  let mut uses = BTreeMap::new();
  let result = visit(template, bindings, &mut uses)?;
  fn count_variables(node: &Cirru, counts: &mut BTreeMap<String, usize>) {
    match node {
      Cirru::Leaf(name) if name.starts_with('?') && name.len() > 1 => {
        *counts.entry(name.to_string()).or_default() += 1;
      }
      Cirru::List(items) => {
        for item in items {
          count_variables(item, counts);
        }
      }
      Cirru::Leaf(_) => {}
    }
  }
  let mut expected_uses = BTreeMap::new();
  count_variables(pattern, &mut expected_uses);
  for name in bindings.keys() {
    let count = uses.get(name).copied().unwrap_or_default();
    let expected = expected_uses.get(name).copied().unwrap_or_default();
    if count != expected {
      return Err(format!(
        "Replacement changes occurrences of pattern variable `{name}` from {expected} to {count}; review discarded or repeated evaluation explicitly."
      ));
    }
  }
  Ok(result)
}

#[cfg(test)]
mod tests {
  use super::{instantiate_reviewed_template, match_structure};
  use cirru_parser::Cirru;

  fn expr(code: &str) -> Cirru {
    cirru_parser::parse(code).unwrap().remove(0)
  }

  #[test]
  fn literal_matching_preserves_exact_and_recursive_prefix_behavior() {
    let node = expr("f (g 1 2) 3");
    assert!(match_structure(&node, &expr("f (g 1)"), true, false).is_some());
    assert!(match_structure(&node, &expr("f (g 1)"), false, false).is_none());
    assert!(match_structure(&node, &node, false, false).is_some());
    assert!(match_structure(&node, &expr("f (g 1 2) 3 4"), true, false).is_none());
    assert!(match_structure(&expr("Foo"), &expr("foo"), false, false).is_none());
  }

  #[test]
  fn variables_bind_whole_subtrees_and_repeated_names_require_equality() {
    let pattern = expr("f ?item ?item");
    let node = expr("f (g 1) (g 1)");
    let bindings = match_structure(&node, &pattern, false, true).unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings["?item"], expr("g 1"));
    assert!(match_structure(&expr("f (g 1) (g 2)"), &pattern, false, true).is_none());
    assert!(match_structure(&expr("f (g 1)"), &pattern, false, true).is_none());
    // One failed candidate must not contaminate the following match.
    assert!(match_structure(&node, &pattern, false, true).is_some());
  }

  #[test]
  fn literal_query_mode_does_not_treat_question_mark_tokens_as_variables() {
    let pattern = expr("f ?item");
    assert!(match_structure(&expr("f 1"), &pattern, false, false).is_none());
    assert!(match_structure(&pattern, &pattern, false, false).unwrap().is_empty());
    let bare_question = Cirru::Leaf("?".into());
    assert!(match_structure(&Cirru::Leaf("value".into()), &bare_question, false, true).is_none());
    assert!(match_structure(&bare_question, &bare_question, false, true).unwrap().is_empty());
  }

  #[test]
  fn empty_lists_and_leaf_subtree_bindings_remain_distinct() {
    let empty = Cirru::List(Vec::new());
    assert!(match_structure(&empty, &empty, false, true).is_some());
    assert!(match_structure(&Cirru::Leaf("[]".into()), &empty, false, true).is_none());
    assert_eq!(
      match_structure(&empty, &Cirru::Leaf("?value".into()), false, true).unwrap()["?value"],
      empty
    );
  }

  #[test]
  fn reviewed_templates_preserve_bound_ast_and_reject_unaccounted_evaluations() {
    let pattern = expr("legacy ?items ?sep");
    let bindings = match_structure(&expr("legacy (effect! |value) |,"), &pattern, false, true).unwrap();
    assert_eq!(
      instantiate_reviewed_template(&pattern, &expr("preferred ?items ?sep"), &bindings).unwrap(),
      expr("preferred (effect! |value) |,")
    );
    for (replacement, expected) in [
      ("preferred ?items ?unknown", "not bound"),
      ("preferred ?items", "from 1 to 0"),
      ("preferred ?items ?items ?sep", "from 1 to 2"),
    ] {
      assert!(
        instantiate_reviewed_template(&pattern, &expr(replacement), &bindings)
          .unwrap_err()
          .contains(expected)
      );
    }
    let repeated = expr("legacy ?item ?item");
    let bindings = match_structure(&expr("legacy (effect!) (effect!)"), &repeated, false, true).unwrap();
    assert!(
      instantiate_reviewed_template(&repeated, &expr("preferred ?item"), &bindings)
        .unwrap_err()
        .contains("from 2 to 1")
    );
    assert!(instantiate_reviewed_template(&repeated, &expr("preferred ?item ?item"), &bindings).is_ok());
  }
}
