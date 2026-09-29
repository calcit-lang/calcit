import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFile, mkdtemp, rm } from "node:fs/promises";
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
} finally {
  await rm(output, { recursive: true, force: true });
}
