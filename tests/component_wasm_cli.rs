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
fn component_boundary_round_trips_bool_buffer_number_string_and_realloc() {
  let output = TestDirectory::create();
  let check = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm.cirru",
      "wasm",
      "--boundary",
      "component",
      "--check-only",
    ])
    .output()
    .expect("component fixture check should run");
  assert!(
    check.status.success(),
    "component check failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&check.stdout),
    String::from_utf8_lossy(&check.stderr)
  );
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
const bytes = fs.readFileSync(process.argv[1]);
const module = new WebAssembly.Module(bytes);
const imports = WebAssembly.Module.imports(module);
const importNames = imports.map(({ module, name }) => `${module}/${name}`).sort();
if (importNames.join(",") !== "host/add-one,host/bool-not,host/buffer,host/echo") {
  throw new Error(`unexpected imports: ${importNames.join(",")}`);
}
let instance;
let hostBoolOverride = null;
let hostBufferReturnsInvalidRange = false;
const host = {
  "add-one": value => value + 1,
  "bool-not": value => {
    if (value !== 0 && value !== 1) throw new Error(`host received invalid bool ${value}`);
    return hostBoolOverride ?? (value === 0 ? 1 : 0);
  },
  buffer: (inputPtr, inputLen, retPtr) => {
    const e = instance.exports;
    if (hostBufferReturnsInvalidRange) {
      const memory = new DataView(e.memory.buffer);
      memory.setUint32(retPtr, e.memory.buffer.byteLength - 1, true);
      memory.setUint32(retPtr + 4, 2, true);
      return;
    }
    const input = Uint8Array.from(new Uint8Array(e.memory.buffer, inputPtr, inputLen));
    input.reverse();
    const outputPtr = e.cabi_realloc(0, 0, 1, input.length);
    new Uint8Array(e.memory.buffer, outputPtr, input.length).set(input);
    const memory = new DataView(e.memory.buffer);
    memory.setUint32(retPtr, outputPtr, true);
    memory.setUint32(retPtr + 4, input.length, true);
  },
  echo: (inputPtr, inputLen, retPtr) => {
    const e = instance.exports;
    const text = Buffer.from(e.memory.buffer, inputPtr, inputLen).toString("utf8");
    const output = Buffer.from(`${text} from host`, "utf8");
    const outputPtr = e.cabi_realloc(0, 0, 1, output.length);
    new Uint8Array(e.memory.buffer, outputPtr, output.length).set(output);
    const memory = new DataView(e.memory.buffer);
    memory.setUint32(retPtr, outputPtr, true);
    memory.setUint32(retPtr + 4, output.length, true);
  },
};
WebAssembly.instantiate(module, { host }).then(result => {
  instance = result;
  const e = instance.exports;
  const allocateBytes = bytes => {
    const input = Uint8Array.from(bytes);
    const ptr = e.cabi_realloc(0, 0, 1, input.length);
    new Uint8Array(e.memory.buffer, ptr, input.length).set(input);
    return [ptr, input.length];
  };
  const readBytesResult = ret => {
    const memory = new DataView(e.memory.buffer);
    const ptr = memory.getUint32(ret, true);
    const len = memory.getUint32(ret + 4, true);
    return Uint8Array.from(new Uint8Array(e.memory.buffer, ptr, len));
  };
  const expectBytes = (actual, expected, label) => {
    if (actual.length !== expected.length) {
      throw new Error(`${label}: got length ${actual.length}, expected ${expected.length}`);
    }
    const mismatch = actual.findIndex((byte, index) => byte !== expected[index]);
    if (mismatch !== -1) {
      throw new Error(`${label}: byte ${mismatch} was ${actual[mismatch]}, expected ${expected[mismatch]}`);
    }
  };
  if (e["add-one"](41) !== 42) throw new Error("Number adapter did not round-trip");
  if (e["bool-not"](1) !== 0 || e["bool-not"](0) !== 1) throw new Error("Bool adapter did not round-trip");
  if (e["choose-number"](1, 3, 4) !== 3 || e["choose-number"](0, 3, 4) !== 4) {
    throw new Error("mixed Bool and Number adapter did not round-trip");
  }
  for (const invoke of [() => e["bool-not"](2), () => e["choose-number"](2, 3, 4)]) {
    let trapped = false;
    try { invoke(); } catch (error) { trapped = error instanceof WebAssembly.RuntimeError; }
    if (!trapped) throw new Error("invalid canonical Bool input did not trap");
  }
  const input = Buffer.from("你好 Calcit", "utf8");
  const ptr = e.cabi_realloc(0, 0, 1, input.length);
  new Uint8Array(e.memory.buffer, ptr, input.length).set(input);
  const ret = e["echo-text"](ptr, input.length);
  const memory = new DataView(e.memory.buffer);
  const outputPtr = memory.getUint32(ret, true);
  const outputLen = memory.getUint32(ret + 4, true);
  const text = Buffer.from(e.memory.buffer, outputPtr, outputLen).toString("utf8");
  if (text !== "你好 Calcit") throw new Error(`String adapter returned ${text}`);
  for (const bytes of [[], [0, 255, 17], [128, 0, 254, 1]]) {
    const [bufferPtr, bufferLen] = allocateBytes(bytes);
    expectBytes(readBytesResult(e["echo-buffer"](bufferPtr, bufferLen)), bytes, "Buffer adapter did not round-trip");
  }
  const [taggedBufferPtr, taggedBufferLen] = allocateBytes([0, 255, 17]);
  if (e["is-buffer"](taggedBufferPtr, taggedBufferLen) !== 1) {
    throw new Error("Buffer lift did not preserve the Calcit type tag");
  }
  const growingBytes = Uint8Array.from({ length: e.memory.buffer.byteLength + 257 }, (_, index) => index % 251);
  const [growingPtr, growingLen] = allocateBytes(growingBytes);
  for (let round = 0; round < 2; round += 1) {
    expectBytes(
      readBytesResult(e["echo-buffer"](growingPtr, growingLen)),
      growingBytes,
      `growing Buffer round ${round}`,
    );
  }
  const [yesPtr, yesLen] = allocateBytes([0, 255]);
  const [noPtr, noLen] = allocateBytes([17, 0, 128]);
  expectBytes(readBytesResult(e["choose-buffer"](1, yesPtr, yesLen, noPtr, noLen)), [0, 255], "Bool+Buffer true branch");
  expectBytes(readBytesResult(e["choose-buffer"](0, yesPtr, yesLen, noPtr, noLen)), [17, 0, 128], "Bool+Buffer false branch");
  let invalidBufferInputTrapped = false;
  try { e["echo-buffer"](e.memory.buffer.byteLength - 1, 2); } catch (error) {
    invalidBufferInputTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!invalidBufferInputTrapped) throw new Error("out-of-bounds Buffer input did not trap");
  const moved = e.cabi_realloc(ptr, input.length, 8, input.length + 4);
  if (moved % 8 !== 0) throw new Error("cabi_realloc ignored alignment");
  const preserved = Buffer.from(e.memory.buffer, moved, input.length).toString("utf8");
  if (preserved !== "你好 Calcit") throw new Error("cabi_realloc did not preserve bytes");
  if (e["call-host-add-one"](9) !== 10) throw new Error("Number import adapter did not round-trip");
  if (e["call-host-bool-not"](1) !== 0) throw new Error("Bool import adapter did not round-trip");
  hostBoolOverride = 2;
  let invalidHostBoolTrapped = false;
  try { e["call-host-bool-not"](1); } catch (error) { invalidHostBoolTrapped = error instanceof WebAssembly.RuntimeError; }
  if (!invalidHostBoolTrapped) throw new Error("invalid imported canonical Bool did not trap");
  const hostInput = Buffer.from("你好", "utf8");
  const hostInputPtr = e.cabi_realloc(0, 0, 1, hostInput.length);
  new Uint8Array(e.memory.buffer, hostInputPtr, hostInput.length).set(hostInput);
  const hostRet = e["call-host-echo"](hostInputPtr, hostInput.length);
  const hostMemory = new DataView(e.memory.buffer);
  const hostOutputPtr = hostMemory.getUint32(hostRet, true);
  const hostOutputLen = hostMemory.getUint32(hostRet + 4, true);
  const hostText = Buffer.from(e.memory.buffer, hostOutputPtr, hostOutputLen).toString("utf8");
  if (hostText !== "你好 from host") throw new Error(`String import adapter returned ${hostText}`);
  const [hostBufferPtr, hostBufferLen] = allocateBytes([0, 255, 17]);
  expectBytes(
    readBytesResult(e["call-host-buffer"](hostBufferPtr, hostBufferLen)),
    [17, 255, 0],
    "Buffer import adapter did not round-trip",
  );
  hostBufferReturnsInvalidRange = true;
  let invalidHostBufferTrapped = false;
  try { e["call-host-buffer"](hostBufferPtr, hostBufferLen); } catch (error) {
    invalidHostBufferTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!invalidHostBufferTrapped) throw new Error("out-of-bounds imported Buffer result did not trap");
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
