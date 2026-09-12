import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [successWasmPath, errorWasmPath] = process.argv.slice(2);
if (successWasmPath == null || errorWasmPath == null) {
  throw new Error("usage: node scripts/test-wasi-read-dir-host.mjs <success.wasm> <error.wasm>");
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const preopenName = encoder.encode("/workspace");
let memory;
let stdout = [];
let readdirCalls = 0;

const view = () => new DataView(memory.buffer);
const bytes = () => new Uint8Array(memory.buffer);
const decode = (ptr, length) => decoder.decode(bytes().subarray(ptr, ptr + length));

const writeDirent = (ptr, next, name) => {
  const encoded = encoder.encode(name);
  view().setBigUint64(ptr, BigInt(next), true);
  view().setBigUint64(ptr + 8, 0n, true);
  view().setUint32(ptr + 16, encoded.length, true);
  view().setUint8(ptr + 20, 4);
  bytes().set(encoded, ptr + 24);
  return 24 + encoded.length;
};

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
  proc_exit(code) {
    throw new Error(`unexpected proc_exit(${code})`);
  },
  clock_time_get() {
    throw new Error("directory fixture unexpectedly requested a clock");
  },
  random_get() {
    throw new Error("directory fixture unexpectedly requested random bytes");
  },
  fd_prestat_get(fd, prestatPtr) {
    if (fd !== 3) return 8;
    view().setUint8(prestatPtr, 0);
    view().setUint32(prestatPtr + 4, preopenName.length, true);
    return 0;
  },
  fd_prestat_dir_name(fd, namePtr, nameLength) {
    assert.equal(fd, 3);
    assert.equal(nameLength, preopenName.length);
    bytes().set(preopenName, namePtr);
    return 0;
  },
  path_open(fd, _dirFlags, pathPtr, pathLength, oflags, rightsBase, rightsInheriting, _fdFlags, openedFdPtr) {
    assert.equal(fd, 3);
    assert.equal(decode(pathPtr, pathLength), "listing");
    assert.equal(oflags, 2);
    assert.equal(rightsBase, 1n << 14n);
    assert.equal(rightsInheriting, 1n << 14n);
    view().setUint32(openedFdPtr, 5, true);
    return 0;
  },
  fd_filestat_get() {
    throw new Error("directory fixture unexpectedly requested file metadata");
  },
  fd_read() {
    throw new Error("directory fixture unexpectedly read a file");
  },
  fd_readdir(fd, bufferPtr, bufferLength, cookie, usedPtr) {
    assert.equal(fd, 5);
    assert.equal(bufferLength, 64 * 1024);
    readdirCalls += 1;
    if (cookie === 0n) {
      let offset = 0;
      for (let next = 1; next <= 2621; next += 1) {
        offset += writeDirent(bufferPtr + offset, next, ".");
      }
      assert.equal(offset, bufferLength - 11);
      bytes().fill(0, bufferPtr + offset, bufferPtr + bufferLength);
      view().setUint32(usedPtr, bufferLength, true);
      return 0;
    }
    assert.equal(cookie, 2621n);
    let offset = 0;
    offset += writeDirent(bufferPtr + offset, 2622, "b.txt");
    offset += writeDirent(bufferPtr + offset, 2623, "子.txt");
    offset += writeDirent(bufferPtr + offset, 2624, "a.txt");
    offset += writeDirent(bufferPtr + offset, 2625, "..");
    view().setUint32(usedPtr, offset, true);
    return 0;
  },
  fd_write(fd, iovsPtr, iovsLength, writtenPtr) {
    assert.equal(fd, 1);
    assert.equal(iovsLength, 1);
    const ptr = view().getUint32(iovsPtr, true);
    const length = view().getUint32(iovsPtr + 4, true);
    stdout.push(...bytes().subarray(ptr, ptr + length));
    view().setUint32(writtenPtr, length, true);
    return 0;
  },
  fd_close(fd) {
    assert.equal(fd, 5);
    return 0;
  },
};

const successModule = await WebAssembly.compile(await readFile(successWasmPath));
const successInstance = await WebAssembly.instantiate(successModule, { wasi_snapshot_preview1: wasi });
memory = successInstance.exports.memory;
successInstance.exports._start();
assert.equal(decoder.decode(Uint8Array.from(stdout)), "WASI-read-dir: ok\n");
assert.equal(readdirCalls, 2, "directory adapter must continue from the last complete cookie");

const errorModule = await WebAssembly.compile(await readFile(errorWasmPath));
const assertMalformedDirectoryReturnsError = async (fdReaddir) => {
  stdout = [];
  const malformedWasi = { ...wasi, fd_readdir: fdReaddir };
  const errorInstance = await WebAssembly.instantiate(errorModule, { wasi_snapshot_preview1: malformedWasi });
  memory = errorInstance.exports.memory;
  errorInstance.exports._start();
  assert.equal(decoder.decode(Uint8Array.from(stdout)), "WASI-read-dir-error: ok\n");
};

await assertMalformedDirectoryReturnsError((fd, bufferPtr, _bufferLength, _cookie, usedPtr) => {
  assert.equal(fd, 5);
  view().setBigUint64(bufferPtr, 1n, true);
  view().setBigUint64(bufferPtr + 8, 0n, true);
  view().setUint32(bufferPtr + 16, 4097, true);
  view().setUint32(usedPtr, 24, true);
  return 0;
});

await assertMalformedDirectoryReturnsError((fd, bufferPtr, _bufferLength, _cookie, usedPtr) => {
  assert.equal(fd, 5);
  view().setBigUint64(bufferPtr, 1n, true);
  view().setBigUint64(bufferPtr + 8, 0n, true);
  view().setUint32(bufferPtr + 16, 5, true);
  bytes().set(encoder.encode("ab"), bufferPtr + 24);
  view().setUint32(usedPtr, 26, true);
  return 0;
});

await assertMalformedDirectoryReturnsError((fd, bufferPtr, _bufferLength, _cookie, usedPtr) => {
  assert.equal(fd, 5);
  view().setBigUint64(bufferPtr, 1n, true);
  view().setBigUint64(bufferPtr + 8, 0n, true);
  view().setUint32(bufferPtr + 16, 1, true);
  view().setUint8(bufferPtr + 24, 0xff);
  view().setUint32(usedPtr, 25, true);
  return 0;
});

await assertMalformedDirectoryReturnsError((fd, bufferPtr, _bufferLength, _cookie, usedPtr) => {
  assert.equal(fd, 5);
  view().setBigUint64(bufferPtr, 1n, true);
  view().setBigUint64(bufferPtr + 8, 0n, true);
  view().setUint32(bufferPtr + 16, 0, true);
  view().setUint32(usedPtr, 24, true);
  return 0;
});
