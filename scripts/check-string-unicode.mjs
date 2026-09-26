import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFile, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const run = (...args) => execFileSync(binary, args, { encoding: "utf8", stdio: "pipe", maxBuffer: 16 * 1024 * 1024 });
const corePath = resolve("src/cirru/calcit-core.cirru");
// Read the authoritative definition tests as AST, not a second JS assertion suite.
const core = JSON.parse(run("cirru", "parse-edn", "--file", corePath));
const tests = core[":files"]["'calcit.core"].defs["'last"].tests.filter((test) =>
  ["unicode-scalar-indexing", "rejects-invalid-string-indices"].includes(test.name),
);
assert.equal(tests.length, 2, "both Unicode definition tests must be selected");
const fixture = await mkdtemp(join(tmpdir(), "calcit-string-unicode-"));
try {
  const snapshot = join(fixture, "calcit.cirru");
  await copyFile(corePath, snapshot);
  await symlink(resolve("node_modules"), join(fixture, "node_modules"), "dir");
  run(snapshot, "edit", "add-ns", "calcit.unicode");
  run(snapshot, "edit", "def", "calcit.unicode/main!", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "main!", [], ...tests.map((test) => test.code.__edn_quote), "&unit"]));
  run(snapshot, "edit", "schema", "calcit.unicode/main!", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  const entry = [snapshot, "--init-fn", "calcit.unicode/main!", "--reload-fn", "calcit.unicode/main!"];
  run(...entry);
  const output = join(fixture, "js-out");
  run(...entry, "--emit-path", output, "js");
  const compiled = await import(pathToFileURL(join(output, "calcit.unicode.mjs")).href);
  compiled.main_$x_();
  console.log("Unicode definition tests passed on native and generated JS");
} finally {
  await rm(fixture, { recursive: true, force: true });
}
