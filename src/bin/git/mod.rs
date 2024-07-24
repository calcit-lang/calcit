use std::path::PathBuf;

pub fn git_checkout(dir: &PathBuf, version: &str) -> Result<(), String> {
  let output = std::process::Command::new("git")
    .current_dir(dir)
    .arg("checkout")
    .arg(version)
    .output()
    .map_err(|e| e.to_string())?;
  if !output.status.success() {
    let err = String::from_utf8(output.stderr).unwrap_or_else(|_| "Failed to decode stderr".to_string());
    Err(format!("{}\nfailed to checkout {} {}", err.trim(), dir.to_str().unwrap(), version))
  } else {
    Ok(())
  }
}

pub fn git_clone(dir: &PathBuf, url: &str, version: &str, shallow: bool) -> Result<(), String> {
  let output = if shallow {
    std::process::Command::new("git")
      .current_dir(dir)
      .arg("clone")
      .arg("--branch")
      .arg(version)
      .arg("--depth")
      .arg("1")
      .arg(url)
      .output()
      .map_err(|e| e.to_string())?
  } else {
    std::process::Command::new("git")
      .current_dir(dir)
      .arg("clone")
      .arg("--branch")
      .arg(version)
      .arg(url)
      .output()
      .map_err(|e| e.to_string())?
  };
  if !output.status.success() {
    let err = String::from_utf8(output.stderr).expect("stderr");
    Err(format!("{}\nfailed to clone {} {}", err.trim(), url, version))
  } else {
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

pub fn git_current_head(dir: &PathBuf) -> Result<GitHead, String> {
  let output = std::process::Command::new("git")
    .current_dir(dir)
    .arg("branch")
    .arg("--show-current")
    .output()
    .map_err(|e| e.to_string())?;
  if !output.status.success() {
    let err = String::from_utf8(output.stderr).expect("stderr");
    Err(format!("{}\nfailed to get current head of {}", err.trim(), dir.to_str().unwrap()))
  } else {
    let mut branch = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    branch = branch.trim().to_string();

    if branch.is_empty() {
      // probably a tag
      Ok(GitHead::Tag(git_describe_tag(dir)?))
    } else {
      Ok(GitHead::Branch(branch))
    }
  }
}

/// get unix timestamp of a commit
/// ```bash
/// git show -s --format=%ct <SHA>
/// ```
pub fn git_timestamp(dir: &PathBuf, sha: &str) -> Result<u32, String> {
  let output = std::process::Command::new("git")
    .current_dir(dir)
    .arg("show")
    .arg("-s")
    .arg("--format=%ct")
    .arg(sha)
    .output()
    .map_err(|e| e.to_string())?;
  if !output.status.success() {
    let err = String::from_utf8(output.stderr).expect("stderr");
    Err(format!("{}\nfailed to get timestamp of {}", err.trim(), sha))
  } else {
    let timestamp = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    let v = timestamp.trim().parse::<u32>().map_err(|e| e.to_string())?;
    Ok(v)
  }
}

/// get latest tag
/// ```bash
/// git describe --tags $(git rev-list --tags --max-count=1)
/// ```
/// fails when no tag is found
pub fn git_latest_tag(dir: &PathBuf) -> Result<String, String> {
  let pre_output = std::process::Command::new("git")
    .current_dir(dir)
    .arg("rev-list")
    .arg("--tags")
    .arg("--max-count=1")
    .output()
    .map_err(|e| e.to_string())?;
  if !pre_output.status.success() {
    let err = String::from_utf8(pre_output.stderr).expect("stderr");
    return Err(format!("{}\nfailed to get latest tag of {}", err.trim(), dir.to_str().unwrap()));
  }
  let pre_output = String::from_utf8(pre_output.stdout).map_err(|e| e.to_string())?;
  let output = std::process::Command::new("git")
    .current_dir(dir)
    .arg("describe")
    .arg("--tags")
    .arg(pre_output.trim())
    .output()
    .map_err(|e| e.to_string())?;
  if !output.status.success() {
    let err = String::from_utf8(output.stderr).expect("stderr");
    Err(format!("{}\nfailed to get latest tag of {}", err.trim(), dir.to_str().unwrap()))
  } else {
    let mut tag = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    tag = tag.trim().to_string();
    Ok(tag)
  }
}

/// get SHA of a tag or ref
/// ```bash
/// git rev-parse <REF>
/// ```
#[allow(dead_code)]
pub fn git_rev_parse(dir: &PathBuf, ref_name: &str) -> Result<String, String> {
  let output = std::process::Command::new("git")
    .current_dir(dir)
    .arg("rev-parse")
    .arg(ref_name)
    .output()
    .map_err(|e| e.to_string())?;
  if !output.status.success() {
    let err = String::from_utf8(output.stderr).expect("stderr");
    Err(format!("{}\nfailed to get SHA of {}", err.trim(), ref_name))
  } else {
    let sha = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    Ok(sha.trim().to_string())
  }
}

pub fn git_check_branch_or_tag(dir: &PathBuf, version: &str) -> Result<bool, String> {
  let output = std::process::Command::new("git")
    .current_dir(dir)
    .arg("show-ref")
    .arg("--verify")
    .arg(&format!("refs/tags/{}", version))
    .output()
    .map_err(|e| e.to_string())?;
  if !output.status.success() {
    let output = std::process::Command::new("git")
      .current_dir(dir)
      .arg("show-ref")
      .arg("--verify")
      .arg(&format!("refs/heads/{}", version))
      .output()
      .map_err(|e| e.to_string())?;
    if !output.status.success() {
      let err = String::from_utf8(output.stderr).expect("stderr");
      Err(format!("{}\nfailed to get current head of {}", err.trim(), dir.to_str().unwrap()))
    } else {
      Ok(true)
    }
  } else {
    Ok(true)
  }
}

pub fn git_fetch(dir: &PathBuf) -> Result<(), String> {
  let output = std::process::Command::new("git")
    .current_dir(dir)
    .arg("fetch")
    .arg("origin")
    .arg("--tags")
    .output()
    .map_err(|e| e.to_string())?;
  if !output.status.success() {
    let err = String::from_utf8(output.stderr).expect("stderr");
    Err(format!("{}\nfailed to fetch {}", err.trim(), dir.to_str().unwrap()))
  } else {
    Ok(())
  }
}

pub fn git_describe_tag(dir: &PathBuf) -> Result<String, String> {
  let output = std::process::Command::new("git")
    .current_dir(dir)
    .arg("describe")
    .arg("--tags")
    .output()
    .map_err(|e| e.to_string())?;
  if !output.status.success() {
    let err = String::from_utf8(output.stderr).expect("stderr");
    Err(format!("{}\nfailed to get current tag of {}", err.trim(), dir.to_str().unwrap()))
  } else {
    let mut tag = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    tag = tag.trim().to_string();
    Ok(tag)
  }
}

pub fn git_pull(dir: &PathBuf, branch: &str) -> Result<(), String> {
  let output = std::process::Command::new("git")
    .current_dir(dir)
    .arg("pull")
    .arg("origin")
    .arg(branch)
    .output()
    .map_err(|e| e.to_string())?;
  if !output.status.success() {
    let err = String::from_utf8(output.stderr).expect("stderr");
    Err(format!("{}\nfailed to pull {} {}", err.trim(), dir.to_str().unwrap(), branch))
  } else {
    Ok(())
  }
}
