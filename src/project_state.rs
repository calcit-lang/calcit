use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const PROJECT_STATE_DIRECTORY: &str = ".calcit";
pub const CURSOR_STATE_FILE: &str = "cursor.cirru";
pub const ERROR_STATE_FILE: &str = "error.cirru";

static ACTIVE_PROJECT_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

pub fn project_directory_for_snapshot(snapshot_file: &str) -> &Path {
  Path::new(snapshot_file)
    .parent()
    .filter(|path| !path.as_os_str().is_empty())
    .unwrap_or(Path::new("."))
}

pub fn state_directory(project_directory: &Path) -> PathBuf {
  project_directory.join(PROJECT_STATE_DIRECTORY)
}

pub fn state_file(project_directory: &Path, file_name: &str) -> PathBuf {
  state_directory(project_directory).join(file_name)
}

pub fn state_file_for_snapshot(snapshot_file: &str, file_name: &str) -> PathBuf {
  state_file(project_directory_for_snapshot(snapshot_file), file_name)
}

pub fn set_active_project_directory_from_snapshot(snapshot_file: &str) {
  let _ = ACTIVE_PROJECT_DIRECTORY.set(project_directory_for_snapshot(snapshot_file).to_path_buf());
}

pub fn active_state_file(file_name: &str) -> PathBuf {
  state_file(
    ACTIVE_PROJECT_DIRECTORY.get().map(PathBuf::as_path).unwrap_or(Path::new(".")),
    file_name,
  )
}

pub fn ensure_state_directory(project_directory: &Path) -> io::Result<PathBuf> {
  let directory = state_directory(project_directory);
  fs::create_dir_all(&directory)?;
  Ok(directory)
}

pub fn migrate_legacy_file(legacy: &Path, destination: &Path) -> io::Result<bool> {
  if destination.exists() || !legacy.exists() {
    return Ok(false);
  }
  if let Some(parent) = destination.parent() {
    fs::create_dir_all(parent)?;
  }
  fs::rename(legacy, destination)?;
  Ok(true)
}

#[cfg(test)]
mod tests {
  use super::{CURSOR_STATE_FILE, migrate_legacy_file, state_file_for_snapshot};
  use std::fs;

  #[test]
  fn state_file_is_next_to_snapshot_under_calcit_directory() {
    assert_eq!(
      state_file_for_snapshot("examples/demo/calcit.cirru", CURSOR_STATE_FILE),
      std::path::PathBuf::from("examples/demo/.calcit/cursor.cirru")
    );
    assert_eq!(
      state_file_for_snapshot("calcit.cirru", CURSOR_STATE_FILE),
      std::path::PathBuf::from("./.calcit/cursor.cirru")
    );
  }

  #[test]
  fn legacy_file_moves_once_without_overwriting_destination() {
    let directory = std::env::temp_dir().join(format!(
      "calcit-project-state-test-{}-{}",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("test clock should be valid")
        .as_nanos()
    ));
    fs::create_dir(&directory).expect("temp dir should create");
    let legacy = directory.join(".calcit-cursor.cirru");
    let destination = directory.join(".calcit/cursor.cirru");
    fs::write(&legacy, "legacy").expect("legacy state should write");

    assert!(migrate_legacy_file(&legacy, &destination).expect("legacy state should migrate"));
    assert_eq!(fs::read_to_string(&destination).expect("new state should read"), "legacy");
    assert!(!legacy.exists());

    fs::write(&legacy, "new legacy").expect("second legacy state should write");
    assert!(!migrate_legacy_file(&legacy, &destination).expect("existing destination should win"));
    assert_eq!(fs::read_to_string(&destination).expect("new state should remain"), "legacy");
    fs::remove_dir_all(directory).expect("temp dir should clean up");
  }
}
