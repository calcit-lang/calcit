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

const fixture = await mkdtemp(join(tmpdir(), "calcit-map-add-trait-"));
try {
  await symlink(resolve("node_modules"), join(fixture, "node_modules"), "dir");
  const output = join(fixture, "js-out");
  run(traitSnapshot, "--emit-path", output, "js");
  const traits = await import(pathToFileURL(join(output, "test-traits.main.mjs")).href);
  traits.test_add_trait();

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
  console.log("Map .add / Add trait boundary passed on native, generated JS, and strict negative checking");
} finally {
  await rm(fixture, { recursive: true, force: true });
}
