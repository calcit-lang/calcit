//! Revision-keyed local cache for definition-local static analysis.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use calcit::cli_args::{CheckTypesCommand, WeakTypesCommand};
use calcit::{cli_args, project_state, snapshot};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::type_coverage::{self, TypeCoverageRow, WeakTypeKind, WeakTypeRow};

const CACHE_SCHEMA_VERSION: u32 = 1;
const CACHE_FILE: &str = "analysis-cache-v1.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct CacheStats {
  pub hits: usize,
  pub misses: usize,
  pub miss_reasons: BTreeMap<String, usize>,
}

impl CacheStats {
  pub(crate) fn status(&self) -> &'static str {
    match (self.hits, self.misses) {
      (0, 0) => "empty",
      (_, 0) => "warm",
      (0, _) => "cold",
      _ => "partial",
    }
  }

  fn record_miss(&mut self, reason: &str) {
    self.misses += 1;
    *self.miss_reasons.entry(reason.to_owned()).or_insert(0) += 1;
  }

  pub(crate) fn as_json(&self) -> serde_json::Value {
    serde_json::json!({
      "enabled": true,
      "scope": "definition-local-inventory",
      "preprocessing_cached": false,
      "status": self.status(),
      "hits": self.hits,
      "misses": self.misses,
      "miss_reasons": self.miss_reasons,
      "cache_file": ".calcit/analysis-cache-v1.json",
    })
  }

  pub(crate) fn human_line(&self) -> String {
    let reasons = self
      .miss_reasons
      .iter()
      .map(|(reason, count)| format!("{reason}={count}"))
      .collect::<Vec<_>>()
      .join(",");
    format!(
      "- incremental-cache: scope=definition-local-inventory preprocessing-cached=false status={} hits={} misses={} reasons={}\n",
      self.status(),
      self.hits,
      self.misses,
      if reasons.is_empty() { "none" } else { reasons.as_str() }
    )
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedDefinition {
  revision: String,
  #[serde(default)]
  check_types: Option<TypeCoverageRow>,
  #[serde(default)]
  weak_types_ready: bool,
  #[serde(default)]
  weak_types: Option<WeakTypeRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnalysisCache {
  schema_version: u32,
  calcit_version: String,
  context_revision: String,
  definitions: BTreeMap<String, CachedDefinition>,
}

impl AnalysisCache {
  fn empty(context_revision: String) -> Self {
    Self {
      schema_version: CACHE_SCHEMA_VERSION,
      calcit_version: cli_args::CALCIT_VERSION.to_owned(),
      context_revision,
      definitions: BTreeMap::new(),
    }
  }
}

fn cache_path(snapshot_file: &str) -> PathBuf {
  project_state::state_file_for_snapshot(snapshot_file, CACHE_FILE)
}

fn context_revision(snapshot: &snapshot::Snapshot) -> Result<String, String> {
  let context = serde_json::json!({
    "package": snapshot.package,
    "active_entry": snapshot.active_entry_name(),
    "entry": snapshot.active_entry()?,
  });
  let bytes = serde_json::to_vec(&context).map_err(|error| format!("Failed to encode analysis cache context: {error}"))?;
  let mut hasher = Md5::new();
  hasher.update(bytes);
  Ok(format!("md5:{}", hex::encode(hasher.finalize())))
}

fn load_cache(path: &Path, expected_context: &str) -> (AnalysisCache, Option<String>) {
  let Ok(content) = fs::read_to_string(path) else {
    return (AnalysisCache::empty(expected_context.to_owned()), Some("cache-missing".to_owned()));
  };
  let Ok(cache) = serde_json::from_str::<AnalysisCache>(&content) else {
    return (AnalysisCache::empty(expected_context.to_owned()), Some("cache-corrupt".to_owned()));
  };
  if cache.schema_version != CACHE_SCHEMA_VERSION {
    return (
      AnalysisCache::empty(expected_context.to_owned()),
      Some("cache-schema-changed".to_owned()),
    );
  }
  if cache.calcit_version != cli_args::CALCIT_VERSION {
    return (
      AnalysisCache::empty(expected_context.to_owned()),
      Some("compiler-version-changed".to_owned()),
    );
  }
  if cache.context_revision != expected_context {
    return (
      AnalysisCache::empty(expected_context.to_owned()),
      Some("entry-policy-changed".to_owned()),
    );
  }
  (cache, None)
}

fn write_cache(snapshot_file: &str, cache: &AnalysisCache) -> Result<(), String> {
  let project_directory = project_state::project_directory_for_snapshot(snapshot_file);
  let directory = project_state::ensure_state_directory(project_directory)
    .map_err(|error| format!("Failed to create analysis cache directory: {error}"))?;
  let path = directory.join(CACHE_FILE);
  let temporary = directory.join(format!(".{CACHE_FILE}.{}.tmp", std::process::id()));
  let content = serde_json::to_string(cache).map_err(|error| format!("Failed to encode analysis cache: {error}"))?;
  fs::write(&temporary, content).map_err(|error| format!("Failed to write analysis cache staging file: {error}"))?;
  fs::rename(&temporary, &path).map_err(|error| format!("Failed to install analysis cache: {error}"))?;
  Ok(())
}

fn miss_reason(entry: Option<&CachedDefinition>, revision: &str, global_reason: Option<&str>, ready: bool) -> String {
  if let Some(reason) = global_reason {
    return reason.to_owned();
  }
  match entry {
    None => "not-cached".to_owned(),
    Some(cached) if cached.revision != revision => "definition-changed".to_owned(),
    Some(_) if !ready => "analysis-missing".to_owned(),
    Some(_) => "not-cached".to_owned(),
  }
}

fn prune_removed_definitions(cache: &mut AnalysisCache, snapshot: &snapshot::Snapshot) {
  let current = snapshot
    .files
    .iter()
    .flat_map(|(namespace, file)| file.defs.keys().map(move |definition| format!("{namespace}/{definition}")))
    .collect::<BTreeSet<_>>();
  cache.definitions.retain(|id, _| current.contains(id));
}

pub(crate) fn collect_check_types(
  options: &CheckTypesCommand,
  snapshot: &snapshot::Snapshot,
  snapshot_file: &str,
) -> Result<(Vec<TypeCoverageRow>, CacheStats), String> {
  let context = context_revision(snapshot)?;
  let path = cache_path(snapshot_file);
  let (mut cache, global_reason) = load_cache(&path, &context);
  let entries = type_coverage::scoped_definition_entries(snapshot, options.ns.as_deref(), options.ns_prefix.as_deref(), options.deps)?;
  let mut rows = Vec::with_capacity(entries.len());
  let mut stats = CacheStats::default();

  for (namespace, definition, entry) in entries {
    let id = format!("{namespace}/{definition}");
    let revision = snapshot::definition_revision(entry)?;
    let cached = cache.definitions.get(&id);
    if let Some(row) = cached
      .filter(|cached| cached.revision == revision)
      .and_then(|cached| cached.check_types.clone())
    {
      stats.hits += 1;
      rows.push(row);
      continue;
    }
    stats.record_miss(&miss_reason(
      cached,
      &revision,
      global_reason.as_deref(),
      cached.and_then(|item| item.check_types.as_ref()).is_some(),
    ));
    let row = type_coverage::analyze_code_entry(namespace, definition, entry);
    let record = cache.definitions.entry(id).or_insert_with(|| CachedDefinition {
      revision: revision.clone(),
      check_types: None,
      weak_types_ready: false,
      weak_types: None,
    });
    if record.revision != revision {
      *record = CachedDefinition {
        revision,
        check_types: None,
        weak_types_ready: false,
        weak_types: None,
      };
    }
    record.check_types = Some(row.clone());
    rows.push(row);
  }

  type_coverage::filter_type_coverage_rows(options, &mut rows)?;
  prune_removed_definitions(&mut cache, snapshot);
  if let Err(error) = write_cache(snapshot_file, &cache) {
    eprintln!("Warning: {error}; incremental analysis continued without persisting the cache.");
  }
  Ok((rows, stats))
}

pub(crate) fn collect_weak_types(
  options: &WeakTypesCommand,
  snapshot: &snapshot::Snapshot,
  snapshot_file: &str,
) -> Result<(Vec<WeakTypeRow>, CacheStats), String> {
  let context = context_revision(snapshot)?;
  let path = cache_path(snapshot_file);
  let (mut cache, global_reason) = load_cache(&path, &context);
  let entries = type_coverage::scoped_definition_entries(snapshot, options.ns.as_deref(), options.ns_prefix.as_deref(), options.deps)?;
  let mut rows = Vec::new();
  let mut stats = CacheStats::default();

  for (namespace, definition, entry) in entries {
    let id = format!("{namespace}/{definition}");
    let revision = snapshot::definition_revision(entry)?;
    let cached = cache.definitions.get(&id);
    if let Some(cached) = cached.filter(|cached| cached.revision == revision && cached.weak_types_ready) {
      stats.hits += 1;
      if let Some(row) = cached.weak_types.clone() {
        rows.push(row);
      }
      continue;
    }
    stats.record_miss(&miss_reason(
      cached,
      &revision,
      global_reason.as_deref(),
      cached.is_some_and(|item| item.weak_types_ready),
    ));
    let row = type_coverage::analyze_weak_types_entry(namespace, definition, entry, &WeakTypeKind::all());
    let record = cache.definitions.entry(id).or_insert_with(|| CachedDefinition {
      revision: revision.clone(),
      check_types: None,
      weak_types_ready: false,
      weak_types: None,
    });
    if record.revision != revision {
      *record = CachedDefinition {
        revision,
        check_types: None,
        weak_types_ready: false,
        weak_types: None,
      };
    }
    record.weak_types_ready = true;
    record.weak_types = row.clone();
    if let Some(row) = row {
      rows.push(row);
    }
  }

  type_coverage::filter_weak_type_rows(options, &mut rows)?;
  prune_removed_definitions(&mut cache, snapshot);
  if let Err(error) = write_cache(snapshot_file, &cache) {
    eprintln!("Warning: {error}; incremental analysis continued without persisting the cache.");
  }
  Ok((rows, stats))
}
