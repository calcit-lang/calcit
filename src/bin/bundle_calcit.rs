use std::{
  collections::{HashMap, HashSet},
  env,
  fmt::Debug,
  fs::{read_to_string, write},
  io,
  path::Path,
  sync::Arc,
};

use calcit::snapshot::ChangesDict;
use calcit::snapshot::{FileChangeInfo, FileInSnapShot};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::channel;
use std::time::Duration;

use walkdir::WalkDir;

use cirru_edn::Edn;
use cirru_parser::Cirru;

pub fn main() -> io::Result<()> {
  let cli_matches = parse_cli();
  let verbose = cli_matches.is_present("verbose");
  let base_dir = Path::new(cli_matches.value_of("src").expect("src"));
  let out_path = Path::new(cli_matches.value_of("out").expect("out"));
  let out_file = match out_path.extension() {
    Some(ext) => {
      let ext_str = ext.to_str().expect("ext");
      if ext_str == "cirru" {
        out_path.to_path_buf()
      } else {
        return Err(io_err(format!("expected *.cirru file, got {}", ext_str)));
      }
    }
    None => out_path.join("compact.cirru"),
  };
  let inc_file_path = out_path.join(".compact-inc.cirru");
  let no_watcher = cli_matches.is_present("once");

  let package_file = Path::new(cli_matches.value_of("src").expect("src"))
    .parent()
    .expect("parent path")
    .join("package.cirru");

  perform_compaction(base_dir, &package_file, &out_file, &inc_file_path, verbose)?;

  if !no_watcher {
    println!("\nwatch changes in {} ...\n", base_dir.display());

    let (tx, rx) = channel();
    let mut watcher: RecommendedWatcher = Watcher::new(
      tx,
      notify::Config::default()
        .with_poll_interval(Duration::from_millis(200))
        .with_compare_contents(true),
    )
    .expect("create watcher");
    watcher
      .watch(Path::new(base_dir), RecursiveMode::NonRecursive)
      .expect("start watcher");

    loop {
      match rx.recv() {
        Ok(Ok(event)) => {
          use notify::EventKind;
          match event.kind {
            EventKind::Modify(_) | EventKind::Create(_) => {
              perform_compaction(base_dir, &package_file, &out_file, &inc_file_path, verbose)?;
            }
            // ignore other events
            _ => println!("other file event: {:?}, ignored", event),
          }
        }
        Ok(Err(e)) => println!("watch error: {:?}", e),
        Err(e) => eprintln!("watch error: {:?}", e),
      }
    }
  } else {
    Ok(())
  }
}

fn perform_compaction(base_dir: &Path, package_file: &Path, out_file: &Path, inc_file_path: &Path, verbose: bool) -> io::Result<()> {
  if verbose {
    println!("reading from {}", base_dir.display());
  }

  let new_compact_file = load_files_to_edn(package_file, base_dir, verbose)?;
  let has_old_file = out_file.exists();
  let changes = if has_old_file {
    let old_compact_data = cirru_edn::parse(&read_file(&out_file)?).map_err(io_err)?;
    find_compact_changes(&new_compact_file, &old_compact_data).map_err(io_err)?
  } else {
    ChangesDict::default()
  };
  let has_changes = !changes.is_empty();

  // println!("data {:?}", changes);

  if has_changes {
    println!("writing changes");
    println!("code formatted");
    write(
      &inc_file_path,
      cirru_edn::format(&changes.try_into().map_err(io_err)?, true).expect("format edn"),
    )?;
    println!("inc file updated {}", inc_file_path.display());
  } else if has_old_file {
    println!("no changes.")
  }

  if !has_old_file || has_changes {
    write(&out_file, cirru_edn::format(&new_compact_file, true).expect("write"))?;
    println!("file wrote {}", out_file.display());
  }

  Ok(())
}

fn read_file<P>(file: P) -> io::Result<String>
where
  P: AsRef<Path> + Debug,
{
  match read_to_string(&file) {
    Ok(s) => Ok(s),
    Err(e) => Err(io_err(format!(
      "failed reading {}, {}",
      file.as_ref().as_os_str().to_string_lossy(),
      e
    ))),
  }
}

fn find_compact_changes(new_data: &Edn, old_data: &Edn) -> Result<ChangesDict, String> {
  let old_files: HashMap<Arc<str>, FileInSnapShot> = old_data.map_get("files")?.try_into()?;
  let new_files: HashMap<Arc<str>, FileInSnapShot> = new_data.map_get("files")?.try_into()?;
  let old_namespaces = old_files.keys().collect::<HashSet<_>>();
  let new_namespaces = new_files.keys().collect::<HashSet<_>>();
  let added_namespaces = new_namespaces.difference(&old_namespaces).collect::<HashSet<_>>();
  let common_namespaces = new_namespaces.intersection(&old_namespaces).collect::<HashSet<_>>();
  let removed_namespaces = old_namespaces
    .difference(&new_namespaces)
    .map(|x| x.to_owned().to_owned())
    .collect::<HashSet<Arc<_>>>();
  let added_files = added_namespaces
    .iter()
    .map(|name| (name.to_owned().to_owned().to_owned(), new_files[**name].to_owned()))
    .collect::<HashMap<Arc<str>, FileInSnapShot>>();

  let mut changed_files: HashMap<Arc<str>, FileChangeInfo> = HashMap::new();
  for namespace in common_namespaces {
    let old_file = old_files[*namespace].to_owned();
    let new_file = new_files[*namespace].to_owned();
    if old_file == new_file {
      continue;
    }
    let changes = find_file_changes(&old_file, &new_file)?;
    changed_files.insert(namespace.to_owned().to_owned(), changes);
  }

  Ok(ChangesDict {
    added: added_files,
    removed: removed_namespaces,
    changed: changed_files,
  })
}

fn find_file_changes(old_file: &FileInSnapShot, new_file: &FileInSnapShot) -> Result<FileChangeInfo, String> {
  let old_defs = old_file.defs.keys().map(ToOwned::to_owned).collect::<HashSet<Arc<str>>>();
  let new_defs = new_file.defs.keys().map(ToOwned::to_owned).collect::<HashSet<Arc<str>>>();
  let added_def_names = new_defs.difference(&old_defs).map(ToOwned::to_owned).collect::<HashSet<Arc<str>>>();
  let removed_defs = old_defs.difference(&new_defs).map(ToOwned::to_owned).collect::<HashSet<Arc<str>>>();
  let added_defs = added_def_names
    .iter()
    .map(|name| (name.to_owned(), new_file.defs[&**name].to_owned()))
    .collect::<HashMap<Arc<str>, Cirru>>();

  let mut changed_defs: HashMap<Arc<str>, Cirru> = HashMap::new();
  let common_defs = new_defs.intersection(&old_defs).collect::<HashSet<_>>();
  for def_name in common_defs {
    let old_def = old_file.defs[&**def_name].to_owned();
    let new_def = new_file.defs[&**def_name].to_owned();
    if old_def == new_def {
      continue;
    }
    changed_defs.insert(def_name.to_owned().to_owned(), new_def.to_owned());
  }

  Ok(FileChangeInfo {
    ns: if old_file.ns == new_file.ns {
      None
    } else {
      Some(new_file.ns.to_owned())
    },
    added_defs,
    removed_defs,
    changed_defs,
  })
}

fn load_files_to_edn(package_file: &Path, base_dir: &Path, verbose: bool) -> Result<Edn, io::Error> {
  let mut dict: HashMap<Edn, Edn> = HashMap::new();

  let content = read_file(package_file)?;
  let package_data = cirru_edn::parse(&content).map_err(io_err)?;

  let pkg = package_data.map_get("package").map_err(io_err)?.read_str().map_err(io_err)?;

  dict.insert(Edn::kwd("package"), Edn::Str(pkg));
  dict.insert(Edn::kwd("configs"), package_data);

  let mut files: HashMap<Edn, Edn> = HashMap::new();

  for dir_entry in WalkDir::new(base_dir) {
    let entry = dir_entry?;
    let entry_path = entry.path();

    if let Some(ext) = entry_path.extension() {
      if ext.to_str().expect("ext") == "cirru" {
        let content = read_file(entry_path)?;
        let xs = cirru_parser::parse(&content).map_err(io_err)?;

        let mut file: HashMap<Edn, Edn> = HashMap::new();
        let (ns_name, ns_code) = if let Some(Cirru::List(ns_form)) = xs.get(0) {
          match (ns_form.get(0), ns_form.get(1)) {
            (Some(Cirru::Leaf(x0)), Some(Cirru::Leaf(x1))) if &**x0 == "ns" => (x1.to_string(), ns_form),
            (a, b) => return Err(io_err(format!("in valid ns starts {:?} {:?}", a, b))),
          }
        } else {
          return Err(io_err(format!(
            "first expression of file should be a ns form, got: {:?}",
            xs.get(0)
          )));
        };
        file.insert(Edn::kwd("ns"), Edn::Quote(Cirru::List(ns_code.to_owned())));

        let mut defs: HashMap<Edn, Edn> = HashMap::with_capacity(xs.len());
        for line in xs.iter().skip(1) {
          if let Cirru::List(ys) = line {
            match (ys.get(0), ys.get(1)) {
              (Some(Cirru::Leaf(x0)), Some(Cirru::Leaf(x1))) => {
                let x0 = &**x0;
                if x0 == "def" || x0 == "defn" || x0 == "defmacro" || x0 == "defatom" || x0 == "defrecord" || x0.starts_with("def") {
                  defs.insert(Edn::str((*x1).to_owned()), Edn::Quote(line.to_owned()));
                } else {
                  return Err(io_err(format!("invalid def op: {}", x0)));
                }
              }
              (a, b) => {
                return Err(io_err(format!("invalid def code {:?} {:?}", a, b)));
              }
            }
          } else {
            return Err(io_err(format!("file line not an expr {}", line)));
          }
        }

        file.insert(Edn::kwd("defs"), Edn::Map(defs));
        files.insert(Edn::str(ns_name), Edn::Map(file));

        if verbose {
          println!("bundling {}", entry_path.display());
        }
        // a.push(entry.path().to_str().expect("extract path").to_string());
      }
    }
  }

  dict.insert(Edn::kwd("files"), Edn::Map(files));

  Ok(Edn::Map(dict))
}

pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn parse_cli() -> clap::ArgMatches {
  clap::Command::new("Calcit")
    .version(CALCIT_VERSION)
    .author("Jon. <jiyinyiyong@gmail.com>")
    .about("Calcit Bundler")
    .arg(
      clap::Arg::new("src")
        .help("source folder")
        .default_value("src/")
        .short('s')
        .long("src")
        .takes_value(true),
    )
    .arg(
      clap::Arg::new("out")
        .help("output folder")
        .default_value("./") // TODO a better default value
        .short('o')
        .long("out")
        .takes_value(true),
    )
    .arg(
      clap::Arg::new("verbose")
        .help("verbose mode")
        .short('v')
        .long("verbose")
        .takes_value(false),
    )
    .arg(
      clap::Arg::new("once")
        .help("run without watcher")
        .short('1')
        .long("once")
        .takes_value(false),
    )
    .get_matches()
}

// simulate an IO error with String
fn io_err(e: String) -> io::Error {
  io::Error::new(io::ErrorKind::InvalidData, e)
}
