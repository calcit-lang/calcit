//! Tree mutation helpers for `calcit.cli/*` write functions.
//! Mirrors `cli_handlers::edit::apply_operation_at_path` without depending on cli_handlers.

use calcit::calcit::CalcitErr;
use cirru_parser::Cirru;
use std::collections::BTreeMap;

pub fn apply_operation_at_path(code: &Cirru, path: &[usize], operation: &str, new_node: Option<&Cirru>) -> Result<Cirru, CalcitErr> {
  if path.is_empty() {
    return match operation {
      "replace" => {
        let node = new_node.ok_or_else(|| CalcitErr::from("Code input required for replace operation".to_string()))?;
        Ok(node.clone())
      }
      "delete" => Err(CalcitErr::from("Cannot delete root node".to_string())),
      _ => Err(CalcitErr::from(format!("Operation '{operation}' not supported at root level"))),
    };
  }

  apply_operation_recursive(code, path, 0, operation, new_node)
}

fn apply_operation_recursive(
  code: &Cirru,
  path: &[usize],
  depth: usize,
  operation: &str,
  new_node: Option<&Cirru>,
) -> Result<Cirru, CalcitErr> {
  match code {
    Cirru::Leaf(_) => Err(CalcitErr::from(format!("Cannot navigate into leaf node at depth {depth}"))),
    Cirru::List(items) => {
      let idx = path[depth];
      if idx >= items.len() {
        return Err(CalcitErr::from(format!(
          "Path index {idx} out of bounds (list has {} items)",
          items.len()
        )));
      }

      if depth == path.len() - 1 {
        let mut new_items = items.clone();
        match operation {
          "delete" => {
            new_items.remove(idx);
          }
          "replace" => {
            let newn = new_node.ok_or_else(|| CalcitErr::from("Code input required for replace operation".to_string()))?;
            new_items[idx] = newn.clone();
          }
          "insert-before" => {
            let newn = new_node.ok_or_else(|| CalcitErr::from("Code input required for insert-before operation".to_string()))?;
            new_items.insert(idx, newn.clone());
          }
          "insert-after" => {
            let newn = new_node.ok_or_else(|| CalcitErr::from("Code input required for insert-after operation".to_string()))?;
            new_items.insert(idx + 1, newn.clone());
          }
          "insert-child" => {
            let newn = new_node.ok_or_else(|| CalcitErr::from("Code input required for insert-child operation".to_string()))?;
            match &new_items[idx] {
              Cirru::List(children) => {
                let mut new_children = vec![newn.clone()];
                new_children.extend(children.clone());
                new_items[idx] = Cirru::List(new_children);
              }
              Cirru::Leaf(_) => {
                return Err(CalcitErr::from("Cannot insert child into leaf node".to_string()));
              }
            }
          }
          "append-child" => {
            let newn = new_node.ok_or_else(|| CalcitErr::from("Code input required for append-child operation".to_string()))?;
            match &new_items[idx] {
              Cirru::List(children) => {
                let mut new_children = children.clone();
                new_children.push(newn.clone());
                new_items[idx] = Cirru::List(new_children);
              }
              Cirru::Leaf(_) => {
                return Err(CalcitErr::from("Cannot append child to leaf node".to_string()));
              }
            }
          }
          "swap-next-sibling" => {
            if idx + 1 >= new_items.len() {
              return Err(CalcitErr::from(format!("Cannot swap: no next sibling at index {idx}")));
            }
            new_items.swap(idx, idx + 1);
          }
          "swap-prev-sibling" => {
            if idx == 0 {
              return Err(CalcitErr::from("Cannot swap: no previous sibling at index 0".to_string()));
            }
            new_items.swap(idx - 1, idx);
          }
          _ => return Err(CalcitErr::from(format!("Unknown operation: {operation}"))),
        }
        Ok(Cirru::List(new_items))
      } else {
        let mut new_items = items.clone();
        new_items[idx] = apply_operation_recursive(&items[idx], path, depth + 1, operation, new_node)?;
        Ok(Cirru::List(new_items))
      }
    }
  }
}

pub fn splice_at_path(code: &Cirru, path: &[usize]) -> Result<Cirru, CalcitErr> {
  if path.is_empty() {
    return Err(CalcitErr::from("Cannot unwrap root node (no parent to splice into)".to_string()));
  }
  splice_recursive(code, path, 0)
}

fn splice_recursive(code: &Cirru, path: &[usize], depth: usize) -> Result<Cirru, CalcitErr> {
  match code {
    Cirru::Leaf(_) => Err(CalcitErr::from(format!("Cannot navigate into leaf node at depth {depth}"))),
    Cirru::List(items) => {
      let idx = path[depth];
      if idx >= items.len() {
        return Err(CalcitErr::from(format!(
          "Path index {idx} out of bounds (list has {} items)",
          items.len()
        )));
      }
      if depth == path.len() - 1 {
        let splice_children = match &items[idx] {
          Cirru::List(children) => children.clone(),
          Cirru::Leaf(_) => return Err(CalcitErr::from("Node at path is a leaf; cannot unwrap".to_string())),
        };
        let mut new_items: Vec<Cirru> = Vec::with_capacity(items.len() - 1 + splice_children.len());
        new_items.extend_from_slice(&items[..idx]);
        new_items.extend(splice_children);
        new_items.extend_from_slice(&items[idx + 1..]);
        Ok(Cirru::List(new_items))
      } else {
        let mut new_items = items.clone();
        new_items[idx] = splice_recursive(&items[idx], path, depth + 1)?;
        Ok(Cirru::List(new_items))
      }
    }
  }
}

pub fn process_node_with_references(node: &Cirru, references: &BTreeMap<String, Cirru>) -> Result<Cirru, CalcitErr> {
  match node {
    Cirru::Leaf(s) => {
      if let Some(replacement) = references.get(s.as_ref()) {
        return Ok(replacement.clone());
      }
      Ok(node.clone())
    }
    Cirru::List(items) => {
      let processed_items: Result<Vec<Cirru>, CalcitErr> =
        items.iter().map(|item| process_node_with_references(item, references)).collect();
      Ok(Cirru::List(processed_items?))
    }
  }
}

pub fn find_exact_leaf_paths(node: &Cirru, pattern: &str, current_path: &mut Vec<usize>, results: &mut Vec<Vec<usize>>) {
  match node {
    Cirru::Leaf(s) => {
      if s.as_ref() == pattern {
        results.push(current_path.clone());
      }
    }
    Cirru::List(items) => {
      for (i, child) in items.iter().enumerate() {
        current_path.push(i);
        find_exact_leaf_paths(child, pattern, current_path, results);
        current_path.pop();
      }
    }
  }
}

pub fn find_regex_leaf_paths(node: &Cirru, pattern: &regex::Regex, current_path: &mut Vec<usize>, results: &mut Vec<Vec<usize>>) {
  match node {
    Cirru::Leaf(s) => {
      if pattern.is_match(s.as_ref()) {
        results.push(current_path.clone());
      }
    }
    Cirru::List(items) => {
      for (i, child) in items.iter().enumerate() {
        current_path.push(i);
        find_regex_leaf_paths(child, pattern, current_path, results);
        current_path.pop();
      }
    }
  }
}

pub fn map_at_to_operation(at: &str) -> Result<&'static str, CalcitErr> {
  match at {
    "before" => Ok("insert-before"),
    "after" => Ok("insert-after"),
    "prepend-child" => Ok("insert-child"),
    "append-child" => Ok("append-child"),
    "replace" => Ok("replace"),
    other => Err(CalcitErr::from(format!(
      "Unsupported position '{other}'. Use: before, after, prepend-child, append-child, replace"
    ))),
  }
}

pub fn to_path_is_inside_from(from_path: &[usize], to_path: &[usize]) -> bool {
  to_path.len() > from_path.len() && to_path[..from_path.len()] == *from_path
}

pub fn compute_adjusted_from_path(from_path: &[usize], to_path: &[usize], operation: &str) -> Vec<usize> {
  let mut adjusted = from_path.to_vec();
  if operation != "insert-before" && operation != "insert-after" {
    return adjusted;
  }
  if from_path.len() != to_path.len() {
    return adjusted;
  }
  let parent_depth = from_path.len() - 1;
  if from_path[..parent_depth] != to_path[..parent_depth] {
    return adjusted;
  }
  let from_idx = from_path[parent_depth];
  let to_idx = to_path[parent_depth];
  let insert_pos = if operation == "insert-before" { to_idx } else { to_idx + 1 };
  if insert_pos <= from_idx {
    adjusted[parent_depth] += 1;
  }
  adjusted
}
