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
    ["([] 1) .len &", false, true],
    [".len ([] 1) &", false, true],
    ["({} (:a 1)) .dissoc :a & & ([] :b)", false, true],
  ];
  for (const [expression, isSet, checkCompatibility] of bad) {
    run("edit", "def", "calcit.spread-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defwasm-export run-tests () (${expression})`);
    const modes = [[], ["--check-only"], ["js"], ["wasm"]];
    if (checkCompatibility) modes.push(...modes.map(mode => ["--compat-types", ...mode]));
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
  // Compatibility keeps this explicitly open boundary reachable. Native/JS
  // validate its value at runtime; WASM must check the heap kind before reads.
  const opaque = [
    [["[]", ":b"], 1],
    [["[]"], 2],
    [["{}", [":bad", "1"]], undefined],
    [["#{}", ":b"], undefined],
    ["0", undefined],
    ["16392.5", undefined],
  ];
  for (let index = 0; index < opaque.length; index++) {
    const [body, expected] = opaque[index];
    const remaining = [["{}", [":a", "1"], [":b", "2"], [":c", "3"]], ".dissoc", ":a", "&", ["dynamic-spread-keys"]];
    // Observe the Map using its existing count proc: compatibility mode does
    // not retain enough result evidence for `.len` in this opaque ABI fixture.
    const count = ["&map:count", remaining];
    const result = expected === undefined ? count : ["let", [["result", count]], ["assert=", String(expected), "result"], "result"];
    const operations = [
      ["edit", "def", "calcit.spread-evidence/dynamic-spread-keys", ...(index === 0 ? [] : ["--overwrite"]), "--input-format", "json-ast", "--code",
        JSON.stringify(["defn", "dynamic-spread-keys", [], body])],
      ["edit", "schema", "calcit.spread-evidence/dynamic-spread-keys", "--input-format", "cirru", "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Dynamic)"],
      ["edit", "def", "calcit.spread-evidence/run-tests", "--overwrite", "--input-format", "json-ast", "--code",
        JSON.stringify(["defwasm-export", "run-tests", [], result])],
    ];
    const transaction = JSON.stringify(operations);
    const preview = JSON.parse(run("edit", "transaction", "--code", transaction, "--dry-run", "--format", "json"));
    run("edit", "transaction", "--code", transaction, "--expect-revision", preview.scoped_revision);
    const native = spawnSync(binary, ["--compat-types", snapshot], options);
    assert.ifError(native.error);
    assert.equal(native.status, expected === undefined ? 1 : 0, native.stderr);
    if (expected === undefined) assert.match(native.stderr, /expected list for spreading/);
    const opaqueJs = join(project, `opaque-js-${index}`);
    run("--compat-types", "--emit-path", opaqueJs, "js");
    const generated = await import(pathToFileURL(join(opaqueJs, "calcit.spread-evidence.mjs")).href);
    if (expected === undefined) assert.throws(() => generated.run_tests());
    else assert.equal(generated.run_tests(), expected);
    const opaqueWasm = join(project, `opaque-wasm-${index}`);
    run("--compat-types", "--emit-path", opaqueWasm, "wasm");
    const module = new WebAssembly.Module(await readFile(join(opaqueWasm, "program.wasm")));
    const imports = {};
    for (const { module: namespace, name, kind } of WebAssembly.Module.imports(module)) {
      assert.equal(kind, "function");
      (imports[namespace] ??= {})[name] = () => { throw new Error(`Unexpected host call: ${namespace}.${name}`); };
    }
    const instance = new WebAssembly.Instance(module, imports);
    if (expected === undefined) assert.throws(() => instance.exports["run-tests"](), WebAssembly.RuntimeError);
    else assert.equal(instance.exports["run-tests"](), expected);
  }
  console.log("List spreads, malformed markers and opaque runtime values retain native/JS/WASM contracts");
} finally {
  await rm(project, { recursive: true, force: true });
}
