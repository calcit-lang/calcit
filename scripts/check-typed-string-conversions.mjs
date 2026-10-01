import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const snapshot = "calcit/test.cirru";

for (const [name, target] of [["to-tag", "Tag"], ["to-symbol", "Symbol"]]) {
  const query = JSON.parse(execFileSync(binary, [snapshot, "query", "def", `calcit.core/${name}`, "--format", "json"], { encoding: "utf8" }));
  assert.equal(query.data.id, `calcit.core/${name}`);
  assert.deepEqual(query.data.schema[2], [":return", `'${target}`]);
  assert.deepEqual(query.data.schema[3], [":args", ["[]", "'String"]]);
  assert.ok(query.data.doc.includes("String"), `${name} should be discoverable with its input contract`);
}

for (const [expression, expected] of [
  ["to-tag |ready", ":ready"],
  ["to-symbol |ready", "'ready"],
]) {
  const result = execFileSync(binary, [snapshot, "eval", expression], { encoding: "utf8" });
  assert.ok(result.includes(expected), `${expression} should produce ${expected} on native`);
}

for (const expression of ["to-tag 1", "to-tag ([] 1)", "to-symbol :ready", "to-symbol nil"]) {
  const result = spawnSync(binary, [snapshot, "eval", expression], { encoding: "utf8" });
  assert.notEqual(result.status, 0, `${expression} must fail strict checking`);
  assert.match(result.stderr, /W_FN_ARG_TYPE_MISMATCH/);
}

const stringConversion = JSON.parse(execFileSync(binary, [snapshot, "query", "def", "calcit.core/to-string", "--format", "json"], { encoding: "utf8" }));
assert.ok(JSON.stringify(stringConversion.data.schema).includes("ToString"), "to-string must require its trait bound");
const legacyStringConversion = JSON.parse(execFileSync(binary, [snapshot, "query", "def", "calcit.core/turn-str", "--format", "json"], { encoding: "utf8" }));
assert.ok(JSON.stringify(legacyStringConversion.data.schema).includes("ToString"), "turn-str must retain the same strict trait boundary");
assert.ok(legacyStringConversion.data.tags.includes("internal"), "the compatibility alias must not be recommended as a public API");
const legacyTurnString = JSON.parse(execFileSync(binary, [snapshot, "query", "def", "calcit.core/turn-string", "--format", "json"], { encoding: "utf8" }));
assert.ok(JSON.stringify(legacyTurnString.data.schema).includes("ToString"), "turn-string must expose the trait-bound wrapper contract");
assert.ok(legacyTurnString.data.tags.includes("internal"), "turn-string must remain a compatibility entry");
for (const expression of ["to-string ([] 1)", "to-string (&{} :a 1)", "to-string (#{} 1)", "to-string &unit", "to-string (fs:path |abc)", "to-string (json-parse |42)", "turn-str ([] 1)", "turn-str (&{} :a 1)", "turn-str &unit", "turn-str (json-parse |42)", "turn-string ([] 1)", "turn-string (&{} :a 1)", "turn-string &unit", "turn-string (json-parse |42)"]) {
  const result = spawnSync(binary, [snapshot, "eval", expression], { encoding: "utf8" });
  assert.notEqual(result.status, 0, `${expression} must fail strict trait-bound checking`);
  assert.match(result.stderr, /W_GENERIC_WHERE_BOUND_MISMATCH/);
}
for (const [source, expected] of [["nil", ""], ["true", "true"], [":ready", "ready"], ["0.5", "0.5"]]) {
  const native = execFileSync(binary, [snapshot, "eval", `to-string ${source}`], { encoding: "utf8" });
  assert.match(native, new RegExp(`: \\|${expected}$`, "m"), `native to-string should convert ${source}`);
}

const output = await mkdtemp(join(tmpdir(), "calcit-typed-string-conversions-"));
try {
  await symlink(resolve("node_modules"), join(output, "node_modules"), "dir");
  execFileSync(binary, ["--emit-path", output, snapshot, "js"], { stdio: "pipe" });
  const core = await import(pathToFileURL(join(output, "calcit.core.mjs")).href);
  const runtime = await import(pathToFileURL(resolve("lib/calcit.procs.mjs")).href);

  // Execute the same attached method assertions on native and generated JS.
  const coreSource = resolve("src/cirru/calcit-core.cirru");
  const source = JSON.parse(execFileSync(binary, ["cirru", "parse-edn", "--file", coreSource], {
    encoding: "utf8", maxBuffer: 16 * 1024 * 1024,
  }));
  const conversionTests = source[":files"]["'calcit.core"].defs["'to-string"].tests;
  const methodTests = ["scalar-method-contract", "wasm-scalar-method-contract"].map(name => {
    const test = conversionTests.find(test => test.name === name);
    assert.ok(test, `the ${name} definition test must exist`);
    return test;
  });
  const methodSnapshot = join(output, "scalar-methods.cirru");
  await copyFile(coreSource, methodSnapshot);
  const edit = (...args) => execFileSync(binary, [methodSnapshot, "edit", ...args], { stdio: "pipe" });
  edit("add-ns", "calcit.conversion-contracts");
  edit("def", "calcit.conversion-contracts/main!", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "main!", [], ...methodTests.map(test => test.code.__edn_quote), "&unit"]));
  edit("schema", "calcit.conversion-contracts/main!", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  const methodEntry = [methodSnapshot, "--init-fn", "calcit.conversion-contracts/main!",
    "--reload-fn", "calcit.conversion-contracts/main!"];
  execFileSync(binary, methodEntry, { stdio: "pipe" });
  const methodOutput = join(output, "methods-js");
  execFileSync(binary, [...methodEntry, "--emit-path", methodOutput, "js"], { stdio: "pipe" });
  // Each generated program owns its reachable builtin-impl registry.
  const methodModule = pathToFileURL(join(methodOutput, "calcit.conversion-contracts.mjs")).href;
  execFileSync(process.execPath, ["--input-type=module", "-e",
    `import * as fixture from ${JSON.stringify(methodModule)}; fixture.main_$x_();`], { stdio: "pipe" });
  console.log("Scalar method definition assertions passed on native and generated JS");

  // WASM cannot lower Symbol conversion; reuse the attached supported-subset AST.
  edit("def", "calcit.conversion-contracts/scalar-methods", "--input-format", "json-ast", "--code",
    JSON.stringify(["defwasm-export", "scalar-methods", [], methodTests[1].code.__edn_quote, "&unit"]));
  edit("schema", "calcit.conversion-contracts/scalar-methods", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  const methodWasmOutput = join(output, "methods-wasm");
  execFileSync(binary, ["wasm", methodSnapshot, "--init-fn", "calcit.conversion-contracts/scalar-methods",
    "--reload-fn", "calcit.conversion-contracts/scalar-methods", "--emit-path", methodWasmOutput], { stdio: "pipe" });
  const methodWasmModule = new WebAssembly.Module(await readFile(join(methodWasmOutput, "program.wasm")));
  const methodWasmImports = {};
  for (const item of WebAssembly.Module.imports(methodWasmModule)) {
    assert.equal(item.kind, "function");
    (methodWasmImports[item.module] ??= {})[item.name] = () => {
      throw new Error(`unexpected scalar-method host call: ${item.module}/${item.name}`);
    };
  }
  const methodWasmInstance = new WebAssembly.Instance(methodWasmModule, methodWasmImports);
  methodWasmInstance.exports["scalar-methods"]();
  console.log("The same supported scalar method assertions passed on actual WASM");

  const numberCases = [
    ["0.0000001", 0.0000001, "0.0000001"],
    ["-0.0000001", -0.0000001, "-0.0000001"],
    ["0.1", 0.1, "0.1"],
    ["0.5", 0.5, "0.5"],
    ["1000000000000000000000", 1e21, "1000000000000000000000"],
    ["0.000001", 1e-6, "0.000001"],
    ["9007199254740992", 2 ** 53, "9007199254740992"],
    ["9007199254740991", 2 ** 53 - 1, "9007199254740991"],
    ["9007199254740994", 2 ** 53 + 2, "9007199254740994"],
    ["864310392341871.2", 864310392341871.2, "864310392341871.2"],
    ["-993946982230940.2", -993946982230940.2, "-993946982230940.2"],
    ["1.7976931348623157e308", Number.MAX_VALUE, "17976931348623157" + "0".repeat(292)],
    ["2.2250738585072014e-308", 2.2250738585072014e-308, `0.${"0".repeat(307)}22250738585072014`],
    ["-0", -0, "-0"],
    ["5e-324", Number.MIN_VALUE, `0.${"0".repeat(323)}5`],
    ["-5e-324", -Number.MIN_VALUE, `-0.${"0".repeat(323)}5`],
    ["1e309", Infinity, "inf"],
    ["-1e309", -Infinity, "-inf"],
    ["nan", NaN, "NaN"],
  ];
  for (const [source, value, expected] of numberCases) {
    const native = execFileSync(binary, [snapshot, "eval", `turn-string ${source}`], { encoding: "utf8" });
    const nativeValue = native.match(/^took [^\r\n]*: \|([^\r\n]*)$/m)?.[1];
    assert.equal(nativeValue, expected, `native Number formatting should produce ${expected}`);
    assert.equal(runtime.turn_string(value), expected, `generated JS should match native turn-string for ${source}`);
  }
  const tag = core.to_tag("ready");
  const symbol = core.to_symbol("ready");
  assert.ok(tag instanceof runtime.CalcitTag);
  assert.equal(tag.value, "ready");
  assert.ok(symbol instanceof runtime.CalcitSymbol);
  assert.equal(symbol.value, "ready");
  for (const [value, expected] of [
    [null, ""],
    [false, "false"],
    [true, "true"],
    ["ready", "ready"],
    [tag, "ready"],
    [symbol, "ready"],
    [42, "42"],
    [0.0000001, "0.0000001"],
  ]) {
    assert.equal(core.to_string(value), expected, `generated JS to-string should convert ${String(value)}`);
    assert.equal(core.turn_str(value), expected, `generated JS compatibility alias should convert ${String(value)}`);
    assert.equal(core.turn_string(value), expected, `generated JS turn-string compatibility wrapper should convert ${String(value)}`);
  }
  const traitsOutput = join(output, "traits-js");
  execFileSync(binary, ["--emit-path", traitsOutput, "calcit/test-traits.cirru", "js"], { stdio: "pipe" });
  const traits = await import(pathToFileURL(join(traitsOutput, "test-traits.main.mjs")).href);
  assert.equal(traits.test_turn_str_custom(), "Person:Alice", "generated JS turn-str must dispatch a nominal ToString implementation");
  const list = new runtime.CalcitSliceList([1, "a"]);
  assert.equal(runtime.format_to_lisp(list), "(1 |a)", "JS Lisp formatting must match native for a simple String in a List");
  assert.equal(runtime.to_lispy_string(list), "([] 1 |a)", "diagnostic List display is not Lisp formatting");
  assert.equal(runtime.format_to_lisp("a b"), '"|a b"', "non-simple Strings still need quoted Lisp formatting");
  assert.equal(runtime.format_to_lisp("a_b"), '"|a_b"', "non-simple ASCII punctuation follows native Lisp formatting");
  assert.equal(runtime.format_to_lisp("中文"), "|中文", "CJK Strings keep the existing readable form");
  assert.equal(runtime.format_to_lisp("中文_"), '"|中文_"', "mixed CJK Strings follow the native per-character rule");

  const wasmSnapshot = join(output, "unsupported-tag-conversion.cirru");
  await copyFile("calcit/test-wasm.cirru", wasmSnapshot);
  execFileSync(binary, [wasmSnapshot, "edit", "def", "test-wasm.main/test-to-tag", "--input-format", "cirru", "--code", "quote $ defwasm-export test-to-tag () (if (&= (to-tag |ready) :ready) 1 0)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "schema", "test-wasm.main/test-to-tag", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'Number) (:args $ [])"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "def", "test-wasm.main/test-to-string-frac", "--input-format", "cirru", "--code", "quote $ defwasm-export test-to-string-frac () (if (&= (to-string 0.5) |0.5) 1 0)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "schema", "test-wasm.main/test-to-string-frac", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'Number) (:args $ [])"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "def", "test-wasm.main/test-to-string-value", "--input-format", "cirru", "--code", "quote $ defwasm-export test-to-string-value (x) (to-string x)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "schema", "test-wasm.main/test-to-string-value", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'String) (:args $ [] 'Number)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "def", "test-wasm.main/test-turn-string-runtime", "--input-format", "cirru", "--code", "quote $ defwasm-export test-turn-string-runtime (x) (if (&= (turn-string x) |42) 1 0)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "schema", "test-wasm.main/test-turn-string-runtime", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'Number) (:args $ [] 'Number)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "def", "test-wasm.main/test-turn-str-runtime", "--input-format", "cirru", "--code", "quote $ defwasm-export test-turn-str-runtime (x) (if (&= (turn-str x) |42) 1 0)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "schema", "test-wasm.main/test-turn-str-runtime", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'Number) (:args $ [] 'Number)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "def", "test-wasm.main/test-turn-string-zero", "--input-format", "cirru", "--code", "quote $ defwasm-export test-turn-string-zero (x) (if (&= (turn-string x) |0) 1 0)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "schema", "test-wasm.main/test-turn-string-zero", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'Number) (:args $ [] 'Number)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "def", "test-wasm.main/test-turn-string-safe-limit", "--input-format", "cirru", "--code", "quote $ defwasm-export test-turn-string-safe-limit (x) (if (&= (turn-string x) |9007199254740992) 1 0)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "schema", "test-wasm.main/test-turn-string-safe-limit", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'Number) (:args $ [] 'Number)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "def", "test-wasm.main/test-turn-string-negative-limit", "--input-format", "cirru", "--code", "quote $ defwasm-export test-turn-string-negative-limit (x) (if (&= (turn-string x) |-9007199254740992) 1 0)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "schema", "test-wasm.main/test-turn-string-negative-limit", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'Number) (:args $ [] 'Number)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "def", "test-wasm.main/test-turn-string-value", "--input-format", "cirru", "--code", "quote $ defwasm-export test-turn-string-value (x) (turn-string x)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "schema", "test-wasm.main/test-turn-string-value", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'String) (:args $ [] 'Number)"], { stdio: "pipe" });
  const wasmJsOutput = join(output, "wasm-js");
  execFileSync(binary, ["--emit-path", wasmJsOutput, wasmSnapshot, "js"], { stdio: "pipe" });
  const wasmJs = await import(pathToFileURL(join(wasmJsOutput, "test-wasm.main.mjs")).href);
  assert.equal(wasmJs.test_to_string_number(42), 1, "generated JS must run the generic ToString definition");
  assert.equal(wasmJs.test_turn_str_runtime(42), 1, "generated JS compatibility alias must run the trait-bound ToString definition");
  assert.equal(wasmJs.test_to_string_scalars(), 1, "generated JS must preserve Nil, Bool, String, and Tag trait text");
  assert.equal(wasmJs.test_custom_trait_score(42), 1, "generated JS must select the same nominal trait implementation");
  assert.equal(wasmJs.test_qualified_symbol_helper(42), 1, "generated JS must keep the local helper despite a same-named definition");
  const wasm = spawnSync(binary, ["wasm", wasmSnapshot, "--emit-path", output], { encoding: "utf8" });
  assert.equal(wasm.status, 0, `WASM should preserve unrelated exports\n${wasm.stdout}\n${wasm.stderr}`);
  assert.match(wasm.stderr, /trapping unsupported dependency calcit\.core\/to-tag: E_WASM_TAG_CONVERSION/);
  const wasmModule = new WebAssembly.Module(await readFile(join(output, "program.wasm")));
  const imports = {};
  for (const item of WebAssembly.Module.imports(wasmModule)) {
    assert.equal(item.kind, "function");
    (imports[item.module] ??= {})[item.name] = () => 0;
  }
  const instance = new WebAssembly.Instance(wasmModule, imports);
  const readWasmString = (pointer) => {
    const memory = instance.exports.memory.buffer;
    const length = new DataView(memory).getFloat64(pointer, true);
    assert.ok(Number.isSafeInteger(length) && length >= 0, "WASM string length must be a nonnegative safe integer");
    return new TextDecoder().decode(new Uint8Array(memory, pointer + 8, length));
  };
  assert.throws(() => instance.exports["test-to-tag"](), WebAssembly.RuntimeError, "WASM must not silently return a String as Tag");
  assert.equal(instance.exports["test-to-string-frac"](), 1, "WASM must lower Number to-string through its trait implementation");
  assert.equal(instance.exports["test-to-string-number"](42), 1, "WASM must specialize the generic ToString trait call for runtime Number arguments");
  assert.equal(instance.exports["test-turn-str-runtime"](42), 1, "WASM compatibility alias must specialize the ToString trait call for runtime Number arguments");
  assert.equal(instance.exports["test-to-string-scalars"](), 1, "WASM must preserve Nil, Bool, String, and Tag trait text");
  assert.equal(instance.exports["test-custom-trait-score"](42), 1, "WASM must select a user-defined nominal trait implementation for a runtime argument");
  assert.equal(instance.exports["test-qualified-symbol-helper"](42), 1, "WASM must resolve a local symbol by namespace before a same-named helper");
  assert.equal(instance.exports["test-turn-string-runtime"](42), 1, "WASM must retain exact integer formatting");
  assert.equal(instance.exports["test-turn-string-zero"](0), 1, "WASM must distinguish Number zero from nil");
  assert.equal(instance.exports["test-turn-string-safe-limit"](2 ** 53), 1, "WASM must format the safe integer boundary");
  assert.equal(instance.exports["test-turn-string-negative-limit"](-(2 ** 53)), 1, "WASM must format the negative safe integer boundary");
  for (const [value, expected] of [[0, "0"], [42, "42"], [-(2 ** 53), "-9007199254740992"], ...numberCases.map(([, value, expected]) => [value, expected])]) {
    assert.equal(readWasmString(instance.exports["test-turn-string-value"](value)), expected,
      `WASM must return exact UTF-8 number text for ${value}`);
    assert.equal(readWasmString(instance.exports["test-to-string-value"](value)), expected,
      `WASM trait to-string must return exact UTF-8 number text for ${value}`);
    assert.equal(wasmJs.test_to_string_method_value(value), expected,
      `generated JS ordinary method must preserve runtime Number text for ${value}`);
    assert.equal(readWasmString(instance.exports["test-to-string-method-value"](value)), expected,
      `WASM ordinary method must return exact UTF-8 number text for ${value}`);
  }
  const random = new DataView(new ArrayBuffer(8));
  let seed = 0x1234abcd;
  for (let i = 0; i < 1024; i++) {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    random.setUint32(0, seed, true);
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    random.setUint32(4, seed, true);
    const value = random.getFloat64(0, true);
    assert.equal(readWasmString(instance.exports["test-turn-string-value"](value)), runtime.turn_string(value),
      `WASM must match JS for f64 bits ${random.getBigUint64(0, true).toString(16)}`);
    assert.equal(readWasmString(instance.exports["test-to-string-method-value"](value)), wasmJs.test_to_string_method_value(value),
      `ordinary method Number text must match across JS/WASM for f64 bits ${random.getBigUint64(0, true).toString(16)}`);
  }

  const openTraitSnapshot = join(output, "open-trait.cirru");
  await copyFile("calcit/test-wasm.cirru", openTraitSnapshot);
  execFileSync(binary, [openTraitSnapshot, "edit", "def", "test-wasm.main/test-to-string-open", "--input-format", "cirru", "--code", "quote $ defwasm-export test-to-string-open (x) (to-string x)"], { stdio: "pipe" });
  execFileSync(binary, [openTraitSnapshot, "edit", "schema", "test-wasm.main/test-to-string-open", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'String) (:args $ [] 'T) (:generics $ [] 'T) (:where $ {} $ 'T 'ToString)"], { stdio: "pipe" });
  const openTrait = spawnSync(binary, ["wasm", openTraitSnapshot, "--emit-path", join(output, "open-trait-wasm")], { encoding: "utf8" });
  assert.notEqual(openTrait.status, 0, "a generic WASM export cannot choose a trait implementation without concrete type evidence");
  assert.match(openTrait.stderr, /E_WASM_TRAIT_TYPE_EVIDENCE:.*calcit\.core\/to-string/);

  const firstClassSnapshot = join(output, "first-class-trait.cirru");
  await copyFile("calcit/test-wasm.cirru", firstClassSnapshot);
  execFileSync(binary, [firstClassSnapshot, "edit", "def", "test-wasm.main/test-to-string-first-class", "--input-format", "cirru", "--code", "quote $ defwasm-export test-to-string-first-class () (let ((f to-string)) (if (&= (f 42) |42) 1 0))"], { stdio: "pipe" });
  execFileSync(binary, [firstClassSnapshot, "edit", "schema", "test-wasm.main/test-to-string-first-class", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'Number) (:args $ [])"], { stdio: "pipe" });
  const firstClass = spawnSync(binary, ["wasm", firstClassSnapshot, "--emit-path", join(output, "first-class-trait-wasm")], { encoding: "utf8" });
  assert.notEqual(firstClass.status, 0, "a first-class generic trait function cannot bypass specialization");
  assert.match(firstClass.stderr, /E_WASM_TRAIT_TYPE_EVIDENCE:.*first-class function/);

  const spreadSnapshot = join(output, "spread-trait.cirru");
  await copyFile("calcit/test-wasm.cirru", spreadSnapshot);
  execFileSync(binary, [spreadSnapshot, "edit", "def", "test-wasm.main/test-to-string-spread", "--input-format", "cirru", "--code", "quote $ defwasm-export test-to-string-spread () (if (&= (to-string 42 & $ []) |42) 1 0)"], { stdio: "pipe" });
  execFileSync(binary, [spreadSnapshot, "edit", "schema", "test-wasm.main/test-to-string-spread", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'Number) (:args $ [])"], { stdio: "pipe" });
  const spread = spawnSync(binary, ["wasm", spreadSnapshot, "--emit-path", join(output, "spread-trait-wasm")], { encoding: "utf8" });
  assert.notEqual(spread.status, 0, "a spread call cannot bypass generic trait specialization");
  // Fixed-arity spread uncertainty is rejected by the shared preprocessor,
  // before WASM-specific specialization can run.
  assert.match(spread.stderr, /expected 1 args in to-string.*got spreading form/);
  assert.match(spread.stderr, /test-wasm\.main\/test-to-string-spread/);
  assert.match(spread.stderr, /Found 1 warnings during preprocessing/);
} finally {
  await rm(output, { recursive: true, force: true });
}
