import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const wasmtime = process.env.WASMTIME_CLI ?? "wasmtime";
const output = await mkdtemp(join(tmpdir(), "calcit-wasi-number-text-"));
const snapshot = join(output, "calcit.cirru");
const artifact = join(output, "component");
const cases = [
  ["0.5", "0.5"],
  ["0.1", "0.1"],
  ["1e21", "1000000000000000000000"],
  ["5e-324", `0.${"0".repeat(323)}5`],
  ["-5e-324", `-0.${"0".repeat(323)}5`],
];
try {
  await copyFile("examples/wasi-command/calcit.cirru", snapshot);
  execFileSync(binary, ["docs", "agents", "--contract"], { stdio: "pipe" });
  const config = execFileSync(binary, [snapshot, "query", "config", "--format", "json"], { encoding: "utf8" });
  assert.equal(JSON.parse(config).data.entries[0].init_fn, "app.main/main!");
  const body = `quote $ defn main! () ${cases.flatMap(([source]) => [`(println $ to-string ${source})`, `(println $ turn-string ${source})`]).join(" ")} &unit`;
  execFileSync(binary, [snapshot, "edit", "def", "app.main/main!", "--overwrite", "--input-format", "cirru", "--code", body], { stdio: "pipe" });
  execFileSync(binary, ["wasi", snapshot, "--emit-path", artifact], { stdio: "pipe" });
  const result = execFileSync(wasmtime, [
    "run", "-W", "component-model-more-async-builtins=y,component-model-async-stackful=y",
    join(artifact, "program.wasm"),
  ], { encoding: "utf8" });
  assert.deepEqual(result.trimEnd().split("\n"), cases.flatMap(([, expected]) => [expected, expected]));

  for (const [name, expression, diagnostic] of [
    ["quote", "quote $ + 1 2", /unsupported runtime quote value in WASM/],
    ["format", "format-to-lisp 42", /unsupported runtime format-to-lisp in WASM/],
  ]) {
    execFileSync(binary, [snapshot, "edit", "def", "app.main/main!", "--overwrite", "--input-format", "cirru",
      "--code", `quote $ defn main! () (${expression}) &unit`], { stdio: "pipe" });
    const rejectedOutput = join(output, `rejected-${name}`);
    const rejected = spawnSync(binary, ["wasi", snapshot, "--emit-path", rejectedOutput], { encoding: "utf8", timeout: 60000 });
    assert.equal(rejected.error, undefined);
    assert.notEqual(rejected.status, 0, "unsupported runtime values must fail before WASI artifact emission");
    assert.match(rejected.stderr, diagnostic);
    await assert.rejects(readFile(join(rejectedOutput, "program.wasm")), { code: "ENOENT" });
  }

  // The static quoted expression used by assertion messages remains supported.
  execFileSync(binary, [snapshot, "edit", "def", "app.main/main!", "--overwrite", "--input-format", "cirru",
    "--code", "quote $ defn main! () (assert= 1 2) &unit"], { stdio: "pipe" });
  const failureOutput = join(output, "failed-assertion");
  execFileSync(binary, ["wasi", snapshot, "--emit-path", failureOutput], { stdio: "pipe" });
  const failure = spawnSync(wasmtime, ["run", "-W", "component-model-more-async-builtins=y,component-model-async-stackful=y",
    join(failureOutput, "program.wasm")], { encoding: "utf8", timeout: 60000 });
  assert.equal(failure.error, undefined);
  assert.notEqual(failure.status, 0, "a failed assertion must not turn into successful zero execution");
  assert.match(failure.stderr, /assert|Assertion/);
} finally {
  await rm(output, { recursive: true, force: true });
}
