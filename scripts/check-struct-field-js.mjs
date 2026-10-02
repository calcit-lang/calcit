import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const nativeTrace = execFileSync(binary, ["calcit/test-wasm.cirru", "test", "--tag", "struct-field-order", "--require-match"], { encoding: "utf8" });
assert.deepEqual(nativeTrace.split(/\r?\n/).filter(line => line.startsWith("struct-order-")),
  ["struct-order-y", "struct-order-x", "struct-order-x", "struct-order-y"]);
const nativeFailure = execFileSync(binary, ["calcit/test-wasm.cirru", "test", "--tag", "struct-field-failure", "--require-match"], { encoding: "utf8" });
assert.deepEqual(nativeFailure.split(/\r?\n/).filter(line => line.startsWith("struct-failure-")), ["struct-failure-y"]);
const typeQuery = JSON.parse(execFileSync(binary, ["calcit/test-wasm.cirru", "query", "type", "test-wasm.main/Point", "--format", "json"], { encoding: "utf8" }));
const fieldMethod = typeQuery.data.methods.find((method) => method.name === ".contains-field?");
assert.equal(fieldMethod?.status, "proven", "Agent method discovery must prove the Struct field contract");
assert.deepEqual(fieldMethod.parameter_types, ["tag"]);
assert.equal(fieldMethod.return_type, "bool");
const invalid = "let ((Point (defstruct Point (:x 'Number))) (point (%{} Point (:x 1)))) (point .contains-field? |x)";
const mismatch = spawnSync(binary, ["src/cirru/calcit-core.cirru", "eval", invalid], { encoding: "utf8" });
assert.notEqual(mismatch.status, 0, "String field names must fail strict type checking");
assert.match(mismatch.stderr, /W_METHOD_ARG_TYPE_MISMATCH/);

const output = await mkdtemp(join(tmpdir(), "calcit-struct-field-js-"));
try {
  await symlink(resolve("node_modules"), join(output, "node_modules"), "dir");
  execFileSync(binary, ["--emit-path", output, "calcit/test-wasm.cirru", "js"], { stdio: "pipe" });
  const compiled = await import(pathToFileURL(join(output, "test-wasm.main.mjs")).href);
  const trace = [];
  const originalLog = console.log;
  try {
    console.log = (...values) => trace.push(values.join(" "));
    assert.equal(compiled.test_struct_field_order(), 1);
    assert.equal(compiled.test_struct_field_order_forward(), 1);
    assert.deepEqual(trace, ["struct-order-y", "struct-order-x", "struct-order-x", "struct-order-y"]);
    trace.length = 0;
    assert.throws(() => compiled.test_struct_field_order_failure(0));
    assert.deepEqual(trace, ["struct-failure-y"]);
  } finally {
    console.log = originalLog;
  }
  assert.equal(compiled.test_struct_contains_field(), 1, "generated JS must preserve the Tag field predicate");
  assert.equal(compiled.test_struct_nominal_equality(), 1, "generated JS must preserve definition identity and structural equality");
  assert.equal(compiled.test_struct_hash(), 1, "equal nested Struct values must have equal hashes");
  assert.equal(compiled.test_struct_map_key(), 1, "Map keys must preserve nominal Struct identity");
  assert.equal(compiled.test_struct_container_hash(), 1, "hashing must recurse through containers of nominal values");
  assert.equal(compiled.test_struct_layout_identity(), 1, "same-named definitions must retain their own field layout");
  assert.equal(compiled.test_struct_edn_identity(), 1, "typed EDN decoding must restore the requested definition identity");
} finally {
  await rm(output, { recursive: true, force: true });
}
