import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFile, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const project = await mkdtemp(join(tmpdir(), "calcit-parse-boundary-"));
const snapshot = join(project, "calcit.cirru");
const options = { encoding: "utf8", stdio: "pipe", timeout: 60000, maxBuffer: 16 * 1024 * 1024 };
const run = (...args) => execFileSync(binary, [snapshot, ...args], options);

try {
  await copyFile("src/cirru/calcit-core.cirru", snapshot);
  await symlink(resolve("node_modules"), join(project, "node_modules"), "dir");
  run("test", "--tag", "parse-boundary", "--require-match");

  // Reuse the source tests verbatim in generated JS, not host-side decoder assertions.
  const trees = ["try-parse-cirru-edn-as", "try-decode-map-as"].flatMap(name => {
    const response = JSON.parse(run("query", "def", `calcit.core/${name}`, "--format", "json"));
    assert.deepEqual(response.diagnostics, []);
    const tests = response.data.tests.filter(test => test.tags.includes("parse-boundary"));
    assert.equal(tests.length, 2, `${name} must retain both deep success and rejection tests`);
    return tests.map(test => test.code);
  });
  for (const name of ["try-parse-json", "try-parse-cirru", "try-parse-cirru-edn", "try-parse-cirru-list", "parse-float"]) {
    const target = `calcit.core/${name}`;
    run("test", target, "--require-match");
    const response = JSON.parse(run("query", "def", target, "--format", "json"));
    const tests = response.data.tests.filter(test => test.name === "result-method-contract");
    assert.equal(tests.length, 1, `${name} must retain the ordinary parsing method contract`);
    trees.push(tests[0].code);
  }
  run("edit", "add-ns", "calcit.parse-boundary");
  run("edit", "def", "calcit.parse-boundary/main!", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "main!", [], ...trees, "&unit"]));
  run("config", "set", "init-fn", "calcit.parse-boundary/main!");
  run("config", "set", "reload-fn", "calcit.parse-boundary/main!");
  run("--check-only");
  run();
  const output = join(project, "js-out");
  run("--emit-path", output, "js");
  const generated = await import(pathToFileURL(join(output, "calcit.parse-boundary.mjs")).href);
  generated.main_$x_();
  console.log("Shared Calcit parsing tests passed native/JS method Result contracts, deep payload checks and structural failure paths");
} finally {
  await rm(project, { recursive: true, force: true });
}
