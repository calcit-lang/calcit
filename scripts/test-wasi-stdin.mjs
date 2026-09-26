import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { closeSync, openSync } from "node:fs";
import { copyFile, mkdtemp, readFile, rm, symlink } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const wasmtime = process.env.WASMTIME_CLI;
assert.ok(wasmtime, "WASMTIME_CLI must select a real WASI 0.3 host; this acceptance cannot silently skip Component");
const source = resolve("examples/wasi-command/calcit.cirru");
const original = await readFile(source);
const temp = await mkdtemp(resolve("target/stdin-pipeline-"));
const run = (command, args, options = {}) => {
  const result = spawnSync(command, args, { encoding: "utf8", timeout: 120000, maxBuffer: 16 * 1024 * 1024, ...options });
  assert.ifError(result.error);
  return result;
};
const calcit = (...args) => {
  const result = run(binary, args);
  assert.equal(result.status, 0, result.stderr);
  return result.stdout;
};
try {
  const snapshot = join(temp, "calcit.cirru");
  await copyFile(source, snapshot);
  await symlink(resolve("node_modules"), join(temp, "node_modules"), "dir");
  calcit("docs", "agents", "--contract");
  calcit(snapshot, "test", "--tag", "wasi", "--require-match");
  const entry = [snapshot, "--init-fn", "app.main/manifest-stdin-main!"];
  calcit(...entry, "--check-only");
  const type = JSON.parse(calcit(snapshot, "query", "type-at", "calcit.core/read-stdin-text", "--path", "code", "--format", "json"));
  assert.match(type.data.inferred_type, /'(?:calcit.core\/)?Result 'String 'String/);
  assert.doesNotMatch(type.data.inferred_type, /Dynamic/);
  const js = join(temp, "js");
  calcit(...entry, "--emit-path", js, "js");
  const component = join(temp, "component");
  const preview1 = run(binary, [...entry, "wasi", "--emit-path", join(temp, "preview1")]);
  assert.notEqual(preview1.status, 0);
  assert.match(preview1.stderr, /read-stdin-text requires.*--boundary component/);
  calcit(...entry, "wasi", "--boundary", "component", "--emit-path", component);
  const wasmArgs = ["run", "-S", "p3", "-W", "component-model-async-stackful=y", "-W", "component-model-more-async-builtins=y"];
  const backends = [
    ["native", binary, entry],
    ["node", process.execPath, [resolve("examples/wasi-command/run-stdin.mjs"), join(js, "app.main.mjs")]],
    ["component", wasmtime, [...wasmArgs, join(component, "program.wasm")]],
  ];
  const input = await readFile("examples/wasi-command/manifest-input.cirru", "utf8");
  // echo/println append one newline to the formatter's canonical document.
  const expected = (await readFile("examples/wasi-command/manifest-output.cirru", "utf8")) + "\n";
  const limit = 4 * 1024 * 1024;
  const boundaryInput = "\n".repeat(65535 - input.indexOf("api")) + input.replace("api", "服务😀");
  const cases = [
    ["valid", Buffer.from(input), 0, expected],
    ["unicode", Buffer.from(input.replace("api", "服务😀")), 0, expected.replace("|prod-api", JSON.stringify("|prod-服务😀"))],
    ["empty", Buffer.alloc(0), 65, ""],
    ["invalid-business", Buffer.from(input.replace("(:revision 3)", "(:revision -1)")), 65, ""],
    ["invalid-utf8", Buffer.from([0xc0, 0xaf]), 66, ""],
    ["surrogate-utf8", Buffer.from([0xed, 0xa0, 0x80]), 66, ""],
    ["truncated-utf8", Buffer.from([0xf0, 0x9f, 0x98]), 66, ""],
    ["oversized", Buffer.alloc(limit + 1, 32), 66, ""],
  ];
  for (const [name, command, args] of backends) {
    for (const [caseName, bytes, status, stdout] of cases) {
      const result = run(command, args, { input: bytes });
      assert.equal(result.status, status, `${name}/${caseName}: ${result.stderr}`);
      assert.equal(result.stdout, stdout, `${name}/${caseName}: stdout must contain only the business result`);
    }
  }

  // The existing WASM EDN parser has its own 64 KiB limit. Test the 4 MiB
  // reader contract directly rather than weakening that parser boundary.
  calcit(snapshot, "query", "def", "app.main/manifest-stdin-main!", "--raw");
  calcit(snapshot, "edit", "def", "app.main/manifest-stdin-main!", "--overwrite", "--input-format", "cirru", "--code",
    "quote $ defn manifest-stdin-main! ()\n  match (read-stdin-text) ((:err message) (fail! 66 message)) ((:ok text) (echo text))\n  assert= (%ok |) (read-stdin-text)\n  , &unit");
  const rawJs = join(temp, "raw-js");
  const rawComponent = join(temp, "raw-component");
  calcit(...entry, "--emit-path", rawJs, "js");
  calcit(...entry, "wasi", "--boundary", "component", "--emit-path", rawComponent);
  const rawBackends = [
    ["native", binary, entry],
    ["node", process.execPath, [resolve("examples/wasi-command/run-stdin.mjs"), join(rawJs, "app.main.mjs")]],
    ["component", wasmtime, [...wasmArgs, join(rawComponent, "program.wasm")]],
  ];
  for (const [name, command, args] of rawBackends) {
    if (process.platform !== "win32") {
      const fd = openSync(temp, "r");
      try {
        const failure = run(command, args, { stdio: [fd, "pipe", "pipe"] });
        assert.equal(failure.status, 66, `${name}: host read failure must be Result.err, not EOF success: ${failure.stderr}`);
        assert.equal(failure.stdout, "");
      } finally {
        closeSync(fd);
      }
    }
    for (const [caseName, bytes, status, stdout] of [
      ["empty", Buffer.alloc(0), 0, "\n"],
      ["unicode", Buffer.from("服务😀"), 0, "服务😀\n"],
      ["split-utf8", Buffer.from(boundaryInput), 0, boundaryInput + "\n"],
      ["bom", Buffer.from("\ufeffa"), 0, "\ufeffa\n"],
      ["limit", Buffer.alloc(limit, 120), 0, "x".repeat(limit) + "\n"],
      ["oversized", Buffer.alloc(limit + 1, 120), 66, ""],
    ]) {
      const result = run(command, args, { input: bytes });
      assert.equal(result.status, status, `${name}/raw-${caseName}: ${result.stderr}`);
      assert.equal(result.stdout, stdout, `${name}/raw-${caseName}: text must round-trip exactly`);
    }
  }

  // Reuse definition-owned semantic tests, not a second host-side business implementation.
  const data = JSON.parse(calcit("cirru", "parse-edn", "--file", snapshot));
  const tests = data[":files"]["'app.main"].defs["'process-manifest"].tests;
  assert.equal(tests.length, 3);
  calcit(snapshot, "edit", "def", "app.main/shared-tests!", "--input-format", "json-ast", "--code",
    JSON.stringify(["defn", "shared-tests!", [], ...tests.map(test => test.code.__edn_quote), "&unit"]));
  calcit(snapshot, "edit", "schema", "app.main/shared-tests!", "--input-format", "cirru", "--code",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)");
  const shared = [snapshot, "--init-fn", "app.main/shared-tests!"];
  calcit(...shared);
  const sharedJs = join(temp, "shared-js");
  calcit(...shared, "--emit-path", sharedJs, "js");
  const app = await import(pathToFileURL(join(sharedJs, "app.main.mjs")).href);
  app.shared_tests_$x_();
  const sharedComponent = join(temp, "shared-component");
  calcit(...shared, "wasi", "--boundary", "component", "--emit-path", sharedComponent);
  const result = run(wasmtime, [...wasmArgs, join(sharedComponent, "program.wasm")]);
  assert.equal(result.status, 0, result.stderr);

  // Host boundary failures cannot be constructed by ordinary Calcit business code.
  const runtimeUrl = pathToFileURL(resolve("lib/calcit.procs.mjs")).href;
  const runtime = await import(runtimeUrl);
  const { Result } = await import(pathToFileURL(join(sharedJs, "calcit.core.mjs")).href);
  for (const read of [undefined, () => "already decoded", () => { throw new Error("read failure"); },
    () => new Uint8Array([0xff]), () => new Uint8Array(limit + 1)]) {
    globalThis.__calcit_injections__ = { read_stdin: read };
    const value = runtime._$n_read_stdin_text(Result, "stdin boundary failure");
    assert.equal(value.tag.value, "err");
    assert.equal(value.extra[0], "stdin boundary failure");
  }
  globalThis.__calcit_injections__ = { read_stdin: () => new Uint8Array([0xef, 0xbb, 0xbf, 0x61]) };
  assert.equal(runtime._$n_read_stdin_text(Result, "error").extra[0], "\ufeffa", "BOM must not silently disappear");
  delete globalThis.__calcit_injections__;
  const browser = run(process.execPath, ["--input-type=module", "-e", `
    import assert from "node:assert/strict";
    globalThis.process = undefined;
    globalThis.__calcit_injections__ = { read_stdin() { throw new Error("browser must not call this"); } };
    const runtime = await import(${JSON.stringify(runtimeUrl)});
    const value = runtime._$n_read_stdin_text(null, "host failure");
    assert.equal(value.tag.value, "err");
    assert.match(value.extra[0], /unsupported in browser/);
  `]);
  assert.equal(browser.status, 0, browser.stderr);
  assert.deepEqual(await readFile(source), original, "acceptance must not rewrite source");
  console.log("stdin pipeline: 42 native/Node/Component input cases, host failures, repeated EOF and shared Calcit business tests passed");
} finally {
  await rm(temp, { recursive: true, force: true });
}
