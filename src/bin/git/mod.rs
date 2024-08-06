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
      Err(err.trim().to_string())
    } else {
      let stdout = String::from_utf8_lossy(&output.stdout);
      Ok(stdout.trim().to_string())
    }
  }

  pub fn checkout(&self, version: &str) -> Result<(), String> {
    self.run_command(&["checkout", version]).map(|_a| ())
  }

  /// clone to directory
  pub fn clone_to(dir: &Path, url: &str, version: &str, shallow: bool) -> Result<(), String> {
    let container = GitRepo { dir: dir.to_path_buf() };
    if shallow {
      container.run_command(&["clone", "--branch", version, "--depth", "1", url])?;
    } else {
      container.run_command(&["clone", "--branch", version, url])?;
    }
    Ok(())
  }

  /// get the current head of the repository
  pub fn current_head(&self) -> Result<GitHead, String> {
    let branch = self.run_command(&["branch", "--show-current"])?;
    if branch.is_empty() {
      // probably a tag
      Ok(GitHead::Tag(self.describe_tag()?))
    } else {
      Ok(GitHead::Branch(branch))
    }
  }

  /// get unix timestamp of a commit
  /// ```bash
  /// git show -s --format=%ct <SHA>
  /// ```
  pub fn timestamp(&self, sha: &str) -> Result<u32, String> {
    let timestamp = self.run_command(&["show", "-s", "--format=%ct", sha])?;
    let v = timestamp.trim().parse::<u32>().map_err(|e| e.to_string())?;
    Ok(v)
  }

  /// get latest tag
  /// ```bash
  /// git describe --tags $(git rev-list --tags --max-count=1)
  /// ```
  /// fails when no tag is found
  pub fn latest_tag(&self) -> Result<String, String> {
    let rev_output = self.run_command(&["rev-list", "--tags", "--max-count=1"])?;
    let tag = self.run_command(&["describe", "--tags", rev_output.trim()])?;
    Ok(tag.trim().to_string())
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

  pub fn check_branch_or_tag(&self, version: &str) -> Result<bool, String> {
    match self.run_command(&["show-ref", "--verify", &format!("refs/tags/{}", version)]) {
      Ok(_) => Ok(true),
      Err(_) => {
        self.run_command(&["show-ref", "--verify", &format!("refs/heads/{}", version)])?;
        Ok(true)
      }
    }
  }

  pub fn fetch(&self) -> Result<(), String> {
    self.run_command(&["fetch", "origin", "--tags"])?;
    Ok(())
  }

  pub fn describe_tag(&self) -> Result<String, String> {
    let tag = self.run_command(&["describe", "--tags"])?.trim().to_string();
    Ok(tag)
  }

  pub fn pull(&self, branch: &str) -> Result<(), String> {
    self.run_command(&["pull", "origin", branch])?;
    Ok(())
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum GitHead {
  Branch(String),
  Tag(String),
}

impl GitHead {
  pub fn get_name(&self) -> String {
    match self {
      GitHead::Branch(s) => s.to_string(),
      GitHead::Tag(s) => s.to_string(),
    }
  }
}
