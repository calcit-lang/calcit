import { spawnSync } from "node:child_process";

const executable = "target/release/calcit";
const fixture = "calcit/bench-literal-paths.cirru";
const samples = 3;
const cases = [
  ["typed-read", "bench-literal-paths.main/bench-read-typed!", "200000"],
  ["dynamic-read", "bench-literal-paths.main/bench-read-dynamic!", "200000"],
  ["typed-write", "bench-literal-paths.main/bench-write-typed!", "1"],
  ["dynamic-write", "bench-literal-paths.main/bench-write-dynamic!", "1"],
];

for (const [label, initFn, expected] of cases) {
  runOnce(initFn, expected);
  const elapsed = Array.from({ length: samples }, () => runOnce(initFn, expected)).sort((a, b) => a - b);
  console.log(`${label}: median=${elapsed[1].toFixed(2)}ms samples=${elapsed.map((x) => x.toFixed(2)).join(",")}`);
}

/** Run one native benchmark process, verify its output, and return Calcit's elapsed time. */
function runOnce(initFn, expected) {
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
