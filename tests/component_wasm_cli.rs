use std::fs;
use std::path::PathBuf;
use std::process::Command;
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
    let path = std::env::temp_dir().join(format!("calcit-component-wasm-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&path).expect("temporary output directory should create");
    Self(path)
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

#[test]
fn component_boundary_round_trips_number_string_and_realloc() {
  let output = TestDirectory::create();
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm.cirru",
      "wasm",
      "--boundary",
      "component",
      "--emit-path",
    ])
    .arg(&output.0)
    .output()
    .expect("component fixture should compile");
  assert!(
    compile.status.success(),
    "component compile failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );

  let wasm = output.0.join("program.wasm");
  let script = r#"
const fs = require("fs");
WebAssembly.instantiate(fs.readFileSync(process.argv[1]), {}).then(({ instance }) => {
  const e = instance.exports;
  if (e["add-one"](41) !== 42) throw new Error("Number adapter did not round-trip");
  const input = Buffer.from("你好 Calcit", "utf8");
  const ptr = e.cabi_realloc(0, 0, 1, input.length);
  new Uint8Array(e.memory.buffer, ptr, input.length).set(input);
  const ret = e["echo-text"](ptr, input.length);
  const memory = new DataView(e.memory.buffer);
  const outputPtr = memory.getUint32(ret, true);
  const outputLen = memory.getUint32(ret + 4, true);
  const text = Buffer.from(e.memory.buffer, outputPtr, outputLen).toString("utf8");
  if (text !== "你好 Calcit") throw new Error(`String adapter returned ${text}`);
  const moved = e.cabi_realloc(ptr, input.length, 8, input.length + 4);
  if (moved % 8 !== 0) throw new Error("cabi_realloc ignored alignment");
  const preserved = Buffer.from(e.memory.buffer, moved, input.length).toString("utf8");
  if (preserved !== "你好 Calcit") throw new Error("cabi_realloc did not preserve bytes");
  const pagesBefore = e.memory.buffer.byteLength / 65536;
  const largeSize = e.memory.buffer.byteLength + 1;
  const largePtr = e.cabi_realloc(0, 0, 1, largeSize);
  const pagesAfter = e.memory.buffer.byteLength / 65536;
  if (pagesAfter <= pagesBefore) throw new Error("cabi_realloc did not grow memory");
  if (largePtr + largeSize > e.memory.buffer.byteLength) throw new Error("grown allocation is out of bounds");
}).catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
"#;
  let runtime = Command::new("node")
    .args(["-e", script])
    .arg(&wasm)
    .output()
    .expect("Node.js should validate and instantiate the generated module");
  assert!(
    runtime.status.success(),
    "component runtime failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&runtime.stdout),
    String::from_utf8_lossy(&runtime.stderr)
  );
}
