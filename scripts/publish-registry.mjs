import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFile, readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export function validateRelease(tag, sha, release, versions, runs) {
  assert.match(tag, /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/);
  assert.match(sha, /^[0-9a-f]{40}$/);
  assert.equal(release.tagName, tag);
  assert.equal(release.isDraft, false, "Publish only an existing, non-draft Release");
  assert.equal(release.isPrerelease, tag.includes("-"), "Release channel must match the version");
  assert.deepEqual(versions, [tag, tag], "Cargo and npm must both match the immutable tag");
  validateMainCI(sha, runs);
}

export function validateMainCI(sha, runs) {
  for (const [name, event, path] of [
    ["Test", "push", ".github/workflows/test.yaml"],
    ["Push on main", "dynamic", "dynamic/github-code-scanning/codeql"],
  ]) {
    const latest = runs.filter(run => run.name === name && run.head_sha === sha
      && run.event === event && run.path === path && run.head_branch === "main")
      .sort((a, b) => b.id - a.id)[0];
    assert.ok(latest, `Missing exact-main ${name} workflow for ${sha}`);
    assert.equal(latest.status, "completed", `${name} must finish before publication`);
    assert.equal(latest.conclusion, "success", `${name} must pass before publication`);
  }
}

export function verifyPublishedSource(metadata, sha, registry) {
  const publishedSha = registry === "crate" ? metadata.git?.sha1 : metadata.gitHead;
  assert.equal(publishedSha, sha, `Existing ${registry} version belongs to a different or unknown commit`);
  if (registry === "crate") assert.notEqual(metadata.git?.dirty, true, "Published crate must have a clean source tree");
}

export function versionLessThan(left, right) {
  const parse = value => {
    const match = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/.exec(value);
    assert.ok(match, `Cannot compare release version ${value}`);
    return { core: match.slice(1, 4).map(BigInt), pre: match[4]?.split(".") };
  };
  const a = parse(left), b = parse(right);
  for (let i = 0; i < 3; i++) if (a.core[i] !== b.core[i]) return a.core[i] < b.core[i];
  if (!a.pre || !b.pre) return Boolean(a.pre && !b.pre);
  for (let i = 0; i < Math.max(a.pre.length, b.pre.length); i++) {
    if (a.pre[i] === b.pre[i]) continue;
    if (a.pre[i] === undefined || b.pre[i] === undefined) return a.pre[i] === undefined;
    const an = /^\d+$/.test(a.pre[i]), bn = /^\d+$/.test(b.pre[i]);
    if (an && bn) return BigInt(a.pre[i]) < BigInt(b.pre[i]);
    if (an !== bn) return an;
    return a.pre[i] < b.pre[i];
  }
  return false;
}

export function validateWorkflowSource(environment, tag, sha) {
  // Registry provenance validation binds these claims to the OIDC certificate.
  assert.equal(environment.GITHUB_REF, `refs/tags/${tag}`, "Run publication from the release tag, not main or another ref");
  assert.equal(environment.GITHUB_SHA, sha, "Workflow source SHA must equal the release checkout");
}

const run = (command, args, options = {}) => execFileSync(command, args, { encoding: "utf8", ...options });
const json = (command, args) => JSON.parse(run(command, args));

async function fetchChecked(url, allowMissing = false) {
  const response = await fetch(url, { signal: AbortSignal.timeout(30000) });
  if (allowMissing && response.status === 404) return null;
  assert.ok(response.ok, `${url}: HTTP ${response.status}; refusing to treat a registry error as absence`);
  return response;
}

export async function existingCrate(version, sha) {
  const index = await (await fetchChecked("https://index.crates.io/ca/lc/calcit")).text();
  const entry = index.trim().split("\n").map(line => JSON.parse(line)).find(item => item.vers === version);
  if (!entry) return false;
  assert.equal(entry.yanked, false, "Do not resume from a yanked crate");
  const archive = Buffer.from(await (await fetchChecked(`https://static.crates.io/crates/calcit/calcit-${version}.crate`)).arrayBuffer());
  assert.equal(createHash("sha256").update(archive).digest("hex"), entry.cksum, "Registry crate checksum mismatch");
  const metadata = JSON.parse(run("tar", ["-xzOf", "-", `calcit-${version}/.cargo_vcs_info.json`], { input: archive }));
  verifyPublishedSource(metadata, sha, "crate");
  return true;
}

export async function existingNpm(version, sha) {
  const response = await fetchChecked(`https://registry.npmjs.org/@calcit%2fprocs/${version}`, true);
  if (!response) return false;
  const metadata = await response.json();
  assert.equal(metadata.name, "@calcit/procs");
  assert.equal(metadata.version, version);
  verifyPublishedSource(metadata, sha, "npm");
  return true;
}

export async function waitForPublished(registry, version, sha, exists, {
  now = () => performance.now(),
  sleep = delay => new Promise(resolve => setTimeout(resolve, delay)),
} = {}) {
  const timeoutMs = registry === "npm" ? 20 * 60 * 1000 : 60 * 1000;
  const intervalMs = registry === "npm" ? 10000 : 5000;
  const deadline = now() + timeoutMs;
  // Only absence is retryable. Identity, integrity and network errors propagate.
  do {
    if (await exists(version, sha)) return;
    const remaining = deadline - now();
    if (remaining <= 0) break;
    await sleep(Math.min(intervalMs, remaining));
  } while (now() < deadline);
  throw new Error(`${registry} upload succeeded but ${version} is not visible; inspect registry processing or manual review before same-tag recovery`);
}

async function main() {
  const [mode, tag] = process.argv.slice(2);
  assert.ok(["prepare", "crate", "npm"].includes(mode), "Expected prepare, crate or npm and a release tag");
  assert.match(tag ?? "", /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/);
  const sha = run("git", ["rev-parse", "HEAD"]).trim();
  assert.equal(run("git", ["cat-file", "-t", `refs/tags/${tag}`]).trim(), "tag", "Release tag must be annotated");
  assert.equal(run("git", ["rev-parse", `refs/tags/${tag}^{}`]).trim(), sha, "Checkout must equal the tag commit");
  validateWorkflowSource(process.env, tag, sha);
  const pkg = JSON.parse(await readFile("package.json", "utf8"));
  const cargo = json("cargo", ["metadata", "--locked", "--no-deps", "--format-version", "1"])
    .packages.find(item => item.name === "calcit");
  assert.equal(pkg.name, "@calcit/procs");
  assert.equal(pkg.version, tag);
  assert.equal(cargo?.version, tag);

  if (mode === "prepare") {
    const repo = process.env.GITHUB_REPOSITORY;
    assert.equal(repo, "calcit-lang/calcit");
    const release = json("gh", ["release", "view", tag, "--repo", repo, "--json", "tagName,isDraft,isPrerelease"]);
    const pages = json("gh", ["api", "--paginate", "--slurp", `repos/${repo}/actions/runs?head_sha=${sha}&per_page=100`]);
    validateRelease(tag, sha, release, [cargo.version, pkg.version], pages.flatMap(page => page.workflow_runs));
    assert.equal(run("git", ["status", "--porcelain"]).trim(), "", "Release checkout must start clean");
    await appendFile(process.env.GITHUB_OUTPUT, `sha=${sha}\nprerelease=${release.isPrerelease}\n`);
    console.log(`Verified immutable release ${tag} at ${sha}`);
    return;
  }

  assert.equal(process.env.RELEASE_SHA, sha, "Publication must use the preflight-verified SHA");
  assert.equal(run("git", ["status", "--porcelain"]).trim(), "", "Publication must not include uncommitted source changes");
  const exists = mode === "crate" ? existingCrate : existingNpm;
  if (await exists(tag, sha)) {
    console.log(`${mode} ${tag} already exists at ${sha}; no republish needed`);
    return;
  }
  if (mode === "crate") {
    run("cargo", ["publish", "--locked", "--no-verify"], { stdio: "inherit" });
  } else {
    const channel = tag.includes("-") ? "next" : "latest";
    const distTags = json("npm", ["view", "@calcit/procs", "dist-tags", "--json"]);
    assert.ok(!distTags[channel] || !versionLessThan(tag, distTags[channel]), `Refusing to roll ${channel} back from ${distTags[channel]} to ${tag}`);
    run("npm", ["publish", "--provenance", "--access", "public", "--tag", channel], {
      stdio: "inherit",
    });
  }
  // Wait for the accepted upload without publishing it a second time.
  await waitForPublished(mode, tag, sha, exists);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(error => { console.error(error.message); process.exitCode = 1; });
}
