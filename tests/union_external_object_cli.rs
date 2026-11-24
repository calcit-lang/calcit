use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
  fn create() -> Self {
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("test clock should be valid")
      .as_nanos();
    let counter = TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("calcit-union-external-object-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&path).expect("temporary project should create");
    Self(path)
  }

  fn path(&self) -> &Path {
    &self.0
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

fn run_calcit(snapshot: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .args(args)
    .output()
    .expect("calcit command should run")
}

fn assert_success(output: &Output, context: &str) {
  assert!(
    output.status.success(),
    "{context} failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn union_external_object_trait_runs_in_js_and_is_rejected_on_wasm() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  let emit_dir = directory.path().join("js-out");

  let snapshot_source = r#"
{}
  :about "|test snapshot for #2903"
  :package |union-host-demo
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'app.main/main!) (:mode :js) (:reload-fn 'app.main/main!) (:target :browser)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :node $ {} (:description |) (:init-fn 'app.main/node-main!) (:mode :js) (:reload-fn 'app.main/node-main!) (:target :node)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :wasm-entry $ {} (:description |) (:init-fn 'app.main/wasm-main!) (:mode :native) (:reload-fn 'app.main/wasm-main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'js-ffi.browser $ %{} 'FileEntry
      :defs $ {}
        'BlobHost $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defexternal BlobHost
            :target :browser
            :size Number
            :type String
          :examples $ []
          :schema $ quote Trait
      :ns $ %{} 'CodeEntry (:doc |)
        :code $ quote $ ns js-ffi.browser
        :examples $ []
        :schema $ quote Dynamic
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'UploadFile $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion UploadFile String js-ffi.browser/BlobHost
          :examples $ []
          :schema $ quote UnionDef
        'describe-upload $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn describe-upload (file)
            hint-fn $ Fn UploadFile (-> String) (:features :js-ffi)
            match file
              (String path) (str |path: path)
              (BlobHost blob) (str |blob: (blob :type) |: (blob :size))
          :examples $ []
          :schema $ quote $ Fn UploadFile (-> String) (:features :js-ffi)
        'make-blob-upload $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-blob-upload ()
            hint-fn $ Fn (-> UploadFile) (:features :js-ffi)
            js-cast (new js/Blob (js-array |hello) (js-object (:type |text/plain))) BlobHost
          :examples $ []
          :schema $ quote $ Fn (-> UploadFile) (:features :js-ffi)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            hint-fn $ Fn (-> Unit) (:features :js-ffi)
            assert= |path:report.txt (describe-upload |report.txt)
            assert= |blob:text/plain:5 (describe-upload (make-blob-upload))
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :js-ffi)
        'describe-node-upload $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn describe-node-upload (file)
            hint-fn $ Fn UploadFile (-> String)
            match file
              (String path) (str |path: path)
              (BlobHost _blob) |blob
          :examples $ []
          :schema $ quote $ Fn UploadFile (-> String)
        'node-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn node-main! ()
            hint-fn $ Fn (-> Unit)
            assert= |path:report.txt (describe-node-upload |report.txt)
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'wasm-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wasm-main! ()
            hint-fn $ Fn (-> Unit)
            describe-node-upload |report.txt
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'CodeEntry (:doc |)
        :code $ quote $ ns app.main
          :require
            js-ffi.browser :refer $ BlobHost
        :examples $ []
        :schema $ quote Dynamic
"#;

  fs::write(&snapshot, snapshot_source).expect("snapshot should write");

  // 1. Both browser and node entries pass strict --check-only.
  let browser_check = run_calcit(&snapshot, &["--check-only"]);
  assert_success(&browser_check, "browser --check-only");
  let node_check = run_calcit(&snapshot, &["--entry", "node", "--check-only"]);
  assert_success(&node_check, "node --check-only");

  // 2. Emit JS and run in Node.js against both String and real globalThis.Blob.
  let emit_js = run_calcit(&snapshot, &["--emit-path", emit_dir.to_str().unwrap(), "js"]);
  assert_success(&emit_js, "calcit js");

  let script = r#"
import { mkdir, symlink } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const projectPath = process.env.CALCIT_JS_PROJECT_PATH;
const emitPath = process.env.CALCIT_JS_EMIT_PATH;
const runtimeRoot = process.env.CALCIT_RUNTIME_ROOT;
const packageScope = join(projectPath, "node_modules", "@calcit");
await mkdir(packageScope, { recursive: true });
await symlink(runtimeRoot, join(packageScope, "procs"), process.platform === "win32" ? "junction" : "dir");
const generated = await import(pathToFileURL(join(emitPath, "app.main.mjs")).href);
await generated.main_$x_();
"#;
  let node_run = Command::new("node")
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .arg("--input-type=module")
    .arg("--eval")
    .arg(script)
    .env("CALCIT_JS_PROJECT_PATH", directory.path())
    .env("CALCIT_JS_EMIT_PATH", &emit_dir)
    .env("CALCIT_RUNTIME_ROOT", env!("CARGO_MANIFEST_DIR"))
    .output()
    .expect("node command should run");
  assert_success(&node_run, "generated JavaScript execution with String and Blob");

  // 3. WASM and WASI targets must explicitly reject external-object union members.
  #[cfg(feature = "wasm")]
  {
    let wasm_out = directory.path().join("wasm-out");
    let wasm_build = run_calcit(
      &snapshot,
      &["--entry", "wasm-entry", "--emit-path", wasm_out.to_str().unwrap(), "wasm"],
    );
    assert!(
      !wasm_build.status.success(),
      "calcit wasm should reject external-object union member"
    );
    let wasm_stderr = format!(
      "{}\n{}",
      String::from_utf8_lossy(&wasm_build.stdout),
      String::from_utf8_lossy(&wasm_build.stderr)
    );
    assert!(
      wasm_stderr.contains("E_WASM_UNSUPPORTED_JS_FFI"),
      "expected E_WASM_UNSUPPORTED_JS_FFI from calcit wasm, got:\n{wasm_stderr}"
    );

    let wasi_out = directory.path().join("wasi-out");
    let wasi_build = run_calcit(
      &snapshot,
      &["--entry", "wasm-entry", "--emit-path", wasi_out.to_str().unwrap(), "wasi"],
    );
    assert!(
      !wasi_build.status.success(),
      "calcit wasi should reject external-object union member"
    );
    let wasi_stderr = format!(
      "{}\n{}",
      String::from_utf8_lossy(&wasi_build.stdout),
      String::from_utf8_lossy(&wasi_build.stderr)
    );
    assert!(
      wasi_stderr.contains("E_WASM_UNSUPPORTED_JS_FFI"),
      "expected E_WASM_UNSUPPORTED_JS_FFI from calcit wasi, got:\n{wasi_stderr}"
    );
  }
}
