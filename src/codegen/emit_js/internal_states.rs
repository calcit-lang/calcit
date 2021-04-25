//! splited since deadlocks are involved

use std::collections::{HashMap, HashSet};

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

// track if it's the first compilation
static FIRST_COMPILATION: AtomicBool = AtomicBool::new(true);

lazy_static! {
  // caches program data for detecting incremental changes of libs
  static ref GLOBAL_PREVIOUS_PROGRAM_CACHES: Mutex<HashMap<String, HashSet<String>>> = Mutex::new(HashMap::new());
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
