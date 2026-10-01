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
  run("test", "--tag", "assert-boundary", "--require-match");
  const response = JSON.parse(run("query", "def", "calcit.core/assert-type", "--format", "json"));
  assert.deepEqual(response.diagnostics, []);
  const tests = response.data.tests.filter(test => test.tags.includes("assert-boundary"));
  assert.equal(tests.length, 6);
  run("test", "calcit.core/hint-fn", "--tag", "return-boundary", "--require-match");
  const returnResponse = JSON.parse(run("query", "def", "calcit.core/hint-fn", "--format", "json"));
  assert.deepEqual(returnResponse.diagnostics, []);
  const returnTests = returnResponse.data.tests.filter(test => test.tags.includes("return-boundary"));
  assert.equal(returnTests.length, 3);
  run("test", "calcit.core/hint-fn", "--tag", "call-boundary", "--require-match");
  const callTests = returnResponse.data.tests.filter(test => test.tags.includes("call-boundary"));
  assert.equal(callTests.length, 4);
  run("test", "calcit.core/hint-fn", "--tag", "async-return-boundary", "--require-match");
  const asyncTests = returnResponse.data.tests.filter(test => test.tags.includes("async-return-boundary"));
  assert.equal(asyncTests.length, 1);
  run("test", "calcit.core/quote", "--tag", "quote-return-boundary", "--require-match");
  const quoteResponse = JSON.parse(run("query", "def", "calcit.core/quote", "--format", "json"));
  assert.deepEqual(quoteResponse.diagnostics, []);
  const quoteTests = quoteResponse.data.tests.filter(test => test.tags.includes("quote-return-boundary"));
  assert.equal(quoteTests.length, 3);
  run("edit", "add-ns", "calcit.assert-evidence");
  const setBody = trees => run("edit", "def", "calcit.assert-evidence/run-tests", "--overwrite",
    "--input-format", "json-ast", "--code", JSON.stringify(["defwasm-export", "run-tests", [], ...trees, "1"]));
  setBody([...tests, ...returnTests, ...callTests, ...asyncTests, ...quoteTests].map(test => test.code));
  run("edit", "schema", "calcit.assert-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)");
  run("config", "set", "init-fn", "calcit.assert-evidence/run-tests");
  run("config", "set", "reload-fn", "calcit.assert-evidence/run-tests");
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
    ["runtime-format", ["format-to-lisp", "42"], /unsupported runtime format-to-lisp in WASM/],
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

  // The shared preprocessor must reject before execution or either codegen.
  const bad = [
    "let ((x |hello)) (assert-type x 'Number)",
    "let ((x 3)) (assert-type x 'String)",
    "let ((x ([] 1 2))) (assert-type x (:: 'List 'String))",
    "let ((x (Option :some 3))) (assert-type x (:: 'Option 'String))",
    "assert-type |hello 'Number",
    "assert-type 3 'String",
    "assert-type (+ 1 2) 'String",
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
  ];
  const rejected = [
    ...bad.map(expression => [expression, "E_ASSERT_TYPE_MISMATCH"]),
    ...badReturns.map(expression => [expression, "W_FN_RETURN_TYPE_MISMATCH"]),
    ...badCalls.map(expression => [expression, "W_LOCAL_FN_ARG_TYPE_MISMATCH"]),
  ];
  for (const [expression, diagnostic] of rejected) {
    run("edit", "def", "calcit.assert-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defwasm-export run-tests () (${expression}) 1`);
    const modes = diagnostic === "E_ASSERT_TYPE_MISMATCH"
      ? [[], ["--check-only"], ["js"], ["wasm"]]
      : [[], ["--check-only"], ["js"], ["wasm"], ["wasm", "--check-only"], ["wasi"], ["wasi", "--check-only"]];
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
  console.log("Known assertions and return/call contracts rejected before native/JS/WASM; native/JS positives, JS async adoption and scalar WASM assertions passed");
} finally {
  await rm(project, { recursive: true, force: true });
}
