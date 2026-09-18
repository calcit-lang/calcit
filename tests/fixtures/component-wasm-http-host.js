const fs = require("fs");
const { spawnSync } = require("child_process");

const bytes = fs.readFileSync(process.argv[2]);
const allowedOrigin = process.argv[3];
const wasmModule = new WebAssembly.Module(bytes);
let instance;
const completions = [];
const HTTP_BODY = Object.freeze({ bytes: 0, empty: 1, text: 2 });
const HTTP_METHOD = Object.freeze({ get: 1 });
const HTTP_ERROR = Object.freeze({ capabilityDenied: 0, invalidRequest: 1, responseTooLarge: 2, transport: 3, unsupported: 4 });

const allocateBytes = bytes => {
  if (bytes.length === 0) return [0, 0];
  const ptr = instance.exports.cabi_realloc(0, 0, 1, bytes.length);
  new Uint8Array(instance.exports.memory.buffer, ptr, bytes.length).set(bytes);
  return [ptr, bytes.length];
};
const allocateText = text => allocateBytes(Buffer.from(text, "utf8"));
const readText = (ptr, len) => Buffer.from(instance.exports.memory.buffer, ptr, len).toString("utf8");
const readHeaders = (ptr, len) => {
  const memory = new DataView(instance.exports.memory.buffer);
  return Array.from({ length: len }, (_, index) => {
    const offset = ptr + index * 16;
    return [
      readText(memory.getUint32(offset, true), memory.getUint32(offset + 4, true)),
      readText(memory.getUint32(offset + 8, true), memory.getUint32(offset + 12, true)),
    ];
  });
};

const writeError = (outPtr, discriminant, payload) => {
  const memory = new DataView(instance.exports.memory.buffer);
  memory.setUint8(outPtr, 1);
  memory.setUint8(outPtr + 8, discriminant);
  if (discriminant === HTTP_ERROR.responseTooLarge) {
    memory.setBigUint64(outPtr + 16, BigInt(payload), true);
  } else {
    const [ptr, len] = allocateText(payload);
    memory.setUint32(outPtr + 16, ptr, true);
    memory.setUint32(outPtr + 20, len, true);
  }
};

const writeResponse = (outPtr, response) => {
  const memory = new DataView(instance.exports.memory.buffer);
  const body = Buffer.from(response.body, "base64");
  const contentType = response.contentType.split(";", 1)[0].trim().toLowerCase();
  const textual = contentType.startsWith("text/") || contentType === "application/json" || contentType.endsWith("+json");
  const [bodyPtr, bodyLen] = allocateBytes(body);
  const headersPtr = response.headers.length === 0
    ? 0
    : instance.exports.cabi_realloc(0, 0, 4, response.headers.length * 16);
  for (const [index, [name, value]] of response.headers.entries()) {
    const [namePtr, nameLen] = allocateText(name);
    const [valuePtr, valueLen] = allocateText(value);
    const offset = headersPtr + index * 16;
    memory.setUint32(offset, namePtr, true);
    memory.setUint32(offset + 4, nameLen, true);
    memory.setUint32(offset + 8, valuePtr, true);
    memory.setUint32(offset + 12, valueLen, true);
  }
  memory.setUint8(outPtr, 0);
  memory.setUint8(outPtr + 8, textual ? HTTP_BODY.text : HTTP_BODY.bytes);
  memory.setUint32(outPtr + 12, bodyPtr, true);
  memory.setUint32(outPtr + 16, bodyLen, true);
  memory.setUint32(outPtr + 20, headersPtr, true);
  memory.setUint32(outPtr + 24, response.headers.length, true);
  memory.setUint16(outPtr + 28, response.status, true);
};

const fetchScript = `
fetch(process.argv[1], { method: process.argv[2], redirect: "error", signal: AbortSignal.timeout(5000) }).then(async response => {
  const limit = BigInt(process.argv[3]);
  let observed = 0n;
  const chunks = [];
  for await (const chunk of response.body) {
    observed += BigInt(chunk.byteLength);
    if (observed > limit) {
      process.stdout.write(JSON.stringify({ tooLarge: observed.toString() }));
      return;
    }
    chunks.push(Buffer.from(chunk));
  }
  const body = Buffer.concat(chunks);
  process.stdout.write(JSON.stringify({
    status: response.status,
    contentType: response.headers.get("content-type") || "",
    headers: Array.from(response.headers.entries()),
    body: body.toString("base64"),
  }));
}).catch(error => { console.error(error); process.exitCode = 1; });`;

const http = {
  "[async-lower]request": (argsPtr, outPtr) => {
    const memory = new DataView(instance.exports.memory.buffer);
    const bodyKind = memory.getUint8(argsPtr);
    const headersLength = memory.getUint32(argsPtr + 16, true);
    const maxResponseBytes = memory.getBigUint64(argsPtr + 24, true);
    const methodKind = memory.getUint8(argsPtr + 32);
    const url = readText(memory.getUint32(argsPtr + 36, true), memory.getUint32(argsPtr + 40, true));
    if (bodyKind !== HTTP_BODY.empty || headersLength !== 0 || methodKind !== HTTP_METHOD.get) {
      writeError(outPtr, HTTP_ERROR.invalidRequest, "JS fixture accepts only GET with an empty body and no headers");
      return 2;
    }
    let origin;
    try {
      origin = new URL(url).origin;
    } catch (_error) {
      writeError(outPtr, HTTP_ERROR.invalidRequest, "invalid request URL");
      return 2;
    }
    if (origin !== allowedOrigin) {
      writeError(outPtr, HTTP_ERROR.capabilityDenied, `origin is not granted: ${origin}`);
      return 2;
    }
    const fetched = spawnSync(process.execPath, ["-e", fetchScript, url, "GET", maxResponseBytes.toString()], { encoding: "utf8" });
    if (fetched.status !== 0) {
      writeError(outPtr, HTTP_ERROR.transport, "HTTP transport failed");
      return 2;
    }
    const response = JSON.parse(fetched.stdout);
    if (response.tooLarge !== undefined) {
      writeError(outPtr, HTTP_ERROR.responseTooLarge, response.tooLarge);
      return 2;
    }
    writeResponse(outPtr, response);
    return 2;
  },
};

const canonical = {
  "[task-return]call-host-http-request": (resultKind, payloadKind, payload, payloadLength, headersPtr, headersLength, status) => {
    if (resultKind === 0) {
      completions.push({
        kind: "ok",
        bodyKind: payloadKind,
        body: readText(Number(payload), payloadLength),
        headers: readHeaders(headersPtr, headersLength),
        status,
      });
    } else if (payloadKind === HTTP_ERROR.responseTooLarge) {
      completions.push({ kind: "error", errorKind: payloadKind, observed: Number(payload) });
    } else {
      completions.push({ kind: "error", errorKind: payloadKind, message: readText(Number(payload), payloadLength) });
    }
  },
  "[waitable-set-new]": () => { throw new Error("immediate HTTP should not allocate a waitable set"); },
  "[waitable-join]": () => { throw new Error("immediate HTTP should not join a waitable set"); },
  "[waitable-set-wait]": () => { throw new Error("immediate HTTP should not wait"); },
  "[subtask-drop]": () => { throw new Error("immediate HTTP should not drop a subtask"); },
  "[waitable-set-drop]": () => { throw new Error("immediate HTTP should not drop a waitable set"); },
  "[task-return]call-host-combine": () => {},
  "[task-return]call-host-flag": () => {},
  "[task-return]call-host-load": () => {},
};
const unusedHost = {
  "[async-lower]combine": () => { throw new Error("unexpected combine call"); },
  "[async-lower]flag": () => { throw new Error("unexpected flag call"); },
  "[async-lower]load": () => { throw new Error("unexpected load call"); },
};

WebAssembly.instantiate(wasmModule, {
  host: unusedHost,
  "calcit:wasi-http/client": http,
  "$root": canonical,
  "[export]$root": canonical,
}).then(result => {
  instance = result;
  const invoke = (url, maxResponseBytes) => {
    const [urlPtr, urlLen] = allocateText(url);
    instance.exports["[async-lift-stackful]call-host-http-request"](
      HTTP_BODY.empty,
      0,
      0,
      0,
      0,
      BigInt(maxResponseBytes),
      HTTP_METHOD.get,
      urlPtr,
      urlLen,
    );
  };
  invoke(`${allowedOrigin}/ok`, 64);
  invoke("::malformed-url", 64);
  invoke("http://127.0.0.1:1/denied", 64);
  invoke(`${allowedOrigin}/redirect`, 64);
  invoke(`${allowedOrigin}/large`, 8);
  const expected = [
    {
      kind: "ok",
      bodyKind: HTTP_BODY.text,
      body: '{"ok":true}',
      headers: [
        ["connection", "close"],
        ["content-length", "11"],
        ["content-type", "application/json"],
      ],
      status: 200,
    },
    { kind: "error", errorKind: HTTP_ERROR.invalidRequest, message: "invalid request URL" },
    { kind: "error", errorKind: HTTP_ERROR.capabilityDenied, message: "origin is not granted: http://127.0.0.1:1" },
    { kind: "error", errorKind: HTTP_ERROR.transport, message: "HTTP transport failed" },
    { kind: "error", errorKind: HTTP_ERROR.responseTooLarge, observed: 9 },
  ];
  if (JSON.stringify(completions) !== JSON.stringify(expected)) {
    throw new Error(`unexpected HTTP completions: ${JSON.stringify(completions)}`);
  }
}).catch(error => {
  console.error(error);
  process.exitCode = 1;
});
