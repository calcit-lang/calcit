import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFile, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const run = (...args) => execFileSync(binary, args, {
  encoding: "utf8", stdio: "pipe", timeout: 60000, maxBuffer: 16 * 1024 * 1024,
});
const traitSnapshot = resolve("calcit/test-traits.cirru");

run(traitSnapshot, "test", "test-traits.main/add-with-trait", "--require-match");
run(traitSnapshot);

const fixture = await mkdtemp(join(tmpdir(), "calcit-map-add-trait-"));
try {
  await symlink(resolve("node_modules"), join(fixture, "node_modules"), "dir");
  const output = join(fixture, "js-out");
  run(traitSnapshot, "--emit-path", output, "js");
  const traits = await import(pathToFileURL(join(output, "test-traits.main.mjs")).href);
  traits.main_$x_();

  const snapshot = join(fixture, "calcit.cirru");
  await copyFile(traitSnapshot, snapshot);
  run(snapshot, "edit", "def", "test-traits.main/invalid-map-add-trait", "--input-format", "cirru", "--code",
    "quote $ defn invalid-map-add-trait ()\n  add-with-trait (&{} :a 1) (&{} :b 2)");
  let rejected = false;
  try {
    run(snapshot, "--init-fn", "test-traits.main/invalid-map-add-trait", "--reload-fn",
      "test-traits.main/invalid-map-add-trait", "--check-only");
  } catch (error) {
    const detail = `${error.stdout ?? ""}\n${error.stderr ?? ""}`;
    assert.match(detail, /W_GENERIC_WHERE_BOUND_MISMATCH.*trait Add/,
      "a Map must fail the generic Add bound, not an unrelated check");
    rejected = true;
  }
  assert.ok(rejected, "legacy Map .add must not satisfy the nominal Add trait bound");

  // Replay every runtime fixture through strict source checking here.
  // The existing strict-default runner owns explicit compatibility testing.
  const definition = JSON.parse(run(traitSnapshot, "query", "def",
    "test-traits.main/test-explicit-trait-call", "--format", "json"));
  const runtimeTests = definition.data.tests.filter(test => test.tags.includes("trait-runtime"));
  assert.equal(runtimeTests.length, 6);
  const setRuntimeBody = tests => run(snapshot, "edit", "def", "test-traits.main/test-explicit-trait-call", "--overwrite",
    "--input-format", "json-ast", "--code", JSON.stringify([
      "defn", "test-explicit-trait-call", [], ...tests.map(test => test.code), "1",
    ]));
  const selection = ["--init-fn", "test-traits.main/test-explicit-trait-call",
    "--reload-fn", "test-traits.main/test-explicit-trait-call"];
  for (const test of runtimeTests) {
    setRuntimeBody([test]);
    for (const mode of [[], ["--check-only"], ["js"], ["wasm"], ["wasi"]]) {
      assert.throws(() => run(snapshot, ...selection, ...mode), error => {
        assert.match(`${error.stdout ?? ""}\n${error.stderr ?? ""}`, /E_DUPLICATE_TRAIT_IMPL/);
        return true;
      }, `${test.name}: strict selection must reject duplicate origins before execution or codegen`);
    }
  }
  console.log("Map .add / Add trait boundary passed on native, generated JS, and strict negative checking");
} finally {
  await rm(fixture, { recursive: true, force: true });
}
