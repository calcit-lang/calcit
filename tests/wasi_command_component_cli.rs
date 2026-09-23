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
fn calcit_stdio_entry_emits_runnable_wasi_03_command_component() {
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
  config.wasm_component_model_more_async_builtins(true);
  let engine = Engine::new(&config).expect("Wasmtime engine");
  let component = Component::new(&engine, &first_wasm).expect("valid command Component");
  assert_eq!(
    component
      .component_type()
      .imports(&engine)
      .map(|(name, _)| name)
      .collect::<Vec<_>>(),
    vec![
      "wasi:cli/environment@0.3.1",
      "wasi:cli/exit@0.3.1",
      "wasi:cli/types@0.3.1",
      "wasi:cli/stdout@0.3.1",
      "wasi:cli/stderr@0.3.1",
    ]
  );
  assert!(
    component
      .component_type()
      .exports(&engine)
      .any(|(name, _)| name == "wasi:cli/run@0.3.1")
  );

  if let Some(cli) = std::env::var_os("WASMTIME_CLI") {
    let result = Command::new(cli)
      .args([
        "run",
        "-S",
        "p3",
        "-W",
        "component-model-more-async-builtins=y",
        "-W",
        "component-model-async-stackful=y",
      ])
      .arg(first.path().join("program.wasm"))
      .output()
      .expect("run WASI 0.3 command");
    assert_eq!(result.status.code(), Some(0), "{}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(result.stdout, "你好 42\ndone\n".as_bytes());
    assert_eq!(result.stderr, "错误\n".as_bytes());
  }
}

#[test]
fn quit_exits_with_the_requested_wasi_03_status_code() {
  let output = tempfile::tempdir().expect("output directory");
  let fixture = "tests/fixtures/wasi-command-03-exit.cirru";
  let check = calcit(&[fixture, "wasi", "--boundary", "component", "--check-only"], output.path());
  assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));
  assert!(!output.path().join("program.wasm").exists());

  let compiled = calcit(&[fixture, "wasi", "--boundary", "component"], output.path());
  assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
  let mut config = Config::new();
  config.wasm_component_model_async(true);
  config.wasm_component_model_async_stackful(true);
  let engine = Engine::new(&config).expect("Wasmtime engine");
  let wasm = fs::read(output.path().join("program.wasm")).expect("command Component");
  Component::new(&engine, &wasm).expect("valid command Component");

  if let Some(cli) = std::env::var_os("WASMTIME_CLI") {
    let result = Command::new(cli)
      .args(["run", "-S", "p3"])
      .arg(output.path().join("program.wasm"))
      .output()
      .expect("run WASI 0.3 command with explicit exit");
    assert_eq!(result.status.code(), Some(7), "{}", String::from_utf8_lossy(&result.stderr));
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
  }
}

#[test]
fn quit_rejects_out_of_range_status_instead_of_wrapping_it() {
  let output = tempfile::tempdir().expect("output directory");
  let fixture = "tests/fixtures/wasi-command-03-exit-invalid.cirru";
  let compiled = calcit(&[fixture, "wasi", "--boundary", "component"], output.path());
  assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));

  if let Some(cli) = std::env::var_os("WASMTIME_CLI") {
    let result = Command::new(cli)
      .args(["run", "-S", "p3"])
      .arg(output.path().join("program.wasm"))
      .output()
      .expect("run WASI 0.3 command with invalid exit code");
    assert!(!result.status.success(), "out-of-range exit status must fail");
    assert_ne!(result.status.code(), Some(0), "exit code must not wrap to success");
    assert!(
      String::from_utf8_lossy(&result.stderr).contains("wasm trap"),
      "{}",
      String::from_utf8_lossy(&result.stderr)
    );
  }
}

#[test]
fn command_arguments_preserve_order_and_utf8_content() {
  let output = tempfile::tempdir().expect("output directory");
  let fixture = "tests/fixtures/wasi-command-03-get-args.cirru";
  let check = calcit(&[fixture, "wasi", "--boundary", "component", "--check-only"], output.path());
  assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));
  assert!(!output.path().join("program.wasm").exists());

  let compiled = calcit(&[fixture, "wasi", "--boundary", "component"], output.path());
  assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
  let mut config = Config::new();
  config.wasm_component_model_async(true);
  config.wasm_component_model_async_stackful(true);
  let engine = Engine::new(&config).expect("Wasmtime engine");
  let wasm = fs::read(output.path().join("program.wasm")).expect("command Component");
  Component::new(&engine, &wasm).expect("valid command Component");

  if let Some(cli) = std::env::var_os("WASMTIME_CLI") {
    for (argument, expected_status) in [("你好", 7), ("beta", 8)] {
      let result = Command::new(&cli)
        .args(["run", "-S", "p3"])
        .arg(output.path().join("program.wasm"))
        .arg(argument)
        .output()
        .expect("run WASI 0.3 command with arguments");
      assert_eq!(
        result.status.code(),
        Some(expected_status),
        "argument {argument:?}: {}",
        String::from_utf8_lossy(&result.stderr)
      );
      assert!(result.stdout.is_empty());
      assert!(result.stderr.is_empty());
    }
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
fn command_environment_preserves_option_and_utf8_content() {
  let output = tempfile::tempdir().expect("output directory");
  let fixture = "tests/fixtures/wasi-command-03-get-env.cirru";
  let check = calcit(&[fixture, "wasi", "--boundary", "component", "--check-only"], output.path());
  assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));
  assert!(!output.path().join("program.wasm").exists());

  let compiled = calcit(&[fixture, "wasi", "--boundary", "component"], output.path());
  assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
  let mut config = Config::new();
  config.wasm_component_model_async(true);
  config.wasm_component_model_async_stackful(true);
  let engine = Engine::new(&config).expect("Wasmtime engine");
  let wasm = fs::read(output.path().join("program.wasm")).expect("command Component");
  Component::new(&engine, &wasm).expect("valid command Component");

  if let Some(cli) = std::env::var_os("WASMTIME_CLI") {
    for (value, expected_status) in [(Some("你好"), 7), (Some("other"), 9), (Some(""), 9), (None, 8)] {
      let mut command = Command::new(&cli);
      command.args(["run", "-S", "p3"]);
      if let Some(value) = value {
        command.arg("--env").arg(format!("CALCIT_TEST={value}"));
      }
      let result = command
        .arg(output.path().join("program.wasm"))
        .output()
        .expect("run WASI 0.3 command with environment");
      assert_eq!(
        result.status.code(),
        Some(expected_status),
        "environment {value:?}: {}",
        String::from_utf8_lossy(&result.stderr)
      );
    }
  }
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
