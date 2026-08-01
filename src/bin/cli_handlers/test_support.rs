use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) struct TestProject {
  pub(crate) directory: PathBuf,
  pub(crate) path: PathBuf,
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
    Self { directory, path }
  }

  pub(crate) fn snapshot_string(&self) -> String {
    self.path.to_string_lossy().into_owned()
  }
}

impl Drop for TestProject {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.directory);
  }
}
