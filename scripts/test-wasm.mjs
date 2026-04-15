#!/usr/bin/env node
// Verify WASM codegen: load program.wasm and check exported function results.
// Usage: node scripts/test-wasm.mjs

import { readFileSync } from "fs";

const wasmPath = "js-out/program.wasm";
const wasm = readFileSync(wasmPath);
const mod = new WebAssembly.Module(wasm);
const inst = new WebAssembly.Instance(mod);
const e = inst.exports;

let fail = 0;

function check(label, expected, fn, ...args) {
  const got = fn(...args);
  if (got === expected) {
    console.log(`  ${label} = ${got}  OK`);
  } else {
    console.log(`  ${label} = ${got}  FAIL (expected ${expected})`);
    fail++;
  }
}

console.log("=== WASM codegen test (Node.js) ===");

check("fibo(10)", 89, e.fibo, 10);
check("factorial(10)", 3628800, e.factorial, 10);
check("add-two(3.5,2.5)", 6, e["add-two"], 3.5, 2.5);
check("sum-range(10)", 55, e["sum-range"], 10);
check("test-floor(3.7)", 3, e["test-floor"], 3.7);
check("test-ceil(3.2)", 4, e["test-ceil"], 3.2);
check("test-round(3.5)", 4, e["test-round"], 3.5);
check("test-sqrt(81)", 9, e["test-sqrt"], 81);
check("test-rem(33,4)", 1, e["test-rem"], 33, 4);
check("test-compare(1,2)", -1, e["test-compare"], 1, 2);
check("test-compare(2,1)", 1, e["test-compare"], 2, 1);
check("test-compare(1,1)", 0, e["test-compare"], 1, 1);
check("test-not(0)", 1, e["test-not"], 0);
check("test-not(1)", 0, e["test-not"], 1);
check("test-let-chain(3)", 20, e["test-let-chain"], 3);
check("collatz-steps(27)", 111, e["collatz-steps"], 27);
check("gcd(48,18)", 6, e.gcd, 48, 18);

if (fail > 0) {
  console.log(`WASM verification FAILED (${fail} failures)`);
  process.exit(1);
}

console.log("=== All WASM checks passed ===");
