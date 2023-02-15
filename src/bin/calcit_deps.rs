//! CLI tool to download packages from github,
//! packages are defined in `package.cirru` file
//!
//! files are stored in `~/.config/calcit/modules/`.

mod git;

use colored::*;
use git::*;
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageDeps {
  dependencies: HashMap<String, String>,
}

impl TryFrom<cirru_edn::Edn> for PackageDeps {
  type Error = String;

  fn try_from(value: cirru_edn::Edn) -> Result<Self, Self::Error> {
    let deps = value.map_get("dependencies")?;
    if let cirru_edn::Edn::Map(dict) = deps {
      let mut deps: HashMap<String, String> = HashMap::new();
      for (k, v) in dict {
        if let cirru_edn::Edn::Str(k) = k {
          if let cirru_edn::Edn::Str(v) = v {
            deps.insert(k.into_string(), v.into());
          }
        }
      }
      Ok(PackageDeps { dependencies: deps })
    } else {
      println!("{:?} {:?}", deps, value);
      Err("dependencies should be a map".to_string())
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliArgs {
  input: String,
  ci: bool,
  verbose: bool,
  local_debug: bool,
}

pub fn main() -> Result<(), String> {
  // parse package.cirru

  let cli_matches = parse_cli();

  let options: CliArgs = CliArgs {
    input: cli_matches.value_of("input").unwrap_or("package.cirru").to_string(),
    ci: cli_matches.is_present("ci"),
    verbose: cli_matches.is_present("verbose"),
    local_debug: cli_matches.is_present("local_debug"),
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
    println!("created dir: {:?}", modules_dir);
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
      if current_head == version {
        dim_println(format!("found {} at {}", gray(folder), gray(&version)));
        continue;
      } else {
        // let msg = format!("module {} is at version {:?}, but required {}", folder, current_head, version);
        // println!("  {}", msg.yellow());

        // try if tag or branch exists in git history
        let has_target = git_check_branch_or_tag(&folder_path, &version)?;
        if !has_target {
          git_fetch(&folder_path)?;
          dim_println(format!("fetched {} at version {}", gray(&org_and_folder), gray(&version)));
          // fetch git repo and checkout target version
        }
        git_checkout(&folder_path, &version)?;
        dim_println(format!("checked out {} at version {}", gray(&org_and_folder), gray(&version)))
      }

      continue;
    }
    let url = if options.ci {
      format!("https://github.com/{}.git", org_and_folder)
    } else {
      format!("git@github.com:{}.git", org_and_folder)
    };
    git_clone(&modules_dir, &url, &version, options.ci)?;
    // println!("downloading {} at version {}", url, version);
    dim_println(format!("downloaded {} at version {}", gray(&org_and_folder), gray(&version)));
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
        .help("verbpse mode")
        .short('v')
        .long("verbose")
        .takes_value(false),
    )
    .arg(clap::Arg::new("ci").help("try CI mode").long("ci").takes_value(false))
    .arg(
      clap::Arg::new("local_debug")
        .help("Debug in a local")
        .long("local-debug")
        .takes_value(false),
    )
    .get_matches()
}

fn dim_println(msg: String) {
  println!("  {}", msg.truecolor(128, 128, 128));
}

fn gray(msg: &str) -> ColoredString {
  msg.truecolor(160, 160, 160)
}
