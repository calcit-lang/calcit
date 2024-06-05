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
