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
    fs::write(&self.temporary, content)
      .map_err(|error| format!("Failed to write staged {label} file '{}': {error}", self.temporary.display()))?;
    OpenOptions::new()
      .read(true)
      .open(&self.temporary)
      .and_then(|file| file.sync_all())
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
  let parent = destination.parent().unwrap_or(Path::new("."));
  fs::create_dir_all(parent)
    .map_err(|error| format!("Failed to create directory '{}' for staged {label}: {error}", parent.display()))?;
  let nonce = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_err(|error| format!("System clock error while staging {label}: {error}"))?
    .as_nanos();
  let file_name = destination.file_name().and_then(|value| value.to_str()).unwrap_or("calcit-state");
  let permissions = fs::metadata(destination).ok().map(|metadata| metadata.permissions());

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
