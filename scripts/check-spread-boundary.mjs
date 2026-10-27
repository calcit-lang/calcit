import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const project = await mkdtemp(join(tmpdir(), "calcit-spread-boundary-"));
const snapshot = join(project, "calcit.cirru");
const output = join(project, "js-out");
const options = { encoding: "utf8", stdio: "pipe", timeout: 60000, maxBuffer: 16 * 1024 * 1024 };
const run = (...args) => execFileSync(binary, [snapshot, ...args], options);

try {
  await copyFile("src/cirru/calcit-core.cirru", snapshot);
  await symlink(resolve("node_modules"), join(project, "node_modules"), "dir");
  run("test", "--tag", "spread-boundary", "--require-match");
  run("test", "--tag", "spread-proof", "--require-match");
  const response = JSON.parse(run("query", "def", "calcit.core/dissoc", "--format", "json"));
  assert.deepEqual(response.diagnostics, []);
  const tests = response.data.tests.filter(test => test.tags.includes("spread-boundary") || test.tags.includes("spread-proof"));
  assert.equal(tests.length, 9);
  run("edit", "add-ns", "calcit.spread-evidence");
  run("edit", "def", "calcit.spread-evidence/run-tests", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "run-tests", [], ...tests.map(test => test.code), "1"]));
  run("edit", "schema", "calcit.spread-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)");
  run("config", "set", "init-fn", "calcit.spread-evidence/run-tests");
  run("config", "set", "reload-fn", "calcit.spread-evidence/run-tests");
  // The transplanted core tests are not project code the default proof audits.
  run("--no-proof-audit");
  run("--no-proof-audit", "--emit-path", output, "js");
  const generated = await import(pathToFileURL(join(output, "calcit.spread-evidence.mjs")).href);
  assert.equal(generated.run_tests(), 1);

  run("edit", "def", "calcit.spread-evidence/fixed-spread", "--input-format", "cirru", "--code",
    "quote $ defwasm-export fixed-spread ()\n  let\n      f $ fn (a b)\n        hint-fn $ {} (:args $ [] 'Number 'Number) (:return 'Number)\n        + a b\n    f & $ [] 1 2");
  run("edit", "schema", "calcit.spread-evidence/fixed-spread", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)");
  const selector = ["fix", "--rule", "spread-call-proof-v1", "--ns", "calcit.spread-evidence", "--def", "fixed-spread", "--format", "json"];
  const preview = JSON.parse(run(...selector));
  assert.equal(preview.data.suggestions.length, 1);
  assert.equal(preview.data.suggestions[0].applicability, "machine-applicable");
  run(...selector, "--apply", "--allow-no-vcs", "--expect-revision", preview.revision);
  assert.equal(JSON.parse(run(...selector)).data.suggestions.length, 0);
  run("edit", "add-test", "calcit.spread-evidence/fixed-spread", "answer", "--tags", "unit", "--input-format", "cirru", "--code",
    "quote $ assert= 3 $ fixed-spread");
  run("test", "calcit.spread-evidence/fixed-spread", "--require-match");
  run("config", "set", "init-fn", "calcit.spread-evidence/fixed-spread");
  run("config", "set", "reload-fn", "calcit.spread-evidence/fixed-spread");
  const fixedJs = join(project, "fixed-js");
  run("--emit-path", fixedJs, "js");
  assert.equal((await import(pathToFileURL(join(fixedJs, "calcit.spread-evidence.mjs")).href)).fixed_spread(), 3);
  // WASM gathers project functions, so keep the native/JS Ref tests out of
  // this separate numeric fixture rather than claiming unsupported Ref parity.
  const wasmSnapshot = join(project, "wasm-snapshot.cirru");
  await copyFile(snapshot, wasmSnapshot);
  execFileSync(binary, [wasmSnapshot, "edit", "rm-def", "calcit.spread-evidence/run-tests"], options);
  execFileSync(binary, [wasmSnapshot, "--emit-path", output, "wasm"], options);
  const module = new WebAssembly.Module(await readFile(join(output, "program.wasm")));
  const imports = {};
  for (const { module: namespace, name, kind } of WebAssembly.Module.imports(module)) {
    assert.equal(kind, "function");
    (imports[namespace] ??= {})[name] = () => { throw new Error(`Unexpected host call: ${namespace}.${name}`); };
  }
  assert.equal(new WebAssembly.Instance(module, imports).exports["fixed-spread"](), 3);
  run("config", "set", "init-fn", "calcit.spread-evidence/run-tests");
  run("config", "set", "reload-fn", "calcit.spread-evidence/run-tests");

  // Failed programs cannot reach attached test execution. Check their CLI
  // diagnostics before native execution and both code-generation paths.
  const bad = [
    ["dissoc ({} (:a 1)) & (#{} :a)", true],
    ["let ((keys (#{} :a))) (dissoc ({} (:a 1)) & keys)", true],
    ["dissoc ({} (:a 1)) & (filter (distinct-values ({} (:id :a))) (fn (key) (= key :a)))", true],
    ["dissoc ({} (:a 1)) & 1", false],
    ["dissoc ({} (:a 1)) & |a", false],
    ["dissoc ({} (:a 1)) & ({} (:a 1))", false],
    ["dissoc ({} (:a 1)) & (Option :some :a)", false],
    ["&map:dissoc ({} (:a 1)) & (#{} :a)", true],
    ["let ((keys (distinct-values ({} (:id :a))))) (dissoc ({} (:a 1)) & keys)", true],
    ["let ((f (fn (x) x))) (f & (#{} 1))", true],
    ["&call-spread dissoc ({} (:a 1)) & (#{} :a)", true],
    ["dissoc ({} (:a 1) (:b 2)) & ([] :a) & (#{} :b)", true],
    ["({} (:a 1) (:b 2)) .dissoc :a & (#{} :b)", true],
    ["({} (:a 1) (:b 2)) .dissoc :a & 1", false],
    ["([] 1) .concat ([] 2) & (#{} ([] 3))", true],
    ["([] 1) .len &", false],
    [".len ([] 1) &", false],
    ["({} (:a 1)) .dissoc :a & & ([] :b)", false],
  ];
  for (const [expression, isSet] of bad) {
    run("edit", "def", "calcit.spread-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defwasm-export run-tests () (${expression})`);
    const modes = [[], ["--check-only"], ["js"], ["wasm"]];
    for (const mode of modes) {
      const result = spawnSync(binary, ["--emit-path", output, snapshot, ...mode], options);
      if (result.error) throw result.error;
      assert.equal(result.status, 1, `${expression} ${mode}\n${result.stdout}\n${result.stderr}`);
      assert.ok(result.stderr.includes("E_SPREAD_TYPE_MISMATCH"), `${expression}\n${result.stderr}`);
      assert.ok(result.stderr.includes("calcit.spread-evidence/run-tests"), result.stderr);
      assert.ok(result.stderr.includes("expected") && result.stderr.includes("got"), result.stderr);
      assert.equal(result.stderr.includes(".to-list"), isSet, result.stderr);
      if (!isSet) assert.ok(result.stderr.includes("pass a List"), result.stderr);
      assert.ok(result.stderr.includes("preprocessing"), result.stderr);
    }
  }
  // Invalid call arity cannot reach definition tests. Literal expansion must
  // reject extra operands before ordinary emission can leave an invalid stack.
  run("edit", "def", "calcit.spread-evidence/sink", "--input-format", "cirru", "--code",
    "quote $ defn sink (n) n");
  run("edit", "schema", "calcit.spread-evidence/sink", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)");
  for (const [index, expression] of [
    "sink & ([]) 6 7",
    "let ((call sink)) (call & ([]) 6 7)",
  ].entries()) {
    const transaction = JSON.stringify([["edit", "def", "calcit.spread-evidence/run-tests", "--overwrite",
      "--input-format", "cirru", "--code", `quote $ defwasm-export run-tests () (${expression})`]]);
    const preview = JSON.parse(run("edit", "transaction", "--code", transaction, "--dry-run", "--format", "json"));
    run("edit", "transaction", "--code", transaction, "--expect-revision", preview.scoped_revision);
    const native = spawnSync(binary, [snapshot], options);
    assert.ifError(native.error);
    assert.equal(native.status, 1, native.stdout + native.stderr);
    assert.match(native.stderr, /function arity mismatch.*expected 1 argument\(s\), got 2/);
    const invalidOutput = join(project, `invalid-arity-${index}`);
    const rejected = spawnSync(binary, [snapshot, "--emit-path", invalidOutput, "wasm"], options);
    assert.ifError(rejected.error);
    assert.equal(rejected.status, 1, rejected.stdout + rejected.stderr);
    assert.match(rejected.stderr, /E_WASM_CALL_SPECIALIZATION:.*expects at most 1 args, got 2/);
    await assert.rejects(readFile(join(invalidOutput, "program.wasm")), { code: "ENOENT" });
  }
  console.log("List spreads, malformed markers and literal-call arity retain native/JS/WASM contracts");
} finally {
  await rm(project, { recursive: true, force: true });
}
