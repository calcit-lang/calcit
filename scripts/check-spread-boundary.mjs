import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, rm, symlink } from "node:fs/promises";
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
  const response = JSON.parse(run("query", "def", "calcit.core/dissoc", "--format", "json"));
  assert.deepEqual(response.diagnostics, []);
  const tests = response.data.tests.filter(test => test.tags.includes("spread-boundary"));
  assert.equal(tests.length, 7);
  run("edit", "add-ns", "calcit.spread-evidence");
  run("edit", "def", "calcit.spread-evidence/run-tests", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "run-tests", [], ...tests.map(test => test.code), "1"]));
  run("edit", "schema", "calcit.spread-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)");
  run("config", "set", "init-fn", "calcit.spread-evidence/run-tests");
  run("config", "set", "reload-fn", "calcit.spread-evidence/run-tests");
  run();
  run("--emit-path", output, "js");
  const generated = await import(pathToFileURL(join(output, "calcit.spread-evidence.mjs")).href);
  assert.equal(generated.run_tests(), 1);

  // Failed programs cannot reach attached test execution. Check their CLI
  // diagnostics before native execution and both code-generation paths.
  const bad = [
    "dissoc ({} (:a 1)) & (#{} :a)",
    "let ((keys (#{} :a))) (dissoc ({} (:a 1)) & keys)",
    "dissoc ({} (:a 1)) & (filter (vals ({} (:id :a))) (fn (key) (= key :a)))",
    "dissoc ({} (:a 1)) & 1",
    "dissoc ({} (:a 1)) & |a",
    "dissoc ({} (:a 1)) & ({} (:a 1))",
    "dissoc ({} (:a 1)) & (Option :some :a)",
    "&map:dissoc ({} (:a 1)) & (#{} :a)",
    "let ((keys (vals ({} (:id :a))))) (dissoc ({} (:a 1)) & keys)",
    "let ((f (fn (x) x))) (f & (#{} 1))",
    "&call-spread dissoc ({} (:a 1)) & (#{} :a)",
    "dissoc ({} (:a 1) (:b 2)) & ([] :a) & (#{} :b)",
  ];
  for (const expression of bad) {
    run("edit", "def", "calcit.spread-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defwasm-export run-tests () (${expression})`);
    for (const mode of [[], ["--check-only"], ["--compat-types", "--check-only"], ["js"], ["wasm"]]) {
      const result = spawnSync(binary, ["--emit-path", output, snapshot, ...mode], options);
      if (result.error) throw result.error;
      assert.equal(result.status, 1, `${expression} ${mode}\n${result.stdout}\n${result.stderr}`);
      assert.ok(result.stderr.includes("E_SPREAD_TYPE_MISMATCH"), `${expression}\n${result.stderr}`);
      assert.ok(result.stderr.includes("calcit.spread-evidence/run-tests"), result.stderr);
      assert.ok(result.stderr.includes("expected") && result.stderr.includes("got"), result.stderr);
      assert.ok(result.stderr.includes(".to-list"), result.stderr);
      assert.ok(result.stderr.includes("preprocessing"), result.stderr);
    }
  }
  console.log("Known non-List spreads rejected before native/JS/WASM; shared native/JS List tests passed");
} finally {
  await rm(project, { recursive: true, force: true });
}
