use crate::git::GitRepo;
use crate::{CALCIT_VERSION, PackageDeps, call_build_script, module_folder};
use cirru_edn::{Edn, EdnMapView};
use colored::Colorize;
use md5::{Digest, Md5};
use semver::Version;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

const MAX_PARALLEL_RESOLVES: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyRequest {
  pub reference: String,
  pub requested_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefKind {
  Tag,
  Branch,
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

pub fn resolve_graph(root_deps: &PackageDeps, options: &GraphOptions) -> Result<ResolvedGraph, String> {
  let root = sorted_dependencies(root_deps);
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
  let temp_root = options.modules_dir.join(".store/tmp");
  fs::create_dir_all(&temp_root).map_err(|e| format!("failed to create {}: {e}", temp_root.display()))?;
  let temp_path = temp_root.join(format!(
    "clone-{}-{}-{}",
    std::process::id(),
    sanitize(repository),
    sanitize(reference)
  ));
  if temp_path.exists() {
    fs::remove_dir_all(&temp_path).map_err(|e| format!("failed to clean {}: {e}", temp_path.display()))?;
  }

  eprintln!("resolving {repository}@{reference}");
  let remote = resolve_remote_ref(repository, reference, options.ci)?;
  let source = options
    .modules_dir
    .join(".store/git")
    .join(owner)
    .join(repo)
    .join(&remote.commit)
    .join("source");
  if source.exists() {
    validate_store_source(repository, &source, &remote.commit)?;
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
  GitRepo::clone_to_path(&temp_path, &remote.url, reference, true)
    .map_err(|e| format!("failed to clone {repository}@{reference}: {e}"))?;
  let temp_repo = GitRepo { dir: temp_path.clone() };
  let commit = temp_repo.head_commit()?;
  if commit != remote.commit {
    return Err(format!(
      "remote ref changed while cloning {repository}@{reference}: resolved {} but cloned {commit}; retry the command",
      remote.commit
    ));
  }
  let source = options
    .modules_dir
    .join(".store/git")
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
  Ok(sorted_dependencies(&deps))
}

fn sorted_dependencies(deps: &PackageDeps) -> BTreeMap<String, String> {
  deps
    .dependencies
    .iter()
    .map(|(repository, reference)| (repository.to_string(), reference.to_string()))
    .collect()
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
  let calcit_dir = options.project_root.join(".calcit");
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
  for module in graph.modules.values() {
    let folder = module_folder(&module.repository)?;
    create_dir_link(&module.link_target, &next_modules.join(folder))?;
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
  if let Err(error) = write_state(graph, &calcit_dir.join("caps-state.cirru")) {
    fs::remove_dir_all(&modules_path).map_err(|e| format!("{error}; also failed to remove new module view: {e}"))?;
    if backup.exists() {
      fs::rename(&backup, &modules_path).map_err(|e| format!("{error}; also failed to restore old module view: {e}"))?;
    }
    return Err(error);
  }
  if backup.exists() {
    fs::remove_dir_all(&backup).map_err(|e| format!("failed to clean old module view: {e}"))?;
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
    let expected =
      fs::canonicalize(&module.link_target).map_err(|e| format!("failed to resolve {}: {e}", module.link_target.display()))?;
    let realization_root = module
      .source
      .parent()
      .map(|parent| parent.join("realizations"))
      .and_then(|path| fs::canonicalize(path).ok());
    let is_known_realization = realization_root.as_ref().is_some_and(|root| actual.starts_with(root));
    if actual != expected && !is_known_realization {
      println!(
        "{}",
        format!(
          "! {} points to {}, expected {}",
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
      let receipt = expected_target.join(".calcit-native.cirru");
      let dylibs = expected_target.join("dylibs");
      if !receipt.exists() || !dylibs.exists() {
        println!(
          "{}",
          format!(
            "! {} native realization is missing its receipt or dylibs directory",
            module.repository
          )
          .red()
        );
        issues += 1;
      } else {
        let extension = match std::env::consts::OS {
          "macos" => "dylib",
          "windows" => "dll",
          _ => "so",
        };
        let libraries = fs::read_dir(&dylibs)
          .map_err(|e| format!("failed to read {}: {e}", dylibs.display()))?
          .filter_map(Result::ok)
          .map(|entry| entry.path())
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
fn create_dir_link(target: &Path, link: &Path) -> Result<(), String> {
  std::os::unix::fs::symlink(target, link).map_err(|e| format!("failed to link {} -> {}: {e}", link.display(), target.display()))
}

#[cfg(windows)]
fn create_dir_link(target: &Path, link: &Path) -> Result<(), String> {
  let junction = Command::new("cmd").args(["/C", "mklink", "/J"]).arg(link).arg(target).output();
  if let Ok(output) = junction
    && output.status.success()
  {
    return Ok(());
  }

  std::os::windows::fs::symlink_dir(target, link).map_err(|error| {
    format!(
      "failed to create junction or directory symlink {} -> {}: {error}",
      link.display(),
      target.display()
    )
  })
}

fn write_state(graph: &ResolvedGraph, state_path: &Path) -> Result<(), String> {
  let mut root = EdnMapView::default();
  let mut modules = EdnMapView::default();
  for (repository, module) in &graph.modules {
    let mut info = EdnMapView::default();
    info.insert(Edn::tag("ref"), Edn::str(module.reference.as_str()));
    info.insert(Edn::tag("commit"), Edn::str(module.commit.as_str()));
    info.insert(Edn::tag("path"), Edn::str(module.link_target.to_string_lossy().as_ref()));
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

fn prepare_native_realization(module: &ResolvedModule, options: &GraphOptions) -> Result<PathBuf, String> {
  if !module.source.join("build.sh").exists() {
    return Ok(module.source.clone());
  }
  let realization = expected_link_target(module)?;
  let build_key = realization.file_name().and_then(|name| name.to_str()).unwrap_or("unknown");
  let receipt = realization.join(".calcit-native.cirru");
  if receipt.exists() {
    return Ok(realization);
  }
  let temp =
    options
      .project_root
      .join(".calcit/tmp")
      .join(format!("native-{}-{}", std::process::id(), module_folder(&module.repository)?));
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
  fs::write(
    temp.join(".calcit-native.cirru"),
    format!(
      "{{}} (:module |{}) (:commit |{}) (:calcit-version |{}) (:ffi-abi |{}) (:cirru-edn |{}) (:build-key |{})\n",
      module.repository,
      module.commit,
      CALCIT_VERSION,
      calcit::FFI_ABI_VERSION,
      cirru_edn::version(),
      build_key
    ),
  )
  .map_err(|e| format!("failed to write native receipt: {e}"))?;
  if let Some(parent) = realization.parent() {
    fs::create_dir_all(parent).map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
  }
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
  use super::{DependencyRequest, parse_tag_version, select_reference};
  use std::collections::BTreeSet;

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
}
