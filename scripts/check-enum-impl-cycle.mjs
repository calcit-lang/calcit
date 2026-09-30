import { execFileSync } from "node:child_process";
import { copyFile, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const fixture = await mkdtemp(join(tmpdir(), "calcit-enum-impl-cycle-"));
const run = (...args) => execFileSync(binary, args, {
  encoding: "utf8", stdio: "pipe", timeout: 60000, maxBuffer: 16 * 1024 * 1024,
});
try {
  const snapshot = join(fixture, "calcit.cirru");
  await copyFile("tests/fixtures/enum-impl-cycle.cirru", snapshot);
  await symlink(resolve("node_modules"), join(fixture, "node_modules"), "dir");
  run(snapshot, "--check-only", "--keep-going", "--format", "edn");
  run(snapshot, "test", "--require-match");
  run(snapshot);
  const output = join(fixture, "js-out");
  run(snapshot, "--emit-path", output, "js");
  const compiled = await import(pathToFileURL(join(output, "fix-command.reader.mjs")).href);
  compiled.main_$x_();
  console.log("Trait-bearing Enum cycle passed nominal graph checks and shared native/JS method semantics");
} finally {
  await rm(fixture, { recursive: true, force: true });
}
