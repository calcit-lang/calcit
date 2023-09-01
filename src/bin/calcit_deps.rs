//! CLI tool to download packages from github,
//! packages are defined in `package.cirru` file
//!
//! files are stored in `~/.config/calcit/modules/`.

mod git;

use cirru_edn::Edn;
use colored::*;
use git::*;
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageDeps {
  dependencies: HashMap<String, String>,
}

impl TryFrom<Edn> for PackageDeps {
  type Error = String;

  fn try_from(value: Edn) -> Result<Self, Self::Error> {
    let deps = value.view_map()?.get_or_nil("dependencies");
    let dict = deps.view_map()?.0;

    let mut deps: HashMap<String, String> = HashMap::new();
    for (k, v) in &dict {
      match (k, v) {
        (Edn::Str(k), Edn::Str(v)) => {
          deps.insert(k.to_owned().into_string(), v.to_owned().into());
        }
        _ => {
          return Err(format!("invalid dependency: {} {}", k, v));
        }
      }
    }
    Ok(PackageDeps { dependencies: deps })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliArgs {
  input: String,
  ci: bool,
  verbose: bool,
  local_debug: bool,
  pull_branch: bool,
}

pub fn main() -> Result<(), String> {
  // parse package.cirru

  let cli_matches = parse_cli();

  let options: CliArgs = CliArgs {
    input: cli_matches.value_of("input").unwrap_or("package.cirru").to_string(),
    ci: cli_matches.is_present("ci"),
    verbose: cli_matches.is_present("verbose"),
    local_debug: cli_matches.is_present("local_debug"),
    pull_branch: cli_matches.is_present("pull_branch"),
  };

  // if file exists

  if Path::new(&options.input).exists() {
    let content = fs::read_to_string(&options.input).map_err(|e| e.to_string())?;
    let parsed = cirru_edn::parse(&content)?;
    let deps: PackageDeps = parsed.try_into()?;

    download_deps(deps.dependencies, &options)?;

    Ok(())
  } else {
    eprintln!("Error: no package.cirru found!");
    std::process::exit(1);
  }
}

fn download_deps(deps: HashMap<String, String>, options: &CliArgs) -> Result<(), String> {
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

  for (org_and_folder, version) in deps {
    // check if exists
    let (_org, folder) = org_and_folder.split_once('/').ok_or("invalid name")?;
    // split with / into (org,folder)
    let folder_path = modules_dir.join(folder);
    if folder_path.exists() {
      // println!("module {} exists", folder);
      // check branch
      let current_head = git_current_head(&folder_path)?;
      if current_head.get_name() == version {
        dim_println(format!("√ found {} at {}", gray(folder), gray(&version)));
        if let GitHead::Branch(branch) = current_head {
          if options.pull_branch {
            dim_println(format!("↺ pulling {} at version {}", gray(&org_and_folder), gray(&version)));
            git_pull(&folder_path, &branch)?;
            dim_println(format!("pulled {} at {}", gray(folder), gray(&version)));
          }
        }
        continue;
      } else {
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
        dim_println(format!("√ checked out {} at version {}", gray(&org_and_folder), gray(&version)));

        let current_head = git_current_head(&folder_path)?;
        if let GitHead::Branch(branch) = current_head {
          if options.pull_branch {
            dim_println(format!("↺ pulling {} at version {}", gray(&org_and_folder), gray(&version)));
            git_pull(&folder_path, &branch)?;
            dim_println(format!("pulled {} at {}", gray(folder), gray(&version)));
          }
        }

        let build_file = folder_path.join("build.sh");
        // if there's a build.sh file in the folder, run it
        if build_file.exists() {
          call_build_script(&folder_path)?;
          dim_println(format!("ran build script for {}", gray(&org_and_folder)));
        }
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
        let build_file = folder_path.join("build.sh");
        // if there's a build.sh file in the folder, run it
        if build_file.exists() {
          call_build_script(&folder_path)?;
          dim_println(format!("ran build script for {}", gray(&org_and_folder)));
        }
      }
    }
  }

  Ok(())
}

pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn parse_cli() -> clap::ArgMatches {
  clap::Command::new("Calcit Deps")
    .version(CALCIT_VERSION)
    .author("Jon. <jiyinyiyong@gmail.com>")
    .about("Calcit Deps")
    .arg(
      clap::Arg::new("input")
        .help("entry file path")
        .default_value("package.cirru")
        .index(1),
    )
    .arg(
      clap::Arg::new("verbose")
        .help("verbose mode")
        .short('v')
        .long("verbose")
        .takes_value(false),
    )
    .arg(
      clap::Arg::new("pull_branch")
        .help("pull branch in the repo")
        .long("pull-branch")
        .takes_value(false),
    )
    .arg(
      clap::Arg::new("ci")
        .help("CI mode loads shallow repo via HTTPS")
        .long("ci")
        .takes_value(false),
    )
    .arg(
      clap::Arg::new("local_debug")
        .help("Debug mode, clone to test-modules/")
        .long("local-debug")
        .takes_value(false),
    )
    .get_matches()
}

fn dim_println(msg: String) {
  if msg.chars().nth(1) == Some(' ') {
    println!("{}", msg.truecolor(128, 128, 128));
  } else {
    println!("  {}", msg.truecolor(128, 128, 128));
  }
}

fn gray(msg: &str) -> ColoredString {
  msg.truecolor(172, 172, 172)
}

/// calcit dynamic libs uses a `build.sh` script to build Rust `.so` files
fn call_build_script(folder_path: &Path) -> Result<(), String> {
  let output = std::process::Command::new("sh")
    .arg("build.sh")
    .current_dir(folder_path)
    .output()
    .map_err(|e| e.to_string())?;
  if !output.status.success() {
    let msg = String::from_utf8(output.stderr).unwrap_or("".to_string());
    Err(format!("failed to build module {}: {}", folder_path.display(), msg))
  } else {
    Ok(())
  }
}
