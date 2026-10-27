import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { mkdir, mkdtemp, readFile, realpath, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { hasNativeParity, markerProtocolError, parseBackendSelection, parseMarkers, parseWasmDiagnosticTable, prepareReplayProject, replayRevision } from "./core-test-protocol.mjs";

test("replay retains named-entry, parent and transitive files without sharing project state", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "calcit-replay-context-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const project = join(root, "original", "project");
  const projectAlias = join(root, "project-link");
  const cache = join(root, "cache");
  const output = join(root, "replay");
  const write = async (path, contents) => {
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, contents);
  };
  const metadata = (modules) => ({ ":entries": { ":default": { ":modules": modules } } });
  // These are opaque host-test metadata files, not generated Calcit programs.
  const source = metadata(["./missing-default.json"]);
  source[":entries"][":browser"] = { ":modules": ["./sibling.json", "library.json", "./.calcit/modules/library.json"] };
  source[":files"] = { main: { defs: { ffi: { ffi: { ":js": { ":file": "impl/identity.js" } } } } } };
  const files = new Map([
    [join(project, "source.json"), JSON.stringify(source)],
    [join(project, "sibling.json"), JSON.stringify(metadata(["./../parent.json"]))],
    [join(root, "original", "parent.json"), JSON.stringify(metadata(["./transitive.json"]))],
    [join(project, "transitive.json"), JSON.stringify(metadata([]))],
    [join(cache, "library.json"), JSON.stringify(metadata(["./transitive.json"]))],
    [join(project, "deps.cirru"), "pinned manifest bytes"],
    [join(project, "impl", "identity.js"), "(value) => value"],
    [join(project, ".calcit", "cursor.cirru"), "original cursor"],
    [join(project, ".calcit", "error.cirru"), "original error"],
  ]);
  for (const [path, contents] of files) await write(path, contents);
  await symlink(cache, join(project, ".calcit", "modules"), "dir");
  await symlink(project, projectAlias, "dir");
  const replay = await prepareReplayProject({
    root: output, snapshotPath: join(projectAlias, "source.json"), snapshot: source, entry: "browser",
    nodeModules: join(root, "runtime"), readSnapshot: path => JSON.parse(readFileSync(path, "utf8")),
    readModules: (path, entry) => JSON.parse(readFileSync(path, "utf8"))[":entries"]?.[`:${entry}`]?.[":modules"],
  });
  for (const path of ["sibling.json", "../parent.json", "transitive.json", "deps.cirru", "impl/identity.js"]) {
    assert.equal(await readFile(join(replay, path), "utf8"), await readFile(join(project, path), "utf8"));
  }
  assert.equal(await realpath(join(replay, ".calcit", "modules")), await realpath(cache));
  assert.equal(existsSync(join(replay, ".calcit", "cursor.cirru")), false);
  assert.equal(existsSync(join(replay, ".calcit", "error.cirru")), false);
  for (const [path, contents] of files) assert.equal(await readFile(path, "utf8"), contents);
});

test("replay never copies a local file into a different linked runtime graph", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "calcit-replay-runtime-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const source = join(root, "original");
  const runtime = join(root, "runtime");
  await mkdir(join(source, "node_modules"), { recursive: true });
  await mkdir(runtime);
  const original = JSON.stringify({ ":entries": { ":default": { ":modules": [] } } });
  await writeFile(join(source, "node_modules", "library.json"), original);
  await writeFile(join(runtime, "library.json"), "unrelated installed bytes");
  await assert.rejects(prepareReplayProject({
    root: join(root, "replay"), snapshotPath: join(source, "source.json"), snapshot: {}, nodeModules: runtime,
    readModules: path => path.endsWith("source.json") ? ["./node_modules/library.json"] : [],
    readSnapshot: path => JSON.parse(readFileSync(path, "utf8")),
  }), /conflicts with the read-only dependency view/);
  assert.equal(await readFile(join(runtime, "library.json"), "utf8"), "unrelated installed bytes");
  assert.equal(await readFile(join(source, "node_modules", "library.json"), "utf8"), original);
});

test("replay rejects genuinely missing modules before repeated compilation", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "calcit-replay-missing-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const path = join(root, "original", "source.json");
  await mkdir(dirname(path));
  let reads = 0;
  await assert.rejects(prepareReplayProject({
    root: join(root, "replay"), snapshotPath: path,
    snapshot: { ":entries": { ":default": { ":modules": ["./missing.json"] } } },
    nodeModules: join(root, "runtime"),
    readSnapshot: path => { reads++; return JSON.parse(readFileSync(path, "utf8")); },
    readModules: () => ["./missing.json"],
  }), /ENOENT.*missing.json/);
  assert.equal(reads, 1);
});

test("copying a JS resource cannot legitimize a symlink outside its original owner", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "calcit-replay-owner-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const project = join(root, "original");
  await mkdir(project);
  await writeFile(join(root, "outside.js"), "() => 1");
  await symlink(join(root, "outside.js"), join(project, "escape.js"));
  const replay = await prepareReplayProject({
    root: join(root, "replay"), snapshotPath: join(project, "source.json"),
    snapshot: { ":entries": { ":default": {} }, ":files": {
      main: { defs: {
        first: { ffi: { ":js": { ":file": "escape.js" } } },
        second: { ffi: { ":js": { ":file": "escape.js" } } },
        unused: { ffi: { ":js": { ":file": "missing.js" } } },
      } },
    } },
    nodeModules: join(root, "runtime"), readSnapshot: () => { throw new Error("unexpected dependency"); },
    readModules: () => [],
  });
  assert.equal(await realpath(join(replay, "escape.js")), await realpath(join(root, "outside.js")));
  assert.ok(!((await realpath(join(replay, "escape.js"))).startsWith(`${replay}/`)));
  assert.equal(existsSync(join(replay, "missing.js")), false);
});

test("large replay mutations use a bounded full-file revision without dropping the guard", () => {
  const original_revision = "0123456789abcdef0123456789abcdef";
  const scoped_revision = "scope:" + Array.from({ length: 5850 }, (_, index) =>
    `def:calcit.core/&calcit:test:${index}@absent`).join(",");
  assert.ok(Buffer.byteLength(scoped_revision) > 128 * 1024);
  assert.equal(replayRevision({ original_revision, scoped_revision }), original_revision);
  assert.equal(replayRevision({ original_revision }), original_revision);
  for (const preview of [{}, { scoped_revision }, { original_revision: "" }]) {
    assert.throws(() => replayRevision(preview), /missing its original Snapshot revision/);
  }
});

test("WASM diagnostic coverage comes from code cells, not prose or other tables", () => {
  const fixture = readFileSync(new URL("../tests/fixtures/wasm-support-matrix-parser.md", import.meta.url), "utf8");
  for (const text of [fixture, fixture.replaceAll("\n", "\r\n")]) {
    assert.deepEqual([...parseWasmDiagnosticTable(text)].sort(), ["E_WASM_FIRST", "E_WASM_SECOND", "E_WASM_THIRD"]);
  }
  const missingRow = fixture.split("\n").filter(line => !line.startsWith("| `E_WASM_FIRST`")).join("\n");
  assert.ok(missingRow.includes("`E_WASM_FIRST`"));
  assert.equal(parseWasmDiagnosticTable(missingRow).has("E_WASM_FIRST"), false);
});

const supported = ["native", "js", "wasm", "wasi"];
const tests = [{ index: 0 }, { index: 1 }];

test("backend selection rejects empty, duplicate and unknown backends", () => {
  for (const value of ["", ",", ",,,"]) assert.throws(() => parseBackendSelection(value, supported), /at least one/);
  assert.throws(() => parseBackendSelection("native,native", supported), /duplicate/);
  assert.throws(() => parseBackendSelection("typo", supported), /unknown/);
  assert.deepEqual(parseBackendSelection("native,js,wasm", supported), ["native", "js", "wasm"]);
});

test("the runner rejects invalid backend arguments before reporting success", () => {
  for (const value of ["", ",", "native,native", "typo"]) {
    const output = spawnSync(process.execPath, [fileURLToPath(new URL("./run-core-tests.mjs", import.meta.url)), "--backend", value], {
      encoding: "utf8", timeout: 10000,
    });
    assert.equal(output.status, 1, output.stderr);
    assert.match(output.stderr, /at least one backend|duplicate backends|unknown backend/);
    assert.doesNotMatch(output.stdout, /tests passed/);
  }
});

test("successful early exit cannot pass unexecuted or unfinished tests", () => {
  for (const stdout of ["", "@@core-test:0\n", "@@core-test:0\n@@core-test-end:0\n", "@@core-test:0\n@@core-test-end:0\n@@core-test:1\n"]) {
    assert.match(markerProtocolError(parseMarkers(stdout).events, tests, true), /before all tests completed/);
  }
});

test("complete markers preserve traces and failures allow only a valid prefix", () => {
  const stdout = "\n@@core-test:0\n\na\n\n@@core-test-end:0\n@@core-test:1\nb\n@@core-test-end:1\n";
  const parsed = parseMarkers(stdout);
  assert.equal(markerProtocolError(parsed.events, tests, true), undefined);
  assert.deepEqual([...parsed.traces], [[0, ["", "a", ""]], [1, ["b"]]]);
  assert.equal(parsed.last, 1);
  assert.equal(markerProtocolError(parseMarkers("@@core-test:0\n").events, tests, false), undefined);
  for (const events of [
    [["start", 1]], [["end", 0]], [["start", 0], ["start", 0]],
    [...parsed.events, ["start", 2]],
  ]) assert.match(markerProtocolError(events, tests, false), /invalid test marker/);
});

test("exclusions are removable only after execution and native parity pass", () => {
  const native = { status: "pass", trace: ["expected"] };
  assert.equal(hasNativeParity({ status: "pass", trace: ["different"] }, native, "js"), false);
  assert.equal(hasNativeParity({ status: "pass", trace: ["expected"] }, undefined, "js"), false);
  assert.equal(hasNativeParity({ status: "pass", trace: ["expected"] }, { status: "fail" }, "wasm"), false);
  assert.equal(hasNativeParity({ status: "fail", trace: ["expected"] }, native, "js"), false);
  assert.equal(hasNativeParity({ status: "pass", trace: ["expected"] }, native, "js"), true);
  assert.equal(hasNativeParity(native, native, "native"), true);
});
