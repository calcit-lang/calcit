//! CLI tool to download packages from github,
//! packages are defined in `deps.cirru` file
//!
//! files are stored in `~/.config/calcit/modules/`.

mod git;

use argh::{self, FromArgs};

use cirru_edn::Edn;
use colored::*;
use git::*;
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
  calcit_version: Option<String>,
  dependencies: HashMap<Arc<str>, Arc<str>>,
}

impl TryFrom<Edn> for PackageDeps {
  type Error = String;

  fn try_from(value: Edn) -> Result<Self, Self::Error> {
    let deps_info = value.view_map()?;
    #[allow(clippy::mutable_key_type)]
    let dict = deps_info.get_or_nil("dependencies").view_map()?.0;

    let mut deps: HashMap<Arc<str>, Arc<str>> = HashMap::new();
    for (k, v) in &dict {
      match (k, v) {
        (Edn::Str(k), Edn::Str(v)) => {
          deps.insert(k.to_owned(), v.to_owned());
        }
        _ => {
          return Err(format!("invalid dependency: {k} {v}"));
        }
      }
    }
    let expected_version: Option<String> = match deps_info.get_or_nil("calcit-version") {
      Edn::Str(s) => Some((*s).to_owned()),
      Edn::Nil => None,
      v => return Err(format!("invalid calcit-version: {v}")),
    };
    Ok(PackageDeps {
      calcit_version: expected_version,
      dependencies: deps,
    })
  }
}

pub fn main() -> Result<(), String> {
  // parse deps.cirru

  let cli_args: TopLevelCaps = argh::from_env();
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
    download_deps(dict, cli_args)?;
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
          download_deps(updated_deps.dependencies, cli_args)?;
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
          download_deps(updated_deps.dependencies, cli_args)?;
        }
      }
      Some(SubCommand::Add(opts)) => {
        if opts.packages.is_empty() {
          return Err("no packages to add".to_string());
        }

        let mut updated_deps = deps;
        for raw in &opts.packages {
          let org_and_folder = normalize_package_name(raw)?;
          updated_deps
            .dependencies
            .insert(org_and_folder.into(), opts.version.to_owned().into());
        }

        write_deps_file(&cli_args.input, &updated_deps)?;
        println!("updated {}", cli_args.input.green());
        download_deps(updated_deps.dependencies, cli_args)?;
      }
      Some(SubCommand::Remove(opts)) => {
        if opts.packages.is_empty() {
          return Err("no packages to remove".to_string());
        }

        let mut updated_deps = deps;
        for raw in &opts.packages {
          let org_and_folder = normalize_package_name(raw)?;
          updated_deps.dependencies.remove(org_and_folder.as_str());
        }

        write_deps_file(&cli_args.input, &updated_deps)?;
        println!("updated {}", cli_args.input.green());
        download_deps(updated_deps.dependencies, cli_args)?;
      }
      Some(SubCommand::Status(_)) => {
        let issues = check_dependency_status(&deps, &cli_args, false)?;
        if issues > 0 {
          return Err(format!("{issues} module(s) are not in the expected state"));
        }
      }
      Some(SubCommand::Reset(_)) => {
        reset_dependency_status(&deps, &cli_args)?;
      }
      Some(SubCommand::Download(dep_names)) => {
        unreachable!("already handled: {:?}", dep_names);
      }
      None => {
        check_dependency_status(&deps, &cli_args, true)?;
        download_deps(deps.dependencies, cli_args)?;
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

fn download_deps(deps: HashMap<Arc<str>, Arc<str>>, options: TopLevelCaps) -> Result<(), String> {
  // ~/.config/calcit/modules/
  let clone_target = if options.local_debug {
    println!("{}", "  [DEBUG] local debug mode, cloning to test-modules/".yellow());
    ".config/calcit/test-modules"
  } else {
    ".config/calcit/modules"
  };
  let modules_dir = dirs::home_dir().ok_or("no config dir")?.join(clone_target);

  if !modules_dir.exists() {
    fs::create_dir_all(&modules_dir).map_err(|e| e.to_string())?;
    dim_println(format!("created dir: {modules_dir:?}"));
  }

  let mut children = vec![];

  for (org_and_folder, version) in deps {
    // cloned

    let org_and_folder = org_and_folder.clone();
    let options = options.to_owned();
    let modules_dir = modules_dir.clone();

    // TODO too many threads do not make it faster though
    let options2 = options.clone();
    let ret = thread::spawn(move || {
      let ret = handle_path(modules_dir, version, &options2, org_and_folder);
      if let Err(e) = ret {
        err_println(format!("{e}\n"));
      }
    });
    children.push(ret);
  }
  for child in children {
    child.join().unwrap();
  }

  Ok(())
}

fn modules_dir(options: &TopLevelCaps) -> Result<PathBuf, String> {
  let dir = if options.local_debug {
    ".config/calcit/test-modules"
  } else {
    ".config/calcit/modules"
  };
  Ok(dirs::home_dir().ok_or("no config dir")?.join(dir))
}

fn check_dependency_status(deps: &PackageDeps, options: &TopLevelCaps, warn_only: bool) -> Result<usize, String> {
  let modules_dir = modules_dir(options)?;
  let mut issues = 0;
  for (org_and_folder, version) in &deps.dependencies {
    let folder = org_and_folder.split_once('/').ok_or("invalid name")?.1;
    let folder_path = modules_dir.join(folder);
    if !folder_path.exists() {
      issues += 1;
      println!("{}", format!("- {org_and_folder} not found (expected {version})").red());
      continue;
    }
    let repo = GitRepo { dir: folder_path.clone() };
    let changes = wrap_module_error(repo.status_porcelain(), org_and_folder, &folder_path, "read working tree status")?;
    let head = wrap_module_error(repo.current_head(), org_and_folder, &folder_path, "read current git head")?;
    if !changes.is_empty() {
      issues += 1;
      let message = format!(
        "! {org_and_folder} has local modifications (expected {version}, at {})",
        head.get_name()
      );
      if warn_only {
        eprintln!("{}", message.yellow());
      } else {
        println!("{}", message.yellow());
      }
    } else if head.get_name() != **version {
      issues += 1;
      println!(
        "{}",
        format!("! {org_and_folder} is at {}, expected {version}", head.get_name()).yellow()
      );
    } else {
      println!("{}", format!("√ {org_and_folder} at {version}").dimmed());
    }
  }
  Ok(issues)
}

fn reset_dependency_status(deps: &PackageDeps, options: &TopLevelCaps) -> Result<(), String> {
  let modules_dir = modules_dir(options)?;
  for org_and_folder in deps.dependencies.keys() {
    let folder = org_and_folder.split_once('/').ok_or("invalid name")?.1;
    let folder_path = modules_dir.join(folder);
    if !folder_path.exists() {
      continue;
    }
    let repo = GitRepo { dir: folder_path.clone() };
    let changes = wrap_module_error(repo.status_porcelain(), org_and_folder, &folder_path, "read working tree status")?;
    if changes.is_empty() {
      continue;
    }
    wrap_module_error(repo.reset_hard(), org_and_folder, &folder_path, "reset local changes")?;
    println!("reset {}", org_and_folder.green());
  }
  Ok(())
}

fn wrap_module_error<T>(result: Result<T, String>, org_and_folder: &str, folder_path: &Path, action: &str) -> Result<T, String> {
  result.map_err(|e| format!("failed to {action} module `{org_and_folder}` at `{}`\n{e}", folder_path.display()))
}

fn handle_path(modules_dir: PathBuf, version: Arc<str>, options: &TopLevelCaps, org_and_folder: Arc<str>) -> Result<(), String> {
  // check if exists
  let (_org, folder) = org_and_folder.split_once('/').ok_or("invalid name")?;
  // split with / into (org,folder)

  let folder_path = modules_dir.join(folder);
  let build_file = folder_path.join("build.sh");
  let git_repo = GitRepo { dir: folder_path.clone() };
  if folder_path.exists() {
    // println!("module {} exists", folder);
    // check branch
    let current_head = wrap_module_error(git_repo.current_head(), &org_and_folder, &folder_path, "read current git head")?;

    if current_head.get_name() == *version {
      dim_println(format!("√ found {} of {}", gray(&version), gray(folder)));
      if let GitHead::Branch(branch) = current_head
        && options.pull_branch
      {
        dim_println(format!("↺ pulling {} at version {}", gray(&org_and_folder), gray(&version)));
        wrap_module_error(git_repo.pull(&branch), &org_and_folder, &folder_path, "pull branch")?;
        dim_println(format!("pulled {} at {}", gray(folder), gray(&version)));

        // if there's a build.sh file in the folder, run it
        if build_file.exists() {
          let build_msg = wrap_module_error(call_build_script(&folder_path), &org_and_folder, &folder_path, "run build.sh")?;
          dim_println(format!("ran build script for {}", gray(&org_and_folder)));
          dim_println(build_msg);
        }
      }
      return Ok(());
    }
    // let msg = format!("module {} is at version {:?}, but required {}", folder, current_head, version);
    // println!("  {}", msg.yellow());

    // load latest tags
    wrap_module_error(git_repo.fetch(), &org_and_folder, &folder_path, "fetch tags")?;
    // try if tag or branch exists in git history
    let has_target = wrap_module_error(
      git_repo.check_branch_or_tag(&version, folder),
      &org_and_folder,
      &folder_path,
      "check target branch or tag",
    )?;
    if !has_target {
      dim_println(format!("↺ fetching {} at version {}", gray(&org_and_folder), gray(&version)));
      wrap_module_error(git_repo.fetch(), &org_and_folder, &folder_path, "fetch tags")?;
      dim_println(format!("fetched {} at version {}", gray(&org_and_folder), gray(&version)));
      // fetch git repo and checkout target version
    }
    wrap_module_error(
      git_repo.checkout(&version),
      &org_and_folder,
      &folder_path,
      &format!("checkout version `{version}`"),
    )?;
    dim_println(format!("√ checked out {} of {}", gray(&version), gray(&org_and_folder)));

    let current_head = wrap_module_error(git_repo.current_head(), &org_and_folder, &folder_path, "read current git head")?;
    if let GitHead::Branch(branch) = current_head
      && options.pull_branch
    {
      dim_println(format!("↺ pulling {} at version {}", gray(&org_and_folder), gray(&version)));
      wrap_module_error(git_repo.pull(&branch), &org_and_folder, &folder_path, "pull branch")?;
      dim_println(format!("pulled {} at {}", gray(folder), gray(&version)));
    }

    // if there's a build.sh file in the folder, run it
    if build_file.exists() {
      let build_msg = wrap_module_error(call_build_script(&folder_path), &org_and_folder, &folder_path, "run build.sh")?;
      dim_println(format!("ran build script for {}", gray(&org_and_folder)));
      dim_println(build_msg);
    }
  } else {
    let url = if options.ci {
      format!("https://github.com/{org_and_folder}.git")
    } else {
      format!("git@github.com:{org_and_folder}.git")
    };
    dim_println(format!("↺ cloning {} at version {}", gray(&org_and_folder), gray(&version)));
    wrap_module_error(
      GitRepo::clone_to(&modules_dir, &url, &version, options.ci),
      &org_and_folder,
      &folder_path,
      &format!("clone version `{version}`"),
    )?;
    // println!("downloading {} at version {}", url, version);
    dim_println(format!("downloaded {} at version {}", gray(&org_and_folder), gray(&version)));

    if !options.ci {
      // if there's a build.sh file in the folder, run it
      if build_file.exists() {
        let build_msg = wrap_module_error(call_build_script(&folder_path), &org_and_folder, &folder_path, "run build.sh")?;
        dim_println(format!("ran build script for {}", gray(&org_and_folder)));
        dim_println(build_msg);
      }
    }
  }
  Ok(())
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

  /// pull branch in the repo
  #[argh(switch)]
  pull_branch: bool,
  /// CI mode loads shallow repo via HTTPS
  #[argh(switch)]
  ci: bool,
  /// debug mode, clone to test-modules/
  #[argh(switch)]
  local_debug: bool,

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
  /// check installed modules for local modifications and version mismatches
  Status(StatusCaps),
  /// discard tracked local modifications in installed modules
  Reset(ResetCaps),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// check installed module versions and local modifications
#[argh(subcommand, name = "status")]
struct StatusCaps {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// discard tracked local modifications in installed modules
#[argh(subcommand, name = "reset")]
struct ResetCaps {}

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
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// remove dependencies from deps.cirru then run default download flow
#[argh(subcommand, name = "remove")]
struct RemoveCaps {
  /// packages in format `org/repo` or github URL
  #[argh(positional)]
  packages: Vec<String>,
}

fn dim_println(msg: String) {
  if msg.chars().nth(1) == Some(' ') {
    println!("{}", msg.truecolor(128, 128, 128));
  } else {
    println!("  {}", msg.truecolor(128, 128, 128));
  }
}

fn err_println(msg: String) {
  if msg.chars().nth(1) == Some(' ') {
    println!("{}", msg.truecolor(255, 80, 80));
  } else {
    println!("  {}", msg.replace('\n', "\n  ").truecolor(255, 80, 80));
  }
}

fn gray(msg: &str) -> ColoredString {
  msg.truecolor(172, 172, 172)
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

  let segments: Vec<&str> = s.split('/').filter(|x| !x.is_empty()).collect();
  if segments.len() < 2 {
    return Err(format!("invalid package '{raw}', expected org/repo or github URL"));
  }

  Ok(format!("{}/{}", segments[0], segments[1]))
}

fn write_deps_file(deps_file: &str, deps: &PackageDeps) -> Result<(), String> {
  let mut updated_edn = Edn::Map(cirru_edn::EdnMapView::default());

  if let Edn::Map(ref mut map) = updated_edn {
    if let Some(ref version) = deps.calcit_version {
      map.insert(Edn::tag("calcit-version"), Edn::str(version.as_str()));
    }

    let mut deps_map = cirru_edn::EdnMapView::default();
    for (k, v) in &deps.dependencies {
      deps_map.insert(Edn::str(&**k), Edn::str(&**v));
    }
    map.insert(Edn::tag("dependencies"), Edn::Map(deps_map));
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

  for (org_and_folder, version) in &deps.dependencies {
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
  let (_org, folder) = org_and_folder.split_once('/').ok_or("invalid name")?;
  let folder_path = dirs::home_dir().ok_or("no config dir")?.join(".config/calcit/modules").join(folder);
  let git_repo = GitRepo { dir: folder_path.clone() };
  if folder_path.exists() {
    git_repo.fetch()?;

    // get latest tag and timestamp
    let latest_tag = git_repo.latest_tag()?;
    let latest_timestamp = git_repo.timestamp(&latest_tag)?;

    // get expected tag and timestamp
    let expected_timestamp = git_repo.timestamp(&version)?;

    let outdated = expected_timestamp < latest_timestamp;

    if outdated {
      print_column(org_and_folder.yellow(), version.yellow(), latest_tag.yellow(), "Outdated".yellow());
      Ok(Some(latest_tag))
    } else {
      print_column(org_and_folder.dimmed(), version.dimmed(), latest_tag.dimmed(), "√".dimmed());
      Ok(None)
    }
  } else {
    print_column(org_and_folder.red(), version.red(), "not found".red(), "-".red());
    Ok(None)
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
    deps.dependencies.insert(org_and_folder.clone(), new_version.clone().into());
  }

  write_deps_file(deps_file, &deps)
}

fn upgrade_packages(deps: PackageDeps, deps_file: &str, opts: &UpgradeCaps) -> Result<bool, String> {
  let mut outdated_packages = Vec::new();
  let mut children = vec![];

  let targets: Vec<Arc<str>> = if opts.all {
    deps.dependencies.keys().cloned().collect()
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
    if let Some(version) = deps.dependencies.get(org_and_folder) {
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

fn print_column(pkg: ColoredString, expected: ColoredString, latest: ColoredString, hint: ColoredString) {
  println!("{pkg:<32} {expected:<12} {latest:<12} {hint:<12}");
}
