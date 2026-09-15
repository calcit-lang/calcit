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
fn component_boundary_round_trips_variants_lists_and_scalar_values() {
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
if (importNames.join(",") !== "host/add-one,host/bool-not,host/buffer,host/echo,host/numbers,host/option-number,host/ping,host/profile,host/result-number") {
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
  "option-number": (discriminant, payload, retPtr) => {
    const memory = new DataView(instance.exports.memory.buffer);
    memory.setUint8(retPtr, discriminant);
    if (discriminant === 1) memory.setFloat64(retPtr + 8, payload, true);
  },
  "result-number": (discriminant, payload0, payload1, retPtr) => {
    const memory = new DataView(instance.exports.memory.buffer);
    memory.setUint8(retPtr, discriminant);
    if (discriminant === 0) {
      memory.setBigUint64(retPtr + 8, payload0, true);
    } else {
      memory.setUint32(retPtr + 8, Number(payload0), true);
      memory.setUint32(retPtr + 12, payload1, true);
    }
  },
  profile: (active, maybeDiscriminant, maybePtr, maybeLen, namePtr, nameLen, outcomeDiscriminant, outcomePtr, outcomeLen, scoresPtr, scoresLen, score, retPtr) => {
    if (active !== 0 && active !== 1) throw new Error(`host received invalid profile bool ${active}`);
    const memory = new DataView(instance.exports.memory.buffer);
    memory.setUint8(retPtr, active === 0 ? 1 : 0);
    memory.setUint8(retPtr + 4, maybeDiscriminant);
    memory.setUint32(retPtr + 8, maybePtr, true);
    memory.setUint32(retPtr + 12, maybeLen, true);
    memory.setUint32(retPtr + 16, namePtr, true);
    memory.setUint32(retPtr + 20, nameLen, true);
    memory.setUint8(retPtr + 24, outcomeDiscriminant);
    memory.setUint32(retPtr + 28, outcomePtr, true);
    memory.setUint32(retPtr + 32, outcomeLen, true);
    memory.setUint32(retPtr + 36, scoresPtr, true);
    memory.setUint32(retPtr + 40, scoresLen, true);
    memory.setFloat64(retPtr + 48, score + 1, true);
  },
  ping: () => undefined,
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
  const readProfile = ret => {
    const memory = new DataView(e.memory.buffer);
    const namePtr = memory.getUint32(ret + 16, true);
    const nameLen = memory.getUint32(ret + 20, true);
    return {
      name: Buffer.from(e.memory.buffer, namePtr, nameLen).toString("utf8"),
      stats: { score: memory.getFloat64(ret + 48, true) },
      maybeName: memory.getUint8(ret + 4) === 0
        ? null
        : Buffer.from(e.memory.buffer, memory.getUint32(ret + 8, true), memory.getUint32(ret + 12, true)).toString("utf8"),
      outcome: memory.getUint8(ret + 24) === 0
        ? { ok: Array.from(
            { length: memory.getUint32(ret + 32, true) },
            (_, index) => memory.getFloat64(memory.getUint32(ret + 28, true) + index * 8, true),
          ) }
        : { err: Buffer.from(
            e.memory.buffer,
            memory.getUint32(ret + 28, true),
            memory.getUint32(ret + 32, true),
          ).toString("utf8") },
      active: memory.getUint8(ret),
      scores: (() => {
        const ptr = memory.getUint32(ret + 36, true);
        const len = memory.getUint32(ret + 40, true);
        return Array.from({ length: len }, (_, index) => memory.getFloat64(ptr + index * 8, true));
      })(),
    };
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
  const floatBits = value => {
    const bytes = new ArrayBuffer(8);
    const view = new DataView(bytes);
    view.setFloat64(0, value, true);
    return view.getBigUint64(0, true);
  };
  const readOptionNumber = ret => {
    const view = new DataView(e.memory.buffer);
    const discriminant = view.getUint8(ret);
    return discriminant === 0 ? null : view.getFloat64(ret + 8, true);
  };
  const readTextVariant = (ret, payloadOffset) => {
    const view = new DataView(e.memory.buffer);
    const discriminant = view.getUint8(ret);
    const itemPtr = view.getUint32(ret + payloadOffset, true);
    const itemLen = view.getUint32(ret + payloadOffset + 4, true);
    return [discriminant, Buffer.from(e.memory.buffer, itemPtr, itemLen).toString("utf8")];
  };
  if (e["add-one"](41) !== 42) throw new Error("Number adapter did not round-trip");
  if (e["bool-not"](1) !== 0 || e["bool-not"](0) !== 1) throw new Error("Bool adapter did not round-trip");
  if (e["choose-number"](1, 3, 4) !== 3 || e["choose-number"](0, 3, 4) !== 4) {
    throw new Error("mixed Bool and Number adapter did not round-trip");
  }
  if (readOptionNumber(e["echo-option-number"](0, 0)) !== null) {
    throw new Error("Option<Number> none did not round-trip");
  }
  if (readOptionNumber(e["echo-option-number"](1, 7.5)) !== 7.5) {
    throw new Error("Option<Number> some did not round-trip");
  }
  const optionText = Buffer.from("可选", "utf8");
  const [optionTextPtr, optionTextLen] = allocateBytes(optionText);
  const [optionTextTag, optionTextValue] = readTextVariant(e["echo-option-text"](1, optionTextPtr, optionTextLen), 4);
  if (optionTextTag !== 1 || optionTextValue !== "可选") throw new Error("Option<String> did not round-trip");
  const resultNumberOk = e["echo-result-number"](0, floatBits(9.25), 0);
  const resultNumberView = new DataView(e.memory.buffer);
  if (resultNumberView.getUint8(resultNumberOk) !== 0 || resultNumberView.getFloat64(resultNumberOk + 8, true) !== 9.25) {
    throw new Error("Result<Number,String> ok did not round-trip");
  }
  const resultError = Buffer.from("bad", "utf8");
  const [resultErrorPtr, resultErrorLen] = allocateBytes(resultError);
  const [resultErrorTag, resultErrorValue] = readTextVariant(
    e["echo-result-number"](1, BigInt(resultErrorPtr), resultErrorLen),
    8,
  );
  if (resultErrorTag !== 1 || resultErrorValue !== "bad") throw new Error("Result<Number,String> err did not round-trip");
  const unitOk = e["echo-result-unit"](0, 0, 0);
  if (new DataView(e.memory.buffer).getUint8(unitOk) !== 0) throw new Error("Result<Unit,String> ok did not round-trip");
  const [unitErrorTag, unitErrorValue] = readTextVariant(e["echo-result-unit"](1, resultErrorPtr, resultErrorLen), 4);
  if (unitErrorTag !== 1 || unitErrorValue !== "bad") throw new Error("Result<Unit,String> err did not round-trip");
  const [resultListPtr, resultListLen] = allocateNumberList([2, 4, 8]);
  const resultListRet = e["echo-result-numbers"](0, resultListPtr, resultListLen);
  const resultListView = new DataView(e.memory.buffer);
  const loweredListPtr = resultListView.getUint32(resultListRet + 4, true);
  const loweredListLen = resultListView.getUint32(resultListRet + 8, true);
  const loweredList = Array.from({ length: loweredListLen }, (_, index) => resultListView.getFloat64(loweredListPtr + index * 8, true));
  if (resultListView.getUint8(resultListRet) !== 0 || loweredList.join(",") !== "2,4,8") {
    throw new Error("Result<List<Number>,String> ok did not round-trip");
  }
  if (e.ping() !== undefined) throw new Error("Unit result should use the zero-result canonical shape");
  if (readOptionNumber(e["call-host-option-number"](1, 12.5)) !== 12.5) {
    throw new Error("imported Option<Number> did not round-trip");
  }
  const importedResultOk = e["call-host-result-number"](0, floatBits(6.5), 0);
  if (new DataView(e.memory.buffer).getFloat64(importedResultOk + 8, true) !== 6.5) {
    throw new Error("imported Result<Number,String> ok did not round-trip");
  }
  const [importedResultTag, importedResultError] = readTextVariant(
    e["call-host-result-number"](1, BigInt(resultErrorPtr), resultErrorLen),
    8,
  );
  if (importedResultTag !== 1 || importedResultError !== "bad") {
    throw new Error("imported Result<Number,String> err did not round-trip");
  }
  if (e["call-host-ping"]() !== undefined) throw new Error("imported Unit result should remain zero-result");
  const [profileNamePtr, profileNameLen] = allocateBytes(Buffer.from("Ada", "utf8"));
  const [profileScoresPtr, profileScoresLen] = allocateNumberList([1, 2, 3]);
  const [profileOutcomePtr, profileOutcomeLen] = allocateNumberList([4, 5]);
  const directProfile = readProfile(e["echo-profile"](1, 1, profileNamePtr, profileNameLen, profileNamePtr, profileNameLen, 0, profileOutcomePtr, profileOutcomeLen, profileScoresPtr, profileScoresLen, 7.5));
  if (JSON.stringify(directProfile) !== JSON.stringify({ name: "Ada", stats: { score: 7.5 }, maybeName: "Ada", outcome: { ok: [4, 5] }, active: 1, scores: [1, 2, 3] })) {
    throw new Error(`Struct record did not round-trip: ${JSON.stringify(directProfile)}`);
  }
  const hostProfile = readProfile(e["call-host-profile"](1, 1, profileNamePtr, profileNameLen, profileNamePtr, profileNameLen, 0, profileOutcomePtr, profileOutcomeLen, profileScoresPtr, profileScoresLen, 7.5));
  if (JSON.stringify(hostProfile) !== JSON.stringify({ name: "Ada", stats: { score: 8.5 }, maybeName: "Ada", outcome: { ok: [4, 5] }, active: 0, scores: [1, 2, 3] })) {
    throw new Error(`imported Struct record did not round-trip: ${JSON.stringify(hostProfile)}`);
  }
  const [profileErrorPtr, profileErrorLen] = allocateBytes(Buffer.from("bad", "utf8"));
  const directAlternateProfile = readProfile(e["echo-profile"](1, 0, 0, 0, profileNamePtr, profileNameLen, 1, profileErrorPtr, profileErrorLen, profileScoresPtr, profileScoresLen, 7.5));
  if (JSON.stringify(directAlternateProfile) !== JSON.stringify({ name: "Ada", stats: { score: 7.5 }, maybeName: null, outcome: { err: "bad" }, active: 1, scores: [1, 2, 3] })) {
    throw new Error(`Struct None/Err record did not round-trip: ${JSON.stringify(directAlternateProfile)}`);
  }
  const hostAlternateProfile = readProfile(e["call-host-profile"](1, 0, 0, 0, profileNamePtr, profileNameLen, 1, profileErrorPtr, profileErrorLen, profileScoresPtr, profileScoresLen, 7.5));
  if (JSON.stringify(hostAlternateProfile) !== JSON.stringify({ name: "Ada", stats: { score: 8.5 }, maybeName: null, outcome: { err: "bad" }, active: 0, scores: [1, 2, 3] })) {
    throw new Error(`imported Struct None/Err record did not round-trip: ${JSON.stringify(hostAlternateProfile)}`);
  }
  for (const discriminant of [2, 256]) {
    let invalidVariantTrapped = false;
    try { e["echo-option-number"](discriminant, 0); } catch (error) {
      invalidVariantTrapped = error instanceof WebAssembly.RuntimeError;
    }
    if (!invalidVariantTrapped) throw new Error(`invalid Option discriminant ${discriminant} did not trap`);
  }
  for (const invoke of [
    () => e["bool-not"](2),
    () => e["choose-number"](2, 3, 4),
    () => e["echo-profile"](2, 1, profileNamePtr, profileNameLen, profileNamePtr, profileNameLen, 0, profileOutcomePtr, profileOutcomeLen, profileScoresPtr, profileScoresLen, 7.5),
  ]) {
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
  const misalignedNumberPtr = e.cabi_realloc(0, 0, 8, 16) + 4;
  let misalignedNumberListTrapped = false;
  try { e["echo-numbers"](misalignedNumberPtr, 1); } catch (error) {
    misalignedNumberListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!misalignedNumberListTrapped) throw new Error("misaligned Number List input did not trap");
  let overflowingNumberListTrapped = false;
  try { e["echo-numbers"](0, 0x20000000); } catch (error) {
    overflowingNumberListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!overflowingNumberListTrapped) throw new Error("overflowing Number List length did not trap");
  const [misalignedNestedPtr, misalignedNestedLen] = allocatePairList([[nestedPairs[0][0] + 4, 1]]);
  let misalignedNestedListTrapped = false;
  try { e["echo-number-lists"](misalignedNestedPtr, misalignedNestedLen); } catch (error) {
    misalignedNestedListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!misalignedNestedListTrapped) throw new Error("misaligned nested Number List input did not trap");
  const [pairListPtr] = allocatePairList([nestedPairs[0]]);
  let misalignedPairListTrapped = false;
  try { e["echo-number-lists"](pairListPtr + 2, 1); } catch (error) {
    misalignedPairListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!misalignedPairListTrapped) throw new Error("misaligned nested List pair input did not trap");
  const [invalidNestedPtr, invalidNestedLen] = allocatePairList([[e.memory.buffer.byteLength - 4, 1]]);
  let invalidNestedListTrapped = false;
  try { e["echo-number-lists"](invalidNestedPtr, invalidNestedLen); } catch (error) {
    invalidNestedListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!invalidNestedListTrapped) throw new Error("out-of-bounds nested Number List input did not trap");
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
