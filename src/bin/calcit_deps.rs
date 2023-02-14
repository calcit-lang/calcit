//! CLI tool to download packages from github,
//! packages are defined in `package.cirru` file
//!
//! files are stored in `~/.config/calcit/modules/`.

mod git;

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

pub fn main() -> Result<(), String> {
  // parse package.cirru

  let cli_matches = parse_cli();

  // if file exists
  let package_cirru = cli_matches.value_of("input").unwrap_or("package.cirru");
  if Path::new(package_cirru).exists() {
    let content = fs::read_to_string(package_cirru).map_err(|e| e.to_string())?;
    let parsed = cirru_edn::parse(&content)?;
    let deps: PackageDeps = parsed.try_into()?;

    download_deps(deps.dependencies)?;

    Ok(())
  } else {
    eprintln!("Error: no package.cirru found!");
    std::process::exit(1);
  }
}

fn download_deps(deps: HashMap<String, String>) -> Result<(), String> {
  // ~/.config/calcit/modules/
  let modules_dir = dirs::home_dir().ok_or("no config dir")?.join(".config/calcit/test-modules");

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
        println!("module {} is at version {}", folder, version);
        continue;
      } else {
        println!("module {} is at version {:?}, but required {}", folder, current_head, version);
        // try if tag or branch exists in git history
        let has_target = git_check_branch_or_tag(&folder_path, &version)?;
        if has_target {
          // fetch git repo and checkout target version
          git_checkout(&folder_path, &version)?;
          println!("checked out {} at version {}", org_and_folder, version)
        } else {
          git_fetch(&folder_path)?;
          println!("fetched {} at version {}", org_and_folder, version);
        }
      }

      continue;
    }
    let url = format!("https://github.com/{}.git", org_and_folder);
    println!("downloading {} at version {}", url, version);
    git_clone(&modules_dir, &url, &version)?;
  }

  Ok(())
}

pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn parse_cli() -> clap::ArgMatches {
  clap::Command::new("Calcit")
    .version(CALCIT_VERSION)
    .author("Jon. <jiyinyiyong@gmail.com>")
    .about("Calcit Deps")
    .arg(
      clap::Arg::new("input")
        .help("entry file path")
        .default_value("compact.cirru")
        .index(1),
    )
    .arg(
      clap::Arg::new("verbose")
        .help("verbpse mode")
        .short('v')
        .long("verbose")
        .takes_value(true),
    )
    .get_matches()
}
