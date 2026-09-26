import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const run = (...args) => execFileSync(binary, args, { encoding: "utf8", stdio: "pipe", timeout: 60000 });
const source = resolve("calcit/test-helper-inference.cirru");
const fixture = await mkdtemp(join(tmpdir(), "calcit-helper-inference-"));
try {
  const snapshot = join(fixture, "calcit.cirru");
  await copyFile(source, snapshot);
  await symlink(resolve("node_modules"), join(fixture, "node_modules"), "dir");
  run(snapshot, "test", "--tag", "helper-inference", "--require-match");
  const data = JSON.parse(run("cirru", "parse-edn", "--file", snapshot));
  const tests = data[":files"]["'app.main"].defs["'main!"].tests;
  assert.equal(tests.length, 1, "the shared semantic contract must be present");
  run(snapshot, "edit", "def", "app.main/run-tests", "--input-format", "json-ast", "--code",
    JSON.stringify(["defwasm-export", "run-tests", [], tests[0].code.__edn_quote, "&unit"]));
  run(snapshot, "edit", "schema", "app.main/run-tests", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  const entry = [snapshot, "--init-fn", "app.main/run-tests", "--reload-fn", "app.main/run-tests"];
  const output = join(fixture, "js-out");
  run(...entry, "--emit-path", output, "js");
  const compiled = await import(pathToFileURL(join(output, "app.main.mjs")).href);
  compiled.run_tests();
  run("wasm", ...entry, "--emit-path", fixture);
  const module = new WebAssembly.Module(await readFile(join(fixture, "program.wasm")));
  const imports = {};
  for (const { module: ns, name } of WebAssembly.Module.imports(module)) {
    (imports[ns] ??= {})[name] = () => { throw new Error(`Unexpected host call: ${ns}.${name}`); };
  }
  new WebAssembly.Instance(module, imports).exports["run-tests"]();

  // Queries consume the same compiled contract and must not write it to source.
  const before = await readFile(snapshot, "utf8");
  const query = (...args) => JSON.parse(run(snapshot, "query", ...args, "--format", "json"));
  const type = query("type-at", "app.main/helper-count", "--path", "code");
  const context = query("context", "app.main/helper-count");
  assert.match(type.data.inferred_type, /'Number/);
  assert.equal(type.data.confidence, "exact");
  assert.equal(context.data.schema, null);
  assert.equal(context.data.inferred_schema, type.data.inferred_type);
  assert.ok(context.diagnostics.some((d) => d.code === "I_SCHEMA_INFERRED"));
  assert.ok(!context.diagnostics.some((d) => d.code === "W_TYPE_COVERAGE_NONE" || d.code === "W_DYNAMIC_TYPE_UNRESOLVED"));
  assert.equal(await readFile(snapshot, "utf8"), before);

  // Compile failures belong to the CLI boundary, not runtime try assertions.
  const failures = [
    ["unproved input", "defn bad (x) x"],
    ["mixed return", "defn bad () $ if (= 3 $ helper-number) 1 |text"],
    ["recursive helper", "defn bad ()\n  bad\n  , 1"],
    ["mutual recursion", "defn bad ()\n  cycle-peer\n  , 1", undefined, "defn cycle-peer ()\n  bad\n  , 2"],
    ["recur helper", "defn bad ()\n  recur\n  , 1"],
    ["qualified recur helper", "defn bad ()\n  calcit.core/recur\n  , 1"],
    ["WASM export boundary", "defwasm-export bad () 1"],
    ["WASM import boundary", "defwasm-import bad () |env |value"],
    ["conflicting use", "defn bad () $ &str:count $ helper-number"],
    ["explicit Dynamic", "defn bad () 1", "quote $ :: 'Dynamic"],
    ["open container", "defn bad () $ []"],
  ];
  for (const [label, code, schema, peer] of failures) {
    const bad = join(fixture, "bad.cirru");
    await copyFile(snapshot, bad);
    run(bad, "edit", "def", "app.main/bad", "--input-format", "cirru", "--code", `quote $ ${code}`);
    if (peer) run(bad, "edit", "def", "app.main/cycle-peer", "--input-format", "cirru", "--code", `quote $ ${peer}`);
    if (schema) run(bad, "edit", "schema", "app.main/bad", "--input-format", "cirru", "--code", schema);
    const result = spawnSync(binary, [bad, "--init-fn", "app.main/bad", "--check-only"], { encoding: "utf8", timeout: 60000 });
    assert.ifError(result.error);
    assert.equal(result.status, 1, `${label} must fail closed:\n${result.stdout}\n${result.stderr}`);
    assert.match(result.stderr, /E_WHOLE_DYNAMIC_PUBLIC_SCHEMA|W_PROC_ARG_TYPE_MISMATCH|expects type|warnings during preprocessing/);
  }
  console.log("Closed helper inference: shared native/JS/WASM tests, query evidence and rejected boundaries passed");
} finally {
  await rm(fixture, { recursive: true, force: true });
}
