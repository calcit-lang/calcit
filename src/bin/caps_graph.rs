use crate::git::GitRepo;
use crate::{CALCIT_VERSION, PackageDeps, call_build_script, module_folder};
use cirru_edn::{Edn, EdnMapView};
use colored::Colorize;
use md5::{Digest, Md5};
use semver::Version;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_PARALLEL_RESOLVES: usize = 6;
const MODULE_CACHE_DIR: &str = "module-caches";
const PROJECT_VIEW_REGISTRY_DIR: &str = "projects";
const VERSION_STORE_METADATA: &str = "metadata.txt";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyRequest {
  pub reference: String,
  pub requested_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefKind {
  Tag,
  Branch,
  Commit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectModuleMode {
  Link,
  #[cfg(windows)]
  Copy,
}

impl ProjectModuleMode {
  fn as_str(self) -> &'static str {
    match self {
      Self::Link => "link",
      #[cfg(windows)]
      Self::Copy => "copy",
    }
  }
}

#[derive(Debug, Clone)]
pub struct ResolvedModule {
  pub repository: String,
  pub reference: String,
  pub commit: String,
  pub kind: RefKind,
  pub source: PathBuf,
  pub link_target: PathBuf,
  pub dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedGraph {
  pub root: BTreeMap<String, String>,
  pub modules: BTreeMap<String, ResolvedModule>,
  pub requests: BTreeMap<String, BTreeSet<DependencyRequest>>,
  pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GraphOptions {
  pub project_root: PathBuf,
  pub modules_dir: PathBuf,
  pub ci: bool,
  pub strict: bool,
  pub build_native: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreCleanup {
  pub modules: usize,
  pub kept: usize,
  pub removed: usize,
}

/// Rewrites the cache-local instructions that explain how cached modules are maintained.
pub fn write_modules_agents(modules_dir: &Path) -> Result<(), String> {
  let cache_root = module_cache_root(modules_dir);
  fs::create_dir_all(&cache_root).map_err(|e| format!("failed to create {}: {e}", cache_root.display()))?;
  let content = r#"# Calcit module cache

This directory is managed by `caps` as a shared, immutable cache of resolved module revisions.
Project snapshots load modules through their own
`<project>/.calcit/modules/` links, not by importing this cache directly.

Do not edit a cached module in place. To change a dependency, clone or open its source repository,
make the change through its normal Git workflow, commit it, publish a new SemVer tag, update the
consumer's `deps.cirru`, and run `caps` again. For local experiments, use an explicit relative
module path instead of modifying the cache.

`caps clean` is a global cache cleanup: it keeps the newest materialized revision of each module
and any revision still linked by a registered project view.
"#;
  fs::write(cache_root.join("AGENTS.md"), content).map_err(|e| format!("failed to write {}/AGENTS.md: {e}", cache_root.display()))
}

/// Removes unreferenced, non-current revisions while preserving the newest revision per module.
pub fn clean_version_store(modules_dir: &Path) -> Result<StoreCleanup, String> {
  let cache_root = module_cache_root(modules_dir);
  let _lock = CacheMetadataLock::acquire(&cache_root)?;
  let git_root = cache_root.join("git");
  if !git_root.exists() {
    return Ok(StoreCleanup {
      modules: 0,
      kept: 0,
      removed: 0,
    });
  }

  let mut cleanup = StoreCleanup {
    modules: 0,
    kept: 0,
    removed: 0,
  };
  let active_targets = active_project_view_targets(modules_dir)?;
  for owner in read_directories(&git_root)? {
    for repository in read_directories(&owner)? {
      let revisions = read_directories(&repository)?
        .into_iter()
        .filter(|revision| is_store_revision(revision))
        .map(stored_revision)
        .collect::<Result<Vec<_>, _>>()?;
      if revisions.is_empty() {
        continue;
      }
      cleanup.modules += 1;
      let newest = revisions.iter().max().expect("non-empty revisions").path.clone();
      cleanup.kept += 1;
      for revision in revisions {
        if revision.path == newest {
          continue;
        }
        if revision_is_linked_by_project_view(&revision.path, &active_targets) {
          cleanup.kept += 1;
          continue;
        }
        fs::remove_dir_all(&revision.path)
          .map_err(|e| format!("failed to remove old module revision {}: {e}", revision.path.display()))?;
        cleanup.removed += 1;
      }
    }
  }
  Ok(cleanup)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredRevision {
  path: PathBuf,
  version: Option<Version>,
  modified: SystemTime,
}

impl Ord for StoredRevision {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self
      .version
      .cmp(&other.version)
      .then_with(|| self.modified.cmp(&other.modified))
      .then_with(|| self.path.cmp(&other.path))
  }
}

impl PartialOrd for StoredRevision {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

fn module_cache_root(modules_dir: &Path) -> PathBuf {
  modules_dir.parent().unwrap_or(modules_dir).join(MODULE_CACHE_DIR)
}

struct CacheMetadataLock {
  file: fs::File,
}

impl CacheMetadataLock {
  fn acquire(cache_root: &Path) -> Result<Self, String> {
    Self::acquire_named(cache_root, ".metadata.lock")
  }

  fn acquire_named(lock_root: &Path, file_name: &str) -> Result<Self, String> {
    fs::create_dir_all(lock_root).map_err(|e| format!("failed to create {}: {e}", lock_root.display()))?;
    let path = lock_root.join(file_name);
    let file = OpenOptions::new()
      .read(true)
      .write(true)
      .create(true)
      .truncate(false)
      .open(&path)
      .map_err(|e| format!("failed to open lock {}: {e}", path.display()))?;
    for _ in 0..500 {
      match fs4::FileExt::try_lock(&file) {
        Ok(()) => return Ok(Self { file }),
        Err(fs4::TryLockError::WouldBlock) => thread::sleep(Duration::from_millis(10)),
        Err(fs4::TryLockError::Error(error)) => return Err(format!("failed to acquire lock {}: {error}", path.display())),
      }
    }
    Err(format!("timed out waiting for lock {}", path.display()))
  }
}

impl Drop for CacheMetadataLock {
  fn drop(&mut self) {
    let _ = fs4::FileExt::unlock(&self.file);
  }
}

#[cfg(test)]
fn metadata_cache_root(source: &Path) -> PathBuf {
  for ancestor in source.ancestors() {
    if ancestor.file_name().and_then(|name| name.to_str()) == Some("git")
      && ancestor
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        == Some(MODULE_CACHE_DIR)
    {
      return ancestor.parent().expect("git cache directory has a parent").to_path_buf();
    }
  }
  source.parent().unwrap_or(source).to_path_buf()
}

fn project_view_lock(calcit_dir: &Path) -> Result<CacheMetadataLock, String> {
  CacheMetadataLock::acquire_named(calcit_dir, ".view.lock")
}

fn register_project_view(options: &GraphOptions) -> Result<(), String> {
  let project_root = fs::canonicalize(&options.project_root).unwrap_or_else(|_| options.project_root.clone());
  let registry = module_cache_root(&options.modules_dir).join(PROJECT_VIEW_REGISTRY_DIR);
  fs::create_dir_all(&registry).map_err(|e| format!("failed to create {}: {e}", registry.display()))?;
  let identifier = hex::encode(Md5::digest(project_root.to_string_lossy().as_bytes()));
  let path = registry.join(format!("{identifier}.path"));
  fs::write(&path, format!("{}\n", project_root.display())).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

fn active_project_view_targets(modules_dir: &Path) -> Result<Vec<PathBuf>, String> {
  let registry = module_cache_root(modules_dir).join(PROJECT_VIEW_REGISTRY_DIR);
  if !registry.exists() {
    return Ok(vec![]);
  }
  let mut targets = vec![];
  for entry in fs::read_dir(&registry).map_err(|e| format!("failed to read {}: {e}", registry.display()))? {
    let entry = entry.map_err(|e| format!("failed to read project view entry: {e}"))?;
    if !entry
      .file_type()
      .map_err(|e| format!("failed to inspect {}: {e}", entry.path().display()))?
      .is_file()
    {
      continue;
    }
    let project_root = match fs::read_to_string(entry.path()) {
      Ok(value) if !value.trim().is_empty() => PathBuf::from(value.trim()),
      Ok(_) => continue,
      Err(error) => return Err(format!("failed to read {}: {error}", entry.path().display())),
    };
    let modules = project_root.join(".calcit/modules");
    if !modules.is_dir() {
      continue;
    }
    for module in fs::read_dir(&modules).map_err(|e| format!("failed to read {}: {e}", modules.display()))? {
      let module = module.map_err(|e| format!("failed to read project module entry: {e}"))?;
      if let Ok(target) = fs::canonicalize(module.path()) {
        targets.push(target);
      }
    }
  }
  Ok(targets)
}

fn revision_is_linked_by_project_view(revision: &Path, active_targets: &[PathBuf]) -> bool {
  let revision = fs::canonicalize(revision).unwrap_or_else(|_| revision.to_path_buf());
  active_targets.iter().any(|target| target.starts_with(&revision))
}

fn read_directories(path: &Path) -> Result<Vec<PathBuf>, String> {
  let mut items = fs::read_dir(path)
    .map_err(|e| format!("failed to read {}: {e}", path.display()))?
    .filter_map(Result::ok)
    .filter_map(|entry| entry.file_type().ok().filter(|kind| kind.is_dir()).map(|_| entry.path()))
    .collect::<Vec<_>>();
  items.sort();
  Ok(items)
}

fn is_store_revision(path: &Path) -> bool {
  fs::symlink_metadata(path.join("source"))
    .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    .unwrap_or(false)
}

fn stored_revision(path: PathBuf) -> Result<StoredRevision, String> {
  let reference = fs::read_to_string(path.join(VERSION_STORE_METADATA))
    .ok()
    .and_then(|content| metadata_value(&content, "reference"));
  let modified = fs::metadata(&path).and_then(|metadata| metadata.modified()).unwrap_or(UNIX_EPOCH);
  Ok(StoredRevision {
    path,
    version: reference.as_deref().and_then(parse_tag_version),
    modified,
  })
}

fn metadata_value(content: &str, name: &str) -> Option<String> {
  content.lines().find_map(|line| {
    line
      .split_once('=')
      .filter(|(key, _)| key.trim() == name)
      .map(|(_, value)| value.trim().to_string())
  })
}

pub fn resolve_graph(root_deps: &PackageDeps, options: &GraphOptions) -> Result<ResolvedGraph, String> {
  let root = sorted_root_dependencies(root_deps)?;
  validate_module_folders(root.keys())?;
  let mut selections = BTreeMap::<String, String>::new();
  let mut stable_result = None;
  let mut cache = BTreeMap::<(String, String), ResolvedModule>::new();

  for _ in 0..256 {
    let pass = resolve_pass(&root, &selections, options, &mut cache)?;
    if pass.selections == selections {
      let final_pass = resolve_pass(&root, &pass.selections, options, &mut cache)?;
      stable_result = Some(final_pass);
      break;
    }
    selections = pass.selections;
  }

  let pass = stable_result.ok_or_else(|| "dependency resolution did not converge after 256 passes".to_string())?;
  let mut modules = pass.modules;
  validate_module_folders(modules.keys())?;

  if options.build_native {
    for module in modules.values_mut() {
      module.link_target = prepare_native_realization(module, options)?;
    }
  }

  let warnings = collect_warnings(&root, &pass.requests, &modules);
  if options.strict && !warnings.is_empty() {
    return Err(format!("strict dependency resolution rejected warnings:\n{}", warnings.join("\n")));
  }

  Ok(ResolvedGraph {
    root,
    modules,
    requests: pass.requests,
    warnings,
  })
}

struct ResolvePass {
  selections: BTreeMap<String, String>,
  modules: BTreeMap<String, ResolvedModule>,
  requests: BTreeMap<String, BTreeSet<DependencyRequest>>,
}

fn resolve_pass(
  root: &BTreeMap<String, String>,
  previous: &BTreeMap<String, String>,
  options: &GraphOptions,
  cache: &mut BTreeMap<(String, String), ResolvedModule>,
) -> Result<ResolvePass, String> {
  let mut requests = BTreeMap::<String, BTreeSet<DependencyRequest>>::new();
  for (repository, reference) in root {
    requests.entry(repository.clone()).or_default().insert(DependencyRequest {
      reference: reference.clone(),
      requested_by: None,
    });
  }

  let selected_items = previous
    .iter()
    .chain(root.iter().filter(|(repository, _)| !previous.contains_key(*repository)))
    .map(|(repository, selected)| (repository.clone(), selected.clone()))
    .collect::<Vec<_>>();
  let mut modules = BTreeMap::<String, ResolvedModule>::new();
  let mut missing = Vec::new();
  for (repository, selected) in selected_items {
    let cache_key = (repository.clone(), selected.clone());
    if let Some(module) = cache.get(&cache_key) {
      modules.insert(repository, module.clone());
    } else {
      missing.push((repository, selected));
    }
  }

  for batch in missing.chunks(MAX_PARALLEL_RESOLVES) {
    let handles = batch
      .iter()
      .map(|(repository, selected)| {
        let repository = repository.clone();
        let selected = selected.clone();
        let options = options.clone();
        thread::spawn(move || {
          let result = materialize_module(&repository, &selected, &options);
          (repository, selected, result)
        })
      })
      .collect::<Vec<_>>();
    for handle in handles {
      let (repository, selected, result) = handle
        .join()
        .map_err(|_| "dependency resolver worker panicked while inspecting a module".to_string())?;
      let module = result?;
      cache.insert((repository.clone(), selected), module.clone());
      modules.insert(repository, module);
    }
  }

  for (repository, module) in &modules {
    for (dependency, reference) in &module.dependencies {
      requests.entry(dependency.clone()).or_default().insert(DependencyRequest {
        reference: reference.clone(),
        requested_by: Some(format!("{}@{}", repository, module.reference)),
      });
    }
  }

  let mut selections = BTreeMap::new();
  for (repository, module_requests) in &requests {
    selections.insert(
      repository.clone(),
      select_reference(repository, module_requests, root.get(repository))?,
    );
  }

  Ok(ResolvePass {
    selections,
    modules,
    requests,
  })
}

fn select_reference(
  repository: &str,
  requests: &BTreeSet<DependencyRequest>,
  root_reference: Option<&String>,
) -> Result<String, String> {
  let refs = requests.iter().map(|request| request.reference.as_str()).collect::<BTreeSet<_>>();
  if refs.len() == 1 {
    return Ok(refs.iter().next().expect("one dependency ref").to_string());
  }

  let versions = refs
    .iter()
    .map(|reference| parse_tag_version(reference).map(|version| ((*reference).to_string(), version)))
    .collect::<Option<Vec<_>>>();
  if let Some(mut versions) = versions {
    versions.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
    return Ok(versions.last().expect("non-empty versions").0.clone());
  }

  let mut published_versions = refs
    .iter()
    .filter_map(|reference| parse_tag_version(reference).map(|version| ((*reference).to_string(), version)))
    .collect::<Vec<_>>();
  if !published_versions.is_empty() {
    published_versions.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
    return Ok(published_versions.last().expect("non-empty published versions").0.clone());
  }

  if let Some(root_reference) = root_reference {
    return Ok(root_reference.clone());
  }

  let details = requests
    .iter()
    .map(|request| {
      format!(
        "  {} requested by {}",
        request.reference,
        request.requested_by.as_deref().unwrap_or("root")
      )
    })
    .collect::<Vec<_>>()
    .join("\n");
  Err(format!(
    "cannot choose between incomparable refs for {repository}; add a direct root dependency to decide:\n{details}"
  ))
}

fn parse_tag_version(reference: &str) -> Option<Version> {
  Version::parse(reference.strip_prefix('v').unwrap_or(reference)).ok()
}

fn materialize_module(repository: &str, reference: &str, options: &GraphOptions) -> Result<ResolvedModule, String> {
  let (owner, repo) = repository
    .split_once('/')
    .ok_or_else(|| format!("invalid repository {repository}"))?;
  let temp_root = module_cache_root(&options.modules_dir).join("tmp");
  fs::create_dir_all(&temp_root).map_err(|e| format!("failed to create {}: {e}", temp_root.display()))?;
  let identity = hex::encode(Md5::digest(format!("{repository}\n{reference}").as_bytes()));
  let temp_path = temp_root.join(format!(
    "clone-{}-{}-{}-{identity}",
    std::process::id(),
    sanitize(repository),
    sanitize(reference)
  ));
  if temp_path.exists() {
    fs::remove_dir_all(&temp_path).map_err(|e| format!("failed to clean {}: {e}", temp_path.display()))?;
  }

  eprintln!("resolving {repository}@{reference}");
  let remote = resolve_remote_ref(repository, reference, options.ci)?;
  let source = module_cache_root(&options.modules_dir)
    .join("git")
    .join(owner)
    .join(repo)
    .join(&remote.commit)
    .join("source");
  let _lock = CacheMetadataLock::acquire(&module_cache_root(&options.modules_dir))?;
  if source.exists() {
    validate_store_source(repository, &source, &remote.commit)?;
    ensure_store_metadata_unlocked(&source, repository, reference, &remote.commit)?;
    let dependencies = read_module_dependencies(&source)?;
    return Ok(ResolvedModule {
      repository: repository.to_string(),
      reference: reference.to_string(),
      commit: remote.commit,
      kind: remote.kind,
      link_target: source.clone(),
      source,
      dependencies,
    });
  }
  GitRepo::clone_to_path(&temp_path, &remote.url, reference, &remote.kind, true)
    .map_err(|e| format!("failed to clone {repository}@{reference}: {e}"))?;
  let temp_repo = GitRepo { dir: temp_path.clone() };
  let commit = temp_repo.head_commit()?;
  if commit != remote.commit {
    return Err(format!(
      "remote ref changed while cloning {repository}@{reference}: resolved {} but cloned {commit}; retry the command",
      remote.commit
    ));
  }
  let source = module_cache_root(&options.modules_dir)
    .join("git")
    .join(owner)
    .join(repo)
    .join(&commit)
    .join("source");
  if source.exists() {
    fs::remove_dir_all(&temp_path).map_err(|e| format!("failed to clean {}: {e}", temp_path.display()))?;
    validate_store_source(repository, &source, &commit)?;
  } else {
    let parent = source.parent().ok_or_else(|| format!("invalid store path {}", source.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    if let Err(error) = fs::rename(&temp_path, &source) {
      if source.exists() {
        fs::remove_dir_all(&temp_path).map_err(|e| format!("failed to clean {} after concurrent install: {e}", temp_path.display()))?;
        validate_store_source(repository, &source, &commit)?;
      } else {
        return Err(format!(
          "failed to move {} into store {}: {error}",
          temp_path.display(),
          source.display()
        ));
      }
    }
  }

  ensure_store_metadata_unlocked(&source, repository, reference, &commit)?;

  let dependencies = read_module_dependencies(&source)?;
  Ok(ResolvedModule {
    repository: repository.to_string(),
    reference: reference.to_string(),
    commit,
    kind: remote.kind,
    link_target: source.clone(),
    source,
    dependencies,
  })
}

#[cfg(test)]
fn ensure_store_metadata(source: &Path, repository: &str, reference: &str, commit: &str) -> Result<(), String> {
  let _lock = CacheMetadataLock::acquire(&metadata_cache_root(source))?;
  ensure_store_metadata_unlocked(source, repository, reference, commit)
}

fn ensure_store_metadata_unlocked(source: &Path, repository: &str, reference: &str, commit: &str) -> Result<(), String> {
  let root = source.parent().ok_or_else(|| format!("invalid store path {}", source.display()))?;
  let metadata = root.join(VERSION_STORE_METADATA);
  let retained_reference = fs::read_to_string(&metadata)
    .ok()
    .and_then(|content| metadata_value(&content, "reference"))
    .map_or_else(|| reference.to_string(), |existing| preferred_store_reference(&existing, reference));
  fs::write(
    &metadata,
    format!("repository = {repository}\nreference = {retained_reference}\ncommit = {commit}\n"),
  )
  .map_err(|e| format!("failed to write {}: {e}", metadata.display()))
}

fn preferred_store_reference(existing: &str, observed: &str) -> String {
  match (parse_tag_version(existing), parse_tag_version(observed)) {
    (None, Some(_)) => observed.to_string(),
    (Some(current), Some(candidate)) if candidate > current => observed.to_string(),
    _ => existing.to_string(),
  }
}

fn validate_store_source(repository: &str, source: &Path, expected_commit: &str) -> Result<(), String> {
  let repo = GitRepo { dir: source.to_path_buf() };
  let actual_commit = repo.head_commit().map_err(|e| {
    format!(
      "failed to inspect immutable store entry for {repository} at {}: {e}",
      source.display()
    )
  })?;
  if actual_commit != expected_commit {
    return Err(format!(
      "immutable store entry for {repository} at {} has commit {actual_commit}, expected {expected_commit}; move the damaged entry aside and reinstall",
      source.display()
    ));
  }
  let changes = repo.status_porcelain().map_err(|e| {
    format!(
      "failed to inspect immutable store entry for {repository} at {}: {e}",
      source.display()
    )
  })?;
  if !changes.is_empty() {
    return Err(format!(
      "immutable store entry for {repository} at {} has local changes ({}); move the damaged entry aside and reinstall",
      source.display(),
      changes.join(", ")
    ));
  }
  Ok(())
}

struct RemoteRef {
  kind: RefKind,
  commit: String,
  url: String,
}

fn resolve_remote_ref(repository: &str, reference: &str, ci: bool) -> Result<RemoteRef, String> {
  if is_full_commit_hash(reference) {
    return Ok(RemoteRef {
      kind: RefKind::Commit,
      commit: reference.to_ascii_lowercase(),
      url: format!("https://github.com/{repository}.git"),
    });
  }
  let https_url = format!("https://github.com/{repository}.git");
  match inspect_remote_ref(&https_url, reference) {
    Ok(remote) => Ok(remote),
    Err(https_error) if !ci => {
      let ssh_url = format!("git@github.com:{repository}.git");
      inspect_remote_ref(&ssh_url, reference).map_err(|ssh_error| {
        format!("failed to resolve {repository}@{reference} over HTTPS and SSH:\n  HTTPS: {https_error}\n  SSH: {ssh_error}")
      })
    }
    Err(error) => Err(error),
  }
}

fn is_full_commit_hash(reference: &str) -> bool {
  matches!(reference.len(), 40 | 64) && reference.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn inspect_remote_ref(url: &str, reference: &str) -> Result<RemoteRef, String> {
  let tag_ref = format!("refs/tags/{reference}");
  let peeled_tag_ref = format!("refs/tags/{reference}^{{}}");
  let branch_ref = format!("refs/heads/{reference}");
  let output = Command::new("git")
    .env("GIT_TERMINAL_PROMPT", "0")
    .args(["ls-remote", url, tag_ref.as_str(), peeled_tag_ref.as_str(), branch_ref.as_str()])
    .output()
    .map_err(|e| format!("failed to inspect remote ref {url}@{reference}: {e}"))?;
  if !output.status.success() {
    return Err(format!(
      "failed to inspect remote ref {url}@{reference}: {}",
      String::from_utf8_lossy(&output.stderr).trim()
    ));
  }
  let stdout = String::from_utf8_lossy(&output.stdout);
  let refs = stdout
    .lines()
    .filter_map(|line| line.split_once('\t'))
    .map(|(commit, name)| (name, commit))
    .collect::<BTreeMap<_, _>>();
  let tag_commit = refs.get(peeled_tag_ref.as_str()).or_else(|| refs.get(tag_ref.as_str()));
  let branch_commit = refs.get(branch_ref.as_str());
  match (tag_commit, branch_commit) {
    (Some(commit), None) => Ok(RemoteRef {
      kind: RefKind::Tag,
      commit: (*commit).to_string(),
      url: url.to_string(),
    }),
    (None, Some(commit)) => Ok(RemoteRef {
      kind: RefKind::Branch,
      commit: (*commit).to_string(),
      url: url.to_string(),
    }),
    (Some(_), Some(_)) => Err(format!(
      "ambiguous Git ref {reference} exists as both a tag and branch in {url}; rename one ref"
    )),
    (None, None) => Err(format!("Git ref {reference} was not found in {url}")),
  }
}

fn read_module_dependencies(source: &Path) -> Result<BTreeMap<String, String>, String> {
  let deps_path = source.join("deps.cirru");
  if !deps_path.exists() {
    return Ok(BTreeMap::new());
  }
  let content = fs::read_to_string(&deps_path).map_err(|e| format!("failed to read {}: {e}", deps_path.display()))?;
  let parsed = cirru_edn::parse(&content).map_err(|e| format!("failed to parse {}: {e}", deps_path.display()))?;
  let deps: PackageDeps = parsed.try_into().map_err(|e| format!("invalid {}: {e}", deps_path.display()))?;
  Ok(sorted_dependencies(&deps.dependencies))
}

fn sorted_dependencies(deps: &std::collections::HashMap<Arc<str>, Arc<str>>) -> BTreeMap<String, String> {
  deps
    .iter()
    .map(|(repository, reference)| (repository.to_string(), reference.to_string()))
    .collect()
}

fn sorted_root_dependencies(deps: &PackageDeps) -> Result<BTreeMap<String, String>, String> {
  Ok(sorted_dependencies(&deps.root_dependencies()?))
}

fn validate_module_folders<'a>(repositories: impl Iterator<Item = &'a String>) -> Result<(), String> {
  let mut folders = BTreeMap::<String, String>::new();
  for repository in repositories {
    let folder = module_folder(repository)?.to_string();
    if let Some(existing) = folders.insert(folder.clone(), repository.clone())
      && existing != *repository
    {
      return Err(format!(
        "module folder collision: {existing} and {repository} both map to .calcit/modules/{folder}"
      ));
    }
  }
  Ok(())
}

fn collect_warnings(
  root: &BTreeMap<String, String>,
  requests: &BTreeMap<String, BTreeSet<DependencyRequest>>,
  modules: &BTreeMap<String, ResolvedModule>,
) -> Vec<String> {
  let mut warnings = vec![];
  for (repository, module) in modules {
    match module.kind {
      RefKind::Branch => warnings.push(format!(
        "warning: {repository}@{} is a branch and currently resolves to {}",
        module.reference, module.commit
      )),
      RefKind::Tag if parse_tag_version(&module.reference).is_none() => warnings.push(format!(
        "warning: {repository}@{} is a reproducible tag but not a SemVer version",
        module.reference
      )),
      RefKind::Commit => warnings.push(format!(
        "warning: {repository}@{} is a pinned commit without SemVer release metadata",
        module.reference
      )),
      RefKind::Tag => {}
    }
    if let Some(module_requests) = requests.get(repository) {
      let distinct = module_requests.iter().map(|request| &request.reference).collect::<BTreeSet<_>>();
      if distinct.len() > 1 {
        let reason =
          if root.get(repository) == Some(&module.reference) && distinct.iter().any(|value| parse_tag_version(value).is_none()) {
            "root override"
          } else if distinct.iter().any(|value| parse_tag_version(value).is_none()) {
            "published SemVer preferred over mutable ref"
          } else {
            "highest requested SemVer"
          };
        let sources = module_requests
          .iter()
          .map(|request| format!("{} by {}", request.reference, request.requested_by.as_deref().unwrap_or("root")))
          .collect::<Vec<_>>()
          .join(", ");
        warnings.push(format!(
          "warning: selected {repository}@{} ({reason}); requested {sources}",
          module.reference
        ));
      }
    }
  }
  warnings
}

pub fn install_project_view(graph: &ResolvedGraph, options: &GraphOptions) -> Result<(), String> {
  install_project_view_with(graph, options, register_project_view)
}

fn install_project_view_with<F>(graph: &ResolvedGraph, options: &GraphOptions, register: F) -> Result<(), String>
where
  F: Fn(&GraphOptions) -> Result<(), String>,
{
  let calcit_dir = options.project_root.join(".calcit");
  let _project_lock = project_view_lock(&calcit_dir)?;
  let temp_root = calcit_dir.join("tmp");
  fs::create_dir_all(&temp_root).map_err(|e| format!("failed to create {}: {e}", temp_root.display()))?;
  let local_ignore = calcit_dir.join(".gitignore");
  if !local_ignore.exists() {
    fs::write(&local_ignore, "*\n").map_err(|e| format!("failed to write {}: {e}", local_ignore.display()))?;
  }
  let next_modules = temp_root.join(format!("modules-{}", std::process::id()));
  if next_modules.exists() {
    fs::remove_dir_all(&next_modules).map_err(|e| format!("failed to clean {}: {e}", next_modules.display()))?;
  }
  fs::create_dir_all(&next_modules).map_err(|e| format!("failed to create {}: {e}", next_modules.display()))?;
  let mut view_modes = BTreeMap::new();
  for module in graph.modules.values() {
    let folder = module_folder(&module.repository)?;
    let mode = create_dir_link(&module.link_target, &next_modules.join(folder))?;
    view_modes.insert(module.repository.clone(), mode);
  }

  let modules_path = calcit_dir.join("modules");
  let backup = temp_root.join(format!("modules-backup-{}", std::process::id()));
  if backup.exists() {
    fs::remove_dir_all(&backup).map_err(|e| format!("failed to clean {}: {e}", backup.display()))?;
  }
  if fs::symlink_metadata(&modules_path).is_ok() {
    fs::rename(&modules_path, &backup).map_err(|e| format!("failed to stage old module view: {e}"))?;
  }
  if let Err(error) = fs::rename(&next_modules, &modules_path) {
    if backup.exists() {
      let _ = fs::rename(&backup, &modules_path);
    }
    return Err(format!("failed to activate project module view: {error}"));
  }
  let state_path = calcit_dir.join("caps-state.cirru");
  let state_backup = temp_root.join(format!("caps-state-backup-{}", std::process::id()));
  if state_backup.exists() {
    fs::remove_file(&state_backup).map_err(|e| format!("failed to clean {}: {e}", state_backup.display()))?;
  }
  if fs::symlink_metadata(&state_path).is_ok() {
    fs::rename(&state_path, &state_backup).map_err(|e| {
      let _ = fs::remove_dir_all(&modules_path);
      if backup.exists() {
        let _ = fs::rename(&backup, &modules_path);
      }
      format!("failed to stage old caps state: {e}")
    })?;
  }
  if let Err(error) = write_state(graph, &view_modes, &state_path) {
    fs::remove_dir_all(&modules_path).map_err(|e| format!("{error}; also failed to remove new module view: {e}"))?;
    if backup.exists() {
      fs::rename(&backup, &modules_path).map_err(|e| format!("{error}; also failed to restore old module view: {e}"))?;
    }
    if state_backup.exists() {
      fs::rename(&state_backup, &state_path).map_err(|e| format!("{error}; also failed to restore old caps state: {e}"))?;
    }
    return Err(error);
  }
  if let Err(error) = register(options) {
    fs::remove_dir_all(&modules_path).map_err(|e| format!("{error}; also failed to remove new module view: {e}"))?;
    if backup.exists() {
      fs::rename(&backup, &modules_path).map_err(|e| format!("{error}; also failed to restore old module view: {e}"))?;
    }
    if fs::symlink_metadata(&state_path).is_ok() {
      fs::remove_file(&state_path).map_err(|e| format!("{error}; also failed to remove new caps state: {e}"))?;
    }
    if state_backup.exists() {
      fs::rename(&state_backup, &state_path).map_err(|e| format!("{error}; also failed to restore old caps state: {e}"))?;
    }
    return Err(error);
  }
  if backup.exists() {
    fs::remove_dir_all(&backup).map_err(|e| format!("failed to clean old module view: {e}"))?;
  }
  if state_backup.exists() {
    fs::remove_file(&state_backup).map_err(|e| format!("failed to clean old caps state: {e}"))?;
  }
  Ok(())
}

pub fn check_project_view(graph: &ResolvedGraph, options: &GraphOptions) -> Result<usize, String> {
  let modules_path = options.project_root.join(".calcit/modules");
  let mut issues = 0;
  for module in graph.modules.values() {
    let folder = module_folder(&module.repository)?;
    let link = modules_path.join(folder);
    if fs::symlink_metadata(&link).is_err() {
      println!("{}", format!("- {} is not linked", module.repository).red());
      issues += 1;
      continue;
    }
    let actual = fs::canonicalize(&link).map_err(|e| format!("failed to resolve {}: {e}", link.display()))?;
    let expected_target = expected_link_target(module)?;
    if !expected_target.exists() {
      println!(
        "{}",
        format!(
          "! {} has no native realization for the current ABI/toolchain; run caps to build it",
          module.repository
        )
        .yellow()
      );
      issues += 1;
      continue;
    }
    let expected = fs::canonicalize(&expected_target).map_err(|e| format!("failed to resolve {}: {e}", expected_target.display()))?;
    if actual != expected {
      let mode = if fs::symlink_metadata(&link)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
      {
        "link"
      } else {
        "copy"
      };
      println!(
        "{}",
        format!(
          "! {} uses a non-shared {mode} at {}, expected {}",
          module.repository,
          actual.display(),
          expected.display()
        )
        .yellow()
      );
      issues += 1;
    } else {
      println!("{}", format!("√ {} at {}", module.repository, module.reference).dimmed());
    }
  }
  Ok(issues)
}

pub fn verify_project_view(graph: &ResolvedGraph, options: &GraphOptions) -> Result<usize, String> {
  let modules_path = options.project_root.join(".calcit/modules");
  let mut issues = 0;
  for module in graph.modules.values() {
    let repo = GitRepo {
      dir: module.source.clone(),
    };
    let commit = repo
      .head_commit()
      .map_err(|e| format!("failed to inspect {}: {e}", module.source.display()))?;
    if commit != module.commit {
      println!(
        "{}",
        format!("! {} store commit is {commit}, expected {}", module.repository, module.commit).red()
      );
      issues += 1;
    }
    let changes = repo
      .status_porcelain()
      .map_err(|e| format!("failed to inspect {}: {e}", module.source.display()))?;
    if !changes.is_empty() {
      println!(
        "{}",
        format!("! {} immutable source has local changes: {}", module.repository, changes.join(", ")).red()
      );
      issues += 1;
    }
    let expected_target = expected_link_target(module)?;
    let link = modules_path.join(module_folder(&module.repository)?);
    if fs::symlink_metadata(&link).is_err() {
      println!("{}", format!("- {} is not linked", module.repository).red());
      issues += 1;
      continue;
    }
    let actual = fs::canonicalize(&link).map_err(|e| format!("failed to resolve {}: {e}", link.display()))?;
    if !expected_target.exists() {
      println!(
        "{}",
        format!(
          "! {} has no native realization for the current ABI/toolchain; run caps to build it",
          module.repository
        )
        .red()
      );
      issues += 1;
      continue;
    }
    let expected = fs::canonicalize(&expected_target).map_err(|e| format!("failed to resolve {}: {e}", expected_target.display()))?;
    if actual != expected {
      println!(
        "{}",
        format!(
          "! {} points to {}, expected {}",
          module.repository,
          actual.display(),
          expected.display()
        )
        .red()
      );
      issues += 1;
    }
    if expected_target != module.source {
      match verify_native_receipt(module, &expected_target) {
        Err(error) => {
          println!("{}", format!("! {} native receipt is invalid: {error}", module.repository).red());
          issues += 1;
        }
        Ok(libraries) => {
          let extension = match std::env::consts::OS {
            "macos" => "dylib",
            "windows" => "dll",
            _ => "so",
          };
          let libraries = libraries
            .into_iter()
            .filter(|path| path.extension().is_some_and(|value| value == extension))
            .collect::<Vec<_>>();
          if libraries.is_empty() {
            println!(
              "{}",
              format!("! {} has no .{extension} library under dylibs/", module.repository).red()
            );
            issues += 1;
          }
          for library in libraries {
            let output = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
              .arg("__verify-native")
              .arg(&library)
              .output()
              .map_err(|e| format!("failed to start native verifier for {}: {e}", library.display()))?;
            if !output.status.success() {
              println!(
                "{}",
                format!(
                  "! {} failed native verification for {}: {}",
                  module.repository,
                  library.display(),
                  String::from_utf8_lossy(&output.stderr).trim()
                )
                .red()
              );
              issues += 1;
            }
          }
        }
      }
    }
  }
  if issues == 0 {
    println!("verified {} module(s)", graph.modules.len());
  }
  Ok(issues)
}

pub fn verify_native_library(path: &Path) -> Result<(), String> {
  type VersionFn = fn() -> String;
  let library = unsafe { libloading::Library::new(path) }.map_err(|e| format!("failed to load {}: {e}", path.display()))?;
  let abi: libloading::Symbol<VersionFn> =
    unsafe { library.get(b"abi_version") }.map_err(|e| format!("failed to read abi_version from {}: {e}", path.display()))?;
  let actual_abi = abi();
  if actual_abi != calcit::FFI_ABI_VERSION {
    return Err(format!(
      "FFI ABI mismatch in {}: found {actual_abi}, expected {}",
      path.display(),
      calcit::FFI_ABI_VERSION
    ));
  }
  let edn: libloading::Symbol<VersionFn> =
    unsafe { library.get(b"edn_version") }.map_err(|e| format!("failed to read edn_version from {}: {e}", path.display()))?;
  let actual_edn = edn();
  if actual_edn != cirru_edn::version() {
    return Err(format!(
      "cirru_edn mismatch in {}: found {actual_edn}, expected {}",
      path.display(),
      cirru_edn::version()
    ));
  }
  Ok(())
}

#[cfg(unix)]
fn create_dir_link(target: &Path, link: &Path) -> Result<ProjectModuleMode, String> {
  std::os::unix::fs::symlink(target, link)
    .map(|_| ProjectModuleMode::Link)
    .map_err(|e| format!("failed to link {} -> {}: {e}", link.display(), target.display()))
}

#[cfg(windows)]
fn create_dir_link(target: &Path, link: &Path) -> Result<ProjectModuleMode, String> {
  let junction = Command::new("cmd").args(["/C", "mklink", "/J"]).arg(link).arg(target).output();
  if let Ok(output) = junction
    && output.status.success()
  {
    return Ok(ProjectModuleMode::Link);
  }

  match std::os::windows::fs::symlink_dir(target, link) {
    Ok(()) => Ok(ProjectModuleMode::Link),
    Err(link_error) => copy_tree(target, link).map(|_| ProjectModuleMode::Copy).map_err(|copy_error| {
      format!(
        "failed to create junction or directory symlink {} -> {} ({link_error}); copy fallback also failed: {copy_error}",
        link.display(),
        target.display()
      )
    }),
  }
}

fn write_state(graph: &ResolvedGraph, view_modes: &BTreeMap<String, ProjectModuleMode>, state_path: &Path) -> Result<(), String> {
  let mut root = EdnMapView::default();
  let mut modules = EdnMapView::default();
  for (repository, module) in &graph.modules {
    let mut info = EdnMapView::default();
    info.insert(Edn::tag("ref"), Edn::str(module.reference.as_str()));
    info.insert(Edn::tag("commit"), Edn::str(module.commit.as_str()));
    info.insert(Edn::tag("path"), Edn::str(module.link_target.to_string_lossy().as_ref()));
    if let Some(mode) = view_modes.get(repository) {
      info.insert(Edn::tag("view-mode"), Edn::str(mode.as_str()));
    }
    modules.insert(Edn::str(repository.as_str()), Edn::Map(info));
  }
  root.insert(Edn::tag("modules"), Edn::Map(modules));
  root.insert(
    Edn::tag("warnings"),
    Edn::List(
      graph
        .warnings
        .iter()
        .map(|warning| Edn::str(warning.as_str()))
        .collect::<Vec<_>>()
        .into(),
    ),
  );
  let content = cirru_edn::format(&Edn::Map(root), false)?;
  let temp = state_path.with_extension(format!("cirru.{}.tmp", std::process::id()));
  fs::write(&temp, content).map_err(|e| format!("failed to write {}: {e}", temp.display()))?;
  fs::rename(&temp, state_path).map_err(|e| format!("failed to activate {}: {e}", state_path.display()))
}

fn prepare_native_realization(module: &ResolvedModule, _options: &GraphOptions) -> Result<PathBuf, String> {
  if !module.source.join("build.sh").exists() {
    return Ok(module.source.clone());
  }
  let realization = expected_link_target(module)?;
  let build_key = realization.file_name().and_then(|name| name.to_str()).unwrap_or("unknown");
  let receipt = realization.join(".calcit-native.cirru");
  if receipt.exists() {
    verify_native_receipt(module, &realization)?;
    return Ok(realization);
  }
  let realization_parent = realization
    .parent()
    .ok_or_else(|| format!("invalid native realization path {}", realization.display()))?;
  fs::create_dir_all(realization_parent).map_err(|e| format!("failed to create {}: {e}", realization_parent.display()))?;
  let temp = realization_parent.join(format!(".tmp-native-{}-{}", std::process::id(), module_folder(&module.repository)?));
  if temp.exists() {
    fs::remove_dir_all(&temp).map_err(|e| format!("failed to clean {}: {e}", temp.display()))?;
  }
  copy_tree(&module.source, &temp)?;
  eprintln!(
    "building native module {}@{} ({}) with {}",
    module.repository,
    module.reference,
    module.commit,
    temp.join("build.sh").display()
  );
  call_build_script(&temp)?;
  let dylibs = temp.join("dylibs");
  if !dylibs.exists() || fs::read_dir(&dylibs).map_err(|e| e.to_string())?.next().is_none() {
    return Err(format!("native module {} did not produce files under dylibs/", module.repository));
  }
  write_native_receipt(module, &temp, build_key)?;
  if realization.exists() {
    fs::remove_dir_all(&temp).map_err(|e| format!("failed to clean {}: {e}", temp.display()))?;
  } else if let Err(error) = fs::rename(&temp, &realization) {
    if realization.exists() {
      fs::remove_dir_all(&temp).map_err(|e| format!("failed to clean {} after concurrent native build: {e}", temp.display()))?;
    } else {
      return Err(format!("failed to store native realization: {error}"));
    }
  }
  Ok(realization)
}

fn expected_link_target(module: &ResolvedModule) -> Result<PathBuf, String> {
  if !module.source.join("build.sh").exists() {
    return Ok(module.source.clone());
  }
  let target = rust_target_identity();
  let build_input = format!(
    "{target}\ncalcit={CALCIT_VERSION}\nffi-abi={}\ncirru-edn={}\ncommit={}",
    calcit::FFI_ABI_VERSION,
    cirru_edn::version(),
    module.commit
  );
  let build_key = hex::encode(Md5::digest(build_input.as_bytes()));
  let commit_root = module
    .source
    .parent()
    .ok_or_else(|| format!("invalid source path {}", module.source.display()))?;
  Ok(commit_root.join("realizations").join(build_key))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeArtifact {
  relative_path: String,
  size: u64,
  md5: String,
}

fn write_native_receipt(module: &ResolvedModule, realization: &Path, build_key: &str) -> Result<(), String> {
  let artifacts = collect_native_artifacts(realization)?;
  let mut receipt = EdnMapView::default();
  receipt.insert(Edn::tag("module"), Edn::str(module.repository.as_str()));
  receipt.insert(Edn::tag("commit"), Edn::str(module.commit.as_str()));
  receipt.insert(Edn::tag("calcit-version"), Edn::str(CALCIT_VERSION));
  receipt.insert(Edn::tag("ffi-abi"), Edn::str(calcit::FFI_ABI_VERSION));
  receipt.insert(Edn::tag("cirru-edn"), Edn::str(cirru_edn::version()));
  receipt.insert(Edn::tag("build-key"), Edn::str(build_key));
  receipt.insert(
    Edn::tag("artifacts"),
    Edn::List(
      artifacts
        .values()
        .map(|artifact| {
          Edn::map_from_iter([
            (Edn::tag("path"), Edn::str(artifact.relative_path.as_str())),
            (Edn::tag("size"), Edn::Number(artifact.size as f64)),
            (Edn::tag("md5"), Edn::str(artifact.md5.as_str())),
          ])
        })
        .collect::<Vec<_>>()
        .into(),
    ),
  );
  let content = cirru_edn::format(&Edn::Map(receipt), false)?;
  fs::write(realization.join(".calcit-native.cirru"), content).map_err(|e| format!("failed to write native receipt: {e}"))
}

fn verify_native_receipt(module: &ResolvedModule, realization: &Path) -> Result<Vec<PathBuf>, String> {
  let receipt_path = realization.join(".calcit-native.cirru");
  let content = fs::read_to_string(&receipt_path).map_err(|e| format!("failed to read {}: {e}", receipt_path.display()))?;
  let receipt = cirru_edn::parse(&content)
    .map_err(|e| format!("failed to parse {}: {e}", receipt_path.display()))?
    .view_map()?;
  let expected_build_key = realization.file_name().and_then(|name| name.to_str()).unwrap_or("unknown");
  for (key, expected) in [
    ("module", module.repository.as_str()),
    ("commit", module.commit.as_str()),
    ("calcit-version", CALCIT_VERSION),
    ("ffi-abi", calcit::FFI_ABI_VERSION),
    ("cirru-edn", cirru_edn::version()),
    ("build-key", expected_build_key),
  ] {
    let actual = receipt
      .tag_get(key)
      .ok_or_else(|| format!("receipt is missing :{key}"))?
      .read_string()?;
    if actual != expected {
      return Err(format!("receipt :{key} is {actual:?}, expected {expected:?}"));
    }
  }

  let declared = receipt
    .tag_get("artifacts")
    .ok_or_else(|| "receipt is missing :artifacts".to_string())?
    .view_list()?
    .0
    .iter()
    .map(native_artifact_from_edn)
    .collect::<Result<Vec<_>, _>>()?;
  let declared = declared
    .into_iter()
    .map(|artifact| {
      if !is_safe_artifact_path(&artifact.relative_path) {
        return Err(format!(
          "receipt artifact path {:?} is not a safe relative dylibs path",
          artifact.relative_path
        ));
      }
      Ok((artifact.relative_path.clone(), artifact))
    })
    .collect::<Result<BTreeMap<_, _>, String>>()?;
  if declared.len() != receipt.tag_get("artifacts").expect("artifacts was read above").view_list()?.0.len() {
    return Err("receipt has duplicate artifact paths".to_string());
  }
  let actual = collect_native_artifacts(realization)?;
  if actual != declared {
    return Err("receipt artifacts do not match the current dylibs contents".to_string());
  }
  Ok(actual.keys().map(|relative_path| realization.join(relative_path)).collect())
}

fn native_artifact_from_edn(value: &Edn) -> Result<NativeArtifact, String> {
  let map = value.view_map()?;
  let relative_path = map
    .tag_get("path")
    .ok_or_else(|| "receipt artifact is missing :path".to_string())?
    .read_string()?;
  let size = map
    .tag_get("size")
    .ok_or_else(|| "receipt artifact is missing :size".to_string())?
    .read_number()?;
  if size < 0.0 || size.fract().abs() > f64::EPSILON || size > u64::MAX as f64 {
    return Err(format!("receipt artifact size must be a non-negative integer, got {size}"));
  }
  let md5 = map
    .tag_get("md5")
    .ok_or_else(|| "receipt artifact is missing :md5".to_string())?
    .read_string()?;
  if md5.len() != 32 || !md5.bytes().all(|byte| byte.is_ascii_hexdigit()) {
    return Err(format!("receipt artifact md5 is invalid: {md5:?}"));
  }
  Ok(NativeArtifact {
    relative_path,
    size: size as u64,
    md5: md5.to_ascii_lowercase(),
  })
}

fn is_safe_artifact_path(value: &str) -> bool {
  let path = Path::new(value);
  !path.is_absolute() && path.starts_with("dylibs") && path.components().all(|component| matches!(component, Component::Normal(_)))
}

fn collect_native_artifacts(realization: &Path) -> Result<BTreeMap<String, NativeArtifact>, String> {
  let root = fs::canonicalize(realization).map_err(|e| format!("failed to resolve {}: {e}", realization.display()))?;
  let dylibs = realization.join("dylibs");
  let dylibs_metadata =
    fs::symlink_metadata(&dylibs).map_err(|e| format!("native realization is missing {}: {e}", dylibs.display()))?;
  if !dylibs_metadata.is_dir() || dylibs_metadata.file_type().is_symlink() {
    return Err(format!("{} must be a real directory, not a symlink", dylibs.display()));
  }
  let mut artifacts = BTreeMap::new();
  collect_native_artifact_dir(realization, &root, &dylibs, &mut artifacts)?;
  if artifacts.is_empty() {
    return Err(format!("native realization has no artifacts under {}", dylibs.display()));
  }
  Ok(artifacts)
}

fn collect_native_artifact_dir(
  realization: &Path,
  canonical_root: &Path,
  directory: &Path,
  artifacts: &mut BTreeMap<String, NativeArtifact>,
) -> Result<(), String> {
  for entry in fs::read_dir(directory).map_err(|e| format!("failed to read {}: {e}", directory.display()))? {
    let entry = entry.map_err(|e| format!("failed to read native artifact entry: {e}"))?;
    let path = entry.path();
    let metadata = fs::symlink_metadata(&path).map_err(|e| format!("failed to inspect {}: {e}", path.display()))?;
    if metadata.file_type().is_symlink() {
      return Err(format!("native artifact {} must not be a symlink", path.display()));
    }
    if metadata.is_dir() {
      collect_native_artifact_dir(realization, canonical_root, &path, artifacts)?;
    } else if metadata.is_file() {
      let canonical_path = fs::canonicalize(&path).map_err(|e| format!("failed to resolve {}: {e}", path.display()))?;
      if !canonical_path.starts_with(canonical_root) {
        return Err(format!("native artifact {} escapes {}", path.display(), realization.display()));
      }
      let relative_path = path
        .strip_prefix(realization)
        .map_err(|e| format!("failed to make artifact path relative for {}: {e}", path.display()))?
        .to_string_lossy()
        .replace('\\', "/");
      let content = fs::read(&path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
      artifacts.insert(
        relative_path.clone(),
        NativeArtifact {
          relative_path,
          size: metadata.len(),
          md5: hex::encode(Md5::digest(&content)),
        },
      );
    } else {
      return Err(format!("native artifact {} has an unsupported file type", path.display()));
    }
  }
  Ok(())
}

fn rust_target_identity() -> String {
  Command::new("rustc")
    .arg("-vV")
    .output()
    .ok()
    .filter(|output| output.status.success())
    .map(|output| String::from_utf8_lossy(&output.stdout).replace('\n', "-"))
    .unwrap_or_else(|| format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH))
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), String> {
  fs::create_dir_all(target).map_err(|e| format!("failed to create {}: {e}", target.display()))?;
  for entry in fs::read_dir(source).map_err(|e| format!("failed to read {}: {e}", source.display()))? {
    let entry = entry.map_err(|e| e.to_string())?;
    if entry.file_name() == ".git" || entry.file_name() == "target" {
      continue;
    }
    let source_path = entry.path();
    let target_path = target.join(entry.file_name());
    let kind = entry.file_type().map_err(|e| e.to_string())?;
    if kind.is_dir() {
      copy_tree(&source_path, &target_path)?;
    } else if kind.is_file() {
      fs::copy(&source_path, &target_path)
        .map_err(|e| format!("failed to copy {} to {}: {e}", source_path.display(), target_path.display()))?;
    } else if kind.is_symlink() {
      return Err(format!(
        "native module source contains unsupported symlink {}; replace it with a regular file or directory",
        source_path.display()
      ));
    } else {
      return Err(format!(
        "native module source contains unsupported file type at {}",
        source_path.display()
      ));
    }
  }
  Ok(())
}

pub fn print_warnings(graph: &ResolvedGraph) {
  for warning in &graph.warnings {
    eprintln!("{}", warning.yellow());
  }
}

pub fn print_tree(graph: &ResolvedGraph) {
  println!("root");
  let mut seen = BTreeSet::new();
  for (index, repository) in graph.root.keys().enumerate() {
    let last = index + 1 == graph.root.len();
    print_tree_module(graph, repository, "", last, &mut seen);
  }
}

fn print_tree_module(graph: &ResolvedGraph, repository: &str, prefix: &str, last: bool, seen: &mut BTreeSet<String>) {
  let branch = if last { "└─" } else { "├─" };
  let Some(module) = graph.modules.get(repository) else {
    println!("{prefix}{branch} {repository} (missing)");
    return;
  };
  let repeated = !seen.insert(repository.to_string());
  println!(
    "{prefix}{branch} {}@{}{}",
    repository,
    module.reference,
    if repeated { " (*)" } else { "" }
  );
  if repeated {
    return;
  }
  let next_prefix = format!("{prefix}{}", if last { "   " } else { "│  " });
  for (index, dependency) in module.dependencies.keys().enumerate() {
    print_tree_module(graph, dependency, &next_prefix, index + 1 == module.dependencies.len(), seen);
  }
}

pub fn print_why(graph: &ResolvedGraph, target: &str) -> Result<(), String> {
  if !graph.modules.contains_key(target) {
    return Err(format!("module {target} is not in the resolved dependency graph"));
  }
  for root in graph.root.keys() {
    if let Some(path) = shortest_path(graph, root, target) {
      println!("{}", path.join(" -> "));
    }
  }
  if let Some(requests) = graph.requests.get(target) {
    println!("requests:");
    for request in requests {
      println!("  {} by {}", request.reference, request.requested_by.as_deref().unwrap_or("root"));
    }
  }
  Ok(())
}

fn shortest_path(graph: &ResolvedGraph, root: &str, target: &str) -> Option<Vec<String>> {
  let mut queue = std::collections::VecDeque::from([vec![root.to_string()]]);
  let mut seen = BTreeSet::new();
  while let Some(path) = queue.pop_front() {
    let current = path.last()?;
    if current == target {
      return Some(path);
    }
    if !seen.insert(current.clone()) {
      continue;
    }
    if let Some(module) = graph.modules.get(current) {
      for child in module.dependencies.keys() {
        let mut next = path.clone();
        next.push(child.clone());
        queue.push_back(next);
      }
    }
  }
  None
}

fn sanitize(value: &str) -> String {
  value
    .chars()
    .map(|ch| {
      if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
        ch
      } else {
        '-'
      }
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::{
    DependencyRequest, GraphOptions, RefKind, ResolvedGraph, ResolvedModule, clean_version_store, ensure_store_metadata,
    install_project_view_with, is_full_commit_hash, parse_tag_version, read_module_dependencies, select_reference,
    sorted_root_dependencies, verify_native_receipt, write_modules_agents, write_native_receipt,
  };
  use crate::PackageDeps;
  use std::collections::{BTreeSet, HashMap};
  use std::fs;
  use std::sync::{Arc, Barrier};

  fn request(reference: &str, requested_by: Option<&str>) -> DependencyRequest {
    DependencyRequest {
      reference: reference.to_string(),
      requested_by: requested_by.map(str::to_string),
    }
  }

  #[test]
  fn selects_highest_requested_semver() {
    let requests = BTreeSet::from([request("0.9.2", Some("a@1.0.0")), request("0.10.0", None)]);
    assert_eq!(
      select_reference("org/repo", &requests, Some(&"0.10.0".to_string())),
      Ok("0.10.0".to_string())
    );
    assert!(parse_tag_version("v1.2.3").is_some());
  }

  #[test]
  fn incomparable_refs_need_root_decision() {
    let requests = BTreeSet::from([request("main", Some("a@1.0.0")), request("next", Some("b@1.0.0"))]);
    assert!(select_reference("org/repo", &requests, None).is_err());
    assert_eq!(
      select_reference("org/repo", &requests, Some(&"main".to_string())),
      Ok("main".to_string())
    );
  }

  #[test]
  fn published_tag_wins_over_transitive_branch() {
    let requests = BTreeSet::from([request("0.0.6", Some("stable@1.0.0")), request("main", Some("legacy@1.0.0"))]);
    assert_eq!(select_reference("org/repo", &requests, None), Ok("0.0.6".to_string()));
  }

  #[test]
  fn root_includes_development_dependencies() {
    let deps = PackageDeps {
      version: None,
      calcit_version: None,
      dependencies: HashMap::from([(Arc::from("org/runtime"), Arc::from("1.0.0"))]),
      dev_dependencies: HashMap::from([(Arc::from("org/test"), Arc::from("main"))]),
    };
    let root = sorted_root_dependencies(&deps).unwrap();
    assert_eq!(root.get("org/runtime").map(String::as_str), Some("1.0.0"));
    assert_eq!(root.get("org/test").map(String::as_str), Some("main"));
  }

  #[test]
  fn transitive_module_excludes_development_dependencies() {
    let root = std::env::temp_dir().join(format!("calcit-caps-transitive-dev-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(
      root.join("deps.cirru"),
      "{} (:dependencies $ {} (|org/runtime |1.0.0)) (:dev-dependencies $ {} (|org/test |main))",
    )
    .unwrap();
    let dependencies = read_module_dependencies(&root).unwrap();
    assert_eq!(dependencies.get("org/runtime").map(String::as_str), Some("1.0.0"));
    assert!(!dependencies.contains_key("org/test"));
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn conflicting_root_dependency_groups_are_rejected() {
    let deps = PackageDeps {
      version: None,
      calcit_version: None,
      dependencies: HashMap::from([(Arc::from("org/shared"), Arc::from("1.0.0"))]),
      dev_dependencies: HashMap::from([(Arc::from("org/shared"), Arc::from("main"))]),
    };
    assert!(sorted_root_dependencies(&deps).is_err());
  }

  #[test]
  fn highest_published_tag_wins_over_lower_root_tag() {
    let requests = BTreeSet::from([
      request("0.16.32", None),
      request("0.16.67", Some("newer@1.0.0")),
      request("main", Some("legacy@1.0.0")),
    ]);
    assert_eq!(
      select_reference("org/repo", &requests, Some(&"0.16.32".to_string())),
      Ok("0.16.67".to_string())
    );
  }

  #[test]
  fn accepts_only_full_commit_hashes() {
    assert!(is_full_commit_hash("0123456789abcdef0123456789abcdef01234567"));
    assert!(!is_full_commit_hash("0123456789abcdef"));
    assert!(!is_full_commit_hash("g123456789abcdef0123456789abcdef01234567"));
  }

  #[test]
  fn native_receipt_rejects_changed_artifacts() {
    let root = std::env::temp_dir().join(format!("calcit-caps-receipt-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let realization = root.join("build-key");
    let dylibs = realization.join("dylibs");
    fs::create_dir_all(&dylibs).unwrap();
    let library = dylibs.join("libdemo.so");
    fs::write(&library, b"first build").unwrap();
    let module = ResolvedModule {
      repository: "calcit-lang/demo".to_string(),
      reference: "0.1.0".to_string(),
      commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
      kind: RefKind::Tag,
      source: root.join("source"),
      link_target: realization.clone(),
      dependencies: Default::default(),
    };
    write_native_receipt(&module, &realization, "build-key").unwrap();
    assert_eq!(verify_native_receipt(&module, &realization).unwrap(), vec![library.clone()]);
    fs::write(&library, b"modified build").unwrap();
    assert!(verify_native_receipt(&module, &realization).is_err());
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn clean_keeps_the_latest_semver_revision_for_each_module() {
    let root = std::env::temp_dir().join(format!("calcit-caps-clean-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let modules_dir = root.join("modules");
    for (commit, version) in [("old", "1.2.0"), ("new", "1.10.0")] {
      let revision = root.join("module-caches/git/org/demo").join(commit);
      fs::create_dir_all(revision.join("source")).unwrap();
      fs::write(
        revision.join("metadata.txt"),
        format!("repository = org/demo\nreference = {version}\ncommit = {commit}\n"),
      )
      .unwrap();
    }

    let cleanup = clean_version_store(&modules_dir).unwrap();
    assert_eq!(cleanup.modules, 1);
    assert_eq!(cleanup.kept, 1);
    assert_eq!(cleanup.removed, 1);
    assert!(!root.join("module-caches/git/org/demo/old").exists());
    assert!(root.join("module-caches/git/org/demo/new/source").exists());
    fs::remove_dir_all(root).unwrap();
  }

  #[cfg(unix)]
  #[test]
  fn clean_keeps_revisions_linked_by_registered_project_views() {
    let root = std::env::temp_dir().join(format!("calcit-caps-clean-linked-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let modules_dir = root.join("modules");
    let cache = root.join("module-caches/git/org/demo");
    for (commit, version) in [("old", "1.2.0"), ("new", "1.10.0")] {
      let revision = cache.join(commit);
      fs::create_dir_all(revision.join("source")).unwrap();
      fs::write(
        revision.join("metadata.txt"),
        format!("repository = org/demo\nreference = {version}\ncommit = {commit}\n"),
      )
      .unwrap();
    }
    let project = root.join("project");
    let project_modules = project.join(".calcit/modules");
    fs::create_dir_all(&project_modules).unwrap();
    std::os::unix::fs::symlink(cache.join("old/source"), project_modules.join("demo")).unwrap();
    let registry = root.join("module-caches/projects");
    fs::create_dir_all(&registry).unwrap();
    fs::write(registry.join("project.path"), format!("{}\n", project.display())).unwrap();

    let cleanup = clean_version_store(&modules_dir).unwrap();
    assert_eq!(cleanup.modules, 1);
    assert_eq!(cleanup.kept, 2);
    assert_eq!(cleanup.removed, 0);
    assert!(cache.join("old/source").exists());
    assert!(cache.join("new/source").exists());
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn store_metadata_retains_the_highest_observed_semver_reference() {
    let root = std::env::temp_dir().join(format!("calcit-caps-metadata-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let source = root.join("revision/source");
    fs::create_dir_all(&source).unwrap();

    ensure_store_metadata(&source, "org/demo", "main", "same-commit").unwrap();
    ensure_store_metadata(&source, "org/demo", "1.10.0", "same-commit").unwrap();
    ensure_store_metadata(&source, "org/demo", "1.2.0", "same-commit").unwrap();

    let metadata = fs::read_to_string(root.join("revision/metadata.txt")).unwrap();
    assert!(metadata.contains("reference = 1.10.0"));
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn concurrent_store_metadata_updates_retain_the_highest_semver_reference() {
    let root = std::env::temp_dir().join(format!("calcit-caps-metadata-concurrent-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let source = root.join("revision/source");
    fs::create_dir_all(&source).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let first_barrier = barrier.clone();
    let first_source = source.clone();
    let first = std::thread::spawn(move || {
      first_barrier.wait();
      ensure_store_metadata(&first_source, "org/demo", "1.10.0", "same-commit")
    });
    let second_barrier = barrier.clone();
    let second_source = source.clone();
    let second = std::thread::spawn(move || {
      second_barrier.wait();
      ensure_store_metadata(&second_source, "org/demo", "1.2.0", "same-commit")
    });
    barrier.wait();
    first.join().unwrap().unwrap();
    second.join().unwrap().unwrap();

    let metadata = fs::read_to_string(root.join("revision/metadata.txt")).unwrap();
    assert!(metadata.contains("reference = 1.10.0"));
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn project_view_registration_failure_restores_previous_view_and_state() {
    let root = std::env::temp_dir().join(format!("calcit-caps-view-rollback-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let project_root = root.join("project");
    let calcit_dir = project_root.join(".calcit");
    let modules_path = calcit_dir.join("modules");
    fs::create_dir_all(&modules_path).unwrap();
    fs::write(modules_path.join("old.txt"), "old view").unwrap();
    fs::write(calcit_dir.join("caps-state.cirru"), "old state").unwrap();

    let graph = ResolvedGraph {
      root: Default::default(),
      modules: Default::default(),
      requests: Default::default(),
      warnings: vec![],
    };
    let options = GraphOptions {
      project_root: project_root.clone(),
      modules_dir: root.join("modules"),
      ci: false,
      strict: false,
      build_native: false,
    };
    let result = install_project_view_with(&graph, &options, |_| Err("registration failed".to_string()));
    assert_eq!(result.unwrap_err(), "registration failed");
    assert_eq!(fs::read_to_string(modules_path.join("old.txt")).unwrap(), "old view");
    assert_eq!(fs::read_to_string(calcit_dir.join("caps-state.cirru")).unwrap(), "old state");
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn module_cache_agents_file_is_overwritten_with_release_workflow() {
    let root = std::env::temp_dir().join(format!("calcit-caps-agents-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let modules_dir = root.join("modules");
    fs::create_dir_all(root.join("module-caches")).unwrap();
    fs::write(root.join("module-caches/AGENTS.md"), "stale").unwrap();

    write_modules_agents(&modules_dir).unwrap();
    let content = fs::read_to_string(root.join("module-caches/AGENTS.md")).unwrap();
    assert!(content.contains("Do not edit a cached module in place."));
    assert!(content.contains("publish a new SemVer tag"));
    assert!(!content.contains("stale"));
    fs::remove_dir_all(root).unwrap();
  }
}
