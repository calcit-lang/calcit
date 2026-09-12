import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [successWasmPath, failureWasmPath] = process.argv.slice(2);
if (successWasmPath == null || failureWasmPath == null) {
  throw new Error("usage: node scripts/test-wasi-wait-host.mjs <success.wasm> <failure.wasm>");
}

let memory;
let stdout = "";
const waits = [];
const view = () => new DataView(memory.buffer);
const bytes = () => new Uint8Array(memory.buffer);

const baseWasi = {
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
    for (let index = 0; index < iovsLength; index += 1) {
      const ptr = view().getUint32(iovsPtr + index * 8, true);
      const length = view().getUint32(iovsPtr + index * 8 + 4, true);
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
    throw new Error("wait fixture unexpectedly read a clock");
  },
  random_get() {
    throw new Error("wait fixture unexpectedly requested random bytes");
  },
  fd_prestat_get() {
    throw new Error("wait fixture unexpectedly requested a filesystem preopen");
  },
  fd_prestat_dir_name() {
    throw new Error("wait fixture unexpectedly requested a preopen name");
  },
  path_open() {
    throw new Error("wait fixture unexpectedly opened a filesystem path");
  },
  fd_filestat_get() {
    throw new Error("wait fixture unexpectedly requested file metadata");
  },
  fd_read() {
    throw new Error("wait fixture unexpectedly read a file");
  },
  fd_readdir() {
    throw new Error("wait fixture unexpectedly enumerated a directory");
  },
  fd_close() {
    throw new Error("wait fixture unexpectedly closed a file");
  },
};

const successWasi = {
  ...baseWasi,
  poll_oneoff(inputPtr, outputPtr, subscriptions, neventsPtr) {
    assert.equal(subscriptions, 1);
    assert.ok(inputPtr >= 16, "wait subscription must use reserved scratch memory");
    assert.equal(view().getUint8(inputPtr + 8), 0, "subscription must select a clock event");
    assert.equal(view().getUint32(inputPtr + 16, true), 1, "wait must use the monotonic clock");
    assert.equal(view().getBigUint64(inputPtr + 24, true), 4_294_967_295_000_000n);
    assert.equal(view().getBigUint64(inputPtr + 32, true), 1_000_000n);
    assert.equal(view().getUint16(inputPtr + 40, true), 0, "wait must use a relative timeout");
    const userdata = view().getBigUint64(inputPtr, true);
    waits.push(view().getBigUint64(inputPtr + 24, true));
    view().setBigUint64(outputPtr, userdata, true);
    view().setUint16(outputPtr + 8, 0, true);
    view().setUint8(outputPtr + 10, 0);
    view().setUint32(neventsPtr, 1, true);
    return 0;
  },
};

const successModule = await WebAssembly.compile(await readFile(successWasmPath));
const successInstance = await WebAssembly.instantiate(successModule, { wasi_snapshot_preview1: successWasi });
memory = successInstance.exports.memory;
successInstance.exports._start();
assert.equal(stdout, "WASI-fixed-wait: ok\n");
assert.deepEqual(waits, [4_294_967_295_000_000n], "zero wait must not call poll_oneoff");

stdout = "";
const failureWasi = { ...baseWasi, poll_oneoff: () => 5 };
const failureModule = await WebAssembly.compile(await readFile(failureWasmPath));
const failureInstance = await WebAssembly.instantiate(failureModule, { wasi_snapshot_preview1: failureWasi });
memory = failureInstance.exports.memory;
failureInstance.exports._start();
assert.equal(stdout, "WASI-wait-error: ok\n");
