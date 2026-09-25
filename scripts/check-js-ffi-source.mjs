import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { cp, mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repository = fileURLToPath(new URL("..", import.meta.url));
const fixture = await mkdtemp(join(repository, "target/js-ffi-fixture-"));
const relocated = await mkdtemp(join(repository, "target/js-ffi-relocated-"));
try {
  const input = join(fixture, "calcit.cirru");
  const output = join(fixture, "generated");
  await cp(resolve(repository, "calcit/js-ffi-consumer.cirru"), input);
  await cp(resolve(repository, "calcit/js-ffi-module"), join(fixture, "js-ffi-module"), { recursive: true });
  const native = spawnSync(resolve(repository, "target/debug/calcit"), [input], {
    cwd: fixture,
    encoding: "utf8",
  });
  assert.notEqual(native.status, 0, "native execution must reject JS-only definitions");
  assert.match(native.stderr, /unavailable in the native runtime/);
  const wasm = spawnSync(resolve(repository, "target/debug/calcit"), ["wasm", input, "--check-only"], {
    cwd: fixture,
    encoding: "utf8",
  });
  assert.notEqual(wasm.status, 0, "WASM checking must reject JS-only definitions");
  assert.match(wasm.stderr, /E_WASM_UNSUPPORTED_JS_FFI/);
  execFileSync(resolve(repository, "target/debug/calcit"), [input, "--emit-path", output, "js"], {
    cwd: fixture,
    stdio: "inherit",
  });

  await cp(output, join(relocated, "generated"), { recursive: true });
  const generated = join(relocated, "generated");
  const source = await readFile(join(generated, "app.main.mjs"), "utf8");
  const apiSource = await readFile(join(generated, "app.api.mjs"), "utf8");
  const consumerSource = await readFile(join(generated, "test-nil.main.mjs"), "utf8");
  assert.match(source, /JS FFI: app\.main\/plus-two/);
  assert.match(source, /\(value\) => value \+ 2/);
  assert.match(source, /from "node:path"/);
  assert.doesNotMatch(source, /\.\/\.ffi\//);
  assert.match(apiSource, /from "\.\/app\.main\.mjs"/);
  assert.match(consumerSource, /from "\.\/app\.api\.mjs"/);
  assert.match(consumerSource, /from "\.\/app\.main\.mjs"/);
  assert.doesNotMatch(apiSource, /js-ffi-assets|\.ffi\//);
  assert.ok(!(await readdir(generated)).includes(".ffi"), "JS snippets must not be copied as modules");
  for (const namespace of ["app.main", "app.api", "test-nil.main"]) {
    execFileSync(process.execPath, ["--check", join(generated, `${namespace}.mjs`)]);
  }
  const consumer = await import(pathToFileURL(join(generated, "test-nil.main.mjs")).href);
  consumer["main_$x_"]();
  const module = await import(pathToFileURL(join(generated, "app.main.mjs")).href);
  const api = await import(pathToFileURL(join(generated, "app.api.mjs")).href);
  assert.equal(module.plus_one(2), 3);
  assert.equal(module.plus_two(2), 4);
  assert.equal(module.base_name("/tmp/example.txt"), "example.txt");
  assert.equal(api.plus_four(2), 6);
  assert.equal(api.file_label("/tmp/example.txt"), "example.txt");
  assert.equal(module.count_a(), 3);
  assert.equal(api.next_count(), 4, "normal Calcit imports must share one JS FFI definition instance");
  assert.notEqual(module.count_a, module.count_b, "each Calcit definition receives its own expression instance");
  console.log("embedded inline/file JS FFI and ordinary cross-namespace imports passed");
} finally {
  await rm(fixture, { recursive: true, force: true });
  await rm(relocated, { recursive: true, force: true });
}
