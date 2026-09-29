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
const core = JSON.parse(run("cirru", "parse-edn", "--file", corePath));
const definitions = core[":files"]["'calcit.core"].defs;
const methodTest = definitions["'contains?"].tests.find(item => item.name === "distinguishes-explicit-index-key-and-value-methods");
const primitiveTest = definitions["'&list:contains?"].tests.find(item => item.name === "checks-list-index-bounds");
const setUpdateTest = definitions["'include"].tests.find(item => item.name === "keeps-persistent-set-add-and-include-equivalent");
const enumIndexTest = definitions["'contains-index?"].tests.find(item => item.name === "checks-enum-position-and-legacy-fractional-boundary");
assert.ok(methodTest, "the naming contract must remain attached to calcit.core/contains?");
assert.ok(primitiveTest, "the direct primitive boundary must remain attached to calcit.core/&list:contains?");
assert.ok(setUpdateTest, "the persistent Set update contract must remain attached to calcit.core/include");
assert.ok(enumIndexTest, "the Enum index contract must remain attached to calcit.core/contains-index?");
const tests = [methodTest, primitiveTest, setUpdateTest, enumIndexTest];

const fixture = await mkdtemp(join(tmpdir(), "calcit-predicate-method-names-"));
try {
  const snapshot = join(fixture, "calcit.cirru");
  await copyFile(corePath, snapshot);
  await symlink(resolve("node_modules"), join(fixture, "node_modules"), "dir");
  run(snapshot, "query", "config");
  run(snapshot, "edit", "add-ns", "calcit.predicate-method-names");
  run(snapshot, "edit", "def", "calcit.predicate-method-names/run-tests", "--input-format", "json-ast", "--code",
    JSON.stringify(["defwasm-export", "run-tests", [], ...tests.map(test => test.code.__edn_quote), "&unit"]));
  run(snapshot, "edit", "schema", "calcit.predicate-method-names/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  const entry = [snapshot, "--init-fn", "calcit.predicate-method-names/run-tests", "--reload-fn", "calcit.predicate-method-names/run-tests"];
  run(...entry);

  const output = join(fixture, "js-out");
  run(...entry, "--emit-path", output, "js");
  const compiled = await import(pathToFileURL(join(output, "calcit.predicate-method-names.mjs")).href);
  compiled.run_tests();

  const traitSnapshot = resolve("calcit/test-traits.cirru");
  run(traitSnapshot, "test", "test-traits.main/test-qualified-contains-boundary", "--require-match");
  const genericFix = JSON.parse(run(traitSnapshot, "fix", "--rule", "core-predicate-method-v1",
    "--ns", "test-traits.main", "--def", "contains-with-trait?", "--format", "json"));
  assert.equal(genericFix.data.suggestions.some((item) => item.applicability === "machine-applicable"), false,
    "predicate migration must not rewrite a method call on a trait-bound generic receiver");
  const traitOutput = join(fixture, "trait-js");
  run(traitSnapshot, "--emit-path", traitOutput, "js");
  const traits = await import(pathToFileURL(join(traitOutput, "test-traits.main.mjs")).href);
  assert.equal(traits.test_qualified_contains_boundary(), true,
    "a generic Contains bound must select its nominal trait, even when the receiver has another .contains? method");

  run("wasm", ...entry, "--emit-path", fixture);
  const module = new WebAssembly.Module(await readFile(join(fixture, "program.wasm")));
  const imports = {};
  for (const { module: namespace, name, kind } of WebAssembly.Module.imports(module)) {
    assert.equal(kind, "function");
    (imports[namespace] ??= {})[name] = () => { throw new Error(`Unexpected host call: ${namespace}.${name}`); };
  }
  const wasm = new WebAssembly.Instance(module, imports);
  assert.equal(typeof wasm.exports["run-tests"], "function");
  wasm.exports["run-tests"]();
  console.log("Core method naming and trait-bound predicate tests passed on native / generated JS / core WASM");

  if (process.env.WASMTIME_CLI) {
    run(snapshot, "tree", "search-replace", "calcit.predicate-method-names/run-tests", "--pattern", "defwasm-export",
      "--input-format", "cirru", "--code", "quote defn");
    const component = join(fixture, "component");
    run("wasi", ...entry, "--emit-path", component);
    execFileSync(process.env.WASMTIME_CLI, [
      "run", "-S", "p3", "-W", "component-model-async-stackful=y",
      "-W", "component-model-more-async-builtins=y", join(component, "program.wasm"),
    ], { encoding: "utf8", timeout: 60000 });
    console.log("Core method naming definition tests passed on WASI 0.3 Component / Wasmtime");
  }
} finally {
  await rm(fixture, { recursive: true, force: true });
}
