//! splited since deadlocks are involved

use std::collections::{HashMap, HashSet};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

// track if it's the first compilation
static FIRST_COMPILATION: AtomicBool = AtomicBool::new(true);

lazy_static! {
  // caches program data for detecting incremental changes of libs
  static ref GLOBAL_PREVIOUS_PROGRAM_CACHES: RwLock<HashMap<Box<str>, HashSet<Box<str>>>> = RwLock::new(HashMap::new());
}

pub fn lookup_prev_ns_cache(ns: &str) -> Option<HashSet<Box<str>>> {
  let previous_program_caches = &GLOBAL_PREVIOUS_PROGRAM_CACHES.read().unwrap();
  if previous_program_caches.contains_key(ns) {
    Some(previous_program_caches[ns].to_owned())
  } else {
    None
  }
}

pub fn write_as_ns_cache(ns: &str, v: HashSet<Box<str>>) {
  let mut previous_program_caches = GLOBAL_PREVIOUS_PROGRAM_CACHES.write().unwrap();
  (*previous_program_caches).insert(ns.to_owned().into_boxed_str(), v);
}

pub fn is_first_compilation() -> bool {
  FIRST_COMPILATION.load(Ordering::Relaxed)
}

pub fn finish_compilation() -> Result<(), String> {
  FIRST_COMPILATION.store(false, Ordering::SeqCst);
  Ok(())
}
