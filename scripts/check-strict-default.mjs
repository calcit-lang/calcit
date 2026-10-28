import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import assert from "node:assert/strict";
import { copyFile, mkdtemp, readFile, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { traitRejectionCases } from "./check-map-add-trait-boundary.mjs";

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
  if (source.includes("--compat-types")) {
    throw new Error(`${path} must not enable compatibility mode in the default workflow`);
  }
}

// This script checks the default proof audit itself, so a fixture-wide opt-out does not apply.
delete process.env.CALCIT_PROOF_AUDIT;

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
  ["wrapped body schema", "fn (value) (hint-fn (:: 'Fn ({} (:args ([] 'String)) (:return 'String)))) value", /hint-fn expects a function type/],
  ["callback wrapper", "hint-fn ({} (:args ([])) (:return 'Number)) (:: fn () 1)", /E_LEGACY_HINT_FN_TARGET.*inside the body of the function it describes/],
  ["local target", "let ((f (fn () 1))) (hint-fn f ({} (:args ([])) (:return 'Number))) (f)", /E_LEGACY_HINT_FN_TARGET.*`hint-fn f schema` was removed in 0\.32\.0/],
  ["numeric local target", "let ((f 7)) (hint-fn f ({} (:args ([])) (:return 'Number))) (type-of f)", /E_LEGACY_HINT_FN_TARGET/],
]) {
  const invalid = run(["eval", source]);
  expectStatus(invalid, 1, label);
  assert.match(invalid.stderr, diagnostic, `${label}: report the malformed hint itself`);
}

// The scoped `:js-ffi` coercion passes ordinary strict checking; the default proof audit
// does not count a non-host coercion as return evidence (#2121).
expectStatus(
  run(["calcit/type-fail/unsafe-coerce-scoped-strict.cirru", "--check-only", "--no-proof-audit"]),
  0,
  "default strict valid fixture",
);
const scoped = run(["calcit/type-fail/unsafe-coerce-scoped-strict.cirru", "--check-only"]);
expectStatus(scoped, 1, "proof audit of a scoped non-host coercion");
assert.match(scoped.stdout + scoped.stderr, /E_FN_RETURN_UNPROVEN/);

// A compatibility policy cannot equate separately declared lexical binders.
for (const inner of ["T", "U"]) {
  const source = `fn (outer)
  hint-fn $ {} (:args $ [] 'T) (:return 'Number) (:generics $ [] 'T)
  let
      callback $ fn (other)
        hint-fn $ {} (:args $ [] '${inner}) (:return '${inner}) (:generics $ [] '${inner})
        , outer
    &+ (callback 1) 1`;
  const rejected = run(["--compat-types", "eval", source]);
  expectStatus(rejected, 1, `compatibility preserves ${inner} binder identity`);
  assert.match(rejected.stderr, /E_ERASED_GENERIC_RELATION/);
}

// Ordinary argument/method diagnostics render source names, not binder keys.
for (const [name, source, code] of [
  ["local generic collection input",
    "fn (data)\n  hint-fn $ {} (:args $ [] 'Dynamic) (:return 'Number)\n  let\n      size $ fn (xs)\n        hint-fn $ {} (:args $ [] (:: 'List 'T)) (:return 'Number) (:generics $ [] 'T)\n        count xs\n    size data", /E_ERASED_GENERIC_RELATION/],
  ["unbound generic method",
    "fn (value)\n  hint-fn $ {} (:args $ [] 'T) (:return 'Number) (:generics $ [] 'T)\n  .add value 1", /E_DYNAMIC_METHOD_DISPATCH/],
]) {
  const rejected = run(["eval", source]);
  expectStatus(rejected, 1, name);
  assert.match(rejected.stderr, code);
  assert.match(rejected.stderr, /'T/);
  assert.doesNotMatch(rejected.stderr, /#lexical:|#call-type:|\u0000/);
}

// Runtime guards establish kind, not a complete signature or permanent write contract.
const guardContracts = "tests/fixtures/guard-continuation.cirru";
expectStatus(run([guardContracts, "--check-only", "--all-defs"]), 0, "check rejecting-guard continuations");
expectStatus(run([guardContracts, "fix", "--workflow", "strict", "--verify", "--format", "json"]), 0,
  "retain guard evidence in the complete strict workflow");
for (const [backend, selection] of [["native", ["--exclude-tag", "js"]], ["js", []]]) {
  // set! is explicitly a :js-ffi operation, not a native runtime capability.
  expectStatus(spawnSync(process.execPath, ["scripts/run-core-tests.mjs", "--snapshot", guardContracts,
    "--backend", backend, "--tag", "guard-continuation", ...selection], {
    cwd: process.cwd(), encoding: "utf8", maxBuffer: 8 * 1024 * 1024,
    env: { ...process.env, CALCIT_BIN: resolve(binary) },
  }), 0, `${backend} guard tests preserve calls, errors, checked values, methods and nominal Options`);
}

// Public contains? on a nominal Enum lowers to a generated impl callable; the
// default proof audit must check it against that callable's generic contract.
const publicContains = "tests/fixtures/public-contains-nominal.cirru";
expectStatus(run([publicContains, "--check-only"]), 0, "check public contains? on nominal Enums from the entry");
expectStatus(run([publicContains, "--check-only", "--all-defs"]), 0, "check public contains? on nominal Enums in every definition");
const publicContainsRun = run([publicContains]);
expectStatus(publicContainsRun, 0, "run public contains? on nominal Enums with the proof audit");
assert.match(publicContainsRun.stdout, /\(\[\] true true false false true true true\)/);
expectStatus(spawnSync(process.execPath, ["scripts/run-core-tests.mjs", "--snapshot", publicContains,
  "--backend", "native,js,wasm", "--tag", "public-contains-nominal"], {
  cwd: process.cwd(), encoding: "utf8", maxBuffer: 8 * 1024 * 1024,
  env: { ...process.env, CALCIT_BIN: resolve(binary) },
}), 0, "public contains? on nominal Enums keeps the index range on every backend");

for (const [label, body] of [
  ["unguarded", "f & args"],
  ["returning failure branch", "when (not (fn? f)) |not-a-function\n  f & args"],
  ["reversed guard", "when (fn? f) (raise |not-a-function)\n  f & args"],
  ["shadowed guard", "let ((f (fn () 1))) (when (not (fn? f)) (raise |expected-function)) &unit\n  f & args"],
  ["returning rejection helper", "let ((reject (fn () &unit))) (when (not (fn? f)) (reject)) (f & args)"],
  ["custom Bool predicate", `let
      predicate $ fn (value)
        hint-fn $ {} (:args $ [] 'Dynamic) (:return 'Bool)
        , true
    when (not $ predicate f)
      raise |not-a-function
    f & args`],
]) {
  const invalid = run(["eval", `fn (f args)
  hint-fn $ {} (:args $ [] 'Dynamic $ :: 'List 'Dynamic) (:return 'Dynamic)
  ${body}`]);
  expectStatus(invalid, 1, `${label} does not prove callability`);
  assert.match(invalid.stderr, /W_DYNAMIC_CALL/, `${label} retains the existing dynamic-call diagnostic`);
}

// These programs must fail before execution, so they cannot be attached tests.
// Exercise the existing return/call proof checker, including :js-ffi write boundaries.
const guardFixture = await mkdtemp(join(tmpdir(), "calcit-guard-lifetime-"));
try {
  const snapshot = join(guardFixture, "calcit.cirru");
  await copyFile("calcit/add.cirru", snapshot);
  const inFixture = (...args) => run([snapshot, ...args]);
  expectStatus(inFixture("query", "config", "--format", "json"), 0, "inspect guard fixture configuration");
  expectStatus(inFixture("query", "def", "app.main/main!", "--format", "json"), 0, "inspect guard fixture entry");
  const apply = operations => {
    const code = JSON.stringify(operations);
    const dry = inFixture("edit", "transaction", "--code", code, "--dry-run", "--format", "json");
    expectStatus(dry, 0, "preview guard lifetime fixture");
    expectStatus(inFixture("edit", "transaction", "--code", code,
      "--expect-revision", JSON.parse(dry.stdout).scoped_revision, "--format", "json"), 0,
    "apply guard lifetime fixture with a scoped revision");
  };
  apply([
    ["edit", "def", "app.main/main!", "--overwrite", "--input-format", "cirru", "--code", "quote $ defn main! () &unit"],
    ["edit", "schema", "app.main/main!", "--input-format", "cirru", "--code",
      "quote $ Fn (-> Unit)"],
    ["config", "set", "reload-fn", "app.main/main!"],
    ["config", "set", "target", "native"],
    ["edit", "def", "app.main/guarded", "--input-format", "cirru", "--code", "quote $ defn guarded (f) f"],
    ["edit", "schema", "app.main/guarded", "--input-format", "cirru", "--code",
      "quote $ Fn Dynamic (-> Fn) (:features :js-ffi)"],
    ["edit", "add-ns", "app.fake"],
    ["edit", "def", "app.fake/js-present?", "--input-format", "cirru", "--code", "quote $ defn js-present? (value) true"],
    ["edit", "schema", "app.fake/js-present?", "--input-format", "cirru", "--code",
      "quote $ Fn Dynamic (-> Bool)"],
    ["edit", "def", "app.fake/invoke", "--input-format", "cirru", "--code", "quote $ defn invoke (effect) (effect)"],
    ["edit", "schema", "app.fake/invoke", "--input-format", "cirru", "--code",
      "quote $ Fn Fn (-> Dynamic)"],
    ["edit", "add-import", "app.main", "--input-format", "cirru", "--code", "quote $ app.fake :as fake"],
  ]);
  const guard = "when (not (fn? f)) (raise |expected-function)";
  for (const [label, body, diagnostic] of [
    ["direct rebinding", `${guard}\n  set! f 1\n  , f`, /E_FN_RETURN_UNPROVEN/],
    ["surviving branch rebinding", "if (not (fn? f)) (raise |expected-function) (do (set! f 1) &unit)\n  , f", /E_FN_RETURN_UNPROVEN/],
    ["captured rebinding", `let ((replace (fn () (set! f 1)))) (${guard}) (replace) f`, /E_FN_RETURN_UNPROVEN/],
    ["known helper invokes an unknown callback", `let ((replace (fn () (set! f 1)))) (${guard}) (fake/invoke replace) f`, /E_FN_RETURN_UNPROVEN/],
    ["argument rebinding", `let ((replace (fn () (set! f 1)))) (${guard}) ((fn (ignored value) (hint-fn ({} (:args ([] 'Dynamic 'Fn)) (:return 'Fn))) value) (replace) f)`, /E_CALL_ARGUMENT_UNPROVEN/],
    ["condition rebinding", `let ((replace (fn () (set! f 1)))) (${guard}) (if (do (replace) true) f (fn () 1))`, /E_FN_RETURN_UNPROVEN/],
    ["readonly index query still checks argument effects", `let ((replace (fn () (set! f 1)))) (${guard}) (&list:contains? ([] 1) (do (replace) 0)) f`, /E_FN_RETURN_UNPROVEN/],
    ["initializer rebinding", `let ((replace (fn () (set! f 1)))) (${guard}) (let ((ignored (replace))) f)`, /E_FN_RETURN_UNPROVEN/],
    ["interpolation rebinding", `let ((replace (fn () (set! f 1)))) (${guard}) (let ((syntax (quasiquote (ignored (~ (replace)))))) f)`, /E_FN_RETURN_UNPROVEN/],
    ["escaping capture", `${guard}\n  let ((get-value (fn () (hint-fn ({} (:args ([])) (:return 'Fn))) f))) (set! f 1) (get-value)`, /E_FN_RETURN_UNPROVEN/],
    ["immediate mutation", `${guard}\n  ((fn () (set! f 1) f))`, /E_FN_RETURN_UNPROVEN/],
    ["escaping from an immediate function", `${guard}\n  let ((get-value ((fn () (fn () (hint-fn ({} (:args ([])) (:return 'Fn))) f))))) (set! f 1) (get-value)`, /E_FN_RETURN_UNPROVEN/],
    ["assertion lifetime", `let ((replace (fn () (set! f 1)))) (${guard}) (assert-type f 'Fn) (replace) f`, /E_FN_RETURN_UNPROVEN/],
    ["mutated local condition cannot hide a captured writer", `let ((replace (fn () (set! f 1)))) (${guard}) (let ((run? false)) (set! run? true) (when run? (replace))) f`, /E_FN_RETURN_UNPROVEN/],
    ["same-named user predicate", "when (not (fake/js-present? f)) (raise |expected-function)\n  , f", /E_FN_RETURN_UNPROVEN/],
  ]) {
    apply([["edit", "def", "app.main/guarded", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn guarded (f)\n  ${body}`]]);
    const result = inFixture("fix", "--rule", "concrete-return-proof-v1", "--ns", "app.main", "--def", "guarded", "--format", "json");
    expectStatus(result, 1, `${label} revokes stale predicate evidence`);
    assert.match(result.stdout + result.stderr, diagnostic, `${label} must fail at the existing proof boundary`);
  }
  apply([
    ["edit", "def", "app.main/guarded", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn guarded (f)\n  ${guard}\n  , f`],
    ["edit", "schema", "app.main/guarded", "--input-format", "cirru", "--code",
      "quote $ Fn Dynamic (-> $ Fn Number (-> Number))"],
  ]);
  const kindOnly = inFixture("fix", "--rule", "concrete-return-proof-v1", "--ns", "app.main", "--def", "guarded", "--format", "json");
  expectStatus(kindOnly, 1, "fn? cannot manufacture a Number callback signature");
  assert.match(kindOnly.stdout + kindOnly.stderr, /E_FN_RETURN_UNPROVEN/);

  for (const [label, effect, consume, advance, diagnostic] of [
    ["direct write", "set! value |wrong", "&+ value 1", "inc i", /E_CALL_ARGUMENT_UNPROVEN/],
    ["captured writer with method lowering", "replace", ".inc value", "inc i", /E_DYNAMIC_METHOD_DISPATCH/],
    ["write in recur arguments", ", &unit", ".inc value", "do (set! value |wrong) (inc i)", /E_DYNAMIC_METHOD_DISPATCH/],
  ]) {
    apply([
      ["edit", "def", "app.main/guarded", "--overwrite", "--input-format", "cirru", "--code",
        `quote $ defn guarded (value)
  let ((replace (fn () (set! value |wrong))))
    when (not (number? value)) (raise |expected-number)
    loop ((i 0))
      if (< i 2)
        do
          ${consume}
          ${effect}
          recur (${advance})
        , &unit
    , &unit`],
      ["edit", "schema", "app.main/guarded", "--input-format", "cirru", "--code",
        "quote $ Fn Dynamic (-> Unit) (:features :js-ffi)"],
    ]);
    const staleLoop = inFixture("fix", "--rule", "callable-contract-proof-v1", "--ns", "app.main", "--def", "guarded", "--format", "json");
    expectStatus(staleLoop, 1, `${label}: a re-entrant body cannot borrow loop-entry evidence`);
    assert.match(staleLoop.stdout + staleLoop.stderr, diagnostic,
      "source replay rechecks method dispatch itself, not an already specialized primitive");
  }

  // Rechecking an immediate loop must discard first-pass diagnostics/evidence,
  // retain original source identities, and never evaluate its source macro twice.
  apply([
    ["edit", "def", "app.main/recheck-once", "--input-format", "cirru", "--code",
      "quote $ defmacro recheck-once () (println |guard-recheck-expansion-token) (quasiquote &unit)"],
    ["edit", "schema", "app.main/recheck-once", "--input-format", "cirru", "--code",
      "quote $ Macro (-> $ Expr Unit) (:capabilities :log)"],
    ["edit", "def", "app.main/guarded", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn guarded (value)
  when (not (number? value)) (raise |expected-number)
  loop ((i 0))
    if (&< i 2)
      do
        recheck-once
        set! value |changed
        recur (&+ i 1)
      , &unit`],
  ]);
  const replay = inFixture("--check-only", "--ns", "app.main");
  expectStatus(replay, 0, "recheck an effectful loop from retained source expansions");
  assert.equal((replay.stdout + replay.stderr).match(/guard-recheck-expansion-token/g)?.length, 1,
    "source macros evaluate once even when the body must be rechecked");

  // Calcit values are immutable, so only a set! can make a guard on a local
  // stale. Effects in a definition that never rewrites the local keep it.
  for (const [label, body, returnType, rule] of [
    ["logging effect", `${guard}\n  println |effect\n  , f`, "'Fn", "concrete-return-proof-v1"],
    ["unknown callback without a writer", `${guard}\n  fake/invoke (fn () (println |effect))\n  , f`, "'Fn",
      "concrete-return-proof-v1"],
    ["re-entrant loop without a writer", `when (not (fn? f)) (raise |expected-function)
  loop ((i 0))
    if (&< i 2)
      do
        f
        println |effect
        recur (&+ i 1)
      , &unit`, "'Unit", "callable-contract-proof-v1"],
  ]) {
    apply([
      ["edit", "def", "app.main/guarded", "--overwrite", "--input-format", "cirru", "--code",
        `quote $ defn guarded (f)\n  ${body}`],
      ["edit", "schema", "app.main/guarded", "--input-format", "cirru", "--code",
        `quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return ${returnType})`],
    ]);
    const kept = inFixture("fix", "--rule", rule, "--ns", "app.main", "--def", "guarded", "--format", "json");
    expectStatus(kept, 0, `${label} keeps predicate evidence for a local no set! rewrites`);
  }

  // A user macro can emit a set! that the source never spells. Its target is
  // only known after expansion, so such a definition keeps the old barrier.
  apply([
    ["edit", "def", "app.main/clobber!", "--input-format", "cirru", "--code",
      "quote $ defmacro clobber! (name) (quasiquote (set! (~ name) 1))"],
    ["edit", "schema", "app.main/clobber!", "--input-format", "cirru", "--code",
      "quote $ :: 'Macro $ {} (:required $ [] 'Syntax) (:expansion $ :: 'Expr 'Dynamic)"],
    ["edit", "def", "app.main/guarded", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn guarded (f)
  ${guard}
  let ((get-value (fn () (hint-fn ({} (:args ([])) (:return 'Fn))) f))) (clobber! f) (get-value)`],
    ["edit", "schema", "app.main/guarded", "--input-format", "cirru", "--code",
      "quote $ Fn Dynamic (-> Fn) (:features :js-ffi)"],
  ]);
  const macroWrite = inFixture("fix", "--rule", "concrete-return-proof-v1", "--ns", "app.main", "--def", "guarded", "--format", "json");
  expectStatus(macroWrite, 1, "a macro-generated set! revokes captured predicate evidence");
  assert.match(macroWrite.stdout + macroWrite.stderr, /E_FN_RETURN_UNPROVEN/);

  // A JS implementation can invoke a previously installed captured writer;
  // its discarded Calcit shadow body is not an effect proof for any backend.
  apply([
    ["config", "set", "mode", "js"],
    ["config", "set", "target", "node"],
    ["edit", "def", "app.fake/opaque!", "--input-format", "cirru", "--code", "quote $ defn opaque! () &unit"],
    ["edit", "schema", "app.fake/opaque!", "--input-format", "cirru", "--code",
      "quote $ Fn (-> Unit) (:features :js-ffi)"],
    ["edit", "ffi", "app.fake/opaque!", "--code",
      '{} (:target :node) (:js $ {} $ :inline "|() => { globalThis.__guard_mutate?.(); }")'],
    ["edit", "def", "app.main/guarded", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn guarded (f)
  let ((replace (fn () (set! f 1))))
    js/Reflect.set js/globalThis |__guard_mutate replace
    ${guard}
    fake/opaque!
    , f`],
    ["edit", "schema", "app.main/guarded", "--input-format", "cirru", "--code",
      "quote $ Fn Dynamic (-> Fn) (:features :js-ffi)"],
  ]);
  const opaque = inFixture("fix", "--rule", "concrete-return-proof-v1", "--ns", "app.main", "--def", "guarded", "--format", "json");
  expectStatus(opaque, 1, "an opaque FFI implementation revokes captured predicate evidence");
  assert.match(opaque.stdout + opaque.stderr, /E_FN_RETURN_UNPROVEN/);
} finally {
  await rm(guardFixture, { recursive: true, force: true });
}

// All-definitions scope (#1556): Calcit's own project and calcit.core carry proofs for every definition,
// including ones no entry references. Core builtin placeholders are excluded, like `--check-only --ns`.
// The language-feature test project keeps idioms the default proof audit rejects, so it opts out of the audit only.
expectStatus(run(["calcit/test.cirru", "--check-only", "--all-defs", "--no-proof-audit"]), 0, "all-defs check of the test project");
expectStatus(run(["calcit/test.cirru", "--check-only", "--no-proof-audit", "--compat-types"]), 1, "--no-proof-audit conflicts with --compat-types");
// `--no-proof-audit` skips only the assertion-proof audit; the directional fixture fails by default and passes without it.
const directionalProof = run(["calcit/type-fail/directional-default-p21.cirru", "--check-only"]);
expectStatus(directionalProof, 1, "the default proof audit rejects a directional Dynamic argument");
assert.match(directionalProof.stdout + directionalProof.stderr, /E_CALL_ARGUMENT_UNPROVEN/);
expectStatus(run(["calcit/type-fail/directional-default-p21.cirru", "--check-only", "--no-proof-audit"]), 0, "--no-proof-audit skips the proof audit");
expectStatus(
  run(["src/cirru/calcit-core.cirru", "--init-fn", "calcit.core/map", "--reload-fn", "calcit.core/map", "--check-only", "--all-defs", "--no-proof-audit"]),
  0,
  "all-defs check of calcit.core",
);

// Runtime tests alone do not prove an internal impl callback's input contract.
for (const [namespace, definition] of [
  ["calcit.core", "&core-enum-methods"],
  ["calcit.internal", "&core-contains-enum-impl"],
  ["calcit.core", "&core-struct-methods"],
]) {
  const proof = run(["src/cirru/calcit-core.cirru", "fix", "--rule", "callable-contract-proof-v1",
    "--ns", namespace, "--def", definition, "--format", "json"]);
  expectStatus(proof, 0, `${namespace}/${definition} independently proves its callback inputs`);
  assert.deepEqual(JSON.parse(proof.stdout).diagnostics, [], "core impl input proof retains no hidden findings");
}

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
for (const [definition, path, expected] of [["increment", "2.1", "Number"], ["count-items", "2", "Number"]]) {
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
      "quote $ Fn (Ref Number) (-> Unit)"],
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
      "quote $ Fn (-> Unit)"],
  ];
  const conflictPreview = run([conflictingHintSnapshot, "edit", "transaction", "--code", JSON.stringify(conflictingHintOps), "--dry-run", "--format", "json"]);
  expectStatus(conflictPreview, 0, "preview contradictory local contract fixture");
  expectStatus(run([conflictingHintSnapshot, "edit", "transaction", "--code", JSON.stringify(conflictingHintOps),
    "--expect-revision", JSON.parse(conflictPreview.stdout).scoped_revision, "--format", "json"]), 0,
    "assemble contradictory local contract with a revision guard");
  for (const [label, mode] of [["check", ["--check-only"]], ["native", []], ["JS", ["js"]], ["WASM", ["wasm"]], ["WASI", ["wasi"]]]) {
    const invalid = run([conflictingHintSnapshot, "--emit-path", join(asyncFixture, `hint-conflict-${label}`), ...mode]);
    expectStatus(invalid, 1, `${label} rejects the removed two-argument hint before execution`);
    assert.match(invalid.stderr, /E_LEGACY_HINT_FN_TARGET/);
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
    "--code", "quote $ Fn (-> Number)"]), 0,
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

const explicitStrict = run(["calcit/type-fail/unsafe-coerce-scoped-strict.cirru", "--strict-types", "--check-only", "--no-proof-audit"]);
expectStatus(explicitStrict, 0, "explicit strict accepts a checked open boundary without a quality budget");

const unscoped = run(["calcit/type-fail/unsafe-coerce-unscoped-strict.cirru", "--check-only"]);
expectStatus(unscoped, 1, "default strict invalid fixture");
if (!unscoped.stderr.includes("E_UNSCOPED_UNSAFE_COERCE")) {
  throw new Error(`default strict failure lost its stable code:\n${unscoped.stderr}`);
}

const compatibility = run(["calcit/type-fail/unsafe-coerce-unscoped-strict.cirru", "--compat-types", "--check-only"]);
expectStatus(compatibility, 0, "compatibility escape hatch");

// Compatibility preprocessing must not turn duplicate origins into runtime
// precedence. Replay the migrated contracts without relaxing any
// ordinary entrypoint or the compatibility scanner above.
const traitSnapshot = "calcit/test-traits.cirru";
const traitFixture = await mkdtemp(join(tmpdir(), "calcit-compat-trait-"));
try {
  const snapshot = join(traitFixture, "calcit.cirru");
  await copyFile(traitSnapshot, snapshot);
  await symlink(resolve("node_modules"), join(traitFixture, "node_modules"), "dir");
  // Restore only the original negatives in this isolated policy fixture;
  // ordinary source remains completely replayable with strict checking.
  for (const [name, code] of traitRejectionCases()) {
    const tag = name === "fresh-origin-rejects-stale-impl" ? "trait-identity-runtime" : "trait-runtime";
    expectStatus(run([snapshot, "edit", "add-test", "test-traits.main/test-explicit-trait-call", name,
      "--tags", tag, "--input-format", "cirru", "--code", `quote $ ${code}`]), 0, `restore ${name} in the private policy fixture`);
  }
  const traitTests = run([snapshot, "--compat-types", "test",
    "test-traits.main/test-explicit-trait-call", "--tag", "trait-runtime", "--require-match"]);
  expectStatus(traitTests, 0, "compatibility native duplicate-trait contracts");
  const definition = run([snapshot, "query", "def", "test-traits.main/test-explicit-trait-call", "--format", "json"]);
  expectStatus(definition, 0, "read duplicate-trait contracts");
  const runtimeTests = JSON.parse(definition.stdout).data.tests.filter(test => test.tags.includes("trait-runtime"));
  assert.equal(runtimeTests.length, 6);
  const identityTests = JSON.parse(definition.stdout).data.tests.filter(test => test.tags.includes("trait-identity-runtime"));
  assert.equal(identityTests.length, 1);
  expectStatus(run([snapshot, "--compat-types", "test", "test-traits.main/test-explicit-trait-call",
    "--tag", "trait-identity-runtime", "--require-match"]), 0, "fresh trait identity rejects a stale implementation");
  // This private replay deliberately returns Number, unlike the Unit source routine.
  const replayEdits = JSON.stringify([
    ["edit", "def", "test-traits.main/test-explicit-trait-call", "--overwrite",
      "--input-format", "json-ast", "--code", JSON.stringify([
        "defn", "test-explicit-trait-call", [], ...runtimeTests.concat(identityTests).map(test => test.code), "1",
      ])],
    ["edit", "schema", "test-traits.main/test-explicit-trait-call", "--input-format", "json-ast", "--code",
      JSON.stringify(["::", "'Fn", ["{}", [":args", ["[]"]], [":return", "'Number"]]])],
  ]);
  const replayPreview = run([snapshot, "edit", "transaction", "--code", replayEdits, "--dry-run", "--format", "json"]);
  expectStatus(replayPreview, 0, "preview compatibility trait replay");
  const edit = run([snapshot, "edit", "transaction", "--code", replayEdits,
    "--expect-revision", JSON.parse(replayPreview.stdout).scoped_revision, "--format", "edn"]);
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

// This isolated policy fixture exercises otherwise rejected runtime operands.
// Keep compatibility tests here, alongside the existing compatibility contracts;
// ordinary entrypoints remain covered by the unchanged scanner above.
const spreadFixture = await mkdtemp(join(tmpdir(), "calcit-compat-spread-"));
try {
  const snapshot = join(spreadFixture, "calcit.cirru");
  const project = spreadFixture;
  await copyFile("src/cirru/calcit-core.cirru", snapshot);
  await symlink(resolve("node_modules"), join(project, "node_modules"), "dir");
  const inFixture = (...args) => {
    const result = run([snapshot, ...args]);
    expectStatus(result, 0, `compatibility spread fixture: ${args.slice(0, 3).join(" ")}`);
    return result.stdout;
  };
  inFixture("edit", "add-ns", "calcit.spread-evidence");
  inFixture("edit", "def", "calcit.spread-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ defwasm-export run-tests () 1");
  inFixture("edit", "schema", "calcit.spread-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ Fn (-> Number)");
  inFixture("config", "set", "init-fn", "calcit.spread-evidence/run-tests");
  inFixture("config", "set", "reload-fn", "calcit.spread-evidence/run-tests");
  for (const expression of ["([] 1) .len &", ".len ([] 1) &", "({} (:a 1)) .dissoc :a & & ([] :b)"]) {
    inFixture("edit", "def", "calcit.spread-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defwasm-export run-tests () (${expression})`);
    for (const mode of [[], ["--check-only"], ["js"], ["wasm"]]) {
      const rejected = run([snapshot, "--compat-types", "--emit-path", join(project, "malformed"), ...mode]);
      expectStatus(rejected, 1, `compatibility rejects malformed spread: ${expression} ${mode}`);
      assert.match(rejected.stderr, /E_SPREAD_TYPE_MISMATCH/);
      assert.match(rejected.stderr, /expected.*got/);
    }
  }
  const opaque = [
    [["[]", ":b"], 1],
    [["[]"], 2],
    [["{}", [":bad", "1"]], undefined],
    [["#{}", ":b"], undefined],
    ["0", undefined],
    ["16392.5", undefined],
  ];
  for (let index = 0; index < opaque.length; index++) {
    const [body, expected] = opaque[index];
    const remaining = [["{}", [":a", "1"], [":b", "2"], [":c", "3"]], ".dissoc", ":a", "&", ["dynamic-spread-keys"]];
    // Observe the Map using its existing count proc: compatibility mode does
    // not retain enough result evidence for `.len` in this opaque ABI fixture.
    const count = ["&map:count", remaining];
    const result = expected === undefined ? count : ["let", [["result", count]], ["assert=", String(expected), "result"], "result"];
    const operations = [
      ["edit", "def", "calcit.spread-evidence/dynamic-spread-keys", ...(index === 0 ? [] : ["--overwrite"]), "--input-format", "json-ast", "--code",
        JSON.stringify(["defn", "dynamic-spread-keys", [], body])],
      ["edit", "schema", "calcit.spread-evidence/dynamic-spread-keys", "--input-format", "cirru", "--code",
        "quote $ Fn (-> Dynamic)"],
      ["edit", "def", "calcit.spread-evidence/run-tests", "--overwrite", "--input-format", "json-ast", "--code",
        JSON.stringify(["defwasm-export", "run-tests", [], result])],
    ];
    const transaction = JSON.stringify(operations);
    const preview = JSON.parse(inFixture("edit", "transaction", "--code", transaction, "--dry-run", "--format", "json"));
    inFixture("edit", "transaction", "--code", transaction, "--expect-revision", preview.scoped_revision);
    const native = run([snapshot, "--compat-types"]);
    assert.ifError(native.error);
    assert.equal(native.status, expected === undefined ? 1 : 0, native.stderr);
    if (expected === undefined) assert.match(native.stderr, /expected list for spreading/);
    const opaqueJs = join(project, `opaque-js-${index}`);
    inFixture("--compat-types", "--emit-path", opaqueJs, "js");
    const generated = await import(pathToFileURL(join(opaqueJs, "calcit.spread-evidence.mjs")).href);
    if (expected === undefined) assert.throws(() => generated.run_tests());
    else assert.equal(generated.run_tests(), expected);
    const opaqueWasm = join(project, `opaque-wasm-${index}`);
    inFixture("--compat-types", "--emit-path", opaqueWasm, "wasm");
    const module = new WebAssembly.Module(await readFile(join(opaqueWasm, "program.wasm")));
    const imports = {};
    for (const { module: namespace, name, kind } of WebAssembly.Module.imports(module)) {
      assert.equal(kind, "function");
      (imports[namespace] ??= {})[name] = () => { throw new Error(`Unexpected host call: ${namespace}.${name}`); };
    }
    const instance = new WebAssembly.Instance(module, imports);
    if (expected === undefined) assert.throws(() => instance.exports["run-tests"](), WebAssembly.RuntimeError);
    else assert.equal(instance.exports["run-tests"](), expected);
  }
  console.log("Compatibility spread boundaries retain native/JS/WASM runtime contracts");

  // Legacy source is rejected by default; its retained compatibility path
  // still obtains arity from lexical markers rather than schema slot count.
  const optionalTests = ["let", [["alias", "legacy-sink"]],
    ["assert=", "7", ["legacy-sink", "&", ["[]", "7"]]],
    ["assert=", "7", ["legacy-sink", "&", ["[]", "7", "8"]]],
    ["assert=", "7", ["alias", "&", ["[]", "7"]]],
    ["assert=", "7", ["legacy-rest", "&", ["[]", "7"]]],
    ["assert=", "7", ["legacy-rest", "&", ["[]", "7", "8", "9", "10"]]]];
  const edits = [];
  for (const [name, params, rest] of [["legacy-sink", ["x", "?", "y"], false], ["legacy-rest", ["x", "?", "y", "&", "values"], true]]) {
    edits.push(
      ["edit", "def", `calcit.spread-evidence/${name}`, "--input-format", "json-ast", "--code", JSON.stringify(["defn", name, params, "x"])],
      ["edit", "schema", `calcit.spread-evidence/${name}`, "--input-format", "json-ast", "--code",
        JSON.stringify(["::", "'Fn", ["{}", [":args", ["[]", "'Number", "'Number"]], [":return", "'Number"], ...(rest ? [[":rest", "'Number"]] : [])]])],
    );
  }
  edits.push(
    ["edit", "add-test", "calcit.spread-evidence/legacy-sink", "lexical-optional-spreads", "--tags", "unit", "--input-format", "json-ast", "--code", JSON.stringify(optionalTests)],
    ["edit", "def", "calcit.spread-evidence/run-tests", "--overwrite", "--input-format", "json-ast", "--code", JSON.stringify(["defn", "run-tests", [], optionalTests, "7"])],
  );
  const transaction = JSON.stringify(edits);
  const preview = JSON.parse(inFixture("edit", "transaction", "--code", transaction, "--dry-run", "--format", "json"));
  inFixture("edit", "transaction", "--code", transaction, "--expect-revision", preview.scoped_revision);
  const strictOptional = run([snapshot, "--check-only"]);
  expectStatus(strictOptional, 1, "default strict still rejects lexical optional parameters");
  assert.match(strictOptional.stderr, /E_LEGACY_OPTIONAL_PARAM/);
  inFixture("--compat-types", "test", "calcit.spread-evidence/legacy-sink", "--require-match");
  inFixture("--compat-types");
  const optionalJs = join(project, "optional-js");
  inFixture("--compat-types", "--emit-path", optionalJs, "js");
  assert.equal((await import(pathToFileURL(join(optionalJs, "calcit.spread-evidence.mjs")).href)).run_tests(), 7);
  for (const expression of ["legacy-sink & ([])", "legacy-sink & ([] 7 8 9)", "legacy-rest & ([])"]) {
    const edits = JSON.stringify([["edit", "def", "calcit.spread-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn run-tests () (${expression})`]]);
    const preview = JSON.parse(inFixture("edit", "transaction", "--code", edits, "--dry-run", "--format", "json"));
    inFixture("edit", "transaction", "--code", edits, "--expect-revision", preview.scoped_revision);
    const rejected = run([snapshot, "--compat-types", "--check-only"]);
    expectStatus(rejected, 1, `lexical optional bounds still reject ${expression}`);
    assert.match(rejected.stderr, /W_FN_ARG_TYPE_MISMATCH/);
  }
  console.log("Lexical optional spreads retain native/JS compatibility and default strict rejection");
} finally {
  await rm(spreadFixture, { recursive: true, force: true });
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

const optionMethod = run(["eval", ".unwrap-or (Option :some 1) 2"]);
expectStatus(optionMethod, 0, "strict core Option receiver method");
if (!optionMethod.stdout.split("\n").some((line) => line.trim().endsWith(": 1"))) {
  throw new Error(`strict core Option receiver method returned an unexpected value:\n${optionMethod.stdout}`);
}

const resultMethod = run(["eval", ".map-err (Result :err 1) $ fn (e) (+ e 1)"]);
expectStatus(resultMethod, 0, "strict core Result receiver method");
if (!resultMethod.stdout.split("\n").some((line) => line.trim().endsWith(": (%:: 'Result :err 2)"))) {
  throw new Error(`strict core Result receiver method returned an unexpected value:\n${resultMethod.stdout}`);
}

const openMerge = run([
  "eval",
  "count $ merge (assert-type ({} (:a 1)) (:: 'Map 'Tag 'Dynamic)) (assert-type ({} (:b 2)) (:: 'Map 'Tag 'Dynamic))",
]);
expectStatus(openMerge, 0, "strict open-container merge");
if (!openMerge.stdout.split("\n").some((line) => line.trim().endsWith(": 2"))) {
  throw new Error(`strict open-container merge returned an unexpected value:\n${openMerge.stdout}`);
}

const openConcat = run([
  "eval",
  "count $ concat (assert-type ([] 1 2) (:: 'List 'Dynamic)) (assert-type ([] 3) (:: 'List 'Dynamic))",
]);
expectStatus(openConcat, 0, "strict open-container concat");
if (!openConcat.stdout.split("\n").some((line) => line.trim().endsWith(": 3"))) {
  throw new Error(`strict open-container concat returned an unexpected value:\n${openConcat.stdout}`);
}

for (const [label, args] of [
  ["generic live Option payload mismatch", "(Option :some 1) (Option :some |x)"],
  ["generic Ref bottom payload invariance", "(ref (Option :none)) (ref (Option :some |x))"],
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
