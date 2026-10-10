import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const project = await mkdtemp(join(tmpdir(), "calcit-parse-boundary-"));
const snapshot = join(project, "calcit.cirru");
const options = { encoding: "utf8", stdio: "pipe", timeout: 60000, maxBuffer: 16 * 1024 * 1024 };
const run = (...args) => execFileSync(binary, [snapshot, ...args], options);
const mutate = operations => {
  const config = JSON.parse(run("query", "config", "--format", "json"));
  assert.deepEqual(config.diagnostics, []);
  const args = ["edit", "transaction", "--code", JSON.stringify(operations),
    "--expect-revision", config.revision, "--format", "json"];
  run(...args, "--dry-run");
  run(...args);
};

try {
  // Host output cannot be observed by a Calcit definition test. Rejecting
  // private EDN must leave error reporting to the caller, not log its payload.
  const runtime = await import(new URL("../lib/calcit.procs.mjs", import.meta.url));
  // Host calls bypass Calcit argument proofs; do not coerce a layout flag.
  for (const flag of [null, 0, "false"]) {
    assert.throws(() => runtime.format_cirru_edn(42, flag), /boolean inline option/);
  }
  const originalError = console.error;
  const errors = [];
  console.error = (...args) => errors.push(args);
  try {
    const shape = { version: 3, root: 0, fingerprint: "quiet-edn-parse", nodes: [{ kind: "nil" }] };
    for (const text of ["{", "nil", "42", "|fixture-password", "[] (invalid |fixture-password)"]) {
      assert.throws(() => runtime.parse_cirru_edn(text), /Unexpected data from EDN/);
      assert.throws(() => runtime.parse_cirru_edn_as(text, shape), /Unexpected data from EDN/);
    }
    assert.deepEqual(errors, [], "rejected EDN must not print raw credentials to console.error");
  } finally {
    console.error = originalError;
  }

  await copyFile("src/cirru/calcit-core.cirru", snapshot);
  await symlink(resolve("node_modules"), join(project, "node_modules"), "dir");
  // The optional layout flag is Bool even when the formatter is a local value.
  const original = await readFile(snapshot);
  for (const flag of ["nil", "0", "|false"]) {
    for (const [snippet, diagnostic] of [
      [`format-cirru-edn 42 ${flag}`, /W_PROC_ARG_TYPE_MISMATCH/],
      [`let ((format-text format-cirru-edn)) (format-text 42 ${flag})`, /W_LOCAL_FN_ARG_TYPE_MISMATCH/],
    ]) {
      for (const lint of ["0", "1"]) {
        const rejected = spawnSync(binary, [snapshot, "eval", snippet], {
          ...options, env: { ...process.env, CALCIT_LINT_CORE: lint },
        });
        if (rejected.error) throw rejected.error;
        const message = `${rejected.stdout}\n${rejected.stderr}`;
        assert.equal(rejected.status, 1, message);
        assert.match(message, diagnostic);
        assert.doesNotMatch(message, /internal compiler error/);
        assert.deepEqual(await readFile(snapshot), original);
      }
    }
  }
  run("test", "--tag", "parse-boundary", "--require-match");

  // Reuse the source tests verbatim in generated JS, not host-side decoder assertions.
  const trees = ["try-parse-cirru-edn-as", "try-decode-map-as"].flatMap(name => {
    const response = JSON.parse(run("query", "def", `calcit.core/${name}`, "--format", "json"));
    assert.deepEqual(response.diagnostics, []);
    const tests = response.data.tests.filter(test => test.tags.includes("parse-boundary"));
    assert.equal(tests.length, 2, `${name} must retain both deep success and rejection tests`);
    return tests.map(test => test.code);
  });
  for (const name of ["try-parse-json", "try-parse-cirru", "try-parse-cirru-edn", "try-parse-cirru-list", "parse-float"]) {
    const target = `calcit.core/${name}`;
    run("test", target, "--require-match");
    const response = JSON.parse(run("query", "def", target, "--format", "json"));
    assert.deepEqual(response.diagnostics, []);
    const tests = response.data.tests.filter(test => test.name === "result-method-contract" || test.tags.includes("parse-boundary"));
    assert.equal(tests.filter(test => test.name === "result-method-contract").length, 1,
      `${name} must retain the ordinary parsing method contract`);
    if (name === "try-parse-cirru-list") {
      assert.equal(tests.filter(test => test.name === "lexer-state-contract").length, 1,
        "Cirru parsing must retain quoted, empty-input and dollar/comma source fixtures");
    }
    trees.push(...tests.map(test => test.code));
  }
  const formatter = JSON.parse(run("query", "def", "calcit.core/format-cirru-edn", "--format", "json"));
  assert.deepEqual(formatter.diagnostics, []);
  const formatTests = formatter.data.tests.filter(test => test.name === "wasm-format-mode-contract");
  assert.equal(formatTests.length, 1, "formatter must retain its shared supported-mode contract");
  trees.push(...formatTests.map(test => test.code));
  mutate([
    ["edit", "add-ns", "calcit.parse-boundary"],
    ["edit", "def", "calcit.parse-boundary/main!", "--input-format", "json-ast", "--code",
      JSON.stringify(["defn", "main!", [], ...trees, "&unit"])],
    ["config", "set", "init-fn", "calcit.parse-boundary/main!"],
    ["config", "set", "reload-fn", "calcit.parse-boundary/main!"],
  ]);
  run("--check-only");
  run();
  const output = join(project, "js-out");
  run("--emit-path", output, "js");
  const generated = await import(pathToFileURL(join(output, "calcit.parse-boundary.mjs")).href);
  generated.main_$x_();
  console.log("Shared Calcit parsing tests passed native/JS method Result contracts, deep payload checks and structural failure paths");

  // Reuse the attached formatter assertions; untyped parsing is outside WASM.
  const exportName = "calcit.parse-boundary/format-modes";
  mutate([
    ["edit", "rm-def", "calcit.parse-boundary/main!"],
    ["edit", "def", exportName, "--input-format", "json-ast", "--code",
      JSON.stringify(["defwasm-export", "format-modes", [], ...formatTests.map(test => test.code), "&unit"])],
    ["edit", "schema", exportName, "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)"],
    ["config", "set", "init-fn", exportName],
    ["config", "set", "reload-fn", exportName],
  ]);
  const wasmArgs = ["wasm", snapshot, "--init-fn", exportName, "--reload-fn", exportName];
  const wasmOutput = join(project, "wasm-supported");
  execFileSync(binary, [...wasmArgs, "--emit-path", wasmOutput], options);
  const module = new WebAssembly.Module(await readFile(join(wasmOutput, "program.wasm")));
  const imports = {};
  for (const item of WebAssembly.Module.imports(module)) {
    assert.equal(item.kind, "function");
    (imports[item.module] ??= {})[item.name] = () => {
      throw new Error(`unexpected formatter host call: ${item.module}/${item.name}`);
    };
  }
  new WebAssembly.Instance(module, imports).exports["format-modes"]();

  // Capability failures need backend checks, not weakened shared assertions.
  for (const [name, args, flag] of [["non-inline", [], "false"], ["runtime", ["inline?"], "inline?"]]) {
    mutate([
      ["edit", "def", exportName, "--overwrite", "--input-format", "json-ast", "--code",
        JSON.stringify(["defwasm-export", "format-modes", args, ["format-cirru-edn", ["[]", "|a"], flag], "&unit"])],
      ["edit", "schema", exportName, "--input-format", "cirru", "--code",
        `quote $ :: 'Fn $ {} (:args $ [] ${args.length ? "'Bool" : ""}) (:return 'Unit)`],
    ]);
    const rejectedOutput = join(project, `wasm-${name}`);
    const rejected = spawnSync(binary, [...wasmArgs, "--emit-path", rejectedOutput], options);
    assert.notEqual(rejected.status, 0, `${name} container formatting must fail explicitly`);
    assert.match(rejected.stderr, /E_WASM_EDN_FORMAT_MODE/);
    await assert.rejects(readFile(join(rejectedOutput, "program.wasm")), { code: "ENOENT" });
  }
  console.log("Shared formatter assertions passed on WASM; unsupported container modes produced no artifact");
} finally {
  await rm(project, { recursive: true, force: true });
}
