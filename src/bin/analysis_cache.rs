//! Revision-keyed local cache for definition-local static analysis.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use calcit::calcit::{CalcitTrait, CalcitTypeAnnotation, LocatedWarning};
use calcit::call_stack::CallStackList;
use calcit::cli_args::{CheckTypesCommand, WeakTypesCommand};
use calcit::{cli_args, program, project_state, runner, snapshot};
use md5::{Digest, Md5};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::cli_handlers;
use crate::type_coverage::{self, TypeCoverageRow, WeakTypeKind, WeakTypeRow};

const CACHE_SCHEMA_VERSION: u32 = 1;
const CACHE_FILE: &str = "analysis-cache-v1.cirru";
const INPUT_CACHE_SCHEMA_VERSION: u32 = 2;
const INPUT_CACHE_FILE: &str = "analysis-input-cache-v2.cirru";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct InputCacheStats {
  pub status: String,
  pub reason: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub entry: Option<String>,
  pub sources: usize,
  pub main_reused: bool,
  pub module_hits: usize,
  pub module_misses: usize,
  pub module_miss_reasons: BTreeMap<String, usize>,
}

impl InputCacheStats {
  fn warm(sources: usize, module_hits: usize) -> Self {
    Self {
      status: "warm".to_owned(),
      reason: None,
      entry: None,
      sources,
      main_reused: true,
      module_hits,
      module_misses: 0,
      module_miss_reasons: BTreeMap::new(),
    }
  }

  fn cold(reason: impl Into<String>, sources: usize, module_misses: usize, module_miss_reasons: BTreeMap<String, usize>) -> Self {
    Self {
      status: "cold".to_owned(),
      reason: Some(reason.into()),
      entry: None,
      sources,
      main_reused: false,
      module_hits: 0,
      module_misses,
      module_miss_reasons,
    }
  }

  fn partial(
    reason: impl Into<String>,
    sources: usize,
    main_reused: bool,
    module_hits: usize,
    module_misses: usize,
    module_miss_reasons: BTreeMap<String, usize>,
  ) -> Self {
    Self {
      status: "partial".to_owned(),
      reason: Some(reason.into()),
      entry: None,
      sources,
      main_reused,
      module_hits,
      module_misses,
      module_miss_reasons,
    }
  }

  pub(crate) fn bypassed(reason: impl Into<String>) -> Self {
    Self {
      status: "bypassed".to_owned(),
      reason: Some(reason.into()),
      entry: None,
      sources: 0,
      main_reused: false,
      module_hits: 0,
      module_misses: 0,
      module_miss_reasons: BTreeMap::new(),
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
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dependency_index: Option<DependencyIndexStats>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct DependencyIndexStats {
  pub status: String,
  pub hits: usize,
  pub misses: usize,
  pub unresolved: usize,
  pub edges: usize,
  pub changed: usize,
  pub affected: usize,
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
      "cache_file": ".calcit/analysis-cache-v1.cirru",
      "input": self.input,
      "dependency_index": self.dependency_index,
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
      "- incremental-cache: scope=definition-local-inventory preprocessing-cached=false status={} hits={} misses={} reasons={} input={} dependencies={}\n",
      self.status(),
      self.hits,
      self.misses,
      if reasons.is_empty() { "none" } else { reasons.as_str() },
      self
        .input
        .as_ref()
        .map(|input| {
          let reason = input.reason.as_deref().unwrap_or("none");
          format!(
            "{}(reason={reason},entry={},main-reused={},module-hits={},module-misses={})",
            input.status,
            input.entry.as_deref().unwrap_or("not-recorded"),
            input.main_reused,
            input.module_hits,
            input.module_misses
          )
        })
        .unwrap_or_else(|| "not-recorded".to_owned()),
      self
        .dependency_index
        .as_ref()
        .map(|index| format!(
          "{}(hits={},misses={},changed={},affected={},edges={},unresolved={})",
          index.status, index.hits, index.misses, index.changed, index.affected, index.edges, index.unresolved
        ))
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
  main: CachedSnapshotUnit,
  modules: BTreeMap<String, CachedSnapshotUnit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedSnapshotUnit {
  snapshot: snapshot::Snapshot,
  sources: BTreeMap<String, String>,
  module_resolutions: BTreeMap<String, String>,
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
  #[serde(default)]
  dependency_index: BTreeMap<String, CachedDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedDependency {
  definition_revision: String,
  namespace_revision: String,
  ready: bool,
  dependencies: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct DependencySource {
  ns: String,
  definition: String,
  definition_revision: String,
  namespace_revision: String,
  schema: std::sync::Arc<CalcitTypeAnnotation>,
}

impl AnalysisCache {
  fn empty(context_revision: String) -> Self {
    Self {
      schema_version: CACHE_SCHEMA_VERSION,
      calcit_version: cli_args::CALCIT_VERSION.to_owned(),
      context_revision,
      definitions: BTreeMap::new(),
      dependency_index: BTreeMap::new(),
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

fn validate_input_cache_header(cache: &CachedAnalysisInput, expected_input: &str) -> Result<(), String> {
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
  Ok(())
}

fn validate_cached_snapshot_unit(unit: &CachedSnapshotUnit, base_dir: &Path, module_folder: &Path) -> Result<(), String> {
  for (path, expected_revision) in &unit.sources {
    let revision = source_revision(Path::new(path)).map_err(|_| "source-missing".to_owned())?;
    if &revision != expected_revision {
      return Err("source-changed".to_owned());
    }
  }
  for (request, expected_path) in &unit.module_resolutions {
    let (_, resolved_path, _) = calcit::resolve_module_snapshot_path(request, base_dir, module_folder);
    let resolved_path = canonical_path(&resolved_path).map_err(|_| "module-resolution-changed".to_owned())?;
    if &resolved_path != expected_path {
      return Err("module-resolution-changed".to_owned());
    }
  }
  Ok(())
}

fn build_cached_snapshot_unit(
  snapshot: snapshot::Snapshot,
  source_paths: impl IntoIterator<Item = PathBuf>,
  module_resolutions: impl IntoIterator<Item = (String, PathBuf)>,
) -> Result<CachedSnapshotUnit, String> {
  let mut sources = BTreeMap::new();
  for source in source_paths {
    let path = canonical_path(&source)?;
    sources.insert(path.clone(), source_revision(Path::new(&path))?);
  }
  let mut resolutions = BTreeMap::new();
  for (request, path) in module_resolutions {
    resolutions.insert(request, canonical_path(&path)?);
  }
  let ffi = encode_cached_ffi(&snapshot)?;
  Ok(CachedSnapshotUnit {
    snapshot,
    sources,
    module_resolutions: resolutions,
    ffi,
  })
}

fn restore_cached_snapshot_unit(unit: &CachedSnapshotUnit) -> Result<snapshot::Snapshot, String> {
  let mut snapshot = unit.snapshot.clone();
  restore_cached_ffi(&mut snapshot, &unit.ffi)?;
  Ok(snapshot)
}

fn uncached_snapshot_unit(snapshot: snapshot::Snapshot) -> CachedSnapshotUnit {
  CachedSnapshotUnit {
    snapshot,
    sources: BTreeMap::new(),
    module_resolutions: BTreeMap::new(),
    ffi: BTreeMap::new(),
  }
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

pub(crate) fn load_snapshot_for_incremental_analysis(
  snapshot_file: &str,
  selected_entry: Option<&str>,
) -> Result<(snapshot::Snapshot, InputCacheStats), String> {
  let expected_input = canonical_path(Path::new(snapshot_file))?;
  let path = input_cache_path(snapshot_file);
  let (cached, global_reason) = match fs::read_to_string(&path) {
    Ok(content) => match decode_cache::<CachedAnalysisInput>(&content) {
      Ok(cache) => match validate_input_cache_header(&cache, &expected_input) {
        Ok(()) => (Some(cache), None),
        Err(reason) => (None, Some(reason)),
      },
      Err(_) => (None, Some("cache-corrupt".to_owned())),
    },
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => (None, Some("cache-missing".to_owned())),
    Err(_) => (None, Some("cache-unreadable".to_owned())),
  };

  let base_dir = Path::new(&expected_input).parent().unwrap_or(Path::new("."));
  let module_folder = calcit::project_module_folder(base_dir);
  let mut first_reason = global_reason.clone();
  let mut cacheable = true;

  let cached_main = cached.as_ref().map(|cache| &cache.main);
  let (mut snapshot, main_unit, main_reused) = match cached_main
    .ok_or_else(|| global_reason.clone().unwrap_or_else(|| "not-cached".to_owned()))
    .and_then(|unit| {
      validate_cached_snapshot_unit(unit, base_dir, &module_folder)?;
      restore_cached_snapshot_unit(unit).map(|snapshot| (snapshot, unit.clone()))
    }) {
    Ok((snapshot, unit)) => (snapshot, unit, true),
    Err(reason) => {
      first_reason.get_or_insert_with(|| reason.clone());
      let snapshot = cli_handlers::load_main_snapshot(snapshot_file)?;
      let unit = match build_cached_snapshot_unit(snapshot.clone(), [PathBuf::from(snapshot_file)], []) {
        Ok(unit) => unit,
        Err(error) => {
          cacheable = false;
          first_reason = Some("source-unreadable".to_owned());
          eprintln!("Warning: Failed to cache main analysis input: {error}");
          uncached_snapshot_unit(snapshot.clone())
        }
      };
      (snapshot, unit, false)
    }
  };

  snapshot.select_entry(selected_entry)?;
  let active_entry = snapshot.active_entry_name().to_owned();
  let project_namespaces = snapshot.files.keys().cloned().collect::<HashSet<_>>();
  let mut modules_to_load = snapshot.active_entry()?.modules.clone();
  let mut seen_modules = HashSet::new();
  modules_to_load.retain(|module_path| seen_modules.insert(module_path.to_owned()));

  let mut module_units = BTreeMap::new();
  let mut module_hits = 0;
  let mut module_misses = 0;
  let mut module_miss_reasons = BTreeMap::new();
  for module_path in &modules_to_load {
    let cached_unit = cached.as_ref().and_then(|cache| cache.modules.get(module_path));
    let reused = cached_unit
      .ok_or_else(|| global_reason.clone().unwrap_or_else(|| "not-cached".to_owned()))
      .and_then(|unit| {
        validate_cached_snapshot_unit(unit, base_dir, &module_folder)?;
        restore_cached_snapshot_unit(unit).map(|snapshot| (snapshot, unit.clone()))
      });
    let (module_snapshot, unit) = match reused {
      Ok((module_snapshot, unit)) => {
        module_hits += 1;
        (module_snapshot, unit)
      }
      Err(reason) => {
        module_misses += 1;
        first_reason.get_or_insert_with(|| reason.clone());
        match cli_handlers::load_module_with_sources_silent(module_path, base_dir, &module_folder) {
          Ok(loaded) => {
            *module_miss_reasons.entry(reason).or_insert(0) += 1;
            let unit = match build_cached_snapshot_unit(loaded.snapshot.clone(), loaded.source_paths, loaded.module_resolutions) {
              Ok(unit) => unit,
              Err(error) => {
                cacheable = false;
                eprintln!("Warning: Failed to cache module '{module_path}': {error}");
                uncached_snapshot_unit(loaded.snapshot.clone())
              }
            };
            (loaded.snapshot, unit)
          }
          Err(error) => {
            cacheable = false;
            first_reason = Some("module-load-failed".to_owned());
            *module_miss_reasons.entry("module-load-failed".to_owned()).or_insert(0) += 1;
            eprintln!("Warning: Failed to load module '{module_path}': {error}");
            continue;
          }
        }
      }
    };
    calcit::merge_project_module_files(&mut snapshot, &module_snapshot, module_path)?;
    module_units.insert(module_path.clone(), unit);
  }

  let core_snapshot = calcit::load_core_snapshot()?;
  for (namespace, file_data) in core_snapshot.files {
    snapshot.files.entry(namespace).or_insert(file_data);
  }
  runner::preprocess::set_project_namespaces(&project_namespaces);

  let source_count = std::iter::once(&main_unit)
    .chain(module_units.values())
    .flat_map(|unit| unit.sources.keys())
    .collect::<BTreeSet<_>>()
    .len();
  if cacheable {
    let cache = CachedAnalysisInput {
      schema_version: INPUT_CACHE_SCHEMA_VERSION,
      calcit_version: cli_args::CALCIT_VERSION.to_owned(),
      core_revision: calcit::core_snapshot_revision(),
      input_path: expected_input,
      main: main_unit,
      modules: module_units,
    };
    if let Err(error) = write_input_cache(snapshot_file, &cache) {
      eprintln!("Warning: {error}; incremental analysis continued without persisting the input cache.");
    }
  }

  let reason = first_reason.unwrap_or_else(|| "not-cached".to_owned());
  let mut stats = if main_reused && module_misses == 0 {
    InputCacheStats::warm(source_count, module_hits)
  } else if main_reused || module_hits > 0 {
    InputCacheStats::partial(reason, source_count, main_reused, module_hits, module_misses, module_miss_reasons)
  } else {
    InputCacheStats::cold(reason, source_count, module_misses, module_miss_reasons)
  };
  stats.entry = Some(active_entry);
  Ok((snapshot, stats))
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

fn namespace_revision(file: &snapshot::FileInSnapShot) -> Result<String, String> {
  let bytes = serde_json::to_vec(&file.ns).map_err(|error| format!("Failed to encode namespace revision: {error}"))?;
  let mut hasher = Md5::new();
  hasher.update(bytes);
  Ok(format!("md5:{}", hex::encode(hasher.finalize())))
}

fn collect_schema_dependencies(schema: &std::sync::Arc<CalcitTypeAnnotation>) -> BTreeSet<String> {
  let dependencies = RefCell::new(BTreeSet::new());
  let _ = runner::preprocess::map_schema_references(
    schema.clone(),
    &|name, args| {
      let normalized = name.trim_start_matches('\'');
      if normalized.contains('/') {
        dependencies.borrow_mut().insert(normalized.to_owned());
      }
      std::sync::Arc::new(CalcitTypeAnnotation::TypeRef(name.clone(), args))
    },
    &|trait_def: std::sync::Arc<CalcitTrait>| {
      if let Some(definition_ref) = trait_def.definition_ref.as_deref() {
        dependencies.borrow_mut().insert(definition_ref.to_owned());
      }
      trait_def
    },
  );
  dependencies.into_inner()
}

fn reverse_affected_definitions(
  previous: &BTreeMap<String, CachedDependency>,
  current: &BTreeMap<String, CachedDependency>,
  changed: &BTreeSet<String>,
) -> BTreeSet<String> {
  let mut reverse = BTreeMap::<String, BTreeSet<String>>::new();
  for (dependent, record) in previous.iter().chain(current.iter()) {
    for dependency in &record.dependencies {
      reverse.entry(dependency.clone()).or_default().insert(dependent.clone());
    }
  }

  let mut affected = changed.clone();
  let mut pending = changed.iter().cloned().collect::<VecDeque<_>>();
  while let Some(definition) = pending.pop_front() {
    if let Some(dependents) = reverse.get(&definition) {
      for dependent in dependents {
        if affected.insert(dependent.clone()) {
          pending.push_back(dependent.clone());
        }
      }
    }
  }
  affected
}

fn unresolved_dependency_record(source: &DependencySource, previous: Option<&CachedDependency>) -> CachedDependency {
  CachedDependency {
    definition_revision: source.definition_revision.clone(),
    namespace_revision: source.namespace_revision.clone(),
    ready: false,
    dependencies: previous.map(|record| record.dependencies.clone()).unwrap_or_default(),
  }
}

fn trace_dependency_record(source: &DependencySource, previous: Option<&CachedDependency>) -> CachedDependency {
  let warnings = RefCell::<Vec<LocatedWarning>>::new(Vec::new());
  match runner::preprocess::trace_definition_source_usages(&source.ns, &source.definition, &warnings, &CallStackList::default()) {
    Ok(usages) => {
      let mut dependencies = usages
        .into_iter()
        .map(|usage| format!("{}/{}", usage.target_ns, usage.target_def))
        .collect::<BTreeSet<_>>();
      dependencies.extend(collect_schema_dependencies(&source.schema));
      CachedDependency {
        definition_revision: source.definition_revision.clone(),
        namespace_revision: source.namespace_revision.clone(),
        ready: true,
        dependencies,
      }
    }
    Err(_) => unresolved_dependency_record(source, previous),
  }
}

fn collect_dependency_index(
  cache: &mut AnalysisCache,
  snapshot: &snapshot::Snapshot,
  namespace: Option<&str>,
  namespace_prefix: Option<&str>,
  include_dependencies: bool,
) -> Result<DependencyIndexStats, String> {
  let entries = type_coverage::scoped_definition_entries(snapshot, namespace, namespace_prefix, include_dependencies)?;
  let previous = cache.dependency_index.clone();
  let all_ids = snapshot
    .files
    .iter()
    .flat_map(|(ns, file)| file.defs.keys().map(move |definition| format!("{ns}/{definition}")))
    .collect::<BTreeSet<_>>();
  let mut changed = BTreeSet::new();
  let mut pending = BTreeSet::new();
  let mut sources = BTreeMap::new();
  let mut stats = DependencyIndexStats::default();

  for (ns, definition, entry) in entries {
    let id = format!("{ns}/{definition}");
    let definition_revision = snapshot::definition_revision(entry)?;
    let namespace_revision = namespace_revision(snapshot.files.get(ns).expect("scoped namespace must exist"))?;
    sources.insert(
      id.clone(),
      DependencySource {
        ns: ns.to_owned(),
        definition: definition.to_owned(),
        definition_revision: definition_revision.clone(),
        namespace_revision: namespace_revision.clone(),
        schema: entry.schema.clone(),
      },
    );
    match previous.get(&id) {
      Some(record) if record.definition_revision == definition_revision && record.namespace_revision == namespace_revision => {
        stats.hits += 1;
        if !record.ready {
          stats.unresolved += 1;
        }
      }
      cached => {
        stats.misses += 1;
        let reason = match cached {
          None => "not-indexed",
          Some(record) if record.definition_revision != definition_revision => "definition-changed",
          Some(record) if record.namespace_revision != namespace_revision => "namespace-changed",
          Some(_) => "index-stale",
        };
        *stats.miss_reasons.entry(reason.to_owned()).or_insert(0) += 1;
        changed.insert(id.clone());
        pending.insert(id);
      }
    }
  }

  for removed in previous.keys().filter(|id| !all_ids.contains(*id)) {
    changed.insert(removed.clone());
  }

  let affected = reverse_affected_definitions(&previous, &cache.dependency_index, &changed);
  let refresh = affected
    .iter()
    .filter(|id| sources.contains_key(*id))
    .cloned()
    .collect::<BTreeSet<_>>();
  for id in refresh.iter().filter(|id| !pending.contains(*id)) {
    debug_assert!(previous.contains_key(id));
    stats.hits = stats.hits.saturating_sub(1);
    if previous.get(id).is_some_and(|record| !record.ready) {
      stats.unresolved = stats.unresolved.saturating_sub(1);
    }
    stats.misses += 1;
    *stats.miss_reasons.entry("dependency-affected".to_owned()).or_insert(0) += 1;
  }

  if !refresh.is_empty() {
    if let Ok(program_data) = program::extract_program_data(snapshot) {
      *program::PROGRAM_CODE_DATA.write().expect("open program data for dependency index") = program_data;
      for id in &refresh {
        let source = sources.get(id).expect("refresh source must exist");
        let record = trace_dependency_record(source, previous.get(id));
        if !record.ready {
          stats.unresolved += 1;
        }
        cache.dependency_index.insert(id.clone(), record);
      }
    } else {
      stats.unresolved += refresh.len();
      *stats.miss_reasons.entry("program-index-failed".to_owned()).or_insert(0) += refresh.len();
      for id in &refresh {
        let source = sources.get(id).expect("refresh source must exist");
        cache
          .dependency_index
          .insert(id.clone(), unresolved_dependency_record(source, previous.get(id)));
      }
    }
  }

  cache.dependency_index.retain(|id, _| all_ids.contains(id));
  stats.changed = changed.len();
  stats.affected = affected.len();
  stats.edges = cache
    .dependency_index
    .values()
    .filter(|record| record.ready)
    .map(|record| record.dependencies.len())
    .sum();
  stats.status = match (stats.hits, stats.misses) {
    (0, 0) => "empty",
    (_, 0) => "warm",
    (0, _) => "cold",
    _ => "partial",
  }
  .to_owned();
  Ok(stats)
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
  stats.dependency_index = Some(collect_dependency_index(
    &mut cache,
    snapshot,
    options.ns.as_deref(),
    options.ns_prefix.as_deref(),
    options.deps,
  )?);
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
  stats.dependency_index = Some(collect_dependency_index(
    &mut cache,
    snapshot,
    options.ns.as_deref(),
    options.ns_prefix.as_deref(),
    options.deps,
  )?);
  prune_removed_definitions(&mut cache, snapshot);
  if let Err(error) = write_cache(snapshot_file, &cache) {
    eprintln!("Warning: {error}; incremental analysis continued without persisting the cache.");
  }
  Ok((rows, stats))
}

#[cfg(test)]
mod tests {
  use super::{CachedDependency, DependencySource, reverse_affected_definitions, unresolved_dependency_record};
  use calcit::calcit::DYNAMIC_TYPE;
  use std::collections::{BTreeMap, BTreeSet};

  fn dependency(dependencies: &[&str]) -> CachedDependency {
    CachedDependency {
      definition_revision: "revision".to_owned(),
      namespace_revision: "namespace".to_owned(),
      ready: true,
      dependencies: dependencies.iter().map(|item| (*item).to_owned()).collect(),
    }
  }

  #[test]
  fn reverse_dependency_index_collects_transitive_callers_and_cycles() {
    let graph = BTreeMap::from([
      ("app/main".to_owned(), dependency(&["app/render"])),
      ("app/render".to_owned(), dependency(&["app/model"])),
      ("app/model".to_owned(), dependency(&["app/helper"])),
      ("app/helper".to_owned(), dependency(&["app/model"])),
      ("app/unrelated".to_owned(), dependency(&[])),
    ]);
    let changed = BTreeSet::from(["app/model".to_owned()]);

    assert_eq!(
      reverse_affected_definitions(&BTreeMap::new(), &graph, &changed),
      BTreeSet::from([
        "app/helper".to_owned(),
        "app/main".to_owned(),
        "app/model".to_owned(),
        "app/render".to_owned(),
      ])
    );
  }

  #[test]
  fn reverse_dependency_index_retains_removed_definition_edges() {
    let previous = BTreeMap::from([
      ("app/main".to_owned(), dependency(&["app/removed"])),
      ("app/removed".to_owned(), dependency(&[])),
    ]);
    let current = BTreeMap::from([("app/main".to_owned(), dependency(&["app/replacement"]))]);
    let changed = BTreeSet::from(["app/removed".to_owned()]);

    assert_eq!(
      reverse_affected_definitions(&previous, &current, &changed),
      BTreeSet::from(["app/main".to_owned(), "app/removed".to_owned()])
    );
  }

  #[test]
  fn unresolved_dependency_records_advance_revisions_but_keep_prior_edges() {
    let previous = dependency(&["app/old-target"]);
    let source = DependencySource {
      ns: "app".to_owned(),
      definition: "caller".to_owned(),
      definition_revision: "next-definition".to_owned(),
      namespace_revision: "next-namespace".to_owned(),
      schema: DYNAMIC_TYPE.clone(),
    };

    let record = unresolved_dependency_record(&source, Some(&previous));
    assert!(!record.ready);
    assert_eq!(record.definition_revision, "next-definition");
    assert_eq!(record.namespace_revision, "next-namespace");
    assert_eq!(record.dependencies, BTreeSet::from(["app/old-target".to_owned()]));
  }
}
