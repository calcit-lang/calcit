import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, rm, symlink } from "node:fs/promises";
import { readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const run = (...args) => execFileSync(binary, args, {
  encoding: "utf8", stdio: "pipe", timeout: 60000, maxBuffer: 16 * 1024 * 1024,
});
const source = resolve("src/cirru/calcit-core.cirru");
const core = JSON.parse(run("cirru", "parse-edn", "--file", source));
const tests = ["read-file", "read-dir"].flatMap(name => core[":files"]["'calcit.core"].defs[`'${name}`].tests);
assert.equal(tests.length, 4);
const fixture = await mkdtemp(join(tmpdir(), "calcit-file-read-names-"));
const previousInjections = globalThis.__calcit_injections__;
try {
  const snapshot = join(fixture, "calcit.cirru");
  await copyFile(source, snapshot);
  await symlink(resolve("node_modules"), join(fixture, "node_modules"), "dir");
  run(snapshot, "query", "config");
  run(snapshot, "edit", "add-ns", "calcit.file-read-names");
  run(snapshot, "edit", "def", "calcit.file-read-names/run-tests", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "run-tests", [], ...tests.map(test => test.code.__edn_quote), "&unit"]));
  run(snapshot, "edit", "schema", "calcit.file-read-names/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  const entry = [snapshot, "--init-fn", "calcit.file-read-names/run-tests", "--reload-fn", "calcit.file-read-names/run-tests"];
  run(...entry);
  const output = join(fixture, "js-out");
  run(...entry, "--emit-path", output, "js");
  const calls = [];
  globalThis.__calcit_injections__ = {
    read_file: path => { calls.push(["file", path]); return readFileSync(path, "utf8"); },
    read_dir: (path, recursive) => {
      calls.push(["dir", path, recursive]);
      if (path === "/calcit-read-name-does-not-exist") throw new Error("directory does not exist");
      return [`${path}/program.rs`];
    },
  };
  const compiled = await import(pathToFileURL(join(output, "calcit.file-read-names.mjs")).href);
  compiled.run_tests();
  assert.equal(calls.filter(([kind]) => kind === "file").length, 3);
  assert.deepEqual(calls.filter(([kind, path]) => kind === "dir" && path === "src").map(([, , recursive]) => recursive),
    [undefined, undefined, false, false, true, true]);
  assert.equal(calls.filter(([kind, path]) => kind === "dir" && path === "/calcit-read-name-does-not-exist").length, 1);
  const shadow = spawnSync(binary, ["eval", "let ((read-file identity)) read-file |example"], { encoding: "utf8", timeout: 60000 });
  assert.ifError(shadow.error);
  assert.notEqual(shadow.status, 0, "strict core-shadowing diagnostics must remain enabled");
  assert.match(shadow.stderr, /local binding.*read-file.*shadowed/);
  assert.doesNotMatch(shadow.stderr, /invalid pair for &let binding/);
  console.log("File-read aliases passed shared native/JS definition tests, host call counts and strict shadowing diagnostics");
} finally {
  if (previousInjections === undefined) delete globalThis.__calcit_injections__;
  else globalThis.__calcit_injections__ = previousInjections;
  await rm(fixture, { recursive: true, force: true });
}
