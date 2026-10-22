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
const apply = operations => {
  const args = ["edit", "transaction", "--code", JSON.stringify(operations)];
  const dry = JSON.parse(run(...args, "--dry-run", "--format", "json"));
  run(...args, "--expect-revision", dry.scoped_revision, "--format", "json");
};

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
  targets.push("fix-command.import-direct/builtin-id");
  targets.push("fix-command.import-core/builtin-id");
  targets.push("fix-command.import-valid/builtin-id");
  targets.push("fix-command.import-valid/qualified-id");
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

  // Invalid programs cannot be attached tests: they must fail before execution.
  // Only mutate this private Snapshot, leaving the shared semantic fixture valid.
  const schema = ["--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)"];
  apply([
    ["config", "set", "target", "node"],
    ["edit", "add-ns", "fix-command.import-partial"],
    ["edit", "def", "fix-command.import-partial/present", "--input-format", "cirru", "--code", "quote $ def present 1"],
    ["edit", "schema", "fix-command.import-partial/present", "--input-format", "cirru", "--code", "quote 'Number"],
  ]);
  for (const source of ["ghost.missing", "fix-command.import-partial"]) {
    const caller = `fix-command.import-invalid-${source === "ghost.missing" ? "ns" : "def"}`;
    apply([
      ["edit", "add-ns", caller],
      ["edit", "add-import", caller, "--input-format", "cirru", "--code", `quote $ ${source} :refer $ generate-id!`],
      ["edit", "def", `${caller}/main!`, "--input-format", "cirru", "--code", "quote $ defn main! () (generate-id!) &unit"],
      ["edit", "schema", `${caller}/main!`, ...schema],
    ]);
    const checked = spawnSync(binary, [snapshot, "--check-only", "--ns", caller, "--format", "json"], options);
    if (checked.error) throw checked.error;
    assert.equal(checked.status, 1, `${caller}\n${checked.stdout}\n${checked.stderr}`);
    const diagnostics = JSON.parse(checked.stdout).diagnostics;
    assert.ok(diagnostics.some(diagnostic => diagnostic.phase === "preprocess"
      && diagnostic.location.ns === caller && diagnostic.location.def === "main!"
      && diagnostic.message.includes(`import \`${source}/generate-id!\` does not exist`)
      && diagnostic.message.includes("builtin `calcit.core/generate-id!`")), checked.stdout);
    const emitted = spawnSync(binary, [snapshot, "--init-fn", `${caller}/main!`, "--emit-path", output, "js"], options);
    if (emitted.error) throw emitted.error;
    assert.equal(emitted.status, 1, `${caller} JS generation must reject the stale import\n${emitted.stdout}\n${emitted.stderr}`);
    assert.ok((emitted.stdout + emitted.stderr).includes(`${source}/generate-id!`), emitted.stdout + emitted.stderr);

    apply([["edit", "def", `${caller}/main!`, "--overwrite", "--input-format", "cirru", "--code",
      "quote $ defn main! () (let ((id generate-id!)) (id)) &unit"]]);
    const firstClass = spawnSync(binary, [snapshot, "--check-only", "--ns", caller], options);
    if (firstClass.error) throw firstClass.error;
    assert.equal(firstClass.status, 1, `First-class builtin references must diagnose stale imports\n${firstClass.stderr}`);
    assert.ok((firstClass.stdout + firstClass.stderr).includes(`${source}/generate-id!`), firstClass.stdout + firstClass.stderr);

    apply([["edit", "def", `${caller}/main!`, "--overwrite", "--input-format", "cirru", "--code",
      "quote $ defn main! () (quote generate-id!) &unit"]]);
    run("--check-only", "--ns", caller);

    apply([["edit", "def", `${caller}/main!`, "--overwrite", "--input-format", "cirru", "--code",
      "quote $ defn main! () (calcit.core/generate-id!) &unit"]]);
    run("--check-only", "--ns", caller);

    apply([["edit", "def", `${caller}/main!`, "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn main! () (${source}/generate-id!) &unit`]]);
    const qualified = spawnSync(binary, [snapshot, "--check-only", "--ns", caller], options);
    if (qualified.error) throw qualified.error;
    assert.equal(qualified.status, 1, `Qualified invalid references must not fall back to builtins\n${qualified.stderr}`);
    assert.ok((qualified.stdout + qualified.stderr).includes(`unknown ns target: ${source}/generate-id!`), qualified.stdout + qualified.stderr);
  }

  // JavaScript module exports are a host boundary, not Calcit source definitions.
  for (const [index, source] of ["|node:crypto", '"node:crypto'].entries()) {
    const caller = `fix-command.import-external-${index}`;
    apply([
      ["edit", "add-ns", caller],
      ["edit", "add-import", caller, "--input-format", "json-ast", "--code", JSON.stringify([source, ":refer", ["generate-id!"]])],
      ["edit", "def", `${caller}/main!`, "--input-format", "cirru", "--code", "quote $ defn main! () (generate-id!) &unit"],
      ["edit", "schema", `${caller}/main!`, ...schema],
    ]);
    run("--check-only", "--ns", caller);
  }

  // Merely checking that an import exists must not preprocess its unused body.
  const provider = "fix-command.import-source/generate-id!";
  const originalProvider = JSON.parse(run("query", "def", provider, "--format", "json")).data.code;
  apply([["edit", "def", provider, "--overwrite", "--input-format", "cirru", "--code",
    "quote $ fn () (&str:count 1) |module-id"]]);
  // The ordinary reload entry replays the qualified-source assertion. Select
  // the builtin-only root for both entries so that target is genuinely unused.
  run("--check-only", "--init-fn", "fix-command.import-valid/main!", "--reload-fn", "fix-command.import-valid/main!");
  const selectedProvider = spawnSync(binary, [snapshot, "--check-only", "--ns", "fix-command.import-source"], options);
  if (selectedProvider.error) throw selectedProvider.error;
  assert.equal(selectedProvider.status, 1, `The invalid provider body must fail when selected\n${selectedProvider.stdout}\n${selectedProvider.stderr}`);
  assert.ok(/type/i.test(selectedProvider.stdout + selectedProvider.stderr), selectedProvider.stdout + selectedProvider.stderr);
  apply([["edit", "def", provider, "--overwrite", "--input-format", "json-ast", "--code", JSON.stringify(originalProvider)]]);

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
  console.log("Source Fn aliases, namespace/local identity, and builtin import precedence passed shared native/JS tests, strict call gates, invalid-import diagnostics, and lazy loading controls");
} finally {
  await rm(project, { recursive: true, force: true });
}
