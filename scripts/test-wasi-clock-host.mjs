import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [wasmPath] = process.argv.slice(2);
if (wasmPath == null) {
  throw new Error("usage: node scripts/test-wasi-clock-host.mjs <program.wasm>");
}

let memory;
let stdout = "";
const requestedClocks = [];

const view = () => new DataView(memory.buffer);
const bytes = () => new Uint8Array(memory.buffer);

const wasi = {
  args_sizes_get(argcPtr, argvSizePtr) {
    view().setUint32(argcPtr, 0, true);
    view().setUint32(argvSizePtr, 0, true);
    return 0;
  },
  args_get() {
    return 0;
  },
  environ_sizes_get(countPtr, sizePtr) {
    view().setUint32(countPtr, 0, true);
    view().setUint32(sizePtr, 0, true);
    return 0;
  },
  environ_get() {
    return 0;
  },
  fd_write(fd, iovsPtr, iovsLength, writtenPtr) {
    assert.equal(fd, 1);
    let written = 0;
    for (let i = 0; i < iovsLength; i += 1) {
      const ptr = view().getUint32(iovsPtr + i * 8, true);
      const length = view().getUint32(iovsPtr + i * 8 + 4, true);
      stdout += new TextDecoder().decode(bytes().subarray(ptr, ptr + length));
      written += length;
    }
    view().setUint32(writtenPtr, written, true);
    return 0;
  },
  proc_exit(code) {
    throw new Error(`unexpected proc_exit(${code})`);
  },
  clock_time_get(clockId, precision, resultPtr) {
    requestedClocks.push([clockId, precision]);
    const nanoseconds = clockId === 0 ? 1_234_000_000n : clockId === 1 ? 5_678_000_000n : null;
    if (nanoseconds == null) {
      return 28;
    }
    view().setBigUint64(resultPtr, nanoseconds, true);
    return 0;
  },
  random_get() {
    throw new Error("clock fixture unexpectedly requested random bytes");
  },
  fd_prestat_get() {
    throw new Error("clock fixture unexpectedly requested a filesystem preopen");
  },
  fd_prestat_dir_name() {
    throw new Error("clock fixture unexpectedly requested a preopen name");
  },
  path_open() {
    throw new Error("clock fixture unexpectedly opened a filesystem path");
  },
  fd_filestat_get() {
    throw new Error("clock fixture unexpectedly requested file metadata");
  },
  fd_read() {
    throw new Error("clock fixture unexpectedly read a file");
  },
  fd_readdir() {
    throw new Error("clock fixture unexpectedly read a directory");
  },
  fd_close() {
    throw new Error("clock fixture unexpectedly closed a file");
  },
};

const module = await WebAssembly.compile(await readFile(wasmPath));
const instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: wasi });
memory = instance.exports.memory;
instance.exports._start();

assert.equal(stdout, "WASI-fixed-clocks: ok\n");
assert.deepEqual(requestedClocks, [
  [0, 1_000_000n],
  [1, 1_000_000n],
]);

const failingWasi = { ...wasi, clock_time_get: () => 5 };
const failingInstance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: failingWasi });
memory = failingInstance.exports.memory;
assert.throws(() => failingInstance.exports._start(), WebAssembly.RuntimeError);
