import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [wasmPath] = process.argv.slice(2);
if (wasmPath == null) {
  throw new Error("usage: node scripts/test-wasi-random-host.mjs <program.wasm>");
}

let memory;
let stdout = "";
let randomDataPtr;
const view = () => new DataView(memory.buffer);
const bytes = () => new Uint8Array(memory.buffer);

const wasi = {
  args_sizes_get(argcPtr, argvSizePtr) {
    view().setUint32(argcPtr, 0, true);
    view().setUint32(argvSizePtr, 0, true);
    return 0;
  },
  args_get: () => 0,
  environ_sizes_get(countPtr, sizePtr) {
    view().setUint32(countPtr, 0, true);
    view().setUint32(sizePtr, 0, true);
    return 0;
  },
  environ_get: () => 0,
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
  clock_time_get() {
    throw new Error("random fixture unexpectedly requested a clock");
  },
  random_get(ptr, length) {
    assert.equal(length, 4);
    randomDataPtr = ptr;
    bytes().set([0, 127, 128, 255], ptr);
    return 0;
  },
  fd_prestat_get() {
    throw new Error("random fixture unexpectedly requested a filesystem preopen");
  },
  fd_prestat_dir_name() {
    throw new Error("random fixture unexpectedly requested a preopen name");
  },
  path_open() {
    throw new Error("random fixture unexpectedly opened a filesystem path");
  },
  fd_filestat_get() {
    throw new Error("random fixture unexpectedly requested file metadata");
  },
  fd_read() {
    throw new Error("random fixture unexpectedly read a file");
  },
  fd_readdir() {
    throw new Error("random fixture unexpectedly read a directory");
  },
  fd_close() {
    throw new Error("random fixture unexpectedly closed a file");
  },
};

const module = await WebAssembly.compile(await readFile(wasmPath));
const instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: wasi });
memory = instance.exports.memory;
instance.exports._start();

assert.equal(stdout, "secure-random-fixed: ok\n");
assert.equal(view().getFloat64(randomDataPtr - 8, true), 4);
assert.deepEqual(Array.from(bytes().subarray(randomDataPtr, randomDataPtr + 4)), [0, 127, 128, 255]);

const failingWasi = { ...wasi, random_get: () => 5 };
const failingInstance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: failingWasi });
memory = failingInstance.exports.memory;
assert.throws(() => failingInstance.exports._start(), /unexpected proc_exit\(1\)/);
