import assert from "node:assert/strict";
import test from "node:test";
import { existingCrate, existingNpm, validateWorkflowSource, validateRelease, verifyPublishedSource, versionLessThan } from "./publish-registry.mjs";

const tag = "0.29.0-alpha.21";
const sha = "a".repeat(40);
const release = { tagName: tag, isDraft: false, isPrerelease: true };
const runs = ["Test", "Push on main"].map((name, index) => ({
  id: index + 1, name, head_sha: sha, event: index === 0 ? "push" : "dynamic",
  path: index === 0 ? ".github/workflows/test.yaml" : "dynamic/github-code-scanning/codeql",
  head_branch: "main", status: "completed", conclusion: "success",
}));

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
