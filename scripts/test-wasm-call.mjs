#!/usr/bin/env node
// Call a specific WASM export by name. Usage: node scripts/test-wasm-call.mjs <fnName>
import { readFileSync } from "fs";

const fnName = process.argv[2];
if (!fnName) {
  console.error("usage: node test-wasm-call.mjs <export-name>");
  process.exit(2);
}

const wasm = readFileSync("js-out/program.wasm");
const mod = new WebAssembly.Module(wasm);

const inst = new WebAssembly.Instance(mod, {
  math: { pow: Math.pow, sin: Math.sin, cos: Math.cos },
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
            process.stdout.write(new TextDecoder().decode(bytes) + "\n");
            return 0;
          }
        }
      }
      process.stdout.write(String(v) + "\n");
      return 0;
    },
  },
});

const fn = inst.exports[fnName];
if (!fn) {
  console.error(`no such export: ${fnName}`);
  process.exit(2);
}

try {
  const r = fn();
  console.log(`[${fnName}] PASS → ${r}`);
} catch (err) {
  console.log(`[${fnName}] FAIL — ${err.message || err}`);
  process.exit(1);
}
