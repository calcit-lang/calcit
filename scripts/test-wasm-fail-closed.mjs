#!/usr/bin/env node
// Verify that ordinary `defn` values stay internal: unsupported dependency slots
// must not leak into the public host ABI. Explicit `defwasm-export` declarations
// are the only business exports, and an unsupported explicit boundary is rejected
// at codegen time (covered by the wasm lowering-failure fixtures).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const wasmPath = process.argv[2];
if (!wasmPath) {
  throw new Error("expected a WASM artifact path");
}

const module = new WebAssembly.Module(readFileSync(wasmPath));
const imports = {};
for (const descriptor of WebAssembly.Module.imports(module)) {
  if (descriptor.kind !== "function") {
    throw new Error(`unsupported test import kind: ${descriptor.kind}`);
  }
  imports[descriptor.module] ??= {};
  imports[descriptor.module][descriptor.name] = () => 0;
}

const instance = new WebAssembly.Instance(module, imports);
const exportNames = Object.keys(instance.exports);

// Unsupported internal dependencies are internal only. They must not appear in
// the public export surface, where a host could mistake a trapping slot for a
// callable business export.
for (const internal of [
  "vals",
  "test-closure-escape",
  "test-recursive-closure-specialization",
  "test-rest-closure-specialization",
  "test-spread-closure-specialization",
  "test-dynamic-closure-callee",
]) {
  const leaked = exportNames.find((name) => name === internal || name.endsWith(`/${internal}`));
  assert.equal(leaked, undefined, `internal dependency ${internal} must not be exported`);
}

// Runtime helpers and `memory` remain available, so rejecting internal slots did
// not strip the reserved host symbols.
assert.ok(exportNames.includes("memory"), "memory must remain exported");
assert.ok(exportNames.includes("__str_new"), "__str_new must remain exported");

console.log("  unsupported internal slots are not exported  OK");
console.log("  reserved runtime exports survive  OK");
