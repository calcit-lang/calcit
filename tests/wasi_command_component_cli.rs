#![cfg(feature = "wasm")]

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
    vec!["wasi:cli/types@0.3.1", "wasi:cli/stdout@0.3.1", "wasi:cli/stderr@0.3.1",]
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
  let listing = host.path().join("listing");
  fs::create_dir(&listing).expect("listing directory");
  fs::write(listing.join("子.txt"), "child").expect("utf8 child file");
  fs::write(listing.join("a.txt"), "alpha").expect("ascii child file");
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

  for (entry, marker) in [
    ("app.main/main-read-loop!", "Read-loop-ok"),
    ("app.main/main-growth-loop!", "Growth-ok"),
    ("app.main/main-capabilities!", "Capabilities-ok"),
  ] {
    let output = tempfile::tempdir().expect("loop Component output directory");
    let compiled = calcit(
      &[
        "--init-fn",
        entry,
        "tests/fixtures/wasi-command-03.cirru",
        "wasi",
        "--boundary",
        "component",
      ],
      output.path(),
    );
    assert!(compiled.status.success(), "{entry}: {}", String::from_utf8_lossy(&compiled.stderr));
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
      .expect("run loop Component");
    assert_eq!(
      result.status.code(),
      Some(0),
      "{entry}: {}",
      String::from_utf8_lossy(&result.stderr)
    );
    assert!(
      String::from_utf8_lossy(&result.stdout).contains(marker),
      "{entry}: expected {marker}"
    );
  }

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
  assert_eq!(report["data"]["inferred_type"], "calcit.core/Result app.main/Manifest String");
  assert_eq!(report["data"]["confidence"], "exact");
  assert_eq!(report["data"]["dynamic_intent"], serde_json::Value::Null);
}

#[test]
fn direct_get_env_call_retains_source_type_evidence() {
  for (snapshot, target, path) in [
    ("calcit/test-wasi-command.cirru", "app.main/main!", "@6.2.0"),
    ("examples/wasi-command/calcit.cirru", "app.main/main!", "@3.2.3.1.2.1.0"),
  ] {
    let result = Command::new(env!("CARGO_BIN_EXE_calcit"))
      .args([snapshot, "query", "type-at", target, "--path", path, "--format", "json"])
      .output()
      .expect("query direct get-env type evidence");
    assert!(result.status.success(), "{}: {}", snapshot, String::from_utf8_lossy(&result.stderr));
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).expect("JSON type-at envelope");
    assert_eq!(report["data"]["inferred_type"], "calcit.core/Option String", "{snapshot}: {report}");
    assert_eq!(report["data"]["confidence"], "exact", "{snapshot}: {report}");
    assert_eq!(report["data"]["dynamic_intent"], serde_json::Value::Null);
  }
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
  assert_eq!(report["data"]["inferred_type"], "calcit.core/Result String String");
  assert_eq!(report["data"]["confidence"], "exact");
  assert_eq!(report["data"]["lowering"]["kind"], "static-method-call");
  assert_eq!(report["data"]["bindings"][0]["type"], "app.main/Manifest");
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
      "quote $ Result :ok |wrong-payload",
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
    diagnostic.contains("E_FN_RETURN_TYPE_MISMATCH") && diagnostic.contains("Manifest") && diagnostic.contains(":string"),
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
  let component = Component::new(&engine, &wasm).expect("valid command Component");
  let component_type = component.component_type();
  let imports = component_type.imports(&engine).map(|(name, _)| name).collect::<Vec<_>>();
  assert!(imports.contains(&"wasi:cli/exit@0.3.1"), "imports: {imports:?}");
  assert!(!imports.contains(&"wasi:cli/environment@0.3.1"), "imports: {imports:?}");

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
  let component = Component::new(&engine, &wasm).expect("valid command Component");
  let component_type = component.component_type();
  let imports = component_type.imports(&engine).map(|(name, _)| name).collect::<Vec<_>>();
  assert!(imports.contains(&"wasi:cli/environment@0.3.1"), "imports: {imports:?}");
  assert!(imports.contains(&"wasi:cli/exit@0.3.1"), "imports: {imports:?}");

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

  let dynamic_valid = calcit(
    &[
      "--init-fn",
      "app.main/main-unknown-alias!",
      "tests/fixtures/wasi-command-03.cirru",
      "wasi",
      "--boundary",
      "component",
      "--check-only",
    ],
    output.path(),
  );
  assert!(
    dynamic_valid.status.success(),
    "app.main/main-unknown-alias!: {}",
    String::from_utf8_lossy(&dynamic_valid.stderr)
  );
  assert!(!output.path().join("program.wasm").exists());

  for (entry, diagnostic) in [
    ("app.main/main-unsupported!", "E_WASI_COMMAND_DEPENDENCY"),
    ("app.main/main-unsupported-alias!", "E_WASI_COMMAND_DEPENDENCY"),
    ("app.main/main-dynamic-unsupported!", "E_WASI_COMMAND_DEPENDENCY"),
  ] {
    let checked = calcit(
      &["--init-fn", entry, "tests/fixtures/wasi-command-03.cirru", "--check-only"],
      output.path(),
    );
    assert!(checked.status.success(), "{entry}: {}", String::from_utf8_lossy(&checked.stderr));
    let unsupported = calcit(
      &[
        "--init-fn",
        entry,
        "tests/fixtures/wasi-command-03.cirru",
        "wasi",
        "--boundary",
        "component",
        "--check-only",
      ],
      output.path(),
    );
    assert!(
      !unsupported.status.success(),
      "{entry}: unsupported capability or unknown callee must fail"
    );
    assert!(
      String::from_utf8_lossy(&unsupported.stderr).contains(diagnostic),
      "{entry}: {}",
      String::from_utf8_lossy(&unsupported.stderr)
    );
    assert!(!output.path().join("program.wasm").exists());
  }
}

#[test]
fn directory_compatibility_reports_recursive_capability_on_wasi() {
  let directory = tempfile::tempdir().expect("temporary directory");
  let snapshot = directory.path().join("directory.cirru");
  fs::copy("calcit/add.cirru", &snapshot).expect("copy minimal snapshot");
  let snapshot_name = snapshot.to_str().expect("UTF-8 snapshot path");
  let edit = |code: &str| {
    let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
      .args([
        "--tips-level",
        "none",
        snapshot_name,
        "edit",
        "def",
        "app.main/main!",
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        code,
      ])
      .output()
      .expect("edit test definition");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  };
  edit(
    "quote $ defn main! () (match (try-read-dir |tests/fixtures/read-dir (Some true)) ((:err message) (println message)) ((:ok _) (raise |unexpected-recursive-success)))",
  );
  let schema = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .args([
      "--tips-level",
      "none",
      snapshot_name,
      "edit",
      "schema",
      "app.main/main!",
      "--input-format",
      "cirru",
      "--code",
      "quote $ Fn (-> Unit) (:features :io)",
    ])
    .output()
    .expect("edit test schema");
  assert!(schema.status.success(), "{}", String::from_utf8_lossy(&schema.stderr));
  let emitted = calcit(&[snapshot_name, "wasi", "--boundary", "component"], directory.path());
  assert!(emitted.status.success(), "{}", String::from_utf8_lossy(&emitted.stderr));
  if let Some(cli) = std::env::var_os("WASMTIME_CLI") {
    let run = || {
      Command::new(&cli)
        .args([
          "run",
          "-S",
          "p3",
          "-W",
          "component-model-async-stackful=y",
          "-W",
          "component-model-more-async-builtins=y",
          "--dir",
          "tests/fixtures::tests/fixtures",
        ])
        .arg(directory.path().join("program.wasm"))
        .output()
        .expect("run directory compatibility command")
    };
    let emit = || {
      let output = calcit(&[snapshot_name, "wasi", "--boundary", "component"], directory.path());
      assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    };
    let result = run();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert!(String::from_utf8_lossy(&result.stdout).contains("E_WASM_CAPABILITY"));

    for code in [
      "quote $ defn main! () (do (&fs-read-dir (do (raise |first-prototype) Result) FsPath |tests/fixtures/read-dir |error false) (println |unexpected-success))",
      "quote $ defn main! () (do (&fs-read-dir Result (do (raise |second-prototype) FsPath) |tests/fixtures/read-dir/does-not-exist |error false) (println |unexpected-success))",
    ] {
      edit(code);
      emit();
      let result = run();
      assert!(!result.status.success(), "a failing prototype expression must trap");
      assert!(!String::from_utf8_lossy(&result.stdout).contains("unexpected-success"));
    }

    edit("quote $ defn main! () (do (&fs-read-dir Result (do (println |type-evaluated) FsPath) |tests/fixtures |error false) &unit)");
    emit();
    let result = run();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(String::from_utf8_lossy(&result.stdout).matches("type-evaluated").count(), 1);
  }

  edit("quote $ defn main! () (do (.walk-dir (fs:path |tests/fixtures/read-dir)) &unit)");
  let rejected = calcit(
    &[snapshot_name, "wasi", "--boundary", "component", "--check-only"],
    directory.path(),
  );
  assert!(!rejected.status.success(), "statically recursive walk-dir must be rejected");
  assert!(String::from_utf8_lossy(&rejected.stderr).contains("E_WASM_CAPABILITY"));
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
  let component = Component::new(&engine, &wasm).expect("valid command Component");
  let component_type = component.component_type();
  let imports = component_type.imports(&engine).map(|(name, _)| name).collect::<Vec<_>>();
  assert!(imports.contains(&"wasi:cli/environment@0.3.1"), "imports: {imports:?}");
  assert!(imports.contains(&"wasi:cli/exit@0.3.1"), "imports: {imports:?}");

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

#[test]
fn wasi_and_wasm_retire_try_with_structured_diagnostic() {
  let directory = tempfile::tempdir().expect("temp directory");
  let output = directory.path().join("emit");
  fs::create_dir_all(&output).expect("emit directory");

  let edit = |snapshot: &Path, target: &str, code: &str, schema: &str| {
    let mut def_cmd = Command::new(env!("CARGO_BIN_EXE_calcit"));
    def_cmd.env("NO_COLOR", "1").args([
      "--tips-level",
      "none",
      snapshot.to_str().expect("utf8 snapshot"),
      "edit",
      "def",
      target,
      "--overwrite",
      "--input-format",
      "cirru",
      "--code",
      code,
    ]);
    let def_res = def_cmd.output().expect("edit def");
    assert!(def_res.status.success(), "{}", String::from_utf8_lossy(&def_res.stderr));

    let mut schema_cmd = Command::new(env!("CARGO_BIN_EXE_calcit"));
    schema_cmd.env("NO_COLOR", "1").args([
      "--tips-level",
      "none",
      snapshot.to_str().expect("utf8 snapshot"),
      "edit",
      "schema",
      target,
      "--input-format",
      "cirru",
      "--code",
      schema,
    ]);
    let schema_res = schema_cmd.output().expect("edit schema");
    assert!(schema_res.status.success(), "{}", String::from_utf8_lossy(&schema_res.stderr));
  };

  let direct_wasi = directory.path().join("direct-wasi.cirru");
  fs::copy("tests/fixtures/wasi-command-03.cirru", &direct_wasi).expect("copy wasi fixture");
  edit(
    &direct_wasi,
    "app.main/main!",
    "quote $ defn main! () (try (raise |boom) (fn (_err) &unit))",
    "quote $ Fn (-> Unit) (:features :io)",
  );
  let wasi_direct_out = calcit(
    &[
      direct_wasi.to_str().expect("utf8 path"),
      "wasi",
      "--boundary",
      "component",
      "--check-only",
    ],
    &output,
  );
  assert!(!wasi_direct_out.status.success(), "WASI must reject direct `try`");
  let wasi_direct_stderr = String::from_utf8_lossy(&wasi_direct_out.stderr);
  assert!(wasi_direct_stderr.contains("E_WASM_UNSUPPORTED_TRY"), "{wasi_direct_stderr}");
  assert!(wasi_direct_stderr.contains("Result"), "{wasi_direct_stderr}");
  assert!(!output.join("program.wasm").exists());

  let helper_wasi = directory.path().join("helper-wasi.cirru");
  fs::copy("tests/fixtures/wasi-command-03.cirru", &helper_wasi).expect("copy wasi fixture");
  for extra_args in [
    vec!["edit", "add-ns", "app.dep"],
    vec![
      "edit",
      "add-import",
      "app.main",
      "--input-format",
      "cirru",
      "--code",
      "quote $ app.dep :refer $ legacy-recover",
    ],
  ] {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_calcit"));
    cmd
      .env("NO_COLOR", "1")
      .args(["--tips-level", "none", helper_wasi.to_str().expect("utf8 snapshot")])
      .args(&extra_args);
    let res = cmd.output().expect("setup dep namespace");
    assert!(res.status.success(), "{}", String::from_utf8_lossy(&res.stderr));
  }
  edit(
    &helper_wasi,
    "app.dep/legacy-recover",
    "quote $ defn legacy-recover () (try (raise |boom) (fn (err) (Result :err err)))",
    "quote $ Fn (-> (Result Unit String))",
  );
  edit(
    &helper_wasi,
    "app.main/main!",
    "quote $ defn main! () (let ((_ignored (legacy-recover))) &unit)",
    "quote $ Fn (-> Unit) (:features :io)",
  );
  let wasi_helper_out = calcit(
    &[
      helper_wasi.to_str().expect("utf8 path"),
      "wasi",
      "--boundary",
      "component",
      "--check-only",
    ],
    &output,
  );
  assert!(!wasi_helper_out.status.success(), "WASI must reject reachable `try` helper");
  let wasi_helper_stderr = String::from_utf8_lossy(&wasi_helper_out.stderr);
  assert!(wasi_helper_stderr.contains("E_WASI_COMMAND_DEPENDENCY"), "{wasi_helper_stderr}");
  assert!(wasi_helper_stderr.contains("E_WASM_UNSUPPORTED_TRY"), "{wasi_helper_stderr}");
  assert!(!output.join("program.wasm").exists());

  let core_wasm = directory.path().join("core-wasm.cirru");
  fs::copy("calcit/add.cirru", &core_wasm).expect("copy minimal fixture");
  edit(
    &core_wasm,
    "app.main/main!",
    "quote $ defn main! () (try (raise |boom) (fn (_err) 1))",
    "quote $ Fn (-> Number)",
  );
  let wasm_out = calcit(&[core_wasm.to_str().expect("utf8 path"), "wasm"], &output);
  assert!(!wasm_out.status.success(), "core WASM must reject `try`");
  let wasm_stderr = String::from_utf8_lossy(&wasm_out.stderr);
  assert!(wasm_stderr.contains("E_WASM_UNSUPPORTED_TRY"), "{wasm_stderr}");
  assert!(wasm_stderr.contains("Result"), "{wasm_stderr}");
  assert!(!output.join("program.wasm").exists());
}

#[test]
fn wasi_command_selective_linking_prunes_unused_number_format_and_preserves_float_println() {
  const NUMBER_TABLE_BASE: i32 = 8192;

  fn component_core_data_offsets(bytes: &[u8]) -> Vec<i32> {
    let mut offsets = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
      if let Ok(wasmparser::Payload::ModuleSection { unchecked_range, .. }) = payload {
        let core = &bytes[unchecked_range.start as usize..unchecked_range.end as usize];
        for inner in wasmparser::Parser::new(0).parse_all(core) {
          if let Ok(wasmparser::Payload::DataSection(section)) = inner {
            for segment in section {
              if let wasmparser::DataKind::Active { offset_expr, .. } = segment.expect("valid data segment").kind {
                let mut ops = offset_expr.get_operators_reader();
                if let Ok(wasmparser::Operator::I32Const { value }) = ops.read() {
                  offsets.push(value);
                }
              }
            }
          }
        }
      }
    }
    offsets
  }

  let out_no_fmt = tempfile::tempdir().expect("temp dir");
  let compile_no_fmt = calcit(
    &["examples/wasi-command/calcit.cirru", "wasi", "--boundary", "component"],
    out_no_fmt.path(),
  );
  assert!(
    compile_no_fmt.status.success(),
    "{}",
    String::from_utf8_lossy(&compile_no_fmt.stderr)
  );
  let no_fmt_bytes = fs::read(out_no_fmt.path().join("program.wasm")).expect("read wasi-command wasm");
  assert!(
    no_fmt_bytes.len() < 30_000,
    "examples/wasi-command must prune unreachable core strings/tags below 30 KB (was ~61 KB before static data pruning), got {} bytes",
    no_fmt_bytes.len()
  );
  let no_fmt_offsets = component_core_data_offsets(&no_fmt_bytes);
  assert!(
    !no_fmt_offsets.contains(&NUMBER_TABLE_BASE),
    "examples/wasi-command does not format f64 and must omit Ryū table {NUMBER_TABLE_BASE}, got {no_fmt_offsets:?}"
  );

  let out_fmt = tempfile::tempdir().expect("temp dir");
  let float_snapshot = out_fmt.path().join("wasi-float.cirru");
  fs::write(
    &float_snapshot,
    r#"
{} (:package |wasi-float-test)
  :about "|WASI 0.3 float println selective linking test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!) (:target :wasm)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (println (str (+ 1.25 2.5))) &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
"#,
  )
  .expect("write float WASI snapshot");
  let compile_fmt = calcit(
    &[float_snapshot.to_str().expect("utf8 path"), "wasi", "--boundary", "component"],
    out_fmt.path(),
  );
  assert!(compile_fmt.status.success(), "{}", String::from_utf8_lossy(&compile_fmt.stderr));
  let fmt_bytes = fs::read(out_fmt.path().join("program.wasm")).expect("read float wasi component");
  wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
    .validate_all(&fmt_bytes)
    .expect("float WASI component must validate cleanly");
  let fmt_offsets = component_core_data_offsets(&fmt_bytes);
  assert!(
    fmt_offsets.contains(&NUMBER_TABLE_BASE),
    "WASI float println must keep Ryū table {NUMBER_TABLE_BASE}, got {fmt_offsets:?}"
  );

  if let Some(cli) = std::env::var_os("WASMTIME_CLI") {
    let run = Command::new(cli)
      .args([
        "run",
        "-S",
        "p3",
        "-W",
        "component-model-more-async-builtins=y",
        "-W",
        "component-model-async-stackful=y",
      ])
      .arg(out_fmt.path().join("program.wasm"))
      .output()
      .expect("run float WASI 0.3 command");
    assert_eq!(run.status.code(), Some(0), "{}", String::from_utf8_lossy(&run.stderr));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "3.75\n");
  }
}

#[test]
fn wasi_command_runs_lifted_closures_composition_and_nested_recur_without_call_indirect() {
  let out = tempfile::tempdir().expect("temp dir");
  let snapshot = out.path().join("wasi-closures.cirru");
  fs::write(
    &snapshot,
    r#"
{} (:package |wasi-closures)
  :about "|WASI 0.3 closure lifting, composition, zero-capture singletons, and nested recursive defn test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!) (:target :wasm)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'inc1 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn inc1 (x) (+ x 1)
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'dec1 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn dec1 (x) (- x 1)
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'make-adder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-adder (k)
            fn (x)
              hint-fn $ Fn Number (-> Number)
              + x k
          :examples $ []
          :schema $ quote $ Fn Number $ -> $ Fn Number (-> Number)
        'compose $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn compose (f g)
            fn (x)
              hint-fn $ Fn Number (-> Number)
              f (g x)
          :examples $ []
          :schema $ quote $ Fn (Fn Number (-> Number)) (Fn Number (-> Number)) $ -> $ Fn Number (-> Number)
        'make-summer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-summer (bias)
            let
                sum-loop $ defn sum-loop (n acc)
                  hint-fn $ Fn Number Number (-> Number)
                  if (&<= n 0) (+ acc bias) (sum-loop (- n 1) (+ acc n))
              fn (n)
                hint-fn $ Fn Number (-> Number)
                sum-loop n 0
          :examples $ []
          :schema $ quote $ Fn Number $ -> $ Fn Number (-> Number)
        'make-countdown $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-countdown (base)
            fn (n acc)
              hint-fn $ Fn Number Number (-> Number)
              if (&<= n 0) (+ acc base) (recur (- n 1) (+ acc n))
          :examples $ []
          :schema $ quote $ Fn Number $ -> $ Fn Number Number (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let
                chosen $ if (empty? (get-args)) inc1 dec1
                c1 $ compose (make-adder 10) chosen
                c2 $ make-summer 100
                c3 $ make-countdown 50
                same-fn? $ identical? inc1 inc1
                r1 $ c1 5
                r2 $ c2 4
                r3 $ c3 4 0
              println $ str (if same-fn? |T |F) |: r1 |: r2 |: r3 |: c1 |: (= (type-of chosen) :fn)
              , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
"#,
  )
  .expect("write WASI closure snapshot");
  let compile = calcit(
    &[snapshot.to_str().expect("utf8 path"), "wasi", "--boundary", "component"],
    out.path(),
  );
  assert!(compile.status.success(), "{}", String::from_utf8_lossy(&compile.stderr));
  let bytes = fs::read(out.path().join("program.wasm")).expect("read wasi closures component");
  wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
    .validate_all(&bytes)
    .expect("WASI closures component must validate cleanly");
  for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
    if let wasmparser::Payload::ModuleSection { unchecked_range, .. } = payload.expect("valid wasm payload") {
      // ModuleSection 0 is Calcit's emitted core module; subsequent sections are wit-component adapter shims.
      let core = &bytes[unchecked_range.start as usize..unchecked_range.end as usize];
      for inner in wasmparser::Parser::new(0).parse_all(core) {
        match inner.expect("valid core module payload") {
          wasmparser::Payload::TableSection(section) => {
            assert_eq!(section.count(), 0, "Calcit core module must emit zero funcref tables");
          }
          wasmparser::Payload::CodeSectionEntry(body) => {
            let mut reader = body.get_operators_reader().expect("operators reader");
            while !reader.eof() {
              let op = reader.read().expect("valid operator");
              assert!(
                !matches!(op, wasmparser::Operator::CallIndirect { .. }),
                "WASI closure module must emit zero CallIndirect instructions"
              );
            }
          }
          _ => {}
        }
      }
      break;
    }
  }
  if let Some(cli) = std::env::var_os("WASMTIME_CLI") {
    let run = Command::new(cli)
      .args([
        "run",
        "-S",
        "p3",
        "-W",
        "component-model-more-async-builtins=y",
        "-W",
        "component-model-async-stackful=y",
      ])
      .arg(out.path().join("program.wasm"))
      .output()
      .expect("run WASI closure component");
    assert_eq!(run.status.code(), Some(0), "{}", String::from_utf8_lossy(&run.stderr));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "T:14:110:60:(&fn):true\n");
  }
}

#[test]
fn wasi_command_closure_edge_cases_and_defref_dependency_tracking() {
  let dir = tempfile::tempdir().expect("create tempdir");
  let snapshot = dir.path().join("calcit.cirru");
  let out = dir.path().join("out");
  fs::create_dir_all(&out).expect("create out dir");

  fs::write(
    &snapshot,
    r#"{} (:package |closure-edges)
  :about "|Closure edge cases: escaping callback identical?, recursive local HOF callback, qualified value call, defref alias & dependency rejection."
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'closure-edges.main/main!) (:mode :native) (:reload-fn 'closure-edges.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'closure-edges.helper
      %{} 'FileEntry
        :defs $ {}
          'inc1 $ %{} 'CodeEntry (:doc |)
            :code $ quote $ defn inc1 (x) (+ x 1)
            :examples $ []
            :schema $ quote $ Fn Number (-> Number)
          'cb $ %{} 'CodeEntry (:doc |)
            :code $ quote $ def cb $ fn (x) (+ x 10)
            :examples $ []
            :schema $ quote $ Fn Number (-> Number)
        :ns $ %{} 'NsEntry (:doc |)
          :code $ quote $ ns closure-edges.helper
    'closure-edges.other
      %{} 'FileEntry
        :defs $ {}
          'cb $ %{} 'CodeEntry (:doc |)
            :code $ quote $ defn cb (x) (+ x 999)
            :examples $ []
            :schema $ quote $ Fn Number (-> Number)
        :ns $ %{} 'NsEntry (:doc |)
          :code $ quote $ ns closure-edges.other
    'closure-edges.main
      %{} 'FileEntry
        :defs $ {}
          'saved-alias $ %{} 'CodeEntry (:doc |)
            :code $ quote $ def saved-alias helper/inc1
            :examples $ []
            :schema $ quote $ Fn Number (-> Number)
          '*alias-cb $ %{} 'CodeEntry (:doc |)
            :code $ quote $ defref *alias-cb saved-alias
            :examples $ []
            :schema $ quote $ Ref $ Fn Number (-> Number)
          'touch-other $ %{} 'CodeEntry (:doc |)
            :code $ quote $ defn touch-other (x) (other/cb x)
            :examples $ []
            :schema $ quote $ Fn Number (-> Number)
          'probe-id $ %{} 'CodeEntry (:doc |)
            :code $ quote $ defn probe-id (f)
              and (= (f 5) 15) (identical? f f)
            :examples $ []
            :schema $ quote $ Fn (Fn Number (-> Number)) (-> Bool)
          'main! $ %{} 'CodeEntry (:doc |)
            :code $ quote $ defn main! ()
              let
                  bias 10
                  same? $ probe-id $ fn (x)
                    hint-fn $ Fn Number (-> Number)
                    + x bias
                  countdown $ defn countdown (n)
                    hint-fn $ Fn Number (-> Number)
                    if (&<= n 0) 0 (countdown (- n 1))
                  d1 $ countdown 3
                  h1 $ &list:nth (map ([] 2 3) countdown) 1
                  q1 $ helper/cb 5
                  a1 $ @*alias-cb 41
                println $ str same? |: d1 |: h1 |: q1 |: a1
                , &unit
            :examples $ []
            :schema $ quote $ Fn (-> Unit) (:features :io)
          'reload! $ %{} 'CodeEntry (:doc |)
            :code $ quote $ defn reload! () &unit
            :examples $ []
            :schema $ quote $ Fn (-> Unit)
        :ns $ %{} 'NsEntry (:doc |)
          :code $ quote $ ns closure-edges.main
            :require
              closure-edges.helper :as helper
              closure-edges.other :as other
"#,
  )
  .expect("write closure edges snapshot");

  let compile = calcit(&[snapshot.to_str().expect("utf8 path"), "wasi", "--boundary", "component"], &out);
  assert!(compile.status.success(), "{}", String::from_utf8_lossy(&compile.stderr));
  if let Some(cli) = std::env::var_os("WASMTIME_CLI") {
    let run = Command::new(cli)
      .args([
        "run",
        "-S",
        "p3",
        "-W",
        "component-model-more-async-builtins=y",
        "-W",
        "component-model-async-stackful=y",
      ])
      .arg(out.join("program.wasm"))
      .output()
      .expect("run WASI closure edge cases component");
    assert_eq!(run.status.code(), Some(0), "{}", String::from_utf8_lossy(&run.stderr));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true:0:0:15:42\n");
  }

  // Unsupported target initialized into defref must be rejected in WASI Component.
  let bad_snapshot = dir.path().join("bad-defref.cirru");
  let bad_out = dir.path().join("bad-out");
  fs::create_dir_all(&bad_out).expect("create bad out");
  fs::write(
    &bad_snapshot,
    r#"{} (:package |bad-defref)
  :about "|Verify WASI Component rejects reachable defref initialized with an unsupported function."
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'bad-defref.main/main!) (:mode :native) (:reload-fn 'bad-defref.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'bad-defref.helper
      %{} 'FileEntry
        :defs $ {}
          'bad-fn $ %{} 'CodeEntry (:doc |)
            :code $ quote $ defn bad-fn (x) (deref (ref x))
            :examples $ []
            :schema $ quote $ Fn Number (-> Number)
        :ns $ %{} 'NsEntry (:doc |)
          :code $ quote $ ns bad-defref.helper
    'bad-defref.main
      %{} 'FileEntry
        :defs $ {}
          '*bad-cb $ %{} 'CodeEntry (:doc |)
            :code $ quote $ defref *bad-cb bad-defref.helper/bad-fn
            :examples $ []
            :schema $ quote $ Ref $ Fn Number (-> Number)
          'main! $ %{} 'CodeEntry (:doc |)
            :code $ quote $ defn main! ()
              let ((f @*bad-cb)) (println (f 1))
              , &unit
            :examples $ []
            :schema $ quote $ Fn (-> Unit) (:features :io)
          'reload! $ %{} 'CodeEntry (:doc |)
            :code $ quote $ defn reload! () &unit
            :examples $ []
            :schema $ quote $ Fn (-> Unit)
        :ns $ %{} 'NsEntry (:doc |)
          :code $ quote $ ns bad-defref.main
            :require
              bad-defref.helper :refer $ bad-fn
"#,
  )
  .expect("write bad defref snapshot");
  let bad_compile = calcit(
    &[bad_snapshot.to_str().expect("utf8 path"), "wasi", "--boundary", "component"],
    &bad_out,
  );
  assert!(!bad_compile.status.success(), "defref to unsupported fn must fail");
  let bad_stderr = String::from_utf8_lossy(&bad_compile.stderr);
  assert!(
    bad_stderr.contains("E_WASI_COMMAND_DEPENDENCY"),
    "expected E_WASI_COMMAND_DEPENDENCY for bad defref target, got: {bad_stderr}"
  );

  // Anonymous closure literal in defref must fail cleanly rather than leaving 0.0.
  let anon_snapshot = dir.path().join("anon-defref.cirru");
  let anon_out = dir.path().join("anon-out");
  fs::create_dir_all(&anon_out).expect("create anon out");
  fs::write(
    &anon_snapshot,
    r#"{} (:package |anon-defref)
  :about "|Verify anonymous closure in defref rejects cleanly instead of leaving null callable."
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'anon-defref.main/main!) (:mode :native) (:reload-fn 'anon-defref.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'anon-defref.main
    %{} 'FileEntry
      :defs $ {}
        '*anon-cb $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *anon-cb (fn (x) (hint-fn $ Fn Number (-> Number)) x)
          :examples $ []
          :schema $ quote $ Ref $ Fn Number (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let ((f @*anon-cb)) (println (f 1))
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns anon-defref.main
"#,
  )
  .expect("write anon defref snapshot");
  let anon_compile = calcit(
    &[anon_snapshot.to_str().expect("utf8 path"), "wasi", "--boundary", "component"],
    &anon_out,
  );
  assert!(!anon_compile.status.success(), "anonymous closure in defref must fail");
  let anon_stderr = String::from_utf8_lossy(&anon_compile.stderr);
  assert!(
    anon_stderr.contains("E_WASM_CLOSURE_SPECIALIZATION"),
    "expected E_WASM_CLOSURE_SPECIALIZATION for anon defref, got: {anon_stderr}"
  );
}
