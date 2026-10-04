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
  // Host output cannot be observed by a Calcit definition test. Rejecting
  // private EDN must leave error reporting to the caller, not log its payload.
  const runtime = await import(new URL("../lib/calcit.procs.mjs", import.meta.url));
  const originalError = console.error;
  const errors = [];
  console.error = (...args) => errors.push(args);
  try {
    const shape = { version: 3, root: 0, fingerprint: "quiet-edn-parse", nodes: [{ kind: "nil" }] };
    for (const text of ["{", "nil", "42", "|fixture-password", "[] (invalid |fixture-password)"]) {
      assert.throws(() => runtime.parse_cirru_edn(text), /Unexpected data from EDN/);
      assert.throws(() => runtime.parse_cirru_edn_as(text, shape), /Unexpected data from EDN/);
    }
    assert.deepEqual(errors, [], "rejected EDN must not print raw credentials to console.error");
  } finally {
    console.error = originalError;
  }

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
    assert.deepEqual(response.diagnostics, []);
    const tests = response.data.tests.filter(test => test.name === "result-method-contract" || test.tags.includes("parse-boundary"));
    assert.equal(tests.filter(test => test.name === "result-method-contract").length, 1,
      `${name} must retain the ordinary parsing method contract`);
    if (name === "try-parse-cirru-list") {
      assert.equal(tests.filter(test => test.name === "lexer-state-contract").length, 1,
        "Cirru parsing must retain quoted, empty-input and dollar/comma source fixtures");
    }
    trees.push(...tests.map(test => test.code));
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
