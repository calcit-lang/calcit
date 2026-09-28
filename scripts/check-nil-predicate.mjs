import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const run = (...args) => execFileSync(binary, args, {
  encoding: "utf8", stdio: "pipe", timeout: 60000, maxBuffer: 16 * 1024 * 1024,
});
const corePath = resolve("src/cirru/calcit-core.cirru");
const core = JSON.parse(run("cirru", "parse-edn", "--file", corePath));
const legacyTests = core[":files"]["'calcit.core"].defs["'some?"].tests;
assert.deepEqual(legacyTests.map(test => test.name).sort(), [
  "non-nil-is-not-option-variant", "typed-callers-preserve-non-nil-semantics",
]);
const canonicalTests = core[":files"]["'calcit.core"].defs["'non-nil?"].tests;
assert.deepEqual(canonicalTests.map(test => test.name).sort(), [
  "distinguishes-nil-from-values-and-option-variants", "typed-callers-preserve-non-nil-semantics",
]);
const tests = [...legacyTests, ...canonicalTests];

const fixture = await mkdtemp(join(tmpdir(), "calcit-nil-predicate-"));
try {
  const snapshot = join(fixture, "calcit.cirru");
  await copyFile(corePath, snapshot);
  await symlink(resolve("node_modules"), join(fixture, "node_modules"), "dir");
  run(snapshot, "query", "config");
  run(snapshot, "edit", "add-ns", "calcit.nil-predicate");
  run(snapshot, "edit", "add-ns", "calcit.nil-helper");
  run(snapshot, "edit", "def", "calcit.nil-helper/typed-bool", "--input-format", "cirru", "--code",
    "quote $ defn typed-bool (x)\n  some? x");
  run(snapshot, "edit", "schema", "calcit.nil-helper/typed-bool", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'Bool) (:return 'Bool)");
  run(snapshot, "edit", "def", "calcit.nil-helper/generic", "--input-format", "cirru", "--code",
    "quote $ defn generic (x)\n  some? x");
  run(snapshot, "edit", "schema", "calcit.nil-helper/generic", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'T) (:generics $ [] 'T) (:return 'Bool)");
  run(snapshot, "edit", "def", "calcit.nil-helper/generic-forward", "--input-format", "cirru", "--code",
    "quote $ defn generic-forward (x)\n  calcit.nil-helper/generic x");
  run(snapshot, "edit", "schema", "calcit.nil-helper/generic-forward", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'T) (:generics $ [] 'T) (:return 'Bool)");

  const extraAssertions = [
    ["assert=", "true", ["calcit.nil-helper/typed-bool", "false"]],
    ["assert=", "true", ["calcit.nil-helper/generic", "false"]],
    ["assert=", "true", ["calcit.nil-helper/generic", "0"]],
    ["assert=", "false", ["calcit.nil-helper/generic", "nil"]],
    ["assert=", "true", ["calcit.nil-helper/generic-forward", "false"]],
    ["assert=", "true", ["calcit.nil-helper/generic-forward", "0"]],
    ["assert=", "false", ["calcit.nil-helper/generic-forward", "nil"]],
    ["assert=", "true", ["calcit.nil-helper/generic-forward", ["Option", ":none"]]],
  ];
  run(snapshot, "edit", "def", "calcit.nil-predicate/run-tests", "--input-format", "json-ast", "--code",
    JSON.stringify(["defwasm-export", "run-tests", [], ...tests.map(test => test.code.__edn_quote), ...extraAssertions, "&unit"]));
  run(snapshot, "edit", "schema", "calcit.nil-predicate/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  const entry = [snapshot, "--init-fn", "calcit.nil-predicate/run-tests", "--reload-fn", "calcit.nil-predicate/run-tests"];
  run(...entry);

  const jsOutput = join(fixture, "js-out");
  run(...entry, "--emit-path", jsOutput, "js");
  const compiled = await import(pathToFileURL(join(jsOutput, "calcit.nil-predicate.mjs")).href);
  compiled.run_tests();

  run("wasm", ...entry, "--emit-path", fixture);
  const module = new WebAssembly.Module(await readFile(join(fixture, "program.wasm")));
  const imports = {};
  for (const { module: namespace, name, kind } of WebAssembly.Module.imports(module)) {
    assert.equal(kind, "function");
    (imports[namespace] ??= {})[name] = () => { throw new Error(`Unexpected host call: ${namespace}.${name}`); };
  }
  const wasm = new WebAssembly.Instance(module, imports);
  wasm.exports["run-tests"]();
  console.log("Nil predicate definition tests passed on native / generated JS / core WASM");

  const openSnapshot = join(fixture, "open.cirru");
  await copyFile(snapshot, openSnapshot);
  run(openSnapshot, "edit", "def", "calcit.nil-predicate/open-nil", "--input-format", "cirru", "--code",
    "quote $ defwasm-export open-nil (x)\n  non-nil? x");
  run(openSnapshot, "edit", "schema", "calcit.nil-predicate/open-nil", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'T) (:generics $ [] 'T) (:return 'Bool)");
  const rejected = spawnSync(binary, ["wasm", openSnapshot, ...entry.slice(1), "--emit-path", join(fixture, "open-output")], {
    encoding: "utf8", timeout: 60000,
  });
  assert.ifError(rejected.error);
  assert.notEqual(rejected.status, 0, "an unbound generic nil check must fail closed");
  assert.match(rejected.stderr, /E_WASM_NIL_TYPE_EVIDENCE/);
  assert.match(rejected.stderr, /open-nil/);

  const spreadSnapshot = join(fixture, "spread.cirru");
  await copyFile(snapshot, spreadSnapshot);
  run(spreadSnapshot, "edit", "def", "calcit.nil-predicate/generic-rest", "--input-format", "cirru", "--code",
    "quote $ defn generic-rest (x & xs)\n  non-nil? x");
  run(spreadSnapshot, "edit", "schema", "calcit.nil-predicate/generic-rest", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'T) (:generics $ [] 'T) (:rest 'T) (:return 'Bool)");
  run(spreadSnapshot, "edit", "def", "calcit.nil-predicate/spread-nil", "--input-format", "cirru", "--code",
    "quote $ defwasm-export spread-nil ()\n  generic-rest 0 & $ []");
  run(spreadSnapshot, "edit", "schema", "calcit.nil-predicate/spread-nil", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Bool)");
  const spreadRejected = spawnSync(binary, [
    "wasm", spreadSnapshot, ...entry.slice(1), "--emit-path", join(fixture, "spread-output"),
  ], { encoding: "utf8", timeout: 60000 });
  assert.ifError(spreadRejected.error);
  assert.notEqual(spreadRejected.status, 0, "a spread call must not bypass nil specialization");
  assert.match(spreadRejected.stderr, /E_WASM_NIL_TYPE_EVIDENCE/);
  assert.match(spreadRejected.stderr, /spread call/);

  const shadowSnapshot = join(fixture, "shadow.cirru");
  await copyFile(snapshot, shadowSnapshot);
  run(shadowSnapshot, "edit", "def", "calcit.nil-predicate/dynamic-number", "--input-format", "cirru", "--code",
    "quote $ defn dynamic-number () 0");
  run(shadowSnapshot, "edit", "schema", "calcit.nil-predicate/dynamic-number", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)");
  run(shadowSnapshot, "edit", "def", "calcit.nil-predicate/shadow-value", "--input-format", "json-ast", "--code",
    JSON.stringify(["defwasm-export", "shadow-value", ["x"], ["let", [
      ["f", ["fn", ["x"], ["non-nil?", "x"]]],
    ], ["f", ["dynamic-number"]]]]));
  run(shadowSnapshot, "edit", "schema", "calcit.nil-predicate/shadow-value", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'String) (:return 'Bool)");
  const shadowOutput = join(fixture, "shadow-output");
  run("wasm", shadowSnapshot, ...entry.slice(1), "--emit-path", shadowOutput);
  const shadowModule = new WebAssembly.Module(await readFile(join(shadowOutput, "program.wasm")));
  const shadowImports = {};
  for (const { module: namespace, name, kind } of WebAssembly.Module.imports(shadowModule)) {
    assert.equal(kind, "function");
    (shadowImports[namespace] ??= {})[name] = () => { throw new Error(`Unexpected host call: ${namespace}.${name}`); };
  }
  const shadowWasm = new WebAssembly.Instance(shadowModule, shadowImports);
  assert.equal(shadowWasm.exports["shadow-value"](0), 1,
    "an inline closure parameter must not inherit a same-named captured String type");

  if (process.env.WASMTIME_CLI) {
    run(snapshot, "tree", "show", "calcit.nil-predicate/run-tests");
    run(snapshot, "tree", "search-replace", "calcit.nil-predicate/run-tests", "--pattern", "defwasm-export",
      "--input-format", "cirru", "--code", "quote defn");
    const component = join(fixture, "component");
    run("wasi", ...entry, "--emit-path", component);
    execFileSync(process.env.WASMTIME_CLI, [
      "run", "-S", "p3", "-W", "component-model-async-stackful=y",
      "-W", "component-model-more-async-builtins=y", join(component, "program.wasm"),
    ], { encoding: "utf8", timeout: 60000 });
    console.log("Nil predicate definition tests passed on WASI 0.3 Component / Wasmtime");
  }
} finally {
  await rm(fixture, { recursive: true, force: true });
}
