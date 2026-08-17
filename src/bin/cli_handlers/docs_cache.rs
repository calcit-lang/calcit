//! Versioned, text-only cache primitives for the document knowledge index.
//!
//! The cache is deliberately independent from the normal docs search path in
//! the first phase. It can be populated and consumed by later graph commands
//! without making `cr docs search/read` depend on cache availability.

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

use super::docs::parse_doc_knowledge_metadata;

#[allow(dead_code)]
pub(crate) const CACHE_SCHEMA_VERSION: u32 = 5;
#[allow(dead_code)]
pub(crate) const PARSER_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CachedFile {
  pub path: String,
  pub content_hash: String,
  pub parser_version: u32,
  pub schema_version: u32,
  pub node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct KnowledgeNode {
  pub id: String,
  pub path: String,
  pub title: Option<String>,
  pub summary: Option<String>,
  pub code_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct KnowledgeEdge {
  pub from: String,
  pub relation: String,
  pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DefinitionRecord {
  pub id: String,
  pub namespace: String,
  pub name: String,
  #[serde(default)]
  pub doc: String,
  pub has_doc: bool,
  pub has_examples: bool,
  pub documented_by: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DocsCache {
  pub nodes: Vec<KnowledgeNode>,
  pub edges: Vec<KnowledgeEdge>,
  pub files: Vec<CachedFile>,
  pub definitions: Vec<DefinitionRecord>,
  pub definition_source_hash: String,
}

#[allow(dead_code)]
pub(crate) fn content_hash(content: &str) -> String {
  let digest = Md5::digest(content.as_bytes());
  hex::encode(digest)
}

pub(crate) fn cache_root() -> Result<PathBuf, String> {
  let home = env::var_os("HOME").ok_or_else(|| "Unable to get HOME environment variable".to_string())?;
  Ok(Path::new(&home).join(".config/calcit/docs-cache/v1"))
}

pub(crate) fn project_id(project_root: &Path) -> String {
  content_hash(&project_root.to_string_lossy())
}

pub(crate) fn cache_path(project_root: &Path) -> Result<PathBuf, String> {
  Ok(cache_root()?.join(project_id(project_root)).join("graph.json"))
}

#[allow(dead_code)]
pub(crate) fn file_is_fresh(cached: &CachedFile, path: &str, content: &str) -> bool {
  cached.path == path
    && cached.content_hash == content_hash(content)
    && cached.parser_version == PARSER_VERSION
    && cached.schema_version == CACHE_SCHEMA_VERSION
}

pub(crate) fn build_cache(root: &Path) -> Result<DocsCache, String> {
  let mut cache = DocsCache::default();
  let mut seen_ids = HashMap::new();
  let previous_cache = read_cache(root).ok();

  for entry in WalkDir::new(root) {
    let entry = entry.map_err(|err| format!("Failed to walk documentation directory {root:?}: {err}"))?;
    let path = entry.path();
    if !entry.file_type().is_file() || path.extension().and_then(|value| value.to_str()) != Some("md") {
      continue;
    }

    let content = fs::read_to_string(path).map_err(|err| format!("Failed to read documentation file {path:?}: {err}"))?;
    let relative_path = path
      .strip_prefix(root)
      .map_err(|_| format!("Unable to make documentation path relative: {path:?}"))?
      .to_string_lossy()
      .replace(std::path::MAIN_SEPARATOR, "/");
    let digest = content_hash(&content);
    let previous_file = previous_cache
      .as_ref()
      .and_then(|previous| previous.files.iter().find(|file| file.path == relative_path));
    let can_reuse = previous_file.is_some_and(|file| {
      file.content_hash == digest && file.parser_version == PARSER_VERSION && file.schema_version == CACHE_SCHEMA_VERSION
    });
    let node_ids = if can_reuse {
      previous_file.map(|file| file.node_ids.clone()).unwrap_or_default()
    } else {
      parse_doc_knowledge_metadata(&content).id.into_iter().collect()
    };

    if can_reuse {
      if let (Some(previous), Some(previous_file)) = (previous_cache.as_ref(), previous_file) {
        for node_id in &previous_file.node_ids {
          if let Some(node) = previous.nodes.iter().find(|node| &node.id == node_id) {
            if let Some(previous_path) = seen_ids.insert(node.id.clone(), relative_path.clone()) {
              return Err(format!(
                "Duplicate documentation node id '{}' in '{previous_path}' and '{relative_path}'",
                node.id
              ));
            }
            cache.nodes.push(node.clone());
          }
        }
        cache.edges.extend(
          previous
            .edges
            .iter()
            .filter(|edge| previous_file.node_ids.iter().any(|node_id| node_id == &edge.from))
            .cloned(),
        );
      }
    } else {
      let metadata = parse_doc_knowledge_metadata(&content);
      if let Some(id) = metadata.id.as_deref() {
        if let Some(previous_path) = seen_ids.insert(id.to_string(), relative_path.clone()) {
          return Err(format!(
            "Duplicate documentation node id '{id}' in '{previous_path}' and '{relative_path}'"
          ));
        }

        cache.nodes.push(KnowledgeNode {
          id: id.to_string(),
          path: relative_path.clone(),
          title: metadata.title,
          summary: metadata.summary,
          code_refs: metadata.code_refs,
        });

        if let Some(parent) = metadata.parent {
          cache.edges.push(KnowledgeEdge {
            from: id.to_string(),
            relation: "parent".to_string(),
            to: parent,
          });
        }
        for (relation, targets) in [
          ("related", metadata.related),
          ("requires", metadata.requires),
          ("leads_to", metadata.leads_to),
        ] {
          for target in targets {
            cache.edges.push(KnowledgeEdge {
              from: id.to_string(),
              relation: relation.to_string(),
              to: target,
            });
          }
        }
      }
    }

    cache.files.push(CachedFile {
      path: relative_path,
      content_hash: digest,
      parser_version: PARSER_VERSION,
      schema_version: CACHE_SCHEMA_VERSION,
      node_ids,
    });
  }

  cache.nodes.sort_by(|left, right| left.id.cmp(&right.id));
  cache
    .edges
    .sort_by(|left, right| (&left.from, &left.relation, &left.to).cmp(&(&right.from, &right.relation, &right.to)));
  cache.files.sort_by(|left, right| left.path.cmp(&right.path));

  let core_snapshot = calcit::load_core_snapshot().map_err(|err| format!("Failed to load embedded Calcit core snapshot: {err}"))?;
  cache.definition_source_hash = definition_source_hash(&core_snapshot);
  let code_ref_docs = cache
    .nodes
    .iter()
    .flat_map(|node| node.code_refs.iter().map(move |reference| (reference.as_str(), node.id.as_str())))
    .fold(HashMap::<&str, Vec<&str>>::new(), |mut index, (reference, node_id)| {
      index.entry(reference).or_default().push(node_id);
      index
    });
  for (namespace, file) in core_snapshot.files {
    for name in file.defs.keys() {
      let id = format!("{namespace}/{name}");
      let documented_by = code_ref_docs
        .get(id.as_str())
        .map(|ids| ids.iter().map(|id| (*id).to_string()).collect())
        .unwrap_or_default();
      cache.definitions.push(DefinitionRecord {
        id,
        namespace: namespace.clone(),
        name: name.clone(),
        doc: file.defs.get(name).map(|entry| entry.doc.clone()).unwrap_or_default(),
        has_doc: file.defs.get(name).is_some_and(|entry| !entry.doc.trim().is_empty()),
        has_examples: file.defs.get(name).is_some_and(|entry| !entry.examples.is_empty()),
        documented_by,
      });
    }
  }
  cache.definitions.sort_by(|left, right| left.id.cmp(&right.id));
  Ok(cache)
}

fn definition_source_hash(snapshot: &calcit::snapshot::Snapshot) -> String {
  let mut entries = snapshot
    .files
    .iter()
    .flat_map(|(namespace, file)| {
      file
        .defs
        .iter()
        .map(move |(name, entry)| format!("{namespace}/{name}\0{}\0{}", entry.doc, entry.examples.len()))
    })
    .collect::<Vec<_>>();
  entries.sort();
  content_hash(&entries.join("\n"))
}

pub(crate) fn write_cache(root: &Path, cache: &DocsCache) -> Result<PathBuf, String> {
  let path = cache_path(root)?;
  let parent = path.parent().ok_or_else(|| format!("Cache path has no parent: {path:?}"))?;
  fs::create_dir_all(parent).map_err(|err| format!("Failed to create docs cache directory {parent:?}: {err}"))?;
  let content = serde_json::to_string_pretty(cache).map_err(|err| format!("Failed to encode docs cache: {err}"))?;
  let nonce = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|duration| duration.as_nanos())
    .unwrap_or_default();
  let temporary = path.with_extension(format!("json.{}.{}.tmp", std::process::id(), nonce));
  fs::write(&temporary, content).map_err(|err| format!("Failed to write docs cache {temporary:?}: {err}"))?;
  fs::rename(&temporary, &path).map_err(|err| format!("Failed to install docs cache {path:?}: {err}"))?;
  Ok(path)
}

pub(crate) fn read_cache(root: &Path) -> Result<DocsCache, String> {
  let path = cache_path(root)?;
  let content = fs::read_to_string(&path).map_err(|err| format!("Failed to read docs cache {path:?}: {err}"))?;
  serde_json::from_str(&content).map_err(|err| format!("Failed to decode docs cache {path:?}: {err}"))
}

pub(crate) fn cache_is_fresh(root: &Path, cache: &DocsCache) -> Result<bool, String> {
  let core_snapshot = calcit::load_core_snapshot().map_err(|err| format!("Failed to load embedded Calcit core snapshot: {err}"))?;
  if cache.definition_source_hash != definition_source_hash(&core_snapshot) {
    return Ok(false);
  }
  let mut current_files = 0usize;
  for entry in WalkDir::new(root) {
    let entry = entry.map_err(|err| format!("Failed to walk documentation directory {root:?}: {err}"))?;
    let path = entry.path();
    if !entry.file_type().is_file() || path.extension().and_then(|value| value.to_str()) != Some("md") {
      continue;
    }
    current_files += 1;
    let relative_path = path
      .strip_prefix(root)
      .map_err(|_| format!("Unable to make documentation path relative: {path:?}"))?
      .to_string_lossy()
      .replace(std::path::MAIN_SEPARATOR, "/");
    let content = fs::read_to_string(path).map_err(|err| format!("Failed to read documentation file {path:?}: {err}"))?;
    let Some(cached_file) = cache.files.iter().find(|file| file.path == relative_path) else {
      return Ok(false);
    };
    if !file_is_fresh(cached_file, &relative_path, &content) {
      return Ok(false);
    }
  }
  Ok(current_files == cache.files.len())
}

pub(crate) fn dangling_edges(cache: &DocsCache) -> Vec<KnowledgeEdge> {
  let known = cache
    .nodes
    .iter()
    .map(|node| node.id.as_str())
    .collect::<std::collections::HashSet<_>>();
  cache
    .edges
    .iter()
    .filter(|edge| !known.contains(edge.to.as_str()))
    .cloned()
    .collect()
}

pub(crate) fn missing_definitions<'a>(cache: &'a DocsCache, namespace_prefix: Option<&str>) -> Vec<&'a DefinitionRecord> {
  cache
    .definitions
    .iter()
    .filter(|definition| {
      definition.has_doc
        && definition.documented_by.is_empty()
        && !definition.name.starts_with('&')
        && !definition.name.starts_with('$')
        && !definition.name.starts_with('#')
        && namespace_prefix.is_none_or(|prefix| definition.namespace.starts_with(prefix))
    })
    .collect()
}

pub(crate) fn unresolved_code_refs(cache: &DocsCache) -> Vec<(String, String)> {
  let known = cache
    .definitions
    .iter()
    .map(|definition| definition.id.as_str())
    .collect::<std::collections::HashSet<_>>();
  cache
    .nodes
    .iter()
    .flat_map(|node| {
      node
        .code_refs
        .iter()
        .filter(|reference| !known.contains(reference.as_str()))
        .map(move |reference| (node.id.clone(), reference.clone()))
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::{
    CACHE_SCHEMA_VERSION, CachedFile, DefinitionRecord, DocsCache, KnowledgeEdge, KnowledgeNode, PARSER_VERSION, build_cache,
    cache_path, content_hash, dangling_edges, file_is_fresh, missing_definitions, project_id,
  };
  use std::fs;
  use std::path::Path;
  use std::time::{SystemTime, UNIX_EPOCH};

  fn unique_temp_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    std::env::temp_dir().join(format!("calcit-doc-cache-{label}-{nanos}"))
  }

  #[test]
  fn content_hash_is_stable_and_changes_with_content() {
    assert_eq!(content_hash("hello"), content_hash("hello"));
    assert_ne!(content_hash("hello"), content_hash("hello!"));
  }

  #[test]
  fn cache_file_freshness_checks_versions_and_content() {
    let cached = CachedFile {
      path: "docs/list.md".to_string(),
      content_hash: content_hash("body"),
      parser_version: PARSER_VERSION,
      schema_version: CACHE_SCHEMA_VERSION,
      node_ids: vec!["structure/list".to_string()],
    };

    assert!(file_is_fresh(&cached, "docs/list.md", "body"));
    assert!(!file_is_fresh(&cached, "docs/list.md", "changed"));
    assert!(!file_is_fresh(&cached, "docs/other.md", "body"));

    let mut changed_parser = cached.clone();
    changed_parser.parser_version += 1;
    assert!(!file_is_fresh(&changed_parser, "docs/list.md", "body"));

    let mut changed_schema = cached;
    changed_schema.schema_version += 1;
    assert!(!file_is_fresh(&changed_schema, "docs/list.md", "body"));
  }

  #[test]
  fn project_cache_identity_is_path_scoped() {
    assert_ne!(project_id(Path::new("/tmp/a")), project_id(Path::new("/tmp/b")));
    assert!(cache_path(Path::new("/tmp/a")).unwrap().ends_with("graph.json"));
  }

  #[test]
  fn cache_schema_round_trips_as_text_json() {
    let cache = DocsCache {
      nodes: vec![KnowledgeNode {
        id: "structure/list".to_string(),
        path: "docs/list.md".to_string(),
        title: Some("List".to_string()),
        summary: None,
        code_refs: vec![],
      }],
      edges: vec![KnowledgeEdge {
        from: "structure/list".to_string(),
        relation: "contains".to_string(),
        to: "api/list/nth".to_string(),
      }],
      files: vec![CachedFile {
        path: "docs/list.md".to_string(),
        content_hash: content_hash("list"),
        parser_version: PARSER_VERSION,
        schema_version: CACHE_SCHEMA_VERSION,
        node_ids: vec!["structure/list".to_string()],
      }],
      definitions: vec![],
      definition_source_hash: String::new(),
    };

    let encoded = serde_json::to_string_pretty(&cache).unwrap();
    let decoded: DocsCache = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, cache);
    assert!(encoded.contains("structure/list"));
    assert!(encoded.contains("contains"));
  }

  #[test]
  fn build_cache_extracts_nodes_edges_and_file_fingerprints() {
    let root = unique_temp_dir("build");
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(
      root.join("list.md"),
      "---\ntitle: List\nsummary: Collection\nid: structure/list\ncode_ref: calcit.core/nth\nleads_to:\n  - api/list/nth\n---\n# List\n",
    )
    .unwrap();
    fs::write(
      root.join("nested/nth.md"),
      "---\ntitle: nth\nid: api/list/nth\nparent: structure/list\n---\n# nth\n",
    )
    .unwrap();
    fs::write(root.join("plain.md"), "# No node\n").unwrap();

    let cache = build_cache(&root).unwrap();
    assert_eq!(cache.nodes.len(), 2);
    assert_eq!(cache.edges.len(), 2);
    assert_eq!(cache.files.len(), 3);
    assert!(
      cache
        .definitions
        .iter()
        .find(|definition| definition.id == "calcit.core/nth")
        .is_some_and(|definition| definition.documented_by == vec!["structure/list"])
    );
    assert!(cache.files.iter().any(|file| file.node_ids.is_empty()));
    assert!(dangling_edges(&cache).is_empty());

    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn build_cache_reports_duplicate_ids_and_dangling_edges() {
    let root = unique_temp_dir("duplicate");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "---\nid: duplicate\n---\n# A\n").unwrap();
    fs::write(root.join("b.md"), "---\nid: duplicate\n---\n# B\n").unwrap();
    assert!(build_cache(&root).unwrap_err().contains("Duplicate documentation node id"));
    fs::remove_dir_all(root).unwrap();

    let cache = DocsCache {
      nodes: vec![KnowledgeNode {
        id: "known".to_string(),
        path: "known.md".to_string(),
        title: None,
        summary: None,
        code_refs: vec![],
      }],
      edges: vec![KnowledgeEdge {
        from: "known".to_string(),
        relation: "requires".to_string(),
        to: "missing".to_string(),
      }],
      files: vec![],
      definitions: vec![],
      definition_source_hash: String::new(),
    };
    assert_eq!(dangling_edges(&cache).len(), 1);
  }

  #[test]
  fn missing_definitions_support_namespace_filters_and_skip_syntax_names() {
    let cache = DocsCache {
      nodes: vec![],
      edges: vec![],
      files: vec![],
      definitions: vec![
        DefinitionRecord {
          id: "calcit.core/public-api".to_string(),
          namespace: "calcit.core".to_string(),
          name: "public-api".to_string(),
          doc: "Public API documentation".to_string(),
          has_doc: true,
          has_examples: false,
          documented_by: vec![],
        },
        DefinitionRecord {
          id: "calcit.core/$syntax".to_string(),
          namespace: "calcit.core".to_string(),
          name: "$syntax".to_string(),
          doc: "Syntax documentation".to_string(),
          has_doc: true,
          has_examples: false,
          documented_by: vec![],
        },
        DefinitionRecord {
          id: "app.core/private-api".to_string(),
          namespace: "app.core".to_string(),
          name: "private-api".to_string(),
          doc: "Private API documentation".to_string(),
          has_doc: true,
          has_examples: false,
          documented_by: vec![],
        },
      ],
      definition_source_hash: String::new(),
    };

    let core_missing = missing_definitions(&cache, Some("calcit.core"));
    assert_eq!(
      core_missing.iter().map(|entry| entry.id.as_str()).collect::<Vec<_>>(),
      vec!["calcit.core/public-api"]
    );
    assert_eq!(missing_definitions(&cache, Some("missing")).len(), 0);
    assert_eq!(missing_definitions(&cache, None).len(), 2);
  }
}
