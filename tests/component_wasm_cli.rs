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
fn component_boundary_round_trips_lists_and_scalar_values() {
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
if (importNames.join(",") !== "host/add-one,host/bool-not,host/buffer,host/echo,host/numbers") {
  throw new Error(`unexpected imports: ${importNames.join(",")}`);
}
let instance;
let hostBoolOverride = null;
let hostBufferReturnsInvalidRange = false;
let hostNumbersReturnsInvalidRange = false;
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
  numbers: (inputPtr, inputLen, retPtr) => {
    const e = instance.exports;
    const memory = new DataView(e.memory.buffer);
    if (hostNumbersReturnsInvalidRange) {
      memory.setUint32(retPtr, e.memory.buffer.byteLength - 4, true);
      memory.setUint32(retPtr + 4, 1, true);
      return;
    }
    const values = Array.from({ length: inputLen }, (_, index) => memory.getFloat64(inputPtr + index * 8, true)).reverse();
    const outputPtr = e.cabi_realloc(0, 0, 8, values.length * 8);
    const outputMemory = new DataView(e.memory.buffer);
    values.forEach((value, index) => outputMemory.setFloat64(outputPtr + index * 8, value, true));
    outputMemory.setUint32(retPtr, outputPtr, true);
    outputMemory.setUint32(retPtr + 4, values.length, true);
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
  const allocateNumberList = values => {
    const ptr = e.cabi_realloc(0, 0, 8, values.length * 8);
    const memory = new DataView(e.memory.buffer);
    values.forEach((value, index) => memory.setFloat64(ptr + index * 8, value, true));
    return [ptr, values.length];
  };
  const readNumberListResult = ret => {
    const memory = new DataView(e.memory.buffer);
    const ptr = memory.getUint32(ret, true);
    const len = memory.getUint32(ret + 4, true);
    return Array.from({ length: len }, (_, index) => memory.getFloat64(ptr + index * 8, true));
  };
  const allocateBoolList = values => {
    const ptr = e.cabi_realloc(0, 0, 1, values.length);
    new Uint8Array(e.memory.buffer, ptr, values.length).set(values);
    return [ptr, values.length];
  };
  const readBoolListResult = ret => {
    const memory = new DataView(e.memory.buffer);
    const ptr = memory.getUint32(ret, true);
    const len = memory.getUint32(ret + 4, true);
    return Array.from(new Uint8Array(e.memory.buffer, ptr, len));
  };
  const allocatePairList = pairs => {
    const ptr = e.cabi_realloc(0, 0, 4, pairs.length * 8);
    const memory = new DataView(e.memory.buffer);
    pairs.forEach(([itemPtr, itemLen], index) => {
      memory.setUint32(ptr + index * 8, itemPtr, true);
      memory.setUint32(ptr + index * 8 + 4, itemLen, true);
    });
    return [ptr, pairs.length];
  };
  const readPairListResult = (ret, readItem) => {
    const memory = new DataView(e.memory.buffer);
    const ptr = memory.getUint32(ret, true);
    const len = memory.getUint32(ret + 4, true);
    return Array.from({ length: len }, (_, index) => {
      const itemPtr = memory.getUint32(ptr + index * 8, true);
      const itemLen = memory.getUint32(ptr + index * 8 + 4, true);
      return readItem(itemPtr, itemLen);
    });
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
  for (const numbers of [[], [1, -2.5, 3, 3]]) {
    const [listPtr, listLen] = allocateNumberList(numbers);
    const actual = readNumberListResult(e["echo-numbers"](listPtr, listLen));
    if (actual.length !== numbers.length || actual.some((value, index) => !Object.is(value, numbers[index]))) {
      throw new Error(`Number List adapter returned ${actual}`);
    }
  }
  const [boolListPtr, boolListLen] = allocateBoolList([1, 0, 1]);
  if (readBoolListResult(e["echo-bools"](boolListPtr, boolListLen)).join(",") !== "1,0,1") {
    throw new Error("Bool List adapter did not round-trip");
  }
  const textValues = ["alpha", "", "世界"];
  const textPairs = textValues.map(value => allocateBytes(Buffer.from(value, "utf8")));
  const [textListPtr, textListLen] = allocatePairList(textPairs);
  const textResult = readPairListResult(
    e["echo-texts"](textListPtr, textListLen),
    (itemPtr, itemLen) => Buffer.from(e.memory.buffer, itemPtr, itemLen).toString("utf8"),
  );
  if (JSON.stringify(textResult) !== JSON.stringify(textValues)) throw new Error(`String List adapter returned ${textResult}`);
  const bufferValues = [[0, 255], [], [17, 0, 128]];
  const bufferPairs = bufferValues.map(allocateBytes);
  const [bufferListPtr, bufferListLen] = allocatePairList(bufferPairs);
  const bufferResult = readPairListResult(
    e["echo-buffers"](bufferListPtr, bufferListLen),
    (itemPtr, itemLen) => Array.from(new Uint8Array(e.memory.buffer, itemPtr, itemLen)),
  );
  if (JSON.stringify(bufferResult) !== JSON.stringify(bufferValues)) throw new Error(`Buffer List adapter returned ${bufferResult}`);
  const nestedValues = [[1, 2], [], [-3, 4.5, 7]];
  const nestedPairs = nestedValues.map(allocateNumberList);
  const [nestedPtr, nestedLen] = allocatePairList(nestedPairs);
  const nestedResult = readPairListResult(
    e["echo-number-lists"](nestedPtr, nestedLen),
    (itemPtr, itemLen) => Array.from(
      { length: itemLen },
      (_, index) => new DataView(e.memory.buffer).getFloat64(itemPtr + index * 8, true),
    ),
  );
  if (JSON.stringify(nestedResult) !== JSON.stringify(nestedValues)) throw new Error(`nested List adapter returned ${nestedResult}`);
  const [invalidBoolListPtr, invalidBoolListLen] = allocateBoolList([0, 2]);
  let invalidBoolListTrapped = false;
  try { e["echo-bools"](invalidBoolListPtr, invalidBoolListLen); } catch (error) {
    invalidBoolListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!invalidBoolListTrapped) throw new Error("invalid Bool List element did not trap");
  let invalidNumberListTrapped = false;
  try { e["echo-numbers"](e.memory.buffer.byteLength - 4, 1); } catch (error) {
    invalidNumberListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!invalidNumberListTrapped) throw new Error("out-of-bounds Number List input did not trap");
  const growingNumberCount = e.memory.buffer.byteLength / 8 + 257;
  const growingNumbers = Array.from({ length: growingNumberCount }, (_, index) => index % 251);
  const [growingNumberPtr, growingNumberLen] = allocateNumberList(growingNumbers);
  for (let round = 0; round < 2; round += 1) {
    const actual = readNumberListResult(e["echo-numbers"](growingNumberPtr, growingNumberLen));
    if (actual.length !== growingNumbers.length || actual.some((value, index) => value !== growingNumbers[index])) {
      throw new Error(`growing Number List round ${round} did not preserve values`);
    }
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
  const [hostNumbersPtr, hostNumbersLen] = allocateNumberList([1, -2.5, 7]);
  const hostNumbersResult = readNumberListResult(e["call-host-numbers"](hostNumbersPtr, hostNumbersLen));
  if (JSON.stringify(hostNumbersResult) !== JSON.stringify([7, -2.5, 1])) {
    throw new Error(`Number List import adapter returned ${hostNumbersResult}`);
  }
  hostNumbersReturnsInvalidRange = true;
  let invalidHostNumbersTrapped = false;
  try { e["call-host-numbers"](hostNumbersPtr, hostNumbersLen); } catch (error) {
    invalidHostNumbersTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!invalidHostNumbersTrapped) throw new Error("out-of-bounds imported Number List result did not trap");
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
