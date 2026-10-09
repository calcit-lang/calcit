import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { hasNativeParity, markerProtocolError, parseBackendSelection, parseMarkers, parseWasmDiagnosticTable } from "./core-test-protocol.mjs";

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
  const stdout = "@@core-test:0\na\n@@core-test-end:0\n@@core-test:1\nb\n@@core-test-end:1\n";
  const parsed = parseMarkers(stdout);
  assert.equal(markerProtocolError(parsed.events, tests, true), undefined);
  assert.deepEqual([...parsed.traces], [[0, ["a"]], [1, ["b"]]]);
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
