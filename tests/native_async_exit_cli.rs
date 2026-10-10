use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn bounded_output(command: &mut Command) -> Output {
  let mut child = command
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("start subprocess");
  let deadline = Instant::now() + Duration::from_secs(15);
  loop {
    if child.try_wait().expect("poll subprocess").is_some() {
      return child.wait_with_output().expect("read subprocess output");
    }
    if Instant::now() >= deadline {
      let _ = child.kill();
      let output = child.wait_with_output().expect("collect timed-out subprocess");
      panic!("subprocess failed to finish\n{}", String::from_utf8_lossy(&output.stderr));
    }
    thread::sleep(Duration::from_millis(10));
  }
}

#[test]
fn native_async_failure_reaches_exit_status_after_cleanup() {
  let workspace = tempfile::tempdir().expect("isolated fixture workspace");
  let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/native-async-exit/Cargo.toml");
  let build = Command::new(env!("CARGO"))
    .args(["build", "--offline", "--locked", "--manifest-path"])
    .arg(&manifest)
    .arg("--target-dir")
    .arg(workspace.path().join("target"))
    .output()
    .expect("build native protocol fixture");
  assert!(
    build.status.success(),
    "fixture build failed: {}",
    String::from_utf8_lossy(&build.stderr)
  );
  let library = workspace.path().join("target/debug").join(format!(
    "{}calcit_async_exit_fixture{}",
    std::env::consts::DLL_PREFIX,
    std::env::consts::DLL_SUFFIX
  ));
  let library = library.to_str().expect("UTF-8 fixture path");
  let library_literal = format!("{:?}", format!("|{library}"));
  let start = |method: &str, callback: &str| format!("&call-dylib-edn-fn {library_literal} |{method} $ {callback}");
  let idle = start("idle", "fn () &unit");
  let idle_fail = start("idle_fail", "fn () &unit");
  let failing = start("emit", "fn () (assert= |expected |wrong) &unit");
  let response_case = |method: &str| {
    format!(
      "let\n    task-ref $ ref $ assert-type (Option :none) $ :: 'Option 'FfiTask\n    task $ ffi:task $ &call-dylib-edn-fn {library_literal} |server\n      fn (raw-response)\n        let\n            response $ ffi:response raw-response\n          assert-type (response.{method}! :payload) 'Unit\n          assert= true $ try\n            do\n              response.{method}! :late\n              , false\n            fn (message)\n              assert-type message 'String\n              , true\n        .cancel! $ .unwrap $ deref task-ref\n        , &unit\n  reset! task-ref $ Option :some task"
    )
  };

  // The callbacks are ordinary Calcit. Rust owns only the native protocol,
  // bounded process execution, exit status and cancellation observations.
  let cases = [
    ("normal", start("emit", "fn () (assert= 1 1) &unit"), true, None, false),
    ("unhandled", failing.clone(), false, Some("not equal in assertion"), false),
    (
      "handled",
      start("emit", "fn () (try (assert= |expected |wrong) (fn (message) &unit)) &unit"),
      true,
      None,
      false,
    ),
    (
      "terminal-failure",
      start("fail", "fn () (println |callback-must-not-run) &unit"),
      false,
      Some("fixture-failure"),
      false,
    ),
    ("cleanup", format!("{idle}\n{failing}"), false, Some("not equal in assertion"), true),
    (
      "synchronous-failure-cleanup",
      format!("{idle}\nraise |synchronous-failure"),
      false,
      Some("synchronous-failure"),
      true,
    ),
    (
      "cleanup-failure-preserves-original",
      format!("{idle_fail}\n{failing}"),
      false,
      Some("not equal in assertion"),
      true,
    ),
    (
      "synchronous-failure-preserves-original",
      format!("{idle_fail}\nraise |synchronous-failure"),
      false,
      Some("synchronous-failure"),
      true,
    ),
    (
      "method-cancel-is-once",
      format!(
        "let\n    task $ ffi:task $ {idle}\n  assert-type (task.cancel!) 'Unit\n  assert-type (task.cancel-with! :already-closing) 'Unit"
      ),
      true,
      None,
      true,
    ),
    ("method-resolve-is-once", response_case("resolve"), true, None, true),
    ("method-reject-is-once", response_case("reject"), true, None, true),
  ];
  for (name, source, success, message, cleanup) in cases {
    let output = bounded_output(
      Command::new(env!("CARGO_BIN_EXE_calcit"))
        .current_dir(workspace.path())
        .env("NO_COLOR", "1")
        .args(["--tips-level", "none", "eval", "--", &source]),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.success(), success, "{name}: wrong exit status\n{stderr}");
    if let Some(message) = message {
      assert!(stderr.contains(message), "{name}: missing failure diagnostic\n{stderr}");
    }
    if name.contains("preserves-original") {
      assert!(
        stderr.contains("shutdown-fixture-failure"),
        "{name}: missing cleanup diagnostic\n{stderr}"
      );
    }
    let response_outcome = match name {
      "method-resolve-is-once" => Some(calcit_native_ffi::response_outcome::RESOLVE),
      "method-reject-is-once" => Some(calcit_native_ffi::response_outcome::REJECT),
      _ => None,
    };
    assert_eq!(
      stderr.matches("fixture-response-outcome=").count(),
      usize::from(response_outcome.is_some()),
      "{name}: response attempts\n{stderr}"
    );
    if let Some(outcome) = response_outcome {
      assert!(
        stderr.contains(&format!("fixture-response-outcome={outcome}")),
        "{name}: wrong response outcome\n{stderr}"
      );
    }
    assert_eq!(
      stderr.matches("fixture-idle-task-cancelled").count(),
      usize::from(cleanup),
      "{name}: cancellation\n{stderr}"
    );
    assert!(
      !String::from_utf8_lossy(&output.stdout).contains("callback-must-not-run"),
      "{name}: terminal failure invoked callback"
    );
  }
}
