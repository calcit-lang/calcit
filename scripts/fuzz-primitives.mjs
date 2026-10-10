#!/usr/bin/env node
// Differential random testing of core primitives across backends (#1561).
//
// Generates seeded calls of numeric, string and list primitives with edge inputs
// (NaN, ±inf, -0, i32 and safe-integer boundaries, non-ASCII text, empty values,
// out-of-range indices), replays them with scripts/run-core-tests.mjs and compares
// each backend with native: the printed result when native returns, or a failure
// (error or trap) when native raises. Outputs go through `turn-string`, the number
// text every backend shares (docs/data/number.md).
//
// Usage:
//   node scripts/fuzz-primitives.mjs [--seed n] [--cases n] [--backend native,js,wasm]
//
// `--cases` is the number of calls per primitive. A failure prints the seed, the
// call and every backend's result; rerunning with the same seed reproduces it.
// Differences already tracked by an issue are listed as known and do not fail.

import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { writeCirruCode } from "@cirru/writer.ts";

const options = { seed: 1, cases: 6, backends: "native,js,wasm" };
for (let i = 2; i < process.argv.length; i++) {
  const flag = process.argv[i];
  const value = process.argv[++i];
  if (value === undefined) throw new Error(`${flag} expects a value`);
  if (flag === "--seed") options.seed = Number(value);
  else if (flag === "--cases") options.cases = Number(value);
  else if (flag === "--backend") options.backends = value;
  else throw new Error(`unknown argument: ${flag}`);
}
if (!Number.isSafeInteger(options.seed) || !Number.isSafeInteger(options.cases) || options.cases < 1) {
  throw new Error("--seed and --cases expect integers, --cases at least 1");
}

// --- inputs ----------------------------------------------------------------

const nan = ["&/", "0", "0"];
const inf = ["&/", "1", "0"];
const negInf = ["&/", "-1", "0"];
const negZero = ["&*", "-1", "0"];
const numbers = [
  "0", negZero, "1", "-1", "2", "3", "0.5", "1.5", "-2.5", "0.1", "31", "32", "255",
  "2147483647", "2147483648", "-2147483649", "4294967296", "9007199254740993", "1e21", "1e-7",
  nan, inf, negInf,
];
const indices = ["0", "1", "2", "3", "5", "100", "-1", "1.5", nan];
const strings = ["|", "|a", "|abc", "|aaa", "|a,b", "|中文", "|😀a", "|a😀b", "| a "];
const numberLists = [["[]", "1", "2", "3"], ["[]", "1.5", nan, negZero]];
// An empty literal has no element type, so it only goes where the element is unused.
const lists = [["[]"], ...numberLists];

// Each primitive declares argument pools and how its result prints.
const primitives = [
  ["&+", [numbers, numbers], "number"],
  ["&-", [numbers, numbers], "number"],
  ["&*", [numbers, numbers], "number"],
  ["&/", [numbers, numbers], "number"],
  ["round", [numbers], "number"],
  ["floor", [numbers], "number"],
  ["ceil", [numbers], "number"],
  ["sqrt", [numbers], "number"],
  ["pow", [numbers, numbers], "number"],
  ["&number:rem", [numbers, numbers], "number"],
  ["&number:fract", [numbers], "number"],
  ["integer?", [numbers], "bool"],
  ["&<", [numbers, numbers], "bool"],
  ["&>", [numbers, numbers], "bool"],
  ["&compare", [numbers, numbers], "number"],
  ["bit-and", [numbers, numbers], "number"],
  ["bit-or", [numbers, numbers], "number"],
  ["bit-xor", [numbers, numbers], "number"],
  ["bit-shl", [numbers, indices], "number"],
  ["bit-shr", [numbers, indices], "number"],
  ["bit-not", [numbers], "number"],
  ["turn-string", [numbers], "string"],
  ["&str:count", [strings], "number"],
  ["&str:utf8-byte-count", [strings], "number"],
  ["&str:slice", [strings, indices, indices], "string"],
  ["&str:find-index", [strings, strings], "number"],
  ["&str:includes?", [strings, strings], "bool"],
  ["starts-with?", [strings, strings], "bool"],
  ["ends-with?", [strings, strings], "bool"],
  ["&str:pad-left", [strings, indices, strings], "string"],
  ["&str:pad-right", [strings, indices, strings], "string"],
  ["&str:compare", [strings, strings], "number"],
  ["trim", [strings], "string"],
  ["get-char-code", [strings], "number"],
  ["&list:count", [lists], "number"],
  ["&list:nth", [numberLists, indices], "number"],
  ["&list:slice", [lists, indices, indices], "count"],
  ["&list:contains?", [lists, indices], "bool"],
  ["&list:includes?", [lists, numbers], "bool"],
];

// The printed text of a result; `turn-string` is the number text every backend shares.
const render = (kind, call) => {
  switch (kind) {
    case "number": return ["turn-string", call];
    case "bool": return ["if", call, "|true", "|false"];
    // A marker keeps an empty result visible in every backend's trace.
    case "string": return ["&str:concat", "|=", call];
    case "count": return ["turn-string", ["&list:count", call]];
    default: throw new Error(`unknown result kind ${kind}`);
  }
};

// Differences tracked by an issue: [primitive, backend, issue].
const known = [
  ["round", "js", "#1847"], ["round", "wasm", "#1847"],
  ...["bit-and", "bit-or", "bit-xor", "bit-shl", "bit-shr", "bit-not"].flatMap((name) => [
    [name, "js", "#1849"], [name, "wasm", "#1849"],
  ]),
  ["&list:slice", "js", "#1849"], ["&list:slice", "wasm", "#1849"],
  ["pow", "js", "#1858"], ["pow", "wasm", "#1858"],
];
const knownIssue = (name, backend) => known.find(([n, b]) => n === name && b === backend)?.[2];

// --- generation -------------------------------------------------------------

// xorshift64*, reproducible from the seed alone.
let state = BigInt.asUintN(64, BigInt(options.seed) * 0x9e3779b97f4a7c15n + 1n);
const nextIndex = (length) => {
  state ^= state >> 12n;
  state ^= BigInt.asUintN(64, state << 25n);
  state ^= state >> 27n;
  return Number(BigInt.asUintN(64, state * 0x2545f4914f6cdd1dn) % BigInt(length));
};

const show = (node) => Array.isArray(node) ? `(${node.map(show).join(" ")})` : node;
const cases = [];
for (const [name, pools, kind] of primitives) {
  for (let i = 0; i < options.cases; i++) {
    const call = [name, ...pools.map((pool) => pool[nextIndex(pool.length)])];
    cases.push({ id: `${name}-${i}`, name, call, text: render(kind, call) });
  }
}

// --- replay -------------------------------------------------------------------

const HOST = "&calcit:fuzz-cases";
const corePath = resolve("src/cirru/calcit-core.cirru");
const coreText = await readFile(corePath, "utf8");
const defsHeader = "\n    'calcit.core $ %{} 'FileEntry\n      :defs $ {}\n";
if (!coreText.includes(defsHeader)) throw new Error("unexpected core Snapshot layout");
const RAISED = "!raised";
const indent = (text, spaces) => text.trimEnd().split("\n").map((line) => `${" ".repeat(spaces)}${line}`).join("\n");
const snapshotText = (codeOf) => {
  const testEntry = (fuzzCase) => ["%{}", "'TestEntry", [":name", `|${fuzzCase.id}`],
    [":code", ["quote", codeOf(fuzzCase)]], [":tags", ["#{}", ":fuzz"]]];
  const host = [`'${HOST}`, ["%{}", "'CodeEntry", [":doc", "|"], [":code", ["quote", "nil"]], [":examples", ["[]"]],
    [":tests", ["[]", ...cases.map(testEntry)]]]];
  return coreText.replace(defsHeader, `${defsHeader}${indent(writeCirruCode([host]), 8)}\n`);
};

// Backends with `try` catch each failure in place, so one process runs every call;
// WASM has no `try`, so its calls run plainly and a trap marks the failure.
const requested = options.backends.split(",");
const catching = requested.filter((backend) => backend === "native" || backend === "js");
const plain = requested.filter((backend) => !catching.includes(backend));
const replays = [
  { backends: catching, code: ({ text }) => ["println", ["try", text, ["fn", ["_error"], `|${RAISED}`]]] },
  { backends: plain, code: ({ text }) => ["println", text] },
].filter((replay) => replay.backends.length > 0);

const work = await mkdtemp(join(tmpdir(), "calcit-fuzz-"));
const results = {};
const timings = [];
const started = Date.now();
try {
  for (const [index, replay] of replays.entries()) {
    const snapshot = join(work, `fuzz-${index}.cirru`);
    await writeFile(snapshot, snapshotText(replay.code));
    const resultsPath = join(work, `results-${index}.json`);
    const output = spawnSync(process.execPath, ["scripts/run-core-tests.mjs", "--snapshot", snapshot, "--tag", "fuzz",
      "--backend", replay.backends.join(","), "--results-json", resultsPath], { encoding: "utf8", maxBuffer: 256 * 1024 * 1024 });
    const replayed = JSON.parse(await readFile(resultsPath, "utf8").catch(() => {
      throw new Error(`replay produced no results:\n${output.stdout}\n${output.stderr}`);
    }));
    for (const [id, byBackend] of Object.entries(replayed)) Object.assign(results[id] ??= {}, byBackend);
    timings.push(...output.stdout.split("\n").filter((line) => /^\w+: ran \d+ tests/.test(line)));
  }
} finally {
  await rm(work, { recursive: true, force: true });
}
const report = { results, timings, seconds: (Date.now() - started) / 1000 };

// --- judge --------------------------------------------------------------------

const backends = requested.filter((backend) => backend !== "native");
const outcome = (item) => item === undefined ? "missing"
  : item.status === "compile-error" ? "unsupported"
  : item.status !== "pass" || item.trace[0] === RAISED ? "fails"
  : `returns ${JSON.stringify(item.trace)}`;
const differences = [];
const knownDifferences = [];
let unsupported = 0;
for (const fuzzCase of cases) {
  const byBackend = report.results[`calcit.core/${HOST}#${fuzzCase.id}`] ?? {};
  const native = outcome(byBackend.native);
  if (native === "unsupported") throw new Error(`native rejected ${show(fuzzCase.call)}: ${byBackend.native.detail}`);
  for (const backend of backends) {
    const other = outcome(byBackend[backend]);
    if (other === "unsupported") { unsupported++; continue; }
    const agrees = other === native;
    if (agrees) continue;
    const line = `${backend} ${show(fuzzCase.call)}: native ${native}, ${backend} ${other}`;
    const issue = knownIssue(fuzzCase.name, backend);
    if (issue) knownDifferences.push({ key: `${fuzzCase.name} on ${backend} (${issue})`, line });
    else differences.push(line);
  }
}

console.log(`seed ${options.seed}: ${cases.length} calls of ${primitives.length} primitives on ${options.backends}`
  + ` in ${report.seconds.toFixed(1)}s; ${unsupported} unsupported, ${knownDifferences.length} known differences`);
for (const line of report.timings) console.log(`  ${line}`);
// Known differences are grouped: a count and the first example per primitive and backend.
const knownGroups = new Map();
for (const { key, line } of knownDifferences) {
  const group = knownGroups.get(key) ?? { count: 0, example: line };
  group.count++;
  knownGroups.set(key, group);
}
for (const [key, { count, example }] of knownGroups) console.log(`  known ${key}: ${count}, e.g. ${example}`);
if (differences.length > 0) {
  console.error(`\n${differences.length} difference(s) from native (reproduce with --seed ${options.seed} --cases ${options.cases}):`);
  for (const line of differences) console.error(`  ${line}`);
  process.exit(1);
}
