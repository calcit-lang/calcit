use cirru_edn::{Edn, from_edn};
use cirru_parser::Cirru;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotConfigs {
  #[serde(rename = "init-fn")]
  pub init_fn: String,
  #[serde(rename = "reload-fn")]
  pub reload_fn: String,
  #[serde(default)]
  pub modules: Vec<String>,
  #[serde(default)]
  pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeEntry {
  pub doc: String,
  #[serde(default)]
  pub examples: Vec<Cirru>,
  pub code: Cirru,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInSnapShot {
  pub ns: CodeEntry,
  pub defs: HashMap<String, CodeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
  pub package: String,
  pub about: Option<String>,
  pub configs: SnapshotConfigs,
  pub entries: HashMap<String, SnapshotConfigs>,
  pub files: HashMap<String, FileInSnapShot>,
}

fn main() {
  println!("cargo:rerun-if-changed=src/cirru/calcit-core.cirru");

  let out_dir = env::var_os("OUT_DIR").unwrap();
  let dest_path = Path::new(&out_dir).join("calcit-core.rmp");

  let core_content = fs::read_to_string("src/cirru/calcit-core.cirru").expect("read core");
  let core_data = cirru_edn::parse(&core_content).expect("parse core");

  // Minimal logic to convert Edn to Snapshot as in src/snapshot.rs
  let data = core_data.view_map().expect("map");
  let pkg: String = from_edn(data.get_or_nil("package")).expect("pkg");
  let about = match data.get_or_nil("about") {
    Edn::Nil => None,
    value => Some(from_edn::<String>(value).expect("about")),
  };

  let files: HashMap<String, FileInSnapShot> = from_edn(data.get_or_nil("files")).expect("files");

  let snapshot = Snapshot {
    package: pkg,
    about,
    configs: from_edn(data.get_or_nil("configs")).expect("configs"),
    entries: from_edn(data.get_or_nil("entries")).expect("entries"),
    files,
  };

  let mut buf = Vec::new();
  snapshot.serialize(&mut rmp_serde::Serializer::new(&mut buf)).expect("serialize");
  fs::write(dest_path, buf).expect("write");
}
