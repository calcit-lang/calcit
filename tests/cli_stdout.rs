use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

fn documentation_fixture() -> tempfile::TempDir {
  let fixture = tempfile::tempdir().expect("create isolated CLI home");
  let docs = fixture.path().join(".config/calcit/docs");
  std::fs::create_dir_all(&docs).expect("create fixture documentation directory");
  std::fs::write(docs.join("query.md"), "# Query Fn\n\nA local Fn documentation fixture.\n").expect("write fixture documentation");
  fixture
}

fn calcit(fixture_home: &Path) -> Command {
  let mut command = Command::new(env!("CARGO_BIN_EXE_calcit"));
  command.env("HOME", fixture_home).env("NO_COLOR", "1").env("RUST_BACKTRACE", "1");
  command
}

#[test]
fn read_commands_exit_successfully_when_stdout_reader_is_closed() {
  let fixture = documentation_fixture();
  for args in [
    vec!["docs", "search", "Fn"],
    vec!["docs", "agents", "--full"],
    vec!["docs", "read", "query.md", "--full"],
    vec!["calcit/test.cirru", "query", "defs", "calcit.core"],
    vec!["calcit/test.cirru", "query", "def", "app.main/main!"],
    vec!["calcit/test.cirru", "query", "def", "app.main/main!", "--format", "edn"],
    vec!["calcit/test.cirru", "query", "def", "app.main/main!", "--format", "json"],
  ] {
    let complete = calcit(fixture.path()).args(&args).output().expect("read complete output");
    assert!(complete.status.success(), "{args:?}: {}", String::from_utf8_lossy(&complete.stderr));
    assert!(
      !complete.stdout.is_empty(),
      "{args:?} must reach stdout before testing a closed pipe"
    );
    // Close the reader before spawning to avoid relying on pipe buffer size
    // or scheduler timing to reproduce EPIPE.
    let (reader, writer) = std::io::pipe().expect("create stdout pipe");
    drop(reader);
    let output = calcit(fixture.path())
      .args(&args)
      .stdout(writer)
      .output()
      .expect("run read command");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{args:?}: {stderr}");
    assert!(!stderr.contains("panicked") && !stderr.contains("stack backtrace"), "{stderr}");
  }
}

#[test]
fn documentation_can_be_read_as_a_prefix_without_panicking() {
  let fixture = documentation_fixture();
  let mut child = calcit(fixture.path())
    .args(["docs", "agents", "--full"])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("spawn docs command");
  let mut reader = BufReader::new(child.stdout.take().expect("piped stdout"));
  let mut first_line = String::new();
  assert!(reader.read_line(&mut first_line).expect("read first line") > 0);
  drop(reader);
  let output = child.wait_with_output().expect("wait for docs command");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(output.status.success(), "{stderr}");
  assert!(!stderr.contains("panicked"), "{stderr}");
}

#[cfg(target_os = "linux")]
#[test]
fn non_broken_pipe_stdout_errors_remain_failures() {
  let fixture = documentation_fixture();
  let full = std::fs::OpenOptions::new()
    .write(true)
    .open("/dev/full")
    .expect("open failing stdout sink");
  let output = calcit(fixture.path())
    .args(["docs", "search", "Fn"])
    .stdout(full)
    .output()
    .expect("run with unwritable stdout");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert_eq!(output.status.code(), Some(1), "{stderr}");
  assert!(stderr.contains("failed writing CLI stdout"), "{stderr}");
  assert!(!stderr.contains("panicked"), "{stderr}");
}
