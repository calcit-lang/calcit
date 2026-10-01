use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

thread_local! {
  static SNAPSHOT_REVISIONS: RefCell<HashMap<PathBuf, std::rc::Weak<SnapshotWriteState>>> = RefCell::new(HashMap::new());
}

struct WriterLock(File);

impl WriterLock {
  fn acquire(path: &Path, timeout: Duration) -> Result<Self, String> {
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
      return Err(format!(
        "Cannot acquire writer lock '{}': symbolic links are not supported.",
        path.display()
      ));
    }
    let file = OpenOptions::new()
      .read(true)
      .write(true)
      .create(true)
      .truncate(false)
      .open(path)
      .map_err(|error| format!("Failed to acquire writer lock '{}': {error}", path.display()))?;
    let started = Instant::now();
    loop {
      match file.try_lock() {
        Ok(()) => break,
        Err(TryLockError::WouldBlock) if started.elapsed() < timeout => std::thread::sleep(Duration::from_millis(10)),
        Err(error) => {
          return Err(format!(
            "Failed to acquire writer lock '{}': {error}. Another writer may be active; re-read the Snapshot and retry after it finishes.",
            path.display()
          ));
        }
      }
    }
    let mut lock = Self(file);
    let mut previous = String::new();
    lock
      .0
      .read_to_string(&mut previous)
      .map_err(|error| format!("Failed to read writer lock '{}': {error}", path.display()))?;
    if !previous.is_empty() {
      eprintln!(
        "Recovered writer lock '{}' left by an interrupted writer: {}",
        path.display(),
        previous.trim()
      );
    }
    lock
      .0
      .rewind()
      .map_err(|error| format!("Failed to rewind writer lock '{}': {error}", path.display()))?;
    lock
      .0
      .set_len(0)
      .map_err(|error| format!("Failed to reset writer lock '{}': {error}", path.display()))?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    writeln!(lock.0, "pid={} acquired-at={timestamp}", std::process::id())
      .map_err(|error| format!("Failed to record writer lock '{}': {error}", path.display()))?;
    Ok(lock)
  }
}

impl Drop for WriterLock {
  fn drop(&mut self) {
    // Keep the inode stable: unlinking lets waiters lock different files.
    let _ = self.0.set_len(0);
  }
}

fn canonical_destination(path: &Path) -> Result<PathBuf, String> {
  let parent = path
    .parent()
    .filter(|parent| !parent.as_os_str().is_empty())
    .unwrap_or(Path::new("."));
  let parent =
    fs::canonicalize(parent).map_err(|error| format!("Failed to resolve Snapshot directory '{}': {error}", parent.display()))?;
  let name = path
    .file_name()
    .ok_or_else(|| format!("Invalid Snapshot path '{}'.", path.display()))?;
  Ok(parent.join(name))
}

fn revision(path: &Path) -> Result<Vec<u8>, String> {
  // Exact byte comparison avoids repeated hashing of large Snapshot sources.
  fs::read(path).map_err(|error| format!("Failed to read Snapshot '{}': {error}", path.display()))
}

/// Serialize a CLI Snapshot mutation from its initial read through its last commit.
/// Kernel locks release on termination; file age never grants ownership.
struct SnapshotWriteState {
  destination: PathBuf,
  revision: RefCell<Vec<u8>>,
  _lock: WriterLock,
}

/// Same-thread nested handlers share ownership until the last guard is dropped.
pub struct SnapshotWriteGuard {
  _state: std::rc::Rc<SnapshotWriteState>,
}

fn active_writer(destination: &Path) -> Option<std::rc::Rc<SnapshotWriteState>> {
  SNAPSHOT_REVISIONS.with(|revisions| revisions.borrow().get(destination).and_then(std::rc::Weak::upgrade))
}

impl SnapshotWriteGuard {
  /// Acquire exclusive ownership before loading source or resolving cursors.
  pub fn acquire(path: impl AsRef<Path>) -> Result<Self, String> {
    let destination = canonical_destination(path.as_ref())?;
    if let Some(state) = active_writer(&destination) {
      return Ok(Self { _state: state });
    }
    if fs::symlink_metadata(&destination).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
      return Err("Cannot mutate Snapshot: symbolic-link destinations are not supported.".to_owned());
    }
    let directory = destination.parent().unwrap().join(".calcit");
    fs::create_dir_all(&directory).map_err(|error| format!("Failed to create Snapshot lock directory: {error}"))?;
    let mut name = destination.file_name().unwrap().to_os_string();
    name.push(".lock");
    let lock = WriterLock::acquire(&directory.join(name), Duration::from_secs(5))?;
    let initial = revision(&destination)?;
    let state = std::rc::Rc::new(SnapshotWriteState {
      destination,
      revision: RefCell::new(initial),
      _lock: lock,
    });
    SNAPSHOT_REVISIONS.with(|revisions| {
      revisions
        .borrow_mut()
        .insert(state.destination.clone(), std::rc::Rc::downgrade(&state))
    });
    Ok(Self { _state: state })
  }
}

impl Drop for SnapshotWriteState {
  fn drop(&mut self) {
    SNAPSHOT_REVISIONS.with(|revisions| revisions.borrow_mut().remove(&self.destination));
  }
}

fn verify_revision(destination: &Path) -> Result<(), String> {
  let path = canonical_destination(destination)?;
  let matches = match active_writer(&path) {
    Some(state) => revision(&path)? == *state.revision.borrow(),
    None => true,
  };
  if !matches {
    return Err(format!(
      "Snapshot revision mismatch for '{}': source changed during this operation. No changes were committed; re-read the Snapshot and retry.",
      path.display()
    ));
  }
  Ok(())
}

/// Replace a Snapshot through the staging path shared with transactions/cursors.
pub fn write_snapshot(path: impl AsRef<Path>, content: &[u8]) -> Result<(), String> {
  stage_atomic_file(path.as_ref(), content, "snapshot")?.commit()
}

/// A file staged beside its destination and committed with one atomic rename.
///
/// Dropping an uncommitted value removes the temporary file, which keeps failed
/// multi-file operations and dry runs from leaving staging artifacts behind.
pub struct StagedFile {
  destination: PathBuf,
  temporary: PathBuf,
  _lock: WriterLock,
  committed: bool,
  _snapshot_writer: Option<std::rc::Rc<SnapshotWriteState>>,
}

impl StagedFile {
  pub fn path(&self) -> &Path {
    &self.temporary
  }

  pub fn write_and_sync(&self, content: &[u8], label: &str) -> Result<(), String> {
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

  pub fn commit(mut self) -> Result<(), String> {
    let path = canonical_destination(&self.destination)?;
    verify_revision(&self.destination)?;
    let next_revision = revision(&self.temporary)?;
    fs::rename(&self.temporary, &self.destination).map_err(|error| {
      format!(
        "Failed to atomically replace '{}' with '{}': {error}",
        self.destination.display(),
        self.temporary.display()
      )
    })?;
    self.committed = true;
    // Directory syncing is best-effort: some Unix filesystems do not support it.
    #[cfg(unix)]
    if let Some(parent) = path.parent()
      && let Ok(directory) = File::open(parent)
    {
      let _ = directory.sync_all();
    }
    if let Some(state) = active_writer(&path) {
      *state.revision.borrow_mut() = next_revision;
    }
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

pub fn stage_atomic_file(destination: &Path, content: &[u8], label: &str) -> Result<StagedFile, String> {
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
  let lock_directory = parent.join(".calcit/atomic");
  fs::create_dir_all(&lock_directory).map_err(|error| format!("Failed to create atomic writer lock directory: {error}"))?;
  let canonical = canonical_destination(destination)?;
  // Native names preserve the filesystem's case-folding identity for aliases.
  let mut lock_name = destination.file_name().unwrap().to_os_string();
  lock_name.push(".lock");
  let lock = lock_directory.join(lock_name);
  let lock = WriterLock::acquire(&lock, Duration::ZERO)?;

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
      _lock: lock,
      committed: false,
      _snapshot_writer: active_writer(&canonical),
    });
  }
  Err(format!(
    "Failed to allocate a unique staged {label} file in '{}'.",
    parent.display()
  ))
}

#[cfg(test)]
mod tests {
  use super::{SnapshotWriteGuard, WriterLock, stage_atomic_file, write_snapshot};
  use std::fs;
  use std::io::Seek;

  #[test]
  fn atomic_writer_identity_matches_filesystem_case_aliases() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("state");
    let alias = fixture.path().join("STATE");
    fs::write(&path, "original").unwrap();
    let case_alias = alias.exists();
    if !case_alias {
      fs::write(&alias, "distinct file").unwrap();
    }
    let _first = stage_atomic_file(&path, b"first", "test state").unwrap();
    let second = stage_atomic_file(&alias, b"second", "test state");
    assert_eq!(
      second.is_err(),
      case_alias,
      "aliases must share ownership, but distinct files must remain independent"
    );
  }

  #[test]
  fn stale_revision_rejects_atomic_commit_and_preserves_external_edit() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("calcit.cirru");
    fs::write(&path, "original").unwrap();
    let guard = SnapshotWriteGuard::acquire(&path).unwrap();
    let staged = stage_atomic_file(&path, b"replacement", "snapshot").unwrap();
    // Deliberately simulate a non-cooperating writer on an isolated fixture.
    fs::write(&path, "external edit").unwrap();
    let error = staged.commit().unwrap_err();
    assert!(error.contains("Snapshot revision mismatch"), "{error}");
    assert_eq!(fs::read_to_string(&path).unwrap(), "external edit");
    drop(guard);
    write_snapshot(&path, b"retry after fresh read").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "retry after fresh read");
  }

  #[test]
  fn successive_commits_advance_the_guard_revision() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("calcit.cirru");
    fs::write(&path, "original").unwrap();
    let _guard = SnapshotWriteGuard::acquire(&path).unwrap();
    write_snapshot(&path, b"first").unwrap();
    write_snapshot(&path, b"second").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "second");
  }

  #[test]
  fn nested_guards_keep_ownership_and_revision_until_the_last_guard_drops() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("calcit.cirru");
    fs::write(&path, "original").unwrap();
    let outer = SnapshotWriteGuard::acquire(&path).unwrap();
    let inner = SnapshotWriteGuard::acquire(&path).unwrap();
    drop(outer);
    let lock_path = fixture.path().join(".calcit/calcit.cirru.lock");
    assert!(WriterLock::acquire(&lock_path, std::time::Duration::ZERO).is_err());
    fs::write(&path, "external edit").unwrap();
    assert!(
      write_snapshot(&path, b"must reject")
        .unwrap_err()
        .contains("Snapshot revision mismatch")
    );
    drop(inner);
    let _fresh = SnapshotWriteGuard::acquire(&path).unwrap();
    write_snapshot(&path, b"fresh edit").unwrap();
  }

  #[test]
  fn staged_commit_retains_the_snapshot_guard_after_its_caller_drops() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("calcit.cirru");
    fs::write(&path, "original").unwrap();
    let guard = SnapshotWriteGuard::acquire(&path).unwrap();
    let staged = stage_atomic_file(&path, b"replacement", "snapshot").unwrap();
    drop(guard);
    assert!(WriterLock::acquire(&fixture.path().join(".calcit/calcit.cirru.lock"), std::time::Duration::ZERO).is_err());
    fs::write(&path, "external edit").unwrap();
    assert!(staged.commit().unwrap_err().contains("Snapshot revision mismatch"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "external edit");
    let _fresh = SnapshotWriteGuard::acquire(&path).unwrap();
  }

  #[test]
  #[ignore = "timing benchmark; run explicitly with --ignored and --test-threads=1"]
  fn snapshot_guard_adds_less_than_ten_milliseconds_to_serial_writes() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("calcit.cirru");
    let source = std::env::var_os("CALCIT_WRITE_GUARD_BENCHMARK_SNAPSHOT")
      .map(std::path::PathBuf::from)
      .unwrap_or_else(|| std::path::PathBuf::from("calcit/test-wasm.cirru"));
    fs::copy(source, &path).unwrap();
    let content = fs::read(&path).unwrap();
    let mut baseline = Vec::new();
    let mut protected = Vec::new();
    for _ in 0..21 {
      let start = std::time::Instant::now();
      write_snapshot(&path, &content).unwrap();
      baseline.push(start.elapsed());
      let start = std::time::Instant::now();
      {
        let _guard = SnapshotWriteGuard::acquire(&path).unwrap();
        write_snapshot(&path, &content).unwrap();
      }
      protected.push(start.elapsed());
    }
    baseline.sort();
    protected.sort();
    let overhead = protected[10].saturating_sub(baseline[10]);
    println!(
      "Snapshot bytes={}, baseline median={:?}, protected median={:?}, overhead={:?}",
      content.len(),
      baseline[10],
      protected[10],
      overhead
    );
    assert!(
      overhead < std::time::Duration::from_millis(10),
      "serial-write overhead: {overhead:?}"
    );
  }

  #[test]
  fn live_old_lock_cannot_be_reclaimed_and_its_inode_is_retained() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("writer.lock");
    let mut first = WriterLock::acquire(&path, std::time::Duration::ZERO).unwrap();
    first.0.rewind().unwrap();
    first.0.set_len(0).unwrap();
    std::io::Write::write_all(&mut first.0, b"pid=123 acquired-at=1\n").unwrap();
    assert!(WriterLock::acquire(&path, std::time::Duration::ZERO).is_err());
    first.0.rewind().unwrap();
    let mut record = String::new();
    std::io::Read::read_to_string(&mut first.0, &mut record).unwrap();
    assert_eq!(record, "pid=123 acquired-at=1\n");
    drop(first);
    assert!(path.exists(), "the stable lock inode must never be unlinked");
    let _next = WriterLock::acquire(&path, std::time::Duration::ZERO).unwrap();
  }

  #[test]
  fn recovered_record_is_read_and_replaced_through_the_locked_handle() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("writer.lock");
    fs::write(&path, "pid=999999 acquired-at=1 old-record-padding\n").unwrap();
    let mut guard = WriterLock::acquire(&path, std::time::Duration::ZERO).unwrap();
    guard.0.rewind().unwrap();
    let mut record = String::new();
    std::io::Read::read_to_string(&mut guard.0, &mut record).unwrap();
    assert!(record.starts_with(&format!("pid={} acquired-at=", std::process::id())));
    assert!(!record.contains("old-record-padding"));
    drop(guard);
    assert!(fs::read(&path).unwrap().is_empty());
  }

  #[test]
  fn staged_file_writes_syncs_and_commits_new_content() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("state");
    let staged = stage_atomic_file(&path, b"before", "test state").expect("staged file should create");
    staged
      .write_and_sync(b"after", "test state")
      .expect("staged file should write and sync");
    staged.commit().expect("staged file should commit");

    assert_eq!(fs::read(&path).expect("committed file should read"), b"after");
  }

  #[cfg(unix)]
  #[test]
  fn staged_file_rejects_symbolic_link_destination() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("state");
    fs::write(&path, "original").unwrap();
    let link = fixture.path().join("snapshot-link.cirru");
    std::os::unix::fs::symlink(&path, &link).expect("symbolic link should create");

    let error = match stage_atomic_file(&link, b"replacement", "test state") {
      Ok(_) => panic!("symbolic link destination should be rejected"),
      Err(error) => error,
    };
    assert!(error.contains("symbolic-link destinations are not supported"), "error: {error}");
  }
}
