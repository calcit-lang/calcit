use calcit::snapshot;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) struct TestProject {
  pub(crate) directory: PathBuf,
  pub(crate) path: PathBuf,
  /// Index of the trailing node in `app.main/main!`. Cursor and query tests use
  /// it so they do not hard-code an index that shifts whenever the shared
  /// fixture gains or loses a `main!` call.
  pub(crate) main_tail: usize,
}

impl TestProject {
  pub(crate) fn from_fixture() -> Self {
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("test clock should be valid")
      .as_nanos();
    let counter = TEST_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!("calcit-cli-test-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&directory).expect("test project directory should be created");
    let path = directory.join("calcit.cirru");
    fs::copy("calcit/test.cirru", &path).expect("test snapshot fixture should copy");
    let main_tail = fixture_main_tail(&path);
    Self {
      directory,
      path,
      main_tail,
    }
  }

  pub(crate) fn snapshot_string(&self) -> String {
    self.path.to_string_lossy().into_owned()
  }
}

fn fixture_main_tail(path: &Path) -> usize {
  let content = fs::read_to_string(path).expect("fixture snapshot should read");
  let data = cirru_edn::parse(&content).expect("fixture snapshot should parse");
  let snapshot = snapshot::load_snapshot_data(&data, &path.to_string_lossy()).expect("fixture snapshot should load");
  let main = snapshot
    .files
    .get("app.main")
    .and_then(|file| file.defs.get("main!"))
    .expect("fixture app.main/main! should exist");
  match &main.code {
    cirru_parser::Cirru::List(items) => items.len() - 1,
    other => panic!("fixture app.main/main! should be a list, got {other:?}"),
  }
}

impl Drop for TestProject {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.directory);
  }
}
