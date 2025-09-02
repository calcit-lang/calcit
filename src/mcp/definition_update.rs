use cirru_parser::Cirru;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum UpdateMode {
  Replace,
  After,
  Before,
  Delete,
  Prepend,
  Append,
}

impl FromStr for UpdateMode {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "replace" => Ok(UpdateMode::Replace),
      "after" => Ok(UpdateMode::After),
      "before" => Ok(UpdateMode::Before),
      "delete" => Ok(UpdateMode::Delete),
      "prepend" => Ok(UpdateMode::Prepend),
      "append" => Ok(UpdateMode::Append),
      _ => Err(format!("Unknown update mode: {s}")),
    }
  }
}

pub fn update_definition_at_coord(
  cirru_tree: &mut Cirru,
  coord: &[usize],
  new_content: Option<&Cirru>,
  mode: UpdateMode,
  match_content: Option<&Cirru>,
) -> Result<(), String> {
  // Handle empty coordinate - operate on root node
  if coord.is_empty() {
    // Verify match content if provided
    if let Some(expected) = match_content {
      if cirru_tree != expected {
        return Err("Content at root does not match expected value".to_string());
      }
    }

    // For empty coord, only replace mode makes sense
    match mode {
      UpdateMode::Replace => {
        if let Some(content) = new_content {
          *cirru_tree = content.clone();
        } else {
          return Err("Replace mode requires new content".to_string());
        }
      }
      UpdateMode::Append | UpdateMode::Prepend => {
        // For append/prepend on root, the root must be a list
        let root_list = match cirru_tree {
          Cirru::List(list) => list,
          _ => return Err("Root node must be a list for append/prepend operations".to_string()),
        };
        
        if let Some(content) = new_content {
          match mode {
            UpdateMode::Append => root_list.push(content.clone()),
            UpdateMode::Prepend => root_list.insert(0, content.clone()),
            _ => unreachable!(),
          }
        } else {
          return Err("Append/Prepend mode requires new content".to_string());
        }
      }
      _ => {
        return Err("Only replace, append, and prepend modes are supported for empty coordinates".to_string());
      }
    }
    return Ok(());
  }

  // Navigate to the parent of the target node
  let (target_index, parent_coord) = coord.split_last().unwrap();
  let target_idx = *target_index;
  let parent = navigate_to_coord_mut(cirru_tree, parent_coord)?;

  // Ensure parent is a list
  let parent_list = match parent {
    Cirru::List(list) => list,
    _ => return Err("Parent node must be a list to perform updates".to_string()),
  };

  // Validate target index
  if target_idx >= parent_list.len() {
    return Err(format!(
      "Index {} out of bounds for list of length {}",
      target_idx,
      parent_list.len()
    ));
  }

  // Verify match content if provided
  if let Some(expected) = match_content {
    if &parent_list[target_idx] != expected {
      return Err("Content at coordinate does not match expected value".to_string());
    }
  }

  // Perform the update based on mode
  match mode {
    UpdateMode::Replace => {
      if let Some(content) = new_content {
        parent_list[target_idx] = content.clone();
      } else {
        return Err("Replace mode requires new content".to_string());
      }
    }
    UpdateMode::Delete => {
      parent_list.remove(target_idx);
    }
    UpdateMode::After => {
      if let Some(content) = new_content {
        parent_list.insert(target_idx + 1, content.clone());
      } else {
        return Err("After mode requires new content".to_string());
      }
    }
    UpdateMode::Before => {
      if let Some(content) = new_content {
        parent_list.insert(target_idx, content.clone());
      } else {
        return Err("Before mode requires new content".to_string());
      }
    }
    UpdateMode::Prepend => {
      if let Some(content) = new_content {
        // Target must be a list for prepend
        match &mut parent_list[target_idx] {
          Cirru::List(target_list) => {
            target_list.insert(0, content.clone());
          }
          _ => return Err("Prepend mode requires target to be a list".to_string()),
        }
      } else {
        return Err("Prepend mode requires new content".to_string());
      }
    }
    UpdateMode::Append => {
      if let Some(content) = new_content {
        // Target must be a list for append
        match &mut parent_list[target_idx] {
          Cirru::List(target_list) => {
            target_list.push(content.clone());
          }
          _ => return Err("Append mode requires target to be a list".to_string()),
        }
      } else {
        return Err("Append mode requires new content".to_string());
      }
    }
  }

  Ok(())
}

fn navigate_to_coord_mut<'a>(cirru_tree: &'a mut Cirru, coord: &[usize]) -> Result<&'a mut Cirru, String> {
  let mut current = cirru_tree;

  for (i, &index) in coord.iter().enumerate() {
    match current {
      Cirru::List(list) => {
        if index >= list.len() {
          return Err(format!("Index {index} out of bounds at coordinate position {i}"));
        }

        // For the last coordinate, return the node itself
        if i == coord.len() - 1 {
          return Ok(&mut list[index]);
        }

        // Navigate deeper
        current = &mut list[index];
      }
      _ => {
        return Err(format!(
          "Cannot navigate into non-list at coordinate position {i}: node is not a list"
        ));
      }
    }
  }

  // If coord is empty, return the root
  Ok(current)
}

#[cfg(test)]
mod tests {
  use super::*;
  use cirru_parser::CirruWriterOptions;

  #[test]
  fn test_replace_mode() {
    let mut tree = Cirru::List(vec![Cirru::Leaf("a".into()), Cirru::Leaf("b".into()), Cirru::Leaf("c".into())]);
    let new_content = Cirru::Leaf("x".into());

    update_definition_at_coord(&mut tree, &[1], Some(&new_content), UpdateMode::Replace, None).unwrap();

    let result = cirru_parser::format(&[tree], CirruWriterOptions { use_inline: false }).unwrap();
    assert_eq!(result.trim(), "a x c");
  }

  #[test]
  fn test_delete_mode() {
    let mut tree = Cirru::List(vec![Cirru::Leaf("a".into()), Cirru::Leaf("b".into()), Cirru::Leaf("c".into())]);

    update_definition_at_coord(&mut tree, &[1], None, UpdateMode::Delete, None).unwrap();

    let result = cirru_parser::format(&[tree], cirru_parser::CirruWriterOptions { use_inline: false }).unwrap();
    assert_eq!(result.trim(), "a c");
  }

  #[test]
  fn test_after_mode() {
    let mut tree = Cirru::List(vec![Cirru::Leaf("a".into()), Cirru::Leaf("b".into()), Cirru::Leaf("c".into())]);
    let new_content = Cirru::Leaf("x".into());

    update_definition_at_coord(&mut tree, &[1], Some(&new_content), UpdateMode::After, None).unwrap();

    let result = cirru_parser::format(&[tree], CirruWriterOptions { use_inline: false }).unwrap();
    assert_eq!(result.trim(), "a b x c");
  }

  #[test]
  fn test_before_mode() {
    let mut tree = Cirru::List(vec![Cirru::Leaf("a".into()), Cirru::Leaf("b".into()), Cirru::Leaf("c".into())]);
    let new_content = Cirru::Leaf("x".into());

    update_definition_at_coord(&mut tree, &[1], Some(&new_content), UpdateMode::Before, None).unwrap();

    let result = cirru_parser::format(&[tree], CirruWriterOptions { use_inline: false }).unwrap();
    assert_eq!(result.trim(), "a x b c");
  }

  #[test]
  fn test_prepend_mode() {
    let mut tree = Cirru::List(vec![
      Cirru::Leaf("a".into()),
      Cirru::List(vec![Cirru::Leaf("b".into()), Cirru::Leaf("c".into())]),
      Cirru::Leaf("d".into()),
    ]);
    let new_content = Cirru::Leaf("x".into());

    update_definition_at_coord(&mut tree, &[1], Some(&new_content), UpdateMode::Prepend, None).unwrap();

    let result = cirru_parser::format(&[tree], CirruWriterOptions { use_inline: false }).unwrap();
    assert_eq!(result.trim(), "a (x b c) d");
  }

  #[test]
  fn test_append_mode() {
    let mut tree = Cirru::List(vec![
      Cirru::Leaf("a".into()),
      Cirru::List(vec![Cirru::Leaf("b".into()), Cirru::Leaf("c".into())]),
      Cirru::Leaf("d".into()),
    ]);
    let new_content = Cirru::Leaf("x".into());

    update_definition_at_coord(&mut tree, &[1], Some(&new_content), UpdateMode::Append, None).unwrap();

    let result = cirru_parser::format(&[tree], CirruWriterOptions { use_inline: false }).unwrap();
    assert_eq!(result.trim(), "a (b c x) d");
  }

  #[test]
  fn test_match_validation() {
    let mut tree = Cirru::List(vec![Cirru::Leaf("a".into()), Cirru::Leaf("b".into()), Cirru::Leaf("c".into())]);
    let new_content = Cirru::Leaf("x".into());
    let expected_match = Cirru::Leaf("b".into());
    let wrong_match = Cirru::Leaf("z".into());

    // Should succeed with correct match
    update_definition_at_coord(&mut tree, &[1], Some(&new_content), UpdateMode::Replace, Some(&expected_match)).unwrap();

    // Should fail with wrong match
    let result = update_definition_at_coord(&mut tree, &[1], Some(&new_content), UpdateMode::Replace, Some(&wrong_match));
    assert!(result.is_err());
  }

  #[test]
  fn test_empty_coord_replace() {
    let mut tree = Cirru::List(vec![Cirru::Leaf("a".into()), Cirru::Leaf("b".into())]);
    let new_content = Cirru::Leaf("x".into());

    update_definition_at_coord(&mut tree, &[], Some(&new_content), UpdateMode::Replace, None).unwrap();

    // Check that the tree was replaced with the new content
    assert_eq!(tree, Cirru::Leaf("x".into()));
  }

  #[test]
  fn test_empty_coord_append() {
    let mut tree = Cirru::List(vec![Cirru::Leaf("a".into()), Cirru::Leaf("b".into())]);
    let new_content = Cirru::Leaf("c".into());

    update_definition_at_coord(&mut tree, &[], Some(&new_content), UpdateMode::Append, None).unwrap();

    let result = cirru_parser::format(&[tree], CirruWriterOptions { use_inline: false }).unwrap();
    assert_eq!(result.trim(), "a b c");
  }

  #[test]
  fn test_empty_coord_prepend() {
    let mut tree = Cirru::List(vec![Cirru::Leaf("a".into()), Cirru::Leaf("b".into())]);
    let new_content = Cirru::Leaf("x".into());

    update_definition_at_coord(&mut tree, &[], Some(&new_content), UpdateMode::Prepend, None).unwrap();

    let result = cirru_parser::format(&[tree], CirruWriterOptions { use_inline: false }).unwrap();
    assert_eq!(result.trim(), "x a b");
  }

  #[test]
  fn test_empty_coord_invalid_modes() {
    let mut tree = Cirru::List(vec![Cirru::Leaf("a".into())]);
    let new_content = Cirru::Leaf("x".into());

    // Delete mode should fail for empty coord
    let result = update_definition_at_coord(&mut tree, &[], Some(&new_content), UpdateMode::Delete, None);
    assert!(result.is_err());

    // After mode should fail for empty coord
    let result = update_definition_at_coord(&mut tree, &[], Some(&new_content), UpdateMode::After, None);
    assert!(result.is_err());

    // Before mode should fail for empty coord
    let result = update_definition_at_coord(&mut tree, &[], Some(&new_content), UpdateMode::Before, None);
    assert!(result.is_err());
  }
}
