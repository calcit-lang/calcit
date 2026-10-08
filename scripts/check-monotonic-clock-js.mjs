import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { monotonic_time_ms } from "../lib/calcit.procs.mjs";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const output = await mkdtemp(join(tmpdir(), "calcit-monotonic-clock-js-"));

try {
  await symlink(resolve("node_modules"), join(output, "node_modules"), "dir");
  execFileSync(binary, ["--emit-path", output, "calcit/test-wasi-command.cirru", "js"], { stdio: "pipe" });
  const core = await import(pathToFileURL(join(output, "calcit.core.mjs")).href);
  const before = core.monotonic_time_ms();
  const runtime = monotonic_time_ms();
  const after = core.monotonic_time_ms();
  assert.equal(typeof before, "number");
  assert.ok(before <= runtime && runtime <= after, "generated JS clocks must stay monotonic between core and @calcit/procs");
} finally {
  await rm(output, { recursive: true, force: true });
}
