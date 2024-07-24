//! CLI tool to download packages from github,
//! packages are defined in `deps.cirru` file
//!
//! files are stored in `~/.config/calcit/modules/`.

mod git;

use argh::{self, FromArgs};

use cirru_edn::Edn;
use colored::*;
use git::*;
use std::{
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
  sync::Arc,
  thread,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageDeps {
  dependencies: HashMap<Arc<str>, Arc<str>>,
}

impl TryFrom<Edn> for PackageDeps {
  type Error = String;

  fn try_from(value: Edn) -> Result<Self, Self::Error> {
    let deps = value.view_map()?.get_or_nil("dependencies");
    let dict = deps.view_map()?.0;

    let mut deps: HashMap<Arc<str>, Arc<str>> = HashMap::new();
    for (k, v) in &dict {
      match (k, v) {
        (Edn::Str(k), Edn::Str(v)) => {
          deps.insert(k.to_owned(), v.to_owned());
        }
        _ => {
          return Err(format!("invalid dependency: {} {}", k, v));
        }
      }
    }
    Ok(PackageDeps { dependencies: deps })
  }
}

pub fn main() -> Result<(), String> {
  // parse deps.cirru

  let cli_args: TopLevelCaps = argh::from_env();

  // if file exists

  if Path::new(&cli_args.input).exists() {
    let content = fs::read_to_string(&cli_args.input).map_err(|e| e.to_string())?;
    let parsed = cirru_edn::parse(&content)?;
    let deps: PackageDeps = parsed.try_into()?;

    if cli_args.subcommand.is_some() {
      outdated_tags(deps.dependencies)?;
    } else {
      download_deps(deps.dependencies, cli_args)?;
    }

    Ok(())
  } else if Path::new("package.cirru").exists() {
    // be compatible with old name
    let content = fs::read_to_string("package.cirru").map_err(|e| e.to_string())?;
    let parsed = cirru_edn::parse(&content)?;
    let deps: PackageDeps = parsed.try_into()?;

    if cli_args.subcommand.is_some() {
      outdated_tags(deps.dependencies)?;
    } else {
      download_deps(deps.dependencies, cli_args)?;
    }

    Ok(())
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
    dim_println(format!("created dir: {:?}", modules_dir));
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
        err_println(format!("{}\n", e));
      }
    });
    children.push(ret);
  }
  for child in children {
    child.join().unwrap();
  }

  Ok(())
}

fn handle_path(modules_dir: PathBuf, version: Arc<str>, options: &TopLevelCaps, org_and_folder: Arc<str>) -> Result<(), String> {
  // check if exists
  let (_org, folder) = org_and_folder.split_once('/').ok_or("invalid name")?;
  // split with / into (org,folder)

  let folder_path = modules_dir.join(folder);
  let build_file = folder_path.join("build.sh");
  if folder_path.exists() {
    // println!("module {} exists", folder);
    // check branch
    let current_head = git_current_head(&folder_path)?;
    if current_head.get_name() == *version {
      dim_println(format!("√ found {} of {}", gray(&version), gray(folder)));
      if let GitHead::Branch(branch) = current_head {
        if options.pull_branch {
          dim_println(format!("↺ pulling {} at version {}", gray(&org_and_folder), gray(&version)));
          git_pull(&folder_path, &branch)?;
          dim_println(format!("pulled {} at {}", gray(folder), gray(&version)));

          // if there's a build.sh file in the folder, run it
          if build_file.exists() {
            let build_msg = call_build_script(&folder_path)?;
            dim_println(format!("ran build script for {}", gray(&org_and_folder)));
            dim_println(build_msg);
          }
        }
      }
      return Ok(());
    }
    // let msg = format!("module {} is at version {:?}, but required {}", folder, current_head, version);
    // println!("  {}", msg.yellow());

    // try if tag or branch exists in git history
    let has_target = git_check_branch_or_tag(&folder_path, &version)?;
    if !has_target {
      dim_println(format!("↺ fetching {} at version {}", gray(&org_and_folder), gray(&version)));
      git_fetch(&folder_path)?;
      dim_println(format!("fetched {} at version {}", gray(&org_and_folder), gray(&version)));
      // fetch git repo and checkout target version
    }
    git_checkout(&folder_path, &version)?;
    dim_println(format!("√ checked out {} of {}", gray(&version), gray(&org_and_folder)));

    let current_head = git_current_head(&folder_path)?;
    if let GitHead::Branch(branch) = current_head {
      if options.pull_branch {
        dim_println(format!("↺ pulling {} at version {}", gray(&org_and_folder), gray(&version)));
        git_pull(&folder_path, &branch)?;
        dim_println(format!("pulled {} at {}", gray(folder), gray(&version)));
      }
    }

    // if there's a build.sh file in the folder, run it
    if build_file.exists() {
      let build_msg = call_build_script(&folder_path)?;
      dim_println(format!("ran build script for {}", gray(&org_and_folder)));
      dim_println(build_msg);
    }
  } else {
    let url = if options.ci {
      format!("https://github.com/{}.git", org_and_folder)
    } else {
      format!("git@github.com:{}.git", org_and_folder)
    };
    dim_println(format!("↺ cloning {} at version {}", gray(&org_and_folder), gray(&version)));
    git_clone(&modules_dir, &url, &version, options.ci)?;
    // println!("downloading {} at version {}", url, version);
    dim_println(format!("downloaded {} at version {}", gray(&org_and_folder), gray(&version)));

    if !options.ci {
      // if there's a build.sh file in the folder, run it
      if build_file.exists() {
        let build_msg = call_build_script(&folder_path)?;
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
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// show outdated versions
#[argh(subcommand, name = "outdated")]
struct OutdatedCaps {}

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
    .map(|line| format!("    {}", line))
    .collect::<Vec<String>>()
    .join("\n");
  format!("\n{}\n", ret)
}

/// calcit dynamic libs uses a `build.sh` script to build Rust `.so` files
fn call_build_script(folder_path: &Path) -> Result<String, String> {
  let output = std::process::Command::new("sh")
    .arg("build.sh")
    .current_dir(folder_path)
    .output()
    .map_err(|e| e.to_string())?;
  if output.status.success() {
    let msg = String::from_utf8(output.stdout).unwrap_or("".to_string());
    Ok(indent4(&msg))
  } else {
    let msg = String::from_utf8(output.stderr).unwrap_or("".to_string());
    err_println(indent4(&msg));
    Err(format!("failed to build module {}", folder_path.display()))
  }
}

/// read packages from deps, find tag(or sha) and committed date,
/// also git fetch to read latest tag from remote,
/// then we can compare, get outdated version printed
fn outdated_tags(deps: HashMap<Arc<str>, Arc<str>>) -> Result<(), String> {
  print_column("package".dimmed(), "expected".dimmed(), "latest".dimmed(), "hint".dimmed());
  println!();
  let mut children = vec![];

  for (org_and_folder, version) in deps {
    let ret = thread::spawn(move || {
      let ret = show_package_versions(org_and_folder, version);
      if let Err(e) = ret {
        err_println(format!("{}\n", e));
      }
    });
    children.push(ret);
  }

  for child in children {
    child.join().unwrap();
  }
  Ok(())
}

fn show_package_versions(org_and_folder: Arc<str>, version: Arc<str>) -> Result<(), String> {
  let (_org, folder) = org_and_folder.split_once('/').ok_or("invalid name")?;
  let folder_path = dirs::home_dir().ok_or("no config dir")?.join(".config/calcit/modules").join(folder);
  if folder_path.exists() {
    git_fetch(&folder_path)?;
    // get timestamp of current head
    // let head = git_current_head(&folder_path)?;
    // let head_timestamp = git_timestamp(&folder_path, &head.get_name())?;

    // get latest tag and timestamp
    let latest_tag = git_latest_tag(&folder_path)?;
    let latest_timestamp = git_timestamp(&folder_path, &latest_tag)?;

    // get expected tag and timestamp
    let expected_timestamp = git_timestamp(&folder_path, &version)?;

    let outdated = expected_timestamp < latest_timestamp;

    if outdated {
      print_column(org_and_folder.yellow(), version.yellow(), latest_tag.yellow(), "Outdated".yellow());
    } else {
      print_column(org_and_folder.dimmed(), version.dimmed(), latest_tag.dimmed(), "√".dimmed());
    }
  } else {
    print_column(org_and_folder.red(), version.red(), "not found".red(), "-".red());
  }

  Ok(())
}

fn print_column(pkg: ColoredString, expected: ColoredString, latest: ColoredString, hint: ColoredString) {
  println!("{:<32} {:<12} {:<12} {:<12}", pkg, expected, latest, hint);
}
