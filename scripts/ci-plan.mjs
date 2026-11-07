#!/usr/bin/env node
// Decide how much of the Test workflow a run needs.
//
//   full  - release pull requests (branch `release/*` or title `chore: release ...`),
//           manual runs, pushes to maintenance branches, and a release commit on main
//           whose exact tree has no green full run yet. Everything runs.
//   light - every other pull request and every other main push. Build, lint, core and
//           docs checks, a check-all smoke, and Rust unit tests always run; integration
//           tests, check-all steps and the WASI / platform jobs run only when the change
//           touches them.
//
// CLI (in the workflow's `plan` job): reads the event from env, writes step outputs to
// $GITHUB_OUTPUT and the changed paths to target/ci-plan/changed.txt.

import { execFileSync } from "node:child_process";
import { appendFileSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const RELEASE_TITLE = /^chore(\([^)]*\))?: release\b/i;

const DOCS_ONLY = [/^docs\//, /\.md$/i, /^LICENSE/, /^\.github\/ISSUE_TEMPLATE\//];
// The workflow and planner themselves: a change there runs every gated job once.
const CI = [/^\.github\/workflows\/test\.yaml$/, /^scripts\/ci-(plan|check-all)\.mjs$/];
// Files the WASI jobs build or run directly, including the snapshots and scripts that the
// WASI 0.3 job replays on the `wasi` backend (keep in sync with .github/workflows/test.yaml).
const WASI = [
  ...CI, /^src\/wasm\//, /^src\/bin\/wasi_preprocess_harness\.rs$/,
  /wasi/i, /^scripts\/.*wasm/, /^tests\/(component_wasm_cli|wasm_support_matrix)\.rs$/,
  /^tests\/fixtures\/component-/, /^Cargo\.(toml|lock)$/, /^rust-toolchain/,
  /^src\/cirru\/calcit-core\.cirru$/, /^calcit\/test-(types-inference|traits)\.cirru$/,
  /^tests\/fixtures\/def-value-schema\.cirru$/,
  /^scripts\/(run-core-tests|check-nil-predicate|check-wasi-number-text|test-published-wasi-http-business)\./,
];
// Files that decide what the macOS and Ubuntu 22.04 no-WASM binaries link and how they start.
const PLATFORM = [...CI, /^Cargo\.(toml|lock)$/, /^build\.rs$/, /^rust-toolchain/, /^\.cargo\//, /^src\/bin\/cr\.rs$/];
// Path segments too generic to pick an integration test by name.
const GENERIC_SEGMENTS = new Set(["src", "bin", "lib", "mod", "main", "cli", "cli_handlers", "common", "util", "tests", "fixtures"]);

/**
 * @param {object} input
 * @param {string} input.event         github.event_name
 * @param {string} input.ref           github.ref
 * @param {string} [input.headRef]     pull request head branch
 * @param {string} [input.title]       pull request title
 * @param {string} [input.headMessage] pushed head commit message
 * @param {boolean} [input.versionBump] the pushed commit changes the Cargo.toml package version
 * @param {boolean} [input.fullEvidence] a green full run exists for this exact tree
 * @param {string[] | null} input.changed  changed paths for a pull request, null otherwise
 * @param {Record<string, string>} [input.testSources] tests/<name>.rs source keyed by name
 */
export function planRun({ event, ref, headRef = "", title = "", headMessage = "", versionBump = false, fullEvidence = false, changed, testSources = {} }) {
  const full = (reason) => ({ mode: "full", reason, rust: true, rustTests: [], wasi: true, platform: true });
  if (event === "workflow_dispatch") return full("manual run");
  if (event === "pull_request") {
    if (headRef.startsWith("release/")) return full(`release branch ${headRef}`);
    if (RELEASE_TITLE.test(title)) return full(`release title "${title}"`);
  }
  if (event === "push") {
    if (ref !== "refs/heads/main") return full(`push to ${ref}`);
    // A version bump marks the release commit however it was merged (squash, merge commit, push).
    if ((versionBump || RELEASE_TITLE.test(headMessage)) && !fullEvidence) return full("release commit on main without a green full run for its tree");
  }

  if (changed === null || changed === undefined) {
    // main push: whatever merged together must still build and pass the smoke.
    const reason = versionBump || RELEASE_TITLE.test(headMessage) ? "release commit on main, tree already passed a full run" : "main push";
    return { mode: "light", reason, rust: true, rustTests: [], wasi: false, platform: false };
  }
  const matches = (patterns) => changed.some((path) => patterns.some((pattern) => pattern.test(path)));
  const docsOnly = changed.length > 0 && changed.every((path) => DOCS_ONLY.some((pattern) => pattern.test(path)));
  return {
    mode: "light",
    reason: docsOnly ? "pull request, docs only" : "pull request",
    rust: !docsOnly,
    rustTests: docsOnly ? [] : selectIntegrationTests(changed, testSources),
    wasi: matches(WASI),
    platform: matches(PLATFORM),
  };
}

// Integration tests to run for a light pull request: the changed test files, tests whose source
// names a changed file (fixtures, calcit/*.cirru, scripts), and tests named after a changed
// source module (src/bin/cli_handlers/fix.rs -> fix_cli, query.rs -> query_*_cli).
export function selectIntegrationTests(changed, testSources) {
  const names = Object.keys(testSources);
  const picked = new Set();
  for (const path of changed) {
    const own = /^tests\/([\w-]+)\.rs$/.exec(path);
    if (own && names.includes(own[1])) picked.add(own[1]);
    // The file itself, and each fixture directory holding it (tests often name only the directory).
    const ancestors = path.split("/").slice(0, -1).map((_, i, parts) => parts.slice(0, i + 1).join("/"))
      .filter((dir) => /^tests\/fixtures\/./.test(dir));
    const needles = [path, ...ancestors].flatMap((item) => [item, item.startsWith("tests/") ? item.slice("tests/".length) : null]).filter(Boolean);
    for (const name of names) {
      if (needles.some((needle) => testSources[name].includes(needle))) picked.add(name);
    }
    if (path.startsWith("src/") && path.endsWith(".rs")) {
      const segments = [...dirname(path).split("/"), basename(path, ".rs")].filter((segment) => !GENERIC_SEGMENTS.has(segment));
      for (const segment of segments) {
        for (const name of names) if (name === segment || name.startsWith(`${segment}_`)) picked.add(name);
      }
    }
  }
  return [...picked].sort();
}

function sh(command, args) {
  return execFileSync(command, args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 }).trim();
}

// A green full run for exactly this tree, from a release branch of this repository (not a fork).
function hasFullEvidence(repo, tree) {
  const artifacts = JSON.parse(sh("gh", ["api", `repos/${repo}/actions/artifacts?name=full-ci-tree-${tree}&per_page=20`])).artifacts;
  for (const artifact of artifacts) {
    const run = artifact.workflow_run;
    if (artifact.expired || !run || run.head_repository_id !== run.repository_id) continue;
    const detail = JSON.parse(sh("gh", ["api", `repos/${repo}/actions/runs/${run.id}`]));
    if (detail.path === ".github/workflows/test.yaml" && detail.conclusion === "success") {
      console.log(`full-run evidence: run ${run.id} on ${run.head_branch}`);
      return true;
    }
  }
  return false;
}

// The [package] version of Cargo.toml at a revision (needs the parent commit fetched).
function packageVersion(revision) {
  const manifest = sh("git", ["show", `${revision}:Cargo.toml`]);
  return /^\[package\][^[]*?^version\s*=\s*"([^"]+)"/m.exec(manifest)?.[1];
}

function main() {
  const env = process.env;
  const event = env.GITHUB_EVENT_NAME;
  const payload = JSON.parse(readFileSync(env.GITHUB_EVENT_PATH, "utf8"));
  const repo = env.GITHUB_REPOSITORY;
  const tree = sh("git", ["rev-parse", "HEAD^{tree}"]);
  const pr = payload.pull_request;
  const changed = event === "pull_request"
    ? sh("gh", ["api", "--paginate", `repos/${repo}/pulls/${pr.number}/files?per_page=100`, "-q", ".[].filename"]).split("\n").filter(Boolean)
    : null;
  const headMessage = payload.head_commit?.message ?? "";
  const mainPush = event === "push" && env.GITHUB_REF === "refs/heads/main";
  const versionBump = mainPush && packageVersion("HEAD") !== packageVersion("HEAD^");
  const fullEvidence = mainPush && (versionBump || RELEASE_TITLE.test(headMessage))
    ? hasFullEvidence(repo, tree)
    : false;
  const testSources = Object.fromEntries(readdirSync("tests").filter((file) => file.endsWith(".rs"))
    .map((file) => [basename(file, ".rs"), readFileSync(join("tests", file), "utf8")]));

  const plan = planRun({ event, ref: env.GITHUB_REF, headRef: pr?.head?.ref, title: pr?.title, headMessage, versionBump, fullEvidence, changed, testSources });
  console.log(JSON.stringify({ ...plan, tree, changed: changed?.length ?? "n/a" }, null, 2));
  mkdirSync("target/ci-plan", { recursive: true });
  writeFileSync("target/ci-plan/changed.txt", (changed ?? []).join("\n") + "\n");
  const out = {
    mode: plan.mode, full: String(plan.mode === "full"), rust: String(plan.rust), wasi: String(plan.wasi),
    platform: String(plan.platform), rust_tests: plan.rustTests.map((name) => `--test ${name}`).join(" "), tree,
  };
  appendFileSync(env.GITHUB_OUTPUT, Object.entries(out).map(([key, value]) => `${key}=${value}\n`).join(""));
  appendFileSync(env.GITHUB_STEP_SUMMARY ?? "/dev/null",
    `### Test plan: ${plan.mode}\n\n${plan.reason}\n\n| job group | runs |\n|---|---|\n`
    + `| Rust tests | ${plan.rust ? (plan.mode === "full" ? "all" : `unit + ${plan.rustTests.length ? plan.rustTests.join(", ") : "no integration tests"}`) : "skipped"} |\n`
    + `| check-all | ${plan.mode === "full" ? "5 shards" : "smoke + touched steps"} |\n`
    + `| WASI jobs | ${plan.wasi ? "yes" : "skipped"} |\n| macOS / no-WASM binaries | ${plan.platform ? "yes" : "skipped"} |\n`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
