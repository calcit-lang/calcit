import { spawnSync } from "node:child_process";

const executable = process.argv[2] ?? "target/release/calcit";
const fixture = "calcit/fibo.cirru";
const iterations = 500_000;
const expected = String(Array.from({ length: iterations }, (_, index) => (index + 1) % 97).reduce((sum, value) => sum + value, 0));
const samples = Number(process.argv[3] ?? 3);
const cases = [
  ["typed-method", "app.main/bench-rem-typed!"],
  ["dynamic-method", "app.main/bench-rem-dynamic!"],
  ["direct-proc", "app.main/bench-rem-direct!"],
];

for (const [label, initFn] of cases) {
  runOnce(initFn);
  const elapsed = Array.from({ length: samples }, () => runOnce(initFn)).sort((a, b) => a - b);
  const median = elapsed[Math.floor(elapsed.length / 2)];
  console.log(`${label}: median=${median.toFixed(2)}ms samples=${elapsed.map((x) => x.toFixed(2)).join(",")}`);
}

/** Run one isolated native sample, verify its result, and return Calcit's elapsed time. */
function runOnce(initFn) {
  const result = spawnSync(executable, ["--init-fn", initFn, fixture], { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`native benchmark failed for ${initFn}\n${result.stdout}\n${result.stderr}`);
  }
  if (!result.stdout.split("\n").some((line) => line.trim() === expected)) {
    throw new Error(`native benchmark returned an unexpected result for ${initFn}\n${result.stdout}`);
  }
  const matched = result.stdout.match(/took ([\d.]+)ms:/);
  if (matched == null) {
    throw new Error(`native benchmark did not report elapsed time for ${initFn}\n${result.stdout}`);
  }
  return Number(matched[1]);
}
