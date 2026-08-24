pub mod data;

pub mod builtins;
pub mod calcit;
pub mod call_graph_diff;
pub mod call_stack;
pub mod call_tree;
pub mod cli_args;
pub mod codegen;
pub mod def_diff;
pub mod detailed_snapshot;
pub mod effects_graph;
pub mod program;
pub mod program_diff;
pub mod project_state;
pub mod runner;
pub mod snapshot;
pub mod util;

use calcit::{CalcitErrKind, LocatedWarning};
use call_stack::CallStackList;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub use calcit::{
  Calcit, CalcitErr, CalcitFnTypeAnnotation, CalcitProc, CalcitSyntax, CalcitTypeAnnotation, ProcArity, ProcTypeSignature,
  SyntaxTypeSignature,
};

use crate::util::string::strip_shebang;

pub const DEFAULT_SNAPSHOT_FILE: &str = "calcit.cirru";
pub const LEGACY_SNAPSHOT_FILE: &str = "compact.cirru";
pub const FFI_ABI_VERSION: &str = "0.0.9";

static QUIET_TOOL_OUTPUT: AtomicBool = AtomicBool::new(false);

pub fn set_quiet_tool_output(v: bool) {
  QUIET_TOOL_OUTPUT.store(v, Ordering::Relaxed);
}

pub fn quiet_tool_output() -> bool {
  QUIET_TOOL_OUTPUT.load(Ordering::Relaxed)
}

fn core_snapshot_schema_needs_fallback(snapshot: &snapshot::Snapshot) -> bool {
  let Some(core_file) = snapshot.files.get("calcit.core") else {
    return true;
  };
  let Some(map_entry) = core_file.defs.get("map") else {
    return true;
  };
  let CalcitTypeAnnotation::Fn(fn_annot) = map_entry.schema.as_ref() else {
    return true;
  };

  fn_annot.where_bounds.is_empty() || !matches!(fn_annot.arg_types.get(1).map(|arg| arg.as_ref()), Some(CalcitTypeAnnotation::Fn(_)))
}

fn load_core_snapshot_from_embedded_source() -> Result<snapshot::Snapshot, String> {
  let content = include_str!("../src/cirru/calcit-core.cirru");
  let data = cirru_edn::parse(content).map_err(|e| format!("Failed to parse embedded core snapshot source: {e}"))?;
  snapshot::load_snapshot_data(&data, "calcit-internal://calcit-core.cirru")
}

fn overlay_core_schemas_from_source(snapshot: &mut snapshot::Snapshot) -> Result<(), String> {
  let source_snapshot = load_core_snapshot_from_embedded_source()?;

  for (ns, source_file) in &source_snapshot.files {
    let Some(target_file) = snapshot.files.get_mut(ns) else {
      continue;
    };

    for (def, source_entry) in &source_file.defs {
      let Some(target_entry) = target_file.defs.get_mut(def) else {
        continue;
      };

      if !matches!(source_entry.schema.as_ref(), CalcitTypeAnnotation::Dynamic) {
        target_entry.schema = source_entry.schema.clone();
      }
    }
  }

  Ok(())
}

pub fn load_core_snapshot() -> Result<snapshot::Snapshot, String> {
  // load core libs
  let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/calcit-core.rmp"));
  let mut snapshot = snapshot::decode_binary_snapshot(bytes).map_err(|e| {
    eprintln!("\n{e}");
    "Failed to deserialize core snapshot".to_string()
  })?;
  if core_snapshot_schema_needs_fallback(&snapshot) {
    overlay_core_schemas_from_source(&mut snapshot)?;
  }
  let path = "calcit-internal://calcit-core.cirru";
  let meta_ns = format!("{}.$meta", snapshot.package);
  snapshot.files.insert(meta_ns.to_owned(), snapshot::gen_meta_ns(&meta_ns, path));
  Ok(snapshot)
}

#[derive(Clone, Debug)]
pub struct ProgramEntries {
  pub init_fn: Arc<str>,
  pub init_ns: Arc<str>,
  pub init_def: Arc<str>,
  pub reload_fn: Arc<str>,
  pub reload_ns: Arc<str>,
  pub reload_def: Arc<str>,
}

pub fn run_program(init_ns: Arc<str>, init_def: Arc<str>, params: &[Calcit]) -> Result<Calcit, CalcitErr> {
  run_program_with_docs(init_ns, init_def, params)
}

pub fn resolve_snapshot_path_alias(path: &Path) -> PathBuf {
  if path.exists() {
    return path.to_path_buf();
  }

  match path.file_name().and_then(|name| name.to_str()) {
    Some(DEFAULT_SNAPSHOT_FILE) => {
      let fallback = path.with_file_name(LEGACY_SNAPSHOT_FILE);
      if fallback.exists() { fallback } else { path.to_path_buf() }
    }
    _ => path.to_path_buf(),
  }
}

fn module_path_candidates(path: &str) -> Vec<String> {
  if path.ends_with('/') {
    vec![format!("{path}{DEFAULT_SNAPSHOT_FILE}"), format!("{path}{LEGACY_SNAPSHOT_FILE}")]
  } else if path.ends_with(DEFAULT_SNAPSHOT_FILE) {
    vec![
      path.to_string(),
      format!(
        "{}{}",
        path.strip_suffix(DEFAULT_SNAPSHOT_FILE).unwrap_or(path),
        LEGACY_SNAPSHOT_FILE
      ),
    ]
  } else {
    vec![path.to_string()]
  }
}

fn materialize_module_path(file_path: &str, base_dir: &Path, module_folder: &Path) -> PathBuf {
  if file_path.starts_with("./") {
    base_dir.join(file_path)
  } else if Path::new(file_path).is_absolute() {
    Path::new(file_path).to_owned()
  } else {
    module_folder.join(file_path)
  }
}

/// The module view installed for the project that owns a snapshot.
///
/// `caps` materializes this directory with links into its immutable global store. Runtime
/// module loading must use this view rather than reaching into the store directly, so every
/// project uses the revisions it resolved.
pub fn project_module_folder(base_dir: &Path) -> PathBuf {
  base_dir.join(".calcit/modules")
}

fn module_candidate_display_path(file_path: &str, fullpath: &Path, module_folder: &Path) -> String {
  if file_path.starts_with("./") {
    file_path.to_string()
  } else if Path::new(file_path).is_absolute() {
    if let Ok(stripped) = fullpath.strip_prefix(module_folder) {
      format!("<mods>/{}", stripped.display())
    } else {
      file_path.to_string()
    }
  } else {
    format!("<mods>/{file_path}")
  }
}

pub fn resolve_module_snapshot_candidates(path: &str, base_dir: &Path, module_folder: &Path) -> Vec<(String, PathBuf, String)> {
  let candidates = module_path_candidates(path);
  let mut items = candidates
    .iter()
    .map(|candidate| {
      let fullpath = materialize_module_path(candidate, base_dir, module_folder);
      let display_path = module_candidate_display_path(candidate, &fullpath, module_folder);
      (candidate.clone(), fullpath, display_path)
    })
    .collect::<Vec<_>>();

  if !items.iter().any(|(_, fullpath, _)| fullpath.exists())
    && let Some((file_path, fullpath, display_path)) = items.first().cloned()
  {
    return vec![(file_path, fullpath, display_path)];
  }

  items.retain(|(_, fullpath, _)| fullpath.exists());
  items
}

pub fn resolve_module_snapshot_path(path: &str, base_dir: &Path, module_folder: &Path) -> (String, PathBuf, String) {
  resolve_module_snapshot_candidates(path, base_dir, module_folder)
    .into_iter()
    .next()
    .unwrap_or_else(|| {
      let fullpath = materialize_module_path(path, base_dir, module_folder);
      let display_path = module_candidate_display_path(path, &fullpath, module_folder);
      (path.to_string(), fullpath, display_path)
    })
}

pub fn run_program_with_docs(init_ns: Arc<str>, init_def: Arc<str>, params: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let check_warnings = RefCell::new(LocatedWarning::default_list());

  match runner::preprocess::ensure_ns_def_compiled(&init_ns, &init_def, &check_warnings, &CallStackList::default()) {
    Ok(()) => {}
    Err(failure) => {
      eprintln!("\nfailed preprocessing, {failure}");
      let headline = failure.headline();
      call_stack::display_stack_with_docs(&headline, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;
      return CalcitErr::err_str(failure.kind, headline);
    }
  };

  let warnings = check_warnings.borrow();
  if !warnings.is_empty() {
    return Err(CalcitErr {
      kind: CalcitErrKind::Unexpected,
      msg: format!("Found {} warnings, runner blocked", warnings.len()),
      code: None,
      warnings: Box::new(warnings.to_owned()),
      stack: CallStackList::default(),
      location: None,
      hint: None,
    });
  }

  match runner::evaluate_symbol_from_program(&init_def, &init_ns, None, &CallStackList::default()) {
    Ok(entry) => match entry {
      Calcit::Fn { info, .. } => {
        let result = runner::run_fn(params, &info, &CallStackList::default());
        match result {
          Ok(v) => Ok(v),
          Err(failure) => {
            call_stack::display_stack_with_docs(&failure.msg, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;
            Err(failure)
          }
        }
      }
      _ => CalcitErr::err_str(CalcitErrKind::Type, format!("expected function entry, got: {entry}")),
    },
    Err(failure) => {
      call_stack::display_stack_with_docs(&failure.msg, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;
      Err(failure)
    }
  }
}

pub fn load_module(path: &str, base_dir: &Path, module_folder: &Path) -> Result<snapshot::Snapshot, String> {
  let mut loaded = HashSet::new();
  load_module_recursive(path, base_dir, module_folder, &mut loaded)
}

fn load_module_recursive(
  path: &str,
  base_dir: &Path,
  module_folder: &Path,
  loaded: &mut HashSet<PathBuf>,
) -> Result<snapshot::Snapshot, String> {
  let candidates = resolve_module_snapshot_candidates(path, base_dir, module_folder);
  let mut last_error: Option<String> = None;

  for (_, fullpath, display_path) in candidates {
    if loaded.contains(&fullpath) {
      return Ok(snapshot::Snapshot {
        package: String::new(),
        about: None,
        version: String::new(),
        entries: HashMap::new(),
        files: HashMap::new(),
        active_entry: snapshot::DEFAULT_ENTRY_NAME.to_owned(),
      });
    }
    let mut content = match fs::read_to_string(&fullpath) {
      Ok(content) => content,
      Err(e) => {
        last_error = Some(format!("Failed to read {}: {e}", fullpath.display()));
        continue;
      }
    };

    strip_shebang(&mut content);
    let data = match cirru_edn::parse(&content) {
      Ok(data) => data,
      Err(e) => {
        last_error = Some(format!("Failed to parse file '{}': {e}", fullpath.display()));
        continue;
      }
    };

    match snapshot::load_snapshot_data(&data, &fullpath.display().to_string()) {
      Ok(mut snapshot) => {
        if !loaded.insert(fullpath.clone()) {
          return Ok(snapshot);
        }

        let dependencies = snapshot.active_entry()?.modules.clone();
        for dependency in dependencies {
          let dependency_snapshot = load_module_recursive(&dependency, base_dir, module_folder, loaded)?;
          merge_module_files(&mut snapshot, &dependency_snapshot, &dependency)?;
        }

        if !quiet_tool_output() {
          println!("loading: {display_path}");
        }
        return Ok(snapshot);
      }
      Err(e) => {
        last_error = Some(format!("Failed to load snapshot '{}': {e}", fullpath.display()));
      }
    }
  }

  Err(last_error.unwrap_or_else(|| format!("expected Cirru snapshot for module path: {path}")))
}

/// Merge a module and its transitive dependencies while tolerating the same
/// namespace being listed both directly and transitively. Same-package
/// namespaces are preserved when a cycle revisits them; different content
/// under a cross-package namespace remains an error because it indicates an
/// invalid module graph or a mismatched dependency resolution.
pub fn merge_module_files(target: &mut snapshot::Snapshot, module: &snapshot::Snapshot, module_path: &str) -> Result<(), String> {
  let target_namespace_prefix = format!("{}.", target.package);
  for (namespace, file) in &module.files {
    if module.package == target.package
      && target.files.contains_key(namespace)
      && (namespace == &target.package || namespace.starts_with(&target_namespace_prefix))
    {
      continue;
    }
    if let Some(existing) = target.files.get(namespace) {
      if existing == file {
        continue;
      }
      return Err(format!(
        "namespace `{namespace}` conflicts with existing content when loading module `{module_path}`"
      ));
    }
    target.files.insert(namespace.to_owned(), file.to_owned());
  }
  Ok(())
}

/// Merge a direct dependency into a project snapshot.
///
/// A library's development graph can revisit that library through a transitive
/// dependency (for example, a documentation UI that renders Markdown). The
/// project source is authoritative for its own package namespaces, so discard
/// those transitive copies before applying the normal, strict module merge.
pub fn merge_project_module_files(
  target: &mut snapshot::Snapshot,
  module: &snapshot::Snapshot,
  module_path: &str,
) -> Result<(), String> {
  if target.package.is_empty() {
    return merge_module_files(target, module, module_path);
  }

  let namespace_prefix = format!("{}.", target.package);
  if !module
    .files
    .keys()
    .any(|namespace| namespace == &target.package || namespace.starts_with(&namespace_prefix))
  {
    return merge_module_files(target, module, module_path);
  }

  let mut filtered_module = module.clone();
  filtered_module
    .files
    .retain(|namespace, _| namespace != &target.package && !namespace.starts_with(&namespace_prefix));
  merge_module_files(target, &filtered_module, module_path)
}

#[cfg(test)]
mod module_resolution_tests {
  use super::{load_module, merge_module_files, merge_project_module_files, project_module_folder, resolve_module_snapshot_candidates};
  use std::fs;
  use std::path::{Path, PathBuf};

  fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("calcit-module-resolution-{label}-{}", std::process::id()))
  }

  fn write_module(root: &Path, name: &str, dependencies: &[&str], namespace: &str) {
    let module_dir = root.join(".calcit/modules").join(name);
    fs::create_dir_all(&module_dir).unwrap();
    let modules = if dependencies.is_empty() {
      "[]".to_owned()
    } else {
      format!(
        "[] {}",
        dependencies.iter().map(|dep| format!("|{dep}/")).collect::<Vec<_>>().join(" ")
      )
    };
    let content = format!(
      "{} (:package |{name})\n  :version |0.0.0\n  :entries $ {{}}\n    :default $ {{}} (:mode :native) (:init-fn |{namespace}/main!) (:reload-fn |{namespace}/main!)\n      :modules $ {modules}\n  :files $ {{}}\n    |{namespace} $ %{{}} :FileEntry\n      :ns $ %{{}} :CodeEntry (:doc |) (:code $ quote (ns {namespace})) (:examples $ []) (:schema nil)\n      :defs $ {{}}\n        |main! $ %{{}} :CodeEntry (:doc |) (:code $ quote (defn main! () nil)) (:examples $ []) (:schema nil)\n",
      "{}"
    );
    fs::write(module_dir.join("calcit.cirru"), content).unwrap();
  }

  #[test]
  fn project_module_view_is_the_only_root_for_named_modules() {
    let root = temp_root("project-first");
    let project = root.join("project");
    let global = root.join("global");
    fs::create_dir_all(project.join(".calcit/modules/demo")).unwrap();
    fs::create_dir_all(global.join("demo")).unwrap();
    fs::write(project.join(".calcit/modules/demo/compact.cirru"), "project").unwrap();
    fs::write(global.join("demo/calcit.cirru"), "global").unwrap();

    let module_folder = project_module_folder(&project);
    let candidates = resolve_module_snapshot_candidates("demo/", &project, &module_folder);
    assert_eq!(candidates[0].1, project.join(".calcit/modules/demo/compact.cirru"));
    assert_eq!(candidates[0].2, "<mods>/demo/compact.cirru");
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn named_modules_do_not_fall_back_to_the_global_store() {
    let root = temp_root("no-global-fallback");
    let project = root.join("project");
    let global = root.join("global");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(global.join("demo")).unwrap();
    fs::write(global.join("demo/calcit.cirru"), "global").unwrap();

    let module_folder = project_module_folder(&project);
    let candidates = resolve_module_snapshot_candidates("demo/", &project, &module_folder);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].1, project.join(".calcit/modules/demo/calcit.cirru"));
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn load_module_includes_transitive_dependencies_and_terminates_cycles() {
    let root = temp_root("transitive");
    write_module(&root, "a", &["b"], "a");
    write_module(&root, "b", &["c"], "b");
    write_module(&root, "c", &[], "c");
    let module_folder = project_module_folder(&root);
    let snapshot = load_module("a/", &root, &module_folder).unwrap();
    assert!(snapshot.files.contains_key("a"));
    assert!(snapshot.files.contains_key("b"));
    assert!(snapshot.files.contains_key("c"));

    write_module(&root, "cycle-a", &["cycle-b"], "cycle-a");
    write_module(&root, "cycle-b", &["cycle-a"], "cycle-b");
    let cycle = load_module("cycle-a/", &root, &module_folder).unwrap();
    assert!(cycle.files.contains_key("cycle-a"));
    assert!(cycle.files.contains_key("cycle-b"));
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn merge_module_files_rejects_conflicting_namespaces() {
    let root = temp_root("conflict");
    write_module(&root, "one", &[], "shared");
    write_module(&root, "two", &[], "shared");
    let second_path = root.join(".calcit/modules/two/calcit.cirru");
    let second = fs::read_to_string(&second_path)
      .unwrap()
      .replace("(defn main! () nil)", "(defn main! (x) x)");
    fs::write(second_path, second).unwrap();
    let module_folder = project_module_folder(&root);
    let mut target = load_module("one/", &root, &module_folder).unwrap();
    let other = load_module("two/", &root, &module_folder).unwrap();
    let error = merge_module_files(&mut target, &other, "two/").unwrap_err();
    assert!(error.contains("namespace `shared` conflicts with existing content"));
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn merge_module_files_reuses_namespaces_for_same_package_modules() {
    let root = temp_root("same-package-meta");
    write_module(&root, "one", &[], "one");
    write_module(&root, "two", &[], "two");
    let module_folder = project_module_folder(&root);
    let mut target = load_module("one/", &root, &module_folder).unwrap();
    let original_meta = target.files.get("one.$meta").unwrap().clone();
    let original_file = target.files.get("one").unwrap().clone();
    let mut dependency = load_module("two/", &root, &module_folder).unwrap();
    dependency.package = target.package.clone();
    dependency.files.remove("two.$meta");
    dependency.files.insert(
      "one.$meta".to_owned(),
      super::snapshot::gen_meta_ns("one.$meta", "dependency/calcit.cirru"),
    );
    dependency
      .files
      .insert("one".to_owned(), super::snapshot::gen_meta_ns("one", "dependency/calcit.cirru"));
    dependency.files.insert(
      "one.extra".to_owned(),
      super::snapshot::gen_meta_ns("one.extra", "dependency/calcit.cirru"),
    );
    merge_module_files(&mut target, &dependency, "two/").unwrap();
    assert_eq!(target.files.get("one.$meta").unwrap(), &original_meta);
    assert_eq!(target.files.get("one").unwrap(), &original_file);
    assert!(target.files.contains_key("one.extra"));
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn merge_module_files_rejects_cross_package_target_namespaces() {
    let root = temp_root("cross-package-target-namespace");
    write_module(&root, "one", &[], "one");
    write_module(&root, "two", &[], "two");
    let module_folder = project_module_folder(&root);
    let mut target = load_module("one/", &root, &module_folder).unwrap();
    let mut dependency = load_module("two/", &root, &module_folder).unwrap();
    dependency
      .files
      .insert("one".to_owned(), super::snapshot::gen_meta_ns("one", "dependency/calcit.cirru"));
    let error = merge_module_files(&mut target, &dependency, "two/").unwrap_err();
    assert!(error.contains("namespace `one` conflicts with existing content"));
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn project_merge_keeps_project_namespaces_over_transitive_copies() {
    let root = temp_root("project-self-dependency");
    write_module(&root, "one", &[], "one");
    write_module(&root, "two", &[], "two");
    let module_folder = project_module_folder(&root);
    let mut target = load_module("one/", &root, &module_folder).unwrap();
    let original = target.files.get("one").unwrap().clone();
    let original_extra = super::snapshot::gen_meta_ns("one.extra", "project/calcit.cirru");
    target.files.insert("one.extra".to_owned(), original_extra.clone());
    let mut dependency = load_module("two/", &root, &module_folder).unwrap();
    dependency
      .files
      .insert("one".to_owned(), super::snapshot::gen_meta_ns("one", "dependency/calcit.cirru"));
    dependency.files.insert(
      "one.extra".to_owned(),
      super::snapshot::gen_meta_ns("one.extra", "dependency/calcit.cirru"),
    );
    dependency.files.insert(
      "two.extra".to_owned(),
      super::snapshot::gen_meta_ns("two.extra", "dependency/calcit.cirru"),
    );

    merge_project_module_files(&mut target, &dependency, "two/").unwrap();
    assert_eq!(target.files.get("one").unwrap(), &original);
    assert_eq!(target.files.get("one.extra").unwrap(), &original_extra);
    assert!(target.files.contains_key("two.extra"));

    let mut conflicting_target = load_module("one/", &root, &module_folder).unwrap();
    conflicting_target.files.insert(
      "two.extra".to_owned(),
      super::snapshot::gen_meta_ns("two.extra", "project/calcit.cirru"),
    );
    let error = merge_project_module_files(&mut conflicting_target, &dependency, "two/").unwrap_err();
    assert!(error.contains("namespace `two.extra` conflicts with existing content"));
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn explicit_relative_modules_do_not_use_project_module_view() {
    let root = temp_root("relative");
    let project = root.join("project");
    let global = root.join("global");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("util.cirru"), "relative").unwrap();

    let candidates = resolve_module_snapshot_candidates("./util.cirru", &project, &global);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].1, project.join("./util.cirru"));
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn absolute_module_paths_do_not_use_project_module_view() {
    let root = temp_root("absolute");
    let project = root.join("project");
    let global = root.join("global");
    let module = root.join("external.cirru");
    fs::create_dir_all(&project).unwrap();
    fs::write(&module, "absolute").unwrap();

    let candidates = resolve_module_snapshot_candidates(module.to_str().unwrap(), &project, &global);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].1, module);
    fs::remove_dir_all(root).unwrap();
  }
}
