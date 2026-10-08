#!/usr/bin/env node
// Replay definition-attached `:tests` of a Snapshot (default: the bundled
// core) on native, generated JS and WASM (plus an opt-in WASI 0.3 command),
// from temporary Snapshots. Every selected test runs on every backend unless
// scripts/core-tests-exclusions.cirru names it (or one of its tags) for that
// backend with a reason.
//
// Usage:
//   node scripts/run-core-tests.mjs [--backend native,js,wasm[,wasi]] [--tag t]...
//     [--exclude-tag t]... [--name test-name] [--target ns|ns/def|test-id]...
//     [--exclusions file] [--snapshot file] [--report-unexpected-pass] [--support-matrix file]
//     [--results-json file]
//
// Native is the reference: the println trace of each test on the other
// backends must match its native trace. A run that selects zero tests fails.

import { execFileSync, spawnSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { writeCirruCode } from "@cirru/writer.ts";
import { hasNativeParity, markerProtocolError, parseBackendSelection, parseMarkers, parseWasmDiagnosticTable } from "./core-test-protocol.mjs";

// `wasi` (a WASI 0.3 command run by $WASMTIME_CLI) is opt-in via --backend.
const BACKENDS = ["native", "js", "wasm", "wasi"];
const DEFAULT_BACKENDS = ["native", "js", "wasm"];
const REPLAY_NS = "calcit.core-test-replay";

const parseArgs = (argv) => {
  const options = {
    backends: [...DEFAULT_BACKENDS], tags: [], excludeTags: [], names: [], targets: [],
    snapshot: "src/cirru/calcit-core.cirru", exclusions: "scripts/core-tests-exclusions.cirru",
    supportMatrix: "docs/installation/wasm-support.md",
    reportUnexpectedPass: false, resultsJson: undefined,
  };
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    const value = () => {
      if (i + 1 >= argv.length) throw new Error(`${flag} expects a value`);
      return argv[++i];
    };
    switch (flag) {
      case "--backend": options.backends = parseBackendSelection(value(), BACKENDS); break;
      case "--tag": options.tags.push(value()); break;
      case "--exclude-tag": options.excludeTags.push(value()); break;
      case "--name": options.names.push(value()); break;
      case "--target": options.targets.push(value()); break;
      case "--snapshot": options.snapshot = value(); break;
      case "--exclusions": options.exclusions = value(); break;
      case "--support-matrix": options.supportMatrix = value(); break;
      case "--report-unexpected-pass": options.reportUnexpectedPass = true; break;
      case "--results-json": options.resultsJson = value(); break;
      default: throw new Error(`unknown argument: ${flag}`);
    }
  }
  for (const backend of options.backends) {
    if (!BACKENDS.includes(backend)) throw new Error(`unknown backend: ${backend}`);
  }
  // Removing an exclusion requires a successful native reference, even for a narrowed run.
  if (options.reportUnexpectedPass && !options.backends.includes("native")) options.backends.unshift("native");
  if (options.backends.includes("wasi") && !process.env.WASMTIME_CLI) {
    throw new Error("--backend wasi needs WASMTIME_CLI pointing at a Wasmtime executable");
  }
  return options;
};

const options = parseArgs(process.argv.slice(2));
const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const execOptions = { encoding: "utf8", stdio: "pipe", timeout: 300000, maxBuffer: 256 * 1024 * 1024 };
const run = (...args) => execFileSync(binary, args, execOptions);
const attempt = (...args) => spawnSync(binary, args, execOptions);
const corePath = resolve(options.snapshot);

const ednTags = (value) => (value?.__edn_set ?? []).map((tag) => tag.__edn_tag ?? tag);
const stripNs = (name) => name.replace(/^'/, "");

// --- selection -------------------------------------------------------------

const source = JSON.parse(run("cirru", "parse-edn", "--file", corePath));
const allTests = [];
for (const [nsKey, file] of Object.entries(source[":files"])) {
  const ns = stripNs(nsKey);
  for (const [defKey, entry] of Object.entries(file.defs ?? {})) {
    const def = stripNs(defKey);
    for (const test of entry.tests ?? []) {
      allTests.push({
        id: `${ns}/${def}#${test.name}`, ns, def, name: test.name,
        tags: ednTags(test.tags), code: test.code.__edn_quote,
      });
    }
  }
}

const selected = allTests.filter((test) =>
  options.tags.every((tag) => test.tags.includes(tag))
  && !options.excludeTags.some((tag) => test.tags.includes(tag))
  && (options.names.length === 0 || options.names.includes(test.name))
  && (options.targets.length === 0 || options.targets.some((target) =>
    target === test.ns || target === `${test.ns}/${test.def}` || target === test.id)));
if (selected.length === 0) {
  console.error("No definition tests matched the selection; refusing to report an empty pass.");
  process.exit(1);
}
// The `calcit test` synthetic name keeps core-only test permissions intact.
selected.forEach((test, index) => {
  test.index = index;
  test.fn = `&calcit:test:${index}`;
  test.jsName = `_$n_calcit_$o_test_$o_${index}`;
});

// --- exclusions ------------------------------------------------------------

const exclusionData = JSON.parse(run("cirru", "parse-edn", "--file", resolve(options.exclusions)));
const exclusionProblems = [];
const exclusionFor = {};
for (const backend of BACKENDS) {
  const section = exclusionData[`:${backend}`] ?? {};
  const tags = section[":tags"] ?? {};
  const tests = section[":tests"] ?? {};
  for (const [key, reason] of [...Object.entries(tags), ...Object.entries(tests)]) {
    if (typeof reason !== "string" || reason.trim() === "") {
      exclusionProblems.push(`${backend} ${key}: every exclusion needs a non-empty reason`);
    }
  }
  const tagReasons = new Map(Object.entries(tags).map(([tag, reason]) => [tag.replace(/^:/, ""), reason]));
  const testReasons = new Map(Object.entries(tests));
  for (const id of testReasons.keys()) {
    // Entries of namespaces outside this Snapshot belong to another selection.
    const known = Object.hasOwn(source[":files"], `'${id.split("/")[0]}`);
    if (known && !allTests.some((test) => test.id === id)) exclusionProblems.push(`${backend} ${id}: no such test (stale exclusion)`);
  }
  exclusionFor[backend] = (test) => {
    if (testReasons.has(test.id)) return testReasons.get(test.id);
    const tag = test.tags.find((item) => tagReasons.has(item));
    return tag === undefined ? undefined : `tag :${tag}: ${tagReasons.get(tag)}`;
  };
}
// The WASM support matrix documents every unsupported proc and diagnostic code
// that an exclusion reason cites, so the two lists cannot drift apart.
const matrixText = readFileSync(resolve(options.supportMatrix), "utf8");
const matrixCodes = parseWasmDiagnosticTable(matrixText);
const matrixProcs = new Set((/```text wasm-unsupported-procs\n([\s\S]*?)```/.exec(matrixText)?.[1] ?? "")
  .split("\n").map((line) => line.trim()).filter((line) => line !== ""));
for (const backend of ["wasm", "wasi"]) {
  const section = exclusionData[`:${backend}`] ?? {};
  for (const reason of [...Object.values(section[":tags"] ?? {}), ...Object.values(section[":tests"] ?? {})]) {
    if (typeof reason !== "string") continue;
    for (const [, proc] of reason.matchAll(/unsupported proc in WASM: (\S+)/g)) {
      if (!matrixProcs.has(proc)) exclusionProblems.push(`${backend}: \`${proc}\` is cited as unsupported but missing from ${options.supportMatrix}`);
    }
    for (const [code] of reason.matchAll(/E_WASM_[A-Z_]+/g)) {
      if (!matrixCodes.has(code)) exclusionProblems.push(`${backend}: ${code} is cited but missing from the diagnostic table in ${options.supportMatrix}`);
    }
  }
}
if (exclusionProblems.length > 0) {
  console.error(`Invalid exclusion list ${options.exclusions}:\n  ${[...new Set(exclusionProblems)].join("\n  ")}`);
  process.exit(1);
}

// --- temporary Snapshots ---------------------------------------------------

const fixture = await mkdtemp(join(tmpdir(), "calcit-core-tests-"));
// Generated JS resolves @calcit/procs from the fixture directory upwards.
await symlink(resolve("node_modules"), join(fixture, "node_modules"), "dir");
let snapshotCounter = 0;

const coreText = await readFile(corePath, "utf8");
const filesHeader = "\n  :files $ {}\n";
const defsHeader = (ns) => `\n    '${ns} $ %{} 'FileEntry\n      :defs $ {}\n`;
for (const ns of new Set(selected.map((test) => test.ns))) {
  if (!coreText.includes(defsHeader(ns))) throw new Error(`${options.snapshot}: unexpected Snapshot layout for ${ns}`);
}
const unitSchema = ["::", "'Fn", ["{}", [":args", ["[]"]], [":return", "'Unit"]]];
const codeEntry = (code) => ["%{}", "'CodeEntry", [":doc", "|"], [":code", ["quote", code]], [":examples", ["[]"]],
  [":schema", unitSchema]];
const indent = (text, spaces) => text.trimEnd().split("\n").map((line) => `${" ".repeat(spaces)}${line}`).join("\n");

// Tests placed outside their owning namespace keep its require rules and refer
// its own definitions (calcit.core is visible everywhere already).
const replayRequires = (tests) => {
  const rules = new Map();
  for (const ns of new Set(tests.map((test) => test.ns))) {
    const file = source[":files"][`'${ns}`];
    for (const item of (file.ns?.code?.__edn_quote ?? []).slice(2)) {
      if (Array.isArray(item) && item[0] === ":require") {
        for (const rule of item.slice(1)) rules.set(JSON.stringify(rule), rule);
      }
    }
    if (ns !== "calcit.core") rules.set(`refer:${ns}`, [ns, ":refer", Object.keys(file.defs ?? {}).map(stripNs)]);
  }
  return [...rules.values()];
};

// Like `calcit test`, each test becomes a definition of its owning namespace,
// so names and namespace-level permissions resolve exactly as in the source.
// One zero-argument function per test keeps every failure attributable. The
// Snapshot text is written in one step; per-definition `edit` commands would
// reload the whole core once per test. With `owner: false` the tests live in a
// separate replay namespace instead (see the WASM runner).
const buildSnapshot = async (tests, { withMain, head = "defwasm-export", owner = true }) => {
  const snapshot = join(fixture, `replay-${snapshotCounter++}.cirru`);
  const home = (test) => owner ? test.ns : REPLAY_NS;
  const testDef = (test) => [`'${test.fn}`, codeEntry([head, test.fn, [], test.code, "&unit"])];
  let text = coreText;
  if (owner) {
    for (const ns of new Set(tests.map((test) => test.ns))) {
      const defs = tests.filter((test) => test.ns === ns).map(testDef);
      text = text.replace(defsHeader(ns), `${defsHeader(ns)}${indent(writeCirruCode(defs), 8)}\n`);
    }
  }
  const replayDefs = owner ? [] : tests.map(testDef);
  let entry = `${home(tests[0])}/${tests[0].fn}`;
  if (withMain) {
    entry = `${REPLAY_NS}/main!`;
    const body = tests.flatMap((test) => [["println", `|@@core-test:${test.index}`],
      [`${home(test)}/${test.fn}`], ["println", `|@@core-test-end:${test.index}`]]);
    replayDefs.push(["'main!", codeEntry(["defn", "main!", [], ...body, "&unit"])]);
  }
  if (replayDefs.length > 0) {
    const nsCode = owner ? ["ns", REPLAY_NS] : ["ns", REPLAY_NS, [":require", ...replayRequires(tests)]];
    const fileEntry = [`'${REPLAY_NS}`, ["%{}", "'FileEntry", [":defs", ["{}", ...replayDefs]],
      [":ns", ["%{}", "'NsEntry", [":doc", "|"], [":code", ["quote", nsCode]]]]]];
    text = text.replace(filesHeader, `${filesHeader}${indent(writeCirruCode([fileEntry]), 4)}\n`);
  }
  await writeFile(snapshot, text);
  return { snapshot, entry: ["--init-fn", entry, "--reload-fn", entry] };
};

const tail = (text, lines = 40) => text.trim().split(/\r?\n/).slice(-lines).join("\n");

const referencesDef = (test, ns, def) => {
  const visit = (node) => Array.isArray(node) ? node.some(visit)
    : node === `${ns}/${def}` || (node === def && (ns === test.ns || ns === "calcit.core"));
  return visit(test.code);
};

// Build the largest passing subset. A rejection that names test definitions
// drops just those; otherwise bisect, so one unsupported test cannot hide the
// result of every other test.
const buildBisect = async (tests, build, rejected) => {
  let pending = tests;
  while (pending.length > 0) {
    const result = await build(pending);
    if (result.ok) return [{ tests: pending, artifact: result.artifact }];
    const named = new Set([...result.error.matchAll(/&calcit:test:(\d+)/g)].map((match) => Number(match[1])));
    // A rejected shared definition blames every test that references it directly.
    const failedDef = /preprocessing failed for ([^\s:]+)\/([^\s:]+):/.exec(result.error);
    if (failedDef && !failedDef[2].startsWith("&calcit:test:")) {
      for (const test of pending) if (referencesDef(test, failedDef[1], failedDef[2])) named.add(test.index);
    }
    const blamed = pending.filter((test) => named.has(test.index));
    if (blamed.length > 0 && blamed.length < pending.length) {
      for (const test of blamed) rejected.set(test.id, result.error);
      pending = pending.filter((test) => !named.has(test.index));
      continue;
    }
    if (pending.length === 1) {
      rejected.set(pending[0].id, result.error);
      return [];
    }
    const middle = Math.ceil(pending.length / 2);
    return [
      ...await buildBisect(pending.slice(0, middle), build, rejected),
      ...await buildBisect(pending.slice(middle), build, rejected),
    ];
  }
  return [];
};

// --- backends --------------------------------------------------------------

const results = new Map(); // id -> { backend -> { status, detail, trace } }
const record = (test, backend, status, detail = "", trace = []) => {
  if (!results.has(test.id)) results.set(test.id, {});
  results.get(test.id)[backend] = { status, detail, trace };
};

// Native and WASI commands run every test from one entry that prints a marker
// before each test. A failing test stops the process, so the rest rerun after it.
const runMarked = async (backend, tests, { head, compile, execute }) => {
  let pending = tests;
  while (pending.length > 0) {
    const built = await buildSnapshot(pending, { withMain: true, head });
    const compiled = compile(built);
    if (!compiled.ok) {
      // Isolate the definitions the backend rejects; everything else still runs.
      const rejected = new Map();
      const groups = await buildBisect(pending, async (subset) => compile(await buildSnapshot(subset, { withMain: true, head })), rejected);
      if (rejected.size === 0) {
        for (const test of pending) record(test, backend, "compile-error", compiled.error);
        return;
      }
      for (const test of pending) if (rejected.has(test.id)) record(test, backend, "compile-error", rejected.get(test.id));
      pending = groups.flatMap((group) => group.tests);
      continue;
    }
    const output = execute(built, compiled);
    const { traces, last, events } = parseMarkers(output.stdout ?? "");
    const protocolError = markerProtocolError(events, pending, output.status === 0);
    if (protocolError) {
      for (const test of pending) record(test, backend, "fail", protocolError, traces.get(test.index) ?? []);
      return;
    }
    if (output.status === 0) {
      for (const test of pending) record(test, backend, "pass", "", traces.get(test.index) ?? []);
      return;
    }
    const failedIndex = pending.findIndex((test) => test.index === last);
    if (failedIndex < 0) {
      for (const test of pending) record(test, backend, "fail", tail(`${output.stdout}\n${output.stderr}`));
      return;
    }
    for (const test of pending.slice(0, failedIndex)) record(test, backend, "pass", "", traces.get(test.index) ?? []);
    const failed = pending[failedIndex];
    record(failed, backend, "fail", tail(output.stderr ?? ""), traces.get(failed.index) ?? []);
    pending = pending.slice(failedIndex + 1);
  }
};

const runNative = (tests) => runMarked("native", tests, {
  head: "defwasm-export",
  // Preprocessing is lazy at runtime; a check-only pass attributes rejections.
  compile: ({ snapshot, entry }) => {
    const check = attempt(snapshot, ...entry, "--check-only");
    return check.status === 0 ? { ok: true } : { ok: false, error: tail(`${check.stdout}\n${check.stderr}`) };
  },
  execute: ({ snapshot, entry }) => attempt(snapshot, ...entry),
});

let wasiIndex = 0;
// A WASI 0.3 command owns its entry export, so tests are plain functions here.
const runWasi = (tests) => runMarked("wasi", tests, {
  head: "defn",
  compile: ({ snapshot, entry }) => {
    const output = join(fixture, `wasi-${wasiIndex++}`);
    const emitted = attempt("wasi", snapshot, ...entry, "--emit-path", output);
    return emitted.status === 0 ? { ok: true, output } : { ok: false, error: tail(`${emitted.stdout}\n${emitted.stderr}`) };
  },
  execute: (_built, { output }) => spawnSync(process.env.WASMTIME_CLI, [
    "run", "-S", "p3", "-W", "component-model-async-stackful=y",
    "-W", "component-model-more-async-builtins=y", join(output, "program.wasm"),
  ], execOptions),
});

const runJs = async (tests) => {
  const rejected = new Map();
  let buildIndex = 0;
  const groups = await buildBisect(tests, async (subset) => {
    const { snapshot, entry } = await buildSnapshot(subset, { withMain: false });
    const output = join(fixture, `js-${buildIndex++}`);
    const emitted = attempt(snapshot, ...entry, "--emit-path", output, "js");
    return emitted.status === 0 ? { ok: true, artifact: output } : { ok: false, error: tail(`${emitted.stdout}\n${emitted.stderr}`) };
  }, rejected);
  for (const test of tests) if (rejected.has(test.id)) record(test, "js", "compile-error", rejected.get(test.id));
  // Node is a file-capable JS host; browser hosts inject nothing (see ts-src).
  const previousInjections = globalThis.__calcit_injections__;
  globalThis.__calcit_injections__ = {
    read_file: (path) => readFileSync(path, "utf8"),
    read_dir: (path, recursive) => readdirSync(path, { recursive: recursive === true }).map((item) => join(path, item)),
  };
  try {
    await runJsGroups(groups);
  } finally {
    if (previousInjections === undefined) delete globalThis.__calcit_injections__;
    else globalThis.__calcit_injections__ = previousInjections;
  }
};

const runJsGroups = async (groups) => {
  for (const { tests: group, artifact } of groups) {
    const modules = new Map();
    for (const ns of new Set(group.map((test) => test.ns))) {
      modules.set(ns, await import(pathToFileURL(join(artifact, `${ns}.mjs`)).href));
    }
    for (const test of group) {
      const compiled = modules.get(test.ns);
      const trace = [];
      const stderr = [];
      const original = { log: console.log, error: console.error, warn: console.warn };
      console.log = (...values) => trace.push(values.join(" "));
      // assert= reports Left/Right through eprintln; keep it for the report.
      console.error = console.warn = (...values) => stderr.push(values.join(" "));
      try {
        compiled[test.jsName]();
        Object.assign(console, original);
        record(test, "js", "pass", "", trace);
      } catch (error) {
        Object.assign(console, original);
        const message = String(error?.stack ?? error).split("\n").slice(0, 3);
        record(test, "js", "fail", [...stderr.filter((line) => line.trim() !== ""), ...message].join("\n"), trace);
      }
    }
  }
};

const runWasm = async (tests) => {
  const rejected = new Map();
  let buildIndex = 0;
  const groups = await buildBisect(tests, async (subset) => {
    // `calcit wasm` preprocesses and exports every definition of the entry
    // namespace, which in calcit.core includes syntax placeholders; replay from
    // a separate namespace, as user code calls core.
    const { snapshot, entry } = await buildSnapshot(subset, { withMain: false, owner: false });
    const output = join(fixture, `wasm-${buildIndex++}`);
    const emitted = attempt("wasm", snapshot, ...entry, "--emit-path", output);
    if (emitted.status !== 0) return { ok: false, error: tail(`${emitted.stdout}\n${emitted.stderr}`) };
    // Codegen success is not enough: the engine must also accept the module.
    try {
      const module = new WebAssembly.Module(await readFile(join(output, "program.wasm")));
      // Unsupported dependencies compile to trapping stubs; keep them so a
      // runtime trap can be attributed in the report.
      const traps = new Map([...`${emitted.stdout}\n${emitted.stderr}`
        .matchAll(/trapping unsupported dependency ([^\s:]+)\/([^\s:]+): (.*)/g)]
        .map((match) => [`${match[1]}/${match[2]}`, match[3].trim()]));
      return { ok: true, artifact: { module, traps } };
    } catch (error) {
      return { ok: false, error: `invalid WASM module: ${error.message}` };
    }
  }, rejected);
  for (const test of tests) if (rejected.has(test.id)) record(test, "wasm", "compile-error", rejected.get(test.id));
  for (const { tests: group, artifact } of groups) {
    const { module, traps } = artifact;
    const trapsReached = (test) => [...traps.entries()]
      .filter(([id]) => { const [ns, def] = id.split("/"); return referencesDef(test, ns, def); })
      .map(([id, reason]) => `trapping stub ${id}: ${reason}`);
    let trace = [];
    let instance;
    const HEAP_MAGIC = 0xca1c17a9 | 0;
    // log_value receives any f64; only tagged heap strings decode as text.
    const readValue = (value) => {
      const memory = new DataView(instance.exports.memory.buffer);
      const stringTag = instance.exports.__string_tag?.value;
      if (Number.isInteger(value) && value >= 8 && value + 8 <= memory.byteLength
        && memory.getInt32(value - 8, true) === HEAP_MAGIC
        && (stringTag === undefined || memory.getInt32(value - 4, true) === stringTag)) {
        const length = memory.getFloat64(value, true);
        if (Number.isSafeInteger(length) && length >= 0 && value + 8 + length <= memory.byteLength) {
          return new TextDecoder().decode(new Uint8Array(memory.buffer, value + 8, length));
        }
      }
      return String(value);
    };
    const imports = {};
    for (const { module: namespace, name, kind } of WebAssembly.Module.imports(module)) {
      if (kind !== "function") throw new Error(`unexpected WASM import kind: ${namespace}.${name} ${kind}`);
      (imports[namespace] ??= {})[name] = () => { throw new Error(`unexpected host call: ${namespace}.${name}`); };
    }
    if (imports.math) for (const name of Object.keys(imports.math)) if (typeof Math[name] === "function") imports.math[name] = Math[name];
    if (imports.io?.log_str) imports.io.log_str = (ptr) => { trace.push(readValue(ptr)); return 0; };
    if (imports.io?.log_value) imports.io.log_value = (value) => { trace.push(readValue(value)); return 0; };
    instance = new WebAssembly.Instance(module, imports);
    for (const test of group) {
      trace = [];
      try {
        instance.exports[test.fn]();
        record(test, "wasm", "pass", "", trace);
      } catch (error) {
        const message = String(error?.stack ?? error).split("\n").slice(0, 2);
        record(test, "wasm", "fail", [...trace, ...message, ...trapsReached(test)].join("\n"), trace);
        // A trap may leave the shared instance inconsistent; continue on a fresh one.
        instance = new WebAssembly.Instance(module, imports);
      }
    }
  }
};

const runners = { native: runNative, js: runJs, wasm: runWasm, wasi: runWasi };

// --- run and report --------------------------------------------------------

const excluded = Object.fromEntries(BACKENDS.map((backend) => [backend, []]));
try {
  for (const backend of options.backends) {
    const included = [];
    for (const test of selected) {
      const reason = exclusionFor[backend](test);
      if (reason === undefined) included.push(test);
      else {
        excluded[backend].push(test);
        if (options.reportUnexpectedPass) included.push(test);
      }
    }
    const started = Date.now();
    if (included.length > 0) await runners[backend](included);
    console.log(`${backend}: ran ${included.length} tests in ${((Date.now() - started) / 1000).toFixed(1)}s`);
  }
} finally {
  await rm(fixture, { recursive: true, force: true });
}

// Per-test, per-backend status and trace for tools that judge results themselves.
if (options.resultsJson) {
  await writeFile(options.resultsJson, JSON.stringify(Object.fromEntries(results), null, 1));
}

const failures = [];
const unexpectedPasses = [];
for (const test of selected) {
  const byBackend = results.get(test.id) ?? {};
  const native = byBackend.native;
  for (const backend of options.backends) {
    const outcome = byBackend[backend];
    if (!outcome) continue;
    const reason = exclusionFor[backend](test);
    if (reason !== undefined) {
      if (options.reportUnexpectedPass && hasNativeParity(outcome, native, backend)) {
        unexpectedPasses.push(`${backend} ${test.id} (${reason})`);
      }
      continue;
    }
    if (outcome.status !== "pass") {
      failures.push({ test, backend, kind: outcome.status, expected: "pass", actual: outcome.detail });
    } else if (backend !== "native" && native?.status === "pass"
      && JSON.stringify(outcome.trace) !== JSON.stringify(native.trace)) {
      failures.push({
        test, backend, kind: "trace-mismatch",
        expected: JSON.stringify(native.trace), actual: JSON.stringify(outcome.trace),
      });
    }
  }
}

console.log(`Selected ${selected.length} of ${allTests.length} definition tests from ${options.snapshot}`);
for (const backend of options.backends) {
  const ran = [...results.values()].filter((item) => item[backend]).length;
  const passed = [...results.values()].filter((item) => item[backend]?.status === "pass").length;
  console.log(`  ${backend}: ${passed}/${ran} passed, ${excluded[backend].length} excluded`);
}
if (unexpectedPasses.length > 0) {
  console.log(`Excluded tests that now pass (${unexpectedPasses.length}); consider removing them from the list:`);
  for (const line of unexpectedPasses) console.log(`  ${line}`);
}
if (failures.length > 0) {
  console.error(`\n${failures.length} failure(s):`);
  for (const { test, backend, kind, expected, actual } of failures) {
    console.error(`\n- ${test.id} [${backend}] ${kind}`);
    console.error(`  expected: ${expected}`);
    console.error(`  actual:   ${actual.split("\n").join("\n            ")}`);
    const others = options.backends.filter((other) => other !== backend)
      .map((other) => `${other}=${results.get(test.id)?.[other]?.status ?? "excluded"}`);
    if (others.length > 0) console.error(`  other backends: ${others.join(", ")}`);
  }
  process.exit(1);
}
console.log("All non-excluded definition tests passed on every requested backend.");
