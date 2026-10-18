import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { monotonic_time_ms, unix_time_ms } from "../lib/calcit.procs.mjs";

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

  const originalDateNow = Date.now;
  try {
    let reads = 0;
    for (const expected of [0, 1_700_000_000_123]) {
      Date.now = () => { reads += 1; return expected; };
      assert.equal(unix_time_ms(), expected, "runtime must return epoch milliseconds without changing units");
      assert.equal(core.unix_time_ms(), expected, "generated core must call the public epoch clock export");
    }
    assert.equal(reads, 4, "each clock call must read the host exactly once");

    Date.now = () => -1;
    assert.throws(() => unix_time_ms(), /unix-time-ms.*before the Unix epoch/);
    assert.throws(() => core.unix_time_ms(), /unix-time-ms.*before the Unix epoch/);

    const hostFailure = new Error("clock host read failed");
    Date.now = () => { throw hostFailure; };
    assert.throws(() => unix_time_ms(), (error) => error === hostFailure, "runtime must preserve the host failure");
    assert.throws(() => core.unix_time_ms(), (error) => error === hostFailure, "generated core must preserve the host failure");
  } finally {
    Date.now = originalDateNow;
  }
} finally {
  await rm(output, { recursive: true, force: true });
}
