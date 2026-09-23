use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use wasmtime::component::Component;
use wasmtime::{Config, Engine};

fn calcit(args: &[&str], output: &Path) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .args(args)
    .args(["--emit-path"])
    .arg(output)
    .output()
    .expect("run Calcit command")
}

#[test]
fn pure_calcit_entry_emits_runnable_wasi_03_command_component() {
  let first = tempfile::tempdir().expect("first output directory");
  let second = tempfile::tempdir().expect("second output directory");
  let check = calcit(
    &[
      "tests/fixtures/wasi-command-03.cirru",
      "wasi",
      "--boundary",
      "component",
      "--check-only",
    ],
    first.path(),
  );
  assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));
  assert!(!first.path().join("program.wasm").exists(), "check-only must not write output");
  for output in [first.path(), second.path()] {
    let result = calcit(&["tests/fixtures/wasi-command-03.cirru", "wasi", "--boundary", "component"], output);
    assert!(
      result.status.success(),
      "Calcit command failed: {}",
      String::from_utf8_lossy(&result.stderr)
    );
  }
  let first_wasm = fs::read(first.path().join("program.wasm")).expect("first Component");
  let second_wasm = fs::read(second.path().join("program.wasm")).expect("second Component");
  assert_eq!(first_wasm, second_wasm, "command generation must be deterministic");

  let mut config = Config::new();
  config.wasm_component_model_async(true);
  config.wasm_component_model_async_stackful(true);
  let engine = Engine::new(&config).expect("Wasmtime engine");
  let component = Component::new(&engine, &first_wasm).expect("valid command Component");
  assert_eq!(component.component_type().imports(&engine).count(), 0);
  assert!(
    component
      .component_type()
      .exports(&engine)
      .any(|(name, _)| name == "wasi:cli/run@0.3.0")
  );

  if let Some(cli) = std::env::var_os("WASMTIME_47_CLI") {
    let result = Command::new(cli)
      .args(["run", "-S", "p3"])
      .arg(first.path().join("program.wasm"))
      .output()
      .expect("run WASI 0.3 command");
    assert_eq!(result.status.code(), Some(0), "{}", String::from_utf8_lossy(&result.stderr));
  }
}

#[test]
fn preview1_capabilities_fail_before_writing_a_component() {
  let output = tempfile::tempdir().expect("output directory");
  let result = calcit(
    &[
      "examples/wasi-command/calcit.cirru",
      "wasi",
      "--boundary",
      "component",
      "--check-only",
    ],
    output.path(),
  );
  assert!(!result.status.success(), "Preview 1 capability must not compile as WASI 0.3");
  assert!(
    String::from_utf8_lossy(&result.stderr).contains("E_WASI_COMMAND_CAPABILITY"),
    "{}",
    String::from_utf8_lossy(&result.stderr)
  );
  assert!(!output.path().join("program.wasm").exists());
}

#[test]
fn reachable_core_wrapper_capabilities_fail_during_check_only() {
  let output = tempfile::tempdir().expect("output directory");
  let result = calcit(
    &[
      "tests/fixtures/wasi-command-03-get-args.cirru",
      "wasi",
      "--boundary",
      "component",
      "--check-only",
    ],
    output.path(),
  );
  assert!(!result.status.success(), "get-args must not compile to a trapping dependency");
  let error = String::from_utf8_lossy(&result.stderr);
  assert!(error.contains("E_WASI_COMMAND_CAPABILITY"), "{error}");
  assert!(error.contains("calcit.core/get-args"), "{error}");
  assert!(!output.path().join("program.wasm").exists());
}

#[test]
fn generic_component_exports_do_not_silently_disappear() {
  let output = tempfile::tempdir().expect("output directory");
  let result = calcit(
    &[
      "tests/fixtures/component-wasm-async-export.cirru",
      "wasi",
      "--boundary",
      "component",
      "--check-only",
    ],
    output.path(),
  );
  assert!(!result.status.success());
  assert!(
    String::from_utf8_lossy(&result.stderr).contains("E_WASI_COMMAND_EXPORT"),
    "{}",
    String::from_utf8_lossy(&result.stderr)
  );
  assert!(!output.path().join("program.wasm").exists());
}
