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

const output = await mkdtemp(join(tmpdir(), "calcit-typed-string-conversions-"));
try {
  await symlink(resolve("node_modules"), join(output, "node_modules"), "dir");
  execFileSync(binary, ["--emit-path", output, snapshot, "js"], { stdio: "pipe" });
  const core = await import(pathToFileURL(join(output, "calcit.core.mjs")).href);
  const runtime = await import(pathToFileURL(resolve("lib/calcit.procs.mjs")).href);
  const tag = core.to_tag("ready");
  const symbol = core.to_symbol("ready");
  assert.ok(tag instanceof runtime.CalcitTag);
  assert.equal(tag.value, "ready");
  assert.ok(symbol instanceof runtime.CalcitSymbol);
  assert.equal(symbol.value, "ready");

  const wasmSnapshot = join(output, "unsupported-tag-conversion.cirru");
  await copyFile("calcit/test-wasm.cirru", wasmSnapshot);
  execFileSync(binary, [wasmSnapshot, "edit", "def", "test-wasm.main/test-to-tag", "--input-format", "cirru", "--code", "quote $ defwasm-export test-to-tag () (if (&= (to-tag |ready) :ready) 1 0)"], { stdio: "pipe" });
  execFileSync(binary, [wasmSnapshot, "edit", "schema", "test-wasm.main/test-to-tag", "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:return 'Number) (:args $ [])"], { stdio: "pipe" });
  const wasm = spawnSync(binary, ["wasm", wasmSnapshot, "--emit-path", output], { encoding: "utf8" });
  assert.equal(wasm.status, 0, "WASM should preserve unrelated exports");
  assert.match(wasm.stderr, /trapping unsupported dependency calcit\.core\/to-tag: E_WASM_TAG_CONVERSION/);
  const wasmModule = new WebAssembly.Module(await readFile(join(output, "program.wasm")));
  const imports = {};
  for (const item of WebAssembly.Module.imports(wasmModule)) {
    assert.equal(item.kind, "function");
    (imports[item.module] ??= {})[item.name] = () => 0;
  }
  const instance = new WebAssembly.Instance(wasmModule, imports);
  assert.throws(() => instance.exports["test-to-tag"](), WebAssembly.RuntimeError, "WASM must not silently return a String as Tag");
} finally {
  await rm(output, { recursive: true, force: true });
}
