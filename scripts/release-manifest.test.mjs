import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

test("release manifest records asset name, size, and SHA-256", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "calcit-release-manifest-"));
  const asset = path.join(directory, "calcit");
  await writeFile(asset, "calcit release asset");

  const result = spawnSync(process.execPath, ["scripts/release-manifest.mjs", "0.13.28", asset], {
    cwd: process.cwd(),
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr);

  const manifest = JSON.parse(result.stdout);
  assert.deepEqual(manifest, {
    schemaVersion: 1,
    version: "0.13.28",
    assets: [
      {
        name: "calcit",
        sha256: createHash("sha256").update("calcit release asset").digest("hex"),
        size: 20,
      },
    ],
  });
});
