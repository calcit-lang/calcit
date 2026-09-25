import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { cp, mkdtemp, readFile, readdir, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repository = fileURLToPath(new URL("..", import.meta.url));
const output = resolve(repository, "target/js-ffi-consumer-check");
await rm(output, { recursive: true, force: true });
const native = spawnSync(resolve(repository, "target/debug/calcit"), [resolve(repository, "calcit/js-ffi-consumer.cirru")], {
  cwd: repository,
  encoding: "utf8",
});
assert.notEqual(native.status, 0, "native execution must reject JS-only definitions");
assert.match(native.stderr, /unavailable in the native runtime/);
const wasm = spawnSync(resolve(repository, "target/debug/calcit"), [
  "wasm",
  resolve(repository, "calcit/js-ffi-consumer.cirru"),
  "--check-only",
], { cwd: repository, encoding: "utf8" });
assert.notEqual(wasm.status, 0, "WASM checking must reject JS-only definitions");
assert.match(wasm.stderr, /E_WASM_UNSUPPORTED_JS_FFI/);
execFileSync(resolve(repository, "target/debug/calcit"), [
  resolve(repository, "calcit/js-ffi-consumer.cirru"),
  "--emit-path",
  output,
  "js",
], { cwd: repository, stdio: "inherit" });

const relocated = await mkdtemp(join(tmpdir(), "calcit-js-ffi-consumer-"));
try {
  await cp(output, join(relocated, "generated"), { recursive: true });
  await symlink(resolve(repository, "node_modules"), join(relocated, "node_modules"), "dir");
  const generated = join(relocated, "generated");
  const source = await readFile(join(generated, "app.main.mjs"), "utf8");
  assert.match(source, /JS FFI: app\.main\/plus-two/);
  assert.match(source, /\(value\) => value \+ 2/);
  assert.match(source, /from "node:path"/);
  assert.doesNotMatch(source, /\.\/\.ffi\//);
  assert.ok(!(await readdir(generated)).includes(".ffi"), "JS snippets must not be copied as modules");
  execFileSync(process.execPath, ["--check", join(generated, "app.main.mjs")]);
  const consumer = await import(pathToFileURL(join(generated, "test-nil.main.mjs")).href);
  consumer["main_$x_"]();
  const module = await import(pathToFileURL(join(generated, "app.main.mjs")).href);
  assert.equal(module.plus_one(2), 3);
  assert.equal(module.plus_two(2), 4);
  assert.equal(module.base_name("/tmp/example.txt"), "example.txt");
  assert.notEqual(module.count_a, module.count_b, "each Calcit definition receives its own expression instance");
  console.log("embedded inline/file JS FFI passed");
} finally {
  await rm(relocated, { recursive: true, force: true });
}
