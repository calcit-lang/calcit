#!/usr/bin/env node
// Verify WASM codegen: load program.wasm and check exported function results.
// Usage: node scripts/test-wasm.mjs

import { readFileSync } from "fs";

const wasmPath = "js-out/program.wasm";
const wasm = readFileSync(wasmPath);
const mod = new WebAssembly.Module(wasm);
const inst = new WebAssembly.Instance(mod, {
  math: {
    pow: Math.pow,
    sin: Math.sin,
    cos: Math.cos,
  },
});
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

function checkApprox(label, expected, fn, ...args) {
  const got = fn(...args);
  if (Math.abs(got - expected) < 1e-10) {
    console.log(`  ${label} ≈ ${got}  OK`);
  } else {
    console.log(`  ${label} = ${got}  FAIL (expected ≈${expected})`);
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

// --- Tag tests ---
check("test-tag-eq()", 1, e["test-tag-eq"]);
check("test-tag-neq()", 0, e["test-tag-neq"]);

// --- Record tests ---
check("test-record-sum(3,4)", 7, e["test-record-sum"], 3, 4);
check("test-record-sum(10,20)", 30, e["test-record-sum"], 10, 20);

// --- Tuple tests ---
check("test-tuple-sum()", 30, e["test-tuple-sum"]);

// --- Bitwise tests ---
check("test-bit-and(0xFF,0x0F)", 0x0F, e["test-bit-and"], 0xFF, 0x0F);
check("test-bit-or(0xF0,0x0F)", 0xFF, e["test-bit-or"], 0xF0, 0x0F);
check("test-bit-xor(0xFF,0x0F)", 0xF0, e["test-bit-xor"], 0xFF, 0x0F);
check("test-bit-not(0)", -1, e["test-bit-not"], 0);
check("test-bit-shl(1,8)", 256, e["test-bit-shl"], 1, 8);
check("test-bit-shr(256,4)", 16, e["test-bit-shr"], 256, 4);

// --- Match tests ---
check("test-match-tag(3,7)", 10, e["test-match-tag"], 3, 7);
check("test-match-sub(10,3)", 7, e["test-match-sub"], 10, 3);
check("test-match-wildcard()", -1, e["test-match-wildcard"]);

// --- Host import tests (pow, sin, cos) ---
check("test-pow(2,10)", 1024, e["test-pow"], 2, 10);
check("test-pow(3,3)", 27, e["test-pow"], 3, 3);
checkApprox("test-sin(0)", 0, e["test-sin"], 0);
checkApprox("test-sin(π/2)", 1, e["test-sin"], Math.PI / 2);
checkApprox("test-cos(0)", 1, e["test-cos"], 0);
checkApprox("test-cos(π)", -1, e["test-cos"], Math.PI);

// --- Cross-namespace tests ---
check("test-cross-ns(3,7)", 20, e["test-cross-ns"], 3, 7);
check("test-cross-ns(5,5)", 20, e["test-cross-ns"], 5, 5);

// --- calcit.core function tests (abs, negate, &<=, &>=) ---
check("test-abs(5)", 5, e["test-abs"], 5);
check("test-abs(-7)", 7, e["test-abs"], -7);
check("test-abs(0)", 0, e["test-abs"], 0);
check("test-negate(3)", -3, e["test-negate"], 3);
check("test-negate(-4)", 4, e["test-negate"], -4);
check("test-negate(0)", 0, e["test-negate"], 0);
check("test-lte(3,5)", 1, e["test-lte"], 3, 5);
check("test-lte(5,5)", 1, e["test-lte"], 5, 5);
check("test-lte(7,5)", 0, e["test-lte"], 7, 5);
check("test-gte(7,5)", 1, e["test-gte"], 7, 5);
check("test-gte(5,5)", 1, e["test-gte"], 5, 5);
check("test-gte(3,5)", 0, e["test-gte"], 3, 5);
check("test-min(3,7)", 3, e["test-min"], 3, 7);
check("test-min(9,2)", 2, e["test-min"], 9, 2);
check("test-max(3,7)", 7, e["test-max"], 3, 7);
check("test-max(9,2)", 9, e["test-max"], 9, 2);

// --- List tests ---
check("test-list-count()", 3, e["test-list-count"]);
check("test-list-nth(0)", 10, e["test-list-nth"], 0);
check("test-list-nth(2)", 30, e["test-list-nth"], 2);
check("test-list-nth(3)", 40, e["test-list-nth"], 3);
check("test-list-first()", 42, e["test-list-first"]);
check("test-list-rest-count()", 2, e["test-list-rest-count"]);
check("test-list-rest-first()", 20, e["test-list-rest-first"]);
check("test-list-empty-true()", 1, e["test-list-empty-true"]);
check("test-list-empty-false()", 0, e["test-list-empty-false"]);
check("test-list-append()", 33, e["test-list-append"]); // count=3 + nth(2)=30
check("test-list-prepend()", 5, e["test-list-prepend"]);
check("test-list-butlast()", 2, e["test-list-butlast"]);
check("test-list-slice()", 23, e["test-list-slice"]); // count=3 + first=20
check("test-list-reverse()", 40, e["test-list-reverse"]); // first=30 + nth(2)=10
check("test-list-concat()", 44, e["test-list-concat"]); // count=4 + nth(3)=40
check("test-list-assoc()", 99, e["test-list-assoc"]);
check("test-list-dissoc()", 32, e["test-list-dissoc"]); // count=2 + nth(1)=30
check("test-list-contains()", 1, e["test-list-contains"]); // 1+0
check("test-list-includes()", 1, e["test-list-includes"]); // 1+0

// --- Map tests ---
check("test-map-count()", 3, e["test-map-count"]);
check("test-map-get()", 20, e["test-map-get"]);
check("test-map-empty-true()", 1, e["test-map-empty-true"]);
check("test-map-empty-false()", 0, e["test-map-empty-false"]);
check("test-map-assoc-new()", 4, e["test-map-assoc-new"]); // count=2 + get(:b)=2
check("test-map-assoc-update()", 99, e["test-map-assoc-update"]);
check("test-map-dissoc()", 5, e["test-map-dissoc"]); // count=2 + get(:c)=3
check("test-map-contains()", 1, e["test-map-contains"]); // 1+0
check("test-map-includes()", 1, e["test-map-includes"]); // 1+0

// --- Set tests ---
check("test-set-count()", 3, e["test-set-count"]);
check("test-set-empty()", 1, e["test-set-empty"]); // 1+0
check("test-set-includes()", 1, e["test-set-includes"]); // 1+0
check("test-set-include()", 3, e["test-set-include"]);
check("test-set-exclude()", 2, e["test-set-exclude"]);
check("test-to-pairs()", 4, e["test-to-pairs"]); // list count=2 + first pair count=2

if (fail > 0) {
  console.log(`WASM verification FAILED (${fail} failures)`);
  process.exit(1);
}

console.log("=== All WASM checks passed ===");
