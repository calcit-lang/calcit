use std::collections::HashSet;
use std::sync::Arc;

use cirru_parser::Cirru;

use super::common::format_path;

const CORE_BOUNDARY_FORMS: &[&str] = &[
  "let",
  "&let",
  "cond",
  "if",
  "when",
  "case-default",
  "tag-match",
  "list-match",
  "fn",
  "loop",
  "do",
  "->",
];

const CORE_FORM_ALIASES: &[(&str, &str)] = &[
  ("defn", "defn"),
  ("defmacro", "macro"),
  ("fn", "fn"),
  ("let", "let"),
  ("&let", "let"),
  ("cond", "cond"),
  ("if", "if"),
  ("when", "when"),
  ("case-default", "case"),
  ("tag-match", "match"),
  ("list-match", "list"),
  ("loop", "loop"),
  ("do", "do"),
  ("->", "flow"),
];

#[derive(Clone, Debug)]
pub struct ChunkDisplayOptions {
  pub trigger_nodes: usize,
  pub target_nodes: usize,
  pub max_nodes: usize,
  pub max_branches: usize,
}

impl Default for ChunkDisplayOptions {
  fn default() -> Self {
    Self {
      trigger_nodes: 88,
      target_nodes: 56,
      max_nodes: 68,
      max_branches: 64,
    }
  }
}

#[derive(Clone, Debug)]
pub struct ChunkedDisplay {
  pub total: NodeStats,
  pub fragments: Vec<RenderedFragment>,
}

#[derive(Clone, Debug)]
pub struct RenderedFragment {
  pub id: String,
  pub coord: String,
  pub path: Vec<usize>,
  pub nodes: usize,
  pub depth: usize,
  pub cirru: String,
}

#[derive(Clone, Debug, Default)]
pub struct NodeStats {
  pub nodes: usize,
  pub branches: usize,
  pub leaves: usize,
  pub max_depth: usize,
}

#[derive(Clone, Debug)]
struct Candidate {
  path: Vec<usize>,
  nodes: usize,
  weighted_score: f64,
}

#[derive(Clone, Debug)]
struct Profile {
  max_fanout: usize,
  max_branch_children: usize,
}

#[derive(Clone, Debug)]
struct PendingFragment {
  id: String,
  path: Vec<usize>,
  tree: Cirru,
}

pub fn maybe_chunk_node(node: &Cirru, options: &ChunkDisplayOptions) -> Result<Option<ChunkedDisplay>, String> {
  let total = collect_stats(node, 0);
  if total.nodes < options.trigger_nodes {
    return Ok(None);
  }

  let fragments = recursive_partition(node, options)?;
  if fragments.len() <= 1 {
    return Ok(None);
  }

  Ok(Some(ChunkedDisplay {
    total,
    fragments: build_ordered_decomposition(&fragments)?,
  }))
}

fn recursive_partition(root: &Cirru, options: &ChunkDisplayOptions) -> Result<Vec<PendingFragment>, String> {
  let mut used_names = HashSet::new();
  let mut placeholder_index = 1;
  let mut pending = vec![PendingFragment {
    id: "ROOT".to_string(),
    path: vec![],
    tree: root.clone(),
  }];
  let mut finished: Vec<PendingFragment> = vec![];

  while !pending.is_empty() {
    pending.sort_by(|left, right| collect_stats(&right.tree, 0).nodes.cmp(&collect_stats(&left.tree, 0).nodes));
    let mut current = pending.remove(0);
    let current_stats = collect_stats(&current.tree, 0);

    if current_stats.nodes <= options.max_nodes {
      finished.push(current);
      continue;
    }

    let Some(cut) = pick_best_semantic_cut(&current.tree, options) else {
      finished.push(current);
      continue;
    };

    let subtree = get_node(&current.tree, &cut.path)
      .cloned()
      .ok_or_else(|| format!("Failed to get subtree at {}", format_path(&cut.path)))?;
    let global_path = extend_path(&current.path, &cut.path);
    let placeholder = make_placeholder_name(&subtree, placeholder_index, &mut used_names);
    placeholder_index += 1;

    let placeholder_leaf = Cirru::Leaf(Arc::from(format!("{placeholder}@{}", format_path(&global_path))));
    current.tree = replace_subtree(&current.tree, &cut.path, &placeholder_leaf)?;

    pending.push(current);
    pending.push(PendingFragment {
      id: placeholder,
      path: global_path,
      tree: subtree,
    });

    if placeholder_index > options.max_branches {
      break;
    }
  }

  finished.extend(pending);
  Ok(finished)
}

fn build_ordered_decomposition(fragments: &[PendingFragment]) -> Result<Vec<RenderedFragment>, String> {
  let mut root_fragment: Option<RenderedFragment> = None;
  let mut non_root: Vec<RenderedFragment> = vec![];

  for entry in fragments {
    let stats = collect_stats(&entry.tree, 0);
    let rendered = RenderedFragment {
      id: entry.id.clone(),
      coord: if entry.path.is_empty() {
        "root".to_string()
      } else {
        format_path(&entry.path)
      },
      path: entry.path.clone(),
      nodes: stats.nodes,
      depth: stats.max_depth,
      cirru: format_cirru_fragment(&entry.tree)?,
    };

    if entry.path.is_empty() {
      root_fragment = Some(rendered);
    } else {
      non_root.push(rendered);
    }
  }

  non_root.sort_by(|left, right| compare_coords(&left.coord, &right.coord));

  let mut result = vec![root_fragment.ok_or_else(|| "Missing ROOT fragment".to_string())?];
  result.extend(non_root);
  Ok(result)
}

pub fn fragment_nesting_level(fragment: &RenderedFragment, fragments: &[RenderedFragment]) -> usize {
  if fragment.path.is_empty() {
    return 0;
  }

  1 + fragments
    .iter()
    .filter(|candidate| {
      !candidate.path.is_empty() && candidate.path.len() < fragment.path.len() && fragment.path.starts_with(&candidate.path)
    })
    .count()
}

fn pick_best_semantic_cut(root: &Cirru, options: &ChunkDisplayOptions) -> Option<Candidate> {
  let min_nodes = usize::max(6, (options.target_nodes as f64 * 0.65).floor() as usize);
  let profile = build_profile(root);
  let candidates = collect_candidates(root, min_nodes, 1, &profile, options.target_nodes);
  let semantic_ceiling = options.max_nodes + 14;
  let bounded: Vec<Candidate> = candidates.iter().filter(|item| item.nodes <= semantic_ceiling).cloned().collect();
  let pool = if bounded.is_empty() { candidates } else { bounded };

  pool.into_iter().max_by(|left, right| {
    left
      .weighted_score
      .partial_cmp(&right.weighted_score)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then(left.nodes.cmp(&right.nodes))
  })
}

fn collect_candidates(root: &Cirru, min_nodes: usize, min_depth: usize, profile: &Profile, target_nodes: usize) -> Vec<Candidate> {
  let mut output = vec![];
  collect_candidates_inner(root, &mut output, vec![], 0, None, min_nodes, min_depth, profile, target_nodes);
  output
}

#[allow(clippy::too_many_arguments)]
fn collect_candidates_inner(
  node: &Cirru,
  output: &mut Vec<Candidate>,
  path: Vec<usize>,
  depth: usize,
  parent_nodes: Option<usize>,
  min_nodes: usize,
  min_depth: usize,
  profile: &Profile,
  target_nodes: usize,
) {
  let Cirru::List(items) = node else {
    return;
  };

  let stats = collect_stats(node, depth);
  let branch_children: Vec<&Cirru> = items.iter().filter(|child| matches!(child, Cirru::List(_))).collect();
  let child_sizes: Vec<usize> = branch_children.iter().map(|child| collect_stats(child, 0).nodes).collect();
  let fanout = items.len();
  let branch_children_len = branch_children.len();
  let parent_share = parent_nodes.map(|size| stats.nodes as f64 / size as f64).unwrap_or(1.0);
  let head_symbol = get_head_symbol(node);

  if !path.is_empty() && depth >= min_depth && stats.nodes >= min_nodes {
    let size_fit = clamp01(1.0 - (stats.nodes as f64 - target_nodes as f64).abs() / target_nodes as f64);
    let share_fit = clamp01(1.0 - (parent_share - 0.3).abs() / 0.22);
    let fanout_score = clamp01(fanout as f64 / usize::max(2, profile.max_fanout) as f64);
    let branchiness_score = clamp01(branch_children_len as f64 / usize::max(1, profile.max_branch_children) as f64);
    let syntax_boundary_score = if head_symbol.as_deref().is_some_and(is_boundary_form) {
      1.0
    } else {
      clamp01(0.3 + 0.4 * fanout_score + 0.3 * branchiness_score)
    };
    let density_score = stats.branches as f64 / usize::max(1, stats.nodes) as f64;
    let semantic_score = 0.28 * share_fit
      + 0.22 * compute_entropy(&child_sizes)
      + 0.2 * branchiness_score
      + 0.18 * syntax_boundary_score
      + 0.12 * density_score;
    let weighted_score = 0.38 * size_fit + 0.62 * semantic_score;

    output.push(Candidate {
      path: path.clone(),
      nodes: stats.nodes,
      weighted_score,
    });
  }

  for (index, child) in items.iter().enumerate() {
    let mut next_path = path.clone();
    next_path.push(index);
    collect_candidates_inner(
      child,
      output,
      next_path,
      depth + 1,
      Some(stats.nodes),
      min_nodes,
      min_depth,
      profile,
      target_nodes,
    );
  }
}

fn build_profile(root: &Cirru) -> Profile {
  fn walk(node: &Cirru, max_fanout: &mut usize, max_branch_children: &mut usize) {
    if let Cirru::List(items) = node {
      let fanout = items.len();
      let branch_children = items.iter().filter(|child| matches!(child, Cirru::List(_))).count();
      *max_fanout = (*max_fanout).max(fanout);
      *max_branch_children = (*max_branch_children).max(branch_children);
      for child in items {
        walk(child, max_fanout, max_branch_children);
      }
    }
  }

  let mut max_fanout = 1;
  let mut max_branch_children = 1;
  walk(root, &mut max_fanout, &mut max_branch_children);
  Profile {
    max_fanout,
    max_branch_children,
  }
}

fn collect_stats(node: &Cirru, depth: usize) -> NodeStats {
  match node {
    Cirru::Leaf(_) => NodeStats {
      nodes: 1,
      branches: 0,
      leaves: 1,
      max_depth: depth,
    },
    Cirru::List(items) => {
      let mut stats = NodeStats {
        nodes: 1,
        branches: 1,
        leaves: 0,
        max_depth: depth,
      };
      for child in items {
        let child_stats = collect_stats(child, depth + 1);
        stats.nodes += child_stats.nodes;
        stats.branches += child_stats.branches;
        stats.leaves += child_stats.leaves;
        stats.max_depth = stats.max_depth.max(child_stats.max_depth);
      }
      stats
    }
  }
}

fn get_head_symbol(node: &Cirru) -> Option<String> {
  match node {
    Cirru::List(items) => match items.first() {
      Some(Cirru::Leaf(s)) => Some(s.to_string()),
      _ => None,
    },
    Cirru::Leaf(_) => None,
  }
}

fn get_node<'a>(node: &'a Cirru, path: &[usize]) -> Option<&'a Cirru> {
  let mut current = node;
  for index in path {
    let Cirru::List(items) = current else {
      return None;
    };
    current = items.get(*index)?;
  }
  Some(current)
}

fn replace_subtree(node: &Cirru, target_path: &[usize], replacement: &Cirru) -> Result<Cirru, String> {
  fn inner(node: &Cirru, target_path: &[usize], replacement: &Cirru, path: &mut Vec<usize>) -> Result<Cirru, String> {
    if path.as_slice() == target_path {
      return Ok(replacement.clone());
    }

    match node {
      Cirru::Leaf(_) => Ok(node.clone()),
      Cirru::List(items) => {
        let mut output = Vec::with_capacity(items.len());
        for (index, child) in items.iter().enumerate() {
          path.push(index);
          output.push(inner(child, target_path, replacement, path)?);
          path.pop();
        }
        Ok(Cirru::List(output))
      }
    }
  }

  inner(node, target_path, replacement, &mut vec![])
}

fn extend_path(left: &[usize], right: &[usize]) -> Vec<usize> {
  let mut output = left.to_vec();
  output.extend_from_slice(right);
  output
}

fn compute_entropy(values: &[usize]) -> f64 {
  let total: usize = values.iter().sum();
  if total == 0 || values.len() <= 1 {
    return 0.0;
  }

  let mut entropy = 0.0;
  for value in values {
    if *value == 0 {
      continue;
    }
    let probability = *value as f64 / total as f64;
    entropy -= probability * probability.log2();
  }
  entropy / (values.len() as f64).log2()
}

fn clamp01(value: f64) -> f64 {
  value.clamp(0.0, 1.0)
}

fn is_boundary_form(symbol: &str) -> bool {
  CORE_BOUNDARY_FORMS.contains(&symbol)
}

fn alias_token(token: &str) -> &str {
  CORE_FORM_ALIASES
    .iter()
    .find_map(|(from, to)| (*from == token).then_some(*to))
    .unwrap_or(token)
}

fn sanitize_token(token: &str) -> Option<String> {
  let alias = alias_token(token);
  let cleaned = alias
    .trim_start_matches(|c| [':', '\'', '"', '~', '&', '|'].contains(&c))
    .chars()
    .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
    .collect::<String>()
    .trim_matches('_')
    .to_string();
  if cleaned.is_empty() {
    None
  } else {
    Some(cleaned.to_lowercase().chars().take(12).collect())
  }
}

fn collect_name_tokens(node: &Cirru, limit: usize, depth_limit: usize, depth: usize, output: &mut Vec<String>) {
  if output.len() >= limit {
    return;
  }
  match node {
    Cirru::Leaf(s) => {
      if let Some(token) = sanitize_token(s) {
        if !output.contains(&token) {
          output.push(token);
        }
      }
    }
    Cirru::List(items) => {
      if depth >= depth_limit {
        return;
      }
      for child in items {
        collect_name_tokens(child, limit, depth_limit, depth + 1, output);
        if output.len() >= limit {
          break;
        }
      }
    }
  }
}

fn make_placeholder_name(node: &Cirru, index: usize, used_names: &mut HashSet<String>) -> String {
  let mut tokens = vec![];
  collect_name_tokens(node, 2, 2, 0, &mut tokens);
  let base = if tokens.is_empty() {
    "branch".to_string()
  } else {
    tokens.join("_")
  };
  let mut suffix = index;
  let mut candidate = format!("{base}_{suffix:02}");
  while used_names.contains(&candidate) {
    suffix += 1;
    candidate = format!("{base}_{suffix:02}");
  }
  used_names.insert(candidate.clone());
  format!("{{{{{candidate}}}}}")
}

fn compare_coords(left: &str, right: &str) -> std::cmp::Ordering {
  let left_parts: Vec<usize> = if left == "root" {
    vec![]
  } else {
    left.split('.').filter_map(|item| item.parse::<usize>().ok()).collect()
  };
  let right_parts: Vec<usize> = if right == "root" {
    vec![]
  } else {
    right.split('.').filter_map(|item| item.parse::<usize>().ok()).collect()
  };

  for index in 0..left_parts.len().max(right_parts.len()) {
    let left_value = left_parts.get(index).copied().unwrap_or(usize::MIN);
    let right_value = right_parts.get(index).copied().unwrap_or(usize::MIN);
    match left_value.cmp(&right_value) {
      std::cmp::Ordering::Equal => continue,
      other => return other,
    }
  }
  std::cmp::Ordering::Equal
}

fn format_cirru_fragment(node: &Cirru) -> Result<String, String> {
  cirru_parser::format(std::slice::from_ref(node), cirru_parser::CirruWriterOptions { use_inline: false })
    .map(|text| text.trim().to_string())
    .map_err(|e| format!("Failed to format chunked Cirru: {e}"))
}

#[cfg(test)]
mod tests {
  use super::{ChunkDisplayOptions, RenderedFragment, fragment_nesting_level, maybe_chunk_node};

  fn parse_first(text: &str) -> cirru_parser::Cirru {
    cirru_parser::parse(text).unwrap().into_iter().next().unwrap()
  }

  fn fragment(id: &str, path: &[usize]) -> RenderedFragment {
    RenderedFragment {
      id: id.to_string(),
      coord: if path.is_empty() {
        "root".to_string()
      } else {
        path.iter().map(|idx| idx.to_string()).collect::<Vec<_>>().join(".")
      },
      path: path.to_vec(),
      nodes: 1,
      depth: 0,
      cirru: id.to_string(),
    }
  }

  #[test]
  fn does_not_chunk_small_nodes() {
    let node = parse_first("defn add (a b) &+ a b");
    let options = ChunkDisplayOptions::default();
    assert!(maybe_chunk_node(&node, &options).unwrap().is_none());
  }

  #[test]
  fn fragment_levels_hide_nested_chunks_by_default() {
    let fragments = vec![
      fragment("ROOT", &[]),
      fragment("L1_A", &[3]),
      fragment("L1_B", &[5]),
      fragment("L2_A", &[3, 2]),
      fragment("L3_A", &[3, 2, 1]),
    ];

    assert_eq!(fragment_nesting_level(&fragments[0], &fragments), 0);
    assert_eq!(fragment_nesting_level(&fragments[1], &fragments), 1);
    assert_eq!(fragment_nesting_level(&fragments[2], &fragments), 1);
    assert_eq!(fragment_nesting_level(&fragments[3], &fragments), 2);
    assert_eq!(fragment_nesting_level(&fragments[4], &fragments), 3);

    let visible_default = fragments
      .iter()
      .filter(|fragment| fragment_nesting_level(fragment, &fragments) <= 1)
      .count();
    let visible_deeper = fragments
      .iter()
      .filter(|fragment| fragment_nesting_level(fragment, &fragments) <= 2)
      .count();

    assert_eq!(visible_default, 3);
    assert_eq!(visible_deeper, 4);
  }
}
