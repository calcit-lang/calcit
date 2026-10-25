import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { existingCrate, existingNpm, publicationBranch, validateWorkflowSource, validateRelease, verifyPublishedSource, versionLessThan, waitForBranchCI, waitForPublished } from "./publish-registry.mjs";

const tag = "0.29.0-alpha.21";
const sha = "a".repeat(40);
const release = { tagName: tag, isDraft: false, isPrerelease: true };
const runs = ["Test", "Push on main"].map((name, index) => ({
  id: index + 1, name, head_sha: sha, event: index === 0 ? "push" : "dynamic",
  path: index === 0 ? ".github/workflows/test.yaml" : "dynamic/github-code-scanning/codeql",
  head_branch: "main", status: "completed", conclusion: "success",
}));

const workflowJobs = workflow => Object.fromEntries(workflow.split(/\n(?=  [a-z_]+:\n)/).slice(1)
  .map(block => [/^  ([a-z_]+):/.exec(block)[1], block]));

test("publication runs only after every parallel release gate and never repeats branch CI", () => {
  const workflow = readFileSync(new URL("../.github/workflows/publish.yaml", import.meta.url), "utf8");
  const jobs = workflowJobs(workflow.slice(workflow.indexOf("\njobs:\n")));
  assert.deepEqual(Object.keys(jobs),
    ["verify_source", "branch_ci", "standard_binary", "no_wasm_binary", "package_dry_run", "publish"]);
  assert.match(jobs.publish,
    /^    needs: \[verify_source, branch_ci, standard_binary, no_wasm_binary, package_dry_run\]$/m);
  for (const gate of ["branch_ci", "standard_binary", "no_wasm_binary", "package_dry_run"]) {
    assert.match(jobs[gate], /^    needs: verify_source$/m, `${gate} must start from the verified tag`);
  }
  assert.match(jobs.branch_ci, /publish-registry\.mjs ci "\$RELEASE_TAG"/);
  assert.match(jobs.package_dry_run, /cargo publish --dry-run --locked/);
  // Branch Test already ran these on the exact commit; the gates must not run them again.
  assert.doesNotMatch(workflow.replace(/^\s*#.*$/gm, ""), /cargo test|check-all|check-docs-md|run-core-tests/);
  // Only the final job may write releases or mint registry identities.
  assert.match(workflow, /^permissions:\n  contents: read\n\n/m);
  for (const [name, job] of Object.entries(jobs)) {
    assert.equal(/contents: write|id-token: write/.test(job), name === "publish", `${name} permissions`);
  }
});

test("publication checkout explicitly selects the event ref and preserves annotated tags", (t) => {
  const workflow = readFileSync(new URL("../.github/workflows/publish.yaml", import.meta.url), "utf8");
  const jobs = workflowJobs(workflow.slice(workflow.indexOf("\njobs:\n")));
  for (const [name, job] of Object.entries(jobs)) {
    const checkout = /- uses: actions\/checkout@v7\.0\.1\n\s+with:([\s\S]*?)(?=\n\n|\n      -)/.exec(job)?.[1];
    assert.ok(checkout, `Expected the ${name} checkout configuration`);
    assert.match(checkout, /^\s+ref: \$\{\{ github\.ref \}\}\s*$/m);
    assert.match(checkout, /^\s+persist-credentials: false\s*$/m);
    // Jobs that run publish-registry.mjs need the annotated tag and branch history.
    if (job.includes("publish-registry.mjs")) assert.match(checkout, /^\s+fetch-depth: 0\s*$/m, name);
  }

  // Reproduce the fetch commands recorded in failed Publish run 37898278970.
  // checkout's implicit event SHA replaces the tag object; explicit ref avoids that fetch.
  const root = mkdtempSync(join(tmpdir(), "calcit-publish-tag-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const git = (cwd, ...args) => execFileSync("git", ["-C", cwd, ...args], {
    encoding: "utf8", stdio: ["ignore", "pipe", "pipe"],
  }).trim();
  const origin = join(root, "origin");
  mkdirSync(origin);
  git(origin, "init", "--initial-branch=main");
  git(origin, "-c", "user.name=Release Test", "-c", "user.email=release@example.invalid",
    "-c", "commit.gpgsign=false", "commit", "--allow-empty", "-m", "release fixture");
  const releaseSha = git(origin, "rev-parse", "HEAD");
  git(origin, "-c", "user.name=Release Test", "-c", "user.email=release@example.invalid",
    "-c", "tag.gpgsign=false", "tag", "-a", tag, "-m", "annotated release fixture");
  const ref = `refs/tags/${tag}`;
  const tagObject = git(origin, "rev-parse", ref);
  assert.notEqual(tagObject, releaseSha);
  for (const mode of ["implicit-sha", "explicit-ref"]) {
    const checkoutRoot = join(root, mode);
    mkdirSync(checkoutRoot);
    git(checkoutRoot, "init");
    git(checkoutRoot, "remote", "add", "origin", origin);
    git(checkoutRoot, "fetch", "--no-recurse-submodules", "origin",
      "+refs/heads/*:refs/remotes/origin/*", "+refs/tags/*:refs/tags/*");
    assert.equal(git(checkoutRoot, "rev-parse", ref), tagObject);
    if (mode === "implicit-sha") {
      git(checkoutRoot, "fetch", "--no-tags", "origin", `+${releaseSha}:${ref}`);
      assert.equal(git(checkoutRoot, "cat-file", "-t", ref), "commit");
    } else {
      assert.equal(git(checkoutRoot, "cat-file", "-t", ref), "tag");
    }
    git(checkoutRoot, "checkout", "--detach", ref);
    assert.equal(git(checkoutRoot, "rev-parse", "HEAD"), releaseSha);
    validateWorkflowSource({ GITHUB_REF: ref, GITHUB_SHA: releaseSha }, tag, releaseSha);
  }
  assert.equal(git(origin, "rev-parse", ref), tagObject, "The remote release tag must remain unchanged");
});

test("recovery cannot roll npm channels back, including numeric alpha ordering", () => {
  for (const [older, newer] of [
    ["0.28.1", "0.29.0"], ["0.29.0-alpha.9", "0.29.0-alpha.21"],
    ["0.29.0-alpha.21", "0.29.0"], ["0.29.0-alpha", "0.29.0-alpha.1"],
    ["0.29.0-1", "0.29.0-alpha"], ["0.29.0-alpha", "0.29.0-beta"],
  ]) {
    assert.equal(versionLessThan(older, newer), true);
    assert.equal(versionLessThan(newer, older), false);
  }
  assert.equal(versionLessThan(tag, tag), false);
  assert.throws(() => versionLessThan(tag, "latest"));
});

test("release recovery accepts only the exact tested tag/version pair", () => {
  validateRelease(tag, sha, release, [tag, tag], runs);
  for (const field of [{ isDraft: true }, { tagName: "0.29.0" }, { isPrerelease: false }]) {
    assert.throws(() => validateRelease(tag, sha, { ...release, ...field }, [tag, tag], runs));
  }
  assert.throws(() => validateRelease(tag, sha, release, [tag, "0.29.0"], runs));
  assert.throws(() => validateRelease("../../main", sha, release, [tag, tag], runs));
});

test("missing, stale, failed, pending and non-main workflows cannot authorize publication", () => {
  assert.throws(() => validateRelease(tag, sha, release, [tag, tag], runs.slice(1)));
  for (const field of [
    { head_sha: "b".repeat(40) }, { event: "pull_request" }, { head_branch: "feature" }, { path: "other.yml" },
    { status: "in_progress" }, { conclusion: "failure" }, { conclusion: "cancelled" },
  ]) {
    assert.throws(() => validateRelease(tag, sha, release, [tag, tag], [{ ...runs[0], ...field }, runs[1]]));
  }
  assert.throws(() => validateRelease(tag, sha, release, [tag, tag], [...runs, { ...runs[0], id: 10, conclusion: "failure" }]));
});

test("only 0.30.1 releases use exact maintenance-branch CI, never main or PR checks", () => {
  const branch = "release/0.30.1";
  const maintenanceRuns = runs.map((run, index) => ({
    ...run, head_branch: branch, name: index === 0 ? "Test" : `Push on ${branch}`,
  }));
  for (const version of ["0.30.1-alpha.5", "0.30.1"]) {
    const metadata = { tagName: version, isDraft: false, isPrerelease: version.includes("-") };
    assert.equal(publicationBranch(version), branch);
    validateRelease(version, sha, metadata, [version, version], maintenanceRuns);
    assert.throws(() => validateRelease(version, sha, metadata, [version, version], runs));
    assert.throws(() => validateRelease(version, sha, metadata, [version, version], maintenanceRuns.slice(1)));
    for (const field of [
      { head_sha: "b".repeat(40) }, { event: "pull_request" }, { head_branch: "main" },
      { path: "other.yml" }, { status: "in_progress" }, { conclusion: "failure" }, { conclusion: "cancelled" },
    ]) {
      assert.throws(() => validateRelease(version, sha, metadata, [version, version],
        [{ ...maintenanceRuns[0], ...field }, maintenanceRuns[1]]));
      assert.throws(() => validateRelease(version, sha, metadata, [version, version],
        [maintenanceRuns[0], { ...maintenanceRuns[1], ...field }]));
    }
    assert.throws(() => validateRelease(version, sha, metadata, [version, version], [
      ...maintenanceRuns, { ...maintenanceRuns[1], id: 10, conclusion: "failure" },
    ]));
  }
  for (const version of ["0.30.0", "0.30.2", "0.30.10", "0.31.0-alpha.1", "0.32.0"]) {
    assert.equal(publicationBranch(version), "main");
    const metadata = { tagName: version, isDraft: false, isPrerelease: version.includes("-") };
    validateRelease(version, sha, metadata, [version, version], runs);
    assert.throws(() => validateRelease(version, sha, metadata, [version, version], maintenanceRuns));
  }
});

test("an existing registry version may be skipped only with matching clean source identity", () => {
  verifyPublishedSource({ git: { sha1: sha } }, sha, "crate");
  verifyPublishedSource({ git: { sha1: sha, dirty: false } }, sha, "crate");
  verifyPublishedSource({ gitHead: sha }, sha, "npm");
  assert.throws(() => verifyPublishedSource({}, sha, "crate"));
  assert.throws(() => verifyPublishedSource({}, sha, "npm"));
  assert.throws(() => verifyPublishedSource({ git: { sha1: sha, dirty: true } }, sha, "crate"));
  assert.throws(() => verifyPublishedSource({ git: { sha1: "b".repeat(40) } }, sha, "crate"));
  assert.throws(() => verifyPublishedSource({ gitHead: "b".repeat(40) }, sha, "npm"));
});

test("publication rejects workflow source claims that differ from the release tag", () => {
  const workflow = `calcit-lang/calcit/.github/workflows/publish.yaml@refs/tags/${tag}`;
  const environment = Object.freeze({ GITHUB_SHA: sha, GITHUB_REF: `refs/tags/${tag}`,
    GITHUB_WORKFLOW_REF: workflow, GITHUB_WORKFLOW_SHA: sha,
    GITHUB_RUN_ID: "123", ACTIONS_ID_TOKEN_REQUEST_URL: "https://example.invalid/oidc" });
  validateWorkflowSource(environment, tag, sha);
  for (const field of [
    { GITHUB_REF: "refs/heads/main" }, { GITHUB_REF: "refs/tags/0.28.1" },
    { GITHUB_SHA: "b".repeat(40) }, { GITHUB_REF: undefined }, { GITHUB_SHA: undefined },
  ]) assert.throws(() => validateWorkflowSource({ ...environment, ...field }, tag, sha));
});

test("registry lookup distinguishes missing versions from failed lookups", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response("[]", { status: 503 }));
  await assert.rejects(existingCrate(tag, sha), /HTTP 503/);
  await assert.rejects(existingNpm(tag, sha), /HTTP 503/);
  globalThis.fetch.mock.mockImplementation(async () => new Response("Not found", { status: 404 }));
  await assert.rejects(existingCrate(tag, sha), /HTTP 404/);
  assert.equal(await existingNpm(tag, sha), false);
  globalThis.fetch.mock.mockImplementation(async () => new Response(JSON.stringify({ vers: "0.28.1", yanked: false })));
  assert.equal(await existingCrate(tag, sha), false);
});

test("published npm identity and crate integrity fail closed", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response(JSON.stringify({ name: "@calcit/procs", version: tag, gitHead: sha })));
  assert.equal(await existingNpm(tag, sha), true);
  globalThis.fetch.mock.mockImplementation(async () => new Response(JSON.stringify({ name: "other", version: tag, gitHead: sha })));
  await assert.rejects(existingNpm(tag, sha));
  globalThis.fetch.mock.mockImplementation(async () => new Response(JSON.stringify({ vers: tag, yanked: true })));
  await assert.rejects(existingCrate(tag, sha), /yanked/);
  globalThis.fetch.mock.mockImplementation(async url => new Response(String(url) === "https://index.crates.io/ca/lc/calcit"
    ? JSON.stringify({ vers: tag, yanked: false, cksum: "invalid" }) : "invalid archive"));
  await assert.rejects(existingCrate(tag, sha), /checksum mismatch/);
});

test("accepted uploads return immediately when verified without sleeping", async () => {
  for (const registry of ["npm", "crate"]) {
    let calls = 0;
    await waitForPublished(registry, tag, sha, async (version, source) => {
      assert.equal(version, tag);
      assert.equal(source, sha);
      calls++;
      return true;
    }, { now: () => 0, sleep: async () => assert.fail("Visible uploads must not sleep") });
    assert.equal(calls, 1);
  }
});

test("npm scanning can exceed one minute before the exact identity becomes visible", async (t) => {
  let time = 0;
  let calls = 0;
  t.mock.method(globalThis, "fetch", async () => {
    calls++;
    return time < 90000 ? new Response("Processing", { status: 404 })
      : new Response(JSON.stringify({ name: "@calcit/procs", version: tag, gitHead: sha }));
  });
  await waitForPublished("npm", tag, sha, existingNpm, {
    now: () => time,
    sleep: async delay => { assert.equal(delay, 10000); time += delay; },
  });
  assert.equal(time, 90000);
  assert.equal(calls, 10);
});

test("visibility waiting is bounded and counts lookup time without real sleeps", async () => {
  for (const [registry, limit, interval] of [["npm", 1200000, 10000], ["crate", 60000, 5000]]) {
    let time = 0;
    let calls = 0;
    const waits = [];
    await assert.rejects(waitForPublished(registry, tag, sha, async () => {
      calls++;
      time += 3000;
      return false;
    }, {
      now: () => time,
      sleep: async delay => { assert.ok(delay > 0 && delay <= interval); waits.push(delay); time += delay; },
    }), /inspect registry processing or manual review before same-tag recovery/);
    assert.equal(time, limit);
    assert.equal(waits.length, calls);
    assert.ok(waits.at(-1) < interval, "The last wait must not exceed the remaining window");
  }
});

test("network and source identity errors stop waiting immediately", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response("Unavailable", { status: 503 }));
  const clock = { now: () => 0, sleep: async () => assert.fail("Errors must not be retried") };
  await assert.rejects(waitForPublished("npm", tag, sha, existingNpm, clock), /HTTP 503/);
  globalThis.fetch.mock.mockImplementation(async () => new Response(JSON.stringify({ name: "@calcit/procs", version: tag, gitHead: "b".repeat(40) })));
  await assert.rejects(waitForPublished("npm", tag, sha, existingNpm, clock), /different or unknown commit/);
  globalThis.fetch.mock.mockImplementation(async () => new Response(JSON.stringify({ vers: tag, yanked: true })));
  await assert.rejects(waitForPublished("crate", tag, sha, existingCrate, clock), /yanked/);
});

test("branch CI that is still running when the tag is pushed is awaited, not rejected", async () => {
  // Publish runs 40188001842 and 40188548225 started before Test on the tagged commit finished.
  let time = 0;
  const states = [[], [{ ...runs[0], status: "in_progress", conclusion: null }, runs[1]], runs];
  const result = await waitForBranchCI(sha, "main", async () => states.shift(), {
    now: () => time, sleep: async delay => { assert.equal(delay, 30000); time += delay; }, log: () => {},
  });
  assert.equal(result, runs);
  assert.equal(time, 60000);
});

test("branch CI waiting fails at once on red runs and is bounded otherwise", async () => {
  const clock = { now: () => 0, sleep: async () => assert.fail("Failed CI must not be awaited"), log: () => {} };
  for (const conclusion of ["failure", "cancelled"]) {
    await assert.rejects(waitForBranchCI(sha, "main", async () => [{ ...runs[0], conclusion }, runs[1]], clock),
      /Test must pass before publication/);
  }
  await assert.rejects(waitForBranchCI(sha, "main", async () => [...runs, { ...runs[0], id: 10, conclusion: "failure" }], clock));
  for (const [pending, limit, message] of [
    [[runs[0]], 5 * 60 * 1000, /Missing exact-main Push on main workflow/],
    [[runs[0], { ...runs[1], status: "queued", conclusion: null }], 45 * 60 * 1000, /Timed out waiting for Push on main \(queued\)/],
  ]) {
    let time = 0;
    await assert.rejects(waitForBranchCI(sha, "main", async () => pending, {
      now: () => time, sleep: async delay => { time += delay; }, log: () => {},
    }), message);
    assert.equal(time, limit);
  }
  // Runs on another branch or commit never count, even while waiting.
  let time = 0;
  await assert.rejects(waitForBranchCI(sha, "main", async () => runs.map(run => ({ ...run, head_branch: "feature" })), {
    now: () => time, sleep: async delay => { time += delay; }, log: () => {},
  }), /Missing exact-main/);
});

test("transient run lookup errors are retried within the deadline", async () => {
  let time = 0;
  let calls = 0;
  await waitForBranchCI(sha, "main", async () => {
    if (calls++ === 0) throw new Error("HTTP 502");
    return runs;
  }, { now: () => time, sleep: async delay => { time += delay; }, log: () => {} });
  assert.equal(calls, 2);
});
