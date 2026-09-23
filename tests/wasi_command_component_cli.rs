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
#[cfg(unix)]
fn calcit_fs_path_read_text_runs_with_real_wasi_03_preopen() {
  let Some(cli) = std::env::var_os("WASMTIME_CLI") else {
    return;
  };
  let host = tempfile::tempdir().expect("preopen directory");
  fs::write(host.path().join("valid.txt"), "你好").expect("valid UTF-8 file");
  fs::write(host.path().join("invalid.txt"), [0xff, 0xfe]).expect("invalid UTF-8 file");
  fs::write(host.path().join("oversized.txt"), vec![b'x'; 4 * 1024 * 1024 + 1]).expect("oversized file");
  fs::write(host.path().join("limit.txt"), vec![b'a'; 4 * 1024 * 1024]).expect("boundary-size input");
  fs::write(host.path().join("written.txt"), "stale content").expect("preexisting output file");
  fs::write(host.path().join("oversized-output.txt"), "sentinel").expect("preexisting oversized target");
  let outside = tempfile::tempdir().expect("outside directory");
  fs::write(outside.path().join("secret.txt"), "not authorized").expect("outside file");
  std::os::unix::fs::symlink(outside.path(), host.path().join("escape")).expect("escape symlink");
  let output = tempfile::tempdir().expect("Component output directory");
  let compiled = calcit(
    &[
      "--init-fn",
      "app.main/main-file!",
      "tests/fixtures/wasi-command-03.cirru",
      "wasi",
      "--boundary",
      "component",
    ],
    output.path(),
  );
  assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
  let mut config = Config::new();
  config.wasm_component_model_async(true);
  config.wasm_component_model_async_stackful(true);
  config.wasm_component_model_more_async_builtins(true);
  let engine = Engine::new(&config).expect("Wasmtime engine");
  let bytes = fs::read(output.path().join("program.wasm")).expect("file-reading Component");
  let component = Component::new(&engine, &bytes).expect("pinned filesystem imports");
  let component_type = component.component_type();
  let imports = component_type.imports(&engine).map(|(name, _)| name).collect::<Vec<_>>();
  assert!(imports.contains(&"wasi:filesystem/preopens@0.3.1"));
  assert!(imports.contains(&"wasi:filesystem/types@0.3.1"));

  let result = Command::new(&cli)
    .args([
      "run",
      "-S",
      "p3",
      "-W",
      "component-model-async-stackful=y",
      "-W",
      "component-model-more-async-builtins=y",
      "--dir",
    ])
    .arg(format!("{}::/workspace", host.path().display()))
    .arg(output.path().join("program.wasm"))
    .output()
    .expect("run file-reading Component");
  assert_eq!(result.status.code(), Some(0), "{}", String::from_utf8_lossy(&result.stderr));
  assert_eq!(fs::read_to_string(host.path().join("written.txt")).expect("written file"), "你好");
  assert_eq!(
    fs::read(host.path().join("limit-output.txt")).expect("boundary-size output"),
    vec![b'a'; 4 * 1024 * 1024]
  );
  assert_eq!(
    fs::read_to_string(host.path().join("oversized-output.txt")).expect("unchanged oversized target"),
    "sentinel"
  );
  assert!(!outside.path().join("denied.txt").exists(), "preopen escape must not write outside");

  let overflow = tempfile::tempdir().expect("oversized write output");
  let compiled = calcit(
    &[
      "--init-fn",
      "app.main/main-overflow!",
      "tests/fixtures/wasi-command-03.cirru",
      "wasi",
      "--boundary",
      "component",
    ],
    overflow.path(),
  );
  assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
  let rejected = Command::new(&cli)
    .args([
      "run",
      "-S",
      "p3",
      "-W",
      "component-model-async-stackful=y",
      "-W",
      "component-model-more-async-builtins=y",
      "--dir",
    ])
    .arg(format!("{}::/workspace", host.path().display()))
    .arg(overflow.path().join("program.wasm"))
    .output()
    .expect("run oversized-write Component");
  assert_eq!(rejected.status.code(), Some(0), "{}", String::from_utf8_lossy(&rejected.stderr));
  assert_eq!(
    fs::read_to_string(host.path().join("oversized-output.txt")).expect("unchanged oversized target"),
    "sentinel"
  );
}

#[test]
fn manifest_result_branches_retain_nominal_payload_type() {
  let result = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .args([
      "examples/wasi-command/calcit.cirru",
      "query",
      "type-at",
      "app.main/transform-manifest",
      "--path",
      "@3",
      "--format",
      "json",
    ])
    .output()
    .expect("query Manifest type evidence");
  assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
  let report: serde_json::Value = serde_json::from_slice(&result.stdout).expect("JSON type-at envelope");
  assert_eq!(report["data"]["inferred_type"], ":: 'Result 'Manifest 'String");
  assert_eq!(report["data"]["confidence"], "exact");
  assert_eq!(report["data"]["dynamic_intent"], serde_json::Value::Null);
}

#[test]
fn effectful_result_method_keeps_nominal_callback_and_static_lowering() {
  let result = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .args([
      "examples/wasi-command/calcit.cirru",
      "query",
      "type-at",
      "app.main/method-eval-main!",
      "--path",
      "@3.1.0.1",
      "--format",
      "json",
    ])
    .output()
    .expect("query effectful Result method evidence");
  assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
  let report: serde_json::Value = serde_json::from_slice(&result.stdout).expect("JSON type-at envelope");
  assert_eq!(report["data"]["inferred_type"], ":: 'calcit.core/Result 'String 'String");
  assert_eq!(report["data"]["confidence"], "exact");
  assert_eq!(report["data"]["lowering"]["kind"], "static-method-call");
  assert_eq!(report["data"]["bindings"][0]["type"], "'app.main/Manifest");
}

#[test]
fn manifest_result_payload_mismatch_fails_strict_preprocessing() {
  let fixture = tempfile::tempdir().expect("temporary Snapshot directory");
  let snapshot = fixture.path().join("calcit.cirru");
  fs::copy("examples/wasi-command/calcit.cirru", &snapshot).expect("copy business Snapshot");
  let edit = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .arg(&snapshot)
    .args([
      "tree",
      "replace",
      "app.main/transform-manifest",
      "--path",
      "@3.2.3",
      "--code",
      "quote $ %ok |wrong-payload",
    ])
    .output()
    .expect("edit temporary Snapshot through Calcit CLI");
  assert!(edit.status.success(), "{}", String::from_utf8_lossy(&edit.stderr));
  let check = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .args(["--init-fn", "app.main/manifest-main!"])
    .arg(&snapshot)
    .arg("--check-only")
    .output()
    .expect("check mismatched Result payload");
  assert!(
    !check.status.success(),
    "a String payload must not satisfy Result<Manifest, String>"
  );
  let diagnostic = String::from_utf8_lossy(&check.stderr);
  assert!(
    diagnostic.contains("W_FN_RETURN_TYPE_MISMATCH") && diagnostic.contains("Manifest") && diagnostic.contains(":string"),
    "{diagnostic}"
  );
}

#[test]
fn command_without_reachable_file_effect_omits_filesystem_imports() {
  let output = tempfile::tempdir().expect("output directory");
  let compiled = calcit(
    &[
      "--init-fn",
      "app.main/reload!",
      "examples/wasi-command/calcit.cirru",
      "wasi",
      "--boundary",
      "component",
    ],
    output.path(),
  );
  assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));

  let mut config = Config::new();
  config.wasm_component_model_async(true);
  config.wasm_component_model_async_stackful(true);
  config.wasm_component_model_more_async_builtins(true);
  let engine = Engine::new(&config).expect("Wasmtime engine");
  let bytes = fs::read(output.path().join("program.wasm")).expect("command Component");
  let component = Component::new(&engine, &bytes).expect("valid command Component");
  let component_type = component.component_type();
  let imports = component_type.imports(&engine).map(|(name, _)| name).collect::<Vec<_>>();
  assert!(!imports.contains(&"wasi:filesystem/preopens@0.3.1"));
  assert!(!imports.contains(&"wasi:filesystem/types@0.3.1"));

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
      .arg(output.path().join("program.wasm"))
      .output()
      .expect("run WASI 0.3 command without filesystem imports");
    assert_eq!(result.status.code(), Some(0), "{}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(result.stdout, b"Reloaded\n");
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
fn component_file_capabilities_check_only_without_writing_an_artifact() {
  let output = tempfile::tempdir().expect("output directory");
  let supported = calcit(
    &[
      "examples/wasi-command/calcit.cirru",
      "wasi",
      "--boundary",
      "component",
      "--check-only",
    ],
    output.path(),
  );
  assert!(supported.status.success(), "{}", String::from_utf8_lossy(&supported.stderr));
  assert!(!output.path().join("program.wasm").exists());

  let unsupported = calcit(
    &[
      "--init-fn",
      "app.main/main-unsupported!",
      "tests/fixtures/wasi-command-03.cirru",
      "wasi",
      "--boundary",
      "component",
      "--check-only",
    ],
    output.path(),
  );
  assert!(!unsupported.status.success(), "unimplemented directory access must fail");
  assert!(
    String::from_utf8_lossy(&unsupported.stderr).contains("E_WASI_COMMAND_CAPABILITY"),
    "{}",
    String::from_utf8_lossy(&unsupported.stderr)
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
