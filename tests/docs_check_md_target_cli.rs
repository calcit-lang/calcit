use std::fs;
use std::process::Command;

#[test]
fn check_md_uses_snapshot_path_and_rejects_legacy_entry_flag() {
  let directory = tempfile::tempdir().expect("temporary directory should create");
  let markdown = directory.path().join("example.md");
  fs::write(&markdown, "```cirru\n+ 1 2\n```\n").expect("markdown fixture should write");
  let markdown = markdown.to_str().expect("temporary path should be UTF-8");

  for snapshot_args in [vec![], vec!["--snapshot", "missing-context.cirru"]] {
    let missing = Command::new(env!("CARGO_BIN_EXE_calcit"))
      .current_dir(directory.path())
      .args(["docs", "check-md", markdown, "--failures-only"])
      .args(&snapshot_args)
      .output()
      .expect("calcit command should run");
    assert!(!missing.status.success(), "a missing evaluation Snapshot must fail");
    let error = String::from_utf8_lossy(&missing.stderr);
    let snapshot = snapshot_args.last().copied().unwrap_or("calcit.cirru");
    assert!(error.contains(snapshot), "missing Snapshot path should be reported:\n{error}");
    assert!(
      error.contains("Use --snapshot <file>"),
      "current option should be actionable:\n{error}"
    );
    assert!(!error.contains("Use --entry"), "retired option must not be recommended:\n{error}");
  }

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

#[test]
fn check_md_edn_blocks_do_not_require_snapshot() {
  let directory = tempfile::tempdir().expect("temporary directory should create");
  let markdown = directory.path().join("data.md");
  fs::write(&markdown, "```cirru.edn\n{} (:name |Ada)\n```\n").expect("markdown fixture should write");

  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .current_dir(directory.path())
    .args(["docs", "check-md"])
    .arg(&markdown)
    .args(["--failures-only"])
    .output()
    .expect("calcit command should run");
  assert!(
    output.status.success(),
    "EDN-only documentation must not require a Snapshot:\n{}",
    String::from_utf8_lossy(&output.stderr)
  );
}
