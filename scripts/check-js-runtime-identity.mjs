import assert from "node:assert/strict";
import { cp, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const fixtureRoot = await mkdtemp(join(tmpdir(), "calcit-js-runtime-identity-"));

try {
  const runtimeAPath = join(fixtureRoot, "runtime-a", "lib");
  const runtimeBPath = join(fixtureRoot, "runtime-b", "lib");
  await cp(new URL("../lib", import.meta.url), runtimeAPath, { recursive: true });
  await cp(new URL("../lib", import.meta.url), runtimeBPath, { recursive: true });
  await symlink(resolve("node_modules"), join(fixtureRoot, "node_modules"), "dir");

  const runtimeA = await import(pathToFileURL(join(runtimeAPath, "calcit.procs.mjs")).href);
  const runtimeB = await import(pathToFileURL(join(runtimeBPath, "calcit.procs.mjs")).href);
  const foreignField = runtimeA.newTag("show");
  const method = () => "demo";
  const foreignImpl = new runtimeA.CalcitImpl(runtimeA.newTag("ForeignImpl"), [foreignField], [method], null);

  assert.ok(
    foreignImpl instanceof runtimeB.CalcitImpl,
    "CalcitImpl values from another runtime module instance should remain recognizable"
  );

  const clonedImpl = runtimeB._$n_impl_$o__$o_new(runtimeB.newTag("ClonedImpl"), foreignImpl);
  assert.ok(clonedImpl instanceof runtimeB.CalcitImpl);
  assert.ok(clonedImpl.fields[0] instanceof runtimeB.CalcitTag);
  assert.equal(clonedImpl.fields[0].value, "show");
  assert.deepEqual(clonedImpl.values, [method]);

  console.log("JS runtime identity check passed");
} finally {
  await rm(fixtureRoot, { recursive: true, force: true });
}
