use std::{
  collections::HashMap,
  env,
  fs::{read_to_string, write},
  io,
  path::Path,
};

use walkdir::WalkDir;

use cirru_edn::Edn;
use cirru_parser::Cirru;

pub fn main() -> io::Result<()> {
  let cli_matches = parse_cli();
  let verbose = cli_matches.is_present("verbose");
  let base_dir = Path::new(cli_matches.value_of("src").unwrap());
  let out_path = Path::new(cli_matches.value_of("out").unwrap());
  let out_file = match out_path.extension() {
    Some(ext) => {
      let ext_str = ext.to_str().unwrap();
      if ext_str == "cirru" {
        out_path.to_path_buf()
      } else {
        return Err(io_err(format!("expected *.cirru file, got {}", ext_str)));
      }
    }
    None => out_path.join("compact.cirru"),
  };
  if verbose {
    println!("reading from {}", base_dir.display());
  }

  let mut dict: HashMap<Edn, Edn> = HashMap::new();
  // let mut a: Vec<String> = vec![];
  let package_file = Path::new(cli_matches.value_of("src").unwrap())
    .parent()
    .unwrap()
    .join("package.cirru");

  let content = read_to_string(package_file)?;
  let package_data = cirru_edn::parse(&content).map_err(io_err)?;

  let pkg = package_data.map_get("package").map_err(io_err)?.read_string().map_err(io_err)?;

  dict.insert(Edn::Keyword(String::from("package")), Edn::Str(pkg));
  dict.insert(Edn::Keyword(String::from("configs")), package_data);

  let mut files: HashMap<Edn, Edn> = HashMap::new();

  for dir_entry in WalkDir::new(base_dir) {
    let entry = dir_entry.unwrap();

    if let Some(ext) = entry.path().extension() {
      if ext.to_str().unwrap() == "cirru" {
        let content = read_to_string(entry.path())?;
        let xs = cirru_parser::parse(&content).map_err(io_err)?;

        let mut file: HashMap<Edn, Edn> = HashMap::new();
        let (ns_name, ns_code) = if let Some(Cirru::List(ns_form)) = xs.get(0) {
          match (ns_form.get(0), ns_form.get(1)) {
            (Some(Cirru::Leaf(x0)), Some(Cirru::Leaf(x1))) if x0.as_str() == "ns" => (x1.to_string(), ns_form),
            (a, b) => return Err(io_err(format!("in valid ns starts {:?} {:?}", a, b))),
          }
        } else {
          return Err(io_err(format!(
            "first expression of file should be a ns form, got: {:?}",
            xs.get(0)
          )));
        };
        file.insert(Edn::Keyword(String::from("ns")), Edn::Quote(Cirru::List(ns_code.to_owned())));

        let mut defs: HashMap<Edn, Edn> = HashMap::new();
        for (idx, line) in xs.iter().enumerate() {
          if idx > 0 {
            if let Cirru::List(ys) = line {
              match (ys.get(0), ys.get(1)) {
                (Some(Cirru::Leaf(x0)), Some(Cirru::Leaf(x1))) => {
                  if x0 == "def" || x0 == "defn" || x0 == "defmacro" || x0 == "defatom" || x0 == "defrecord" || x0.starts_with("def") {
                    defs.insert(Edn::Str(x1.to_owned()), Edn::Quote(line.to_owned()));
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
          } else {
          }
        }

        file.insert(Edn::Keyword(String::from("defs")), Edn::Map(defs));
        files.insert(Edn::Str(ns_name.to_owned()), Edn::Map(file));

        if verbose {
          println!("bundling {}", entry.path().display());
        }
        // a.push(entry.path().to_str().unwrap().to_string());
      }
    }
  }

  dict.insert(Edn::Keyword(String::from("files")), Edn::Map(files));

  // println!("data {}", Edn::Map(dict));

  write(&out_file, cirru_edn::format(&Edn::Map(dict), true).unwrap())?;
  println!("file created at {}", out_file.display());

  Ok(())
}

pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn parse_cli<'a>() -> clap::ArgMatches<'a> {
  clap::App::new("Calcit Runner")
    .version(CALCIT_VERSION)
    .author("Jon. <jiyinyiyong@gmail.com>")
    .about("Calcit Runner Bundler")
    .arg(
      clap::Arg::with_name("src")
        .help("source folder")
        .default_value("src/")
        .short("s")
        .long("src")
        .takes_value(true),
    )
    .arg(
      clap::Arg::with_name("out")
        .help("output folder")
        .default_value("./") // TODO a better default value
        .short("o")
        .long("out")
        .takes_value(true),
    )
    .arg(
      clap::Arg::with_name("verbose")
        .help("verbose mode")
        .short("v")
        .long("verbose")
        .takes_value(false),
    )
    .get_matches()
}

// simulate an IO error with String
fn io_err(e: String) -> io::Error {
  io::Error::new(io::ErrorKind::InvalidData, e)
}
