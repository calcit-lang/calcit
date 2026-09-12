#!/usr/bin/env node
// Verify that an unsupported dependency slot traps instead of returning a placeholder value.

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
const unsupported = Object.entries(instance.exports).find(([name]) => name === "vals" || name.endsWith("/vals"))?.[1];
assert.equal(typeof unsupported, "function", "expected calcit.core/vals to remain addressable as a dependency slot");
assert.throws(() => unsupported(), WebAssembly.RuntimeError, "unsupported dependency must trap instead of returning 0.0");

for (const name of [
  "test-closure-escape",
  "test-recursive-closure-specialization",
  "test-rest-closure-specialization",
  "test-dynamic-closure-callee",
]) {
  const boundary = Object.entries(instance.exports).find(([exportName]) => exportName === name || exportName.endsWith(`/${name}`))?.[1];
  assert.equal(typeof boundary, "function", `expected ${name} to remain addressable as a dependency slot`);
  assert.throws(() => boundary(), WebAssembly.RuntimeError, `${name} must fail closed instead of returning a placeholder`);
}

console.log("  unsupported dependency trap  OK");
console.log("  closure specialization boundaries trap  OK");
