import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import assert from "node:assert/strict";
import { copyFile, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = process.env.CALCIT_STRICT_BIN ?? "./target/debug/calcit";

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

const scoped = run(["calcit/type-fail/unsafe-coerce-scoped-strict.cirru", "--check-only"]);
expectStatus(scoped, 0, "default strict valid fixture");

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
const traitFixture = await mkdtemp(join(tmpdir(), "calcit-compat-trait-"));
try {
  const snapshot = join(traitFixture, "calcit.cirru");
  await copyFile(traitSnapshot, snapshot);
  await symlink(resolve("node_modules"), join(traitFixture, "node_modules"), "dir");
  const edit = run([snapshot, "edit", "def", "test-traits.main/test-explicit-trait-call", "--overwrite",
    "--input-format", "json-ast", "--code", JSON.stringify([
      "defn", "test-explicit-trait-call", [], ...runtimeTests.map(test => test.code), "1",
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
