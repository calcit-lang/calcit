import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [wasmPath] = process.argv.slice(2);
if (wasmPath == null) {
  throw new Error("usage: node scripts/test-wasi-filesystem-host.mjs <program.wasm>");
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const preopenNames = new Map([
  [3, encoder.encode("/workspace")],
  [4, encoder.encode(".")],
]);
const input = encoder.encode("WASI-file: 你好");
const output = [];
const openedPaths = [];
let inputOffset = 0;
let readCalls = 0;
let writeCalls = 0;
let memory;

const view = () => new DataView(memory.buffer);
const bytes = () => new Uint8Array(memory.buffer);
const decode = (ptr, length) => decoder.decode(bytes().subarray(ptr, ptr + length));

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
    throw new Error("filesystem fixture unexpectedly requested a clock");
  },
  random_get() {
    throw new Error("filesystem fixture unexpectedly requested random bytes");
  },
  fd_prestat_get(fd, prestatPtr) {
    const preopenName = preopenNames.get(fd);
    if (preopenName == null) return 8;
    view().setUint8(prestatPtr, 0);
    view().setUint32(prestatPtr + 4, preopenName.length, true);
    return 0;
  },
  fd_prestat_dir_name(fd, namePtr, nameLength) {
    const preopenName = preopenNames.get(fd);
    assert.ok(preopenName != null);
    assert.equal(nameLength, preopenName.length);
    bytes().set(preopenName, namePtr);
    return 0;
  },
  path_open(fd, _dirFlags, pathPtr, pathLength, oflags, _rightsBase, _rightsInheriting, _fdFlags, openedFdPtr) {
    assert.equal(fd, 3);
    const path = decode(pathPtr, pathLength);
    openedPaths.push(path);
    if (path === "input.txt" && oflags === 0) {
      view().setUint32(openedFdPtr, 5, true);
      return 0;
    }
    if (path === "output.txt" && oflags === 9) {
      view().setUint32(openedFdPtr, 6, true);
      return 0;
    }
    return 44;
  },
  fd_filestat_get(fd, statPtr) {
    assert.equal(fd, 5);
    view().setBigUint64(statPtr + 32, BigInt(input.length), true);
    return 0;
  },
  fd_read(fd, iovsPtr, iovsLength, readPtr) {
    assert.equal(fd, 5);
    assert.equal(iovsLength, 1);
    const ptr = view().getUint32(iovsPtr, true);
    const requested = view().getUint32(iovsPtr + 4, true);
    const length = Math.min(requested, 2, input.length - inputOffset);
    bytes().set(input.subarray(inputOffset, inputOffset + length), ptr);
    inputOffset += length;
    readCalls += 1;
    view().setUint32(readPtr, length, true);
    return 0;
  },
  fd_write(fd, iovsPtr, iovsLength, writtenPtr) {
    assert.equal(iovsLength, 1);
    const ptr = view().getUint32(iovsPtr, true);
    const requested = view().getUint32(iovsPtr + 4, true);
    if (fd === 1) {
      const chunk = bytes().slice(ptr, ptr + requested);
      output.push(...chunk);
      view().setUint32(writtenPtr, requested, true);
      return 0;
    }
    assert.equal(fd, 6);
    const length = Math.min(requested, 3);
    output.push(...bytes().subarray(ptr, ptr + length));
    writeCalls += 1;
    view().setUint32(writtenPtr, length, true);
    return 0;
  },
  fd_close(fd) {
    assert.ok(fd === 5 || fd === 6);
    return 0;
  },
};

const module = await WebAssembly.compile(await readFile(wasmPath));
const instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: wasi });
memory = instance.exports.memory;
instance.exports._start();

assert.deepEqual(openedPaths, ["input.txt", "output.txt"]);
assert.ok(readCalls > 1, "read adapter must handle partial fd_read progress");
assert.ok(writeCalls > 1, "write adapter must handle partial fd_write progress");
assert.equal(decoder.decode(Uint8Array.from(output)), "WASI-written: 好WASI-filesystem: ok\n");

const zeroReadWasi = {
  ...wasi,
  fd_read(_fd, _iovsPtr, _iovsLength, readPtr) {
    view().setUint32(readPtr, 0, true);
    return 0;
  },
};
const zeroReadInstance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: zeroReadWasi });
memory = zeroReadInstance.exports.memory;
assert.throws(() => zeroReadInstance.exports._start(), /unexpected proc_exit\(1\)/);

inputOffset = 0;
const zeroWriteWasi = {
  ...wasi,
  fd_write(fd, iovsPtr, iovsLength, writtenPtr) {
    if (fd === 6) {
      view().setUint32(writtenPtr, 0, true);
      return 0;
    }
    return wasi.fd_write(fd, iovsPtr, iovsLength, writtenPtr);
  },
};
const zeroWriteInstance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: zeroWriteWasi });
memory = zeroWriteInstance.exports.memory;
assert.throws(() => zeroWriteInstance.exports._start(), /unexpected proc_exit\(1\)/);
