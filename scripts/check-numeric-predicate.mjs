import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const run = (...args) => execFileSync(binary, args, {
  encoding: "utf8", stdio: "pipe", timeout: 60000, maxBuffer: 16 * 1024 * 1024,
});
const corePath = resolve("src/cirru/calcit-core.cirru");
// Replay the authoritative Calcit tests, including method calls, on each target.
const core = JSON.parse(run("cirru", "parse-edn", "--file", corePath));
const definitions = core[":files"]["'calcit.core"].defs;
const roundTests = definitions["'round?"].tests;
const integerTests = definitions["'integer?"].tests;
assert.deepEqual(roundTests.map(test => test.name).sort(), [
  "distinguishes-integers", "finite-exact-integer-boundaries", "integer-method-and-evaluation",
]);
assert.deepEqual(integerTests.map(test => test.name), ["integer-alias-boundaries-and-evaluation"]);
// Preserve the existing refinement assertions verbatim, including Result payload types.
const refinementTests = ["int8", "int16", "int32", "int64", "uint8", "uint16", "uint32", "uint64", "float32", "float64"]
  .flatMap(type => {
    const name = `number->${type}`;
    const attached = definitions[`'${name}`]?.tests;
    assert.ok(attached, `${name} must retain definition-attached tests`);
    assert.deepEqual(attached.map(test => test.name), ["checks-boundaries-and-type"],
      `${name} must retain its reviewed success, rejection and type assertions`);
    return attached;
  });
const remTests = definitions["'&number:rem"].tests;
assert.deepEqual(remTests.map(test => test.name), ["calculates-truncated-remainder", "rejects-zero-and-non-safe-integers"]);
// WASM has no recoverable try; its rem traps are checked by scripts/test-wasm.mjs instead.
const predicateTests = [...roundTests, ...integerTests, remTests[0]];
const tests = [...predicateTests, ...refinementTests, remTests[1]];
const expectedTrace = [
  "integer-free-argument", "integer-method-argument", "integer-alias-free-argument", "integer-alias-method-argument",
];
const fixture = await mkdtemp(join(tmpdir(), "calcit-numeric-predicate-"));
try {
  const snapshot = join(fixture, "calcit.cirru");
  await copyFile(corePath, snapshot);
  await symlink(resolve("node_modules"), join(fixture, "node_modules"), "dir");
  run(snapshot, "query", "config");
  run(snapshot, "edit", "add-ns", "calcit.numeric-predicate");
  run(snapshot, "edit", "def", "calcit.numeric-predicate/run-tests", "--input-format", "json-ast", "--code",
    JSON.stringify(["defwasm-export", "run-tests", [], ...tests.map(test => test.code.__edn_quote), "&unit"]));
  run(snapshot, "edit", "schema", "calcit.numeric-predicate/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  const entry = [snapshot, "--init-fn", "calcit.numeric-predicate/run-tests", "--reload-fn", "calcit.numeric-predicate/run-tests"];
  assert.deepEqual(run(...entry).trim().split(/\r?\n/), expectedTrace);

  const output = join(fixture, "js-out");
  run(...entry, "--emit-path", output, "js");
  const compiled = await import(pathToFileURL(join(output, "calcit.numeric-predicate.mjs")).href);
  const jsTrace = [];
  const originalLog = console.log;
  try {
    console.log = (...values) => jsTrace.push(values.join(" "));
    compiled.run_tests();
  } finally {
    console.log = originalLog;
  }
  assert.deepEqual(jsTrace, expectedTrace, "JS must evaluate each predicate argument exactly once");
  console.log("Numeric predicate, refinement and remainder definition tests passed on native / generated JS");

  // The Int8 test also uses a throwing EDN parser unavailable in WASM.
  // Keep its full AST on native/JS; do not strip assertions to claim target parity.
  run(snapshot, "tree", "show", "calcit.numeric-predicate/run-tests");
  run(snapshot, "edit", "def", "calcit.numeric-predicate/run-tests", "--overwrite",
    "--input-format", "json-ast", "--code",
    JSON.stringify(["defwasm-export", "run-tests", [], ...predicateTests.map(test => test.code.__edn_quote), "&unit"]));
  run("wasm", ...entry, "--emit-path", fixture);
  const module = new WebAssembly.Module(await readFile(join(fixture, "program.wasm")));
  const imports = {};
  for (const { module: namespace, name, kind } of WebAssembly.Module.imports(module)) {
    assert.equal(kind, "function");
    (imports[namespace] ??= {})[name] = () => { throw new Error(`Unexpected host call: ${namespace}.${name}`); };
  }
  const wasmTrace = [];
  let wasm;
  const captureString = ptr => {
    const memory = new DataView(wasm.exports.memory.buffer);
    const length = memory.getFloat64(ptr, true);
    wasmTrace.push(new TextDecoder("utf-8", { fatal: true }).decode(new Uint8Array(memory.buffer, ptr + 8, length)));
  };
  if (imports.io?.log_value) imports.io.log_value = captureString;
  if (imports.io?.log_str) imports.io.log_str = captureString;
  wasm = new WebAssembly.Instance(module, imports);
  assert.equal(typeof wasm.exports["run-tests"], "function");
  wasm.exports["run-tests"]();
  assert.deepEqual(wasmTrace, expectedTrace, "WASM must evaluate each predicate argument exactly once");
  console.log("Numeric predicate and remainder definition tests passed on core WASM; refinement conversion and remainder error tests are native/JS only");

  // The Component CI job supplies the pinned Wasmtime executable explicitly.
  if (process.env.WASMTIME_CLI) {
    // A WASI command owns its entry export; preserve the same test body.
    run(snapshot, "tree", "show", "calcit.numeric-predicate/run-tests");
    run(snapshot, "tree", "search-replace", "calcit.numeric-predicate/run-tests", "--pattern", "defwasm-export",
      "--input-format", "cirru", "--code", "quote defn");
    const component = join(fixture, "component");
    run("wasi", ...entry, "--emit-path", component);
    const result = execFileSync(process.env.WASMTIME_CLI, [
      "run", "-S", "p3", "-W", "component-model-async-stackful=y",
      "-W", "component-model-more-async-builtins=y", join(component, "program.wasm"),
    ], { encoding: "utf8", timeout: 60000 });
    assert.deepEqual(result.trim().split(/\r?\n/), expectedTrace);
    console.log("Numeric predicate definition tests passed on WASI 0.3 Component / Wasmtime");
  }
} finally {
  await rm(fixture, { recursive: true, force: true });
}
