use std::fs;
use std::process::Command;

#[test]
fn check_md_uses_snapshot_path_and_rejects_legacy_entry_flag() {
  let directory = tempfile::tempdir().expect("temporary directory should create");
  let markdown = directory.path().join("example.md");
  fs::write(&markdown, "```cirru\n+ 1 2\n```\n").expect("markdown fixture should write");
  let markdown = markdown.to_str().expect("temporary path should be UTF-8");

  let valid = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .args([
      "calcit/test.cirru",
      "docs",
      "check-md",
      markdown,
      "--snapshot",
      "calcit/test.cirru",
      "--quiet",
    ])
    .output()
    .expect("calcit command should run");
  assert!(
    valid.status.success(),
    "--snapshot should select the evaluation Snapshot:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&valid.stdout),
    String::from_utf8_lossy(&valid.stderr)
  );

  let legacy = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .args(["calcit/test.cirru", "docs", "check-md", markdown, "--entry", "calcit/test.cirru"])
    .output()
    .expect("calcit command should run");
  assert!(
    !legacy.status.success(),
    "legacy --entry must not silently retain a different meaning"
  );
  let error = String::from_utf8_lossy(&legacy.stderr);
  assert!(
    error.contains("Use --snapshot <file> instead"),
    "migration hint should be actionable:\n{error}"
  );
}
