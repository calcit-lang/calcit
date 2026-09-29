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
for (const expression of ["to-string ([] 1)", "to-string (&{} :a 1)", "to-string (#{} 1)", "to-string &unit", "to-string (fs:path |abc)", "to-string (json-parse |42)"]) {
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
  for (const [source, value, expected] of [
    ["0.0000001", 0.0000001, "0.0000001"],
    ["-0.0000001", -0.0000001, "-0.0000001"],
    ["1000000000000000000000", 1e21, "1000000000000000000000"],
    ["-0", -0, "-0"],
    ["5e-324", Number.MIN_VALUE, `0.${"0".repeat(323)}5`],
    ["1e309", Infinity, "inf"],
    ["-1e309", -Infinity, "-inf"],
    ["nan", NaN, "NaN"],
  ]) {
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
  }

  const wasmSnapshot = join(output, "unsupported-tag-conversion.cirru");
  await copyFile("calcit/test-wasm.cirru", wasmSnapshot);
  execFileSync(binary, [wasmSnapshot, "edit", "def", "test-wasm.main/test-to-tag", "--input-format", "cirru", "--code", "quote $ defwasm-export test-to-tag () (if (&= (to-tag |ready) :ready) 1 0)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "schema", "test-wasm.main/test-to-tag", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'Number) (:args $ [])"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "def", "test-wasm.main/test-to-string-frac", "--input-format", "cirru", "--code", "quote $ defwasm-export test-to-string-frac () (if (&= (to-string 0.5) |0.5) 1 0)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "schema", "test-wasm.main/test-to-string-frac", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'Number) (:args $ [])"], { stdio: "pipe" });
  const wasm = spawnSync(binary, ["wasm", wasmSnapshot, "--emit-path", output], { encoding: "utf8" });
  assert.equal(wasm.status, 0, "WASM should preserve unrelated exports");
  assert.match(wasm.stderr, /trapping unsupported dependency calcit\.core\/to-tag: E_WASM_TAG_CONVERSION/);
  assert.match(wasm.stderr, /trapping unsupported dependency calcit\.core\/to-string/);
  const wasmModule = new WebAssembly.Module(await readFile(join(output, "program.wasm")));
  const imports = {};
  for (const item of WebAssembly.Module.imports(wasmModule)) {
    assert.equal(item.kind, "function");
    (imports[item.module] ??= {})[item.name] = () => 0;
  }
  const instance = new WebAssembly.Instance(wasmModule, imports);
  assert.throws(() => instance.exports["test-to-tag"](), WebAssembly.RuntimeError, "WASM must not silently return a String as Tag");
  assert.throws(() => instance.exports["test-to-string-frac"](), WebAssembly.RuntimeError, "WASM must not silently misformat fractional Numbers");
} finally {
  await rm(output, { recursive: true, force: true });
}
