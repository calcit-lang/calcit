import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const run = (...args) => execFileSync(binary, args, { encoding: "utf8", stdio: "pipe", maxBuffer: 16 * 1024 * 1024 });
const corePath = resolve("src/cirru/calcit-core.cirru");
// Read the authoritative definition tests as AST, not a second JS assertion suite.
const core = JSON.parse(run("cirru", "parse-edn", "--file", corePath));
const definitions = core[":files"]["'calcit.core"].defs;
const successNames = ["unicode-scalar-indexing", "scalar-slice-boundaries"];
const failureNames = ["rejects-invalid-string-indices", "scalar-slice-invalid-evaluation"];
const available = [...definitions["'last"].tests, ...definitions["'&str:slice"].tests];
const tests = [...successNames, ...failureNames].map((name) => {
  const test = available.find((entry) => entry.name === name);
  assert.ok(test, `missing Unicode definition test: ${name}`);
  return test;
});
const expectedTrace = ["unicode-receiver", "unicode-start", "unicode-end",
  "unicode-invalid-receiver", "unicode-invalid-start", "unicode-invalid-end"];
const fixture = await mkdtemp(join(tmpdir(), "calcit-string-unicode-"));
try {
  const snapshot = join(fixture, "calcit.cirru");
  await copyFile(corePath, snapshot);
  await symlink(resolve("node_modules"), join(fixture, "node_modules"), "dir");
  run(snapshot, "edit", "add-ns", "calcit.unicode");
  run(snapshot, "edit", "def", "calcit.unicode/main!", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "main!", [], ...tests.map((test) => test.code.__edn_quote), "&unit"]));
  run(snapshot, "edit", "schema", "calcit.unicode/main!", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  const entry = [snapshot, "--init-fn", "calcit.unicode/main!", "--reload-fn", "calcit.unicode/main!"];
  const nativeOutput = run(...entry);
  assert.deepEqual(nativeOutput.split(/\r?\n/).filter((line) => line.startsWith("unicode-")), expectedTrace);
  const output = join(fixture, "js-out");
  run(...entry, "--emit-path", output, "js");
  const compiled = await import(pathToFileURL(join(output, "calcit.unicode.mjs")).href);
  const jsTrace = [];
  const originalLog = console.log;
  try {
    console.log = (...values) => jsTrace.push(values.join(" "));
    compiled.main_$x_();
  } finally {
    console.log = originalLog;
  }
  assert.deepEqual(jsTrace, expectedTrace, "JS must evaluate each argument once, including before invalid-index failure");
  console.log("Unicode definition tests passed on native and generated JS");

  // The existing WASM subset has no catchable try boundary. Run successful
  // assertions unchanged; expose invalid operations separately to check traps.
  run(snapshot, "edit", "rm-def", "calcit.unicode/main!");
  const assertions = tests.filter((test) => successNames.includes(test.name)).flatMap((test) => test.code.__edn_quote.slice(1));
  for (const [index, assertion] of assertions.entries()) {
    const name = `wasm-case-${index}`;
    run(snapshot, "edit", "def", `calcit.unicode/${name}`, "--input-format", "json-ast", "--code",
      JSON.stringify(["defwasm-export", name, [], assertion, "&unit"]));
    run(snapshot, "edit", "schema", `calcit.unicode/${name}`, "--code", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  }
  const invalidAssertions = tests.filter((test) => failureNames.includes(test.name)).flatMap((test) => test.code.__edn_quote.slice(1));
  for (const [index, assertion] of invalidAssertions.entries()) {
    assert.equal(assertion[0], "assert=");
    assert.equal(assertion[2][0], "try");
    const name = `wasm-invalid-${index}`;
    run(snapshot, "edit", "def", `calcit.unicode/${name}`, "--input-format", "json-ast", "--code",
      JSON.stringify(["defwasm-export", name, [], assertion[2][1], "&unit"]));
    run(snapshot, "edit", "schema", `calcit.unicode/${name}`, "--code", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  }
  // Host f64 values include NaN/infinities that have no ordinary source literal.
  const hostOperations = [["&str:nth", "|😀", "index"], ["&str:contains?", "|😀", "index"], ["&str:slice", "|😀", "0", "index"]];
  for (const [index, operation] of hostOperations.entries()) {
    const name = `wasm-host-${index}`;
    run(snapshot, "edit", "def", `calcit.unicode/${name}`, "--input-format", "json-ast", "--code",
      JSON.stringify(["defwasm-export", name, ["index"], operation, "&unit"]));
    run(snapshot, "edit", "schema", `calcit.unicode/${name}`, "--code", "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Unit)");
  }
  run("wasm", snapshot, "--init-fn", "calcit.unicode/wasm-case-0", "--reload-fn", "calcit.unicode/wasm-case-0", "--emit-path", fixture);
  const module = new WebAssembly.Module(await readFile(join(fixture, "program.wasm")));
  const imports = {};
  const wasmTrace = [];
  let wasm;
  for (const { module: namespace, name, kind } of WebAssembly.Module.imports(module)) {
    assert.equal(kind, "function");
    (imports[namespace] ??= {})[name] = () => { throw new Error(`Unexpected host call: ${namespace}.${name}`); };
  }
  const captureString = (ptr) => {
    const memory = new DataView(wasm.exports.memory.buffer);
    const length = memory.getFloat64(ptr, true);
    const text = new TextDecoder("utf-8", { fatal: true }).decode(new Uint8Array(memory.buffer, ptr + 8, length));
    assert.ok(expectedTrace.includes(text), `unexpected WASM output: ${text}`);
    wasmTrace.push(text);
  };
  if (imports.io?.log_value) imports.io.log_value = captureString;
  if (imports.io?.log_str) imports.io.log_str = captureString;
  wasm = new WebAssembly.Instance(module, imports);
  for (const [index, assertion] of assertions.entries()) {
    assert.doesNotThrow(() => wasm.exports[`wasm-case-${index}`](), JSON.stringify(assertion));
  }
  for (const [index, assertion] of invalidAssertions.entries()) {
    assert.throws(() => wasm.exports[`wasm-invalid-${index}`](), WebAssembly.RuntimeError, JSON.stringify(assertion));
  }
  assert.deepEqual(wasmTrace, expectedTrace, "WASM must preserve eager argument order and single evaluation");
  for (const index of hostOperations.keys()) {
    for (const value of [-1, 0.5, NaN, Infinity, -Infinity]) {
      assert.throws(() => wasm.exports[`wasm-host-${index}`](value), WebAssembly.RuntimeError);
    }
    assert.doesNotThrow(() => wasm.exports[`wasm-host-${index}`](1e100));
  }
  console.log("Unicode definition tests passed on generated WASM");
} finally {
  await rm(fixture, { recursive: true, force: true });
}
