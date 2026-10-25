#!/usr/bin/env node
// Run one CI shard of `yarn check-all` against a prebuilt CLI ($CALCIT_BIN).
//
// Usage:
//   node scripts/ci-check-all.mjs --shard i/n [--dry-run]
//
// The top-level steps of `check-all` in package.json stay the single source of
// truth: every step runs in exactly one shard, and a step added there later
// lands in a shard without touching the workflow. Known-assertion parts are
// spread over all shards and run beside the sequential steps.

import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { availableParallelism } from "node:os";

// Approximate seconds per step on a GitHub runner, used only to balance shards.
// Unlisted steps count as DEFAULT_WEIGHT.
const WEIGHTS = {
  "yarn check-js-runtime": 150,
  "yarn check-js-ffi-source": 60,
  "yarn check-strict-default": 160,
  "yarn check-agent-interface": 200,
  "yarn check-core-tests": 120,
  "yarn try-core-tests": 15,
  "yarn try-rs": 25,
  "yarn try-js": 20,
  "yarn try-wasm": 60,
  "yarn check-literal-paths": 20,
  "yarn check-typed-method-rem": 20,
};
const DEFAULT_WEIGHT = 30;
const KNOWN_ASSERTION = "node scripts/check-known-assertion.mjs";

const args = process.argv.slice(2);
const shardArg = args[args.indexOf("--shard") + 1];
const match = /^(\d+)\/(\d+)$/.exec(args.includes("--shard") ? shardArg : "");
if (!match || Number(match[1]) >= Number(match[2])) {
  console.error("usage: node scripts/ci-check-all.mjs --shard i/n [--dry-run]");
  process.exit(2);
}
const [index, count] = [Number(match[1]), Number(match[2])];
const dryRun = args.includes("--dry-run");
if (!dryRun && !process.env.CALCIT_BIN) {
  console.error("CALCIT_BIN must point at the prebuilt CLI");
  process.exit(2);
}

const scripts = JSON.parse(readFileSync("package.json", "utf8")).scripts;
const expand = (command) => command.split(" && ").flatMap((part) => {
  const yarn = /^yarn ([\w:-]+)$/.exec(part);
  return yarn && scripts[yarn[1]] ? expand(scripts[yarn[1]]) : [part];
});
// The CLI is prebuilt and the JS runtime is compiled before any shard starts.
const rewrite = (part) => {
  if (part === "cargo build --bin calcit" || part === "rm -rf lib" || part === "tsc") return [];
  if (part === KNOWN_ASSERTION) return [];
  return [part.replace(/^cargo run --bin calcit -- /, '"$CALCIT_BIN" ')];
};

const steps = scripts["check-all"].split(" && ").map((step) => ({
  step,
  commands: expand(step).flatMap(rewrite),
  weight: WEIGHTS[step] ?? DEFAULT_WEIGHT,
})).filter(({ commands }) => commands.length > 0);
if (!expand(scripts["check-all"]).includes(KNOWN_ASSERTION)) {
  throw new Error(`check-all no longer runs ${KNOWN_ASSERTION}; update this script`);
}

// Longest step first onto the lightest shard; ties keep package.json order.
const loads = Array(count).fill(0);
const assigned = Array.from({ length: count }, () => []);
for (const step of [...steps].sort((a, b) => b.weight - a.weight)) {
  const target = loads.indexOf(Math.min(...loads));
  loads[target] += step.weight;
  assigned[target].push(step);
}
const mine = steps.filter((step) => assigned[index].includes(step));

console.log(`check-all shard ${index + 1}/${count}: known-assertion parts with index % ${count} == ${index}`);
for (const { step, commands } of mine) console.log(`  ${step}\n    ${commands.join("\n    ")}`);
if (dryRun) process.exit(0);

const started = Date.now();
const elapsed = () => `${((Date.now() - started) / 1000).toFixed(1)}s`;
const run = (command, env = process.env) => new Promise((done) => {
  const child = spawn("bash", ["-c", command], { stdio: "inherit", env });
  child.on("close", (status, signal) => done(status === 0 ? null : `${command} failed (${signal ?? `exit ${status}`})`));
});

// Known-assertion parts buffer their own output, so they can run beside the sequential steps.
const knownAssertion = run(KNOWN_ASSERTION, {
  ...process.env,
  CALCIT_KNOWN_ASSERTION_SHARD: `${index}/${count}`,
  CALCIT_KNOWN_ASSERTION_JOBS: String(Math.max(1, availableParallelism() - 1)),
}).then((failure) => {
  console.log(`known-assertion shard finished after ${elapsed()}`);
  return failure;
});

const failures = [];
for (const { step, commands } of mine) {
  console.log(`::group::${step}`);
  const failure = await run(commands.join(" && "));
  console.log(`::endgroup::`);
  console.log(`${step} ${failure ? "failed" : "passed"} at ${elapsed()}`);
  if (failure) failures.push(failure);
}
const knownFailure = await knownAssertion;
if (knownFailure) failures.push(knownFailure);
if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exit(1);
}
console.log(`check-all shard ${index + 1}/${count} passed in ${elapsed()}`);
