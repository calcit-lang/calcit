//! splited since deadlocks are involved

use std::collections::{HashMap, HashSet};

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

// track if it's the first compilation
static FIRST_COMPILATION: AtomicBool = AtomicBool::new(true);

#[derive(Debug, PartialEq, Clone)]
pub struct CollectedImportItem {
  pub ns: String,
  pub just_ns: bool,
  pub ns_in_str: bool,
}

lazy_static! {
  // caches program data for detecting incremental changes of libs
  static ref GLOBAL_PREVIOUS_PROGRAM_CACHES: Mutex<HashMap<String, HashSet<String>>> = Mutex::new(HashMap::new());

  // TODO mutable way of collect things of a single tile
  static ref GLOBAL_COLLECTED_IMPORTS: Mutex <HashMap<String, CollectedImportItem>> = Mutex::new(HashMap::new());
}

pub fn lookup_prev_ns_cache(ns: &str) -> Option<HashSet<String>> {
  let previous_program_caches = &GLOBAL_PREVIOUS_PROGRAM_CACHES.lock().unwrap();
  if previous_program_caches.contains_key(ns) {
    Some(previous_program_caches[ns].clone())
  } else {
    None
  }
}

pub fn write_as_ns_cache(ns: &str, v: HashSet<String>) {
  let previous_program_caches = &mut GLOBAL_PREVIOUS_PROGRAM_CACHES.lock().unwrap();
  previous_program_caches.insert(ns.to_string(), v);
}

pub fn is_first_compilation() -> bool {
  FIRST_COMPILATION.load(Ordering::Relaxed)
}

pub fn finish_compilation() -> Result<(), String> {
  FIRST_COMPILATION.store(false, Ordering::SeqCst);
  Ok(())
}

pub fn clone_imports() -> Result<HashMap<String, CollectedImportItem>, String> {
  let mut xs: HashMap<String, CollectedImportItem> = HashMap::new();
  let collected_imports = &GLOBAL_COLLECTED_IMPORTS.lock().unwrap();
  for k in collected_imports.keys() {
    xs.insert(k.to_string(), collected_imports[k].clone());
  }
  Ok(xs)
}

pub fn track_import(k: String, v: CollectedImportItem) -> Result<(), String> {
  let collected_imports = &mut GLOBAL_COLLECTED_IMPORTS.lock().unwrap();
  collected_imports.insert(k, v);
  Ok(())
}

pub fn clear_imports() -> Result<(), String> {
  let collected_imports = &mut GLOBAL_COLLECTED_IMPORTS.lock().unwrap();
  collected_imports.clear();
  Ok(())
}

pub fn lookup_import(k: &str) -> Option<CollectedImportItem> {
  let collected_imports = &GLOBAL_COLLECTED_IMPORTS.lock().unwrap();
  match collected_imports.get(k) {
    Some(v) => Some(v.clone()),
    None => None,
  }
}
