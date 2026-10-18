import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import assert from "node:assert/strict";
import { copyFile, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = process.env.CALCIT_STRICT_BIN ?? process.env.CALCIT_BIN ?? "./target/debug/calcit";

// Ordinary project entrypoints must never silently opt back into compatibility mode.
const entrypoints = ["package.json"];
for (const dir of ["scripts", ".github/workflows"]) {
  for (const name of readdirSync(dir, { recursive: true })) {
    if (/\.(?:js|mjs|sh|ts|ya?ml)$/.test(name)) entrypoints.push(`${dir}/${name}`);
  }
}
for (const path of entrypoints) {
  if (path === "scripts/check-strict-default.mjs") continue;
  const source = readFileSync(path, "utf8");
  if (path === "scripts/test-wasi-preprocess.sh") {
    const compatibilityCase = 'cargo run --bin calcit -- --compat-types --check-only "$FIXTURE"';
    if (source.split("--compat-types").length !== 2 || !source.includes(compatibilityCase)) {
      throw new Error(`${path} may use compatibility mode only for its captured legacy fixture`);
    }
  } else if (source.includes("--compat-types")) {
    throw new Error(`${path} must not enable compatibility mode in the default workflow`);
  }
}

function run(args, input) {
  return spawnSync(binary, args, {
    cwd: process.cwd(),
    encoding: "utf8",
    input,
    maxBuffer: 4 * 1024 * 1024,
  });
}

function expectStatus(result, expected, label) {
  if (result.error) throw result.error;
  if (result.status !== expected) {
    throw new Error(`${label} returned ${result.status}, expected ${expected}\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`);
  }
}

// Metadata must not silently discard a malformed contract or manufacture a callable.
for (const [label, source, diagnostic] of [
  ["missing hint schema", "hint-fn", /hint-fn expects.*got 0 arguments/],
  ["extra hint argument", "let ((f (fn () 1))) (hint-fn f ({} (:args ([])) (:return 'Number)) :extra) (f)", /hint-fn expects.*got 3 arguments/],
  ["wrapped body schema", "fn (value) (hint-fn (:: 'Fn ({} (:args ([] 'String)) (:return 'String)))) value", /hint-fn expects a schema Map/],
  ["callback wrapper", "hint-fn ({} (:args ([])) (:return 'Number)) (:: fn () 1)", /hint-fn expects a schema Map/],
  ["numeric local target", "let ((f 7)) (hint-fn f ({} (:args ([])) (:return 'Number))) (type-of f)", /hint-fn target must be an existing local function/],
  ["conflicting return", "let ((f (fn () |wrong))) (hint-fn f ({} (:args ([])) (:return 'Number))) (&+ (f) 1)", /hint-fn schema conflicts with the existing local function signature/],
  ["conflicting input", `let
    f $ fn (n)
      hint-fn $ {} (:args $ [] 'Number) (:return 'Number)
      &+ n 1
  hint-fn f $ {} (:args $ [] 'String) (:return 'Number)
  f |wrong`, /hint-fn schema conflicts with the existing local function signature/],
  ["unproved open target", "fn (f) (hint-fn f ({} (:args ([])) (:return 'Number))) &unit", /a schema does not prove callability/],
  ["nullable function target", `fn (callback)
  hint-fn $ {}
    :args $ [] $ :: 'JsNullish $ :: 'Fn $ {}
      :args $ [] 'Number
      :return 'Number
    :return 'Unit
  hint-fn callback $ {} (:args $ [] 'Number) (:return 'Number)
  , &unit`, /a schema does not prove callability/],
  ["new function target", "hint-fn (fn () 1) ({} (:args ([])) (:return 'Number))", /put .*hint-fn schema.* inside a new function body/],
]) {
  const invalid = run(["eval", source]);
  expectStatus(invalid, 1, label);
  assert.match(invalid.stderr, diagnostic, `${label}: report the malformed hint itself`);
}

const scoped = run(["calcit/type-fail/unsafe-coerce-scoped-strict.cirru", "--check-only"]);
expectStatus(scoped, 0, "default strict valid fixture");

// All-definitions scope (#1556): Calcit's own project and calcit.core carry proofs for every definition,
// including ones no entry references. Core builtin placeholders are excluded, like `--check-only --ns`.
expectStatus(run(["calcit/test.cirru", "--check-only", "--all-defs"]), 0, "all-defs check of the test project");
expectStatus(
  run(["src/cirru/calcit-core.cirru", "--init-fn", "calcit.core/map", "--reload-fn", "calcit.core/map", "--check-only", "--all-defs"]),
  0,
  "all-defs check of calcit.core",
);

// A malformed Snapshot root cannot be expressed by an ordinary attached test:
// the declaration itself, before any test body runs, must fail preprocessing.
const wrappedDefinition = "calcit/type-fail/wrapped-definition-root.cirru";
for (const [label, mode] of [
  ["reachable check", ["--check-only"]],
  ["namespace check", ["--check-only", "--ns", "app.main"]],
  ["all-definitions check", ["--check-only", "--all-defs"]],
  ["JavaScript", ["js"]],
  ["WASM", ["wasm"]],
  ["WASI", ["wasi"]],
  ["strict workflow", ["fix", "--workflow", "strict", "--verify", "--format", "json"]],
]) {
  const result = run([wrappedDefinition, ...mode]);
  expectStatus(result, 1, `${label} rejects a wrapped definition root`);
  assert.match(result.stdout + result.stderr, /app\.main\/increment[^\n]*wrapped top-level definition/,
    `${label} must report the malformed declaration, not an unrelated failure`);
}
const rootLocation = run([wrappedDefinition, "--check-only", "--ns", "app.main", "--format", "json"]);
expectStatus(rootLocation, 1, "report the malformed root as structured source evidence");
const rootDiagnostic = JSON.parse(rootLocation.stdout).diagnostics.find(diagnostic =>
  diagnostic.kind === "syntax" && diagnostic.message.includes("wrapped top-level definition"));
assert.ok(rootDiagnostic, "retain the existing Syntax diagnostic category");
assert.deepEqual(rootDiagnostic.location, { ns: "app.main", def: "increment", coord: [] });
const rootFixture = await mkdtemp(join(tmpdir(), "calcit-definition-root-"));
try {
  const snapshot = join(rootFixture, "calcit.cirru");
  await copyFile(wrappedDefinition, snapshot);
  await symlink(resolve("node_modules"), join(rootFixture, "node_modules"), "dir");
  const inFixture = (...args) => run([snapshot, ...args]);
  const readDefinition = () => {
    const result = inFixture("query", "def", "app.main/increment", "--format", "json");
    expectStatus(result, 0, "query an old malformed root before repairing it");
    return JSON.parse(result.stdout).data.code;
  };
  const wrapped = readDefinition();
  assert.equal(wrapped[0][0], "defn");
  const apply = (operations) => {
    const code = JSON.stringify(operations);
    const dry = inFixture("edit", "transaction", "--code", code, "--dry-run", "--format", "json");
    expectStatus(dry, 0, "preview definition-root repair");
    expectStatus(inFixture("edit", "transaction", "--code", code,
      "--expect-revision", JSON.parse(dry.stdout).scoped_revision, "--format", "json"), 0,
      "apply definition-root repair with the scoped revision");
  };
  apply([
    ["tree", "raise", "app.main/increment", "--path", "0"],
    ["config", "set", "mode", "native"],
  ]);
  assert.deepEqual(readDefinition(), wrapped[0]);
  const anonymous = ["fn", ...wrapped[0].slice(2)];
  for (const [index, root] of [wrapped[0], anonymous].entries()) {
    apply([["edit", "def", "app.main/increment", "--overwrite", "--input-format", "json-ast", "--code", JSON.stringify(root)]]);
    expectStatus(inFixture("fix", "--workflow", "strict", "--verify", "--format", "json"), 0,
      "a canonical named or anonymous function root passes the complete strict workflow");
    expectStatus(inFixture(), 0, "native retains the original increment assertion");
    const output = join(rootFixture, `js-${index}`);
    expectStatus(inFixture("--emit-path", output, "js"), 0, "generate the repaired function root");
    const module = await import(pathToFileURL(join(output, "app.main.mjs")).href);
    module.main_$x_();
    const before = readFileSync(snapshot, "utf8");
    for (const command of [
      ["edit", "def", "app.main/increment", "--overwrite", "--input-format", "json-ast", "--code", JSON.stringify(wrapped)],
      ["tree", "replace", "app.main/increment", "--path", "", "--expect", JSON.stringify(root), "--input-format", "json-ast", "--code", JSON.stringify(wrapped)],
    ]) {
      const result = inFixture(...command);
      expectStatus(result, 1, "edit and tree reject reintroducing the wrapper");
      assert.match(result.stdout + result.stderr, /app\.main\/increment[^\n]*wrapped top-level definition/);
      assert.equal(readFileSync(snapshot, "utf8"), before, "a rejected root must leave the entire Snapshot unchanged");
    }
  }
  apply([["edit", "def", "app.main/increment", "--overwrite", "--input-format", "json-ast",
    "--code", JSON.stringify(["fn", wrapped[0][2], "|wrong"])]]);
  for (const mode of [["--check-only", "--ns", "app.main"], ["js"],
    ["fix", "--workflow", "strict", "--verify", "--format", "json"]]) {
    const result = inFixture(...mode);
    expectStatus(result, 1, "an anonymous String return cannot borrow the definition's Number result");
    assert.match(result.stdout + result.stderr, /E_SCHEMA_DEF_MISMATCH/);
  }
} finally {
  await rm(rootFixture, { recursive: true, force: true });
}

// The declaration's own input context does not prove its return contract.
// An actual nil cannot satisfy the nominal Option<Nil> result, even if an
// anonymous root's CodeEntry says it does.
const anonymousOptionReturn = "calcit/type-fail/anonymous-function-option-return.cirru";
for (const [label, mode] of [
  ["reachable check", ["--check-only"]],
  ["namespace check", ["--check-only", "--ns", "app.main"]],
  ["all-definitions check", ["--check-only", "--all-defs"]],
  ["JavaScript", ["js"]],
  ["WASM", ["wasm"]],
  ["WASI", ["wasi"]],
  ["strict workflow", ["fix", "--workflow", "strict", "--verify", "--format", "json"]],
]) {
  const result = run([anonymousOptionReturn, ...mode]);
  expectStatus(result, 1, `${label} rejects an anonymous nil-for-Option return`);
  assert.match(result.stdout + result.stderr, /E_SCHEMA_DEF_MISMATCH/);
  assert.match(result.stdout + result.stderr, /app\.main\/option-value/);
}
const optionReturnLocation = run([anonymousOptionReturn, "--check-only", "--ns", "app.main", "--format", "json"]);
expectStatus(optionReturnLocation, 1, "report the actual anonymous return contradiction in structured output");
assert.ok(JSON.parse(optionReturnLocation.stdout).diagnostics.some(diagnostic =>
  diagnostic.code === "E_SCHEMA_DEF_MISMATCH" && diagnostic.location?.ns === "app.main"
  && diagnostic.location.def === "option-value" && JSON.stringify(diagnostic.location.coord) === "[2]"),
"the error belongs to the producer's first body expression, not its caller");
const anonymousContracts = "tests/fixtures/anonymous-fn-contract.cirru";
expectStatus(run([anonymousContracts, "--check-only", "--all-defs"]), 0, "check anonymous definition contracts");
expectStatus(run([anonymousContracts, "fix", "--workflow", "strict", "--verify", "--format", "json"]), 0,
  "reuse owned function inputs and trait bounds in the complete strict workflow");
for (const [definition, path, expected] of [["increment", "2.1", "'Number"], ["count-items", "2", "'Number"]]) {
  const query = run([anonymousContracts, "query", "type-at", `app.main/${definition}`, "--path", path, "--format", "json"]);
  expectStatus(query, 0, "query the same owned argument and method evidence used by compilation");
  const report = JSON.parse(query.stdout);
  assert.equal(report.data.inferred_type, expected, `${definition}: infer from owned inputs, not expected return alone`);
  assert.deepEqual(report.diagnostics, [], `${definition}: query must not invent a Dynamic dispatch failure`);
}
expectStatus(spawnSync(process.execPath, ["scripts/run-core-tests.mjs", "--snapshot", anonymousContracts,
  "--backend", "native,js", "--tag", "fn-contract"], {
  cwd: process.cwd(), encoding: "utf8", maxBuffer: 8 * 1024 * 1024,
  env: { ...process.env, CALCIT_BIN: resolve(binary) },
}), 0, "shared anonymous definition tests preserve Number, Option, traits and nested functions");

// A body hint can mark the owned definition async without forging its logical
// return type or turning an async callable into a synchronous callback.
const ownedAsyncFixture = await mkdtemp(join(resolve("target"), "owned-fn-async-"));
try {
  await symlink(resolve("node_modules"), join(ownedAsyncFixture, "node_modules"), "dir");
  const output = join(ownedAsyncFixture, "js-out");
  expectStatus(run([anonymousContracts, "--emit-path", output, "js"]), 0, "generate owned async definitions");
  const generated = await import(pathToFileURL(join(output, "app.main.mjs")).href);
  const pending = generated.async_number();
  assert.ok(pending instanceof Promise, "a first-body async hint must produce an actual Promise");
  assert.equal(await pending, 7, "the Promise resolves to the declared logical Number result");
  const callbackResult = generated.async_callback(1, 2);
  assert.ok(callbackResult instanceof Promise, "an owned async callback retains its invocation contract");
  await callbackResult;

  const invalid = join(ownedAsyncFixture, "sync-watch.cirru");
  await copyFile(anonymousContracts, invalid);
  expectStatus(run([invalid, "query", "config", "--format", "json"]), 0, "inspect the callback fixture configuration");
  expectStatus(run([invalid, "query", "def", "app.main/async-callback", "--format", "json"]), 0,
    "inspect the owned async callback before adding its consumer");
  const operations = [
    ["edit", "def", "app.main/watch-counter", "--input-format", "cirru", "--code",
      "quote $ defn watch-counter (counter) (add-watch! counter :changes async-callback) &unit"],
    ["edit", "schema", "app.main/watch-counter", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ [] $ :: 'Ref 'Number) (:return 'Unit)"],
  ];
  const preview = run([invalid, "edit", "transaction", "--code", JSON.stringify(operations), "--dry-run", "--format", "json"]);
  expectStatus(preview, 0, "preview the synchronous callback consumer");
  expectStatus(run([invalid, "edit", "transaction", "--code", JSON.stringify(operations),
    "--expect-revision", JSON.parse(preview.stdout).scoped_revision, "--format", "json"]), 0,
    "assemble the callback consumer with a revision guard");
  for (const mode of [["--check-only", "--ns", "app.main"], ["--check-only", "--all-defs"],
    ["fix", "--workflow", "strict", "--verify", "--format", "json"]]) {
    const result = run([invalid, ...mode]);
    expectStatus(result, 1, "an owned async callable cannot satisfy a synchronous watch callback");
    assert.match(result.stdout + result.stderr, /async fn/);
    assert.match(result.stdout + result.stderr, /add-watch!|watch-counter/);
  }
} finally {
  await rm(ownedAsyncFixture, { recursive: true, force: true });
}

const asyncWatcher = run(["eval", `fn (counter)
  hint-fn $ {}
    :args $ [] $ :: 'Ref 'Number
    :return 'Unit
  add-watch! counter :changes $ fn (current previous)
    hint-fn $ {}
      :args $ [] 'Number 'Number
      :return 'Unit
      :async true
    , &unit`]);
expectStatus(asyncWatcher, 1, "an explicitly async watcher still violates the synchronous callback contract");
assert.match(asyncWatcher.stderr, /got `async fn\(:number, :number\) -> :unit`/);

const asyncDefinition = run(["src/cirru/calcit-core.cirru", "query", "def", "calcit.core/fn", "--format", "json"]);
expectStatus(asyncDefinition, 0, "read the definition-attached async scope contract");
const asyncTest = JSON.parse(asyncDefinition.stdout).data.tests.find(test => test.name === "async-scope-keeps-local-callbacks-sync");
assert.ok(asyncTest, "the shared async scope contract must exist");
const asyncFixture = await mkdtemp(join(resolve("target"), "async-scope-replay-"));
try {
  const conflictingHintSnapshot = join(asyncFixture, "conflicting-local-hint.cirru");
  await copyFile("calcit/add.cirru", conflictingHintSnapshot);
  const conflictingHintOps = [
    ["config", "set", "reload-fn", "app.main/main!"],
    ["edit", "def", "app.main/main!", "--overwrite", "--input-format", "cirru", "--code",
      "quote $ defn main! () (let ((f (fn () |wrong))) (hint-fn f ({} (:args ([])) (:return 'Number))) (&+ (f) 1)) &unit"],
    ["edit", "schema", "app.main/main!", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)"],
  ];
  const conflictPreview = run([conflictingHintSnapshot, "edit", "transaction", "--code", JSON.stringify(conflictingHintOps), "--dry-run", "--format", "json"]);
  expectStatus(conflictPreview, 0, "preview contradictory local contract fixture");
  expectStatus(run([conflictingHintSnapshot, "edit", "transaction", "--code", JSON.stringify(conflictingHintOps),
    "--expect-revision", JSON.parse(conflictPreview.stdout).scoped_revision, "--format", "json"]), 0,
    "assemble contradictory local contract with a revision guard");
  for (const [label, mode] of [["check", ["--check-only"]], ["native", []], ["JS", ["js"]], ["WASM", ["wasm"]], ["WASI", ["wasi"]]]) {
    const invalid = run([conflictingHintSnapshot, "--emit-path", join(asyncFixture, `hint-conflict-${label}`), ...mode]);
    expectStatus(invalid, 1, `${label} rejects the contradictory local contract before execution`);
    assert.match(invalid.stderr, /hint-fn schema conflicts with the existing local function signature/);
  }
  const snapshot = join(asyncFixture, "calcit.cirru");
  await copyFile("src/cirru/calcit-core.cirru", snapshot);
  await symlink(resolve("node_modules"), join(asyncFixture, "node_modules"), "dir");
  expectStatus(run([snapshot, "edit", "def", "calcit.core/async-scope-replay", "--input-format", "json-ast",
    "--code", JSON.stringify(["defn", "async-scope-replay", [], asyncTest.code, "1"])]), 0,
    "assemble the shared async scope replay");
  const selection = ["--init-fn", "calcit.core/async-scope-replay", "--reload-fn", "calcit.core/async-scope-replay"];
  expectStatus(run([snapshot, ...selection]), 0, "native async scope replay");
  const output = join(asyncFixture, "js-out");
  expectStatus(run([snapshot, ...selection, "--emit-path", output, "js"]), 0, "JS async scope generation");
  const generated = await import(pathToFileURL(join(output, "calcit.core.mjs")).href);
  assert.equal(generated.async_scope_replay(), 1, "JS preserves the shared async scope contract");
  const hintDefinition = run(["src/cirru/calcit-core.cirru", "query", "def", "calcit.core/hint-fn", "--format", "json"]);
  expectStatus(hintDefinition, 0, "read source-hint recheck contract");
  const hintTests = JSON.parse(hintDefinition.stdout).data.tests.filter(test => test.tags.includes("parameter-recheck"));
  assert.equal(hintTests.length, 1);
  expectStatus(run([snapshot, "edit", "def", "calcit.core/parameter-recheck-replay", "--input-format", "json-ast",
    "--code", JSON.stringify(["defn", "parameter-recheck-replay", [], ...hintTests.map(test => test.code), "1"])]), 0,
    "assemble source-hint recheck replay");
  expectStatus(run([snapshot, "edit", "schema", "calcit.core/parameter-recheck-replay", "--input-format", "cirru",
    "--code", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)"]), 0,
    "declare the replay entry contract without annotating inferred loop parameters");
  const hintSelection = ["--init-fn", "calcit.core/parameter-recheck-replay", "--reload-fn", "calcit.core/parameter-recheck-replay"];
  // Compatibility regressions belong here, not in ordinary project entrypoints.
  for (const [mode, policy] of [["strict", []], ["compat", ["--compat-types"]]]) {
    expectStatus(run(["src/cirru/calcit-core.cirru", ...policy, "test", "calcit.core/hint-fn", "--tag", "parameter-recheck", "--require-match"]), 0,
      `${mode} source-hint attached test`);
    expectStatus(run([snapshot, ...policy, ...hintSelection]), 0, `${mode} source-hint native replay`);
    const hintOutput = join(asyncFixture, `hint-recheck-${mode}`);
    expectStatus(run([snapshot, ...policy, ...hintSelection, "--emit-path", hintOutput, "js"]), 0,
      `${mode} source-hint JS generation`);
    const replay = await import(pathToFileURL(join(hintOutput, "calcit.core.mjs")).href);
    assert.equal(replay.parameter_recheck_replay(), 1, `${mode} JS preserves the trailing source hint`);
  }
} finally {
  await rm(asyncFixture, { recursive: true, force: true });
}

const explicitStrict = run(["calcit/type-fail/unsafe-coerce-scoped-strict.cirru", "--strict-types", "--check-only"]);
expectStatus(explicitStrict, 0, "explicit strict accepts a checked open boundary without a quality budget");

const unscoped = run(["calcit/type-fail/unsafe-coerce-unscoped-strict.cirru", "--check-only"]);
expectStatus(unscoped, 1, "default strict invalid fixture");
if (!unscoped.stderr.includes("E_UNSCOPED_UNSAFE_COERCE")) {
  throw new Error(`default strict failure lost its stable code:\n${unscoped.stderr}`);
}

const compatibility = run(["calcit/type-fail/unsafe-coerce-unscoped-strict.cirru", "--compat-types", "--check-only"]);
expectStatus(compatibility, 0, "compatibility escape hatch");

// Compatibility preprocessing must not turn duplicate origins into runtime
// precedence. Replay the definition-owned contracts without relaxing any
// ordinary entrypoint or the compatibility scanner above.
const traitSnapshot = "calcit/test-traits.cirru";
const traitTests = run([traitSnapshot, "--compat-types", "test",
  "test-traits.main/test-explicit-trait-call", "--tag", "trait-runtime", "--require-match"]);
expectStatus(traitTests, 0, "compatibility native duplicate-trait contracts");
const definition = run([traitSnapshot, "query", "def", "test-traits.main/test-explicit-trait-call", "--format", "json"]);
expectStatus(definition, 0, "read duplicate-trait contracts");
const runtimeTests = JSON.parse(definition.stdout).data.tests.filter(test => test.tags.includes("trait-runtime"));
assert.equal(runtimeTests.length, 6);
const identityTests = JSON.parse(definition.stdout).data.tests.filter(test => test.tags.includes("trait-identity-runtime"));
assert.equal(identityTests.length, 1);
expectStatus(run([traitSnapshot, "--compat-types", "test", "test-traits.main/test-explicit-trait-call",
  "--tag", "trait-identity-runtime", "--require-match"]), 0, "fresh trait identity rejects a stale implementation");
const traitFixture = await mkdtemp(join(tmpdir(), "calcit-compat-trait-"));
try {
  const snapshot = join(traitFixture, "calcit.cirru");
  await copyFile(traitSnapshot, snapshot);
  await symlink(resolve("node_modules"), join(traitFixture, "node_modules"), "dir");
  const edit = run([snapshot, "edit", "def", "test-traits.main/test-explicit-trait-call", "--overwrite",
    "--input-format", "json-ast", "--code", JSON.stringify([
      "defn", "test-explicit-trait-call", [], ...runtimeTests.concat(identityTests).map(test => test.code), "1",
    ])]);
  expectStatus(edit, 0, "assemble compatibility trait replay");
  const output = join(traitFixture, "js-out");
  const selection = ["--init-fn", "test-traits.main/test-explicit-trait-call",
    "--reload-fn", "test-traits.main/test-explicit-trait-call"];
  expectStatus(run([snapshot, ...selection, "--compat-types"]), 0, "compatibility native assembled replay");
  expectStatus(run([snapshot, ...selection, "--compat-types", "--emit-path", output, "js"]), 0,
    "compatibility JS duplicate-trait generation");
  const generated = await import(pathToFileURL(join(output, "test-traits.main.mjs")).href);
  assert.equal(generated.test_explicit_trait_call(), 1, "generated JS must preserve duplicate-trait runtime errors");
} finally {
  await rm(traitFixture, { recursive: true, force: true });
}

const conflict = run(["calcit/test.cirru", "--strict-types", "--compat-types", "--check-only"]);
expectStatus(conflict, 1, "conflicting type policies");
if (!conflict.stderr.includes("cannot be used together")) {
  throw new Error(`conflicting policy failure was not actionable:\n${conflict.stderr}`);
}

const evalResult = run(["eval", "+ 1 2"]);
expectStatus(evalResult, 0, "default strict eval");
if (!evalResult.stdout.split("\n").some((line) => line.trim().endsWith(": 3"))) {
  throw new Error(`default strict eval returned an unexpected value:\n${evalResult.stdout}`);
}

const optionMethod = run(["eval", ".unwrap-or (%some 1) 2"]);
expectStatus(optionMethod, 0, "strict core Option receiver method");
if (!optionMethod.stdout.split("\n").some((line) => line.trim().endsWith(": 1"))) {
  throw new Error(`strict core Option receiver method returned an unexpected value:\n${optionMethod.stdout}`);
}

const resultMethod = run(["eval", ".map-err (%err 1) $ fn (e) (+ e 1)"]);
expectStatus(resultMethod, 0, "strict core Result receiver method");
if (!resultMethod.stdout.split("\n").some((line) => line.trim().endsWith(": (%:: 'Result :err 2)"))) {
  throw new Error(`strict core Result receiver method returned an unexpected value:\n${resultMethod.stdout}`);
}

const openMerge = run([
  "eval",
  "count $ merge-dynamic (assert-type ({} (:a 1)) (:: 'Map 'Tag 'Dynamic)) (assert-type ({} (:b 2)) (:: 'Map 'Tag 'Dynamic))",
]);
expectStatus(openMerge, 0, "strict open-container merge");
if (!openMerge.stdout.split("\n").some((line) => line.trim().endsWith(": 2"))) {
  throw new Error(`strict open-container merge returned an unexpected value:\n${openMerge.stdout}`);
}

const openConcat = run([
  "eval",
  "count $ concat-dynamic (assert-type ([] 1 2) (:: 'List 'Dynamic)) (assert-type ([] 3) (:: 'List 'Dynamic))",
]);
expectStatus(openConcat, 0, "strict open-container concat");
if (!openConcat.stdout.split("\n").some((line) => line.trim().endsWith(": 3"))) {
  throw new Error(`strict open-container concat returned an unexpected value:\n${openConcat.stdout}`);
}

for (const [label, args] of [
  ["generic live Option payload mismatch", "(Option :some 1) (Option :some |x)"],
  ["generic Ref bottom payload invariance", "(atom (Option :none)) (atom (Option :some |x))"],
  ["generic Number does not widen to nullable", "1 nil"],
  ["generic nil does not widen to Number", "nil 1"],
  ["generic String does not widen to nullable", "|text nil"],
  ["generic nil does not widen to String", "nil |text"],
  ["generic Option does not widen to nullable", "(Option :some 1) nil"],
  ["generic nil does not widen to Option", "nil (Option :some 1)"],
]) {
  const result = run(["eval", `let ((check (fn (left right) (hint-fn ({} (:generics ([] 'T)) (:args ([] 'T 'T)) (:return 'Unit))) &unit))) (check ${args})`]);
  expectStatus(result, 1, label);
  if (!result.stderr.includes("W_LOCAL_FN_ARG_TYPE_MISMATCH")) {
    throw new Error(`${label} lost its type diagnostic:\n${result.stderr}`);
  }
  const restResult = run(["eval", `let ((check (fn (left & right) (hint-fn ({} (:generics ([] 'T)) (:args ([] 'T)) (:rest 'T) (:return 'Unit))) &unit))) (check ${args})`]);
  expectStatus(restResult, 1, `${label} in local rest call`);
  if (!restResult.stderr.includes("W_LOCAL_FN_ARG_TYPE_MISMATCH")) {
    throw new Error(`${label} lost its local rest diagnostic:\n${restResult.stderr}`);
  }
  const equalityResult = run(["eval", `= ${args}`]);
  expectStatus(equalityResult, 1, `${label} in variadic equality`);
  if (!equalityResult.stderr.includes("W_FN_ARG_TYPE_MISMATCH")) {
    throw new Error(`${label} lost its equality diagnostic:\n${equalityResult.stderr}`);
  }
}

const validMapMethod = run(["eval", "let ((m ({} (:a 1)))) (m .assoc :b 2)"]);
expectStatus(validMapMethod, 0, "typed Map .assoc accepts a matching value");

const invalidDirectMapAssoc = run(["eval", "&map:assoc ({} (:a 1)) :b |oops"]);
expectStatus(invalidDirectMapAssoc, 1, "direct Map association rejects a mismatched value");
if (!invalidDirectMapAssoc.stderr.includes("W_PROC_ARG_TYPE_MISMATCH")) {
  throw new Error(`direct Map association lost its type diagnostic:\n${invalidDirectMapAssoc.stderr}`);
}

const invalidMapMethod = run(["eval", "let ((m ({} (:a 1)))) (m .assoc :b |oops)"]);
expectStatus(invalidMapMethod, 1, "typed Map .assoc rejects a mismatched value");
if (!invalidMapMethod.stderr.includes("W_PROC_ARG_TYPE_MISMATCH")) {
  throw new Error(`typed Map .assoc lost its lowered Proc type diagnostic:\n${invalidMapMethod.stderr}`);
}

const validMapDissoc = run(["eval", "let ((m ({} (:a 1)))) (m .dissoc :missing)"]);
expectStatus(validMapDissoc, 0, "typed Map .dissoc accepts a matching key");

const invalidDirectMapDissoc = run(["eval", "&map:dissoc ({} (:a 1)) |oops"]);
expectStatus(invalidDirectMapDissoc, 1, "direct Map dissociation rejects a mismatched key");
if (!invalidDirectMapDissoc.stderr.includes("W_PROC_ARG_TYPE_MISMATCH")) {
  throw new Error(`direct Map dissociation lost its type diagnostic:\n${invalidDirectMapDissoc.stderr}`);
}

const invalidMapDissoc = run(["eval", "let ((m ({} (:a 1)))) (m .dissoc |oops)"]);
expectStatus(invalidMapDissoc, 1, "typed Map .dissoc rejects a mismatched key");
if (!invalidMapDissoc.stderr.includes("W_PROC_ARG_TYPE_MISMATCH")) {
  throw new Error(`typed Map .dissoc lost its lowered Proc type diagnostic:\n${invalidMapDissoc.stderr}`);
}

console.log(
  "Strict-default CLI smoke passed: valid, failure, compatibility, conflict, eval, core Option/Result methods, open-container merge/concat, and Map assoc/dissoc method types",
);
