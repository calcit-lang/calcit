import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import path from "node:path";

const [, , version, ...assetPaths] = process.argv;

if (!version || assetPaths.length === 0) {
  throw new Error("Usage: node scripts/release-manifest.mjs <version> <asset>...");
}

const assets = await Promise.all(
  assetPaths.map(async (assetPath) => {
    const [content, metadata] = await Promise.all([readFile(assetPath), stat(assetPath)]);
    return {
      name: path.basename(assetPath),
      sha256: createHash("sha256").update(content).digest("hex"),
      size: metadata.size,
    };
  }),
);

process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      version,
      assets,
    },
    null,
    2,
  )}\n`,
);
