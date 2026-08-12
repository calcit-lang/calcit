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
  pub fn clone_to_path(target: &Path, url: &str, version: &str, shallow: bool) -> Result<(), String> {
    let parent = target
      .parent()
      .ok_or_else(|| format!("missing parent directory for {}", target.display()))?;
    let container = GitRepo { dir: parent.to_path_buf() };
    let target_name = target
      .file_name()
      .and_then(|name| name.to_str())
      .ok_or_else(|| format!("invalid clone target {}", target.display()))?;
    if shallow {
      container.run_command(&["clone", "--branch", version, "--depth", "1", url, target_name])?;
    } else {
      container.run_command(&["clone", "--branch", version, url, target_name])?;
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
