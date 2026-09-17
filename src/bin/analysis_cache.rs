//! Revision-keyed local cache for definition-local static analysis.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use calcit::cli_args::{CheckTypesCommand, WeakTypesCommand};
use calcit::{cli_args, project_state, runner, snapshot};
use md5::{Digest, Md5};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::cli_handlers;
use crate::type_coverage::{self, TypeCoverageRow, WeakTypeKind, WeakTypeRow};

const CACHE_SCHEMA_VERSION: u32 = 1;
const CACHE_FILE: &str = "analysis-cache-v1.cirru";
const INPUT_CACHE_SCHEMA_VERSION: u32 = 1;
const INPUT_CACHE_FILE: &str = "analysis-input-cache-v1.cirru";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct InputCacheStats {
  pub status: String,
  pub reason: Option<String>,
  pub sources: usize,
}

impl InputCacheStats {
  fn warm(sources: usize) -> Self {
    Self {
      status: "warm".to_owned(),
      reason: None,
      sources,
    }
  }

  fn cold(reason: impl Into<String>, sources: usize) -> Self {
    Self {
      status: "cold".to_owned(),
      reason: Some(reason.into()),
      sources,
    }
  }

  pub(crate) fn bypassed(reason: impl Into<String>) -> Self {
    Self {
      status: "bypassed".to_owned(),
      reason: Some(reason.into()),
      sources: 0,
    }
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct CacheStats {
  pub hits: usize,
  pub misses: usize,
  pub miss_reasons: BTreeMap<String, usize>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub input: Option<InputCacheStats>,
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
      "cache_file": ".calcit/analysis-cache-v1.cirru",
      "input": self.input,
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
      "- incremental-cache: scope=definition-local-inventory preprocessing-cached=false status={} hits={} misses={} reasons={} input={}\n",
      self.status(),
      self.hits,
      self.misses,
      if reasons.is_empty() { "none" } else { reasons.as_str() },
      self
        .input
        .as_ref()
        .map(|input| match input.reason.as_deref() {
          Some(reason) => format!("{}({reason})", input.status),
          None => input.status.clone(),
        })
        .unwrap_or_else(|| "not-recorded".to_owned())
    )
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedAnalysisInput {
  schema_version: u32,
  calcit_version: String,
  core_revision: String,
  input_path: String,
  sources: BTreeMap<String, String>,
  module_resolutions: BTreeMap<String, String>,
  project_namespaces: BTreeSet<String>,
  snapshot: snapshot::Snapshot,
  ffi: BTreeMap<String, String>,
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

fn input_cache_path(snapshot_file: &str) -> PathBuf {
  project_state::state_file_for_snapshot(snapshot_file, INPUT_CACHE_FILE)
}

fn source_revision(path: &Path) -> Result<String, String> {
  let bytes = fs::read(path).map_err(|error| format!("Failed to read analysis input '{}': {error}", path.display()))?;
  let mut hasher = Md5::new();
  hasher.update(bytes);
  Ok(format!("md5:{}", hex::encode(hasher.finalize())))
}

fn canonical_path(path: &Path) -> Result<String, String> {
  fs::canonicalize(path)
    .map(|path| path.to_string_lossy().into_owned())
    .map_err(|error| format!("Failed to resolve analysis input '{}': {error}", path.display()))
}

fn validate_input_cache(cache: &CachedAnalysisInput, expected_input: &str) -> Result<(), String> {
  if cache.schema_version != INPUT_CACHE_SCHEMA_VERSION {
    return Err("cache-schema-changed".to_owned());
  }
  if cache.calcit_version != cli_args::CALCIT_VERSION {
    return Err("compiler-version-changed".to_owned());
  }
  if cache.core_revision != calcit::core_snapshot_revision() {
    return Err("core-revision-changed".to_owned());
  }
  if cache.input_path != expected_input {
    return Err("input-path-changed".to_owned());
  }
  for (path, expected_revision) in &cache.sources {
    let revision = source_revision(Path::new(path)).map_err(|_| "source-missing".to_owned())?;
    if &revision != expected_revision {
      return Err("source-changed".to_owned());
    }
  }
  let base_dir = Path::new(expected_input).parent().unwrap_or(Path::new("."));
  let module_folder = calcit::project_module_folder(base_dir);
  for (request, expected_path) in &cache.module_resolutions {
    let (_, resolved_path, _) = calcit::resolve_module_snapshot_path(request, base_dir, &module_folder);
    let resolved_path = canonical_path(&resolved_path).map_err(|_| "module-resolution-changed".to_owned())?;
    if &resolved_path != expected_path {
      return Err("module-resolution-changed".to_owned());
    }
  }
  Ok(())
}

fn encode_cache<T: Serialize>(value: &T, label: &str) -> Result<String, String> {
  let data = cirru_edn::to_edn(value).map_err(|error| format!("Failed to encode {label} as Cirru EDN: {error}"))?;
  cirru_edn::format(&data, true).map_err(|error| format!("Failed to format {label} as Cirru EDN: {error}"))
}

fn decode_cache<T: DeserializeOwned>(content: &str) -> Result<T, String> {
  let data = cirru_edn::parse(content).map_err(|error| error.to_string())?;
  cirru_edn::from_edn(data)
}

fn write_input_cache(snapshot_file: &str, cache: &CachedAnalysisInput) -> Result<(), String> {
  let project_directory = project_state::project_directory_for_snapshot(snapshot_file);
  let directory = project_state::ensure_state_directory(project_directory)
    .map_err(|error| format!("Failed to create analysis input cache directory: {error}"))?;
  let path = directory.join(INPUT_CACHE_FILE);
  let temporary = directory.join(format!(".{INPUT_CACHE_FILE}.{}.tmp", std::process::id()));
  let content = encode_cache(cache, "analysis input cache")?;
  fs::write(&temporary, content).map_err(|error| format!("Failed to write analysis input cache staging file: {error}"))?;
  fs::rename(&temporary, &path).map_err(|error| format!("Failed to install analysis input cache: {error}"))?;
  Ok(())
}

fn encode_cached_ffi(snapshot: &snapshot::Snapshot) -> Result<BTreeMap<String, String>, String> {
  let mut ffi = BTreeMap::new();
  for (namespace, file) in &snapshot.files {
    for (definition, entry) in &file.defs {
      if let Some(metadata) = &entry.ffi {
        let rendered = cirru_edn::format(metadata, true)
          .map_err(|error| format!("Failed to encode cached FFI metadata for {namespace}/{definition}: {error}"))?;
        ffi.insert(format!("{namespace}/{definition}"), rendered);
      }
    }
  }
  Ok(ffi)
}

fn restore_cached_ffi(snapshot: &mut snapshot::Snapshot, cached: &BTreeMap<String, String>) -> Result<(), String> {
  let mut restored = 0;
  for (namespace, file) in &mut snapshot.files {
    for (definition, entry) in &mut file.defs {
      if entry.ffi.is_some() {
        let id = format!("{namespace}/{definition}");
        let rendered = cached.get(&id).ok_or_else(|| "cache-corrupt".to_owned())?;
        entry.ffi = Some(cirru_edn::parse(rendered).map_err(|_| "cache-corrupt".to_owned())?);
        restored += 1;
      }
    }
  }
  if restored != cached.len() {
    return Err("cache-corrupt".to_owned());
  }
  Ok(())
}

pub(crate) fn load_snapshot_for_incremental_analysis(snapshot_file: &str) -> Result<(snapshot::Snapshot, InputCacheStats), String> {
  let expected_input = canonical_path(Path::new(snapshot_file))?;
  let path = input_cache_path(snapshot_file);
  let mut reason = match fs::read_to_string(&path) {
    Ok(content) => match decode_cache::<CachedAnalysisInput>(&content) {
      Ok(cache) => match validate_input_cache(&cache, &expected_input) {
        Ok(()) => {
          let mut snapshot = cache.snapshot;
          match restore_cached_ffi(&mut snapshot, &cache.ffi) {
            Ok(()) => {
              let project_namespaces = cache.project_namespaces.iter().cloned().collect::<std::collections::HashSet<_>>();
              runner::preprocess::set_project_namespaces(&project_namespaces);
              let stats = InputCacheStats::warm(cache.sources.len());
              return Ok((snapshot, stats));
            }
            Err(reason) => reason,
          }
        }
        Err(reason) => reason,
      },
      Err(_) => "cache-corrupt".to_owned(),
    },
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => "cache-missing".to_owned(),
    Err(_) => "cache-unreadable".to_owned(),
  };

  let loaded = cli_handlers::load_snapshot_for_static_analysis_with_sources(snapshot_file)?;
  let mut sources = BTreeMap::new();
  let mut module_resolutions = BTreeMap::new();
  let mut source_error = None;
  for source in &loaded.source_paths {
    match canonical_path(source).and_then(|path| source_revision(Path::new(&path)).map(|revision| (path, revision))) {
      Ok((path, revision)) => {
        sources.insert(path, revision);
      }
      Err(error) => {
        source_error = Some(error);
        break;
      }
    }
  }
  if source_error.is_none() {
    for (request, resolved_path) in &loaded.module_resolutions {
      match canonical_path(resolved_path) {
        Ok(path) => {
          module_resolutions.insert(request.clone(), path);
        }
        Err(error) => {
          source_error = Some(error);
          break;
        }
      }
    }
  }

  if !loaded.cacheable {
    reason = "module-load-failed".to_owned();
  } else if source_error.is_some() {
    reason = "source-unreadable".to_owned();
  } else {
    match encode_cached_ffi(&loaded.snapshot) {
      Ok(ffi) => {
        let cache = CachedAnalysisInput {
          schema_version: INPUT_CACHE_SCHEMA_VERSION,
          calcit_version: cli_args::CALCIT_VERSION.to_owned(),
          core_revision: calcit::core_snapshot_revision(),
          input_path: expected_input,
          sources,
          module_resolutions,
          project_namespaces: loaded.project_namespaces.iter().cloned().collect(),
          snapshot: loaded.snapshot.clone(),
          ffi,
        };
        if let Err(error) = write_input_cache(snapshot_file, &cache) {
          eprintln!("Warning: {error}; incremental analysis continued without persisting the input cache.");
        }
      }
      Err(error) => eprintln!("Warning: {error}; incremental analysis continued without persisting the input cache."),
    }
  }

  Ok((loaded.snapshot, InputCacheStats::cold(reason, loaded.source_paths.len())))
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
  let Ok(cache) = decode_cache::<AnalysisCache>(&content) else {
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
  let content = encode_cache(cache, "analysis cache")?;
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
