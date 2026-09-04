import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const args = process.argv.slice(2);
let calcitBin;
const projects = [];

for (let idx = 0; idx < args.length; idx += 1) {
  const arg = args[idx];
  if (arg === "--calcit") {
    calcitBin = args[idx + 1];
    idx += 1;
  } else if (arg === "--project") {
    const value = args[idx + 1] ?? "";
    const separator = value.indexOf("=");
    if (separator <= 0 || separator === value.length - 1) {
      throw new Error("--project expects LABEL=PATH");
    }
    projects.push({ label: value.slice(0, separator), path: resolve(value.slice(separator + 1)) });
    idx += 1;
  } else if (arg === "--help") {
    process.stdout.write(
      "Usage: node scripts/check-strict-nil-ecosystem.mjs --calcit PATH --project LABEL=PATH [--project LABEL=PATH ...]\n",
    );
    process.exit(0);
  } else {
    throw new Error(`unknown argument: ${arg}`);
  }
}

if (calcitBin == null || projects.length === 0) {
  throw new Error("--calcit and at least one --project are required; use --help for usage");
}
calcitBin = resolve(calcitBin);

const run = (command, commandArgs, cwd) =>
  spawnSync(command, commandArgs, {
    cwd,
    encoding: "utf8",
    env: process.env,
    maxBuffer: 16 * 1024 * 1024,
  });

const gitText = (cwd, commandArgs) => {
  const result = run("git", commandArgs, cwd);
  if (result.status !== 0) {
    throw new Error(`git ${commandArgs.join(" ")} failed in ${cwd}: ${result.stderr.trim()}`);
  }
  return result.stdout.trim();
};

const parseCalcitJson = (result, label) => {
  const output = `${result.stdout}\n${result.stderr}`;
  if (/Failed to load (?:module|snapshot)|Failed to load Snapshot/.test(output)) {
    throw new Error(`${label} did not load every requested module:\n${output.trim()}`);
  }
  if (result.status !== 0) {
    throw new Error(`${label} exited ${result.status}:\n${output.trim()}`);
  }
  const match = result.stdout.match(/(?:^|\n)(\{[\s\S]*\})\s*$/);
  if (match == null) {
    throw new Error(`${label} did not end with a JSON object:\n${output.trim()}`);
  }
  const parsed = JSON.parse(match[1]);
  if (parsed.command !== "analyze.weak-types" || parsed.schema_version !== 5) {
    throw new Error(`${label} returned an unexpected analyzer envelope`);
  }
  return parsed.data.summary;
};

const count = (text, pattern) => [...text.matchAll(pattern)].length;

const auditProject = ({ label, path }) => {
  const source = readFileSync(resolve(path, "calcit.cirru"), "utf8");
  const baseArgs = [
    "calcit.cirru",
    "analyze",
    "weak-types",
    "--only",
    "code-nil",
    "--intent",
    "unresolved,declared-unit,declared-optional",
    "--summary-only",
    "--format",
    "json",
  ];
  const projectOnly = parseCalcitJson(run(calcitBin, baseArgs, path), `${label} project-only nil audit`);
  const withDependencies = parseCalcitJson(
    run(calcitBin, [...baseArgs, "--deps"], path),
    `${label} dependency nil audit`,
  );
  const strict = run(calcitBin, ["--check-only", "--strict-types", "calcit.cirru"], path);
  const strictOutput = `${strict.stdout}\n${strict.stderr}`;
  const strictCode = strictOutput.match(/\[(E_[A-Z0-9_]+)\]/)?.[1] ?? null;

  return {
    label,
    path,
    revision: gitText(path, ["rev-parse", "HEAD"]),
    remote: gitText(path, ["remote", "get-url", "origin"]),
    clean: gitText(path, ["status", "--porcelain"]) === "",
    sourceCandidates: {
      legacyOptionalParameters: count(source, /\(\?\s+[^\s)]+/g),
      partialStructConstructors: count(source, /(?:&)?%\{\}\?/g),
      legacyOptionalSchemas: count(source, /::\s+'Optional\b/g),
      mapKvCalls: count(source, /(?<!filter-)\bmap-kv\b/g),
      filterMapKvCalls: count(source, /\bfilter-map-kv\b/g),
    },
    nilAudit: {
      projectOnly,
      withDependencies,
    },
    strictPreflight: {
      passed: strict.status === 0,
      exitCode: strict.status,
      firstDiagnosticCode: strictCode,
    },
  };
};

const report = {
  schemaVersion: 1,
  calcit: {
    path: calcitBin,
    version: run(calcitBin, ["--version"], process.cwd()).stdout.trim(),
  },
  projects: projects.map(auditProject),
};

process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
