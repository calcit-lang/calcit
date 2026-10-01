import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const project = await mkdtemp(join(tmpdir(), "calcit-known-assertion-"));
const snapshot = join(project, "calcit.cirru");
const options = { encoding: "utf8", stdio: "pipe", timeout: 60000, maxBuffer: 16 * 1024 * 1024 };
const run = (...args) => execFileSync(binary, [snapshot, ...args], options);

try {
  await copyFile("src/cirru/calcit-core.cirru", snapshot);
  await symlink(resolve("node_modules"), join(project, "node_modules"), "dir");
  run("test", "--tag", "assert-boundary", "--require-match");
  const response = JSON.parse(run("query", "def", "calcit.core/assert-type", "--format", "json"));
  assert.deepEqual(response.diagnostics, []);
  const tests = response.data.tests.filter(test => test.tags.includes("assert-boundary"));
  assert.equal(tests.length, 6);
  run("edit", "add-ns", "calcit.assert-evidence");
  const setBody = trees => run("edit", "def", "calcit.assert-evidence/run-tests", "--overwrite",
    "--input-format", "json-ast", "--code", JSON.stringify(["defwasm-export", "run-tests", [], ...trees, "1"]));
  setBody(tests.map(test => test.code));
  run("edit", "schema", "calcit.assert-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)");
  run("config", "set", "init-fn", "calcit.assert-evidence/run-tests");
  run("config", "set", "reload-fn", "calcit.assert-evidence/run-tests");
  run();
  const output = join(project, "js-out");
  run("--emit-path", output, "js");
  const generated = await import(pathToFileURL(join(output, "calcit.assert-evidence.mjs")).href);
  assert.equal(generated.run_tests(), 1);

  // Only replay the already-supported scalar subset in core WASM.
  const scalar = tests.filter(test => test.tags.includes("assert-scalar-wasm"));
  assert.equal(scalar.length, 4);
  setBody(scalar.map(test => test.code));
  run("wasm", "--emit-path", project);
  const module = new WebAssembly.Module(await readFile(join(project, "program.wasm")));
  const imports = {};
  for (const item of WebAssembly.Module.imports(module)) {
    assert.equal(item.kind, "function");
    (imports[item.module] ??= {})[item.name] = () => { throw new Error(`Unexpected host call: ${item.name}`); };
  }
  const wasm = new WebAssembly.Instance(module, imports);
  assert.equal(wasm.exports["run-tests"](), 1);

  // The shared preprocessor must reject before execution or either codegen.
  const bad = [
    "let ((x |hello)) (assert-type x 'Number)",
    "let ((x 3)) (assert-type x 'String)",
    "let ((x ([] 1 2))) (assert-type x (:: 'List 'String))",
    "let ((x (Option :some 3))) (assert-type x (:: 'Option 'String))",
    "assert-type |hello 'Number",
    "assert-type 3 'String",
    "assert-type (+ 1 2) 'String",
    "assert-type ([] 1 2) (:: 'List 'String)",
    "assert-type (Option :some 3) (:: 'Option 'String)",
    "assert-type (Option :some 3) (:: 'Result 'Number 'String)",
  ];
  for (const expression of bad) {
    run("edit", "def", "calcit.assert-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defwasm-export run-tests () (${expression})`);
    for (const mode of [[], ["--check-only"], ["js"], ["wasm"]]) {
      const result = spawnSync(binary, ["--emit-path", output, snapshot, ...mode], options);
      if (result.error) throw result.error;
      assert.equal(result.status, 1, `${expression} ${mode}\n${result.stdout}\n${result.stderr}`);
      assert.ok(result.stderr.includes("E_ASSERT_TYPE_MISMATCH"), result.stderr);
      assert.ok(result.stderr.includes("calcit.assert-evidence/run-tests"), result.stderr);
      assert.ok(result.stderr.includes("expected") && result.stderr.includes("got"), result.stderr);
      assert.ok(result.stderr.includes("preprocessing"), result.stderr);
    }
  }
  console.log("Known incompatible local and expression assertions rejected before native/JS/WASM; shared positive tests passed");
} finally {
  await rm(project, { recursive: true, force: true });
}
