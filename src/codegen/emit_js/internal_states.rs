//! splited since deadlocks are involved

use std::collections::{HashMap, HashSet};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

// track if it's the first compilation
static FIRST_COMPILATION: AtomicBool = AtomicBool::new(true);

lazy_static! {
  // caches program data for detecting incremental changes of libs
  static ref GLOBAL_PREVIOUS_PROGRAM_CACHES: RwLock<HashMap<Arc<str>, HashSet<Arc<str>>>> = RwLock::new(HashMap::new());
}

pub fn lookup_prev_ns_cache(ns: &str) -> Option<HashSet<Arc<str>>> {
  let previous_program_caches = &GLOBAL_PREVIOUS_PROGRAM_CACHES.read().expect("load cache");
  previous_program_caches.get(ns).map(|v| v.to_owned())
}

pub fn write_as_ns_cache(ns: &str, v: HashSet<Arc<str>>) {
  let mut previous_program_caches = GLOBAL_PREVIOUS_PROGRAM_CACHES.write().expect("write cache");
  (*previous_program_caches).insert(ns.to_owned().into(), v);
}

pub fn is_first_compilation() -> bool {
  FIRST_COMPILATION.load(Ordering::Relaxed)
}

pub fn finish_compilation() -> Result<(), String> {
  FIRST_COMPILATION.store(false, Ordering::SeqCst);
  Ok(())
}
