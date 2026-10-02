import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const project = await mkdtemp(join(tmpdir(), "calcit-known-assertion-"));
const snapshot = join(project, "calcit.cirru");
const options = { encoding: "utf8", stdio: "pipe", timeout: 60000, maxBuffer: 16 * 1024 * 1024 };
const run = (...args) => execFileSync(binary, [snapshot, ...args], options);

try {
  await copyFile("src/cirru/calcit-core.cirru", snapshot);
  await symlink(resolve("node_modules"), join(project, "node_modules"), "dir");
  const coreOriginal = await readFile(snapshot);
  run("fix", "--rule", "concrete-return-proof-v1", "--ns", "calcit.core", "--def", "every?", "--format", "edn");
  assert.deepEqual(await readFile(snapshot), coreOriginal);
  run("test", "--tag", "assert-boundary", "--require-match");
  const response = JSON.parse(run("query", "def", "calcit.core/assert-type", "--format", "json"));
  assert.deepEqual(response.diagnostics, []);
  const tests = response.data.tests.filter(test => test.tags.includes("assert-boundary"));
  assert.equal(tests.length, 8);
  run("test", "calcit.core/hint-fn", "--tag", "return-boundary", "--require-match");
  const returnResponse = JSON.parse(run("query", "def", "calcit.core/hint-fn", "--format", "json"));
  assert.deepEqual(returnResponse.diagnostics, []);
  const returnTests = returnResponse.data.tests.filter(test => test.tags.includes("return-boundary"));
  assert.equal(returnTests.length, 3);
  run("test", "calcit.core/hint-fn", "--tag", "call-boundary", "--require-match");
  const callTests = returnResponse.data.tests.filter(test => test.tags.includes("call-boundary"));
  assert.equal(callTests.length, 8);
  run("test", "calcit.core/hint-fn", "--tag", "hint-value-boundary", "--require-match");
  const hintTests = returnResponse.data.tests.filter(test => test.tags.includes("hint-value-boundary"));
  assert.equal(hintTests.length, 5);
  run("test", "calcit.core/hint-fn", "--tag", "async-return-boundary", "--require-match");
  const asyncTests = returnResponse.data.tests.filter(test => test.tags.includes("async-return-boundary"));
  assert.equal(asyncTests.length, 1);
  run("test", "calcit.core/quote", "--tag", "quote-return-boundary", "--require-match");
  const quoteResponse = JSON.parse(run("query", "def", "calcit.core/quote", "--format", "json"));
  assert.deepEqual(quoteResponse.diagnostics, []);
  const quoteTests = quoteResponse.data.tests.filter(test => test.tags.includes("quote-return-boundary"));
  assert.equal(quoteTests.length, 3);
  run("test", "calcit.core/try-decode-map-as", "--tag", "ref-alias-boundary", "--require-match");
  const refResponse = JSON.parse(run("query", "def", "calcit.core/try-decode-map-as", "--format", "json"));
  const refTests = refResponse.data.tests.filter(test => test.tags.includes("ref-alias-boundary"));
  assert.equal(refTests.length, 2);
  run("edit", "add-ns", "calcit.assert-evidence");
  const setBody = trees => run("edit", "def", "calcit.assert-evidence/run-tests", "--overwrite",
    "--input-format", "json-ast", "--code", JSON.stringify(["defwasm-export", "run-tests", [], ...trees, "1"]));
  run("test", "calcit.core/foldl-shortcut", "--tag", "shortcut-fold-proof", "--require-match");
  const shortcutResponse = JSON.parse(run("query", "def", "calcit.core/foldl-shortcut", "--format", "json"));
  const shortcutTests = shortcutResponse.data.tests.filter(test => test.tags.includes("shortcut-fold-proof"));
  assert.equal(shortcutTests.length, 5);
  const openFoldTests = shortcutResponse.data.tests.filter(test => test.tags.includes("open-fold-proof"));
  assert.equal(openFoldTests.length, 3);
  run("test", "calcit.core/foldl-shortcut", "--tag", "open-fold-proof", "--require-match");
  // Audit the typed boundary independently of assert='s generic equality
  // implementation, whose separate core proof obligations remain visible.
  const directShortcutTests = shortcutTests.filter(test => ["proven-shortcut-bool", "shortcut-empty-default", "shortcut-right-order"].includes(test.name));
  assert.equal(directShortcutTests.length, 3);
  setBody(directShortcutTests.map(test => test.code.slice(0, -1)));
  run("edit", "schema", "calcit.assert-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)");
  run("config", "set", "init-fn", "calcit.assert-evidence/run-tests");
  run("config", "set", "reload-fn", "calcit.assert-evidence/run-tests");
  run("fix", "--rule", "concrete-return-proof-v1", "--ns", "calcit.assert-evidence", "--def", "run-tests", "--format", "edn");
  setBody(shortcutTests.map(test => test.code));
  const shortcutWasmOutput = join(project, "shortcut-wasm");
  run("wasm", "--emit-path", shortcutWasmOutput);
  const shortcutModule = new WebAssembly.Module(await readFile(join(shortcutWasmOutput, "program.wasm")));
  const shortcutImports = {};
  for (const item of WebAssembly.Module.imports(shortcutModule)) {
    assert.equal(item.kind, "function");
    shortcutImports[item.module] ??= {};
    shortcutImports[item.module][item.name] = () => { throw new Error(`unexpected shortcut import ${item.module}.${item.name}`); };
  }
  assert.equal(new WebAssembly.Instance(shortcutModule, shortcutImports).exports["run-tests"](), 1);
  run();
  const shortcutOutput = join(project, "shortcut-js-out");
  run("--emit-path", shortcutOutput, "js");
  const shortcutGenerated = await import(pathToFileURL(join(shortcutOutput, "calcit.assert-evidence.mjs")).href);
  assert.equal(shortcutGenerated.run_tests(), 1);
  for (const [name, defaultValue, callback, items = ["[]", "1", "2"]] of [
    ["wrong-payload", "0", ["fn", ["acc", "item"], ["::", "true", "|bad"]]],
    ["wrong-control", "0", ["fn", ["acc", "item"], ["::", "1", "acc"]]],
    ["missing-payload", "0", ["fn", ["acc", "item"], ["::", "true"]]],
    ["wrong-default", "|bad", ["fn", ["acc", "item"], ["::", "true", "acc"]]],
    ["open-payload", "0", ["fn", ["acc", "item"], ["::", "true", ["parse-cirru-edn", "|do 1"]]]],
    ["mixed-branches", "0", ["fn", ["acc", "item"], ["if", "item", ["::", "true", "acc"], ["::", "false", "|bad"]]], ["[]", "true", "false"]],
  ]) {
    setBody([["assert-type", ["foldl-shortcut", items, "0", defaultValue, callback], "'Number"]]);
    const original = await readFile(snapshot);
    const rejected = spawnSync(binary, [snapshot, "fix", "--rule", "assert-type-proof-v1", "--ns", "calcit.assert-evidence", "--def", "run-tests", "--format", "edn"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${name}\n${rejected.stdout}\n${rejected.stderr}`);
    assert.match(`${rejected.stdout}\n${rejected.stderr}`, /E_ASSERT_TYPE_UNPROVEN|W_PROC_ARG_TYPE_MISMATCH|contradictory producer contract/);
    assert.deepEqual(await readFile(snapshot), original);
  }
  // A typed caller's name must not lend proof to a shadowing Dynamic local.
  for (const shadowBody of [
    ["let", [["payload", ["parse-cirru-edn", "|do |bad"]]], ["::", "true", "payload"]],
    ["match", ["Option", ":some", ["parse-cirru-edn", "|do |bad"]], [[":some", "payload"], ["::", "true", "payload"]], [[":none"], ["::", "true", "acc"]]],
  ]) {
    run("edit", "def", "calcit.assert-evidence/shadow-proof", "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "shadow-proof", ["payload"], ["foldl-shortcut", ["[]", "1"], "0", "0", ["fn", ["acc", "item"], shadowBody]]]));
    run("edit", "schema", "calcit.assert-evidence/shadow-proof", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)");
    const original = await readFile(snapshot);
    const rejected = spawnSync(binary, [snapshot, "fix", "--rule", "concrete-return-proof-v1", "--ns", "calcit.assert-evidence", "--def", "shadow-proof", "--format", "edn"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${rejected.stdout}\n${rejected.stderr}`);
    assert.match(`${rejected.stdout}\n${rejected.stderr}`, /E_FN_RETURN_UNPROVEN/);
    assert.deepEqual(await readFile(snapshot), original);
  }
  run("edit", "rm-def", "calcit.assert-evidence/shadow-proof");
  // Known initial/default values do not prove an externally supplied reducer.
  run("edit", "def", "calcit.assert-evidence/open-shortcut", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "open-shortcut", ["callback"], ["foldl-shortcut", ["[]", "1"], "0", "0", "callback"]]));
  run("edit", "schema", "calcit.assert-evidence/open-shortcut", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'Fn) (:return 'Number)");
  const openShortcutOriginal = await readFile(snapshot);
  const openShortcut = spawnSync(binary, [snapshot, "fix", "--rule", "concrete-return-proof-v1", "--ns", "calcit.assert-evidence", "--def", "open-shortcut", "--format", "edn"], options);
  if (openShortcut.error) throw openShortcut.error;
  assert.equal(openShortcut.status, 1, `${openShortcut.stdout}\n${openShortcut.stderr}`);
  assert.match(`${openShortcut.stdout}\n${openShortcut.stderr}`, /E_FN_RETURN_UNPROVEN/);
  assert.deepEqual(await readFile(snapshot), openShortcutOriginal);
  run("edit", "rm-def", "calcit.assert-evidence/open-shortcut");
  setBody(refTests.map(test => test.code));
  run();
  const refOutput = join(project, "ref-alias-js");
  run("--emit-path", refOutput, "js");
  const refGenerated = await import(pathToFileURL(join(refOutput, "calcit.assert-evidence.mjs")).href);
  assert.equal(refGenerated.run_tests(), 1);
  setBody([["try-decode-map-as", "nil", ["::", "'Ref", "'Number"]]]);
  const refWasmOutput = join(project, "ref-decode-wasm");
  const refWasm = spawnSync(binary, [snapshot, "wasm", "--emit-path", refWasmOutput], options);
  if (refWasm.error) throw refWasm.error;
  assert.notEqual(refWasm.status, 0, `${refWasm.stdout}\n${refWasm.stderr}`);
  assert.match(`${refWasm.stdout}\n${refWasm.stderr}`, /try-decode-map-as is not yet supported in WASM codegen/);
  await assert.rejects(readFile(join(refWasmOutput, "program.wasm")), { code: "ENOENT" });
  // Decoding produces a new cell; proof must never narrow the original alias.
  run("edit", "def", "calcit.assert-evidence/alias-proof", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "alias-proof", ["raw"], ["let", [["alias", "raw"]], ["match", ["try-decode-map-as", "raw", ["::", "'Ref", "'Number"]], [[":ok", "decoded"], ["assert-type", "alias", ["::", "'Ref", "'Number"]]], [[":err", "_"], "nil"]]]]));
  run("edit", "schema", "calcit.assert-evidence/alias-proof", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] $ :: 'Ref 'Dynamic) (:return 'Nil)");
  const aliasOriginal = await readFile(snapshot);
  const aliasProof = spawnSync(binary, [snapshot, "fix", "--rule", "assert-type-proof-v1", "--ns", "calcit.assert-evidence", "--def", "alias-proof", "--format", "edn"], options);
  if (aliasProof.error) throw aliasProof.error;
  assert.equal(aliasProof.status, 1, `${aliasProof.stdout}\n${aliasProof.stderr}`);
  assert.match(`${aliasProof.stdout}\n${aliasProof.stderr}`, /E_ASSERT_TYPE_UNPROVEN/);
  assert.deepEqual(await readFile(snapshot), aliasOriginal);
  run("edit", "rm-def", "calcit.assert-evidence/alias-proof");
  setBody(openFoldTests.map(test => test.code.slice(0, -1)));
  run("fix", "--rule", "concrete-return-proof-v1", "--ns", "calcit.assert-evidence", "--def", "run-tests", "--format", "edn");
  setBody(openFoldTests.map(test => test.code));
  run();
  const openFoldOutput = join(project, "open-fold-js");
  run("--emit-path", openFoldOutput, "js");
  const openFoldGenerated = await import(pathToFileURL(join(openFoldOutput, "calcit.assert-evidence.mjs")).href);
  assert.equal(openFoldGenerated.run_tests(), 1);
  // Open input cannot lend member, callback, or payload evidence to the result.
  for (const [name, receiver, defaultValue, result, foldName = "foldl-shortcut"] of [
    ["open-member", "xs", "0", ["::", "true", "item"]],
    ["open-wrong-payload", "xs", "0", ["::", "true", "|bad"]],
    ["open-wrong-default", "xs", "|bad", ["::", "true", "acc"]],
    ["known-invalid-receiver", "1", "0", ["::", "true", "acc"]],
    ["ordinary-open-member", "xs", null, "item", "foldl"],
  ]) {
    run("edit", "def", "calcit.assert-evidence/open-fold-bad", "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "open-fold-bad", ["xs"], [foldName, receiver, "0", ...(defaultValue === null ? [] : [defaultValue]), ["fn", ["acc", "item"], result]]]));
    run("edit", "schema", "calcit.assert-evidence/open-fold-bad", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Number)");
    const original = await readFile(snapshot);
    const rejected = spawnSync(binary, [snapshot, "fix", "--rule", "concrete-return-proof-v1", "--ns", "calcit.assert-evidence", "--def", "open-fold-bad", "--format", "edn"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${name}\n${rejected.stdout}\n${rejected.stderr}`);
    assert.match(`${rejected.stdout}\n${rejected.stderr}`, /E_FN_RETURN_UNPROVEN|W_PROC_ARG_TYPE_MISMATCH|contradictory producer contract/);
    assert.deepEqual(await readFile(snapshot), original);
  }
  run("edit", "rm-def", "calcit.assert-evidence/open-fold-bad");
  setBody([...tests, ...returnTests, ...callTests, ...hintTests, ...asyncTests, ...quoteTests].map(test => test.code));
  run("edit", "schema", "calcit.assert-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)");
  run("config", "set", "init-fn", "calcit.assert-evidence/run-tests");
  run("config", "set", "reload-fn", "calcit.assert-evidence/run-tests");
  const concreteCalls = callTests.filter(test => test.tags.includes("concrete-call-proof"));
  assert.equal(concreteCalls.length, 4);
  setBody(concreteCalls.map(test => test.code));
  run("fix", "--rule", "concrete-return-proof-v1", "--ns", "calcit.assert-evidence", "--def", "run-tests", "--format", "edn");
  const callableTests = callTests.filter(test => ["fixed-callback-call-contract", "typed-callable-to-open-storage"].includes(test.name));
  assert.equal(callableTests.length, 2);
  setBody(callableTests.map(test => test.code));
  run("fix", "--rule", "callable-contract-proof-v1", "--ns", "calcit.assert-evidence", "--def", "run-tests", "--format", "edn");
  setBody([...tests, ...returnTests, ...callTests, ...hintTests, ...asyncTests, ...quoteTests].map(test => test.code));
  run();
  const output = join(project, "js-out");
  run("--emit-path", output, "js");
  const generated = await import(pathToFileURL(join(output, "calcit.assert-evidence.mjs")).href);
  assert.equal(generated.run_tests(), 1);

  // Replay the same Calcit expression as a factory and invoke its async
  // function in JS, where Promise adoption is observable at the host boundary.
  run("edit", "def", "calcit.assert-evidence/make-async-tail", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "make-async-tail", [], asyncTests[0].code]));
  run("edit", "schema", "calcit.assert-evidence/make-async-tail", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Fn)");
  const asyncOutput = join(project, "async-js-out");
  run("--emit-path", asyncOutput, "js");
  const asyncGenerated = await import(pathToFileURL(join(asyncOutput, "calcit.assert-evidence.mjs")).href);
  const pending = asyncGenerated.make_async_tail()(3);
  assert.ok(pending instanceof Promise, "async forwarding must return a Promise, not merely a synchronous value");
  assert.equal(await pending, 3);
  run("edit", "rm-def", "calcit.assert-evidence/make-async-tail");

  // Only replay the already-supported scalar subset in core WASM.
  const scalar = tests.filter(test => test.tags.includes("assert-scalar-wasm"));
  assert.equal(scalar.length, 4);
  setBody(scalar.map(test => test.code));
  run("wasm", "--emit-path", project);
  const module = new WebAssembly.Module(await readFile(join(project, "program.wasm")));
  const imports = {};
  for (const item of WebAssembly.Module.imports(module)) {
    assert.equal(item.kind, "function");
    (imports[item.module] ??= {})[item.name] = () => { throw new Error(`Unexpected host call: ${item.name}`); };
  }
  const wasm = new WebAssembly.Instance(module, imports);
  assert.equal(wasm.exports["run-tests"](), 1);

  // Unsupported runtime data must fail codegen, not become a zero result.
  // Native execution still verifies that each source expression is valid.
  for (const [name, expression, diagnostic] of [
    ["runtime-quote", ["quote", ["+", "1", "2"]], /unsupported runtime quote value in WASM/],
    ["runtime-quasiquote", ["quasiquote", ["+", "1", "2"]], /unsupported runtime quasiquote value in WASM/],
    ["runtime-format", ["format-to-lisp", "42"], /unsupported runtime format-to-lisp in WASM/],
    ["number-format", ["&number:format", "1.5", "2"], /unsupported proc in WASM: &number:format/],
    ["value-print", ["to-lispy-string", "42"], /unsupported proc in WASM: to-lispy-string/],
    ["host-os", ["&get-os"], /unsupported proc in WASM: &get-os/],
    ["runtime-backend", ["&get-calcit-backend"], /unsupported proc in WASM: &get-calcit-backend/],
    ["runtime-enum-validation", ["&enum:validate", ["Option", ":some", "3"], ":some"], /runtime enum validation is not supported in WASM/],
    ["runtime-builtin-registration", ["register-calcit-builtin-impls", ["format-to-lisp", "42"]], /runtime builtin impl registration must be eliminated before WASM codegen/],
    ["definition-doc", ["&get-def-doc", "|calcit.core/inc"], /unsupported proc in WASM: &get-def-doc/],
    ["definition-schema", ["&get-def-schema", "|calcit.core/inc"], /unsupported proc in WASM: &get-def-schema/],
    ["branch-format", ["if", "true", ["format-to-lisp", "42"], "|fallback"], /unsupported runtime format-to-lisp in WASM/],
  ]) {
    setBody([expression]);
    run();
    const rejectedOutput = join(project, name);
    const rejected = spawnSync(binary, [snapshot, "wasm", "--emit-path", rejectedOutput], options);
    assert.equal(rejected.error, undefined);
    assert.notEqual(rejected.status, 0, `${name} must fail before artifact emission`);
    assert.match(rejected.stderr, diagnostic);
    assert.match(rejected.stderr, /calcit\.assert-evidence\/run-tests/);
    await assert.rejects(readFile(join(rejectedOutput, "program.wasm")), { code: "ENOENT" });
  }

  run("edit", "add-ns", "calcit.placeholder-helper");
  run("edit", "def", "calcit.placeholder-helper/format-value", "--input-format", "cirru", "--code",
    "quote $ defn format-value () $ format-to-lisp 42");
  run("edit", "schema", "calcit.placeholder-helper/format-value", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'String)");
  run("edit", "add-import", "calcit.assert-evidence", "--input-format", "cirru", "--code",
    "quote $ calcit.placeholder-helper :refer $ format-value");
  for (const [name, expression] of [
    ["helper-format", ["format-value"]],
    ["indirect-helper-format", ["let", [["callback", "format-value"]], ["callback"]]],
  ]) {
    setBody([expression]);
    run();
    const helperOutput = join(project, name);
    const helperRejected = spawnSync(binary, [snapshot, "wasm", "--emit-path", helperOutput], options);
    assert.equal(helperRejected.error, undefined);
    assert.notEqual(helperRejected.status, 0, "a reachable failed helper must reject before runtime");
    assert.match(helperRejected.stderr, /reachable `calcit\.placeholder-helper\/format-value` cannot compile/);
    assert.match(helperRejected.stderr, /unsupported runtime format-to-lisp in WASM/);
    await assert.rejects(readFile(join(helperOutput, "program.wasm")), { code: "ENOENT" });
  }

  // Leaving the same helper uncalled must not block a supported export.
  setBody(scalar.map(test => test.code));
  const supportedOutput = join(project, "uncalled-helper");
  run("wasm", "--emit-path", supportedOutput);
  const supportedModule = new WebAssembly.Module(await readFile(join(supportedOutput, "program.wasm")));
  assert.equal(new WebAssembly.Instance(supportedModule, imports).exports["run-tests"](), 1);

  run("edit", "def", "calcit.placeholder-helper/do", "--input-format", "cirru", "--code",
    "quote $ defn do () 42");
  run("edit", "schema", "calcit.placeholder-helper/do", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)");
  run("edit", "add-import", "calcit.assert-evidence", "--overwrite", "--input-format", "cirru", "--code",
    "quote $ calcit.placeholder-helper :as helper");
  run("edit", "add-test", "calcit.assert-evidence/run-tests", "keeps-imported-named-function-value", "--tags", "unit",
    "--input-format", "cirru", "--code", "quote $ assert= 42 $ let ((callback helper/do)) (callback)");
  run("test", "calcit.assert-evidence/run-tests", "--require-match");
  setBody([["assert=", "42", ["let", [["callback", "helper/do"]], ["callback"]]]]);
  run();
  const namedDoOutput = join(project, "named-do-function");
  run("wasm", "--emit-path", namedDoOutput);
  const namedDoModule = new WebAssembly.Module(await readFile(join(namedDoOutput, "program.wasm")));
  assert.equal(new WebAssembly.Instance(namedDoModule, imports).exports["run-tests"](), 1);

  run("edit", "def", "calcit.placeholder-helper/stored-quote", "--input-format", "cirru", "--code",
    "quote $ def stored-quote $ quote $ + 1 2");
  // Keep the alias used by the attached callback regression in later replays.
  setBody(["helper/stored-quote"]);
  run();
  const valueOutput = join(project, "imported-quote");
  const valueRejected = spawnSync(binary, [snapshot, "wasm", "--emit-path", valueOutput], options);
  assert.equal(valueRejected.error, undefined);
  assert.notEqual(valueRejected.status, 0);
  assert.match(valueRejected.stderr, /unsupported runtime quote value in WASM/);
  assert.match(valueRejected.stderr, /imported value `calcit\.placeholder-helper\/stored-quote`/);
  await assert.rejects(readFile(join(valueOutput, "program.wasm")), { code: "ENOENT" });

  // Exercise both runtime-selected branches and distinguish legitimate zero.
  run("edit", "def", "calcit.assert-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
    "quote $ defwasm-export run-tests (flag) $ if flag 0 7");
  run("edit", "schema", "calcit.assert-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'Bool) (:return 'Number)");
  const parameterOutput = join(project, "runtime-parameter");
  run("wasm", "--emit-path", parameterOutput);
  const parameterModule = new WebAssembly.Module(await readFile(join(parameterOutput, "program.wasm")));
  const parameterWasm = new WebAssembly.Instance(parameterModule, imports);
  assert.equal(parameterWasm.exports["run-tests"](1), 0);
  assert.equal(parameterWasm.exports["run-tests"](0), 7);

  run("edit", "def", "calcit.assert-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
    "quote $ defwasm-export run-tests (flag) $ if flag &unit &unit");
  run("edit", "schema", "calcit.assert-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'Bool) (:return 'Unit)");
  const unitOutput = join(project, "legitimate-unit");
  run("wasm", "--emit-path", unitOutput);
  const unitModule = new WebAssembly.Module(await readFile(join(unitOutput, "program.wasm")));
  const unitWasm = new WebAssembly.Instance(unitModule, imports);
  assert.equal(unitWasm.exports["run-tests"](1), 0);
  assert.equal(unitWasm.exports["run-tests"](0), 0);

  run("edit", "def", "calcit.assert-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
    "quote $ defwasm-export run-tests (flag) $ if flag (format-to-lisp 42) |fallback");
  run("edit", "schema", "calcit.assert-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'Bool) (:return 'String)");
  const branchRejected = spawnSync(binary, [snapshot, "wasm", "--emit-path", join(project, "parameter-format")], options);
  assert.equal(branchRejected.error, undefined);
  assert.notEqual(branchRejected.status, 0);
  assert.match(branchRejected.stderr, /unsupported runtime format-to-lisp in WASM/);
  await assert.rejects(readFile(join(project, "parameter-format", "program.wasm")), { code: "ENOENT" });
  setBody(scalar.map(test => test.code));
  run("edit", "schema", "calcit.assert-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)");
  // Source imports are values, not automatically callable namespace bindings.
  run("edit", "add-ns", "calcit.call-values");
  run("edit", "def", "calcit.call-values/site", "--input-format", "cirru", "--code",
    "quote $ def site $ {} (:storage-key |proto-shuangpin)");
  run("edit", "schema", "calcit.call-values/site", "--input-format", "cirru", "--code",
    "quote $ :: 'Map 'Tag 'String");
  run("edit", "def", "calcit.call-values/consume-callback", "--input-format", "cirru", "--code",
    "quote $ defn consume-callback (callback) (callback 1)");
  run("edit", "schema", "calcit.call-values/consume-callback", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([] (:: 'Fn ({} (:args ([] 'Number)) (:return 'Number))))) (:return 'Number)");
  run("edit", "add-import", "calcit.assert-evidence", "--input-format", "cirru", "--code",
    "quote $ calcit.call-values :as config");
  run("edit", "add-test", "calcit.assert-evidence/run-tests", "reads-imported-map", "--tags", "unit",
    "--input-format", "cirru", "--code", "quote $ assert= |proto-shuangpin $ (get config/site :storage-key) .unwrap");
  run("test", "calcit.assert-evidence/run-tests", "--require-match");
  for (const expression of ["config/site :storage-key", "let ((value config/site)) (value :storage-key)"]) {
    run("edit", "def", "calcit.assert-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defwasm-export run-tests () (${expression}) 1`);
    for (const mode of [["--check-only"], ["js"], ["wasm"], ["wasi"]]) {
      const destination = join(project, `non-callable-${mode[0]}`);
      const rejected = spawnSync(binary, ["--emit-path", destination, snapshot, ...mode], options);
      assert.equal(rejected.error, undefined);
      assert.notEqual(rejected.status, 0, `${expression} must reject before code generation`);
      assert.match(`${rejected.stdout}\n${rejected.stderr}`, /non-function.*map</);
      assert.match(`${rejected.stdout}\n${rejected.stderr}`, /calcit\.assert-evidence\/run-tests/);
      await assert.rejects(readFile(join(destination, "program.wasm")), { code: "ENOENT" });
    }
  }
  setBody(scalar.map(test => test.code));

  // The shared preprocessor must reject before execution or either codegen.
  const bad = [
    "let ((x |hello)) (assert-type x 'Number)",
    "let ((x 3)) (assert-type x 'String)",
    "let ((x ([] 1 2))) (assert-type x (:: 'List 'String))",
    "let ((x (Option :some 3))) (assert-type x (:: 'Option 'String))",
    "assert-type |hello 'Number",
    "assert-type 3 'String",
    "assert-type (+ 1 2) 'String",
    "assert-type (hint-fn ({} (:args ([] 'Number)) (:return 'Number))) 'Fn",
    "assert-type ([] 1 2) (:: 'List 'String)",
    "assert-type (Option :some 3) (:: 'Option 'String)",
    "assert-type (Option :some 3) (:: 'Result 'Number 'String)",
  ];
  const detailed = new Map([
    ["assert-type (to-pairs (&{} :a 1)) (:: 'Set 'Enum)",
      ["expected `set<:enum>`", "got `set<list<dynamic>>`"]],
    ["assert-type (.map (Option :some 3) $ fn (x) ([] x)) (:: 'Option (:: 'List 'String))",
      ["expected `'Option<list<:string>>`", "got `'calcit.core/Option<list<:number>>`"]],
    ["assert-type (.map (Result :ok 3) $ fn (x) ([] x)) (:: 'Result (:: 'List 'String) 'String)",
      ["expected `'Result<list<:string>, :string>`", "got `'calcit.core/Result<list<:number>, dynamic>`"]],
    ["assert-type ({} (:a ([] 1))) (:: 'Map 'Tag (:: 'List 'String))",
      ["expected `map<:tag,list<:string>>`", "got `map<:tag,list<:number>>`"]],
    ["assert-type (#{} 1) (:: 'Set 'String)",
      ["expected `set<:string>`", "got `set<:number>`"]],
    ["assert-type (atom 1) (:: 'Ref 'String)",
      ["expected `ref<:string>`", "got `ref<:number>`"]],
  ]);
  bad.push(...detailed.keys());
  const badReturns = [
    "let ((empty (fn () (hint-fn ({} (:args ([])) (:return 'Number)))))) (empty)",
    "let ((f (fn (x) (+ x 1))) (wrong (fn () (hint-fn ({} (:args ([])) (:return 'Number))) 3 (hint-fn f ({} (:args ([] 'Number)) (:return 'Number)))))) (wrong)",
    "let ((facade (fn (callback) (hint-fn ({} (:args ([] 'Fn)) (:return 'Number))) callback))) facade (fn (x) x)",
    "let ((facade (fn (callback) (hint-fn ({} (:args ([] 'Fn)) (:return 'String))) callback))) facade (fn (x) x)",
    "let ((facade (fn (callback) (hint-fn ({} (:args ([] 'Fn)) (:return 'Unit))) callback))) facade (fn (x) x)",
    "let ((facade (fn (callback) (hint-fn ({} (:args ([] 'Fn)) (:return (:: 'List 'Number)))) callback))) facade (fn (x) x)",
    "let ((facade (fn (callback) (hint-fn ({} (:args ([] 'Fn)) (:return (:: 'Option 'Number)))) callback))) facade (fn (x) x)",
    "let ((facade (fn (value) (hint-fn ({} (:args ([] 'String)) (:return 'Number))) value))) facade |hello",
    "let ((facade (fn (value) (hint-fn ({} (:args ([] (:: 'List 'Number))) (:return (:: 'List 'String)))) value))) facade ([] 1 2)",
    "let ((load (fn (x) (hint-fn ({} (:async true) (:args ([] 'Number)) (:return 'Number))) x)) (forward (fn (x) (hint-fn ({} (:args ([] 'Number)) (:return 'Number))) (load x)))) (fn? forward)",
    "let ((load (fn (x) (hint-fn ({} (:async true) (:args ([] 'String)) (:return 'String))) x)) (forward (fn (x) (hint-fn ({} (:async true) (:args ([] 'String)) (:return 'Number))) (load x)))) (fn? forward)",
  ];
  const badCalls = [
    "let ((consume (fn (value) (hint-fn ({} (:args ([] 'Number)) (:return 'Unit))) &unit))) (consume |hello)",
    "let ((consume (fn (value) (hint-fn ({} (:args ([] 'String)) (:return 'Unit))) &unit))) (consume 3)",
    "let ((consume (fn (values) (hint-fn ({} (:args ([] (:: 'List 'Number))) (:return 'Unit))) &unit))) (consume ([] |hello))",
    "let ((consume (fn (value) (hint-fn ({} (:args ([] (:: 'Option 'Number))) (:return 'Unit))) &unit))) (consume (Option :some |hello))",
    "let ((consume (fn (callback) (hint-fn ({} (:args ([] (:: 'Fn ({} (:args ([] 'Number)) (:return 'Number))))) (:return 'Unit))) &unit)) (wrong (fn (x) (hint-fn ({} (:args ([] 'String)) (:return 'String))) x))) (consume wrong)",
    "let ((consume (fn (value) (hint-fn ({} (:args ([] 'Number)) (:return 'Unit))) &unit)) (forward (fn (callback) (hint-fn ({} (:args ([] 'Fn)) (:return 'Unit))) (consume callback)))) (forward (fn (x) x))",
    // Annotation expressions are Nil, including through bindings and helpers.
    "let ((consume (fn (callback) (hint-fn ({} (:args ([] (:: 'Fn ({} (:args ([] 'Number)) (:return 'Number))))) (:return 'Number))) (callback 1)))) (consume (hint-fn ({} (:args ([] 'Number)) (:return 'Number)) (fn (x) x)))",
    "let ((consume (fn (callback) (hint-fn ({} (:args ([] 'Fn)) (:return 'Unit))) &unit)) (callback (hint-fn ({} (:args ([] 'Number)) (:return 'Number))))) (consume callback)",
    "let ((consume (fn (callback) (hint-fn ({} (:args ([] 'Fn)) (:return 'Unit))) &unit)) (f (fn (x) x))) (consume (hint-fn f ({} (:args ([] 'Number)) (:return 'Number))))",
    "let ((consume (fn (callback) (hint-fn ({} (:args ([] 'Fn)) (:return 'Unit))) &unit)) (make-metadata (fn () (hint-fn ({} (:args ([])) (:return 'Nil)))))) (consume (make-metadata))",
  ];
  const rejected = [
    ...bad.map(expression => [expression, "E_ASSERT_TYPE_MISMATCH"]),
    ...badReturns.map(expression => [expression, "W_FN_RETURN_TYPE_MISMATCH"]),
    ...badCalls.map(expression => [expression, "W_LOCAL_FN_ARG_TYPE_MISMATCH"]),
    ["config/consume-callback (hint-fn ({} (:args ([] 'Number)) (:return 'Number)) (fn (x) x))", "W_FN_ARG_TYPE_MISMATCH"],
    ["let ((empty (fn () (hint-fn ({} (:args ([])) (:return 'Unit)))))) (empty)", "E_NIL_FOR_UNIT"],
  ];
  for (const [expression, diagnostic] of rejected) {
    run("edit", "def", "calcit.assert-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defwasm-export run-tests () (${expression}) 1`);
    const modes = [[], ["--check-only"], ["js"], ["wasm"], ["wasm", "--check-only"], ["wasi"], ["wasi", "--check-only"]];
    for (const mode of modes) {
      const result = spawnSync(binary, ["--emit-path", output, snapshot, ...mode], options);
      if (result.error) throw result.error;
      assert.equal(result.status, 1, `${expression} ${mode}\n${result.stdout}\n${result.stderr}`);
      // JS warning diagnostics use stdout and the existing build-error artifact.
      const diagnostics = `${result.stdout}\n${result.stderr}`;
      assert.ok(diagnostics.includes(diagnostic), `${expression} ${mode}\n${diagnostics}`);
      assert.ok(diagnostics.includes("calcit.assert-evidence/run-tests"), diagnostics);
      if (diagnostic === "E_ASSERT_TYPE_MISMATCH") {
        assert.ok(diagnostics.includes("expected") && diagnostics.includes("got"), diagnostics);
      } else if (diagnostic === "W_FN_RETURN_TYPE_MISMATCH") {
        assert.ok(diagnostics.includes("declares return type") && diagnostics.includes("body returns"), diagnostics);
      } else if (diagnostic === "E_NIL_FOR_UNIT") {
        assert.ok(diagnostics.includes("declares Unit but returns nil"), diagnostics);
      } else {
        assert.ok(diagnostics.includes("expects type") && diagnostics.includes("but got"), diagnostics);
      }
      assert.ok(/preprocessing|warnings, (?:runner|codegen) blocked/.test(diagnostics), diagnostics);
      if (mode.includes("wasm") || mode.includes("wasi")) {
        await assert.rejects(readFile(join(output, "program.wasm")), { code: "ENOENT" });
      }
      for (const fragment of detailed.get(expression) ?? []) {
        assert.ok(diagnostics.includes(fragment), `${expression} must preserve ${fragment}\n${diagnostics}`);
      }
    }
  }
  run("edit", "def", "calcit.assert-evidence/WriteState", "--input-format", "cirru", "--code",
    "quote $ defstruct WriteState (:count 'Number) (:label 'String)");
  run("edit", "def", "calcit.assert-evidence/write-count", "--input-format", "cirru", "--code",
    "quote $ defn write-count (state incoming) (state .assoc :count incoming)");
  run("edit", "schema", "calcit.assert-evidence/write-count", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ [] 'WriteState 'Number) (:return 'WriteState)");
  run("edit", "add-test", "calcit.assert-evidence/write-count", "nominal-method-write", "--tags", "nominal-write",
    "--input-format", "cirru", "--code",
    "quote $ let ((original $ WriteState :count 1 :label |kept) (updated $ write-count original 2)) (assert= 1 $ :count original) (assert= 2 $ :count updated) (assert= |kept $ :label updated) (assert= WriteState $ &struct:definition updated)");
  run("test", "calcit.assert-evidence/write-count", "--tag", "nominal-write", "--require-match");
  const nominalTests = JSON.parse(run("query", "def", "calcit.assert-evidence/write-count", "--format", "json"))
    .data.tests.filter(test => test.tags.includes("nominal-write"));
  assert.equal(nominalTests.length, 1);
  setBody(nominalTests.map(test => test.code));
  run("fix", "--rule", "nominal-write-proof-v1", "--ns", "calcit.assert-evidence", "--def", "write-count", "--format", "edn");
  run();
  const nominalOutput = join(project, "nominal-write-js");
  run("--emit-path", nominalOutput, "js");
  const nominalJs = await import(pathToFileURL(join(nominalOutput, "calcit.assert-evidence.mjs")).href);
  assert.equal(nominalJs.run_tests(), 1);
  const nominalWasmOutput = join(project, "nominal-write-wasm");
  run("wasm", "--emit-path", nominalWasmOutput);
  const nominalModule = new WebAssembly.Module(await readFile(join(nominalWasmOutput, "program.wasm")));
  assert.equal(new WebAssembly.Instance(nominalModule, imports).exports["run-tests"](), 1);

  // Low-level stale metadata must trap instead of overwriting another field.
  setBody([["&struct:assoc-at", ["WriteState", ":count", "1", ":label", "|kept"], "1", ":count", "2"]]);
  run("wasm", "--emit-path", nominalWasmOutput);
  const staleModule = new WebAssembly.Module(await readFile(join(nominalWasmOutput, "program.wasm")));
  assert.throws(() => new WebAssembly.Instance(staleModule, imports).exports["run-tests"](), WebAssembly.RuntimeError);

  await copyFile("tests/fixtures/def-value-schema.cirru", snapshot);
  run("test", "--tag", "def-value-contract", "--require-match");
  run("test", "--tag", "diary-boundary", "--require-match");
  const diaryResponse = JSON.parse(run("query", "def", "app.main/verify-login-decode", "--format", "json"));
  assert.deepEqual(diaryResponse.diagnostics, []);
  const diaryTests = diaryResponse.data.tests.filter(test => test.tags.includes("diary-boundary"));
  assert.equal(diaryTests.length, 6);
  // Compile the definition-attached expressions themselves, not a JS rewrite
  // of the application boundary semantics.
  run("edit", "def", "app.main/verify-diary-boundaries", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "verify-diary-boundaries", [], ...diaryTests.map(test => test.code), "&unit"]));
  run("edit", "schema", "app.main/verify-diary-boundaries", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  run("config", "set", "init-fn", "app.main/verify-diary-boundaries");
  run("--check-only");
  const defValueOutput = join(project, "def-value-js-out");
  run("--emit-path", defValueOutput, "js");
  const defValues = await import(pathToFileURL(join(defValueOutput, "app.main.mjs")).href);
  defValues.verify_values();
  defValues.verify_diary_boundaries();
  for (const [target, schema] of [
    ["app.values/initial-state", "quote $ :: 'Map 'Tag 'String"],
    ["app.main/initial-state", "quote $ :: 'Map 'Tag 'String"],
    ["app.main/state-alias", "quote $ :: 'Map 'Tag 'String"],
    ["app.reader/state-alias", "quote $ :: 'Map 'Tag 'String"],
    ["app.main/members", "quote $ :: 'Map 'String 'String"],
    ["app.main/answer", "quote $ :: 'String"],
  ]) {
    await copyFile("tests/fixtures/def-value-schema.cirru", snapshot);
    run("edit", "schema", target, "--input-format", "cirru", "--code", schema);
    const original = await readFile(snapshot);
    for (const mode of [["--check-only"], ["js"], ["wasm", "--check-only"], ["wasi", "--check-only"]]) {
      const result = spawnSync(binary, ["--emit-path", defValueOutput, snapshot, ...mode], options);
      if (result.error) throw result.error;
      assert.equal(result.status, 1, `${target} ${mode}\n${result.stdout}\n${result.stderr}`);
      assert.ok(`${result.stdout}\n${result.stderr}`.includes("E_SCHEMA_DEF_MISMATCH"));
      assert.deepEqual(await readFile(snapshot), original);
    }
  }
  console.log("Known assertions and return/call contracts rejected before native/JS/WASM/WASI; native/JS positives, JS async adoption, scalar WASM assertions, top-level value schema contracts and five Diary boundaries passed");
} finally {
  await rm(project, { recursive: true, force: true });
}
