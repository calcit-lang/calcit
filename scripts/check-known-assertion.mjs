import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, readdir, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const project = await mkdtemp(join(tmpdir(), "calcit-known-assertion-"));
const snapshot = join(project, "calcit.cirru");
const options = { encoding: "utf8", stdio: "pipe", timeout: 180000, maxBuffer: 16 * 1024 * 1024 };
const run = (...args) => execFileSync(binary, [snapshot, ...args], options);

async function assertRejectedArtifacts(output, label, diagnostic, requireDiagnosticArtifact = false) {
  const artifacts = await readdir(output).catch(error => {
    if (error.code !== "ENOENT") throw error;
    return [];
  });
  assert.deepEqual(artifacts.filter(file => file !== "calcit.build-errors.mjs"), [],
    `${label}: rejected preprocessing must not emit application or WASM artifacts`);
  if (requireDiagnosticArtifact) {
    assert.ok(artifacts.includes("calcit.build-errors.mjs"),
      `${label}: JS rejection must emit calcit.build-errors.mjs`);
  }
  if (artifacts.includes("calcit.build-errors.mjs")) {
    assert.match(await readFile(join(output, "calcit.build-errors.mjs"), "utf8"), diagnostic);
  }
}

try {
  // Reuse the source rejection fixture on every preprocessing entry. Codegen
  // must not hide a non-tail recurrence behind an unsupported target feature.
  const recurFixture = join(project, "non-tail-recur.cirru");
  await copyFile("tests/fixtures/non-tail-recur.cirru", recurFixture);
  const recurOriginal = await readFile(recurFixture);
  for (const name of ["bad-in-list", "bad-in-str", "bad-before-tail", "bad-in-try",
    "bad-in-match", "bad-alias", "bad-from-macro"]) {
    for (const mode of [[], ["--check-only"], ["js"], ["wasm"], ["wasm", "--check-only"], ["wasi"], ["wasi", "--check-only"]]) {
      const label = `${name} ${mode.join(" ") || "native"}`;
      const output = join(project, `recur-${name}-${mode.join("-") || "native"}`);
      const rejected = spawnSync(binary, [recurFixture, "--init-fn", `app.main/${name}`,
        "--reload-fn", `app.main/${name}`, "--emit-path", output, ...mode], options);
      if (rejected.error) throw rejected.error;
      assert.equal(rejected.status, 1, `${label}\n${rejected.stdout}\n${rejected.stderr}`);
      assert.match(`${rejected.stdout}\n${rejected.stderr}`, /tail position/, label);
      await assertRejectedArtifacts(output, label, /tail position/, mode[0] === "js");
      assert.deepEqual(await readFile(recurFixture), recurOriginal);
    }
  }

  // These are shared surface contracts, not JavaScript coercion rules. Replay
  // the exact attached ASTs before testing compiler rejection boundaries.
  const truthinessCore = "src/cirru/calcit-core.cirru";
  const truthinessOriginal = await readFile(truthinessCore);
  const truthinessRun = (...args) => execFileSync(binary, [truthinessCore, ...args], options);
  const truthinessReport = JSON.parse(truthinessRun("test", "--tag", "truthiness", "--summary-only", "--require-match", "--format", "json"));
  assert.equal(truthinessReport.selected, 15);
  assert.equal(truthinessReport.passed, 15);
  const truthinessTests = ["if", "or", "and"].flatMap(name =>
    JSON.parse(truthinessRun("query", "def", `calcit.core/${name}`, "--format", "json")).data.tests
      .filter(test => test.tags.includes("truthiness")));
  assert.equal(truthinessTests.length, 15);
  const truthinessSnapshot = join(project, "truthiness.cirru");
  await copyFile(truthinessCore, truthinessSnapshot);
  await symlink(resolve("node_modules"), join(project, "node_modules"), "dir");
  const replay = (...args) => execFileSync(binary, [truthinessSnapshot, ...args], options);
  const truthinessOperations = [["edit", "add-ns", "calcit.truthiness"]];
  for (const [name, type] of [["choose-open", "'Dynamic"], ["choose-bool", "'Bool"]]) {
    truthinessOperations.push(
      ["edit", "def", `calcit.truthiness/${name}`, "--input-format", "json-ast", "--code",
        JSON.stringify(["defn", name, ["condition"], ["if", "condition", "|yes", "|no"]])],
      ["edit", "schema", `calcit.truthiness/${name}`, "--input-format", "cirru", "--code",
        `quote $ :: 'Fn $ {} (:args ([] ${type})) (:return 'String)`],
    );
  }
  truthinessOperations.push(
    ["edit", "def", "calcit.truthiness/run-tests", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "run-tests", [], ...truthinessTests.map(test => test.code),
        ["assert=", "|yes", ["choose-open", "0"]], ["assert=", "|no", ["choose-bool", "false"]], "&unit"])],
    ["edit", "schema", "calcit.truthiness/run-tests", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)"],
  );
  const truthinessRevision = JSON.parse(replay("query", "config", "--format", "json")).revision;
  const truthinessTransaction = ["edit", "transaction", "--code", JSON.stringify(truthinessOperations),
    "--expect-revision", truthinessRevision, "--format", "json"];
  replay(...truthinessTransaction, "--dry-run");
  replay(...truthinessTransaction);
  const truthinessEntry = ["--init-fn", "calcit.truthiness/run-tests", "--reload-fn", "calcit.truthiness/run-tests"];
  replay(...truthinessEntry);
  const truthinessOutput = join(project, "truthiness-js");
  replay(...truthinessEntry, "--emit-path", truthinessOutput, "js");
  const truthinessModule = await import(pathToFileURL(join(truthinessOutput, "calcit.truthiness.mjs")).href);
  truthinessModule.run_tests();
  // Host-only values are not Calcit source literals. Strict identity checks
  // must never invoke coercion hooks or treat NaN/zero/empty text as falsey.
  for (const value of [null, undefined, false]) assert.equal(truthinessModule.choose_open(value), "no");
  for (const value of [0, -0, "", NaN, Infinity, {}, [], Symbol("host"),
    { [Symbol.toPrimitive]() { throw new Error("coercion must not run"); } }]) {
    assert.equal(truthinessModule.choose_open(value), "yes");
  }
  assert.equal(truthinessModule.choose_bool(true), "yes");
  assert.equal(truthinessModule.choose_bool(false), "no");
  const truthinessCode = await readFile(join(truthinessOutput, "calcit.truthiness.mjs"), "utf8");
  assert.match(truthinessCode, /if \(\(\(condition\) \?\? false\) !== false\)/,
    "Bool and open conditions must share single-evaluation Calcit truthiness");
  assert.doesNotMatch(truthinessCode, /_calcit_truthy/, "conditional lowering must not require a new runtime export");
  assert.deepEqual(await readFile(truthinessCore), truthinessOriginal);

  // Replay every stored group whose operations have a supported WASM
  // representation. Local closures specialize their concrete inputs.
  // Atom allocation, try and JS FFI escapes remain separate target boundaries.
  const truthinessWasmTests = truthinessTests.filter(test => [
    "only-nil-false-unit-select-else", "zero-empty-and-nominal-values-select-then",
    "open-values-have-the-same-truthiness", "typed-bool-conditions-preserve-both-branches",
    "nested-expression-and-tail-conditions", "typed-number-conditions-are-truthy",
    "nullable-bool-and-text-conditions", "zero-and-empty-string-do-not-use-fallback",
    "zero-empty-and-falsey-values-preserve-existing-contract", "function-and-definition-values-are-truthy",
  ].includes(test.name));
  assert.equal(truthinessWasmTests.length, 10);
  // Preserve the stored error expressions, but leave the native/JS try handler
  // outside the WASM boundary: raise must propagate as a real runtime trap.
  const truthinessErrorTest = truthinessTests.find(test => test.name === "condition-and-selected-branch-errors-propagate");
  assert.ok(truthinessErrorTest);
  assert.equal(truthinessErrorTest.code[0], "do");
  assert.equal(truthinessErrorTest.code.length, 4);
  const truthinessTraps = ["condition", "then", "else"].map((name, index) => {
    const assertion = truthinessErrorTest.code[index + 1];
    assert.equal(assertion[0], "assert=");
    assert.equal(assertion[1], `|${name}-failure`);
    assert.equal(assertion[2][0], "try");
    const expression = assertion[2][1];
    assert.equal(expression[0], "if");
    assert.equal(expression.length, 4);
    assert.deepEqual(expression[index + 1], ["raise", `|${name}-failure`]);
    return { name: `trap-${name}`, expression };
  });
  const truthinessWasmSnapshot = join(project, "truthiness-wasm.cirru");
  await copyFile(truthinessCore, truthinessWasmSnapshot);
  const truthinessWasmRun = (...args) => execFileSync(binary, [truthinessWasmSnapshot, ...args], options);
  const truthinessWasmOperations = [
    ["edit", "add-ns", "calcit.truthiness-wasm"],
    ["edit", "def", "calcit.truthiness-wasm/run-tests", "--input-format", "json-ast", "--code",
      JSON.stringify(["defwasm-export", "run-tests", [], ...truthinessWasmTests.map(test => test.code), "&unit"])],
    ["edit", "schema", "calcit.truthiness-wasm/run-tests", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)"],
    ...truthinessTraps.flatMap(({ name, expression }) => [
      ["edit", "def", `calcit.truthiness-wasm/${name}`, "--input-format", "json-ast", "--code",
        JSON.stringify(["defwasm-export", name, [], expression])],
      ["edit", "schema", `calcit.truthiness-wasm/${name}`, "--input-format", "cirru", "--code",
        "quote $ :: 'Fn $ {} (:args ([])) (:return 'String)"],
    ]),
  ];
  const truthinessWasmRevision = JSON.parse(truthinessWasmRun("query", "config", "--format", "json")).revision;
  const truthinessWasmTransaction = ["edit", "transaction", "--code", JSON.stringify(truthinessWasmOperations),
    "--expect-revision", truthinessWasmRevision, "--format", "json"];
  truthinessWasmRun(...truthinessWasmTransaction, "--dry-run");
  truthinessWasmRun(...truthinessWasmTransaction);
  const truthinessWasmOutput = join(project, "truthiness-wasm");
  execFileSync(binary, ["wasm", truthinessWasmSnapshot, "--init-fn", "calcit.truthiness-wasm/run-tests",
    "--reload-fn", "calcit.truthiness-wasm/run-tests", "--emit-path", truthinessWasmOutput], options);
  const truthinessWasmModule = new WebAssembly.Module(await readFile(join(truthinessWasmOutput, "program.wasm")));
  const truthinessWasmImports = {};
  for (const { module, name, kind } of WebAssembly.Module.imports(truthinessWasmModule)) {
    assert.equal(kind, "function");
    (truthinessWasmImports[module] ??= {})[name] = () => { throw new Error(`unexpected truthiness host call: ${module}/${name}`); };
  }
  new WebAssembly.Instance(truthinessWasmModule, truthinessWasmImports).exports["run-tests"]();
  for (const { name } of truthinessTraps) {
    assert.throws(() => new WebAssembly.Instance(truthinessWasmModule, truthinessWasmImports).exports[name](),
      WebAssembly.RuntimeError, `${name}: the original condition or selected branch error must trap`);
  }
  assert.deepEqual(await readFile(truthinessCore), truthinessOriginal);

  // Use a stored global-backed Calcit contract to verify single evaluation
  // and the selected branch, rather than replacing the effects with JS code.
  const effectsSnapshot = join(project, "truthiness-effects.cirru");
  await copyFile(truthinessCore, effectsSnapshot);
  const effectsRun = (...args) => execFileSync(binary, [effectsSnapshot, ...args], options);
  execFileSync(binary, ["calcit/test-struct.cirru", "test", "test-struct.main/truthiness-number-once", "--require-match"], options);
  const effectsDefinitions = ["truthiness-count", "truthiness-number-once"].map(name =>
    JSON.parse(execFileSync(binary, ["calcit/test-struct.cirru", "query", "def", `test-struct.main/${name}`, "--format", "json"], options)).data);
  const effectsTests = effectsDefinitions[1].tests.filter(test => test.tags.includes("truthiness"));
  assert.equal(effectsTests.length, 1);
  const effectsOperations = [
    ["edit", "add-ns", "calcit.truthiness-effects"],
    ...effectsDefinitions.flatMap(definition => {
      const target = `calcit.truthiness-effects/${definition.id.split("/")[1]}`;
      const schema = definition.schema && ["::", "'Fn", ["{}",
        ...definition.schema.slice(1).filter(field => field[0] !== ":kind")]];
      return [
        ["edit", "def", target, "--input-format", "json-ast", "--code", JSON.stringify(definition.code)],
        ...(schema ? [["edit", "schema", target, "--input-format", "json-ast", "--code",
          JSON.stringify(schema)]] : []),
      ];
    }),
    ["edit", "def", "calcit.truthiness-effects/run-tests", "--input-format", "json-ast", "--code",
      JSON.stringify(["defwasm-export", "run-tests", [], ...effectsTests.map(test => test.code), "&unit"])],
    ["edit", "schema", "calcit.truthiness-effects/run-tests", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)"],
  ];
  const effectsRevision = JSON.parse(effectsRun("query", "config", "--format", "json")).revision;
  const effectsTransaction = ["edit", "transaction", "--code", JSON.stringify(effectsOperations),
    "--expect-revision", effectsRevision, "--format", "json"];
  effectsRun(...effectsTransaction, "--dry-run");
  effectsRun(...effectsTransaction);
  const effectsEntry = ["--init-fn", "calcit.truthiness-effects/run-tests", "--reload-fn", "calcit.truthiness-effects/run-tests"];
  effectsRun(...effectsEntry);
  const effectsJs = join(project, "truthiness-effects-js");
  effectsRun(...effectsEntry, "--emit-path", effectsJs, "js");
  (await import(pathToFileURL(join(effectsJs, "calcit.truthiness-effects.mjs")).href)).run_tests();
  const effectsWasm = join(project, "truthiness-effects-wasm");
  effectsRun(...effectsEntry, "--emit-path", effectsWasm, "wasm");
  const effectsModule = new WebAssembly.Module(await readFile(join(effectsWasm, "program.wasm")));
  const effectsImports = {};
  for (const { module, name, kind } of WebAssembly.Module.imports(effectsModule)) {
    assert.equal(kind, "function");
    (effectsImports[module] ??= {})[name] = () => { throw new Error(`unexpected conditional effect host call: ${module}/${name}`); };
  }
  new WebAssembly.Instance(effectsModule, effectsImports).exports["run-tests"]();

  // Runtime ABI inputs must not pass only because a literal call was inlined.
  // The attached core tests above remain the shared language contract.
  const scalarSnapshot = join(project, "truthiness-scalars.cirru");
  await copyFile("tests/fixtures/deep-recursion.cirru", scalarSnapshot);
  const scalarRun = (...args) => execFileSync(binary, [scalarSnapshot, ...args], options);
  const scalarEdit = operations => {
    const original = execFileSync(binary, [scalarSnapshot, "query", "config", "--format", "json"], options);
    const transaction = ["edit", "transaction", "--code", JSON.stringify(operations),
      "--expect-revision", JSON.parse(original).revision, "--format", "json"];
    scalarRun(...transaction, "--dry-run");
    scalarRun(...transaction);
  };
  const scalarDefinition = (name, parameters, type, body, returnType = "'Number", features = []) => [
    ["edit", "def", `app.main/${name}`, "--input-format", "json-ast", "--code",
      JSON.stringify(["defwasm-export", name, parameters, body])],
    ["edit", "schema", `app.main/${name}`, "--input-format", "json-ast", "--code",
      JSON.stringify(["::", "'Fn", ["{}", [":args", ["[]", ...type]], [":return", returnType],
        ...(features.length ? [[":features", ["#{}", ...features]]] : [])]])],
  ];
  scalarEdit([
    ["edit", "rm-def", "app.main/f"],
    ...["main!", "reload!"].map(name => ["edit", "def", `app.main/${name}`, "--overwrite",
      "--input-format", "json-ast", "--code", JSON.stringify(["defn", name, [], "&unit"])]),
    ...scalarDefinition("choose-number", ["value"], ["'Number"], ["if", "value", "1", "2"]),
    ...scalarDefinition("erase-refinement", ["value"], ["'UInt32"], ["unsafe-coerce", "value", "'Number"], "'Number", [":js-ffi"]),
    ...scalarDefinition("choose-alias", ["value"], ["'Number"], ["let", [["alias", "value"]], ["if", "alias", "1", "2"]]),
    ...scalarDefinition("choose-bool", ["value"], ["'Bool"], ["if", "value", "1", "2"]),
    // Nullable evidence is inferred inside the function; public schemas keep
    // the current nominal-absence contract rather than exposing legacy Optional.
    ...scalarDefinition("choose-nullable-bool", ["flag"], ["'Bool"],
      ["let", [["value", ["if", "flag", "true", "nil"]]], ["if", "value", "1", "2"]]),
    ...scalarDefinition("choose-nullable-text", ["flag"], ["'Bool"],
      ["let", [["value", ["if", "flag", "|", "nil"]]], ["if", "value", "1", "2"]]),
    ...scalarDefinition("empty-text", [], [], "|", "'String"),
    ...scalarDefinition("a-first-function", [], [], "0"),
    ...scalarDefinition("first-function", [], [], "a-first-function", ["::", "'Fn", ["{}", [":args", ["[]"]], [":return", "'Number"]]]),
    ...scalarDefinition("choose-function", [], [], ["if", "a-first-function", "1", "2"]),
  ]);
  const scalarOutput = join(project, "truthiness-scalars-wasm");
  scalarRun("--emit-path", scalarOutput, "wasm");
  const scalarModule = new WebAssembly.Module(await readFile(join(scalarOutput, "program.wasm")));
  const scalarImports = {};
  for (const { module, name, kind } of WebAssembly.Module.imports(scalarModule)) {
    assert.equal(kind, "function");
    (scalarImports[module] ??= {})[name] = () => { throw new Error(`unexpected scalar host call: ${module}/${name}`); };
  }
  const scalarExports = new WebAssembly.Instance(scalarModule, scalarImports).exports;
  for (const value of [0, -0, 7, -7, NaN, Infinity, -Infinity]) {
    assert.equal(scalarExports["choose-number"](value), 1);
    assert.equal(scalarExports["choose-alias"](value), 1);
  }
  for (const value of [0, 7, 4294967295]) assert.equal(scalarExports["erase-refinement"](value), value);
  for (const name of ["choose-bool", "choose-nullable-bool"]) {
    assert.equal(scalarExports[name](0), 2);
    assert.equal(scalarExports[name](1), 1);
  }
  assert.equal(scalarExports["choose-nullable-text"](0), 2);
  const emptyText = scalarExports["empty-text"]();
  assert.notEqual(emptyText, 0, "an empty String must retain a nonzero heap identity");
  assert.equal(scalarExports["choose-nullable-text"](1), 1);
  assert.equal(scalarExports["first-function"](), 0, "exercise the zero-based function table slot");
  assert.equal(scalarExports["choose-function"](), 1);

  // Ambiguous open/nullable scalar inputs and unsafe reinterpretations fail
  // closed on both core WASM and the explicit Preview 1 command boundary.
  const scalarPositive = await readFile(scalarSnapshot);
  for (const [label, parameters, types, body, diagnostic, features] of [
    ["open", ["value"], ["'Dynamic"], ["if", "value", "1", "2"], /E_WASM_NIL_TYPE_EVIDENCE/, []],
    ["nullable-number", ["flag"], ["'Bool"], ["let", [["value", ["if", "flag", "0", "nil"]]],
      ["if", "value", "1", "2"]], /E_WASM_NIL_TYPE_EVIDENCE/, []],
    ["nullable-function", ["flag"], ["'Bool"], ["let", [["value", ["if", "flag", "a-first-function", "nil"]]],
      ["if", "value", "1", "2"]], /E_WASM_NIL_TYPE_EVIDENCE/, []],
    ["unsafe-direct", [], [], ["if", ["unsafe-coerce", "0", "'Bool"], "1", "2"], /E_WASM_UNSUPPORTED_JS_FFI/, [":js-ffi"]],
    ["unsafe-alias", [], [], ["let", [["flag", ["unsafe-coerce", "0", "'Bool"]]], ["if", "flag", "1", "2"]], /E_WASM_UNSUPPORTED_JS_FFI/, [":js-ffi"]],
    ["unsafe-producer", [], [], ["let", [["as-bool", ["fn", [],
      ["hint-fn", ["{}", [":args", ["[]"]], [":return", "'Bool"], [":features", ["#{}", ":js-ffi"]]]],
      ["unsafe-coerce", "0", "'Bool"]]]], ["if", ["as-bool"], "1", "2"]], /E_WASM_UNSUPPORTED_JS_FFI/, [":js-ffi"]],
    ["unsafe-nil", [], [], ["if", ["unsafe-coerce", "nil", "'Number"], "1", "2"], /E_WASM_UNSUPPORTED_JS_FFI/, [":js-ffi"]],
    ["unsafe-unit", [], [], ["if", ["unsafe-coerce", "&unit", "'Number"], "1", "2"], /E_WASM_UNSUPPORTED_JS_FFI/, [":js-ffi"]],
    ["unsafe-refinement", ["value"], ["'Number"], ["let", [["refined", ["unsafe-coerce", "value", "'UInt32"]]],
      ["if", "refined", "1", "2"]], /E_WASM_UNSUPPORTED_JS_FFI/, [":js-ffi"]],
  ]) {
    scalarEdit(scalarDefinition(`reject-${label}`, parameters, types, body, "'Number", features));
    for (const [target, extra] of [["wasm", []], ["wasi", ["--boundary", "native"]]]) {
      const output = join(project, `truthiness-${label}-${target}`);
      const result = spawnSync(binary, [target, scalarSnapshot, ...extra, "--emit-path", output], options);
      assert.notEqual(result.status, 0, `${label}/${target} must reject ambiguous representation`);
      assert.match(result.stderr, diagnostic);
      await assertRejectedArtifacts(output, `${label}/${target}`, diagnostic);
    }
    scalarEdit([["edit", "rm-def", `app.main/reject-${label}`]]);
  }
  assert.deepEqual(await readFile(scalarSnapshot), scalarPositive);

  // Replay the stored language contracts; nullability introduction must not
  // authorize elimination, mutable widening, or unproved callback signatures.
  await copyFile("calcit/test-struct.cirru", snapshot);
  await copyFile("calcit/util.cirru", join(project, "util.cirru"));
  // Environment results come from the raw operation, not a wrapper's return
  // declaration. Replay the same stored contracts with controlled host values.
  const envOwners = ["read-env-option", "read-env-default", "read-env-null-default",
    "read-env-open-default", "read-env-number-default"];
  const envDefinitions = envOwners.map(name =>
    JSON.parse(run("query", "def", `test-struct.main/${name}`, "--format", "json")).data);
  const envTests = envDefinitions.flatMap(definition => definition.tests.filter(test => test.tags.includes("env-proof")));
  const envCoreTests = JSON.parse(truthinessRun("query", "def", "calcit.core/get-env", "--format", "json")).data.tests;
  assert.equal(envTests.length, 3);
  assert.equal(envCoreTests.length, 2);
  const envSnapshot = join(project, "env-workflow.cirru");
  await copyFile("tests/fixtures/deep-recursion.cirru", envSnapshot);
  const envRun = (...args) => execFileSync(binary, [envSnapshot, ...args], options);
  const envOperations = [["edit", "rm-def", "app.main/f"]];
  for (const definition of envDefinitions) {
    const name = definition.id.split("/")[1];
    assert.equal(definition.schema[0], "{}");
    assert.deepEqual(definition.schema.find(field => field[0] === ":kind"), [":kind", ":fn"]);
    const schema = ["::", "'Fn", ["{}", ...definition.schema.slice(1).filter(field => field[0] !== ":kind")]];
    envOperations.push(
      ["edit", "def", `app.main/${name}`, "--input-format", "json-ast", "--code", JSON.stringify(definition.code)],
      ["edit", "schema", `app.main/${name}`, "--input-format", "json-ast", "--code", JSON.stringify(schema)],
    );
  }
  envOperations.push(
    ["edit", "def", "app.main/main!", "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "main!", [], ...[...envCoreTests, ...envTests].map(test => test.code), "&unit"])],
    ["edit", "def", "app.main/reload!", "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "reload!", [], "&unit"])],
  );
  const envRevision = JSON.parse(envRun("query", "config", "--format", "json")).revision;
  const envTransaction = ["edit", "transaction", "--code", JSON.stringify(envOperations), "--expect-revision", envRevision, "--format", "json"];
  const envBefore = await readFile(envSnapshot);
  envRun(...envTransaction, "--dry-run");
  assert.deepEqual(await readFile(envSnapshot), envBefore);
  envRun(...envTransaction);
  const envSource = await readFile(envSnapshot);
  envRun("fix", "--workflow", "strict", "--verify", "--format", "edn");
  assert.deepEqual(await readFile(envSnapshot), envSource);
  const envOutput = join(project, "env-js");
  envRun("--emit-path", envOutput, "js");
  const envModule = await import(pathToFileURL(join(envOutput, "app.main.mjs")).href);
  const envKey = "__CALCIT_ENV_PROOF_1788__";
  const missingEnvKey = "__CALCIT_ENV_MISSING_7E01__";
  const savedEnv = new Map([envKey, missingEnvKey].map(key => [key, process.env[key]]));
  try {
    delete process.env[missingEnvKey];
    for (const value of [undefined, "", "hello", "你好 🌱"]) {
      if (value === undefined) delete process.env[envKey];
      else process.env[envKey] = value;
      execFileSync(binary, [envSnapshot], { ...options, env: process.env });
      envModule.main_$x_();
      // The host supplies exact text. Empty text is present, never None or a
      // fallback; native and generated JS replay the stored assertions above.
      assert.equal(envModule.read_env_default(envKey, "fallback"), value ?? "fallback");
    }
  } finally {
    for (const [key, value] of savedEnv) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
  }
  assert.deepEqual(await readFile(envSnapshot), envSource);
  // A fallback with genuinely open or incompatible evidence cannot be
  // narrowed by the caller's return declaration, including through an alias.
  for (const [name, args, types, body, returned] of [
    ["env-wrong-number", ["name", "fallback"], ["'String", "'Number"], ["&get-env", "name", "fallback"], "'String"],
    ["env-open-text", ["name", "fallback"], ["'String", "'Dynamic"], ["&get-env", "name", "fallback"], "'String"],
    ["env-nil-text", ["name"], ["'String"], ["&get-env", "name", "nil"], "'String"],
    ["env-optional-number", ["name"], ["'String"], ["optionally", ["&get-env", "name"]], ["::", "'Option", "'Number"]],
    ["env-nominal-default", ["name", "fallback"], ["'String", ["::", "'Option", "'String"]],
      ["&get-env", "name", "fallback"], ["::", "'Option", "'String"]],
    ["env-alias-text", ["name", "fallback"], ["'String", "'Dynamic"],
      ["let", [["alias", "fallback"]], ["&get-env", "name", "alias"]], "'String"],
  ]) {
    const target = `app.main/${name}`;
    envRun("edit", "def", target, "--input-format", "json-ast", "--code", JSON.stringify(["defn", name, args, body]));
    envRun("edit", "schema", target, "--input-format", "json-ast", "--code",
      JSON.stringify(["::", "'Fn", ["{}", [":args", ["[]", ...types]], [":return", returned]]]));
    const before = await readFile(envSnapshot);
    const rejected = spawnSync(binary, [envSnapshot, "fix", "--rule", "concrete-return-proof-v1", "--ns", "app.main", "--def", name, "--format", "json"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, target);
    if (rejected.stdout.trim()) {
      const diagnostics = JSON.parse(rejected.stdout).diagnostics;
      assert.ok(diagnostics.some(d => d.code === "E_FN_RETURN_UNPROVEN" && d.definition === target), target);
    } else {
      // A concrete mismatch can fail ordinary preprocessing before the proof
      // preview is produced. Require the actual return diagnostic and owner.
      assert.match(rejected.stderr, /W_FN_RETURN_TYPE_MISMATCH/, `${target}: ${rejected.stderr}`);
      assert.ok(rejected.stderr.includes(name), target);
    }
    assert.deepEqual(await readFile(envSnapshot), before);
  }

  // Reset returns independent assigned-value evidence, not Unit or a borrowed
  // receiver payload. Replay the stored source contracts on native and JS.
  const resetOwners = ["assign-number", "assign-alias", "assign-option", "assign-open", "reset-proof-count", "assign-global"];
  const resetDefinitions = resetOwners.map(name =>
    JSON.parse(execFileSync(binary, ["calcit/test-struct.cirru", "query", "def", `test-struct.main/${name}`, "--format", "json"], options)).data);
  const resetTests = resetDefinitions.flatMap(definition => definition.tests.filter(test => test.tags.includes("reset-proof")));
  const resetCoreTests = ["reset!", "swap!"].flatMap(name =>
    JSON.parse(truthinessRun("query", "def", `calcit.core/${name}`, "--format", "json")).data.tests
      .filter(test => test.tags.includes("reset-proof")));
  assert.equal(resetTests.length, 4);
  assert.equal(resetCoreTests.length, 4);
  const resetSnapshot = join(project, "reset-result.cirru");
  await copyFile("tests/fixtures/deep-recursion.cirru", resetSnapshot);
  const resetRun = (...args) => execFileSync(binary, [resetSnapshot, ...args], options);
  const resetOperations = [["edit", "rm-def", "app.main/f"]];
  for (const definition of resetDefinitions) {
    const name = definition.id.split("/")[1];
    const schema = name === "reset-proof-count" ? ["::", "'Ref", "'Number"]
      : ["::", "'Fn", ["{}", ...definition.schema.slice(1).filter(field => field[0] !== ":kind")]];
    resetOperations.push(
      ["edit", "def", `app.main/${name}`, "--input-format", "json-ast", "--code", JSON.stringify(definition.code)],
      ["edit", "schema", `app.main/${name}`, "--input-format", "json-ast", "--code", JSON.stringify(schema)],
    );
  }
  resetOperations.push(
    ["edit", "def", "app.main/main!", "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "main!", [], ...[...resetTests, ...resetCoreTests].map(test => test.code), "&unit"])],
    ["edit", "def", "app.main/reload!", "--overwrite", "--input-format", "cirru", "--code", "quote $ defn reload! () &unit"],
  );
  const resetRevision = JSON.parse(resetRun("query", "config", "--format", "json")).revision;
  const resetTransaction = ["edit", "transaction", "--code", JSON.stringify(resetOperations), "--expect-revision", resetRevision, "--format", "json"];
  const resetBefore = await readFile(resetSnapshot);
  resetRun(...resetTransaction, "--dry-run");
  assert.deepEqual(await readFile(resetSnapshot), resetBefore);
  resetRun(...resetTransaction);
  const resetSource = await readFile(resetSnapshot);
  resetRun("fix", "--workflow", "strict", "--verify", "--format", "edn");
  resetRun();
  const resetOutput = join(project, "reset-js");
  resetRun("--emit-path", resetOutput, "js");
  (await import(pathToFileURL(join(resetOutput, "app.main.mjs")).href)).main_$x_();
  assert.deepEqual(await readFile(resetSnapshot), resetSource);

  // The numeric global-atom contract is already supported on WASM; local
  // atom allocation and generic Ref forwarding remain explicit boundaries.
  const resetWasm = join(project, "reset-global.wasm.cirru");
  await copyFile("tests/fixtures/deep-recursion.cirru", resetWasm);
  const resetWasmRun = (...args) => execFileSync(binary, [resetWasm, ...args], options);
  const resetWasmOperations = resetOperations.filter(operation =>
    operation[0] === "edit" && ["app.main/f", "app.main/reset-proof-count", "app.main/assign-global"].includes(operation[2]));
  const resetWasmTests = resetTests.filter(test => test.tags.includes("reset-wasm"));
  assert.equal(resetWasmTests.length, 1);
  resetWasmOperations.push(
    ...["main!", "reload!"].map(name => ["edit", "def", `app.main/${name}`, "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", name, [], "&unit"])]),
    ["edit", "def", "app.main/run-tests", "--input-format", "json-ast", "--code",
      JSON.stringify(["defwasm-export", "run-tests", [], ...resetWasmTests.map(test => test.code), "&unit"])],
    ["edit", "schema", "app.main/run-tests", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)"],
  );
  const resetWasmRevision = JSON.parse(resetWasmRun("query", "config", "--format", "json")).revision;
  const resetWasmTransaction = ["edit", "transaction", "--code", JSON.stringify(resetWasmOperations), "--expect-revision", resetWasmRevision, "--format", "json"];
  resetWasmRun(...resetWasmTransaction, "--dry-run");
  resetWasmRun(...resetWasmTransaction);
  const resetWasmOutput = join(project, "reset-global-wasm");
  resetWasmRun("--init-fn", "app.main/run-tests", "--reload-fn", "app.main/run-tests", "--emit-path", resetWasmOutput, "wasm");
  const resetModule = new WebAssembly.Module(await readFile(join(resetWasmOutput, "program.wasm")));
  const resetImports = {};
  for (const { module, name, kind } of WebAssembly.Module.imports(resetModule)) {
    assert.equal(kind, "function");
    (resetImports[module] ??= {})[name] = () => { throw new Error(`unexpected reset host call: ${module}/${name}`); };
  }
  new WebAssembly.Instance(resetModule, resetImports).exports["run-tests"]();

  for (const [name, types, returned, body, generics = [], features = []] of [
    ["reset-is-not-unit", [["::", "'Ref", "'Number"], "'Number"], "'Unit", ["reset!", "source", "value"]],
    ["reset-wrong-return", [["::", "'Ref", "'Bool"], "'Bool"], "'Number", ["reset!", "source", "value"]],
    ["reset-open-return", [["::", "'Ref", "'Dynamic"], "'Dynamic"], "'String", ["reset!", "source", "value"]],
    ["reset-open-alias", [["::", "'Ref", "'Dynamic"], "'Dynamic"], "'Number", ["let", [["alias", "value"]], ["reset!", "source", "alias"]]],
    ["reset-unsafe-return", [["::", "'Ref", "'Number"], "'Dynamic"], "'Number", ["reset!", "source", ["unsafe-coerce", "value", "'Number"]], [], [":js-ffi"]],
    ["reset-wrong-write", [["::", "'Ref", "'Number"], "'String"], "'String", ["reset!", "source", "value"]],
    ["reset-unrelated-generic", [["::", "'Ref", "'T"], "'U"], "'U", ["reset!", "source", "value"], ["'T", "'U"]],
    ["reset-borrowed-payload", [["::", "'Ref", "'Number"], "'Dynamic"], "'Number", ["reset!", "source", "value"]],
  ]) {
    const target = `app.main/${name}`;
    resetRun("edit", "def", target, "--input-format", "json-ast", "--code", JSON.stringify(["defn", name, ["source", "value"], body]));
    resetRun("edit", "schema", target, "--input-format", "json-ast", "--code", JSON.stringify(["::", "'Fn", ["{}",
      [":args", ["[]", ...types]], [":return", returned], [":generics", ["[]", ...generics]], [":features", ["#{}", ...features]]]]));
    const before = await readFile(resetSnapshot);
    const rejected = spawnSync(binary, [resetSnapshot, "fix", "--rule", "concrete-return-proof-v1", "--ns", "app.main", "--def", name, "--format", "json"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, target);
    if (rejected.stdout.trim()) {
      assert.ok(JSON.parse(rejected.stdout).diagnostics.some(d => d.code === "E_FN_RETURN_UNPROVEN" && d.definition === target), target);
    } else {
      assert.match(rejected.stderr, /W_FN_RETURN_TYPE_MISMATCH|W_RESET_ARG_TYPE_MISMATCH/, `${target}: ${rejected.stderr}`);
      assert.ok(rejected.stderr.includes(name), target);
    }
    assert.deepEqual(await readFile(resetSnapshot), before);
    resetRun("edit", "rm-def", target);
  }

  // Join independent normal/handler results, preserving the runtime's String
  // input and lazy effects. Replay the attached AST on both supported backends.
  const tryOwners = ["try-bool", "try-string", "try-option", "try-result", "try-normal-never",
    "try-handler-never", "try-lazy!", "try-effect-order", "try-imported-handler", "try-proc-handler", "try-hinted-handler", "try-factory-never", "try-rest-handler", "try-rest-ignored", "try-rest-prefix", "try-optional-handler"];
  run("test", "--tag", "try-proof", "--require-match");
  const tryDefinitions = tryOwners.map(name =>
    JSON.parse(run("query", "def", `test-struct.main/${name}`, "--format", "json")).data);
  const tryTests = tryDefinitions.flatMap(definition => definition.tests.filter(test => test.tags.includes("try-proof")));
  assert.equal(tryTests.length, 30);
  for (const name of tryOwners) {
    const original = await readFile(snapshot);
    run("fix", "--rule", "concrete-return-proof-v1", "--ns", "test-struct.main", "--def", name, "--format", "edn");
    assert.deepEqual(await readFile(snapshot), original);
  }
  run("edit", "def", "test-struct.main/try-replay", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "try-replay", [], ...tryTests.map(test => test.code), "&unit"]));
  run("edit", "schema", "test-struct.main/try-replay", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)");
  const tryEntry = ["--init-fn", "test-struct.main/try-replay", "--reload-fn", "test-struct.main/try-replay"];
  run(...tryEntry);
  const tryOutput = join(project, "try-js");
  run(...tryEntry, "--emit-path", tryOutput, "js");
  (await import(pathToFileURL(join(tryOutput, "test-struct.main.mjs")).href)).try_replay();

  // Exercise the complete strict workflow without pulling unrelated util
  // implementation proofs into this minimal language contract.
  const trySnapshot = join(project, "try-workflow.cirru");
  await copyFile("tests/fixtures/deep-recursion.cirru", trySnapshot);
  const tryRun = (...args) => execFileSync(binary, [trySnapshot, ...args], options);
  const tryOperations = [
    ["edit", "rm-def", "app.main/f"],
    ["edit", "def", "app.main/main!", "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "main!", [], ...tryTests.map(test => test.code), "&unit"])],
    ["edit", "def", "app.main/reload!", "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "reload!", [], "&unit"])],
  ];
  for (const definition of tryDefinitions) {
    const target = definition.id.replace("test-struct.main/", "app.main/");
    const contract = ["::", "'Fn", ["{}", ...definition.schema.slice(1).filter(field => field[0] !== ":kind")]];
    tryOperations.push(
      ["edit", "def", target, "--input-format", "json-ast", "--code", JSON.stringify(definition.code)],
      ["edit", "schema", target, "--input-format", "json-ast", "--code", JSON.stringify(contract)],
    );
    for (const test of definition.tests) {
      tryOperations.push(["edit", "add-test", target, test.name, "--tags", test.tags.join(","),
        "--input-format", "json-ast", "--code", JSON.stringify(test.code)]);
    }
  }
  const tryRevision = JSON.parse(tryRun("query", "config", "--format", "json")).revision;
  const tryTransaction = ["edit", "transaction", "--code", JSON.stringify(tryOperations),
    "--expect-revision", tryRevision, "--format", "json"];
  tryRun(...tryTransaction, "--dry-run");
  tryRun(...tryTransaction);
  const tryOriginal = await readFile(trySnapshot);
  tryRun("fix", "--workflow", "strict", "--verify", "--format", "edn");
  tryRun("--check-only");
  tryRun("test", "--tag", "try-proof", "--require-match");
  assert.deepEqual(await readFile(trySnapshot), tryOriginal);

  // Open values and incompatible inputs/results must not obtain a proof from
  // the enclosing concrete declaration. No new checker or migration is used.
  for (const [label, argumentType, body, definite] of [
    ["wrong-error-input", "'Bool", "try 7 $ fn (message) (hint-fn $ {} (:args ([] 'Number)) (:return 'Number)) 0", true],
    // As with if, incompatible branch joins stay open in ordinary migration
    // mode; the independent concrete-return gate must reject the result.
    ["wrong-handler-result", "'Bool", "try 7 $ fn (message) |wrong", false],
    ["contradictory-handler-hint", "'Bool", "try 7 $ fn (message) (hint-fn $ {} (:args ([] 'String)) (:return 'Number)) |wrong", true],
    ["not-callable", "'Bool", "try 7 1", true],
    ["too-few-handler-parameters", "'Bool", "try 7 $ fn () 0", false],
    ["hint-cannot-invent-handler-parameters", "'Bool", "try 7 $ fn () (hint-fn $ {} (:args ([] 'String)) (:return 'Number)) 0", false],
    ["fixed-context-cannot-prove-rest-inputs", "'Bool", "try 7 $ fn (& messages) $ .count $ .trim messages", false],
    ["wrong-rest-input", "'Bool", "try 7 $ fn (& messages) (hint-fn $ {} (:args ([])) (:rest 'Number) (:return 'Number)) (.count messages)", true],
    ["variadic-fixed-option-is-not-omittable", "'Bool", "try 7 $ fn (message extra & tail) (hint-fn $ {} (:args ([] 'String (:: 'Option 'Number))) (:rest 'String) (:return 'Number)) 0", false],
    ["too-many-handler-parameters", "'Bool", "try 7 $ fn (message other) 0", false],
    ["open-normal-value", "'Dynamic", "try raw $ fn (message) 0", false],
    ["open-handler-value", "'Dynamic", "try 7 $ fn (message) raw", false],
    ["open-callable", "'DynFn", "try 7 raw", false],
  ]) {
    const target = "test-struct.main/try-rejected";
    run("edit", "def", target, "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn try-rejected (raw) $ ${body}`);
    run("edit", "schema", target, "--input-format", "cirru", "--code",
      `quote $ :: 'Fn $ {} (:args ([] ${argumentType})) (:return 'Number)`);
    const original = await readFile(snapshot);
    const diagnostic = /E_FN_RETURN_UNPROVEN|E_CALL_ARGUMENT_MISMATCH|E_DYNAMIC_METHOD_DISPATCH|W_FN_RETURN_TYPE_MISMATCH|W_LOCAL_FN_ARG_TYPE_MISMATCH|trying to call a non-function value|js-ffi/;
    const audit = spawnSync(binary, [snapshot, "fix", "--rule", "concrete-return-proof-v1", "--ns", "test-struct.main",
      "--def", "try-rejected", "--format", "edn"], options);
    if (audit.error) throw audit.error;
    assert.equal(audit.status, 1, `${label}\n${audit.stdout}\n${audit.stderr}`);
    assert.match(`${audit.stdout}\n${audit.stderr}`, diagnostic);
    assert.deepEqual(await readFile(snapshot), original);
    if (definite) {
      for (const mode of [["--check-only"], ["js"]]) {
        const output = join(project, `try-${label}-${mode[0] === "js" ? "js" : "native"}`);
        const rejected = spawnSync(binary, [snapshot, "--init-fn", target, "--reload-fn", target,
          "--emit-path", output, ...mode], options);
        if (rejected.error) throw rejected.error;
        assert.equal(rejected.status, 1, `${label}\n${rejected.stdout}\n${rejected.stderr}`);
        assert.match(`${rejected.stdout}\n${rejected.stderr}`, diagnostic);
        assert.deepEqual(await readFile(snapshot), original);
        if (mode[0] === "js") await assertRejectedArtifacts(output, label, diagnostic, true);
      }
    }
  }
  // Host capability checking is a JS-codegen boundary, not a native return
  // audit. Parameter context neither grants nor removes lexical permission.
  for (const authorized of [false, true]) {
    const target = "test-struct.main/try-rejected";
    run("edit", "def", target, "--overwrite", "--input-format", "cirru", "--code",
      "quote $ defn try-rejected () $ try (raise |fixture-failure) $ fn (message) (js-get message |length) 0");
    run("edit", "schema", target, "--input-format", "cirru", "--code",
      `quote $ :: 'Fn $ {} (:args ([])) (:return 'Number)${authorized ? " (:features $ #{} :js-ffi)" : ""}`);
    const original = await readFile(snapshot);
    const output = join(project, `try-ffi-${authorized ? "authorized" : "rejected"}`);
    if (authorized) {
      run("--init-fn", target, "--reload-fn", target, "--emit-path", output, "js");
      assert.equal((await import(pathToFileURL(join(output, "test-struct.main.mjs")).href)).try_rejected(), 0);
    } else {
      const rejected = spawnSync(binary, [snapshot, "--init-fn", target, "--reload-fn", target,
        "--emit-path", output, "js"], options);
      if (rejected.error) throw rejected.error;
      assert.equal(rejected.status, 1, `${rejected.stdout}\n${rejected.stderr}`);
      assert.match(`${rejected.stdout}\n${rejected.stderr}`, /E_JS_FFI_FEATURE_REQUIRED/);
      await assertRejectedArtifacts(output, "try-ffi-without-permission", /E_JS_FFI_FEATURE_REQUIRED/, true);
    }
    assert.deepEqual(await readFile(snapshot), original);
  }
  run("edit", "rm-def", "test-struct.main/try-rejected");
  // Concrete Optional field admission preserves evidence without authorizing
  // elimination, open payloads, or mutable Ref widening (Diary #61/#64).
  run("test", "--tag", "optional-proof", "--require-match");
  const optionalOwners = ["OptionalFields", "set-optional-fields", "OptionalGeneric", "set-optional-generic"];
  const optionalDefinitions = optionalOwners.map(name =>
    JSON.parse(run("query", "def", `test-struct.main/${name}`, "--format", "json")).data);
  const optionalFieldTests = optionalDefinitions.flatMap(definition =>
    definition.tests.filter(test => test.tags.includes("optional-proof")));
  assert.equal(optionalFieldTests.length, 5);
  for (const name of ["set-optional-fields", "set-optional-generic"]) {
    const before = await readFile(snapshot);
    run("fix", "--rule", "nominal-write-proof-v1", "--ns", "test-struct.main", "--def", name, "--format", "edn");
    assert.deepEqual(await readFile(snapshot), before);
  }
  run("edit", "def", "test-struct.main/optional-replay", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "optional-replay", [], ...optionalFieldTests.map(test => test.code), "&unit"]));
  run("edit", "schema", "test-struct.main/optional-replay", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)");
  const optionalEntry = ["--init-fn", "test-struct.main/optional-replay", "--reload-fn", "test-struct.main/optional-replay"];
  run(...optionalEntry);
  const optionalFieldOutput = join(project, "optional-js");
  run(...optionalEntry, "--emit-path", optionalFieldOutput, "js");
  (await import(pathToFileURL(join(optionalFieldOutput, "test-struct.main.mjs")).href)).optional_replay();

  // Replay the same source in a dependency-free project to exercise the full
  // strict workflow, not just a selected nominal-write migration rule.
  const workflowSnapshot = join(project, "optional-workflow.cirru");
  await copyFile("tests/fixtures/deep-recursion.cirru", workflowSnapshot);
  const workflowRun = (...args) => execFileSync(binary, [workflowSnapshot, ...args], options);
  const workflowSchema = node => Array.isArray(node) ? node.map(workflowSchema)
    : typeof node === "string" && node.startsWith("'test-struct.main/") ? node.replace("'test-struct.main/", "'app.main/") : node;
  const workflowOperations = [
    ["edit", "rm-def", "app.main/f"],
    ["edit", "def", "app.main/main!", "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "main!", [], ["set-optional-fields", "7", "|saved"],
        ["set-optional-generic", ["OptionalGeneric", ":value", "1"], "9"], "&unit"])],
    ["edit", "def", "app.main/reload!", "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "reload!", [], "&unit"])],
  ];
  for (const definition of optionalDefinitions) {
    const target = definition.id.replace("test-struct.main/", "app.main/");
    const queriedSchema = workflowSchema(definition.schema);
    // Query exposes a callable contract map; edit schema requires its Fn wrapper.
    const schema = Array.isArray(queriedSchema) && queriedSchema[0] === "{}"
      ? ["::", "'Fn", ["{}", ...queriedSchema.slice(1).filter(field => field[0] !== ":kind")]] : queriedSchema;
    workflowOperations.push(
      ["edit", "def", target, "--input-format", "json-ast", "--code", JSON.stringify(definition.code)],
      ["edit", "schema", target, "--input-format", "json-ast", "--code", JSON.stringify(schema)],
    );
    for (const test of definition.tests) {
      workflowOperations.push(["edit", "add-test", target, test.name, "--tags", test.tags.join(","),
        "--input-format", "json-ast", "--code", JSON.stringify(test.code)]);
    }
  }
  const workflowRevision = JSON.parse(workflowRun("query", "config", "--format", "json")).revision;
  const workflowTransaction = ["edit", "transaction", "--code", JSON.stringify(workflowOperations),
    "--expect-revision", workflowRevision, "--format", "json"];
  workflowRun(...workflowTransaction, "--dry-run");
  workflowRun(...workflowTransaction);
  const workflowOriginal = await readFile(workflowSnapshot);
  workflowRun("fix", "--workflow", "strict", "--verify", "--format", "edn");
  workflowRun("--check-only");
  workflowRun("test", "--tag", "optional-proof", "--require-match");
  assert.deepEqual(await readFile(workflowSnapshot), workflowOriginal);
  run("edit", "def", "test-struct.main/OptionalRefField", "--input-format", "cirru", "--code",
    "quote $ defstruct OptionalRefField (:cell $ :: 'Ref $ :: 'Optional 'Number)");
  run("edit", "schema", "test-struct.main/OptionalRefField", "--input-format", "cirru", "--code", "quote 'StructDef");
  for (const [label, argumentType, body, returnType, rule] of [
    ["wrong-payload", "'String", "struct-with (OptionalFields :count nil :label nil) (:count raw)", "'test-struct.main/OptionalFields", "nominal-write-proof-v1"],
    ["open-payload", "'Dynamic", "struct-with (OptionalFields :count nil :label nil) (:count raw)", "'test-struct.main/OptionalFields", "nominal-write-proof-v1"],
    ["host-wrapper-conversion", ":: 'JsNullish 'Number", "struct-with (OptionalFields :count nil :label nil) (:count raw)", "'test-struct.main/OptionalFields", "nominal-write-proof-v1"],
    ["optional-elimination", "'test-struct.main/OptionalFields", ":count raw", "'Number", "concrete-return-proof-v1"],
    ["mutable-ref-widening", ":: 'Ref 'Number", "OptionalRefField :cell raw", "'test-struct.main/OptionalRefField", "nominal-write-proof-v1"],
    ["generic-payload-mismatch", "'String", "set-optional-generic (OptionalGeneric :value 1) raw", "'Dynamic", "concrete-return-proof-v1"],
  ]) {
    run("edit", "def", "test-struct.main/optional-rejected", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn optional-rejected (raw) $ ${body}`);
    run("edit", "schema", "test-struct.main/optional-rejected", "--input-format", "cirru", "--code",
      `quote $ :: 'Fn $ {} (:args $ [] ${argumentType.startsWith("::") ? `(${argumentType})` : argumentType}) (:return ${returnType})`);
    const before = await readFile(snapshot);
    const rejected = spawnSync(binary, [snapshot, "fix", "--rule", rule, "--ns", "test-struct.main",
      "--def", "optional-rejected", "--format", "edn"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${label}\n${rejected.stdout}\n${rejected.stderr}`);
    assert.match(`${rejected.stdout}\n${rejected.stderr}`,
      /E_CALL_ARGUMENT_UNPROVEN|E_CALL_ARGUMENT_MISMATCH|E_FN_RETURN_UNPROVEN|W_FN_ARG_TYPE_MISMATCH|W_FN_RETURN_TYPE_MISMATCH/);
    assert.deepEqual(await readFile(snapshot), before);
  }
  run("edit", "rm-def", "test-struct.main/optional-rejected");
  // Contextual nominal literals must pass the ordinary constructor checks
  // after lowering, before the nominal wrapper hides its field payloads.
  run("test", "--tag", "nominal-contextual", "--require-match");
  const contextualNominalTests = ["read-context-box", "read-number-box"].flatMap(name =>
    JSON.parse(run("query", "def", `test-struct.main/${name}`, "--format", "json")).data.tests
      .filter(test => test.tags.includes("nominal-contextual")));
  assert.equal(contextualNominalTests.length, 2);
  run("edit", "def", "test-struct.main/nominal-replay", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "nominal-replay", [], ...contextualNominalTests.map(test => test.code), "&unit"]));
  run("edit", "schema", "test-struct.main/nominal-replay", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)");
  const contextualNominalEntry = ["--init-fn", "test-struct.main/nominal-replay", "--reload-fn", "test-struct.main/nominal-replay"];
  run(...contextualNominalEntry);
  const contextualNominalOutput = join(project, "nominal-js");
  run(...contextualNominalEntry, "--emit-path", contextualNominalOutput, "js");
  (await import(pathToFileURL(join(contextualNominalOutput, "test-struct.main.mjs")).href)).nominal_replay();
  for (const [label, body] of [
    ["generic-wrong-field", "read-context-box $ {} (:value 1) (:count 160)"],
    ["concrete-wrong-field", "read-number-box $ {} (:value 1) (:count 160)"],
    ["wrong-option-payload", "read-context-box $ {} (:value 1) (:count $ Option :some |wrong)"],
    ["missing-required-field", "read-context-box $ {} (:value 1)"],
    ["unknown-field", "read-context-box $ {} (:value 1) (:count $ Option :none) (:extra 1)"],
    ["concrete-generic-mismatch", "read-number-box $ {} (:value |wrong) (:count $ Option :none)"],
    ["loose-wrong-field", "read-context-box $ ?{} :value 1 :count 160"],
  ]) {
    const target = "test-struct.main/nominal-rejected";
    run("edit", "def", target, "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn nominal-rejected () $ ${body}`);
    run("edit", "schema", target, "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args ([])) (:return 'Number)");
    const original = await readFile(snapshot);
    const diagnostic = /W_FN_ARG_TYPE_MISMATCH|E_CALL_ARGUMENT_MISMATCH|E_CALL_ARGUMENT_UNPROVEN|map-to-struct rewrite skipped/;
    for (const mode of [["--check-only"], [], ["js"]]) {
      const output = join(project, `nominal-${label}-${mode[0] ?? "native"}`);
      const rejected = spawnSync(binary, [snapshot, "--init-fn", target, "--reload-fn", target,
        "--emit-path", output, ...mode], options);
      if (rejected.error) throw rejected.error;
      assert.equal(rejected.status, 1, `${label}\n${rejected.stdout}\n${rejected.stderr}`);
      assert.match(`${rejected.stdout}\n${rejected.stderr}`, diagnostic);
      assert.deepEqual(await readFile(snapshot), original);
      if (mode[0] === "js") {
        await assertRejectedArtifacts(output, label, diagnostic, true);
      }
    }
  }
  run("edit", "rm-def", "test-struct.main/nominal-rejected");
  // Validate each stored member before homogeneous synthesis erases a mixed
  // literal. The same source tests must survive native and JS compilation.
  run("test", "--tag", "collection-proof", "--require-match");
  const collectionOwners = ["NullableNumberStore", "NullableEventStore", "NullableLiteralStore",
    "nullable-literal-return", "nullish-literal-list", "checked-literal-alias"];
  const collectionTests = collectionOwners.flatMap(name =>
    JSON.parse(run("query", "def", `test-struct.main/${name}`, "--format", "json")).data.tests
      .filter(test => test.tags.includes("collection-proof")));
  assert.equal(collectionTests.length, 7);
  for (const [name, rule] of [["nullable-literal-return", "concrete-return-proof-v1"],
    ["nullish-literal-list", "concrete-return-proof-v1"], ["checked-literal-alias", "assert-type-proof-v1"]]) {
    const before = await readFile(snapshot);
    run("fix", "--rule", rule, "--ns", "test-struct.main", "--def", name, "--format", "edn");
    assert.deepEqual(await readFile(snapshot), before);
  }
  run("edit", "def", "test-struct.main/collection-replay", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "collection-replay", [], ...collectionTests.map(test => test.code), "&unit"]));
  run("edit", "schema", "test-struct.main/collection-replay", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)");
  const collectionEntry = ["--init-fn", "test-struct.main/collection-replay", "--reload-fn", "test-struct.main/collection-replay"];
  run(...collectionEntry);
  const collectionOutput = join(project, "collection-js");
  run(...collectionEntry, "--emit-path", collectionOutput, "js");
  (await import(pathToFileURL(join(collectionOutput, "test-struct.main.mjs")).href)).collection_replay();
  for (const [label, parameters, argumentTypes, body] of [
    ["mixed-wrong-payload", [], "[]", "NullableNumberStore :values ({} (:live |wrong) (:absent nil)) :nested ([])"],
    ["mixed-wrong-key", [], "[]", "NullableNumberStore :values ({} (|live 7) (:absent nil)) :nested ([])"],
    ["nested-wrong-payload", [], "[]", "NullableLiteralStore :items ([]) :unique (#{}) :nested ({} (:group ([] ({} (:live |wrong) (:absent nil)))))"],
    ["list-wrong-payload", [], "[]", "NullableLiteralStore :items ([] 7 |wrong nil) :unique (#{}) :nested ({})"],
    ["set-wrong-payload", [], "[]", "NullableLiteralStore :items ([]) :unique (#{} 7 |wrong nil) :nested ({})"],
    ["mixed-wrong-callback-input", [], "[]", "NullableEventStore :handlers ({} (:click (fn (value) (hint-fn $ {} (:args ([] String)) (:return Unit)) &unit)) (:focus nil))"],
    ["mixed-wrong-callback-return", [], "[]", "NullableEventStore :handlers ({} (:click (fn (value) (hint-fn $ {} (:args ([] Number)) (:return Number)) 1)) (:focus nil))"],
    ["mixed-wrong-callback-arity", [], "[]", "NullableEventStore :handlers ({} (:click (fn (left right) (hint-fn $ {} (:args ([] Number Number)) (:return Unit)) &unit)) (:focus nil))"],
    ["mixed-open-callback", ["callback"], "[] 'Fn", "NullableEventStore :handlers ({} (:click callback) (:focus nil))"],
    ["mixed-open-member", ["raw"], "[] 'Dynamic", "NullableNumberStore :values ({} (:live raw) (:absent nil)) :nested ([])"],
    ["mixed-optional-member", [], "[]", "NullableNumberStore :values ({} (:live (&parse-float |1)) (:absent nil)) :nested ([])"],
    ["open-container", ["values"], "[] $ :: 'Map 'Tag 'Dynamic", "NullableNumberStore :values values :nested ([])"],
    ["alias-contract-confusion", [], "[]", "NullableNumberStore :values (let ((values ({} (:live 7) (:absent nil)))) ({} (:nested values) (:absent nil))) :nested ([])"],
    ["alias-shadowed-member", ["raw"], "[] 'Dynamic", "NullableNumberStore :values (let ((values ({} (:live 7) (:absent nil)))) (let ((values raw)) ({} (:live values) (:absent nil)))) :nested ([])"],
  ]) {
    const target = "test-struct.main/collection-rejected";
    run("edit", "def", target, "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "collection-rejected", parameters, JSON.parse(run("cirru", "parse", "-e", body))]));
    run("edit", "schema", target, "--input-format", "cirru", "--code",
      `quote $ :: 'Fn $ {} (:args $ ${argumentTypes}) (:return 'Dynamic)`);
    const before = await readFile(snapshot);
    for (const mode of [["--check-only"], ["js"]]) {
      const output = join(project, `collection-${label}-${mode[0] === "js" ? "js" : "native"}`);
      const rejected = spawnSync(binary, [snapshot, "--init-fn", target, "--reload-fn", target,
        "--emit-path", output, ...mode], options);
      if (rejected.error) throw rejected.error;
      assert.equal(rejected.status, 1, `${label}\n${rejected.stdout}\n${rejected.stderr}`);
      assert.match(`${rejected.stdout}\n${rejected.stderr}`, /W_FN_ARG_TYPE_MISMATCH/);
      assert.deepEqual(await readFile(snapshot), before);
      if (mode[0] === "js") await assertRejectedArtifacts(output, label, /W_FN_ARG_TYPE_MISMATCH/, true);
    }
  }
  for (const [label, parameters, argumentTypes, member] of [
    ["asserted-wrong-member", [], "[]", "|wrong"],
    ["asserted-open-member", ["raw"], "[] 'Dynamic", "raw"],
  ]) {
    const target = "test-struct.main/collection-rejected";
    run("edit", "def", target, "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn collection-rejected (${parameters.join(" ")}) $ assert-type ({} (:live ${member}) (:absent nil)) $ :: 'Map 'Tag $ :: 'JsNullish 'Number`);
    run("edit", "schema", target, "--input-format", "cirru", "--code",
      `quote $ :: 'Fn $ {} (:args $ ${argumentTypes}) (:return 'Dynamic)`);
    const before = await readFile(snapshot);
    const rejected = spawnSync(binary, [snapshot, "fix", "--rule", "assert-type-proof-v1", "--ns", "test-struct.main",
      "--def", "collection-rejected", "--format", "edn"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${label}\n${rejected.stdout}\n${rejected.stderr}`);
    assert.match(`${rejected.stdout}\n${rejected.stderr}`, /E_ASSERT_TYPE_UNPROVEN|E_ASSERT_TYPE_MISMATCH/);
    assert.deepEqual(await readFile(snapshot), before);
  }
  // Empty constructors prove only their own container family. Replay the
  // attached contracts, including the real consumer's typed Set accumulator.
  run("test", "--tag", "empty-container-proof", "--require-match");
  const emptyOwners = ["empty-list-proof", "empty-set-proof", "empty-map-proof", "collect-unique-proof"];
  const emptyTests = emptyOwners.flatMap(name =>
    JSON.parse(run("query", "def", `test-struct.main/${name}`, "--format", "json")).data.tests
      .filter(test => test.tags.includes("empty-container-proof")));
  assert.equal(emptyTests.length, 4);
  for (const name of emptyOwners) {
    const before = await readFile(snapshot);
    for (const rule of ["assert-type-proof-v1", "concrete-return-proof-v1", "callable-contract-proof-v1"]) {
      run("fix", "--rule", rule, "--ns", "test-struct.main", "--def", name, "--format", "edn");
      assert.deepEqual(await readFile(snapshot), before);
    }
  }
  run("edit", "def", "test-struct.main/empty-replay", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "empty-replay", [], ...emptyTests.map(test => test.code), "&unit"]));
  run("edit", "schema", "test-struct.main/empty-replay", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)");
  const emptyEntry = ["--init-fn", "test-struct.main/empty-replay", "--reload-fn", "test-struct.main/empty-replay"];
  run(...emptyEntry);
  const emptyOutput = join(project, "empty-js");
  run(...emptyEntry, "--emit-path", emptyOutput, "js");
  (await import(pathToFileURL(join(emptyOutput, "test-struct.main.mjs")).href)).empty_replay();
  // A Dynamic input may happen to be empty at runtime, but its schema is not
  // emptiness proof. Nonempty open members and shadowed aliases stay rejected.
  for (const [body, inputType, resultType] of [
    [", raw", ":: 'List 'Dynamic", ":: 'List 'Number"],
    [", raw", ":: 'Set 'Dynamic", ":: 'Set 'Number"],
    [", raw", ":: 'Map 'Tag 'Dynamic", ":: 'Map 'Tag 'String"],
    ["[] raw", "'Dynamic", ":: 'List 'Number"],
    ["#{} raw", "'Dynamic", ":: 'Set 'Number"],
    ["&{} :value raw", "'Dynamic", ":: 'Map 'Tag 'String"],
    ["let ((values (#{}))) (let ((values raw)) , values)", "'Dynamic", ":: 'Set 'String"],
  ]) {
    run("edit", "def", "test-struct.main/collection-rejected", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn collection-rejected (raw) $ ${body}`);
    run("edit", "schema", "test-struct.main/collection-rejected", "--input-format", "cirru", "--code",
      `quote $ :: 'Fn $ {} (:args ([] ${inputType.startsWith("::") ? `(${inputType})` : inputType})) (:return $ ${resultType})`);
    const before = await readFile(snapshot);
    const rejected = spawnSync(binary, [snapshot, "fix", "--rule", "concrete-return-proof-v1", "--ns", "test-struct.main",
      "--def", "collection-rejected", "--format", "edn"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${body}\n${rejected.stdout}\n${rejected.stderr}`);
    assert.match(`${rejected.stdout}\n${rejected.stderr}`, /E_FN_RETURN_UNPROVEN/);
    assert.deepEqual(await readFile(snapshot), before);
  }
  for (const body of ["assert-type (#{}) $ :: 'List 'String", "assert-type ([]) $ :: 'Set 'String",
    "assert-type (&{}) 'Number", "assert-type (#{} |wrong) $ :: 'Set 'Number"]) {
    run("edit", "def", "test-struct.main/collection-rejected", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn collection-rejected () $ ${body}`);
    run("edit", "schema", "test-struct.main/collection-rejected", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args ([])) (:return 'Dynamic)");
    const before = await readFile(snapshot);
    const rejected = spawnSync(binary, [snapshot, "fix", "--rule", "assert-type-proof-v1", "--ns", "test-struct.main",
      "--def", "collection-rejected", "--format", "edn"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${body}\n${rejected.stdout}\n${rejected.stderr}`);
    assert.match(`${rejected.stdout}\n${rejected.stderr}`, /E_ASSERT_TYPE_MISMATCH/);
    assert.deepEqual(await readFile(snapshot), before);
  }
  // Adopting a Promise as the whole async return does not await a stored member.
  const pendingTarget = "test-struct.main/collection-rejected";
  run("edit", "def", pendingTarget, "--overwrite", "--input-format", "cirru", "--code",
    "quote $ defn collection-rejected () (hint-fn $ {} (:async true) (:args ([])) (:return $ :: List Number)) (let ((load (fn (x) (hint-fn $ {} (:async true) (:args ([] Number)) (:return Number)) x))) ([] (load 1)))");
  run("edit", "schema", pendingTarget, "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:async true) (:args ([])) (:return $ :: 'List 'Number)");
  const pendingOriginal = await readFile(snapshot);
  const pendingAudit = spawnSync(binary, [snapshot, "fix", "--rule", "concrete-return-proof-v1", "--ns", "test-struct.main",
    "--def", "collection-rejected", "--format", "edn"], options);
  if (pendingAudit.error) throw pendingAudit.error;
  assert.equal(pendingAudit.status, 1, `${pendingAudit.stdout}\n${pendingAudit.stderr}`);
  assert.match(`${pendingAudit.stdout}\n${pendingAudit.stderr}`, /E_ASYNC_INVOCATION_REQUIRES_AWAIT/);
  assert.deepEqual(await readFile(snapshot), pendingOriginal);
  run("edit", "rm-def", pendingTarget);
  // Check the stored exits, not a nullable join masquerading as a conversion.
  const returnNames = ["nullable-choice", "nullable-reversed-choice", "nullable-match", "nullable-let-choice",
    "nullable-implicit-choice", "nullable-shadow-choice", "nullable-callback-choice", "nullable-raised-choice"];
  run("test", "--tag", "contextual-proof", "--require-match");
  const contextualTests = returnNames.flatMap(name =>
    JSON.parse(run("query", "def", `test-struct.main/${name}`, "--format", "json")).data.tests
      .filter(test => test.tags.includes("contextual-proof")));
  assert.equal(contextualTests.length, 8);
  for (const name of returnNames) {
    const before = await readFile(snapshot);
    run("fix", "--rule", "concrete-return-proof-v1", "--ns", "test-struct.main", "--def", name, "--format", "edn");
    assert.deepEqual(await readFile(snapshot), before);
  }
  run("edit", "def", "test-struct.main/contextual-replay", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "contextual-replay", [], ...contextualTests.map(test => test.code), "&unit"]));
  run("edit", "schema", "test-struct.main/contextual-replay", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)");
  const contextualEntry = ["--init-fn", "test-struct.main/contextual-replay", "--reload-fn", "test-struct.main/contextual-replay"];
  run(...contextualEntry);
  const contextualOutput = join(project, "contextual-js");
  run(...contextualEntry, "--emit-path", contextualOutput, "js");
  (await import(pathToFileURL(join(contextualOutput, "test-struct.main.mjs")).href)).contextual_replay();
  const nullableNumber = ":: 'JsNullish 'Number";
  const nullableCallback = ":: 'JsNullish 'test-struct.main/NullableEvent";
  for (const [label, parameters, argumentTypes, body, resultType, definite] of [
    ["wrong-live-payload", [], "[]", "if flag |bad nil", nullableNumber, true],
    ["wrong-reversed-payload", [], "[]", "if flag nil |bad", nullableNumber, true],
    ["open-live-payload", ["raw"], "[] 'Dynamic", "if flag raw nil", nullableNumber, false],
    ["alias-shadow", ["raw"], "[] 'Dynamic", "let ((selected (if flag 7 nil))) (let ((selected raw)) selected)", nullableNumber, false],
    ["match-shadow", ["raw"], "[] $ :: 'Option 'Dynamic", "let ((selected (if flag 7 nil))) (match raw ((:some selected) selected) ((:none) nil))", nullableNumber, false],
    ["nullish-elimination", ["raw"], `[] $ ${nullableNumber}`, "if flag raw 7", "'Number", true],
    ["stored-optional", [], "[]", "&parse-float |1", nullableNumber, true],
    ["wrong-callback-input", [], "[]", "if flag (fn (value) (hint-fn $ {} (:args ([] 'String)) (:return 'Unit)) &unit) nil", nullableCallback, true],
    ["wrong-callback-return", [], "[]", "if flag (fn (value) (hint-fn $ {} (:args ([] 'Number)) (:return 'Number)) 1) nil", nullableCallback, true],
    ["wrong-callback-arity", [], "[]", "if flag (fn (left right) (hint-fn $ {} (:args ([] 'Number 'Number)) (:return 'Unit)) &unit) nil", nullableCallback, true],
    ["open-callback", ["raw"], "[] 'Fn", "if flag raw nil", nullableCallback, false],
    ["recursive-cycle", ["value"], "[] 'Number", "recur flag value", nullableNumber, false],
  ]) {
    const target = "test-struct.main/contextual-rejected";
    run("edit", "def", target, "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "contextual-rejected", ["flag", ...parameters], JSON.parse(run("cirru", "parse", "-e", body))]));
    run("edit", "schema", target, "--input-format", "json-ast", "--code",
      JSON.stringify(["::", "'Fn", ["{}", [":args", ["[]", "'Bool", ...JSON.parse(run("cirru", "parse", "-e", argumentTypes)).slice(1)]],
        [":return", resultType.startsWith("::") ? JSON.parse(run("cirru", "parse", "-e", resultType)) : resultType]]]));
    const original = await readFile(snapshot);
    const audit = spawnSync(binary, [snapshot, "fix", "--rule", "concrete-return-proof-v1", "--ns", "test-struct.main",
      "--def", "contextual-rejected", "--format", "edn"], options);
    if (audit.error) throw audit.error;
    assert.equal(audit.status, 1, `${label}\n${audit.stdout}\n${audit.stderr}`);
    assert.match(`${audit.stdout}\n${audit.stderr}`, /E_FN_RETURN_UNPROVEN|W_FN_RETURN_TYPE_MISMATCH/);
    assert.deepEqual(await readFile(snapshot), original);
    // Ordinary compilation retains its migration policy for open returns,
    // while definite contradictions must fail on both existing backends.
    if (definite) {
      for (const mode of [["--check-only"], ["js"]]) {
        const output = join(project, `contextual-${label}-${mode[0] === "js" ? "js" : "native"}`);
        const rejected = spawnSync(binary, [snapshot, "--init-fn", target, "--reload-fn", target,
          "--emit-path", output, ...mode], options);
        if (rejected.error) throw rejected.error;
        assert.equal(rejected.status, 1, `${label}\n${rejected.stdout}\n${rejected.stderr}`);
        assert.match(`${rejected.stdout}\n${rejected.stderr}`, /W_FN_RETURN_TYPE_MISMATCH/);
        assert.deepEqual(await readFile(snapshot), original);
        if (mode[0] === "js") await assertRejectedArtifacts(output, label, /W_FN_RETURN_TYPE_MISMATCH/, true);
      }
    }
  }
  run("edit", "rm-def", "test-struct.main/contextual-rejected");
  run("test", "--tag", "js-nullish-container", "--require-match");
  const nullishTests = ["NullableNumberStore", "NullableEventStore"].flatMap(name =>
    JSON.parse(run("query", "def", `test-struct.main/${name}`, "--format", "json")).data.tests
      .filter(test => test.tags.includes("js-nullish-container")));
  assert.equal(nullishTests.length, 5);
  run("edit", "def", "test-struct.main/nullish-replay", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "nullish-replay", [], ...nullishTests.map(test => test.code), "&unit"]));
  run("edit", "schema", "test-struct.main/nullish-replay", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)");
  const nullishEntry = ["--init-fn", "test-struct.main/nullish-replay", "--reload-fn", "test-struct.main/nullish-replay"];
  run(...nullishEntry);
  const nullishOutput = join(project, "nullish-js");
  run(...nullishEntry, "--emit-path", nullishOutput, "js");
  (await import(pathToFileURL(join(nullishOutput, "test-struct.main.mjs")).href)).nullish_replay();
  run("edit", "def", "test-struct.main/ConcreteNumberStore", "--input-format", "cirru", "--code",
    "quote $ defstruct ConcreteNumberStore $ :values $ :: Map Tag Number");
  run("edit", "schema", "test-struct.main/ConcreteNumberStore", "--input-format", "cirru", "--code", "quote 'StructDef");
  run("edit", "def", "test-struct.main/NullableRefStore", "--input-format", "cirru", "--code",
    "quote $ defstruct NullableRefStore $ :cell $ :: Ref $ :: JsNullish Number");
  run("edit", "schema", "test-struct.main/NullableRefStore", "--input-format", "cirru", "--code", "quote 'StructDef");
  run("edit", "def", "test-struct.main/NullableInputCallbackStore", "--input-format", "cirru", "--code",
    "quote $ defstruct NullableInputCallbackStore $ :handlers $ :: Map Tag $ :: JsNullish $ :: Fn $ {} (:args $ [] $ :: JsNullish Number) (:return Unit)");
  run("edit", "schema", "test-struct.main/NullableInputCallbackStore", "--input-format", "cirru", "--code", "quote 'StructDef");
  for (const [label, args, body, schema] of [
    ["wrong-payload", [], "NullableNumberStore :values ({} (:a |wrong)) :nested ([])", ":: 'Fn $ {} (:args ([])) (:return 'Dynamic)"],
    ["open-members", ["values"], "NullableNumberStore :values values :nested ([])", ":: 'Fn $ {} (:args $ [] $ :: 'Map 'Tag 'Dynamic) (:return 'Dynamic)"],
    ["wrong-key", [], "NullableNumberStore :values ({} (|a 1)) :nested ([])", ":: 'Fn $ {} (:args ([])) (:return 'Dynamic)"],
    ["wrong-callback-input", [], "NullableEventStore :handlers $ {} $ :click $ fn (value) (hint-fn $ {} (:args $ [] 'String) (:return 'Unit)) &unit", ":: 'Fn $ {} (:args ([])) (:return 'Dynamic)"],
    ["wrong-callback-return", [], "NullableEventStore :handlers $ {} $ :click $ fn (value) (hint-fn $ {} (:args $ [] 'Number) (:return 'Number)) 1", ":: 'Fn $ {} (:args ([])) (:return 'Dynamic)"],
    ["wrong-callback-arity", [], "NullableEventStore :handlers $ {} $ :click $ fn (left right) (hint-fn $ {} (:args $ [] 'Number 'Number) (:return 'Unit)) &unit", ":: 'Fn $ {} (:args ([])) (:return 'Dynamic)"],
    ["narrower-callback-input", [], "NullableInputCallbackStore :handlers $ {} $ :click NullableEvent", ":: 'Fn $ {} (:args ([])) (:return 'Dynamic)"],
    ["erased-callback", ["callback"], "NullableEventStore :handlers $ {} (:click callback)", ":: 'Fn $ {} (:args $ [] 'Fn) (:return 'Dynamic)"],
    ["nullable-elimination", ["values"], "ConcreteNumberStore :values values", ":: 'Fn $ {} (:args $ [] $ :: 'Map 'Tag $ :: 'JsNullish 'Number) (:return 'Dynamic)"],
    ["mutable-widening", ["cell"], "NullableRefStore :cell cell", ":: 'Fn $ {} (:args $ [] $ :: 'Ref 'Number) (:return 'Dynamic)"],
  ]) {
    run("edit", "def", "test-struct.main/nullish-rejected", "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "nullish-rejected", args, JSON.parse(run("cirru", "parse", "-e", body))]));
    run("edit", "schema", "test-struct.main/nullish-rejected", "--input-format", "cirru", "--code", `quote $ ${schema}`);
    const original = await readFile(snapshot);
    for (const mode of [["--check-only"], ["js"]]) {
      const output = join(project, `nullish-${label}-${mode[0] === "js" ? "js" : "native"}`);
      const rejected = spawnSync(binary, [snapshot, "--init-fn", "test-struct.main/nullish-rejected",
        "--reload-fn", "test-struct.main/nullish-rejected", "--emit-path", output, ...mode], options);
      if (rejected.error) throw rejected.error;
      assert.equal(rejected.status, 1, `${label}\n${rejected.stdout}\n${rejected.stderr}`);
      assert.match(`${rejected.stdout}\n${rejected.stderr}`, /W_FN_ARG_TYPE_MISMATCH/);
      assert.deepEqual(await readFile(snapshot), original);
      if (mode[0] === "js") await assertRejectedArtifacts(output, label, /W_FN_ARG_TYPE_MISMATCH/, true);
    }
  }
  await copyFile("src/cirru/calcit-core.cirru", snapshot);
  // Replay the attached open-value view contracts without inventing deep proof.
  run("test", "calcit.core/data-view", "--tag", "data-view", "--require-match");
  const dataView = JSON.parse(run("query", "def", "calcit.core/data-view", "--format", "json"));
  assert.deepEqual(dataView.diagnostics, []);
  const dataViewTests = dataView.data.tests.filter(test => test.tags.includes("data-view"));
  assert.deepEqual(dataViewTests.map(test => test.name).sort(), [
    "classifies-symbol-struct-ref", "classifies-values", "exhaustive-view", "preserves-shallow-payloads",
  ]);
  run("edit", "add-ns", "calcit.data-view-replay");
  run("edit", "def", "calcit.data-view-replay/run!", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "run!", [], ...dataViewTests.map(test => test.code), "&unit"]));
  run("edit", "schema", "calcit.data-view-replay/run!", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)");
  const dataViewEntry = ["--init-fn", "calcit.data-view-replay/run!", "--reload-fn", "calcit.data-view-replay/run!"];
  run(...dataViewEntry);
  const dataViewOutput = join(project, "data-view-js");
  run(...dataViewEntry, "--emit-path", dataViewOutput, "js");
  const dataViewModule = await import(pathToFileURL(join(dataViewOutput, "calcit.data-view-replay.mjs")).href);
  dataViewModule.run_$x_();
  run("edit", "def", "calcit.data-view-replay/number-only", "--input-format", "cirru", "--code",
    "quote $ defn number-only (x) (&+ x 1)");
  run("edit", "schema", "calcit.data-view-replay/number-only", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([] 'Number)) (:return 'Number)");
  run("edit", "def", "calcit.data-view-replay/RequiredNumber", "--input-format", "cirru", "--code",
    "quote $ defstruct RequiredNumber (:value 'Number)");
  run("edit", "schema", "calcit.data-view-replay/RequiredNumber", "--input-format", "cirru", "--code", "quote 'StructDef");
  for (const [label, body, diagnostic] of [
    ["wrong-scalar", "match (data-view |text) ((:string value) (number-only value)) (_ 0)", /W_FN_ARG_TYPE_MISMATCH/],
    ["open-list-element", "match (data-view ([] 1)) ((:list items) (do (RequiredNumber :value (&list:nth items 0)) 0)) (_ 0)", /W_FN_ARG_TYPE_MISMATCH/],
    ["non-exhaustive", "match (data-view 1) ((:number value) value)", /non-exhaustive|not exhaustive|W_MATCH/],
  ]) {
    run("edit", "def", "calcit.data-view-replay/rejected", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn rejected () (${body})`);
    run("edit", "schema", "calcit.data-view-replay/rejected", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args ([])) (:return 'Number)");
    const before = await readFile(snapshot);
    for (const mode of [["--check-only"], ["js"]]) {
      const output = join(project, `data-view-${label}-${mode[0] === "js" ? "js" : "native"}`);
      const result = spawnSync(binary, [snapshot, "--init-fn", "calcit.data-view-replay/rejected",
        "--reload-fn", "calcit.data-view-replay/rejected", "--emit-path", output, ...mode], options);
      if (result.error) throw result.error;
      assert.equal(result.status, 1, `${label}: ${result.stdout}\n${result.stderr}`);
      assert.match(`${result.stdout}\n${result.stderr}`, diagnostic);
      assert.deepEqual(await readFile(snapshot), before);
      if (mode[0] === "js") await assertRejectedArtifacts(output, label, diagnostic, true);
    }
  }
  await copyFile("src/cirru/calcit-core.cirru", snapshot);
  const coreOriginal = await readFile(snapshot);
  run("fix", "--rule", "concrete-return-proof-v1", "--ns", "calcit.core", "--def", "every?", "--format", "edn");
  run("fix", "--rule", "concrete-return-proof-v1", "--ns", "calcit.core", "--def", "foldl-compare", "--format", "edn");
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
  assert.equal(callTests.length, 11);
  run("test", "calcit.core/hint-fn", "--tag", "generic-call-proof", "--require-match");
  const genericTests = returnResponse.data.tests.filter(test => test.tags.includes("generic-call-proof"));
  assert.equal(genericTests.length, 1);
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
  run("test", "calcit.core/foldl-compare", "--tag", "tail-return-proof", "--require-match");
  const tailResponse = JSON.parse(run("query", "def", "calcit.core/foldl-compare", "--format", "json"));
  const tailTests = tailResponse.data.tests.filter(test => test.tags.includes("tail-return-proof"));
  assert.equal(tailTests.length, 3);
  run("test", "calcit.core/recur", "--require-match");
  const recurResponse = JSON.parse(run("query", "def", "calcit.core/recur", "--format", "json"));
  const recurTests = recurResponse.data.tests;
  assert.deepEqual(recurTests.map(test => test.name), [
    "recurs-from-try-and-match-tail-positions",
    "recurs-from-match-tail-position",
  ]);
  assert.deepEqual(recurTests[1].code, recurTests[0].code[2]);
  run("test", "calcit.core/&str-spaced", "--tag", "tail-return-proof", "--require-match");
  const restResponse = JSON.parse(run("query", "def", "calcit.core/&str-spaced", "--format", "json"));
  const restTests = restResponse.data.tests.filter(test => test.tags.includes("tail-return-proof"));
  assert.equal(restTests.length, 1);
  run("test", "calcit.core/str-spaced", "--tag", "tail-return-proof", "--require-match");
  const formattingResponse = JSON.parse(run("query", "def", "calcit.core/str-spaced", "--format", "json"));
  const formattingTests = formattingResponse.data.tests.filter(test => test.tags.includes("tail-return-proof"));
  assert.equal(formattingTests.length, 1);
  run("test", "calcit.core/&list:map", "--tag", "generic-fold-proof", "--require-match");
  const mappingResponse = JSON.parse(run("query", "def", "calcit.core/&list:map", "--format", "json"));
  const mappingTests = mappingResponse.data.tests.filter(test => test.tags.includes("generic-fold-proof"));
  assert.equal(mappingTests.length, 1);
  run("test", "calcit.core/&enum:definition", "--tag", "nominal-definition-proof", "--require-match");
  const definitionResponse = JSON.parse(run("query", "def", "calcit.core/&enum:definition", "--format", "json"));
  const definitionTests = definitionResponse.data.tests.filter(test => test.tags.includes("nominal-definition-proof"));
  assert.equal(definitionTests.length, 1);
  run("test", "calcit.core/assert=", "--tag", "tail-return-proof", "--require-match");
  const assertionResponse = JSON.parse(run("query", "def", "calcit.core/assert=", "--format", "json"));
  const assertionTests = assertionResponse.data.tests.filter(test => test.tags.includes("tail-return-proof"));
  assert.equal(assertionTests.length, 1);
  run("test", "calcit.core/Result", "--tag", "nominal-branch-proof", "--require-match");
  const resultResponse = JSON.parse(run("query", "def", "calcit.core/Result", "--format", "json"));
  const resultTests = resultResponse.data.tests.filter(test => test.tags.includes("nominal-branch-proof"));
  assert.equal(resultTests.length, 1);
  run("test", "--tag", "checked-exit-proof", "--require-match");
  const exitResponse = JSON.parse(run("query", "def", "calcit.core/hint-fn", "--format", "json"));
  const exitTests = exitResponse.data.tests.filter(test => test.tags.includes("checked-exit-proof"));
  const getResponse = JSON.parse(run("query", "def", "calcit.core/get", "--format", "json"));
  const getTests = getResponse.data.tests.filter(test => test.tags.includes("checked-exit-proof"));
  assert.equal(exitTests.length, 1);
  assert.equal(getTests.length, 1);
  run("test", "calcit.core/&list:apply", "--require-match");
  const applyResponse = JSON.parse(run("query", "def", "calcit.core/&list:apply", "--format", "json"));
  const applyTests = applyResponse.data.tests.filter(test => test.name === "preserves-homogeneous-function-result-types");
  assert.equal(applyTests.length, 1);
  setBody([...tailTests, ...recurTests, ...restTests, ...genericTests, ...formattingTests, ...mappingTests, ...definitionTests, ...assertionTests, ...resultTests, ...exitTests, ...getTests, ...applyTests].map(test => test.code));
  run("edit", "schema", "calcit.assert-evidence/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)");
  run("config", "set", "init-fn", "calcit.assert-evidence/run-tests");
  run("config", "set", "reload-fn", "calcit.assert-evidence/run-tests");
  run();
  const tailOutput = join(project, "tail-exits-js");
  run("--emit-path", tailOutput, "js");
  const tailGenerated = await import(pathToFileURL(join(tailOutput, "calcit.assert-evidence.mjs")).href);
  assert.equal(tailGenerated.run_tests(), 1);
  const scalarTailTests = tailTests.filter(test => test.name === "typed-number-exits");
  assert.equal(scalarTailTests.length, 1);
  setBody(scalarTailTests.map(test => test.code));
  const closureTailOutput = join(project, "closure-tail-wasm");
  const closureTail = spawnSync(binary, [snapshot, "wasm", "--emit-path", closureTailOutput], options);
  if (closureTail.error) throw closureTail.error;
  assert.equal(closureTail.status, 1);
  assert.match(closureTail.stderr, /recur in a statically specialized closure is not yet supported/);
  await assert.rejects(readFile(join(closureTailOutput, "program.wasm")), { code: "ENOENT" });
  // Replay the same recurrence as a named function, the currently supported
  // WASM ownership boundary; local closure recur remains explicitly rejected.
  const stepCode = scalarTailTests[0].code[1][0][1];
  assert.equal(stepCode[0], "fn");
  assert.equal(stepCode[2][0], "hint-fn");
  run("edit", "def", "calcit.assert-evidence/tail-step", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "tail-step", ...stepCode.slice(1)]));
  run("edit", "schema", "calcit.assert-evidence/tail-step", "--input-format", "json-ast", "--code",
    JSON.stringify(["::", "'Fn", stepCode[2][1]]));
  setBody([["assert=", "0", ["tail-step", "4"]]]);
  const tailWasmOutput = join(project, "tail-exits-wasm");
  run("wasm", "--emit-path", tailWasmOutput);
  const tailModule = new WebAssembly.Module(await readFile(join(tailWasmOutput, "program.wasm")));
  const tailImports = {};
  for (const item of WebAssembly.Module.imports(tailModule)) {
    assert.equal(item.kind, "function");
    tailImports[item.module] ??= {};
    tailImports[item.module][item.name] = () => { throw new Error(`unexpected tail import ${item.module}.${item.name}`); };
  }
  assert.equal(new WebAssembly.Instance(tailModule, tailImports).exports["run-tests"](), 1);
  // A common input context cannot invent a common concrete output for the
  // members of a function list, including independently open callback bodies.
  for (const [name, callbacks] of [
    ["mixed-callable-outputs", [["fn", ["x"], ["str", "x"]], ["fn", ["x"], ["+", "x", "1"]]]],
    ["open-callable-output", [["fn", ["x"], ["parse-cirru-edn", "|1"]]]],
  ]) {
    setBody([["assert-type", [".apply", ["[]", "1", "2"], ["[]", ...callbacks]], ["::", "'List", "'String"]]]);
    const original = await readFile(snapshot);
    const rejected = spawnSync(binary, [snapshot, "fix", "--rule", "assert-type-proof-v1", "--ns", "calcit.assert-evidence", "--def", "run-tests", "--format", "edn"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${name}\n${rejected.stdout}\n${rejected.stderr}`);
    assert.match(`${rejected.stdout}\n${rejected.stderr}`, /E_ASSERT_TYPE_UNPROVEN|E_ASSERT_TYPE_MISMATCH|E_ERASED_GENERIC_RELATION|E_CALL_ARGUMENT_UNPROVEN/);
    assert.deepEqual(await readFile(snapshot), original);
  }
  setBody([["str-spaced"]]);
  const zeroArgumentOriginal = await readFile(snapshot);
  for (const mode of [[], ["--check-only"], ["js"], ["wasm"], ["wasi"]]) {
    const destination = join(project, `empty-format-${mode[0] ?? "native"}`);
    const rejected = spawnSync(binary, [snapshot, "--emit-path", destination, ...mode], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${rejected.stdout}\n${rejected.stderr}`);
    const arityDiagnostic = mode.includes("js")
      ? await readFile(join(destination, "calcit.build-errors.mjs"), "utf8")
      : rejected.stderr;
    assert.match(arityDiagnostic, /lack of args in str-spaced/);
    assert.deepEqual(await readFile(snapshot), zeroArgumentOriginal);
    await assert.rejects(readFile(join(destination, "program.wasm")), { code: "ENOENT" });
  }
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
  for (const [name, parameters, body, args, diagnostic] of [
    ["recursive-cycle", ["value"], ["recur", "value"], ["[]", "'Number"], /E_FN_RETURN_UNPROVEN/],
    ["reserved-raise-binding", ["raise"], ["raise", "|returns"], ["[]", ["::", "'Fn", ["{}", [":args", ["[]", "'String"]], [":return", "'String"]]]], /expected defn args to be symbols, got: \(&proc raise\)/],
  ]) {
    run("edit", "def", "calcit.assert-evidence/exit-proof", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "exit-proof", parameters, body]));
    run("edit", "schema", "calcit.assert-evidence/exit-proof", "--input-format", "json-ast", "--code",
      JSON.stringify(["::", "'Fn", ["{}", [":args", args], [":return", "'Number"]]]));
    const original = await readFile(snapshot);
    const rejected = spawnSync(binary, [snapshot, "fix", "--rule", "concrete-return-proof-v1", "--ns", "calcit.assert-evidence", "--def", "exit-proof", "--format", "edn"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${name}\n${rejected.stdout}\n${rejected.stderr}`);
    assert.match(`${rejected.stdout}\n${rejected.stderr}`, diagnostic);
    assert.deepEqual(await readFile(snapshot), original);
    run("edit", "rm-def", "calcit.assert-evidence/exit-proof");
  }
  // A constructor's absent slot is not an actual Dynamic payload or open value.
  for (const [name, inputType, left, right] of [
    ["used-open-payload", "'Dynamic", ["Result", ":ok", "raw"], ["Result", ":err", "|failed"]],
    ["open-result-instance", ["::", "'Result", "'Dynamic", "'String"], "raw", ["Result", ":err", "|failed"]],
    ["same-variant-open-payload", "'Dynamic", ["Result", ":ok", "1"], ["Result", ":ok", "raw"]],
  ]) {
    run("edit", "def", "calcit.assert-evidence/open-result", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "open-result", ["raw", "flag"], ["if", "flag", left, right]]));
    run("edit", "schema", "calcit.assert-evidence/open-result", "--input-format", "json-ast", "--code",
      JSON.stringify(["::", "'Fn", ["{}", [":args", ["[]", inputType, "'Bool"]], [":return", ["::", "'Result", "'Number", "'String"]]]]));
    const original = await readFile(snapshot);
    const rejected = spawnSync(binary, [snapshot, "fix", "--rule", "concrete-return-proof-v1", "--ns", "calcit.assert-evidence", "--def", "open-result", "--format", "edn"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${name}\n${rejected.stdout}\n${rejected.stderr}`);
    assert.match(`${rejected.stdout}\n${rejected.stderr}`, /E_FN_RETURN_UNPROVEN/);
    assert.deepEqual(await readFile(snapshot), original);
    run("edit", "rm-def", "calcit.assert-evidence/open-result");
  }
  // A kind predicate on the value typed by the generic return variable refines
  // that variable inside the guarded branch. Unguarded exits and predicates on other
  // values still owe the original generic contract (#1529).
  const genericReturnSchema = "quote $ :: 'Fn $ {} (:args $ [] 'T 'Bool) (:generics $ [] 'T) (:return 'T)";
  for (const [name, code, accepted, jsFfi] of [
    ["kind-refined", ["defn", "kind-refined", ["x", "flag"], ["if", ["list?", "x"], ["&list:rest", "x"], ["if", ["string?", "x"], ["&str:rest", "x"], ["raise", "|neither"]]]], true],
    ["kind-refined-binding", ["defn", "kind-refined-binding", ["x", "flag"], ["&let", ["open-x", "x"], ["if", ["list?", "x"], ["&list:rest", "x"], ["raise", "|not-list"]]]], true],
    ["kind-other-value", ["defn", "kind-other-value", ["x", "flag"], ["if", ["list?", "flag"], ["&list:rest", "x"], "|text"]], false],
    ["kind-rebound-local", ["defn", "kind-rebound-local", ["x", "flag"], ["if", ["list?", "x"], ["&let", ["unused", ["set!", "x", "|text"]], "x"], ["if", ["string?", "x"], ["&str:rest", "x"], ["raise", "|neither"]]]], false, true],
    ["kind-unguarded-join", ["defn", "kind-unguarded-join", ["x", "flag"], ["if", "flag", ["[]", "1"], "|text"]], false],
  ]) {
    run("edit", "def", `calcit.assert-evidence/${name}`, "--overwrite", "--input-format", "json-ast", "--code", JSON.stringify(code));
    run("edit", "schema", `calcit.assert-evidence/${name}`, "--input-format", "cirru", "--code", genericReturnSchema);
    if (jsFfi) run("edit", "schema", `calcit.assert-evidence/${name}`, "--add-feature", "js-ffi");
    const original = await readFile(snapshot);
    const checked = spawnSync(binary, [snapshot, "fix", "--rule", "concrete-return-proof-v1", "--ns", "calcit.assert-evidence", "--def", name, "--format", "edn"], options);
    if (checked.error) throw checked.error;
    if (accepted) {
      assert.equal(checked.status, 0, `${name}\n${checked.stdout}\n${checked.stderr}`);
    } else {
      assert.equal(checked.status, 1, `${name}\n${checked.stdout}\n${checked.stderr}`);
      assert.match(`${checked.stdout}\n${checked.stderr}`, /E_FN_RETURN_UNPROVEN/, name);
    }
    assert.deepEqual(await readFile(snapshot), original);
    run("edit", "rm-def", `calcit.assert-evidence/${name}`);
  }
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
      ["expected `'Result<list<:string>, :string>`", "got `'calcit.core/Result<list<:number>, :never>`"]],
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
  // Open values need their own proof before a concrete parameter or payload (#1767).
  const numberOnly = "(number-only (fn (value) (hint-fn ({} (:args ([] 'Number)) (:return 'Number))) (&+ value 1)))";
  const openCalls = [
    [`let (${numberOnly} (from-open (fn (items) (hint-fn ({} (:args ([] (:: 'List 'Dynamic))) (:return 'Number))) (number-only (&list:nth items 0))))) (from-open ([] |wrong))`,
      "@3.1.1.1.3.1"],
    [`let (${numberOnly} (from-open (fn (value) (hint-fn ({} (:args ([] 'Dynamic)) (:return 'Number))) (number-only value)))) (from-open |wrong)`,
      "@3.1.1.1.3.1"],
    ["let ((classify (fn (items) (hint-fn ({} (:args ([] (:: 'List 'Dynamic))) (:return 'Data))) (Data :number (&list:nth items 0))))) (classify ([] |wrong))",
      "@3.1.0.1.3.2"],
    [`let (${numberOnly} (from-open (fn (items) (hint-fn ({} (:args ([] (:: 'List 'Dynamic))) (:return 'Number))) (&let (v (&list:nth items 0)) (number-only v))))) (from-open ([] |wrong))`,
      "@3.1.1.1.3.2.1"],
  ];
  const openCallLocations = new Map(openCalls);
  const rejected = [
    ...bad.map(expression => [expression, "E_ASSERT_TYPE_MISMATCH"]),
    ...openCalls.map(([expression]) => [expression, "E_CALL_ARGUMENT_UNPROVEN"]),
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
      } else if (diagnostic === "E_CALL_ARGUMENT_UNPROVEN") {
        assert.ok(diagnostics.includes("has no proof for concrete parameter"), diagnostics);
        // Point at the open argument, not at arithmetic inside the callee.
        assert.ok(diagnostics.includes(`calcit.assert-evidence/run-tests ${openCallLocations.get(expression)}`), diagnostics);
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

  // Recursive nominal fields must be checked after nested constructors lower.
  // This is the shared Respo #194 boundary, not a consumer-specific rule (#1553).
  run("edit", "add-ns", "calcit.constructor-wrappers");
  run("edit", "def", "calcit.constructor-wrappers/ImportedShape", "--input-format", "cirru", "--code",
    "quote $ defstruct ImportedShape (:left 'Number) (:right 'Number)");
  run("edit", "schema", "calcit.constructor-wrappers/ImportedShape", "--input-format", "cirru", "--code", "quote 'StructDef");
  for (const [name, expression] of [["empty-tree", "Option :none"], ["open-tree", "Option :some 1"]]) {
    run("edit", "def", `calcit.constructor-wrappers/${name}`, "--input-format", "cirru", "--code",
      `quote $ defn ${name} () $ ${expression}`);
    run("edit", "schema", `calcit.constructor-wrappers/${name}`, "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ []) (:return $ :: 'Option 'Dynamic)");
  }
  run("edit", "add-import", "calcit.assert-evidence", "--input-format", "cirru", "--code",
    "quote $ calcit.constructor-wrappers :as wrappers");
  for (const [name, code, schema] of [
    ["RecursiveNode", "defenum RecursiveNode (:element 'calcit.assert-evidence/RecursiveElement) (:component 'calcit.assert-evidence/RecursiveComponent)", "'EnumDef"],
    ["RecursivePair", "defstruct RecursivePair (:key 'Dynamic) (:node 'calcit.assert-evidence/RecursiveNode)", "'StructDef"],
    ["RecursiveElement", "defstruct RecursiveElement (:children $ :: 'List 'calcit.assert-evidence/RecursivePair)", "'StructDef"],
    ["RecursiveComponent", "defstruct RecursiveComponent (:tree $ :: 'Option 'calcit.assert-evidence/RecursiveNode)", "'StructDef"],
    ["RecursiveSignal", "defenum RecursiveSignal ([] 'T) (:idle) (:data 'T)", "'EnumDef"],
    ["RecursiveSignalHolder", "defstruct RecursiveSignalHolder (:signal $ :: 'calcit.assert-evidence/RecursiveSignal 'calcit.assert-evidence/RecursiveNode)", "'StructDef"],
    ["make-open-signal", "defn make-open-signal () $ RecursiveSignal :data 1", ":: 'Fn $ {} (:args $ []) (:return $ :: 'calcit.assert-evidence/RecursiveSignal 'Dynamic)"],
    ["RecursiveCell", "defstruct RecursiveCell ([] 'T) (:value 'T)", "'StructDef"],
    ["BroadNodeHolder", "defstruct BroadNodeHolder (:cell $ :: 'calcit.assert-evidence/RecursiveCell 'Struct) (:tree $ :: 'Option 'Struct)", "'StructDef"],
    ["SpecificNodeHolder", "defstruct SpecificNodeHolder (:cell $ :: 'calcit.assert-evidence/RecursiveCell 'calcit.assert-evidence/RecursiveElement) (:tree $ :: 'Option 'calcit.assert-evidence/RecursiveElement)", "'StructDef"],
    ["make-broad-element", "defn make-broad-element () $ RecursiveElement :children $ []", ":: 'Fn $ {} (:args $ []) (:return 'Struct)"],
    ["make-empty-tree", "defn make-empty-tree () $ Option :none", ":: 'Fn $ {} (:args $ []) (:return $ :: 'Option 'Dynamic)"],
    ["make-open-tree", "defn make-open-tree () $ Option :some 1", ":: 'Fn $ {} (:args $ []) (:return $ :: 'Option 'Dynamic)"],
    ["make-open-number", "defn make-open-number () 1", ":: 'Fn $ {} (:args $ []) (:return 'Dynamic)"],
    ["CallbackHolder", "defstruct CallbackHolder (:handler $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number))", "'StructDef"],
    ["NullishCallbackHolder", "defstruct NullishCallbackHolder (:handler $ :: 'JsNullish $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number))", "'StructDef"],
    ["imported-component", "defn imported-component (flag) $ RecursiveComponent :tree $ if flag (wrappers/empty-tree) (Option :some (RecursiveNode :element (RecursiveElement :children ([]))))", ":: 'Fn $ {} (:args $ [] 'Bool) (:return 'calcit.assert-evidence/RecursiveComponent)"],
    ["joined-local-component", "defn joined-local-component (flag) $ let ((absent-tree (Option :none))) (RecursiveComponent :tree (if flag absent-tree (Option :some (RecursiveNode :element (RecursiveElement :children ([]))))))", ":: 'Fn $ {} (:args $ [] 'Bool) (:return 'calcit.assert-evidence/RecursiveComponent)"],
    ["folded-component", "defn folded-component (flag) $ RecursiveComponent :tree $ option:fold (if flag (Option :some true) (Option :none)) (fn () $ wrappers/empty-tree) (fn (present?) $ Option :some $ RecursiveNode :element $ RecursiveElement :children $ [])", ":: 'Fn $ {} (:args $ [] 'Bool) (:return 'calcit.assert-evidence/RecursiveComponent)"],
    ["folded-signal", "defn folded-signal (flag) $ RecursiveSignalHolder :signal $ option:fold (if flag (Option :some true) (Option :none)) (fn () $ RecursiveSignal :data $ RecursiveNode :element $ RecursiveElement :children $ []) (fn (present?) $ RecursiveSignal :idle)", ":: 'Fn $ {} (:args $ [] 'Bool) (:return 'calcit.assert-evidence/RecursiveSignalHolder)"],
    ["folded-open", "defn folded-open (flag) $ option:fold (if flag (Option :some true) (Option :none)) (fn () $ Option :none) (fn (present?) $ make-open-number)", ":: 'Fn $ {} (:args $ [] 'Bool) (:return 'Dynamic)"],
    ["loop-component", "defn loop-component (flag) $ loop ((remaining (if flag 1 0)) (tree (Option :none))) (if (&> remaining 0) (recur (dec remaining) (Option :some (RecursiveNode :element (RecursiveElement :children ([]))))) (RecursiveComponent :tree tree))", ":: 'Fn $ {} (:args $ [] 'Bool) (:return 'calcit.assert-evidence/RecursiveComponent)"],
    ["loop-signal", "defn loop-signal (flag) $ loop ((remaining (if flag 1 0)) (signal (RecursiveSignal :idle))) (if (&> remaining 0) (let ((next (RecursiveSignal :data (RecursiveNode :element (RecursiveElement :children ([])))))) (recur (dec remaining) next)) (RecursiveSignalHolder :signal signal))", ":: 'Fn $ {} (:args $ [] 'Bool) (:return 'calcit.assert-evidence/RecursiveSignalHolder)"],
    ["loop-alias", "defn loop-alias () $ let ((absent (Option :none))) (loop ((n 0) (value absent)) (if (&< n 1) (let ((next (Option :some 7)) (alias next)) (recur 1 alias)) (option:unwrap-or value 0)))", ":: 'Fn $ {} (:args $ []) (:return 'Number)"],
    ["loop-propagated", "defn loop-propagated () $ loop ((n 0) (left (Option :none)) (right (Option :none))) (if (&< n 2) (recur (inc n) (Option :some 7) left) (option:unwrap-or right 0))", ":: 'Fn $ {} (:args $ []) (:return 'Number)"],
    ["loop-matched", "defn loop-matched () $ loop ((value (Option :none))) (match value ((:none) (recur (Option :some 7))) ((:some payload) (+ payload 1)))", ":: 'Fn $ {} (:args $ []) (:return 'Number)"],
    ["loop-asserted", "defn loop-asserted () $ loop ((n 0) (value (Option :none))) (assert-type value (:: 'Option 'Number)) (if (&< n 1) (recur 1 (Option :some 7)) (value .unwrap-or 0))", ":: 'Fn $ {} (:args $ []) (:return 'Number)"],
    ["loop-once-log", "defmacro loop-once-log () (println |loop-expansion-token) (quasiquote 7)", ":: 'Macro $ {} (:required $ []) (:capabilities $ #{} :log) (:expansion $ :: 'Expr 'Number)"],
    ["require-string-expression", "defmacro require-string-expression (value) (quasiquote 42)", ":: 'Macro $ {} (:required $ [] $ :: 'Expr 'String) (:expansion $ :: 'Expr 'Number)"],
    ["require-string-option", "defmacro require-string-option (value) (quasiquote 42)", ":: 'Macro $ {} (:required $ [] $ :: 'Expr $ :: 'Option 'String) (:expansion $ :: 'Expr 'Number)"],
    ["string-option-recur", "defmacro string-option-recur (value replacement pending?) (if pending? (recur replacement replacement false) (quasiquote 42))", ":: 'Macro $ {} (:required $ [] (:: 'Expr (:: 'Option 'String)) 'Syntax 'Syntax) (:expansion $ :: 'Expr 'Number)"],
    ["string-option-result", "defmacro string-option-result (value) , value", ":: 'Macro $ {} (:required $ [] 'Syntax) (:expansion $ :: 'Expr $ :: 'Option 'String)"],
    ["string-option-head", "defmacro string-option-head (value) (quasiquote $ fn () 42)", ":: 'Macro $ {} (:required $ [] $ :: 'Expr $ :: 'Option 'String) (:expansion $ :: 'Expr $ :: 'Fn $ {} (:args $ []) (:return 'Number))"],
    ["string-option-type", "defmacro string-option-type (value) (quasiquote Option)", ":: 'Macro $ {} (:required $ [] $ :: 'Expr $ :: 'Option 'String) (:expansion $ :: 'Expr 'EnumDef)"],
    ["loop-generic-identity", "defmacro loop-generic-identity (value) , value", ":: 'Macro $ {} (:generics $ [] 'T) (:required $ [] $ :: 'Expr 'T) (:expansion $ :: 'Expr 'T)"],
    ["loop-occurrence-log", "defmacro loop-occurrence-log () (println |loop-occurrence-token) (quasiquote 7)", ":: 'Macro $ {} (:required $ []) (:capabilities $ #{} :log) (:expansion $ :: 'Expr 'Number)"],
    ["loop-assertion-source", "defmacro loop-assertion-source (value) (println |loop-assertion-token) , value", ":: 'Macro $ {} (:generics $ [] 'T) (:required $ [] $ :: 'Expr 'T) (:capabilities $ #{} :log) (:expansion $ :: 'Expr 'T)"],
    ["loop-repeat-source", "defmacro loop-repeat-source (form) (quasiquote $ &let () (~ form) (~ form))", ":: 'Macro $ {} (:required $ [] 'Syntax) (:expansion $ :: 'Expr 'Number)"],
    ["loop-duplicated-effects", "defn loop-duplicated-effects () $ loop ((n 0) (value (Option :none))) (if (&< n 1) (recur 1 (Option :some (loop-repeat-source (loop-occurrence-log)))) (value .unwrap-or 0))", ":: 'Fn $ {} (:args $ []) (:return 'Number)"],
    ["loop-captured-effects", "defn loop-captured-effects () $ loop ((n 0) (value (Option :none))) (let ((render (fn () (loop-occurrence-log)))) (if (&< n 1) (recur 1 (Option :some (render))) (value .unwrap-or 0)))", ":: 'Fn $ {} (:args $ []) (:return 'Number)"],
    ["loop-head-effects", "defn loop-head-effects () $ loop ((n 0) (value (Option :none))) (if (&< n 1) (recur 1 (Option :some ((fn () (loop-occurrence-log))))) (value .unwrap-or 0))", ":: 'Fn $ {} (:args $ []) (:return 'Number)"],
    ["first-method-direct", "defn first-method-direct () $ loop ((n 0) (value (Option :none))) (if (&< n 1) (recur 1 (Option :some 7)) ((value .unwrap) .to-string))", ":: 'Fn $ {} (:args $ []) (:return 'String)"],
    ["first-method-reversed", "defn first-method-reversed () $ loop ((n 0) (value (Option :none))) (if (&>= n 1) ((value .unwrap) .to-string) (recur 1 (Option :some 7)))", ":: 'Fn $ {} (:args $ []) (:return 'String)"],
    ["first-method-alias", "defn first-method-alias () $ loop ((n 0) (value (Option :none))) (let ((alias value)) (if (&< n 1) (recur 1 (Option :some 7)) ((alias .unwrap) .to-string)))", ":: 'Fn $ {} (:args $ []) (:return 'String)"],
    ["first-method-captured", "defn first-method-captured () $ loop ((n 0) (value (Option :none))) (let ((render (fn () ((value .unwrap) .to-string)))) (if (&< n 1) (recur 1 (Option :some 7)) (render)))", ":: 'Fn $ {} (:args $ []) (:return 'String)"],
    ["first-method-shadowed", "defn first-method-shadowed () $ loop ((n 0) (value (Option :none))) (let ((alias value) (value (Option :some |shadow))) (assert= |shadow ((value .unwrap) .to-string)) (if (&< n 1) (recur 1 (Option :some 7)) ((alias .unwrap) .to-string)))", ":: 'Fn $ {} (:args $ []) (:return 'String)"],
    ["first-method-effects", "defn first-method-effects () $ loop ((n 0) (value (Option :none))) (if (&< n 1) (recur 1 (Option :some (loop-occurrence-log))) ((value .unwrap) .to-string))", ":: 'Fn $ {} (:args $ []) (:return 'String)"],
    ["first-method-forwarded", "defn first-method-forwarded () $ loop ((n 0) (left (Option :none)) (right (Option :none))) (if (&< n 2) (recur (inc n) (Option :some 7) left) ((right .unwrap) .to-string))", ":: 'Fn $ {} (:args $ []) (:return 'String)"],
    ["first-method-staged", "defn first-method-staged () $ loop ((n 0) (left (Option :none)) (right (Option :none))) (if (&< n 1) (recur 1 (Option :some 7) right) (if (&< n 2) (recur 2 left (Option :some (left .unwrap))) ((right .unwrap) .to-string)))", ":: 'Fn $ {} (:args $ []) (:return 'String)"],
    ["first-method-match-join", "defn first-method-match-join () $ loop ((n 0) (value (Option :none))) (let ((next (match value ((:none) (Option :some 7)) ((:some payload) value)))) (if (&< n 1) (recur 1 next) ((value .unwrap) .to-string)))", ":: 'Fn $ {} (:args $ []) (:return 'String)"],
    ["first-method-if-join", "defn first-method-if-join () $ loop ((n 0) (value (Option :none))) (let ((next (if (&>= n 1) value (Option :some 7)))) (if (&< n 1) (recur 1 next) ((value .unwrap) .to-string)))", ":: 'Fn $ {} (:args $ []) (:return 'String)"],
    ["first-method-open-seven", "defn first-method-open-seven () 7", ":: 'Fn $ {} (:args $ []) (:return 'Dynamic)"],
    ["first-method-independent-boundary", "defn first-method-independent-boundary () $ loop ((n 0) (value (Option :none))) (if (&< n 1) (recur 1 (Option :some (assert-type (first-method-open-seven) 'Number))) ((value .unwrap) .to-string))", ":: 'Fn $ {} (:args $ []) (:return 'String)"],
    ["loop-expansion-once", "defn loop-expansion-once () $ loop ((n 0) (value (Option :none))) (if (&< n 1) (recur 1 (Option :some (loop-once-log))) (value .unwrap-or 0))", ":: 'Fn $ {} (:args $ []) (:return 'Number)"],
    ["recursive-count", `defn recursive-count (node)
  match node
    (:element element)
      inc $ foldl (:children element) 0 $ fn (total pair)
        + total $ recursive-count $ :node pair
    (:component component)
      match (:tree component)
        (:none) 0
        (:some child) $ recursive-count child`, ":: 'Fn $ {} (:args $ [] 'calcit.assert-evidence/RecursiveNode) (:return 'Number)"],
  ]) {
    run("edit", "def", `calcit.assert-evidence/${name}`, "--input-format", "cirru", "--code", `quote $ ${code}`);
    run("edit", "schema", `calcit.assert-evidence/${name}`, "--input-format", "cirru", "--code",
      `quote ${schema.startsWith("::") ? "$ " : ""}${schema}`);
  }
  run("edit", "add-test", "calcit.assert-evidence/recursive-count", "recursive-nominal-fields", "--tags", "recursive-fields",
    "--input-format", "cirru", "--code", `quote $ let
    leaf $ RecursiveElement :children $ []
    wrapped $ RecursiveNode :element leaf
    present $ %{} RecursiveComponent (:tree $ Option :some wrapped)
    absent $ RecursiveComponent :tree $ Option :none
    absent-tree $ Option :none
    absent-alias absent-tree
    local-absent $ RecursiveComponent :tree absent-alias
    folded-tree $ if true (Option :none) (Option :some wrapped)
    folded-absent $ RecursiveComponent :tree folded-tree
    imported-tree $ wrappers/empty-tree
    local-imported $ RecursiveComponent :tree imported-tree
    idle-signal $ RecursiveSignal :idle
    signal-holder $ RecursiveSignalHolder :signal idle-signal
    node-cell $ atom $ assert-type (Option :none) $ :: 'Option 'calcit.assert-evidence/RecursiveNode
    from-wrapper $ RecursiveComponent :tree $ make-empty-tree
    from-import $ RecursiveComponent :tree $ wrappers/empty-tree
    broad $ BroadNodeHolder :cell (RecursiveCell :value leaf) :tree $ Option :some leaf
    callback $ CallbackHolder :handler $ fn (value) $ inc value
    literal-callback $ %{} CallbackHolder $ :handler $ fn (value) $ inc value
    nullish-callback $ NullishCallbackHolder :handler nil
    parent $ %{} RecursiveElement $ :children
      []
        %{} RecursivePair (:key |string-key) (:node $ RecursiveNode :component present)
        RecursivePair :key :tag-key :node $ RecursiveNode :component absent
        RecursivePair :key 2 :node wrapped
  assert= 3 $ recursive-count $ RecursiveNode :element parent
  assert= 0 $ recursive-count $ RecursiveNode :component absent
  assert= 0 $ recursive-count $ RecursiveNode :component local-absent
  assert= 0 $ recursive-count $ RecursiveNode :component folded-absent
  assert= 0 $ recursive-count $ RecursiveNode :component local-imported
  reset! node-cell $ Option :some wrapped
  assert= 1 $ recursive-count $ RecursiveNode :component $ RecursiveComponent :tree $ deref node-cell
  assert= 7 $ match (:signal signal-holder) ((:idle) 7) ((:data node) (recursive-count node))
  assert= 0 $ recursive-count $ RecursiveNode :component from-wrapper
  assert= 0 $ recursive-count $ RecursiveNode :component from-import
  assert= 1 $ recursive-count $ RecursiveNode :component $ imported-component false
  assert= 0 $ recursive-count $ RecursiveNode :component $ imported-component true
  assert= 1 $ recursive-count $ RecursiveNode :component $ joined-local-component false
  assert= 0 $ recursive-count $ RecursiveNode :component $ joined-local-component true
  assert= 1 $ recursive-count $ RecursiveNode :component $ folded-component true
  assert= 0 $ recursive-count $ RecursiveNode :component $ folded-component false
  assert= 7 $ match (:signal $ folded-signal true) ((:idle) 7) ((:data node) (recursive-count node))
  assert= 1 $ match (:signal $ folded-signal false) ((:idle) 7) ((:data node) (recursive-count node))
  assert= 1 $ folded-open true
  assert= (Option :none) $ folded-open false
  assert= 1 $ recursive-count $ RecursiveNode :component $ loop-component true
  assert= 0 $ recursive-count $ RecursiveNode :component $ loop-component false
  assert= 1 $ match (:signal $ loop-signal true) ((:idle) 0) ((:data node) (recursive-count node))
  assert= 0 $ match (:signal $ loop-signal false) ((:idle) 0) ((:data node) (recursive-count node))
  assert= 7 $ loop-alias
  assert= 7 $ loop-propagated
  assert= 8 $ loop-matched
  assert= 7 $ loop-asserted
  assert= 42 $ let ((value |kept)) (require-string-expression value)
  assert= 42 $ let ((value |kept) (alias value)) (require-string-expression alias)
  assert= 42 $ let ((value |kept) (render (fn () (require-string-expression value)))) (render)
  assert= 42 $ loop
      n 0
      cell $ atom $ assert-type (Option :none) $ :: 'Option 'Dynamic
    hint-fn $ {} (:args $ [] 'Number $ :: 'Ref $ :: 'Option 'Dynamic) (:return 'Number)
    if (&< n 1)
      recur 1 $ atom $ assert-type (Option :some 7) $ :: 'Option 'Dynamic
      , 42
  assert= 1 $ loop
      n 0
      value $ Option :none
    if (&< n 1)
      let
          decoded $ decode-map-as ([] 7) $ :: 'List 'Number
        recur 1 $ Option :some $ count decoded
      value .unwrap-or 0
  assert= 7 $ loop-expansion-once
  assert= 7 $ loop
      n 0
      value $ Option :none
    if (&< n 1)
      recur 1 $ Option :some 7
      value .unwrap-or 0
  assert= true $ struct? $ :value $ :cell broad
  assert= true $ struct? $ (:tree broad).unwrap
  assert= 5 $ (:handler callback) 4
  assert= 5 $ (:handler literal-callback) 4
  assert= nil $ :handler nullish-callback
  assert= |string-key $ :key $ &list:nth (:children parent) 0
  assert= :tag-key $ :key $ &list:nth (:children parent) 1
  assert= 2 $ :key $ &list:nth (:children parent) 2`);
  // Empty core expressions are not executable JS expressions. Keep this
  // traversal regression attached to its definition but replay it natively.
  run("edit", "add-test", "calcit.assert-evidence/recursive-count", "empty-tail-recheck", "--tags", "recursive-empty-tail",
    "--input-format", "cirru", "--code", `quote $ do
  assert= 42 $ loop ((n 0) (value (Option :none))) (if (&< n 1) (if (&< n 1) (recur 1 (Option :some 7)) ()) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) (if (&< n 1) (if (&>= n 1) () (recur 1 (Option :some 7))) 42)`);
  run("test", "calcit.assert-evidence/recursive-count", "--tag", "recursive-empty-tail", "--require-match");
  run("edit", "add-test", "calcit.assert-evidence/recursive-count", "loop-source-contracts", "--tags", "recursive-fields",
    "--input-format", "cirru", "--code", `quote $ do
  assert= 42 $ loop ((n 0) (value (Option :none))) (require-string-option value) (if (&< n 1) (recur 1 (Option :some |kept)) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) (string-option-recur (Option :none) value true) (if (&< n 1) (recur 1 (Option :some |kept)) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) (let ((alias value)) (require-string-option alias)) (if (&< n 1) (recur 1 (Option :some |kept)) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) (let ((check (fn () (require-string-option value)))) (check)) (if (&< n 1) (recur 1 (Option :some |kept)) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) (let ((value (Option :some |kept))) (require-string-option value)) (if (&< n 1) (recur 1 (Option :some 7)) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) ((string-option-head value)) (if (&< n 1) (recur 1 (Option :some |kept)) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) ((string-option-type value) :some 7) (if (&< n 1) (recur 1 (Option :some |kept)) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) (string-option-result value) (if (&< n 1) (recur 1 (Option :some |kept)) 42)
  assert= 7 $ loop ((n 0) (value (Option :none))) (if (&< n 1) (recur 1 (Option :some 7)) ((loop-generic-identity value) .unwrap-or 0))
  assert= 7 $ loop-duplicated-effects
  assert= 7 $ loop-captured-effects
  assert= 7 $ loop-head-effects
  assert= 42 $ loop ((n 0) (value (Option :none))) (assert-type value (:: 'Option 'String)) (if (&< n 1) (recur 1 (Option :some |kept)) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) (let ((check (fn () (assert-type value (:: 'Option 'String)) 42))) (if (&< n 1) (recur 1 (Option :some |kept)) (check)))
  assert= 42 $ loop ((n 0) (value (Option :none))) (assert-type (if (&< n 1) value value) (:: 'Option 'String)) (if (&< n 1) (recur 1 (Option :some |kept)) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) (let ((value (Option :some |kept))) (assert-type value (:: 'Option 'String))) (if (&< n 1) (recur 1 (Option :some 7)) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) (assert-type (make-open-number) 'Number) (if (&< n 1) (recur 1 (Option :some |kept)) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) (assert-type (loop-assertion-source value) (:: 'Option 'String)) (if (&< n 1) (recur 1 (Option :some |kept)) 42)
  assert= 42 $ loop ((n 0) (value (Option :none))) (let ((check (fn () (assert-type (loop-assertion-source value) (:: 'Option 'String)) 42))) (if (&< n 1) (recur 1 (Option :some |kept)) (check)))`);
  for (const name of ["first-method-direct", "first-method-reversed", "first-method-alias", "first-method-captured", "first-method-shadowed", "first-method-effects", "first-method-forwarded", "first-method-staged", "first-method-match-join", "first-method-if-join", "first-method-independent-boundary"]) {
    run("edit", "add-test", "calcit.assert-evidence/recursive-count", name, "--tags", "recursive-fields,recursive-method-phase",
      "--input-format", "cirru", "--code", `quote $ assert= |7 $ ${name}`);
  }
  run("test", "calcit.assert-evidence/recursive-count", "--tag", "recursive-fields", "--require-match");
  const recursiveTests = JSON.parse(run("query", "def", "calcit.assert-evidence/recursive-count", "--format", "json"))
    .data.tests.filter(test => test.tags.includes("recursive-fields"));
  assert.equal(recursiveTests.length, 13);
  setBody(recursiveTests.map(test => test.code));
  const loggedLoop = spawnSync(binary, [snapshot, "--check-only"], options);
  if (loggedLoop.error) throw loggedLoop.error;
  assert.equal(loggedLoop.status, 0, `${loggedLoop.stdout}\n${loggedLoop.stderr}`);
  assert.equal((`${loggedLoop.stdout}\n${loggedLoop.stderr}`.match(/loop-expansion-token/g) ?? []).length, 1,
    "solving loop input constraints must not repeat an effectful macro expansion");
  assert.equal((`${loggedLoop.stdout}\n${loggedLoop.stderr}`.match(/loop-occurrence-token/g) ?? []).length, 5,
    "two interpolated occurrences, two nested function occurrences and a pre-dispatch transfer must each expand exactly once");
  assert.equal((`${loggedLoop.stdout}\n${loggedLoop.stderr}`.match(/loop-assertion-token/g) ?? []).length, 2,
    "direct and captured assertion inputs must keep their source identity during retries");
  run();
  const recursiveOutput = join(project, "recursive-fields-js");
  run("--emit-path", recursiveOutput, "js");
  const recursiveJs = await import(pathToFileURL(join(recursiveOutput, "calcit.assert-evidence.mjs")).href);
  assert.equal(recursiveJs.run_tests(), 1);
  for (const [name, expression] of [
    ["wrong-scalar-literal", "%{} WriteState (:count |wrong) (:label |kept)"],
    ["wrong-scalar-head", "WriteState :count |wrong :label |kept"],
    ["broad-enum-payload", "SpecificNodeHolder :cell (RecursiveCell :value (RecursiveElement :children ([]))) :tree $ Option :some $ make-broad-element"],
    ["broad-struct-payload", "SpecificNodeHolder :cell (RecursiveCell :value (make-broad-element)) :tree $ Option :none"],
    ["open-enum-payload", "RecursiveComponent :tree $ make-open-tree"],
    ["open-local-payload", "let ((open-tree (make-open-tree))) (RecursiveComponent :tree open-tree)"],
    ["wrong-fold-payload", "RecursiveComponent :tree $ option:fold (Option :some true) (fn () $ Option :none) (fn (present?) $ Option :some 1)"],
    ["open-fold-output", "(fn (flag) (RecursiveComponent :tree (option:fold (Option :some flag) (fn () (Option :none)) (fn (present?) (make-open-number))))) true"],
    ["wrong-loop-payload-use", "loop ((n 0) (tree (Option :none))) (if (&< n 1) (recur 1 (Option :some |wrong)) (RecursiveComponent :tree tree))"],
    ["open-pre-dispatch-payload", "loop ((n 0) (value (Option :none))) (if (&< n 1) (recur 1 (Option :some (make-open-number))) ((value .unwrap) .to-string))"],
    ["open-pre-dispatch-alias", "loop ((n 0) (value (Option :none))) (let ((alias value)) (if (&< n 1) (recur 1 (Option :some (make-open-number))) ((alias .unwrap) .to-string)))"],
    ["open-pre-dispatch-capture", "loop ((n 0) (value (Option :none))) (let ((render (fn () ((value .unwrap) .to-string)))) (if (&< n 1) (recur 1 (Option :some (make-open-number))) (render)))"],
    ["wrong-pre-dispatch-method", "loop ((n 0) (value (Option :none))) (if (&< n 1) (recur 1 (Option :some 7)) ((value .unwrap) .unknown-method))"],
    ["wrong-static-literal-method", "7 .unknown-method"],
    ["wrong-static-expression-method", "(+ 3 4) .unknown-method"],
    ["recursive-pre-dispatch-slot", "loop ((value (Option :none))) (recur (Option :some value))"],
    ["indirect-recursive-pre-dispatch-slots", "loop ((left (Option :none)) (right (Option :none))) (recur (Option :some right) left)"],
    ["nonexhaustive-pre-dispatch-match", "loop ((n 0) (value (Option :none))) (match value ((:none) (if (&< n 1) (recur 1 (Option :some 7)) 42)))"],
    ["duplicate-pre-dispatch-constructor-field", "loop ((n 0) (value (Option :none))) (WriteState :count 1 :label |kept :count 2) (if (&< n 1) (recur 1 (Option :some 7)) 42)"],
    ["unknown-pre-dispatch-constructor-variant", "loop ((n 0) (value (Option :none))) (Option :unknown) (if (&< n 1) (recur 1 (Option :some 7)) 42)"],
    ["wrong-macro-literal", "require-string-expression 7"],
    ["wrong-macro-local", "let ((value 7)) (require-string-expression value)"],
    ["wrong-macro-alias", "let ((value 7) (alias value)) (require-string-expression alias)"],
    ["wrong-macro-capture", "let ((value 7) (render (fn () (require-string-expression value)))) (render)"],
    ["wrong-macro-loop", "loop ((n 0) (value (Option :none))) (require-string-option value) (if (&< n 1) (recur 1 (Option :some 7)) 42)"],
    ["wrong-macro-internal-recur", "loop ((n 0) (value (Option :none))) (string-option-recur (Option :none) value true) (if (&< n 1) (recur 1 (Option :some 7)) 42)"],
    ["wrong-macro-loop-alias", "loop ((n 0) (value (Option :none))) (let ((alias value)) (require-string-option alias)) (if (&< n 1) (recur 1 (Option :some 7)) 42)"],
    ["wrong-macro-loop-capture", "loop ((n 0) (value (Option :none))) (let ((check (fn () (require-string-option value)))) (check)) (if (&< n 1) (recur 1 (Option :some 7)) 42)"],
    ["wrong-macro-loop-head", "loop ((n 0) (value (Option :none))) ((string-option-head value)) (if (&< n 1) (recur 1 (Option :some 7)) 42)"],
    ["wrong-macro-loop-constructor-head", "loop ((n 0) (value (Option :none))) ((string-option-type value) :some 7) (if (&< n 1) (recur 1 (Option :some 7)) 42)"],
    ["wrong-macro-result-loop", "loop ((n 0) (value (Option :none))) (string-option-result value) (if (&< n 1) (recur 1 (Option :some 7)) 42)"],
    ["wrong-macro-result-alias", "loop ((n 0) (value (Option :none))) (let ((alias value)) (string-option-result alias)) (if (&< n 1) (recur 1 (Option :some 7)) 42)"],
    ["wrong-macro-result-capture", "loop ((n 0) (value (Option :none))) (let ((check (fn () (string-option-result value) 42))) (check)) (if (&< n 1) (recur 1 (Option :some 7)) 42)"],
    ["wrong-matched-loop-payload", "loop ((tree (Option :none))) (match tree ((:none) (recur (Option :some |wrong))) ((:some payload) (RecursiveComponent :tree (Option :some payload))))"],
    ["contradictory-loop-assertion", "loop ((n 0) (value (Option :none))) (assert-type value (:: 'Option 'String)) (if (&< n 1) (recur 1 (Option :some 7)) 42)"],
    ["contradictory-captured-loop-assertion", "loop ((n 0) (value (Option :none))) (let ((check (fn () (assert-type value (:: 'Option 'String)) 42))) (if (&< n 1) (recur 1 (Option :some 7)) (check)))"],
    ["unproven-loop-assertion", "loop ((n 0) (value (Option :none))) (assert-type value (:: 'Option 'String)) (if (&< n 1) (recur 1 (Option :some (make-open-number))) 42)"],
    ["unproven-captured-loop-assertion", "loop ((n 0) (value (Option :none))) (let ((check (fn () (assert-type value (:: 'Option 'String)) 42))) (if (&< n 1) (recur 1 (Option :some (make-open-number))) (check)))"],
    ["unproven-loop-expression-assertion", "loop ((n 0) (value (Option :none))) (assert-type (if (&< n 1) value value) (:: 'Option 'String)) (if (&< n 1) (recur 1 (Option :some (make-open-number))) 42)"],
    ["unproven-captured-loop-expression-assertion", "loop ((n 0) (value (Option :none))) (let ((check (fn () (assert-type (if (&< n 1) value value) (:: 'Option 'String)) 42))) (if (&< n 1) (recur 1 (Option :some (make-open-number))) (check)))"],
    ["open-loop-payload-use", "loop ((n 0) (tree (Option :none))) (if (&< n 1) (recur 1 (Option :some (make-open-number))) (RecursiveComponent :tree tree))"],
    ["mixed-loop-payloads", "(fn (flag) (loop ((n 0) (tree (Option :none))) (if (&< n 1) (recur 1 (if flag (Option :some 1) (Option :some |wrong))) (RecursiveComponent :tree tree)))) true"],
    ["wrong-loop-family", "loop ((n 0) (signal (RecursiveSignal :idle))) (if (&< n 1) (recur 1 (Option :some 1)) (RecursiveSignalHolder :signal signal))"],
    // A Ref's payload is fixed at construction; an empty initializer needs explicit Option<T> context (#1737).
    ["empty-ref-wrong-write", "let ((cell (atom (assert-type (Option :none) (:: 'Option 'Number))))) (reset! cell (Option :some |wrong))"],
    ["empty-ref-alias-write", "let ((cell (atom (assert-type (Option :none) (:: 'Option 'Number)))) (alias cell)) (reset! alias (Option :some |wrong))"],
    ["empty-ref-unannotated-write", "let ((cell (atom (Option :none)))) (reset! cell (Option :some 7))"],
    ["invariant-loop-ref", "let ((fixed (assert-type (atom (Option :some 7)) (:: 'Ref (:: 'Option 'Number))))) (loop ((n 0) (cell (atom (assert-type (Option :none) (:: 'Option 'Dynamic))))) (hint-fn ({} (:args ([] 'Number (:: 'Ref (:: 'Option 'Dynamic)))) (:return 'Number))) (if (&< n 1) (recur 1 fixed) 42))"],
    ["explicit-loop-contract", "loop ((n 0) (value (Option :none))) (hint-fn ({} (:args ([] 'Number (:: 'Option 'Number))) (:return 'Number))) (if (&< n 1) (recur 1 (Option :some |wrong)) (option:unwrap-or value 0))"],
    ["explicit-loop-unknown-update", "(fn (flag) (loop ((n 0) (value (Option :none))) (hint-fn ({} (:args ([] 'Number (:: 'Option 'Number))) (:return 'Number))) (if (&< n 1) (recur 1 (Option :some (if flag 7 |wrong))) (option:unwrap-or value 0)))) true"],
    ["wrong-empty-family", "let ((absent-tree (Option :none))) (RecursiveSignalHolder :signal absent-tree)"],
    ["open-signal-local", "let ((open-signal (make-open-signal))) (RecursiveSignalHolder :signal open-signal)"],
    ["wrong-callback-return", "CallbackHolder :handler $ fn (value) |wrong"],
    ["open-imported-branch", "(fn (flag) (RecursiveComponent :tree (if flag (wrappers/open-tree) (Option :some (RecursiveNode :element (RecursiveElement :children ([]))))))) false"],
    ["wrong-variant-payload", "RecursiveNode :element $ RecursiveComponent :tree $ Option :none"],
    ["unwrapped-node-literal", "%{} RecursivePair (:key :a) (:node $ %{} RecursiveElement (:children $ []))"],
    ["unwrapped-node-head", "RecursivePair :key :a :node $ RecursiveElement :children $ []"],
    ["raw-pair-list-literal", "%{} RecursiveElement (:children $ [] $ [] :a $ RecursiveNode :element $ RecursiveElement :children $ [])"],
    ["raw-pair-list-head", "RecursiveElement :children $ [] $ [] :a $ RecursiveNode :element $ RecursiveElement :children $ []"],
    ["wrong-matched-field", "let ((node (RecursiveNode :component (RecursiveComponent :tree (Option :none))))) (match node ((:element element) (:children element)) ((:component component) (:children component)))"],
    ["raw-shape-missing", "%{} WriteState (:count 1)"],
    ["raw-shape-duplicate", "%{} WriteState (:count 1) (:count 2)"],
    ["raw-shape-unknown", "%{} WriteState (:count 1) (:other |wrong)"],
    ["raw-shape-string-unknown", "%{} WriteState (:count 1) (|other |wrong)"],
    ["raw-shape-string-duplicate", "%{} WriteState (|count 1) (|count 2)"],
    ["raw-shape-symbol-unknown", "%{} WriteState (:count 1) ('other |wrong)"],
    ["raw-shape-invalid-key", "%{} WriteState (:count 1) (42 |wrong)"],
    ["raw-shape-invalid-quoted-key", "%{} WriteState (:count 1) ((quote true) |wrong)"],
    ["raw-shape-string-payload", "%{} WriteState (|count |wrong) (|label |kept)"],
    ["raw-shape-symbol-payload", "%{} WriteState ('count |wrong) ('label |kept)"],
    ["raw-shape-local-alias", "let ((Shape WriteState)) (%{} Shape (:count 1))"],
    ["raw-shape-imported", "%{} wrappers/ImportedShape (:left 1)"],
    ["raw-shape-imported-alias", "let ((Shape wrappers/ImportedShape)) (%{} Shape (:left 1))"],
    ["raw-shape-payload", "%{} WriteState (:count |wrong) (:label |kept)"],
  ]) {
    run("edit", "def", "calcit.assert-evidence/run-tests", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defwasm-export run-tests () (${expression}) 1`);
    const original = await readFile(snapshot);
    const expectedDiagnostic = ["raw-shape-duplicate", "raw-shape-string-duplicate"].includes(name) ? /construction duplicate field `:count`/
      : ["raw-shape-unknown", "raw-shape-string-unknown", "raw-shape-symbol-unknown"].includes(name) ? /construction unknown field `:other`/
      : name.startsWith("raw-shape-invalid-") ? /field key must be a Tag, String or Symbol/
      : name.startsWith("raw-shape-") && !name.endsWith("-payload") ? /construction expected 2 fields, but received 1/
      : name.startsWith("open-pre-dispatch-") ? /E_DYNAMIC_POSTFIX_METHOD/
      : name === "nonexhaustive-pre-dispatch-match" ? /match on `Option` is not exhaustive/
      : name === "duplicate-pre-dispatch-constructor-field" ? /duplicate field `:count`/
      : name === "unknown-pre-dispatch-constructor-variant" ? /enum `Option` does not have variant `:unknown`/
      : ["recursive-pre-dispatch-slot", "indirect-recursive-pre-dispatch-slots"].includes(name) ? /W_RECUR_ARG_TYPE_MISMATCH/
      : ["wrong-pre-dispatch-method", "wrong-static-literal-method", "wrong-static-expression-method"].includes(name) ? /unknown method `.unknown-method`/
      : name.startsWith("wrong-macro-result-") ? /E_MACRO_EXPANSION_EXPR_TYPE/
      : name.startsWith("wrong-macro-") ? /E_MACRO_INPUT_EXPR_TYPE/
      : name.startsWith("contradictory-") ? /E_ASSERT_TYPE_MISMATCH/
      : name.startsWith("unproven-") ? /E_ASSERT_TYPE_UNPROVEN/
      : name === "wrong-variant-payload" ? /Enum `RecursiveNode::element` payload 1 expects type/
      : name === "wrong-matched-field" ? /Field `:children` does not exist in struct `RecursiveComponent`/
      : /expects type/;
    for (const mode of [[], ["--check-only"], ["js"], ["wasm"], ["wasm", "--check-only"], ["wasi"], ["wasi", "--check-only"]]) {
      const destination = join(project, `recursive-rejected-${name}-${mode.join("-") || "native"}`);
      const rejected = spawnSync(binary, [snapshot, "--emit-path", destination, ...mode], options);
      if (rejected.error) throw rejected.error;
      assert.equal(rejected.status, 1, `${name} ${mode}\n${rejected.stdout}\n${rejected.stderr}`);
      const diagnostics = `${rejected.stdout}\n${rejected.stderr}`;
      if (name === "nonexhaustive-pre-dispatch-match") {
        assert.equal((diagnostics.match(/match on `Option` is not exhaustive/g) ?? []).length, 1,
          `${mode}: preparation must defer ordinary match warnings to the actual checking stage`);
      }
      if (name === "duplicate-pre-dispatch-constructor-field" || name === "unknown-pre-dispatch-constructor-variant") {
        const constructorWarning = name === "duplicate-pre-dispatch-constructor-field"
          ? /duplicate field `:count`/g : /enum `Option` does not have variant `:unknown`/g;
        assert.equal((diagnostics.match(constructorWarning) ?? []).length, 1,
          `${mode}: preparation must defer skipped constructor warnings to ordinary source checking`);
      }
      assert.match(diagnostics, expectedDiagnostic);
      assert.match(diagnostics, /calcit.assert-evidence/);
      if (name.startsWith("raw-shape-")) {
        assert.match(diagnostics, /W_FN_ARG_TYPE_MISMATCH/);
        assert.match(diagnostics, /@calcit\.assert-evidence\/run-tests @[0-9]/,
          "resolved raw constructors must locate the source call");
      }
      if (name.startsWith("empty-ref-")) {
        assert.match(diagnostics, /W_RESET_ARG_TYPE_MISMATCH/);
        const fixHint = /give the initializer explicit type context, e\.g\. `ref \$ assert-type \(Option :none\) \$ :: 'Option 'Number`/;
        if (name === "empty-ref-unannotated-write") assert.match(diagnostics, fixHint);
        else assert.doesNotMatch(diagnostics, fixHint);
      }
      if (name === "contradictory-loop-assertion" || name === "contradictory-captured-loop-assertion") {
        assert.match(diagnostics, /E_ASSERT_TYPE_MISMATCH/);
        assert.match(diagnostics, /assert-type cannot prove local `value`/);
      }
      if (name.startsWith("unproven-") && name.endsWith("-assertion")) {
        assert.match(diagnostics, /E_ASSERT_TYPE_UNPROVEN/);
        assert.match(diagnostics, /assert-type lacks independent input proof/);
      }
      const field = name.startsWith("wrong-scalar") ? "count"
        : ["broad-enum-payload", "open-enum-payload", "open-local-payload", "open-imported-branch", "wrong-fold-payload", "open-fold-output", "wrong-loop-payload-use", "wrong-matched-loop-payload", "open-loop-payload-use"].includes(name) ? "tree"
        : name === "broad-struct-payload" ? "cell"
        : ["wrong-empty-family", "open-signal-local"].includes(name) ? "signal"
        : name === "wrong-callback-return" ? "handler"
        : name.startsWith("unwrapped-node") ? "node"
        : name.startsWith("raw-pair-list") ? "children" : null;
      if (field !== null) {
        assert.match(diagnostics, /W_FN_ARG_TYPE_MISMATCH/);
        assert.ok(diagnostics.includes(`field \`:${field}\` expects type`), diagnostics);
        assert.match(diagnostics, /@calcit\.assert-evidence\/run-tests @[0-9]/,
          "constructor diagnostics must locate the source call, not only a generated macro");
      }
      await assertRejectedArtifacts(destination, `${name} ${mode}`, expectedDiagnostic, mode.includes("js"));
      assert.deepEqual(await readFile(snapshot), original);
    }
  }
  setBody(nominalTests.map(test => test.code));

  // Raw WASM exports let the host supply malformed pointers despite source
  // schemas. Keep ordinary method calls and probe the allocation boundary.
  for (const [name, parameters, body, schema] of [
    ["make-state", [], ["WriteState", ":count", "1", ":label", "|kept"], "quote $ :: 'Fn $ {} (:args $ []) (:return 'WriteState)"],
    ["make-list", ["layout"], ["[]", "layout", "9"], "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return $ :: 'List 'Number)"],
    ["raw-write", ["receiver"], ["receiver", ".assoc", ":count", "2"], "quote $ :: 'Fn $ {} (:args $ [] 'WriteState) (:return 'WriteState)"],
  ]) {
    run("edit", "def", `calcit.assert-evidence/${name}`, "--input-format", "json-ast", "--code",
      JSON.stringify(["defwasm-export", name, parameters, body]));
    run("edit", "schema", `calcit.assert-evidence/${name}`, "--input-format", "cirru", "--code", schema);
  }
  run("wasm", "--emit-path", nominalWasmOutput);
  const guardedModule = new WebAssembly.Module(await readFile(join(nominalWasmOutput, "program.wasm")));
  const forgedInstance = new WebAssembly.Instance(guardedModule, imports);
  const forgedState = forgedInstance.exports["make-state"]();
  const forgedMemory = new DataView(forgedInstance.exports.memory.buffer);
  const forgedList = forgedInstance.exports["make-list"](forgedMemory.getFloat64(forgedState + 8, true));
  assert.throws(() => forgedInstance.exports["raw-write"](forgedList), WebAssembly.RuntimeError,
    "a List with a matching nominal-layout slot must not become a Struct");
  for (const count of [-1, 0, 1, 3, 1.5, NaN, Infinity]) {
    const instance = new WebAssembly.Instance(guardedModule, imports);
    const original = instance.exports["make-state"]();
    const memory = new DataView(instance.exports.memory.buffer);
    memory.setFloat64(original, count, true);
    assert.throws(() => instance.exports["raw-write"](original), WebAssembly.RuntimeError, `wrong count: ${count}`);
    assert.equal(memory.getFloat64(original + 16, true), 1, "rejected updates preserve the source field");
    assert.equal(instance.exports["make-state"](), original + 40, "rejected updates must not allocate a result");
  }
  for (const malformed of ["magic", "fractional", "unaligned", "outside", "nan", "negative"]) {
    const instance = new WebAssembly.Instance(guardedModule, imports);
    const original = instance.exports["make-state"]();
    const memory = new DataView(instance.exports.memory.buffer);
    if (malformed === "magic") memory.setUint32(original - 8, 0, true);
    const receiver = malformed === "fractional" ? original + 0.5
      : malformed === "unaligned" ? original + 1
      : malformed === "outside" ? memory.byteLength
      : malformed === "nan" ? NaN
      : malformed === "negative" ? -1 : original;
    assert.throws(() => instance.exports["raw-write"](receiver), WebAssembly.RuntimeError, malformed);
  }
  for (const offset of [16, 24]) {
    const instance = new WebAssembly.Instance(guardedModule, imports);
    const original = instance.exports["make-state"]();
    const memory = new DataView(instance.exports.memory.buffer);
    const magic = memory.getUint32(original - 8, true);
    const kind = memory.getUint32(original - 4, true);
    const layout = memory.getFloat64(original + 8, true);
    const receiver = original + offset;
    memory.setUint32(receiver - 8, magic, true);
    memory.setUint32(receiver - 4, kind, true);
    memory.setFloat64(receiver, 2, true);
    memory.setFloat64(receiver + 8, layout, true);
    assert.throws(() => instance.exports["raw-write"](receiver), WebAssembly.RuntimeError,
      "a forged prefix or copy span beyond the allocated heap must trap");
    assert.equal(instance.exports["make-state"](), original + 40, "heap-span rejection must precede result allocation");
  }

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
  // Declaration-owned field names must survive construction, reads and updates.
  await copyFile("tests/fixtures/def-value-schema.cirru", snapshot);
  run("test", "--tag", "struct-field-origin", "--require-match");
  const fieldTests = ["verify-map", "verify-optional", "verify-generic", "verify-generic-origin", "verify-update"].flatMap(name => {
    const report = JSON.parse(run("query", "def", `app.field-consumer/${name}`, "--format", "json"));
    assert.deepEqual(report.diagnostics, []);
    assert.equal(report.data.tests.length, 1);
    return report.data.tests.map(test => test.code);
  });
  run("edit", "def", "app.field-consumer/replay!", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "replay!", [], ...fieldTests, "&unit"]));
  run("edit", "schema", "app.field-consumer/replay!", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)");
  run("config", "set", "init-fn", "app.field-consumer/replay!");
  run("config", "set", "reload-fn", "app.field-consumer/replay!");
  run("--check-only");
  const fieldOutput = join(project, "field-origin-js");
  run("--emit-path", fieldOutput, "js");
  const fieldModule = await import(pathToFileURL(join(fieldOutput, "app.field-consumer.mjs")).href);
  fieldModule.replay_$x_();
  for (const [name, expression] of [
    ["foreign-map-value", "owner/Database :users ({} (|one (User :name |wrong-origin))) :maybe nil"],
    ["wrong-map-value", "owner/Database :users ({} (|one 42)) :maybe nil"],
    ["foreign-optional-value", "owner/Database :users ({}) :maybe (User :name |wrong-origin)"],
    ["foreign-generic-field", "owner/Envelope :user (User :name |wrong-origin) :value 42"],
    ["foreign-update", "let ((db (owner/Database :users ({}) :maybe nil))) (db .assoc :maybe (User :name |wrong-origin))"],
  ]) {
    run("edit", "def", "app.field-consumer/replay!", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn replay! () (${expression}) &unit`);
    const original = await readFile(snapshot);
    for (const mode of [["--check-only"], ["js"]]) {
      const output = join(project, `field-${name}-${mode[0] === "js" ? "js" : "native"}`);
      const result = spawnSync(binary, ["--emit-path", output, snapshot, ...mode], options);
      if (result.error) throw result.error;
      assert.equal(result.status, 1, `${name} ${mode}\n${result.stdout}\n${result.stderr}`);
      assert.match(`${result.stdout}\n${result.stderr}`, /expects type|W_FN_ARG_TYPE_MISMATCH/);
      assert.deepEqual(await readFile(snapshot), original);
      if (mode[0] === "js") await assertRejectedArtifacts(output, name, /expects type|W_FN_ARG_TYPE_MISMATCH/, true);
    }
  }

  // Optional adds nil, not a stricter contract for proven empty literals.
  await copyFile("tests/fixtures/def-value-schema.cirru", snapshot);
  run("test", "--tag", "optional-empty-field", "--require-match");
  const optionalTests = ["verify-empty", "verify-nil"].flatMap(name => {
    const report = JSON.parse(run("query", "def", `app.empty-fields/${name}`, "--format", "json"));
    assert.deepEqual(report.diagnostics, []);
    assert.equal(report.data.tests.length, 1);
    return report.data.tests.map(test => test.code);
  });
  run("edit", "def", "app.empty-fields/replay!", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "replay!", [], ...optionalTests, "&unit"]));
  run("edit", "schema", "app.empty-fields/replay!", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)");
  run("config", "set", "init-fn", "app.empty-fields/replay!");
  run("config", "set", "reload-fn", "app.empty-fields/replay!");
  run("--check-only");
  const optionalOutput = join(project, "optional-empty-js");
  run("--emit-path", optionalOutput, "js");
  const optionalModule = await import(pathToFileURL(join(optionalOutput, "app.empty-fields.mjs")).href);
  optionalModule.replay_$x_();
  for (const [name, field, value] of [
    ["list-is-not-map", ":mapping", "[]"],
    ["set-is-not-map", ":mapping", "#{}"],
    ["map-is-not-list", ":items", "{}"],
    ["list-is-not-set", ":members", "[]"],
    ["wrong-map-key", ":mapping", "{} (|key |value)"],
    ["wrong-map-value", ":mapping", "{} (:key 42)"],
    ["wrong-list-value", ":items", "[] |wrong"],
    ["wrong-set-value", ":members", "#{} 42"],
    ["open-map-is-not-literal", ":mapping", "open-map"],
  ]) {
    const fields = [":mapping", ":items", ":members"].map(key => `${key} ${key === field ? `(${value})` : "nil"}`).join(" ");
    run("edit", "def", "app.empty-fields/replay!", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn replay! () (Fields ${fields}) &unit`);
    const original = await readFile(snapshot);
    for (const mode of [["--check-only"], ["js"]]) {
      const output = join(project, `optional-${name}-${mode[0] === "js" ? "js" : "native"}`);
      const result = spawnSync(binary, ["--emit-path", output, snapshot, ...mode], options);
      if (result.error) throw result.error;
      assert.equal(result.status, 1, `${name} ${mode}\n${result.stdout}\n${result.stderr}`);
      assert.match(`${result.stdout}\n${result.stderr}`, /W_FN_ARG_TYPE_MISMATCH/);
      assert.deepEqual(await readFile(snapshot), original);
      if (mode[0] === "js") await assertRejectedArtifacts(output, name, /W_FN_ARG_TYPE_MISMATCH/, true);
    }
  }

  // Nullable branch evidence must survive local bindings without proving open values.
  await copyFile("tests/fixtures/def-value-schema.cirru", snapshot);
  run("test", "--tag", "nullable-branch-binding", "--require-match");
  run("test", "--tag", "open-match-payload", "--require-match");
  const bindingTests = ["lookup-inline", "lookup-local", "if-value-first", "if-nil-first",
    "choose-option", "choose-nullable", "choose-nil", "decode-lookup", "decode-option",
    "decode-result", "retain-open"].flatMap(name => {
    const report = JSON.parse(run("query", "def", `app.binding-proof/${name}`, "--format", "json"));
    assert.deepEqual(report.diagnostics, []);
    assert.equal(report.data.tests.length, 1);
    return report.data.tests.map(test => test.code);
  });
  run("edit", "def", "app.binding-proof/replay!", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "replay!", [], ...bindingTests, "&unit"]));
  run("edit", "schema", "app.binding-proof/replay!", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args ([])) (:return 'Unit)");
  run("config", "set", "init-fn", "app.binding-proof/replay!");
  run("config", "set", "reload-fn", "app.binding-proof/replay!");
  run("--check-only");
  const bindingOutput = join(project, "nullable-binding-js");
  run("--emit-path", bindingOutput, "js");
  const bindingModule = await import(pathToFileURL(join(bindingOutput, "app.binding-proof.mjs")).href);
  bindingModule.replay_$x_();
  run("edit", "def", "app.binding-proof/RequiredValue", "--input-format", "cirru", "--code",
    "quote $ defstruct RequiredValue (:value 'app.binding-proof/Value)");
  run("edit", "schema", "app.binding-proof/RequiredValue", "--input-format", "cirru", "--code", "quote 'StructDef");
  run("edit", "def", "app.binding-proof/RequiredOption", "--input-format", "cirru", "--code",
    "quote $ defstruct RequiredOption (:value (:: 'Option 'String))");
  run("edit", "schema", "app.binding-proof/RequiredOption", "--input-format", "cirru", "--code", "quote 'StructDef");
  run("config", "set", "init-fn", "app.binding-proof/rejected");
  run("config", "set", "reload-fn", "app.binding-proof/rejected");
  for (const [name, type, expression, consumer] of [
    ["nullable-is-not-value", "'app.binding-proof/Value", "if present value nil", "RequiredValue :value selected"],
    ["reversed-nullable-is-not-value", "'app.binding-proof/Value", "if present nil value", "RequiredValue :value selected"],
    ["open-is-not-value", "'Dynamic", "if present value nil", "Box :value selected"],
    ["reversed-open-is-not-value", "'Dynamic", "if present nil value", "Box :value selected"],
    ["foreign-is-not-value", "'app.field-consumer/User", "if present value nil", "Box :value selected"],
    ["string-is-not-value", "'String", "if present value nil", "Box :value selected"],
    ["nil-is-not-option", "(:: 'Option 'String)", "if present value nil", "RequiredOption :value selected"],
    ["wrong-match-payload", "(:: 'Map 'String 'String)", "match (value .get |key) ((:some item) item) ((:none) nil)", "Box :value selected"],
    ["open-map-match", "(:: 'Map 'String 'Dynamic)", "match (value .get |key) ((:some item) item) ((:none) nil)", "Box :value selected"],
    ["reversed-open-map-match", "(:: 'Map 'String 'Dynamic)", "match (value .get |key) ((:none) nil) ((:some item) item)", "Box :value selected"],
    ["open-option-match", "(:: 'Option 'Dynamic)", "match value ((:some item) item) ((:none) nil)", "Box :value selected"],
    ["open-result-ok-match", "(:: 'Result 'Dynamic 'String)", "match value ((:ok item) item) ((:err reason) nil)", "Box :value selected"],
    ["open-result-err-match", "(:: 'Result 'String 'Dynamic)", "match value ((:ok item) nil) ((:err reason) reason)", "Box :value selected"],
    ["open-payload-alias", "(:: 'Option 'Dynamic)", "match value ((:some item) item) ((:none) nil)", "let ((alias selected)) (Box :value alias)"],
  ]) {
    run("edit", "def", "app.binding-proof/rejected", "--overwrite", "--input-format", "cirru", "--code",
      `quote $ defn rejected (present value) (let ((selected (${expression}))) (${consumer}) &unit)`);
    run("edit", "schema", "app.binding-proof/rejected", "--input-format", "cirru", "--code",
      `quote $ :: 'Fn $ {} (:args ([] 'Bool ${type})) (:return 'Unit)`);
    const original = await readFile(snapshot);
    for (const mode of [["--check-only"], ["js"]]) {
      const output = join(project, `binding-${name}-${mode[0] === "js" ? "js" : "native"}`);
      const result = spawnSync(binary, ["--emit-path", output, snapshot, ...mode], options);
      if (result.error) throw result.error;
      assert.equal(result.status, 1, `${name} ${mode}\n${result.stdout}\n${result.stderr}`);
      const diagnostic = /W_FN_ARG_TYPE_MISMATCH/;
      assert.match(`${result.stdout}\n${result.stderr}`, diagnostic);
      assert.deepEqual(await readFile(snapshot), original);
      if (mode[0] === "js") await assertRejectedArtifacts(output, name, diagnostic, true);
    }
  }

  await copyFile("tests/fixtures/trait-bound-return.cirru", snapshot);
  const traitOriginal = await readFile(snapshot);
  run("test", "--require-match");
  run("fix", "--workflow", "strict", "--verify", "--format", "edn");
  assert.deepEqual(await readFile(snapshot), traitOriginal);
  const traitTests = [];
  for (const definition of ["checked-count", "render-with-trait", "identity-with-trait", "typed-rest-forward"]) {
    const response = JSON.parse(run("query", "def", `fix-command.main/${definition}`, "--format", "json"));
    assert.equal(response.data.tests.length, 1);
    traitTests.push(...response.data.tests.map(test => test.code));
  }
  run("edit", "def", "fix-command.main/main!", "--overwrite", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "main!", [], ...traitTests, "&unit"]));
  run("--check-only");
  const traitOutput = join(project, "trait-return-js");
  run("--emit-path", traitOutput, "js");
  const traitGenerated = await import(pathToFileURL(join(traitOutput, "fix-command.main.mjs")).href);
  traitGenerated.main_$x_();
  for (const [name, target, command, code] of [
    ["wrong-result", "checked-count", "schema", "quote $ :: 'Fn $ {} (:args $ [] 'T) (:return 'String) (:generics $ [] 'T) (:where $ {} $ 'T 'Countable)"],
    ["open-generic-result", "identity-with-trait", "schema", "quote $ :: 'Fn $ {} (:args $ [] 'T 'U) (:return 'String) (:generics $ [] 'T 'U) (:where $ {} $ 'T 'fix-command.main/Renderable)"],
    ["dynamic-argument", "render-with-trait", "schema", "quote $ :: 'Fn $ {} (:args $ [] 'T 'Dynamic) (:return 'String) (:generics $ [] 'T) (:where $ {} $ 'T 'fix-command.main/Renderable)"],
    ["open-receiver", "checked-count", "schema", "quote $ :: 'Fn $ {} (:args $ [] 'T) (:return 'Number) (:generics $ [] 'T)"],
    ["wrong-argument", "render-with-trait", "def", "quote $ defn render-with-trait (value prefix) (.render value 1)"],
    ["missing-argument", "render-with-trait", "def", "quote $ defn render-with-trait (value prefix) (.render value)"],
    ["extra-argument", "render-with-trait", "def", "quote $ defn render-with-trait (value prefix) (.render value prefix prefix)"],
  ]) {
    await copyFile("tests/fixtures/trait-bound-return.cirru", snapshot);
    run("edit", command, `fix-command.main/${target}`, ...(command === "def" ? ["--overwrite"] : []),
      "--input-format", "cirru", "--code", code);
    const original = await readFile(snapshot);
    const rejected = spawnSync(binary, [snapshot, "fix", "--workflow", "strict", "--verify", "--format", "edn"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${name}\n${rejected.stdout}\n${rejected.stderr}`);
    assert.deepEqual(await readFile(snapshot), original);
  }
  // Core callable/collection/rest contracts already run in run-core-tests.mjs.
  // Keep fixture-only nominal and typed-rest contracts here, without replacing
  // their original expressions or duplicating the bundled core replay.
  for (const [source, namespace, definitions, expectedCount, outputName] of [
    ["tests/fixtures/count-contract.cirru", "fix-command.main", ["typed-rest-forward", "nominal-counts", "checked-open-count", "checked-string-count", "checked-core-alias-count", "local-bound-counts", "typed-loop-count"], 7, "count-contract-js"],
    ["tests/fixtures/typed-rest-spread.cirru", "fix-command.main", ["typed-rest-forward"], 2, "typed-rest-spread-js"],
  ]) {
    await copyFile(source, snapshot);
    const original = await readFile(snapshot);
    const expressions = [];
    for (const definition of definitions) {
      run("test", `${namespace}/${definition}`, "--require-match");
      const response = JSON.parse(run("query", "def", `${namespace}/${definition}`, "--format", "json"));
      assert.deepEqual(response.diagnostics, []);
      expressions.push(...response.data.tests.map(test => test.code));
    }
    assert.equal(expressions.length, expectedCount);
    assert.deepEqual(await readFile(snapshot), original);
    run("edit", "def", `${namespace}/replay-count-tests`, "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "replay-count-tests", [], ...expressions, "&unit"]));
    run("edit", "schema", `${namespace}/replay-count-tests`, "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
    run("config", "set", "init-fn", `${namespace}/replay-count-tests`);
    run("config", "set", "reload-fn", `${namespace}/replay-count-tests`);
    run("--check-only");
    const output = join(project, outputName);
    run("--emit-path", output, "js");
    const generated = await import(pathToFileURL(join(output, `${namespace}.mjs`)).href);
    generated.replay_count_tests();
  }
  // An open or concrete result cannot independently prove a bare generic
  // return, whether it is the whole result or one unguarded branch; a branch
  // guarded by a kind predicate on the generic argument still proves it.
  for (const [name, body, accepted] of [
    ["open-list-for-bare-t", "([])", false],
    ["concrete-list-for-bare-t", "([] 1)", false],
    ["open-branch-for-bare-t", "(if (= 1 1) ([] 1) xs)", false],
    ["same-t-branches", "(if (= 1 1) xs xs)", true],
    ["guarded-list-branch", "(if (list? xs) ([] 1) xs)", true],
  ]) {
    await copyFile("tests/fixtures/count-contract.cirru", snapshot);
    run("edit", "def", "fix-command.main/bare-return", "--input-format", "cirru", "--code",
      `quote $ defn bare-return (xs) ${body}`);
    run("edit", "schema", "fix-command.main/bare-return", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'T) (:return 'T) (:generics $ [] 'T)");
    run("edit", "def", "fix-command.main/bare-return-entry", "--input-format", "cirru", "--code",
      "quote $ defn bare-return-entry () (bare-return 1) &unit");
    run("edit", "schema", "fix-command.main/bare-return-entry", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
    run("config", "set", "init-fn", "fix-command.main/bare-return-entry");
    const result = spawnSync(binary, [snapshot, "fix", "--workflow", "strict", "--verify", "--format", "json"], options);
    if (result.error) throw result.error;
    assert.equal(JSON.parse(result.stdout).data.workflow.status, accepted ? "passed" : "failed", `${name}\n${result.stdout}`);
  }
  for (const [name, input, body, outputType, accepted] of [
    // Nominal `assoc` is lowered to a Struct update after direct-call checks;
    // the strict field-value proof must still run before that lowering.
    ["assoc-struct-wrong-field", "'fix-command.main/Point", "assoc xs :x |wrong", "'fix-command.main/Point", false],
    ["assoc-struct-same-field", "'fix-command.main/Point", "assoc xs :x 3", "'fix-command.main/Point", true],
  ]) {
    await copyFile("tests/fixtures/count-contract.cirru", snapshot);
    run("edit", "def", "fix-command.main/Point", "--input-format", "cirru", "--code",
      "quote $ defstruct Point (:x 'Number)");
    run("edit", "def", "fix-command.main/rejected-map", "--input-format", "cirru", "--code",
      `quote $ defn rejected-map (xs) (${body})`);
    run("edit", "schema", "fix-command.main/rejected-map", "--input-format", "cirru", "--code",
      `quote $ :: 'Fn $ {} (:args $ [] ${input}) (:return ${outputType})`);
    const result = spawnSync(binary, [snapshot, "fix", "--workflow", "strict", "--verify", "--format", "json"], options);
    if (result.error) throw result.error;
    const report = JSON.parse(result.stdout);
    const diagnostics = report.diagnostics.filter(diagnostic => diagnostic.definition === "fix-command.main/rejected-map");
    if (accepted) {
      assert.equal(result.status, 0, `${name}\n${result.stdout}\n${result.stderr}`);
      assert.deepEqual(diagnostics, [], name);
    } else {
      assert.equal(result.status, 1, `${name}\n${result.stdout}\n${result.stderr}`);
      assert.ok(diagnostics.some(diagnostic => diagnostic.code === "E_CALL_ARGUMENT_MISMATCH"
        && /argument 3: expected `:number`, got `:string`/.test(diagnostic.message)
        && diagnostic.hint?.includes("Nominal field :x;")), `${name}\n${result.stdout}`);
    }
  }
  // Original Map callback types stay constrained; a typed internal fold must
  // not specialize open payloads or accept contradictory input/output contracts.
  for (const [name, input, body, outputType, generics = ""] of [
    ["wrong-key", "(:: 'Map 'Number 'Number)", "map-list-kv xs $ fn (key value) (hint-fn $ {} (:args $ [] 'String 'Number) (:return 'Number)) value", "(:: 'List 'Number)"],
    ["wrong-value", "(:: 'Map 'String 'Number)", "map-list-kv xs $ fn (key value) (hint-fn $ {} (:args $ [] 'String 'String) (:return 'String)) value", "(:: 'List 'String)"],
    ["open-value", "(:: 'Map 'String 'Dynamic)", "map-list-kv xs $ fn (key value) (hint-fn $ {} (:args $ [] 'String 'Number) (:return 'Number)) value", "(:: 'List 'Number)"],
    ["wrong-result", "(:: 'Map 'String 'Number)", "map-list-kv xs $ fn (key value) |wrong", "(:: 'List 'Number)"],
    ["wrong-decision", "(:: 'Map 'String 'Number)", "filter-map-kv xs $ fn (key value) true", "(:: 'Map 'String 'Number)"],
    ["wrong-predicate", "(:: 'Map 'String 'Number)", "xs .filter-kv $ fn (key value) 1", "(:: 'Map 'String 'Number)"],
    ["rigid-key", "(:: 'Map 'K 'V)", "map-list-kv xs $ fn (key value) (hint-fn $ {} (:args $ [] 'String 'V) (:return 'V)) value", "(:: 'List 'V)", "(:generics $ [] 'K 'V)"],
    ["rigid-result", "(:: 'Map 'K 'V)", "map-list-kv xs $ fn (key value) key", "(:: 'List 'U)", "(:generics $ [] 'K 'V 'U)"],
  ]) {
    await copyFile("tests/fixtures/count-contract.cirru", snapshot);
    run("edit", "def", "fix-command.main/rejected-map", "--input-format", "cirru", "--code",
      `quote $ defn rejected-map (xs) (${body})`);
    run("edit", "schema", "fix-command.main/rejected-map", "--input-format", "cirru", "--code",
      `quote $ :: 'Fn $ {} (:args $ [] ${input}) (:return ${outputType}) ${generics}`);
    const original = await readFile(snapshot);
    if (generics) {
      // Generic declarations are audited independently rather than trusted
      // through entry-only preprocessing of an uninstantiated function.
      const rejected = spawnSync(binary, [snapshot, "fix", "--workflow", "strict", "--verify",
        "--format", "json"], options);
      if (rejected.error) throw rejected.error;
      assert.equal(rejected.status, 1, `${name}\n${rejected.stdout}\n${rejected.stderr}`);
      const report = JSON.parse(rejected.stdout);
      assert.ok(report.diagnostics.some(diagnostic => diagnostic.definition === "fix-command.main/rejected-map"
        && /^(?:E_CALL_ARGUMENT_UNPROVEN|E_CONCRETE_RETURN_UNPROVEN|E_ERASED_GENERIC_RELATION)$/.test(diagnostic.code)),
        `${name}: missing independent generic proof diagnostic\n${rejected.stdout}`);
      assert.deepEqual(await readFile(snapshot), original);
      continue;
    }
    for (const mode of [[], ["js"]]) {
      const output = join(project, `map-rejected-${name}-${mode[0] ?? "native"}`);
      const rejected = spawnSync(binary, [snapshot, "--init-fn", "fix-command.main/rejected-map",
        "--reload-fn", "fix-command.main/rejected-map", ...(mode.length === 0 ? ["--check-only"] : []), "--emit-path", output, ...mode], options);
      if (rejected.error) throw rejected.error;
      assert.equal(rejected.status, 1, `${name} ${mode}\n${rejected.stdout}\n${rejected.stderr}`);
      assert.match(`${rejected.stdout}\n${rejected.stderr}`, /(?:W_FN_ARG_TYPE_MISMATCH|W_METHOD_ARG_TYPE_MISMATCH|W_FN_RETURN_TYPE_MISMATCH|E_CALL_ARGUMENT_UNPROVEN|E_ERASED_GENERIC_RELATION)/);
      assert.deepEqual(await readFile(snapshot), original);
      await assertRejectedArtifacts(output, name, /(?:W_FN_ARG_TYPE_MISMATCH|W_METHOD_ARG_TYPE_MISMATCH|W_FN_RETURN_TYPE_MISMATCH|E_CALL_ARGUMENT_UNPROVEN|E_ERASED_GENERIC_RELATION)/, mode[0] === "js");
    }
  }
  for (const [name, body, schema] of [
    ["nil", ["count", "nil"], "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)"],
    ["number", ["count", "1"], "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)"],
    ["function", ["count", ["fn", [], "1"]], "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)"],
    ["dynamic", ["count", "value"], "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Number)"],
    ["unbounded-generic", ["count", "value"], "quote $ :: 'Fn $ {} (:args $ [] 'T) (:return 'Number) (:generics $ [] 'T)"],
    ["explicit-open-inline", [["fn", ["value"], ["hint-fn", ["{}", [":args", ["[]", "'Dynamic"]], [":return", "'Number"]]], ["count", "value"]], ["[]", "1", "2"]],
      "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)"],
  ]) {
    await copyFile("tests/fixtures/count-contract.cirru", snapshot);
    run("edit", "def", "fix-command.main/rejected-count", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "rejected-count", name === "dynamic" || name === "unbounded-generic" ? ["value"] : [], body]));
    run("edit", "schema", "fix-command.main/rejected-count", "--input-format", "cirru", "--code", schema);
    run("edit", "def", "fix-command.main/main!", "--overwrite", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "main!", [], ["rejected-count", ...(name === "dynamic" ? [["parse-cirru-edn", "|1"]] : name === "unbounded-generic" ? ["1"] : [])], "&unit"]));
    const original = await readFile(snapshot);
    const rejected = spawnSync(binary, [snapshot, "fix", "--workflow", "strict", "--verify", "--format", "json"], options);
    if (rejected.error) throw rejected.error;
    assert.equal(rejected.status, 1, `${name}\n${rejected.stdout}\n${rejected.stderr}`);
    const report = JSON.parse(rejected.stdout);
    // Read the actual failed workflow, not the available-rule catalog.
    assert.equal(report.data.workflow.status, "failed");
    assert.ok(JSON.stringify(report.data.workflow).includes("W_GENERIC_WHERE_BOUND_MISMATCH"),
      `${name}: missing bound diagnostic\n${JSON.stringify(report.data.workflow)}`);
    assert.deepEqual(await readFile(snapshot), original);
  }
  // Same-name user functions return Bool, not a trusted core type predicate.
  for (const name of ["string?", "list?", "map?", "set?", "enum?", "struct?"]) {
    await copyFile("tests/fixtures/count-contract.cirru", snapshot);
    run("edit", "add-ns", "fix-command.fake");
    run("edit", "def", `fix-command.fake/${name}`, "--input-format", "cirru", "--code", "quote $ fn (value) true");
    run("edit", "schema", `fix-command.fake/${name}`, "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Bool)");
    // Calling the ordinary Bool-returning function remains legal. Only using
    // its short name as evidence for an open value must be rejected.
    run("eval", "--dep", `${project}/`, `assert= true $ fix-command.fake/${name} 1`);
    run("edit", "def", "fix-command.main/rejected-predicate", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "rejected-predicate", ["value"], ["if", [`fix-command.fake/${name}`, "value"], ["count", "value"], "0"]]));
    run("edit", "schema", "fix-command.main/rejected-predicate", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Number)");
    run("edit", "def", "fix-command.main/main!", "--overwrite", "--input-format", "cirru", "--code",
      "quote $ defn main! () (rejected-predicate 1) &unit");
    const original = await readFile(snapshot);
    for (const mode of [["--check-only"], [], ["js"], ["wasm"], ["wasi"]]) {
      const output = join(project, `predicate-rejected-${name}-${mode.join("-") || "native"}`);
      const rejected = spawnSync(binary, [snapshot, "--emit-path", output, ...mode], options);
      if (rejected.error) throw rejected.error;
      assert.equal(rejected.status, 1, `${name} ${mode.join(" ")}\n${rejected.stdout}\n${rejected.stderr}`);
      assert.match(`${rejected.stdout}\n${rejected.stderr}`, /W_GENERIC_WHERE_BOUND_MISMATCH/);
      assert.deepEqual(await readFile(snapshot), original);
      await assertRejectedArtifacts(output, name, /W_GENERIC_WHERE_BOUND_MISMATCH/);
    }
  }
  // A project may supply calcit.core instead of the embedded namespace.
  // Its source-defined guards must not gain proofs from the namespace name.
  for (const [name, answer, guardedBranch] of [["js-present?", "true", 1], ["js-nullish?", "false", 2]]) {
    await copyFile("src/cirru/calcit-core.cirru", snapshot);
    run("edit", "def", `calcit.core/${name}`, "--overwrite", "--input-format", "cirru", "--code", `quote $ defn ${name} (value) ${answer}`);
    run("edit", "add-ns", "calcit.predicate-origin");
    run("edit", "def", "calcit.predicate-origin/consume-present", "--input-format", "cirru", "--code",
      "quote $ defn consume-present (value) &unit");
    run("edit", "schema", "calcit.predicate-origin/consume-present", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'JsObject) (:return 'Unit)");
    const branches = ["&unit", "&unit"];
    branches[guardedBranch - 1] = ["consume-present", "value"];
    const hint = ["hint-fn", ["{}", [":args", ["[]", ["::", "'JsNullish", "'JsObject"]]], [":return", "'Unit"], [":features", ["#{}", ":js-ffi"]]]];
    run("edit", "def", "calcit.predicate-origin/rejected-guard", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "rejected-guard", ["value"], hint, ["if", [name, "value"], ...branches]]));
    run("edit", "schema", "calcit.predicate-origin/rejected-guard", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ [] $ :: 'JsNullish 'JsObject) (:return 'Unit) (:features $ #{} :js-ffi)");
    // Deliberately construct one host-nullability boundary for this rejection
    // fixture; do not rely on a mismatched Unit argument or lazy Fn reflection.
    run("edit", "def", "calcit.predicate-origin/main!", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "main!", [],
        ["hint-fn", ["{}", [":args", ["[]"]], [":return", "'Unit"], [":features", ["#{}", ":js-ffi"]]]],
        ["rejected-guard", ["unsafe-coerce", "&unit", ["::", "'JsNullish", "'JsObject"]]], "&unit"]));
    run("config", "set", "init-fn", "calcit.predicate-origin/main!");
    run("config", "set", "reload-fn", "calcit.predicate-origin/main!");
    const original = await readFile(snapshot);
    for (const mode of [["--check-only"], [], ["js"], ["wasm"], ["wasi"]]) {
      const output = join(project, `source-predicate-rejected-${name}-${mode.join("-") || "native"}`);
      const rejected = spawnSync(binary, [snapshot, "--emit-path", output, ...mode], options);
      if (rejected.error) throw rejected.error;
      assert.equal(rejected.status, 1, `${name} source core ${mode.join(" ")}\n${rejected.stderr}`);
      assert.match(`${rejected.stdout}\n${rejected.stderr}`, /W_FN_ARG_TYPE_MISMATCH/);
      assert.deepEqual(await readFile(snapshot), original);
      await assertRejectedArtifacts(output, name, /W_FN_ARG_TYPE_MISMATCH/);
    }
  }
  console.log("Known assertions and return/call contracts rejected before native/JS/WASM/WASI; native/JS positives, JS async adoption, scalar WASM assertions, value schema contracts, Diary boundaries, lowered trait return proofs, Countable count contracts and predicate origins passed");
} finally {
  await rm(project, { recursive: true, force: true });
}
