//! CLI tool to download packages from github,
//! packages are defined in `deps.cirru` file
//!
//! files are stored in `~/.config/calcit/modules/`.

mod caps_graph;
mod git;

use argh::{self, FromArgs};

use caps_graph::*;
use cirru_edn::Edn;
use colored::*;
use semver::Version;
use std::{
  collections::HashMap,
  fs,
  io::Write,
  path::{Path, PathBuf},
  process::Command,
  sync::Arc,
  thread,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageDeps {
  version: Option<String>,
  calcit_version: Option<String>,
  dependencies: HashMap<Arc<str>, Arc<str>>,
  dev_dependencies: HashMap<Arc<str>, Arc<str>>,
}

impl TryFrom<Edn> for PackageDeps {
  type Error = String;

  fn try_from(value: Edn) -> Result<Self, Self::Error> {
    let deps_info = value.view_map()?;
    let dependencies = parse_dependency_group(&deps_info, "dependencies")?;
    let dev_dependencies = parse_dependency_group(&deps_info, "dev-dependencies")?;
    let expected_version: Option<String> = match deps_info.get_or_nil("calcit-version") {
      Edn::Str(s) => Some((*s).to_owned()),
      Edn::Nil => None,
      v => return Err(format!("invalid calcit-version: {v}")),
    };
    let package_version: Option<String> = match deps_info.get_or_nil("version") {
      Edn::Str(s) => Some((*s).to_owned()),
      Edn::Nil => None,
      v => return Err(format!("invalid version: {v}")),
    };
    Ok(PackageDeps {
      version: package_version,
      calcit_version: expected_version,
      dependencies,
      dev_dependencies,
    })
  }
}

impl PackageDeps {
  fn root_dependencies(&self) -> Result<HashMap<Arc<str>, Arc<str>>, String> {
    let mut root = self.dependencies.clone();
    for (repository, reference) in &self.dev_dependencies {
      if let Some(runtime_reference) = root.get(repository)
        && runtime_reference != reference
      {
        return Err(format!(
          "root dependency {repository} is declared in dependencies as {runtime_reference} and dev-dependencies as {reference}; keep one declaration or use the same ref"
        ));
      }
      root.insert(repository.clone(), reference.clone());
    }
    Ok(root)
  }
}

fn parse_dependency_group(deps_info: &cirru_edn::EdnMapView, field: &str) -> Result<HashMap<Arc<str>, Arc<str>>, String> {
  #[allow(clippy::mutable_key_type)]
  let dict = match deps_info.get_or_nil(field) {
    Edn::Nil => cirru_edn::EdnMapView::default().0,
    value => value.view_map()?.0,
  };
  let mut dependencies = HashMap::new();
  for (key, value) in &dict {
    match (key, value) {
      (Edn::Str(key), Edn::Str(value)) => {
        dependencies.insert(key.to_owned(), value.to_owned());
      }
      _ => return Err(format!("invalid {field} entry: {key} {value}")),
    }
  }
  Ok(dependencies)
}

pub fn main() -> Result<(), String> {
  // parse deps.cirru

  let raw_args = std::env::args().collect::<Vec<_>>();
  if raw_args.get(1).is_some_and(|arg| arg == "__verify-native") {
    let path = raw_args.get(2).ok_or_else(|| "missing native library path".to_string())?;
    verify_native_library(Path::new(path))?;
    return Ok(());
  }

  let cli_args: TopLevelCaps = argh::from_env();
  if cli_args.pull_branch {
    eprintln!("[Warn] --pull-branch is deprecated; branch refs are always resolved from the remote");
  }
  if let Some(SubCommand::Download(dep_names)) = &cli_args.subcommand {
    if dep_names.packages.is_empty() {
      eprintln!("Error: no packages to download!");
      std::process::exit(1);
    }
    let dict: HashMap<Arc<str>, Arc<str>> = dep_names
      .packages
      .iter()
      .map(|s| {
        let (org_and_folder, version) = s.split_once('@').ok_or("invalid name")?;
        Ok((org_and_folder.to_owned().into(), version.to_owned().into()))
      })
      .collect::<Result<_, String>>()?;
    download_deps(
      PackageDeps {
        version: None,
        calcit_version: None,
        dependencies: dict,
        dev_dependencies: Default::default(),
      },
      cli_args,
    )?;
    return Ok(());
  }

  // if file exists

  if Path::new(&cli_args.input).exists() {
    let content = fs::read_to_string(&cli_args.input).map_err(|e| e.to_string())?;
    let parsed = cirru_edn::parse(&content).map_err(|e| {
      eprintln!("\nFailed to parse '{}':", cli_args.input);
      eprintln!("{e}");
      format!("Failed to parse '{}'", cli_args.input)
    })?;
    let deps: PackageDeps = parsed.try_into()?;

    if deps.version.is_none() {
      eprintln!(
        "[Warn] {} has no :version; initialize the project version with `caps {} version set <version>`",
        cli_args.input, cli_args.input
      );
    }

    if let Some(version) = &deps.calcit_version
      && version != CALCIT_VERSION
    {
      eprintln!("[Warn] calcit version mismatch, deps.cirru expected {version}, running {CALCIT_VERSION}");
    }

    match &cli_args.subcommand {
      Some(SubCommand::Outdated(opts)) => {
        let updated = outdated_tags(deps, &cli_args.input, opts.yes)?;
        if updated {
          // Re-read deps.cirru and download updated dependencies
          println!("\nDownloading updated dependencies...");
          let content = fs::read_to_string(&cli_args.input).map_err(|e| e.to_string())?;
          let parsed = cirru_edn::parse(&content).map_err(|e| {
            eprintln!("\nFailed to parse '{}':", cli_args.input);
            eprintln!("{e}");
            format!("Failed to parse '{}'", cli_args.input)
          })?;
          let updated_deps: PackageDeps = parsed.try_into()?;
          download_deps(updated_deps, cli_args)?;
        }
      }
      Some(SubCommand::Upgrade(opts)) => {
        let updated = upgrade_packages(deps, &cli_args.input, opts)?;
        if updated {
          // Re-read deps.cirru and download updated dependencies
          println!("\nDownloading updated dependencies...");
          let content = fs::read_to_string(&cli_args.input).map_err(|e| e.to_string())?;
          let parsed = cirru_edn::parse(&content).map_err(|e| {
            eprintln!("\nFailed to parse '{}':", cli_args.input);
            eprintln!("{e}");
            format!("Failed to parse '{}'", cli_args.input)
          })?;
          let updated_deps: PackageDeps = parsed.try_into()?;
          download_deps(updated_deps, cli_args)?;
        }
      }
      Some(SubCommand::Add(opts)) => {
        if opts.packages.is_empty() {
          return Err("no packages to add".to_string());
        }

        let mut updated_deps = deps;
        for raw in &opts.packages {
          let split_at = raw.rfind('@').filter(|index| {
            let path_boundary = raw.rfind(['/', ':']).unwrap_or(0);
            *index > path_boundary
          });
          let (package, inline_version) = split_at.map_or((raw.as_str(), None), |index| {
            let (package, version) = raw.split_at(index);
            (package, version.strip_prefix('@').filter(|version| !version.is_empty()))
          });
          let org_and_folder = normalize_package_name(package)?;
          let target = if opts.dev {
            &mut updated_deps.dev_dependencies
          } else {
            &mut updated_deps.dependencies
          };
          target.insert(org_and_folder.into(), inline_version.unwrap_or(&opts.version).to_owned().into());
        }

        updated_deps.root_dependencies()?;
        write_deps_file(&cli_args.input, &updated_deps)?;
        println!("updated {}", cli_args.input.green());
        download_deps(updated_deps, cli_args)?;
      }
      Some(SubCommand::Remove(opts)) => {
        if opts.packages.is_empty() {
          return Err("no packages to remove".to_string());
        }

        let mut updated_deps = deps;
        for raw in &opts.packages {
          let org_and_folder = normalize_package_name(raw)?;
          let target = if opts.dev {
            &mut updated_deps.dev_dependencies
          } else {
            &mut updated_deps.dependencies
          };
          target.remove(org_and_folder.as_str());
        }

        write_deps_file(&cli_args.input, &updated_deps)?;
        println!("updated {}", cli_args.input.green());
        download_deps(updated_deps, cli_args)?;
      }
      Some(SubCommand::Status(_)) => {
        let graph = resolve_for_cli(&deps, &cli_args, false)?;
        print_warnings(&graph);
        let issues = check_project_view(&graph, &graph_options(&cli_args, false)?)?;
        if issues > 0 {
          return Err(format!("{issues} module(s) are not in the expected state"));
        }
      }
      Some(SubCommand::Verify(_)) => {
        let graph = resolve_for_cli(&deps, &cli_args, false)?;
        print_warnings(&graph);
        let issues = verify_project_view(&graph, &graph_options(&cli_args, false)?)?;
        if issues > 0 {
          return Err(format!("{issues} module verification issue(s) found"));
        }
      }
      Some(SubCommand::Tree(_)) => {
        let graph = resolve_for_cli(&deps, &cli_args, false)?;
        print_warnings(&graph);
        print_tree(&graph);
      }
      Some(SubCommand::Why(opts)) => {
        let graph = resolve_for_cli(&deps, &cli_args, false)?;
        print_warnings(&graph);
        print_why(&graph, &normalize_package_name(&opts.package)?)?;
      }
      Some(SubCommand::Version(opts)) => {
        handle_version_command(deps, &cli_args.input, opts)?;
      }
      Some(SubCommand::Reset(_)) => {
        let graph = resolve_for_cli(&deps, &cli_args, true)?;
        print_warnings(&graph);
        let graph_options = graph_options(&cli_args, true)?;
        install_project_view(&graph, &graph_options)?;
        println!(
          "restored {} module link(s) under {}",
          graph.modules.len(),
          graph_options.project_root.join(".calcit/modules").display()
        );
      }
      Some(SubCommand::Download(dep_names)) => {
        unreachable!("already handled: {:?}", dep_names);
      }
      None => {
        download_deps(deps, cli_args)?;
      }
    }

    Ok(())
  } else if Path::new("package.cirru").exists() {
    eprintln!("{}", "Error: 'package.cirru' is deprecated!".red().bold());
    eprintln!("Please rename it to 'deps.cirru':");
    eprintln!("  {}", "mv package.cirru deps.cirru".yellow());
    std::process::exit(1);
  } else {
    eprintln!("Error: no {} found!", cli_args.input);
    std::process::exit(1);
  }
}

fn download_deps(deps: PackageDeps, options: TopLevelCaps) -> Result<(), String> {
  let graph = resolve_for_cli(&deps, &options, !options.ci)?;
  print_warnings(&graph);
  let graph_options = graph_options(&options, !options.ci)?;
  install_project_view(&graph, &graph_options)?;
  println!(
    "installed {} module(s) into {}",
    graph.modules.len(),
    graph_options.project_root.join(".calcit/modules").display()
  );
  Ok(())
}

fn graph_options(options: &TopLevelCaps, build_native: bool) -> Result<GraphOptions, String> {
  let input = PathBuf::from(&options.input);
  let absolute_input = if input.is_absolute() {
    input
  } else {
    std::env::current_dir().map_err(|e| e.to_string())?.join(input)
  };
  let project_root = absolute_input
    .parent()
    .ok_or_else(|| format!("cannot determine project directory from {}", absolute_input.display()))?
    .to_path_buf();
  Ok(GraphOptions {
    project_root,
    modules_dir: modules_dir(options)?,
    ci: options.ci,
    strict: options.strict,
    build_native,
  })
}

fn resolve_for_cli(deps: &PackageDeps, options: &TopLevelCaps, build_native: bool) -> Result<ResolvedGraph, String> {
  resolve_graph(deps, &graph_options(options, build_native)?)
}

fn modules_dir(options: &TopLevelCaps) -> Result<PathBuf, String> {
  if let Some(path) = std::env::var_os("CALCIT_MODULES_DIR") {
    return Ok(PathBuf::from(path));
  }
  let dir = if options.local_debug {
    ".config/calcit/test-modules"
  } else {
    ".config/calcit/modules"
  };
  Ok(dirs::home_dir().ok_or("no config dir")?.join(dir))
}

pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// Top-level command.
struct TopLevelCaps {
  /// verbose mode
  #[argh(switch, short = 'v')]
  verbose: bool,

  /// outdated command
  #[argh(subcommand)]
  subcommand: Option<SubCommand>,

  /// deprecated compatibility flag; branch refs resolve remotely
  #[argh(switch)]
  pull_branch: bool,
  /// CI mode loads shallow repo via HTTPS
  #[argh(switch)]
  ci: bool,
  /// debug mode, clone to test-modules/
  #[argh(switch)]
  local_debug: bool,

  /// reject branch and version-conflict warnings
  #[argh(switch)]
  strict: bool,

  /// input file
  #[argh(positional, default = "\"deps.cirru\".to_owned()")]
  input: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
enum SubCommand {
  /// show outdated versions
  Outdated(OutdatedCaps),
  Upgrade(UpgradeCaps),
  Download(DownloadCaps),
  Add(AddCaps),
  Remove(RemoveCaps),
  /// show the resolved recursive dependency graph
  Tree(TreeCaps),
  /// explain why a module is present
  Why(WhyCaps),
  /// read or update the package version in deps.cirru
  Version(VersionCaps),
  /// check installed modules for local modifications and version mismatches
  Status(StatusCaps),
  /// verify store contents, project links, and native receipts
  Verify(VerifyCaps),
  /// rebuild the project module links from resolved immutable store entries
  Reset(ResetCaps),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// check installed module versions and local modifications
#[argh(subcommand, name = "status")]
struct StatusCaps {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// verify store contents, project links, and native receipts
#[argh(subcommand, name = "verify")]
struct VerifyCaps {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// rebuild the project module links from resolved immutable store entries
#[argh(subcommand, name = "reset")]
struct ResetCaps {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// show the resolved recursive dependency graph
#[argh(subcommand, name = "tree")]
struct TreeCaps {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// explain why a module is present in the resolved graph
#[argh(subcommand, name = "why")]
struct WhyCaps {
  /// package in owner/repo form
  #[argh(positional)]
  package: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// read or update the package version in deps.cirru
#[argh(subcommand, name = "version")]
struct VersionCaps {
  #[argh(subcommand)]
  command: VersionSubcommand,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
enum VersionSubcommand {
  Get(VersionGetCaps),
  Set(VersionSetCaps),
  Bump(VersionBumpCaps),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// print the package version
#[argh(subcommand, name = "get")]
struct VersionGetCaps {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// set the package version
#[argh(subcommand, name = "set")]
struct VersionSetCaps {
  /// exact SemVer version
  #[argh(positional)]
  version: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// bump the package version
#[argh(subcommand, name = "bump")]
struct VersionBumpCaps {
  /// one of major, minor, or patch
  #[argh(positional)]
  level: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// upgrade dependencies
#[argh(subcommand, name = "upgrade")]
struct UpgradeCaps {
  /// packages to upgrade
  #[argh(positional)]
  packages: Vec<String>,
  /// upgrade all dependencies
  #[argh(switch)]
  all: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// show outdated versions
#[argh(subcommand, name = "outdated")]
struct OutdatedCaps {
  /// update deps.cirru directly without interactive confirmation
  #[argh(switch, short = 'y', long = "yes")]
  yes: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// download named packages with org/repo@branch
#[argh(subcommand, name = "download")]
struct DownloadCaps {
  /// packages to download, in format of `org/repo@branch`
  #[argh(positional)]
  packages: Vec<String>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// add dependencies to deps.cirru then run default download flow
#[argh(subcommand, name = "add")]
struct AddCaps {
  /// packages in format `org/repo` or github URL
  #[argh(positional)]
  packages: Vec<String>,
  /// version/branch written to deps.cirru
  #[argh(option, short = 'r', default = "\"main\".to_string()")]
  version: String,
  /// add packages to dev-dependencies
  #[argh(switch)]
  dev: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// remove dependencies from deps.cirru then run default download flow
#[argh(subcommand, name = "remove")]
struct RemoveCaps {
  /// packages in format `org/repo` or github URL
  #[argh(positional)]
  packages: Vec<String>,
  /// remove packages from dev-dependencies
  #[argh(switch)]
  dev: bool,
}

fn err_println(msg: String) {
  if msg.chars().nth(1) == Some(' ') {
    println!("{}", msg.truecolor(255, 80, 80));
  } else {
    println!("  {}", msg.replace('\n', "\n  ").truecolor(255, 80, 80));
  }
}

fn indent4(msg: &str) -> String {
  let ret = msg
    .trim()
    .lines()
    .map(|line| format!("    {line}"))
    .collect::<Vec<String>>()
    .join("\n");
  format!("\n{ret}\n")
}

fn normalize_package_name(raw: &str) -> Result<String, String> {
  let mut s = raw.trim().to_string();

  if let Some(rest) = s.strip_prefix("https://github.com/") {
    s = rest.to_string();
  } else if let Some(rest) = s.strip_prefix("http://github.com/") {
    s = rest.to_string();
  } else if let Some(rest) = s.strip_prefix("git@github.com:") {
    s = rest.to_string();
  }

  if s.ends_with(".git") {
    s.truncate(s.len() - 4);
  }
  s = s.trim_end_matches('/').to_string();

  let (org, repo) =
    validated_module_parts(&s).map_err(|_| format!("invalid package '{raw}', expected canonical org/repo or github URL"))?;
  Ok(format!("{org}/{repo}"))
}

fn module_folder(name: &str) -> Result<&str, String> {
  validated_module_parts(name).map(|(_, repo)| repo)
}

fn validated_module_parts(name: &str) -> Result<(&str, &str), String> {
  let mut segments = name.split('/');
  let org = segments.next().ok_or_else(|| format!("invalid module name '{name}'"))?;
  let repo = segments.next().ok_or_else(|| format!("invalid module name '{name}'"))?;
  if segments.next().is_some() || !valid_module_component(org) || !valid_module_component(repo) {
    return Err(format!("invalid module name '{name}', expected canonical org/repo"));
  }
  Ok((org, repo))
}

fn valid_module_component(component: &str) -> bool {
  !component.is_empty()
    && component != "."
    && component != ".."
    && component
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
  use super::{
    PackageDeps, VersionBumpCaps, VersionCaps, VersionGetCaps, VersionSubcommand, handle_version_command, module_folder,
    normalize_package_name,
  };
  use cirru_edn::Edn;
  use std::collections::HashMap;
  use std::sync::Arc;

  #[test]
  fn module_names_are_canonical_before_path_use() {
    assert_eq!(module_folder("calcit-lang/respo.calcit"), Ok("respo.calcit"));
    for invalid in ["../outside", "org/../outside", "org//repo", "org/repo/extra", "/tmp/repo"] {
      assert!(module_folder(invalid).is_err(), "{invalid} should be rejected");
    }
  }

  #[test]
  fn normalized_package_names_reject_extra_path_segments() {
    assert_eq!(
      normalize_package_name("https://github.com/calcit-lang/respo.calcit.git"),
      Ok("calcit-lang/respo.calcit".to_owned())
    );
    assert!(normalize_package_name("calcit-lang/respo.calcit/extra").is_err());
  }

  #[test]
  fn missing_dependencies_field_is_an_empty_graph() {
    let deps: PackageDeps = Edn::Map(cirru_edn::EdnMapView::default()).try_into().unwrap();
    assert!(deps.version.is_none());
    assert!(deps.dependencies.is_empty());
    assert!(deps.dev_dependencies.is_empty());
  }

  #[test]
  fn version_commands_do_not_read_snapshot_version() {
    let root = std::env::temp_dir().join(format!("calcit-caps-version-migration-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let deps_path = root.join("deps.cirru");
    std::fs::write(&deps_path, "{} (:dependencies $ {})").unwrap();
    // A legacy snapshot version must not be used as a fallback anymore.
    std::fs::write(root.join("calcit.cirru"), "{} (:version |1.2.3)").unwrap();

    let deps: PackageDeps = cirru_edn::parse("{} (:dependencies $ {})").unwrap().try_into().unwrap();
    let deps_file = deps_path.to_str().unwrap();
    let get_error = handle_version_command(
      deps.clone(),
      deps_file,
      &VersionCaps {
        command: VersionSubcommand::Get(VersionGetCaps {}),
      },
    )
    .expect_err("missing deps.cirru version should reject version get");
    assert!(get_error.contains("no :version declared in"), "unexpected error: {get_error}");
    assert!(get_error.contains("deps.cirru"), "unexpected error: {get_error}");

    let bump_error = handle_version_command(
      deps,
      deps_file,
      &VersionCaps {
        command: VersionSubcommand::Bump(VersionBumpCaps { level: "patch".to_owned() }),
      },
    )
    .expect_err("missing deps.cirru version should reject version bump");
    assert!(bump_error.contains("no :version declared in"), "unexpected error: {bump_error}");
    assert!(bump_error.contains("deps.cirru"), "unexpected error: {bump_error}");

    std::fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn parses_development_dependencies_separately() {
    let parsed = cirru_edn::parse("{} (:dependencies $ {} (|org/runtime |1.0.0)) (:dev-dependencies $ {} (|org/test |main))").unwrap();
    let deps: PackageDeps = parsed.try_into().unwrap();
    assert_eq!(deps.dependencies.get("org/runtime").map(AsRef::as_ref), Some("1.0.0"));
    assert_eq!(deps.dev_dependencies.get("org/test").map(AsRef::as_ref), Some("main"));
  }

  #[test]
  fn write_deps_preserves_package_version() {
    let path = std::env::temp_dir().join(format!("calcit-caps-version-{}.cirru", std::process::id()));
    let deps = PackageDeps {
      version: Some("1.2.3".to_string()),
      calcit_version: Some("0.13.12".to_string()),
      dependencies: Default::default(),
      dev_dependencies: HashMap::from([(Arc::from("calcit-lang/test"), Arc::from("main"))]),
    };
    super::write_deps_file(path.to_str().unwrap(), &deps).unwrap();
    let parsed = cirru_edn::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let restored: PackageDeps = parsed.try_into().unwrap();
    assert_eq!(restored.version.as_deref(), Some("1.2.3"));
    assert_eq!(restored.dev_dependencies.get("calcit-lang/test").map(AsRef::as_ref), Some("main"));
    std::fs::remove_file(path).unwrap();
  }
}

fn write_deps_file(deps_file: &str, deps: &PackageDeps) -> Result<(), String> {
  let mut updated_edn = Edn::Map(cirru_edn::EdnMapView::default());

  if let Edn::Map(ref mut map) = updated_edn {
    if let Some(ref version) = deps.version {
      map.insert(Edn::tag("version"), Edn::str(version.as_str()));
    }
    if let Some(ref version) = deps.calcit_version {
      map.insert(Edn::tag("calcit-version"), Edn::str(version.as_str()));
    }

    let mut deps_map = cirru_edn::EdnMapView::default();
    for (k, v) in &deps.dependencies {
      deps_map.insert(Edn::str(&**k), Edn::str(&**v));
    }
    map.insert(Edn::tag("dependencies"), Edn::Map(deps_map));

    if !deps.dev_dependencies.is_empty() {
      let mut dev_deps_map = cirru_edn::EdnMapView::default();
      for (k, v) in &deps.dev_dependencies {
        dev_deps_map.insert(Edn::str(&**k), Edn::str(&**v));
      }
      map.insert(Edn::tag("dev-dependencies"), Edn::Map(dev_deps_map));
    }
  }

  let updated_content = cirru_edn::format(&updated_edn, false)?;
  fs::write(deps_file, updated_content).map_err(|e| e.to_string())?;
  Ok(())
}

/// calcit dynamic libs uses a `build.sh` script to build Rust `.so` files
fn call_build_script(folder_path: &Path) -> Result<String, String> {
  let output = std::process::Command::new("sh")
    .arg("build.sh")
    .current_dir(folder_path)
    .output()
    .map_err(|e| e.to_string())?;
  if output.status.success() {
    let msg = std::str::from_utf8(&output.stdout).unwrap_or("");
    Ok(indent4(msg))
  } else {
    let msg = std::str::from_utf8(&output.stderr).unwrap_or("");
    err_println(indent4(msg));
    Err(format!("failed to build module {}", folder_path.display()))
  }
}

/// read packages from deps, find tag(or sha) and committed date,
/// also git fetch to read latest tag from remote,
/// then we can compare, get outdated version printed
/// Returns true if deps.cirru was updated
fn outdated_tags(deps: PackageDeps, deps_file: &str, auto_yes: bool) -> Result<bool, String> {
  print_column("package".dimmed(), "expected".dimmed(), "latest".dimmed(), "hint".dimmed());
  println!();

  let mut outdated_packages = Vec::new();
  let mut children = vec![];

  for (org_and_folder, version) in deps.root_dependencies()? {
    let org_and_folder_clone = org_and_folder.clone();
    let version_clone = version.clone();
    let ret = thread::spawn(move || {
      let ret = show_package_versions(org_and_folder_clone, version_clone);
      if let Err(e) = ret {
        err_println(format!("{e}\n"));
        return None;
      }
      ret.ok()
    });
    children.push((org_and_folder, version, ret));
  }

  for (org_and_folder, version, child) in children {
    if let Ok(Some(Some(latest_tag))) = child.join()
      && latest_tag != *version
    {
      outdated_packages.push((org_and_folder.to_owned(), version.to_owned(), latest_tag));
    }
  }

  // Check if calcit-version needs to be added (missing) or upgraded (outdated).
  // old_calcit_version is None when the field is absent from deps.cirru.
  let old_calcit_version = deps.calcit_version.as_deref();
  let calcit_version_needs_update = match old_calcit_version {
    None => true,
    Some(version) => match (Version::parse(version).ok(), Version::parse(CALCIT_VERSION).ok()) {
      (Some(expected), Some(current)) => expected < current,
      _ => false,
    },
  };

  if !outdated_packages.is_empty() || calcit_version_needs_update {
    if auto_yes {
      update_deps_file(&outdated_packages, calcit_version_needs_update, deps_file)?;
      println!("deps.cirru updated successfully!");
      return Ok(true);
    }

    println!();
    let mut changes = Vec::new();
    if !outdated_packages.is_empty() {
      changes.push(format!("{} outdated package(s)", outdated_packages.len()));
    }
    if calcit_version_needs_update {
      match old_calcit_version {
        None => changes.push(format!("calcit-version missing, will set to {CALCIT_VERSION}")),
        Some(v) => changes.push(format!("calcit-version {v} -> {CALCIT_VERSION}")),
      }
    }
    print!("Found {}. Update deps.cirru? (y/N): ", changes.join(", "));
    std::io::stdout().flush().map_err(|e| e.to_string())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).map_err(|e| e.to_string())?;
    let input = input.trim();

    if input.is_empty() || input.to_lowercase() == "y" || input.to_lowercase() == "yes" {
      update_deps_file(&outdated_packages, calcit_version_needs_update, deps_file)?;
      println!("deps.cirru updated successfully!");
      return Ok(true);
    }
  }

  Ok(false)
}

fn show_package_versions(org_and_folder: Arc<str>, version: Arc<str>) -> Result<Option<String>, String> {
  let https_url = format!("https://github.com/{org_and_folder}.git");
  let output = match list_remote_tags(&https_url) {
    Ok(output) => output,
    Err(https_error) => {
      let ssh_url = format!("git@github.com:{org_and_folder}.git");
      list_remote_tags(&ssh_url).map_err(|ssh_error| {
        format!("failed to inspect tags for {org_and_folder} over HTTPS and SSH:\n  HTTPS: {https_error}\n  SSH: {ssh_error}")
      })?
    }
  };
  let latest = String::from_utf8_lossy(&output.stdout)
    .lines()
    .filter_map(|line| line.split_once("refs/tags/").map(|(_, tag)| tag))
    .filter_map(|tag| {
      Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .ok()
        .map(|parsed| (tag.to_string(), parsed))
    })
    .max_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
  if let Some((latest_tag, latest_version)) = latest {
    let current = Version::parse(version.strip_prefix('v').unwrap_or(&version));
    if current.as_ref().is_ok_and(|current| current < &latest_version) {
      print_column(org_and_folder.yellow(), version.yellow(), latest_tag.yellow(), "Outdated".yellow());
      Ok(Some(latest_tag))
    } else {
      print_column(org_and_folder.dimmed(), version.dimmed(), latest_tag.dimmed(), "√".dimmed());
      Ok(None)
    }
  } else {
    print_column(org_and_folder.yellow(), version.yellow(), "no SemVer tags".yellow(), "-".yellow());
    Ok(None)
  }
}

fn list_remote_tags(url: &str) -> Result<std::process::Output, String> {
  let output = Command::new("git")
    .env("GIT_TERMINAL_PROMPT", "0")
    .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes")
    .args(["ls-remote", "--tags", "--refs", url])
    .output()
    .map_err(|e| format!("failed to inspect {url}: {e}"))?;
  if output.status.success() {
    Ok(output)
  } else {
    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
  }
}

fn update_deps_file(
  outdated_packages: &[(Arc<str>, Arc<str>, String)],
  update_calcit_version: bool,
  deps_file: &str,
) -> Result<(), String> {
  if !Path::new(deps_file).exists() {
    return Err("deps.cirru file not found".to_string());
  }

  let content = fs::read_to_string(deps_file).map_err(|e| e.to_string())?;
  let parsed = cirru_edn::parse(&content).map_err(|e| {
    eprintln!("\nFailed to parse '{deps_file}':");
    eprintln!("{e}");
    format!("Failed to parse '{deps_file}'")
  })?;
  let mut deps: PackageDeps = parsed.try_into()?;

  if update_calcit_version {
    deps.calcit_version = Some(CALCIT_VERSION.to_string());
  }

  // Update the dependencies in the parsed structure
  for (org_and_folder, _old_version, new_version) in outdated_packages {
    if let Some(version) = deps.dependencies.get_mut(org_and_folder) {
      *version = new_version.clone().into();
    }
    if let Some(version) = deps.dev_dependencies.get_mut(org_and_folder) {
      *version = new_version.clone().into();
    }
  }

  write_deps_file(deps_file, &deps)
}

fn upgrade_packages(deps: PackageDeps, deps_file: &str, opts: &UpgradeCaps) -> Result<bool, String> {
  let mut outdated_packages = Vec::new();
  let mut children = vec![];

  let targets: Vec<Arc<str>> = if opts.all {
    deps.root_dependencies()?.keys().cloned().collect()
  } else {
    opts
      .packages
      .iter()
      .map(|p| normalize_package_name(p).map(|s| s.into()))
      .collect::<Result<Vec<Arc<str>>, String>>()?
  };

  if targets.is_empty() && !opts.all {
    return Err("no packages to upgrade".to_string());
  }

  for org_and_folder in &targets {
    if let Some(version) = deps
      .dependencies
      .get(org_and_folder)
      .or_else(|| deps.dev_dependencies.get(org_and_folder))
    {
      let org_and_folder_clone = org_and_folder.clone();
      let version_clone = version.clone();
      let ret = thread::spawn(move || {
        let ret = show_package_versions(org_and_folder_clone, version_clone);
        if let Err(e) = ret {
          err_println(format!("{e}\n"));
          return None;
        }
        ret.ok()
      });
      children.push((org_and_folder.clone(), version.clone(), ret));
    } else {
      return Err(format!("package {org_and_folder} not found in deps.cirru"));
    }
  }

  for (org_and_folder, version, child) in children {
    if let Ok(Some(Some(latest_tag))) = child.join()
      && latest_tag != *version
    {
      outdated_packages.push((org_and_folder.to_owned(), version.to_owned(), latest_tag));
    }
  }

  let calcit_version_needs_update = if opts.all {
    let old_calcit_version = deps.calcit_version.as_deref();
    match old_calcit_version {
      None => true,
      Some(version) => match (Version::parse(version).ok(), Version::parse(CALCIT_VERSION).ok()) {
        (Some(expected), Some(current)) => expected < current,
        _ => false,
      },
    }
  } else {
    false
  };

  if !outdated_packages.is_empty() || calcit_version_needs_update {
    update_deps_file(&outdated_packages, calcit_version_needs_update, deps_file)?;
    if opts.all {
      sync_calcit_procs_package()?;
    }
    println!("deps.cirru updated successfully!");
    Ok(true)
  } else {
    println!("Already up to date.");
    Ok(false)
  }
}

fn sync_calcit_procs_package() -> Result<(), String> {
  if !Path::new("package.json").exists() {
    println!("skipping {} sync: no package.json found", "@calcit/procs".yellow());
    return Ok(());
  }

  println!("syncing npm package {}...", "@calcit/procs".green());
  let status = Command::new("yarn")
    .args(["up", "@calcit/procs"])
    .status()
    .map_err(|e| format!("failed to run `yarn up @calcit/procs`: {e}"))?;

  if status.success() {
    Ok(())
  } else {
    Err(format!(
      "`yarn up @calcit/procs` exited with status {}",
      status.code().map(|code| code.to_string()).unwrap_or_else(|| "signal".to_string())
    ))
  }
}

fn handle_version_command(mut deps: PackageDeps, deps_file: &str, opts: &VersionCaps) -> Result<(), String> {
  match &opts.command {
    VersionSubcommand::Get(_) => {
      let version = deps
        .version
        .as_deref()
        .ok_or_else(|| format!("no :version declared in {deps_file}; initialize it with `caps {deps_file} version set <version>`"))?;
      println!("{version}");
    }
    VersionSubcommand::Set(opts) => {
      Version::parse(&opts.version).map_err(|e| format!("invalid SemVer version '{}': {e}", opts.version))?;
      deps.version = Some(opts.version.clone());
      write_package_version(deps_file, &opts.version)?;
      println!("updated {deps_file} to {}", opts.version);
    }
    VersionSubcommand::Bump(opts) => {
      let current = deps
        .version
        .as_deref()
        .ok_or_else(|| format!("no :version declared in {deps_file}; initialize it with `caps {deps_file} version set <version>`"))?;
      let mut version = Version::parse(current).map_err(|e| format!("invalid existing SemVer version '{current}': {e}"))?;
      match opts.level.as_str() {
        "major" => {
          version.major += 1;
          version.minor = 0;
          version.patch = 0;
        }
        "minor" => {
          version.minor += 1;
          version.patch = 0;
        }
        "patch" => version.patch += 1,
        other => return Err(format!("invalid bump level '{other}', expected major, minor, or patch")),
      }
      version.pre = semver::Prerelease::EMPTY;
      version.build = semver::BuildMetadata::EMPTY;
      deps.version = Some(version.to_string());
      write_package_version(deps_file, &version.to_string())?;
      println!("updated {deps_file} to {version}");
    }
  }
  Ok(())
}

fn write_package_version(deps_file: &str, version: &str) -> Result<(), String> {
  let content = fs::read_to_string(deps_file).map_err(|e| e.to_string())?;
  let mut parsed = cirru_edn::parse(&content).map_err(|e| format!("failed to parse {deps_file}: {e}"))?;
  let Edn::Map(ref mut map) = parsed else {
    return Err(format!("expected map in {deps_file}"));
  };
  map.insert(Edn::tag("version"), Edn::str(version));
  fs::write(deps_file, cirru_edn::format(&parsed, false)?).map_err(|e| e.to_string())
}

fn print_column(pkg: ColoredString, expected: ColoredString, latest: ColoredString, hint: ColoredString) {
  println!("{pkg:<32} {expected:<12} {latest:<12} {hint:<12}");
}
