use super::*;
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::sync::Once;

static PLATFORM_INIT: Once = Once::new();

fn lock_suite() -> std::sync::MutexGuard<'static, ()> {
  super::GLOBAL_TEST_LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

fn load_and_run_cirru(path: &str) {
  builtins::effects::init_effects_states();

  PLATFORM_INIT.call_once(|| {
    #[cfg(not(target_arch = "wasm32"))]
    super::injection::inject_platform_apis();
  });

  let content = fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read: {path}"));
  let data = cirru_edn::parse(&content).unwrap_or_else(|e| panic!("Failed to parse {path}: {e}"));
  let mut snapshot = snapshot::load_snapshot_data(&data, path).unwrap_or_else(|e| panic!("Failed to load {path}: {e}"));

  // load modules (local .cirru files referenced in the snapshot config)
  let base_dir = Path::new(path).parent().expect("extract parent");
  let module_folder = dirs::home_dir()
    .map(|buf| buf.as_path().join(".config/calcit/modules/"))
    .expect("failed to load $HOME");
  for module_path in &snapshot.active_entry().expect("default entry").modules.clone() {
    let module_data = calcit::load_module(module_path, base_dir, &module_folder)
      .unwrap_or_else(|e| panic!("Failed to load module {module_path} for {path}: {e}"));
    for (k, v) in &module_data.files {
      if !snapshot.files.contains_key(k) {
        snapshot.files.insert(k.to_owned(), v.to_owned());
      }
    }
  }

  let core_snapshot = calcit::load_core_snapshot().expect("load core snapshot");
  for (k, v) in core_snapshot.files {
    snapshot.files.insert(k.to_owned(), v.to_owned());
  }

  {
    let mut prgm = program::PROGRAM_CODE_DATA.write().expect("open program data");
    *prgm = program::extract_program_data(&snapshot).expect("extract program data");
  }

  let selected_entry = snapshot.active_entry().expect("default entry");
  let config_init = selected_entry.init_fn.to_string();
  let config_reload = selected_entry.reload_fn.to_string();
  let (init_ns, init_def) = util::string::extract_ns_def(&config_init).expect("extract init ns/def");
  let (reload_ns, _reload_def) = util::string::extract_ns_def(&config_reload).expect("extract reload ns/def");

  program::clear_runtime_caches_for_reload(init_ns.clone().into(), reload_ns.clone().into(), true).expect("clear runtime caches");

  let warmup_warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
  runner::preprocess::ensure_ns_def_compiled(
    calcit::calcit::CORE_NS,
    calcit::calcit::BUILTIN_IMPLS_ENTRY,
    &warmup_warnings,
    &CallStackList::default(),
  )
  .expect("preprocess builtin impls");

  let result = calcit::run_program_with_docs(init_ns.into(), init_def.into(), &[]);
  if let Err(e) = result {
    if !e.warnings.is_empty() {
      for w in e.warnings.iter().take(5) {
        eprintln!("  warning: {w}");
      }
    }
    panic!("{path} failed: {}", e.msg);
  }
}

/// Run a test in a dedicated thread with a large stack to avoid stack overflows
/// in deeply recursive Calcit evaluation.  The suite lock is acquired inside the
/// spawned thread so it is held for the duration of the run and dropped before
/// the thread exits.
fn run_with_large_stack(path: &'static str) {
  const STACK_SIZE: usize = 32 * 1024 * 1024; // 32 MiB
  std::thread::Builder::new()
    .stack_size(STACK_SIZE)
    .spawn(move || {
      let _guard = lock_suite();
      load_and_run_cirru(path);
    })
    .expect("spawn test thread")
    .join()
    .expect("test thread panicked");
}

/// Main integration test: runs calcit/test.cirru which loads all 23 sub-test
/// modules plus util.cirru — equivalent to `cargo run --bin cr -- calcit/test.cirru`.
#[test]
fn cirru_test_suite() {
  run_with_large_stack("calcit/test.cirru");
}

#[test]
fn cirru_test_recur_arity() {
  run_with_large_stack("calcit/test-recur-arity.cirru");
}
