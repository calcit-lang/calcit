import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { cp, mkdtemp, readFile, readdir, rename, rm, symlink, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import * as procs from "../lib/calcit.procs.mjs";

const repository = fileURLToPath(new URL("..", import.meta.url));
const fixture = await mkdtemp(join(repository, "target/js-ffi-fixture-"));
const relocated = await mkdtemp(join(repository, "target/js-ffi-relocated-"));
try {
  // A shared target directory must not select another checkout's runtime.
  for (const directory of [fixture, relocated]) {
    await symlink(resolve(repository, "node_modules"), join(directory, "node_modules"), "dir");
  }
  const input = join(fixture, "calcit.cirru");
  const output = join(fixture, "generated");
  await cp(resolve(repository, "calcit/js-ffi-consumer.cirru"), input);
  await cp(resolve(repository, "calcit/js-ffi-module"), join(fixture, "js-ffi-module"), { recursive: true });
  const moduleSnapshot = join(fixture, "js-ffi-module/calcit.cirru");
  await writeFile(join(fixture, "js-ffi-module/js-ffi-assets/browser-available.js"), '() => typeof document !== "undefined"\n');
  for (const args of [
    ["edit", "def", "app.main/browser-available?", "--code", "quote $ defn browser-available? () false"],
    ["edit", "schema", "app.main/browser-available?", "--code", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Bool) (:features $ #{} :js-ffi)"],
    ["edit", "ffi", "app.main/browser-available?", "--code", "{} (:target :browser) (:js $ {} $ :file |js-ffi-assets/browser-available.js)"],
  ]) {
    execFileSync(resolve(repository, "target/debug/calcit"), [moduleSnapshot, ...args], { cwd: fixture, stdio: "pipe" });
  }
  const native = spawnSync(resolve(repository, "target/debug/calcit"), [input], {
    cwd: fixture,
    encoding: "utf8",
  });
  assert.notEqual(native.status, 0, "native execution must reject JS-only definitions");
  assert.match(native.stderr, /unavailable in the native runtime/);
  const wasm = spawnSync(resolve(repository, "target/debug/calcit"), ["wasm", input, "--check-only"], {
    cwd: fixture,
    encoding: "utf8",
  });
  assert.notEqual(wasm.status, 0, "WASM checking must reject JS-only definitions");
  assert.match(wasm.stderr, /E_WASM_UNSUPPORTED_JS_FFI/);
  execFileSync(resolve(repository, "target/debug/calcit"), [input, "--emit-path", output, "js"], {
    cwd: fixture,
    stdio: "inherit",
  });

  const browserInput = join(fixture, "browser-consumer.cirru");
  const browserOutput = join(fixture, "browser-generated");
  await cp(input, browserInput);
  execFileSync(resolve(repository, "target/debug/calcit"), [browserInput, "config", "set", "target", "browser"], {
    cwd: fixture,
    stdio: "pipe",
  });
  execFileSync(resolve(repository, "target/debug/calcit"), [browserInput, "--init-fn", "test-nil.main/reload!", "--emit-path", browserOutput, "js"], {
    cwd: fixture,
    stdio: "pipe",
  });
  for (const name of await readdir(browserOutput)) {
    if (name.endsWith(".mjs")) {
      assert.doesNotMatch(await readFile(join(browserOutput, name), "utf8"), /from "node:path"/);
    }
  }
  const browserModule = await readFile(join(browserOutput, "app.main.mjs"), "utf8");
  assert.match(browserModule, /JS FFI: app\.main\/browser-available\?/);
  assert.doesNotMatch(browserModule, /JS FFI: app\.main\/(?:plus-one|plus-two|base-name)/);
  const crossTarget = spawnSync(resolve(repository, "target/debug/calcit"), [browserInput, "--init-fn", "test-nil.main/main!", "--check-only"], {
    cwd: fixture,
    encoding: "utf8",
  });
  assert.notEqual(crossTarget.status, 0, "a browser entry must reject a reachable Node-only JS FFI call");
  assert.match(crossTarget.stderr, /E_JS_FFI_TARGET_MISMATCH/);
  for (const args of [
    ["edit", "def", "test-nil.main/reference-only", "--code", "quote $ defn reference-only () (println app.main/base-name)"],
    ["edit", "schema", "test-nil.main/reference-only", "--code", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)"],
  ]) {
    execFileSync(resolve(repository, "target/debug/calcit"), [browserInput, ...args], { cwd: fixture, stdio: "pipe" });
  }
  const crossTargetValue = spawnSync(resolve(repository, "target/debug/calcit"), [browserInput, "--init-fn", "test-nil.main/reference-only", "--check-only"], {
    cwd: fixture,
    encoding: "utf8",
  });
  assert.notEqual(crossTargetValue.status, 0, "a browser entry must reject a Node-only JS FFI function value");
  assert.match(crossTargetValue.stderr, /E_JS_FFI_TARGET_MISMATCH/);

  await cp(output, join(relocated, "generated"), { recursive: true });
  const generated = join(relocated, "generated");
  const source = await readFile(join(generated, "app.main.mjs"), "utf8");
  const apiSource = await readFile(join(generated, "app.api.mjs"), "utf8");
  const consumerSource = await readFile(join(generated, "test-nil.main.mjs"), "utf8");
  assert.match(source, /JS FFI: app\.main\/plus-two/);
  assert.match(source, /JS FFI: app\.main\/plus-one/);
  assert.doesNotMatch(source, /JS FFI: app\.main\/browser-available\?/);
  assert.match(source, /sourceMappingURL=data:application\/json;base64,/);
  assert.match(source, /JS FFI module: calcit:\/\/app@[^\n]+ alias path\nimport \* as [^\n]+ from "node:path"/);
  assert.match(source, /\(value\) => value \+ 2/);
  assert.match(source, /from "node:path"/);
  assert.doesNotMatch(source, /\.\/\.ffi\//);
  assert.match(apiSource, /from "\.\/app\.main\.mjs"/);
  assert.match(consumerSource, /from "\.\/app\.api\.mjs"/);
  assert.match(consumerSource, /from "\.\/app\.main\.mjs"/);
  assert.doesNotMatch(apiSource, /js-ffi-assets|\.ffi\//);
  assert.ok(!(await readdir(generated)).includes(".ffi"), "JS snippets must not be copied as modules");
  for (const namespace of ["app.main", "app.api", "test-nil.main"]) {
    execFileSync(process.execPath, ["--check", join(generated, `${namespace}.mjs`)]);
  }
  const consumer = await import(pathToFileURL(join(generated, "test-nil.main.mjs")).href);
  consumer["main_$x_"]();
  const module = await import(pathToFileURL(join(generated, "app.main.mjs")).href);
  const api = await import(pathToFileURL(join(generated, "app.api.mjs")).href);
  assert.equal(module.plus_one(2), 3);
  assert.equal(module.plus_two(2), 4);
  assert.equal(module.base_name("/tmp/example.txt"), "example.txt");
  assert.equal(api.plus_four(2), 6);
  assert.equal(api.file_label("/tmp/example.txt"), "example.txt");
  assert.equal(module.count_a(), 3);
  assert.equal(api.next_count(), 4, "normal Calcit imports must share one JS FFI definition instance");
  assert.notEqual(module.count_a, module.count_b, "each Calcit definition receives its own expression instance");

  // Replay the actual attached Calcit expressions, rather than JS copies of their assertions.
  const hostContext = JSON.parse(execFileSync(resolve(repository, "target/debug/calcit"),
    [input, "query", "context", "app.main/checked-counter-host", "--format", "json"], { cwd: fixture, encoding: "utf8" }));
  assert.equal(hostContext.data.tests.truncated, false);
  assert.equal(hostContext.data.tests.items.length, 3);
  const hostEntry = ["defn", "replay-host-tests!", [], ...hostContext.data.tests.items.map(test => test.tree), "&unit"];
  execFileSync(resolve(repository, "target/debug/calcit"), [moduleSnapshot, "edit", "def", "app.main/replay-host-tests!",
    "--input-format", "json-ast", "--code", JSON.stringify(hostEntry)], { cwd: fixture });
  execFileSync(resolve(repository, "target/debug/calcit"), [moduleSnapshot, "edit", "schema", "app.main/replay-host-tests!",
    "--input-format", "cirru", "--code", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit) (:features $ #{} :js-ffi)"], { cwd: fixture });
  const hostOutput = join(fixture, "host-generated");
  execFileSync(resolve(repository, "target/debug/calcit"), [input, "--init-fn", "app.main/replay-host-tests!",
    "--emit-path", hostOutput, "js"], { cwd: fixture, encoding: "utf8", stdio: "pipe" });
  const hostModule = await import(pathToFileURL(join(hostOutput, "app.main.mjs")).href);
  hostModule.replay_host_tests_$x_();
  const host = hostModule.make_counter_host(3);
  assert.equal(hostModule.checked_counter_host(host), host, "checked casts preserve frozen host identity");
  for (const value of [null, undefined, 1, "host", {}, { "total-value": 3, add: 7 }, procs._$L_(3), procs.newTag("host"), new Uint8Array(1)]) {
    assert.throws(() => hostModule.checked_counter_host(value), /js-cast app\.main\/CounterHost:/);
  }
  const inherited = Object.create(host);
  assert.equal(hostModule.checked_counter_host(inherited), inherited, "host members may come from a prototype");
  const underlying = new Error("host getter failed");
  const hostile = { "total-value": 3, get add() { throw underlying; } };
  assert.throws(() => hostModule.checked_counter_host(hostile), error => error instanceof TypeError && error.cause === underlying);
  const fieldGetter = { get "total-value"() { throw underlying; }, add() { return 3; } };
  assert.equal(hostModule.checked_counter_host(fieldGetter), fieldGetter, "field-presence checks do not invoke getters");
  const trapping = new Proxy(host, { has() { throw underlying; } });
  assert.throws(() => hostModule.checked_counter_host(trapping), error => error instanceof TypeError && error.cause === underlying);
  // A shape check must not be represented as validation of foreign signatures.
  const declaredTrust = { "total-value": "wrong", add() { return "wrong"; } };
  assert.equal(hostModule.checked_counter_host(declaredTrust), declaredTrust);
  const castNative = spawnSync(resolve(repository, "target/debug/calcit"), [input, "--init-fn", "app.main/replay-host-tests!"],
    { cwd: fixture, encoding: "utf8" });
  assert.notEqual(castNative.status, 0);
  assert.match(castNative.stderr, /unavailable in the native runtime/);

  const checkedSchema = "quote $ :: 'Fn $ {} (:args $ [] 'JsObject) (:return 'app.main/CounterHost) (:features $ #{} :js-ffi)";
  const checkedBody = "quote $ defn checked-counter-host (value) (js-cast value 'CounterHost)";
  const mutateCast = (command, code) => execFileSync(resolve(repository, "target/debug/calcit"),
    [moduleSnapshot, "edit", command, "app.main/checked-counter-host", ...(command === "def" ? ["--overwrite"] : []),
      "--input-format", "cirru", "--code", code], { cwd: fixture, stdio: "pipe" });
  const rejectCast = pattern => {
    const checked = spawnSync(resolve(repository, "target/debug/calcit"),
      [input, "--init-fn", "app.main/replay-host-tests!", "--emit-path", hostOutput, "js"], { cwd: fixture, encoding: "utf8" });
    assert.notEqual(checked.status, 0, "invalid cast contracts must fail before JS execution");
    assert.match(`${checked.stdout}\n${checked.stderr}`, pattern);
  };
  mutateCast("schema", "quote $ :: 'Fn $ {} (:args $ [] 'JsObject) (:return 'app.main/CounterHost)");
  rejectCast(/E_JS_FFI_FEATURE_REQUIRED/);
  mutateCast("schema", checkedSchema);
  for (const target of ["Number", "calcit.core/Len", "MissingHost"]) {
    mutateCast("def", `quote $ defn checked-counter-host (value) (js-cast value '${target})`);
    rejectCast(/external-object trait/);
  }
  mutateCast("def", "quote $ defn checked-counter-host (value) (js-cast value)");
  rejectCast(/expects a value and an external-object trait/);
  mutateCast("def", checkedBody);

  const fileExpression = join(fixture, "js-ffi-module/js-ffi-assets/add-two.js");
  const originalExpression = await readFile(fileExpression, "utf8");
  const buildContractCase = (name) => {
    const caseOutput = join(fixture, `contract-${name}`);
    execFileSync(resolve(repository, "target/debug/calcit"), [input, "--emit-path", caseOutput, "js"], {
      cwd: fixture,
      stdio: "pipe",
    });
    execFileSync(process.execPath, ["--check", join(caseOutput, "app.main.mjs")]);
    return caseOutput;
  };
  await writeFile(fileExpression, "42\n");
  const nonFunctionOutput = buildContractCase("non-function");
  const nonFunction = spawnSync(process.execPath, ["--input-type=module", "-e", 'import("./app.main.mjs").then((module) => module.plus_two(2))'], {
    cwd: nonFunctionOutput,
    encoding: "utf8",
  });
  assert.notEqual(nonFunction.status, 0, "a syntactically valid non-function must fail when the Calcit definition is called");
  assert.match(nonFunction.stderr, /TypeError/);
  for (const [name, expression, actual] of [
    ["wrong-return", '() => "wrong"\n', "wrong"],
    ["null-return", "() => null\n", "null"],
    ["undefined-return", "() => undefined\n", "undefined"],
  ]) {
    await writeFile(fileExpression, expression);
    const caseOutput = buildContractCase(name);
    const observed = spawnSync(process.execPath, ["--input-type=module", "-e", 'import("./app.main.mjs").then((module) => { const value = module.plus_two(2); if (typeof value !== "number") { console.error(`FFI_RETURN_CONTRACT_MISMATCH:${String(value)}`); process.exitCode = 23; } })'], {
      cwd: caseOutput,
      encoding: "utf8",
    });
    assert.equal(observed.status, 23, `${name} must remain visible to an external runtime-contract check`);
    assert.match(observed.stderr, new RegExp(`FFI_RETURN_CONTRACT_MISMATCH:${actual}`));
  }
  await writeFile(fileExpression, originalExpression);
  const rebuilt = join(fixture, "rebuilt");
  const rebuild = () => execFileSync(resolve(repository, "target/debug/calcit"), [input, "--emit-path", rebuilt, "js"], {
    cwd: fixture,
    stdio: "inherit",
  });
  await writeFile(join(fixture, "js-ffi-module/js-ffi-assets/add-two.js"), "(value) => value + 3\n");
  rebuild();
  assert.match(await readFile(join(rebuilt, "app.main.mjs"), "utf8"), /\(value\) => value \+ 3/);
  const replacement = join(fixture, "js-ffi-module/js-ffi-assets/add-two.js.next");
  await writeFile(replacement, "(value) => value + 4\n");
  await rename(replacement, join(fixture, "js-ffi-module/js-ffi-assets/add-two.js"));
  rebuild();
  assert.match(await readFile(join(rebuilt, "app.main.mjs"), "utf8"), /\(value\) => value \+ 4/);
  await writeFile(join(fixture, "js-ffi-module/js-ffi-assets/add-two.js"), '\r\n(value) => {\r\n  const label = "测试";\r\n  throw new Error(`ffi-intentional ${label}`, { cause: new Error("underlying") });\r\n}\r\n');
  rebuild();
  const relocatedRebuild = join(relocated, "rebuilt");
  await cp(rebuilt, relocatedRebuild, { recursive: true });
  const thrown = spawnSync(process.execPath, ["--enable-source-maps", "--input-type=module", "-e", 'import("./app.api.mjs").then((api) => { try { api.plus_four(2); } catch (error) { if (error.cause?.message !== "underlying") process.exit(3); throw error; } })'], {
    cwd: relocatedRebuild,
    encoding: "utf8",
  });
  assert.notEqual(thrown.status, 0, "a JS FFI exception must propagate through the Calcit wrapper");
  assert.match(thrown.stderr, /ffi-intentional/);
  assert.match(thrown.stderr, /calcit:\/\/app@[^/]*\/app\.main\/plus-two\/file\/js-ffi-assets\/add-two\.js\?hash=[a-f0-9]+:4:9/);
  assert.match(thrown.stderr, /app\.api\.mjs/);
  assert.match(await readFile(join(rebuilt, "app.main.mjs"), "utf8"), /JS FFI: app\.main\/plus-two/);
  const moduleSnapshotSource = await readFile(moduleSnapshot, "utf8");
  await writeFile(moduleSnapshot, moduleSnapshotSource.replace("|node:path", "|missing-ffi-package-1362"));
  const missingOutput = join(fixture, "missing-generated");
  execFileSync(resolve(repository, "target/debug/calcit"), [input, "--emit-path", missingOutput, "js"], {
    cwd: fixture,
    stdio: "inherit",
  });
  const missingCode = await readFile(join(missingOutput, "app.main.mjs"), "utf8");
  assert.match(missingCode, /JS FFI module: calcit:\/\/app@[^\n]+ alias path\nimport \* as [^\n]+ from "missing-ffi-package-1362"/);
  const missingModule = spawnSync(process.execPath, ["--input-type=module", "-e", 'import("./app.main.mjs")'], {
    cwd: missingOutput,
    encoding: "utf8",
  });
  assert.notEqual(missingModule.status, 0);
  assert.match(missingModule.stderr, /ERR_MODULE_NOT_FOUND/);
  assert.match(missingModule.stderr, /missing-ffi-package-1362/);
  await writeFile(moduleSnapshot, moduleSnapshotSource);
  await writeFile(join(fixture, "js-ffi-module/js-ffi-assets/add-two.js"), "(value) => {\n");
  rebuild();
  const invalidSource = await readFile(join(rebuilt, "app.main.mjs"), "utf8");
  assert.match(invalidSource, /JS FFI: app\.main\/plus-two \(calcit:\/\//);
  const syntax = spawnSync(process.execPath, ["--check", join(rebuilt, "app.main.mjs")], { encoding: "utf8" });
  assert.notEqual(syntax.status, 0, "invalid external JavaScript must fail syntax validation");
  assert.match(syntax.stderr, /SyntaxError/);
  assert.match(syntax.stderr, /app\.main\.mjs/);
  await writeFile(fileExpression, "(value) => value + 2\n");
  const renamedExpression = join(fixture, "js-ffi-module/js-ffi-assets/add-two-renamed.js");
  await rename(fileExpression, renamedExpression);
  execFileSync(resolve(repository, "target/debug/calcit"), [
    moduleSnapshot,
    "edit", "ffi", "app.main/plus-two",
    "--code", "{} (:target :node) (:js $ {} $ :file |js-ffi-assets/add-two-renamed.js)",
  ], { cwd: fixture, stdio: "pipe" });
  rebuild();
  const renamedSource = await readFile(join(rebuilt, "app.main.mjs"), "utf8");
  assert.match(renamedSource, /JS FFI: app\.main\/plus-two \(calcit:\/\/app@[^/]+\/app\.main\/plus-two\/file\/js-ffi-assets\/add-two-renamed\.js\?hash=/);
  const sourceMapData = renamedSource.match(/sourceMappingURL=data:application\/json;base64,([^\n]+)/)?.[1];
  assert.ok(sourceMapData, "renamed JS FFI must emit a source map");
  const renamedMap = JSON.parse(Buffer.from(sourceMapData, "base64").toString("utf8"));
  assert.ok(renamedMap.sources.some((sourceName) => sourceName.includes("/file/js-ffi-assets/add-two-renamed.js?hash=")));
  assert.ok(renamedMap.sources.every((sourceName) => !sourceName.includes("/file/js-ffi-assets/add-two.js?hash=")));
  assert.ok(renamedMap.sourcesContent.includes("(value) => value + 2\n"));
  execFileSync(process.execPath, ["--check", join(rebuilt, "app.main.mjs")]);
  console.log("embedded JS FFI, runtime contract failures, ordinary Calcit imports, exception stack, and explicit JS-only rebuild/rename passed");
} finally {
  await rm(fixture, { recursive: true, force: true });
  await rm(relocated, { recursive: true, force: true });
}
