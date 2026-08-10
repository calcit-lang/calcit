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

  pub fn checkout(&self, version: &str) -> Result<(), String> {
    if self.run_command(&["checkout", version]).is_ok() {
      return Ok(());
    }

    // A branch fetched into an existing module clone may only exist as a
    // remote-tracking ref. `git checkout <branch>` cannot resolve that name
    // until a local branch is created. `-B` works even for clones with a
    // narrow/non-standard remote refspec, while still pointing the local
    // branch at the explicitly fetched origin ref.
    let remote_branch = format!("origin/{version}");
    self.run_command(&["checkout", "-B", version, &remote_branch]).map(|_| ())
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

  /// get unix timestamp of the commit resolved from a ref/tag/sha
  /// ```bash
  /// git rev-list -n 1 <REF>
  /// git show -s --format=%ct <COMMIT>
  /// ```
  pub fn timestamp(&self, sha: &str) -> Result<u32, String> {
    let commit = self.run_command(&["rev-list", "-n", "1", sha])?;
    let timestamp = self.run_command(&["show", "-s", "--format=%ct", commit.trim()])?;
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

  pub fn check_branch_or_tag(&self, version: &str, folder: &str) -> Result<bool, String> {
    let refs = [
      format!("refs/tags/{version}"),
      format!("refs/heads/{version}"),
      format!("refs/remotes/origin/{version}"),
    ];
    for r in &refs {
      if self.run_command(&["show-ref", "--verify", r]).is_ok() {
        return Ok(true);
      }
    }
    Err(format!("failed to check branch or tag `{version}` in `{folder}`"))
  }

  pub fn fetch(&self) -> Result<(), String> {
    // Module versions may be development branches, not only release tags.
    // Fetch remote branch refs explicitly: existing module clones can have a
    // narrow fetch refspec, in which case `git fetch --tags` leaves a newly
    // pushed branch invisible to `show-ref` and checkout.
    self.run_command(&["fetch", "--prune", "origin", "--tags", "+refs/heads/*:refs/remotes/origin/*"])?;
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

  /// Return paths changed from HEAD, including untracked files.
  pub fn status_porcelain(&self) -> Result<Vec<String>, String> {
    let output = self.run_command(&["status", "--porcelain"])?;
    Ok(output.lines().map(str::to_owned).collect())
  }

  /// Discard tracked changes in the working tree and index.
  pub fn reset_hard(&self) -> Result<(), String> {
    self.run_command(&["reset", "--hard", "HEAD"]).map(|_| ())
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
