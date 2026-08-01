use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A file written beside its destination and committed with one atomic rename.
///
/// Dropping an uncommitted value removes the temporary file, which keeps failed
/// multi-file operations and dry runs from leaving staging artifacts behind.
pub(crate) struct StagedFile {
  destination: PathBuf,
  temporary: PathBuf,
  committed: bool,
}

impl StagedFile {
  pub(crate) fn path(&self) -> &Path {
    &self.temporary
  }

  pub(crate) fn write_and_sync(&self, content: &[u8], label: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
      .write(true)
      .truncate(true)
      .open(&self.temporary)
      .map_err(|error| format!("Failed to write staged {label} file '{}': {error}", self.temporary.display()))?;
    file
      .write_all(content)
      .map_err(|error| format!("Failed to write staged {label} file '{}': {error}", self.temporary.display()))?;
    file
      .sync_all()
      .map_err(|error| format!("Failed to flush staged {label} file '{}': {error}", self.temporary.display()))
  }

  pub(crate) fn commit(mut self) -> Result<(), String> {
    fs::rename(&self.temporary, &self.destination).map_err(|error| {
      format!(
        "Failed to atomically replace '{}' with '{}': {error}",
        self.destination.display(),
        self.temporary.display()
      )
    })?;
    self.committed = true;
    Ok(())
  }
}

impl Drop for StagedFile {
  fn drop(&mut self) {
    if !self.committed {
      let _ = fs::remove_file(&self.temporary);
    }
  }
}

pub(crate) fn stage_atomic_file(destination: &Path, content: &[u8], label: &str) -> Result<StagedFile, String> {
  let permissions = match fs::symlink_metadata(destination) {
    Ok(metadata) if metadata.file_type().is_symlink() => {
      return Err(format!(
        "Cannot stage {label} file '{}': symbolic-link destinations are not supported.",
        destination.display()
      ));
    }
    Ok(metadata) => Some(metadata.permissions()),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
    Err(error) => {
      return Err(format!(
        "Failed to inspect staged {label} destination '{}': {error}",
        destination.display()
      ));
    }
  };
  let parent = destination.parent().unwrap_or(Path::new("."));
  fs::create_dir_all(parent)
    .map_err(|error| format!("Failed to create directory '{}' for staged {label}: {error}", parent.display()))?;
  let nonce = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_err(|error| format!("System clock error while staging {label}: {error}"))?
    .as_nanos();
  let file_name = destination.file_name().and_then(|value| value.to_str()).unwrap_or("calcit-state");

  for attempt in 0..32_u8 {
    let temporary = parent.join(format!(".{file_name}.{}.{nonce}.{attempt}.tmp", std::process::id()));
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&temporary) {
      Ok(file) => file,
      Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
      Err(error) => return Err(format!("Failed to create staged {label} file '{}': {error}", temporary.display())),
    };
    if let Err(error) = file.write_all(content).and_then(|_| file.sync_all()) {
      let _ = fs::remove_file(&temporary);
      return Err(format!("Failed to write staged {label} file '{}': {error}", temporary.display()));
    }
    if let Some(permissions) = &permissions
      && let Err(error) = fs::set_permissions(&temporary, permissions.clone())
    {
      let _ = fs::remove_file(&temporary);
      return Err(format!(
        "Failed to preserve permissions on staged {label} file '{}': {error}",
        temporary.display()
      ));
    }
    return Ok(StagedFile {
      destination: destination.to_path_buf(),
      temporary,
      committed: false,
    });
  }
  Err(format!(
    "Failed to allocate a unique staged {label} file in '{}'.",
    parent.display()
  ))
}

#[cfg(test)]
mod tests {
  use super::stage_atomic_file;
  use crate::cli_handlers::test_support::TestProject;
  use std::fs;

  #[test]
  fn staged_file_writes_syncs_and_commits_new_content() {
    let fixture = TestProject::from_fixture();
    let staged = stage_atomic_file(&fixture.path, b"before", "test state").expect("staged file should create");
    staged
      .write_and_sync(b"after", "test state")
      .expect("staged file should write and sync");
    staged.commit().expect("staged file should commit");

    assert_eq!(fs::read(&fixture.path).expect("committed file should read"), b"after");
  }

  #[cfg(unix)]
  #[test]
  fn staged_file_rejects_symbolic_link_destination() {
    let fixture = TestProject::from_fixture();
    let link = fixture.directory.join("snapshot-link.cirru");
    std::os::unix::fs::symlink(&fixture.path, &link).expect("symbolic link should create");

    let error = match stage_atomic_file(&link, b"replacement", "test state") {
      Ok(_) => panic!("symbolic link destination should be rejected"),
      Err(error) => error,
    };
    assert!(error.contains("symbolic-link destinations are not supported"), "error: {error}");
  }
}
