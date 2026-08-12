use crate::caps_graph::RefKind;
use std::{
  path::{Path, PathBuf},
  process::Command,
};

/// abstraction of a local git repository
pub struct GitRepo {
  pub dir: PathBuf,
}

impl GitRepo {
  fn run_command(&self, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.current_dir(&self.dir).args(args);

    let output = command.output().map_err(|e| e.to_string())?;
    if !output.status.success() {
      let err = String::from_utf8_lossy(&output.stderr);
      Err(format!("{} from args {:?}", err.trim(), command.get_args()))
    } else {
      let stdout = String::from_utf8_lossy(&output.stdout);
      Ok(stdout.trim().to_string())
    }
  }

  /// Clone one ref into an explicit destination path.
  pub fn clone_to_path(target: &Path, url: &str, version: &str, kind: &RefKind, shallow: bool) -> Result<(), String> {
    let parent = target
      .parent()
      .ok_or_else(|| format!("missing parent directory for {}", target.display()))?;
    let target_name = target
      .file_name()
      .and_then(|name| name.to_str())
      .ok_or_else(|| format!("invalid clone target {}", target.display()))?;
    if matches!(kind, RefKind::Commit) {
      let mut init = Command::new("git");
      init.current_dir(parent).args(["init", target_name]);
      run_noninteractive(&mut init)?;
      let repo = GitRepo { dir: target.to_path_buf() };
      repo.run_command(&["remote", "add", "origin", url])?;
      let mut fetch = Command::new("git");
      fetch.current_dir(target).args(["fetch", "--depth", "1", "origin", version]);
      run_noninteractive(&mut fetch)?;
      repo.run_command(&["checkout", "--detach", "FETCH_HEAD"])?;
    } else {
      let mut command = Command::new("git");
      command.current_dir(parent).args(["clone", "--branch", version]);
      if shallow {
        command.args(["--depth", "1"]);
      }
      command.args([url, target_name]);
      run_noninteractive(&mut command)?;
    }
    Ok(())
  }

  /// get SHA of a tag or ref
  /// ```bash
  /// git rev-parse <REF>
  /// ```
  #[allow(dead_code)]
  pub fn rev_parse(&self, ref_name: &str) -> Result<String, String> {
    let sha = self.run_command(&["rev-parse", ref_name])?;
    Ok(sha.trim().to_string())
  }

  pub fn head_commit(&self) -> Result<String, String> {
    self.rev_parse("HEAD")
  }

  /// Return paths changed from HEAD, including untracked files.
  pub fn status_porcelain(&self) -> Result<Vec<String>, String> {
    let output = self.run_command(&["status", "--porcelain"])?;
    Ok(output.lines().map(str::to_owned).collect())
  }
}

fn run_noninteractive(command: &mut Command) -> Result<(), String> {
  command
    .env("GIT_TERMINAL_PROMPT", "0")
    .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
  let output = command.output().map_err(|e| e.to_string())?;
  if output.status.success() {
    Ok(())
  } else {
    Err(format!(
      "{} from args {:?}",
      String::from_utf8_lossy(&output.stderr).trim(),
      command.get_args()
    ))
  }
}
