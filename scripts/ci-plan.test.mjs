import assert from "node:assert/strict";
import { test } from "node:test";
import { planRun, selectIntegrationTests } from "./ci-plan.mjs";

const pr = (changed, extra = {}) => planRun({ event: "pull_request", ref: "refs/pull/1/merge", headRef: "fix/x", title: "fix: x", changed, ...extra });
const sources = {
  fix_cli: 'fixture("fixtures/fix-command.cirru")',
  query_usages_cli: "",
  query_entry_cli: "",
  edit_cli: '"calcit/test-edit-defmacro.cirru"',
  wasi_command_component_cli: "tests/fixtures/wasi-command-03.cirru",
  native_async_exit_cli: 'join("tests/fixtures/native-async-exit/Cargo.toml")',
};

test("release pull requests and manual runs are full", () => {
  assert.equal(pr([], { headRef: "release/0.36.0" }).mode, "full");
  assert.equal(pr([], { title: "chore: release 0.36.0" }).mode, "full");
  assert.equal(pr([], { title: "Chore(release): Release 1.0.0" }).mode, "full");
  assert.equal(pr([], { title: "chore: releases notes" }).mode, "light");
  assert.equal(planRun({ event: "workflow_dispatch", ref: "refs/heads/x", changed: null }).mode, "full");
  assert.equal(planRun({ event: "push", ref: "refs/heads/release/0.30.1", changed: null }).mode, "full");
});

test("main pushes are light unless a release commit lacks a full run for its tree", () => {
  const main = (headMessage, fullEvidence) => planRun({ event: "push", ref: "refs/heads/main", headMessage, fullEvidence, changed: null });
  assert.deepEqual(main("fix: a (#1)", false), { mode: "light", reason: "main push", rust: true, rustTests: [], wasi: false, platform: false });
  assert.equal(main("chore: release 0.36.0 (#2624)", false).mode, "full");
  assert.equal(main("chore: release 0.36.0 (#2624)", true).mode, "light");
  const merged = (fullEvidence) => planRun({ event: "push", ref: "refs/heads/main", headMessage: "Merge pull request #9 from x/release/1.0.0", versionBump: true, fullEvidence, changed: null });
  assert.equal(merged(false).mode, "full");
  assert.equal(merged(true).mode, "light");
});

test("light pull requests pick jobs by path", () => {
  const docs = pr(["docs/intro.md", "README.md"]);
  assert.equal(docs.rust, false);
  assert.equal(docs.wasi, false);
  const core = pr(["src/runner/preprocess.rs"], { testSources: sources });
  assert.deepEqual([core.rust, core.wasi, core.platform], [true, false, false]);
  assert.equal(pr(["src/wasm/emit_wasm.rs"]).wasi, true);
  assert.equal(pr(["scripts/test-wasi-stdin.mjs"]).wasi, true);
  for (const path of ["calcit/test-traits.cirru", "tests/fixtures/def-value-schema.cirru", "src/cirru/calcit-core.cirru", "scripts/run-core-tests.mjs"]) {
    assert.equal(pr([path]).wasi, true, path);
  }
  const workflow = pr([".github/workflows/test.yaml"]);
  assert.deepEqual([workflow.wasi, workflow.platform], [true, true]);
  const deps = pr(["Cargo.lock"]);
  assert.deepEqual([deps.wasi, deps.platform], [true, true]);
});

test("integration tests follow the changed files", () => {
  assert.deepEqual(selectIntegrationTests(["tests/edit_cli.rs"], sources), ["edit_cli"]);
  assert.deepEqual(selectIntegrationTests(["tests/fixtures/fix-command.cirru"], sources), ["fix_cli"]);
  assert.deepEqual(selectIntegrationTests(["calcit/test-edit-defmacro.cirru"], sources), ["edit_cli"]);
  assert.deepEqual(selectIntegrationTests(["src/bin/cli_handlers/query.rs"], sources), ["query_entry_cli", "query_usages_cli"]);
  assert.deepEqual(selectIntegrationTests(["src/bin/cli_handlers/fix/apply.rs"], sources), ["fix_cli"]);
  assert.deepEqual(selectIntegrationTests(["tests/fixtures/native-async-exit/src/lib.rs"], sources), ["native_async_exit_cli"]);
  assert.deepEqual(selectIntegrationTests(["src/lib.rs", "src/bin/cli_handlers/mod.rs"], sources), []);
});
