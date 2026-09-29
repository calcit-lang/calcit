import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const run = (...args) => execFileSync(binary, args, { encoding: "utf8", stdio: "pipe", timeout: 60000 });
const rule = "core-identity-conversion-v1";
const fixture = await mkdtemp(join(tmpdir(), "calcit-identity-conversion-fix-"));

try {
  const snapshot = join(fixture, "calcit.cirru");
  await copyFile("calcit/test-wasm.cirru", snapshot);
  run(snapshot, "query", "config");
  run(snapshot, "edit", "add-ns", "test-wasm.conversion");
  for (const [name, code, schema] of [
    ["safe-tag", "quote $ defn safe-tag () $ turn-tag |ready", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Tag)"],
    ["safe-symbol", "quote $ defn safe-symbol () $ turn-symbol |ready", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Symbol)"],
    ["dynamic-tag", "quote $ defn dynamic-tag (value) $ turn-tag value", "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Tag)"],
    ["dynamic-symbol", "quote $ defn dynamic-symbol (value) $ turn-symbol value", "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Symbol)"],
    ["tag-input", "quote $ defn tag-input () $ turn-tag :ready", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Tag)"],
    ["macro-tag", "quote $ defn macro-tag () $ or (turn-tag |ready) :fallback", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Tag)"],
    ["quoted", "quote $ defn quoted () $ quote $ turn-tag |ready", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Dynamic)"],
  ]) {
    run(snapshot, "edit", "def", `test-wasm.conversion/${name}`, "--input-format", "cirru", "--code", code);
    run(snapshot, "edit", "schema", `test-wasm.conversion/${name}`, "--input-format", "cirru", "--code", schema);
  }
  run(snapshot, "edit", "add-test", "test-wasm.conversion/safe-tag", "preserves-tag", "--tags", "unit", "--input-format", "cirru", "--code", "quote $ assert= :ready $ safe-tag");
  run(snapshot, "edit", "add-test", "test-wasm.conversion/safe-symbol", "preserves-symbol", "--tags", "unit", "--input-format", "cirru", "--code", "quote $ assert= |ready $ turn-string $ safe-symbol");
  run(snapshot, "test", "test-wasm.conversion/safe-tag", "--require-match");
  run(snapshot, "test", "test-wasm.conversion/safe-symbol", "--require-match");
  const before = await readFile(snapshot, "utf8");
  const preview = JSON.parse(run(snapshot, "fix", "--rule", rule, "--ns", "test-wasm.conversion", "--format", "json"));
  const applicable = preview.data.suggestions.filter((item) => item.applicability === "machine-applicable");
  assert.deepEqual(applicable.map((item) => item.definition).sort(), ["test-wasm.conversion/safe-symbol", "test-wasm.conversion/safe-tag"]);
  assert.equal(preview.data.suggestions.some((item) => item.definition === "test-wasm.conversion/tag-input"), false,
    "Tag input must not be treated as the new String-only contract");
  assert.equal(preview.data.suggestions.find((item) => item.definition === "test-wasm.conversion/dynamic-tag")?.applicability, "requires-review");
  assert.equal(preview.data.suggestions.find((item) => item.definition === "test-wasm.conversion/dynamic-symbol")?.applicability, "requires-review");
  assert.equal(preview.data.suggestions.find((item) => item.definition === "test-wasm.conversion/macro-tag")?.applicability, "requires-review");
  assert.equal(preview.data.suggestions.some((item) => item.definition === "test-wasm.conversion/quoted"), false);
  assert.equal(await readFile(snapshot, "utf8"), before, "preview must not modify the Snapshot");

  const stale = spawnSync(binary, [snapshot, "fix", "--rule", rule, "--ns", "test-wasm.conversion", "--apply", "--expect-revision", "md5:stale", "--allow-no-vcs", "--format", "json"], { encoding: "utf8" });
  assert.notEqual(stale.status, 0, "a stale revision must not apply changes");
  assert.equal(await readFile(snapshot, "utf8"), before);

  const applied = JSON.parse(run(snapshot, "fix", "--rule", rule, "--ns", "test-wasm.conversion", "--apply", "--expect-revision", preview.revision, "--allow-no-vcs", "--format", "json"));
  assert.equal(applied.data.changed, true);
  const safeTag = JSON.parse(run(snapshot, "query", "def", "test-wasm.conversion/safe-tag", "--format", "json"));
  const safeSymbol = JSON.parse(run(snapshot, "query", "def", "test-wasm.conversion/safe-symbol", "--format", "json"));
  assert.match(JSON.stringify(safeTag.data.code), /calcit\.core\/to-tag/);
  assert.match(JSON.stringify(safeSymbol.data.code), /calcit\.core\/to-symbol/);
  run(snapshot, "test", "test-wasm.conversion/safe-tag", "--require-match");
  run(snapshot, "test", "test-wasm.conversion/safe-symbol", "--require-match");
  for (const [name, result] of [["safe-tag", /:ready/], ["safe-symbol", /'ready/]]) {
    const executed = spawnSync(binary, [snapshot, "--init-fn", `test-wasm.conversion/${name}`, "--reload-fn", `test-wasm.conversion/${name}`], { encoding: "utf8" });
    assert.equal(executed.status, 0, executed.stderr);
    assert.match(executed.stderr, result);
  }
  const remaining = JSON.parse(run(snapshot, "fix", "--rule", rule, "--ns", "test-wasm.conversion", "--format", "json"));
  assert.equal(remaining.data.suggestions.some((item) => item.applicability === "machine-applicable"), false,
    "applying the guarded fix twice must be idempotent");
} finally {
  await rm(fixture, { recursive: true, force: true });
}
