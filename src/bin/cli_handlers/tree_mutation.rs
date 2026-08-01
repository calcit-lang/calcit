#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TreeOperation {
  Delete,
  Replace,
  InsertBefore,
  InsertAfter,
  InsertChild,
  AppendChild,
  SwapNextSibling,
  SwapPrevSibling,
}

impl TreeOperation {
  pub(crate) fn as_str(self) -> &'static str {
    match self {
      TreeOperation::Delete => "delete",
      TreeOperation::Replace => "replace",
      TreeOperation::InsertBefore => "insert-before",
      TreeOperation::InsertAfter => "insert-after",
      TreeOperation::InsertChild => "insert-child",
      TreeOperation::AppendChild => "append-child",
      TreeOperation::SwapNextSibling => "swap-next-sibling",
      TreeOperation::SwapPrevSibling => "swap-prev-sibling",
    }
  }

  pub(crate) fn from_insert_position(position: &str) -> Option<Self> {
    match position {
      "before" => Some(TreeOperation::InsertBefore),
      "after" => Some(TreeOperation::InsertAfter),
      "prepend-child" => Some(TreeOperation::InsertChild),
      "append-child" => Some(TreeOperation::AppendChild),
      "replace" => Some(TreeOperation::Replace),
      _ => None,
    }
  }

  pub(crate) fn cursor_mutation(self, path: Vec<usize>) -> TreeCursorMutation {
    match self {
      TreeOperation::Delete => TreeCursorMutation::Delete { path },
      TreeOperation::Replace => TreeCursorMutation::Replace { path },
      TreeOperation::InsertBefore => TreeCursorMutation::InsertBefore { path },
      TreeOperation::InsertAfter => TreeCursorMutation::InsertAfter { path },
      TreeOperation::InsertChild => TreeCursorMutation::InsertChild { path },
      TreeOperation::AppendChild => TreeCursorMutation::NoPathShift,
      TreeOperation::SwapNextSibling => TreeCursorMutation::SwapNext { path },
      TreeOperation::SwapPrevSibling => TreeCursorMutation::SwapPrev { path },
    }
  }

  pub(crate) fn inserted_path(self, destination: &[usize], append_index: usize) -> Result<Vec<usize>, String> {
    let mut inserted = destination.to_vec();
    match self {
      TreeOperation::InsertBefore | TreeOperation::Replace => {}
      TreeOperation::InsertAfter => {
        *inserted.last_mut().ok_or("Cannot insert after the definition root.")? += 1;
      }
      TreeOperation::InsertChild => inserted.push(0),
      TreeOperation::AppendChild => inserted.push(append_index),
      other => return Err(format!("Operation '{}' does not insert a tree node.", other.as_str())),
    }
    Ok(inserted)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TreeCursorMutation {
  NoPathShift,
  Replace { path: Vec<usize> },
  InsertBefore { path: Vec<usize> },
  InsertAfter { path: Vec<usize> },
  InsertChild { path: Vec<usize> },
  Delete { path: Vec<usize> },
  SwapNext { path: Vec<usize> },
  SwapPrev { path: Vec<usize> },
  Unwrap { path: Vec<usize>, child_count: usize },
  Raise { path: Vec<usize> },
  Wrap { path: Vec<usize> },
}

pub(crate) fn path_is_strict_descendant(root: &[usize], path: &[usize]) -> bool {
  path.len() > root.len() && path.starts_with(root)
}

/// Adjust a source path after inserting a sibling at the destination.
///
/// This is shared by edit execution and cursor maintenance so the source node
/// deleted after a move is always the same node the cursor logic tracks.
pub(crate) fn adjusted_source_path_after_insertion(source: &[usize], destination: &[usize], operation: TreeOperation) -> Vec<usize> {
  let mut adjusted = source.to_vec();
  if !matches!(operation, TreeOperation::InsertBefore | TreeOperation::InsertAfter)
    || source.len() != destination.len()
    || source.is_empty()
  {
    return adjusted;
  }
  let parent_depth = source.len() - 1;
  if source[..parent_depth] != destination[..parent_depth] {
    return adjusted;
  }
  let insert_position = if operation == TreeOperation::InsertBefore {
    destination[parent_depth]
  } else {
    destination[parent_depth] + 1
  };
  if insert_position <= source[parent_depth] {
    adjusted[parent_depth] += 1;
  }
  adjusted
}

fn shift_sibling(path: &mut [usize], mutation_path: &[usize], include_equal: bool, delta: isize) {
  if mutation_path.is_empty() || path.len() < mutation_path.len() {
    return;
  }
  let depth = mutation_path.len() - 1;
  if path[..depth] != mutation_path[..depth] {
    return;
  }
  let threshold = mutation_path[depth];
  if path[depth] > threshold || (include_equal && path[depth] == threshold) {
    path[depth] = path[depth].saturating_add_signed(delta);
  }
}

pub(crate) fn transform_delete(cursor: &[usize], deleted: &[usize]) -> (Vec<usize>, &'static str) {
  if cursor.starts_with(deleted) {
    return (
      deleted.get(..deleted.len().saturating_sub(1)).unwrap_or(&[]).to_vec(),
      "selected subtree was deleted; moved to parent",
    );
  }
  let mut next = cursor.to_vec();
  shift_sibling(&mut next, deleted, false, -1);
  if next == cursor {
    (next, "deletion did not shift cursor")
  } else {
    (next, "sibling deleted before cursor")
  }
}

pub(crate) fn transform_cursor_path(cursor: &[usize], mutation: &TreeCursorMutation) -> (Vec<usize>, &'static str) {
  match mutation {
    TreeCursorMutation::NoPathShift => (cursor.to_vec(), "tree content changed without shifting cursor path"),
    TreeCursorMutation::Replace { path } => {
      if cursor.starts_with(path) && cursor.len() > path.len() {
        (path.clone(), "cursor ancestor was replaced; moved to replacement root")
      } else if cursor == path {
        (cursor.to_vec(), "selected node was refreshed after replacement")
      } else {
        (cursor.to_vec(), "replacement did not affect cursor")
      }
    }
    TreeCursorMutation::InsertBefore { path } => {
      let mut next = cursor.to_vec();
      shift_sibling(&mut next, path, true, 1);
      if next == cursor {
        (next, "insertion did not shift cursor")
      } else {
        (next, "node inserted before cursor")
      }
    }
    TreeCursorMutation::InsertAfter { path } => {
      let mut next = cursor.to_vec();
      shift_sibling(&mut next, path, false, 1);
      if next == cursor {
        (next, "insertion did not shift cursor")
      } else {
        (next, "node inserted before cursor")
      }
    }
    TreeCursorMutation::InsertChild { path } => {
      let mut next = cursor.to_vec();
      if cursor.starts_with(path) && cursor.len() > path.len() {
        next[path.len()] += 1;
      }
      if next == cursor {
        (next, "child insertion did not shift cursor")
      } else {
        (next, "first child inserted before cursor descendant")
      }
    }
    TreeCursorMutation::Delete { path } => transform_delete(cursor, path),
    TreeCursorMutation::SwapNext { path } | TreeCursorMutation::SwapPrev { path } => {
      if path.is_empty() || cursor.len() < path.len() {
        return (cursor.to_vec(), "sibling swap did not affect cursor");
      }
      let depth = path.len() - 1;
      if cursor[..depth] != path[..depth] {
        return (cursor.to_vec(), "sibling swap did not affect cursor");
      }
      let other = match mutation {
        TreeCursorMutation::SwapNext { .. } => path[depth] + 1,
        TreeCursorMutation::SwapPrev { .. } => path[depth].saturating_sub(1),
        _ => unreachable!(),
      };
      let mut next = cursor.to_vec();
      if cursor[depth] == path[depth] {
        next[depth] = other;
      } else if cursor[depth] == other {
        next[depth] = path[depth];
      }
      if next == cursor {
        (next, "sibling swap did not affect cursor")
      } else {
        (next, "cursor followed swapped subtree")
      }
    }
    TreeCursorMutation::Unwrap { path, child_count } => {
      if path.is_empty() {
        return (cursor.to_vec(), "root unwrap is unsupported");
      }
      let parent = &path[..path.len() - 1];
      let wrapper_index = path[path.len() - 1];
      if cursor.starts_with(path) {
        if cursor.len() == path.len() {
          return (parent.to_vec(), "selected wrapper was removed; moved to parent");
        }
        let mut next = parent.to_vec();
        next.push(wrapper_index + cursor[path.len()]);
        next.extend_from_slice(&cursor[path.len() + 1..]);
        return (next, "cursor followed child out of unwrapped node");
      }
      let mut next = cursor.to_vec();
      if cursor.len() >= path.len() && cursor[..parent.len()] == *parent && cursor[parent.len()] > wrapper_index {
        next[parent.len()] = next[parent.len()].saturating_add(child_count.saturating_sub(1));
      }
      if next == cursor {
        (next, "unwrap did not affect cursor")
      } else {
        (next, "unwrapped siblings shifted cursor")
      }
    }
    TreeCursorMutation::Raise { path } => {
      if path.is_empty() {
        return (cursor.to_vec(), "root raise is unsupported");
      }
      let parent = &path[..path.len() - 1];
      if cursor.starts_with(path) {
        let mut next = parent.to_vec();
        next.extend_from_slice(&cursor[path.len()..]);
        (next, "cursor followed raised subtree")
      } else if cursor.starts_with(parent) {
        (parent.to_vec(), "cursor subtree was discarded by raise; moved to raised root")
      } else {
        (cursor.to_vec(), "raise did not affect cursor")
      }
    }
    TreeCursorMutation::Wrap { path } => {
      if cursor.starts_with(path) && cursor.len() > path.len() {
        (path.clone(), "cursor ancestor was wrapped; moved to wrapper root")
      } else if cursor == path {
        (cursor.to_vec(), "cursor now selects wrapper")
      } else {
        (cursor.to_vec(), "wrap did not affect cursor")
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{
    TreeCursorMutation, TreeOperation, adjusted_source_path_after_insertion, path_is_strict_descendant, transform_cursor_path,
  };

  #[test]
  fn insertion_adjusts_source_only_for_earlier_siblings() {
    assert_eq!(
      adjusted_source_path_after_insertion(&[2, 4], &[2, 1], TreeOperation::InsertBefore),
      vec![2, 5]
    );
    assert_eq!(
      adjusted_source_path_after_insertion(&[2, 4], &[2, 4], TreeOperation::InsertAfter),
      vec![2, 4]
    );
    assert_eq!(
      adjusted_source_path_after_insertion(&[2, 4], &[3, 1], TreeOperation::InsertBefore),
      vec![2, 4]
    );
  }

  #[test]
  fn descendant_check_is_strict() {
    assert!(path_is_strict_descendant(&[2, 1], &[2, 1, 0]));
    assert!(!path_is_strict_descendant(&[2, 1], &[2, 1]));
    assert!(!path_is_strict_descendant(&[2, 1], &[2, 2, 0]));
  }

  #[test]
  fn cursor_mutation_notes_leave_unrelated_paths_unaffected() {
    let (_, note) = transform_cursor_path(&[7, 1], &TreeCursorMutation::Replace { path: vec![3, 2] });
    assert_eq!(note, "replacement did not affect cursor");

    let (_, note) = transform_cursor_path(&[3, 5], &TreeCursorMutation::SwapNext { path: vec![3, 2] });
    assert_eq!(note, "sibling swap did not affect cursor");

    let (_, note) = transform_cursor_path(
      &[3, 5],
      &TreeCursorMutation::Unwrap {
        path: vec![3, 2],
        child_count: 1,
      },
    );
    assert_eq!(note, "unwrap did not affect cursor");

    let (_, note) = transform_cursor_path(&[7, 1], &TreeCursorMutation::Wrap { path: vec![3, 2] });
    assert_eq!(note, "wrap did not affect cursor");
  }
}
