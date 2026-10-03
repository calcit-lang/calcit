import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const project = await mkdtemp(join(tmpdir(), "calcit-typed-source-alias-"));
const snapshot = join(project, "calcit.cirru");
const options = { encoding: "utf8", timeout: 60000, maxBuffer: 16 * 1024 * 1024 };
const run = (...args) => execFileSync(binary, [snapshot, ...args], options);

try {
  await copyFile("tests/fixtures/typed-source-alias.cirru", snapshot);
  await symlink(resolve("node_modules"), join(project, "node_modules"), "dir");
  run("--check-only", "--keep-going", "--format", "edn");
  run("test", "--tag", "alias-contract", "--require-match");

  const ns = "fix-command.alias-evidence";
  const cases = [
    [`${ns}/consume-base (${ns}/OtherBase :value 1)`, "W_FN_ARG_TYPE_MISMATCH"],
    [`${ns}/consume-base (fix-command.other-side/AliasBase :value 1)`, "W_FN_ARG_TYPE_MISMATCH"],
    [`${ns}/consume-choice (${ns}/OtherChoice :value 1)`, "W_FN_ARG_TYPE_MISMATCH"],
    [`${ns}/echo-alias 3`, "W_FN_ARG_TYPE_MISMATCH"],
    [`${ns}/echo-chain 3`, "W_FN_ARG_TYPE_MISMATCH"],
    [`${ns}/echo-chain`, "expected 1 args"],
    [`${ns}/echo-chain |a |b`, "expected 1 args"],
    [`${ns}/same-alias 3 |b`, "W_FN_ARG_TYPE_MISMATCH"],
    [`${ns}/apply-alias (fn (x) 3) |a`, "W_FN_ARG_TYPE_MISMATCH"],
    [`${ns}/apply-alias ${ns}/echo-string 3`, "W_FN_ARG_TYPE_MISMATCH"],
    [`${ns}/rest-alias |a 3`, "W_FN_ARG_TYPE_MISMATCH"],
    [`${ns}/optional-alias (Option :some 3)`, "W_FN_ARG_TYPE_MISMATCH"],
  ];
  for (const [expression, diagnostic] of cases) {
    const result = spawnSync(binary, [snapshot, "eval", "--dep", `${project}/`, "--", expression], options);
    if (result.error) throw result.error;
    assert.equal(result.status, 1, `${expression}\n${result.stdout}\n${result.stderr}`);
    assert.ok(result.stderr.includes(diagnostic), `${expression}\n${result.stderr}`);
    assert.ok(result.stderr.includes("app.main/main!"), `Missing caller: ${result.stderr}`);
    assert.ok(result.stderr.includes("preprocessing"), `Must fail before runtime: ${result.stderr}`);
  }

  // Execute the same Calcit attached-test trees in generated JS, not a second
  // JavaScript implementation of the language assertions.
  const targets = [
    "echo-alias", "same-alias", "apply-alias", "rest-alias", "optional-alias", "empty-proc",
    "sort-proc", "range-proc", "fold-proc", "shortcut-proc", "list-question-proc",
  ].map(name => `${ns}/${name}`);
  targets.push(`${ns}/consume-base`);
  targets.push(`${ns}/consume-choice`);
  targets.push("fix-command.reader/state-alias");
  const trees = targets.flatMap(target => {
    const context = JSON.parse(run("query", "context", target, "--format", "json"));
    assert.equal(context.data.tests.truncated, false);
    assert.equal(context.data.tests.items.length, 1);
    return context.data.tests.items.map(test => test.tree);
  });
  const entry = ["defn", "main!", [], ...trees, "&unit"];
  run("edit", "def", "fix-command.reader/main!", "--overwrite", "--input-format", "json-ast", "--code", JSON.stringify(entry));
  run();
  const output = join(project, "js-out");
  run("--emit-path", output, "js");
  const generated = await import(pathToFileURL(join(output, "fix-command.reader.mjs")).href);
  generated.main_$x_();

  // A precise declaration alone must not cause checking to execute a lazy
  // producer. Only direct source references participate in this fix.
  run("edit", "def", `${ns}/computed-alias`, "--input-format", "cirru", "--code",
    "quote $ def computed-alias $ do (println |alias-producer-must-not-execute) fix-command.alias-evidence/echo-string");
  run("edit", "schema", `${ns}/computed-alias`, "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'String) (:return 'String)");
  run("edit", "def", "fix-command.reader/main!", "--overwrite", "--input-format", "cirru", "--code",
    "quote $ defn main! () (fix-command.alias-evidence/computed-alias |ok) &unit");
  const checked = run("--check-only");
  assert.ok(!checked.includes("alias-producer-must-not-execute"), "Checking executed a lazy producer");

  // A cyclic source alias may not supply a terminal Fn. Check termination
  // without executing the cycle or asserting new Dynamic policy.
  run("edit", "def", `${ns}/echo-alias`, "--overwrite", "--input-format", "cirru", "--code",
    "quote $ def echo-alias fix-command.alias-evidence/echo-chain");
  run("edit", "def", "fix-command.reader/main!", "--overwrite", "--input-format", "cirru", "--code",
    "quote $ defn main! () (fix-command.alias-evidence/echo-chain |ok) &unit");
  const cycle = spawnSync(binary, [snapshot, "--check-only"], options);
  if (cycle.error) throw cycle.error;
  assert.ok(cycle.status === 0 || cycle.status === 1, `Cycle aborted: ${cycle.stderr}`);
  assert.ok(!cycle.stderr.includes("stack overflow"), cycle.stderr);
  console.log("Source Fn aliases and same-named namespace/local values passed shared native/JS tests and strict argument/arity/generic/callback gates");
} finally {
  await rm(project, { recursive: true, force: true });
}
