import fs from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

// The runtime owns decoding; this host supplies at most the requested bytes.
globalThis.__calcit_injections__ = {
  read_stdin(maximumBytes) {
    const buffer = Buffer.alloc(maximumBytes);
    let length = 0;
    while (length < maximumBytes) {
      let count;
      try {
        count = fs.readSync(0, buffer, length, Math.min(65536, maximumBytes - length), null);
      } catch (error) {
        if (error.code === "EINTR") continue;
        throw error;
      }
      if (count === 0) break;
      length += count;
    }
    return buffer.subarray(0, length);
  },
};

const modulePath = process.argv[2];
if (!modulePath) throw new Error("usage: node run-stdin.mjs <generated app.main.mjs>");
const app = await import(pathToFileURL(resolve(modulePath)).href);
app.manifest_stdin_main_$x_();
