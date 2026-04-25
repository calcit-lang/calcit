#!/usr/bin/env node
// Generic WASM test runner: load js-out/program.wasm, call main!(), report pass/fail.
// Usage: node scripts/test-wasm-run.mjs [label]
// Exit code: 0 = pass, 1 = fail

import { readFileSync } from "fs";

const label = process.argv[2] || "test";
const wasmPath = "js-out/program.wasm";
const wasm = readFileSync(wasmPath);
const mod = new WebAssembly.Module(wasm);

const wasmLog = [];

const inst = new WebAssembly.Instance(mod, {
  math: {
    pow: Math.pow,
    sin: Math.sin,
    cos: Math.cos,
  },
  io: {
    log_value: (v) => {
      const mem = new DataView(inst.exports.memory.buffer);
      const ptr = v | 0;
      const HEAP_MAGIC = 0xca1c17a9 | 0;
      if (ptr >= 16 && ptr < mem.byteLength - 8) {
        const magic = mem.getInt32(ptr - 8, true);
        if (magic === HEAP_MAGIC) {
          const typeTag = mem.getInt32(ptr - 4, true);
          if (typeTag === 10) {
            const byteLen = mem.getFloat64(ptr, true);
            const bytes = new Uint8Array(mem.buffer, ptr + 8, byteLen);
            const str = new TextDecoder().decode(bytes);
            process.stdout.write(str + "\n");
            wasmLog.push(str);
            return 0;
          }
        }
      }
      process.stdout.write(String(v) + "\n");
      wasmLog.push(v);
      return 0;
    },
  },
});

const e = inst.exports;

// Find main! export — Calcit uses fully-qualified names like "test-list.main/main!"
const mainFn = Object.entries(e).find(([k]) => k.endsWith("/main!") || k === "main!")?.[1];
if (!mainFn) {
  console.error(`  [${label}] SKIP — no main! export`);
  process.exit(0);
}

try {
  mainFn();
  console.log(`  [${label}] PASS`);
  process.exit(0);
} catch (err) {
  console.log(`  [${label}] FAIL — ${err.message || err}`);
  process.exit(1);
}
