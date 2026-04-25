#!/usr/bin/env node
// Verify WASM codegen: load program.wasm and check exported function results.
// Usage: node scripts/test-wasm.mjs

import { readFileSync } from "fs";

const wasmPath = "js-out/program.wasm";
const wasm = readFileSync(wasmPath);
const mod = new WebAssembly.Module(wasm);

// Collect values printed from WASM for test assertions
const wasmLog = [];

const inst = new WebAssembly.Instance(mod, {
  math: {
    pow: Math.pow,
    sin: Math.sin,
    cos: Math.cos,
  },
  io: {
    // log_value(v: f64) — decode v and print using host memory view
    log_value: (v) => {
      const mem = new DataView(inst.exports.memory.buffer);
      // f64 pointer: if it looks like a heap address, read type_tag from header
      const raw = v;
      // Try to read as a heap pointer (i32 stored in f64)
      const ptr = raw | 0; // truncate to i32
      const HEAP_MAGIC = 0xca1c17a9 | 0;
      if (ptr >= 16 && ptr < mem.byteLength - 8) {
        const magic = mem.getInt32(ptr - 8, true);
        if (magic === HEAP_MAGIC) {
          const typeTag = mem.getInt32(ptr - 4, true);
          if (typeTag === 10) {
            // string: read byte_len then UTF-8 bytes
            const byteLen = mem.getFloat64(ptr, true);
            const bytes = new Uint8Array(mem.buffer, ptr + 8, byteLen);
            const str = new TextDecoder().decode(bytes);
            console.log("[wasm-println]", str);
            wasmLog.push(str);
            return 0;
          }
        }
      }
      // Fall back: print as number
      console.log("[wasm-println]", raw);
      wasmLog.push(raw);
      return 0;
    },
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

const STRING_TAG = 34;
const HEAP_MAGIC = 0xca1c17a9 | 0;
function readWasmStr(ptr) {
  const mem = new DataView(inst.exports.memory.buffer);
  const iptr = ptr | 0;
  if (iptr < 16) return null;
  const magic = mem.getInt32(iptr - 8, true);
  if (magic !== HEAP_MAGIC) return null;
  const tag = mem.getInt32(iptr - 4, true);
  if (tag !== STRING_TAG) return null;
  const byteLen = mem.getFloat64(iptr, true);
  return new TextDecoder().decode(new Uint8Array(mem.buffer, iptr + 8, byteLen));
}

function checkStr(label, expected, fn, ...args) {
  const got = fn(...args);
  const s = readWasmStr(got);
  if (s === expected) {
    console.log(`  ${label} = ${JSON.stringify(s)}  OK`);
  } else {
    console.log(`  ${label} = ${JSON.stringify(s)}  FAIL (expected ${JSON.stringify(expected)})`);
    fail++;
  }
}

function getHashSlots(n) {
  const hash = e["test-map-hash-value"](n) >>> 0;
  return [hash & 31, (hash >>> 5) & 31];
}

function findSameTopDifferentLeaf(limit = 4096) {
  const byTop = new Map();
  for (let n = 1; n <= limit; n++) {
    const [idx0, idx1] = getHashSlots(n);
    const seen = byTop.get(idx0);
    if (seen != null && seen.idx1 !== idx1) {
      return [seen.value, n, idx0, seen.idx1, idx0, idx1];
    }
    if (seen == null) {
      byTop.set(idx0, { value: n, idx1 });
    }
  }
  throw new Error("failed to find numeric pair with shared top slot");
}

function findSameBucketCollision(limit = 4096) {
  const seenBuckets = new Map();
  for (let n = 1; n <= limit; n++) {
    const [idx0, idx1] = getHashSlots(n);
    const bucketKey = `${idx0}:${idx1}`;
    const seen = seenBuckets.get(bucketKey);
    if (seen != null) {
      return [seen, n, idx0, idx1, idx0, idx1];
    }
    seenBuckets.set(bucketKey, n);
  }
  throw new Error("failed to find numeric bucket collision for WASM map test");
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
check("test-record-matches-true()", 1, e["test-record-matches-true"]);
check("test-record-field-tag()", 1, e["test-record-field-tag"]);
check("test-record-get-name()", 1, e["test-record-get-name"]);
check("test-record-struct-eq()", 1, e["test-record-struct-eq"]);
check("test-record-to-map()", 3, e["test-record-to-map"]);
check("test-call-spread-rest()", 15, e["test-call-spread-rest"]);

// --- Tuple tests ---
check("test-tuple-sum()", 30, e["test-tuple-sum"]);
check("test-tuple-count()", 3, e["test-tuple-count"]);

// --- Bitwise tests ---
check("test-bit-and(0xFF,0x0F)", 0x0f, e["test-bit-and"], 0xff, 0x0f);
check("test-bit-or(0xF0,0x0F)", 0xff, e["test-bit-or"], 0xf0, 0x0f);
check("test-bit-xor(0xFF,0x0F)", 0xf0, e["test-bit-xor"], 0xff, 0x0f);
check("test-bit-not(0)", -1, e["test-bit-not"], 0);
check("test-bit-shl(1,8)", 256, e["test-bit-shl"], 1, 8);
check("test-bit-shr(256,4)", 16, e["test-bit-shr"], 256, 4);

// --- Match tests ---
check("test-match-tag(3,7)", 10, e["test-match-tag"], 3, 7);
check("test-match-sub(10,3)", 7, e["test-match-sub"], 10, 3);
check("test-match-wildcard()", -1, e["test-match-wildcard"]);
check("test-hash-number()", 1, e["test-hash-number"]);

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
check("test-list-first-generic()", 42, e["test-list-first-generic"]);
check("test-list-nth(3)", 40, e["test-list-nth"], 3);
check("test-list-first()", 42, e["test-list-first"]);
check("test-list-rest-generic-first()", 20, e["test-list-rest-generic-first"]);
check("test-list-rest-count()", 2, e["test-list-rest-count"]);
check("test-list-rest-first()", 20, e["test-list-rest-first"]);
check("test-list-empty-true()", 1, e["test-list-empty-true"]);
check("test-list-empty-false()", 0, e["test-list-empty-false"]);
check("test-list-empty-method()", 0, e["test-list-empty-method"]);
check("test-list-empty?-method()", 1, e["test-list-empty?-method"]);
check("test-list-append()", 33, e["test-list-append"]); // count=3 + nth(2)=30
check("test-list-prepend()", 5, e["test-list-prepend"]);
check("test-tuple-assoc()", 29, e["test-tuple-assoc"]);
check("test-list-butlast()", 2, e["test-list-butlast"]);
check("test-list-slice()", 23, e["test-list-slice"]); // count=3 + first=20
check("test-list-reverse()", 40, e["test-list-reverse"]); // first=30 + nth(2)=10
check("test-list-concat()", 44, e["test-list-concat"]); // count=4 + nth(3)=40
check("test-list-assoc()", 99, e["test-list-assoc"]);
check("test-list-assoc-before()", 103, e["test-list-assoc-before"]); // count=4 + nth(1)=99
check("test-list-assoc-after()", 103, e["test-list-assoc-after"]); // count=4 + nth(1)=99
check("test-list-dissoc()", 32, e["test-list-dissoc"]); // count=2 + nth(1)=30
check("test-list-to-set()", 3, e["test-list-to-set"]); // {10,20,30} deduplicated
check("test-list-contains()", 1, e["test-list-contains"]); // 1+0
check("test-list-includes()", 1, e["test-list-includes"]); // 1+0
check("test-list-contains-method()", 1, e["test-list-contains-method"]); // 1+0
check("test-list-includes-method()", 1, e["test-list-includes-method"]); // 1+0
check("test-list-max-method()", 30, e["test-list-max-method"]);
check("test-list-min-method()", 10, e["test-list-min-method"]);

// --- Map tests ---
check("test-map-count()", 3, e["test-map-count"]);
check("test-map-get()", 20, e["test-map-get"]);
check("test-map-empty-true()", 1, e["test-map-empty-true"]);
check("test-map-empty-false()", 0, e["test-map-empty-false"]);
check("test-map-empty-method()", 0, e["test-map-empty-method"]);
check("test-map-assoc-new()", 4, e["test-map-assoc-new"]); // count=2 + get(:b)=2
check("test-map-assoc-update()", 99, e["test-map-assoc-update"]);
check("test-map-dissoc()", 5, e["test-map-dissoc"]); // count=2 + get(:c)=3
check("test-map-contains()", 1, e["test-map-contains"]); // 1+0
check("test-map-includes()", 1, e["test-map-includes"]); // 1+0
check("test-map-contains-method()", 1, e["test-map-contains-method"]); // 1+0
check("test-map-includes-method()", 1, e["test-map-includes-method"]); // 1+0

// --- Set tests ---
check("test-set-count()", 3, e["test-set-count"]);
check("test-set-empty()", 1, e["test-set-empty"]); // 1+0
check("test-set-empty-method()", 0, e["test-set-empty-method"]);
check("test-set-includes()", 1, e["test-set-includes"]); // 1+0
check("test-set-contains-method()", 1, e["test-set-contains-method"]); // 1+0
check("test-set-includes-method()", 1, e["test-set-includes-method"]); // 1+0
check("test-set-max-method()", 30, e["test-set-max-method"]);
check("test-set-min-method()", 10, e["test-set-min-method"]);
check("test-set-include()", 3, e["test-set-include"]);
check("test-set-exclude()", 2, e["test-set-exclude"]);
check("test-set-difference()", 2, e["test-set-difference"]); // {10,30} from {10,20,30,40} - {20,40}
check("test-set-difference-empty()", 2, e["test-set-difference-empty"]); // disjoint, keeps all
check("test-set-union()", 4, e["test-set-union"]); // {10,20,30,40}
check("test-set-union-same()", 3, e["test-set-union-same"]); // {10,20,30}
check("test-to-pairs()", 4, e["test-to-pairs"]); // list count=2 + first pair count=2

// --- Map merge/diff tests ---
check("test-map-merge()", 3, e["test-map-merge"]); // {a:1, b:3, c:4}
check("test-map-merge-value()", 99, e["test-map-merge-value"]); // b overridden to 99
check("test-map-diff-new()", 1, e["test-map-diff-new"]); // {a:1} — entries of a not in b
check("test-map-diff-keys()", 2, e["test-map-diff-keys"]); // #{a, c} not in b
check("test-map-common-keys()", 2, e["test-map-common-keys"]); // #{b, c} in both

const sameTopDifferentLeaf = findSameTopDifferentLeaf();
check(
  `test-map-two-keys-sum(${sameTopDifferentLeaf[0]},${sameTopDifferentLeaf[1]})`,
  30,
  e["test-map-two-keys-sum"],
  sameTopDifferentLeaf[0],
  sameTopDifferentLeaf[1]
);

const sameBucketCollision = findSameBucketCollision();
check(
  `test-map-two-keys-sum collision(${sameBucketCollision[0]},${sameBucketCollision[1]})`,
  30,
  e["test-map-two-keys-sum"],
  sameBucketCollision[0],
  sameBucketCollision[1]
);
check(
  `test-map-bucket-update(${sameBucketCollision[0]},${sameBucketCollision[1]})`,
  109,
  e["test-map-bucket-update"],
  sameBucketCollision[0],
  sameBucketCollision[1]
);

// --- Range tests ---
check("test-range()", 5, e["test-range"]); // [0,1,2,3,4]
check("test-range-sum()", 4, e["test-range-sum"]); // 0+4
check("test-range-two-args()", 3, e["test-range-two-args"]); // [2,3,4]

// --- Rest args tests ---
check("test-rest-count()", 3, e["test-rest-count"]);
check("test-rest-sum() 1+2+3+4+5", 15, e["test-rest-sum"]);
check("test-rest-empty() 10+20", 30, e["test-rest-empty"]);

// --- type-of tests ---
check("test-type-of-list", 1, e["test-type-of-list"]);
check("test-type-of-map", 1, e["test-type-of-map"]);
check("test-type-of-set", 1, e["test-type-of-set"]);
check("test-type-of-number", 1, e["test-type-of-number"]);
check("test-type-of-tuple", 1, e["test-type-of-tuple"]);

// --- derived predicates (list?, number?, map?) ---
check("test-list?-true", 1, e["test-list?-true"]);
check("test-list?-false", 0, e["test-list?-false"]);
check("test-number?-true", 1, e["test-number?-true"]);
check("test-map?-true", 1, e["test-map?-true"]);

// --- BufList tests ---
check("test-buf-list-push()", 3, e["test-buf-list-push"]);
check("test-buf-list-to-list()", 3, e["test-buf-list-to-list"]);

// --- String operation tests ---
check("test-str-count()", 5, e["test-str-count"]);
check("test-str-empty-true()", 1, e["test-str-empty-true"]); // count("") == 0
check("test-str-empty-false()", 0, e["test-str-empty-false"]); // count("hi") == 0 is false
check("test-str-concat()", 6, e["test-str-concat"]);
check("test-str-nth()", 1, e["test-str-nth"]); // &str:nth returns the one-character string "e"
checkStr("test-str-first()", "h", e["test-str-first"]); // &str:first returns "h" as a one-character string
check("test-str-rest()", 4, e["test-str-rest"]);
check("test-str-slice()", 3, e["test-str-slice"]); // &str:slice "abcde" 1 4 = "bcd"
check("test-str-compare-eq()", 0, e["test-str-compare-eq"]);
check("test-str-compare-lt()", -1, e["test-str-compare-lt"]);
check("test-str-compare-gt()", 1, e["test-str-compare-gt"]);
check("test-str-contains-true()", 1, e["test-str-contains-true"]); // idx 1 < len 5
check("test-str-contains-false()", 0, e["test-str-contains-false"]); // idx 10 >= len 5
check("test-str-find-index-found()", 1, e["test-str-find-index-found"]); // "ell" at byte 1
check("test-str-find-index-not-found()", -1, e["test-str-find-index-not-found"]); // "xyz" not found
check("test-str-includes-true()", 1, e["test-str-includes-true"]); // "ell" in "hello"
check("test-str-includes-false()", 0, e["test-str-includes-false"]); // "xyz" not in "hello"
check("test-str-pad-left()", 5, e["test-str-pad-left"]); // pad-left "hi" 5 "-" = "---hi"
check("test-str-pad-right()", 5, e["test-str-pad-right"]); // pad-right "hi" 5 "-" = "hi---"
check("test-display-by-bin()", 7, e["test-display-by-bin"]); // 17 in binary = "0b10001" (len 7)
check("test-display-by-hex()", 4, e["test-display-by-hex"]); // 17 in hex = "0x11" (len 4)

// --- __str_new FFI test (JS → WASM string passing) ---
// Protocol: read heap top, write bytes at top+16 (zero-copy), call __str_new(top+16, len)
{
  const mem = inst.exports.memory.buffer;
  const heapTop = inst.exports.__heap_ptr.value;
  const encoder = new TextEncoder();
  const bytes = encoder.encode("world");
  new Uint8Array(mem, heapTop + 16, bytes.length).set(bytes);
  const strPtr = inst.exports.__str_new(heapTop + 16, bytes.length);
  // Decode: byte_len at strPtr, content at strPtr+8
  const view = new DataView(mem);
  const byteLen = view.getFloat64(strPtr | 0, true);
  const decoded = new TextDecoder().decode(new Uint8Array(mem, (strPtr | 0) + 8, byteLen));
  if (decoded === "world") {
    console.log("  __str_new FFI: 'world'  OK");
  } else {
    console.log(`  __str_new FFI: '${decoded}'  FAIL (expected 'world')`);
    fail++;
  }
}

// --- println host import test ---
wasmLog.length = 0;
e["test-println"]();
if (wasmLog[0] === 42) {
  console.log("  test-println() logged 42  OK");
} else {
  console.log(`  test-println() logged ${wasmLog[0]}  FAIL (expected 42)`);
  fail++;
}

if (fail > 0) {
  console.log(`WASM verification FAILED (${fail} failures)`);
  process.exit(1);
}

console.log("=== All WASM checks passed ===");
