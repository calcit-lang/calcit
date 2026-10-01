import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { assertHistoryPreserved, assertPreserved, baselinePath, collect, readEdn, writeEdn } from "./core-api-contract.mjs";

const baseline = readEdn(readFileSync(baselinePath, "utf8"));
const definition = (data, name) => data.definitions.find(row => row.name.symbol === `calcit.core/${name}`);

test("scalar conversion evidence preserves precise results and distinct display traits", () => {
  for (const receiver of ["'String", "'Number", "'Bool", "'Tag", "'Symbol", "'Nil"]) {
    const conversion = baseline["method-contracts"].find(row => row.receiver === receiver && row.name === ".to-string");
    assert.ok(conversion, `${receiver} must retain its conversion evidence`);
    assert.deepEqual(conversion.parameters, []);
    assert.equal(conversion.rest, null);
    assert.equal(conversion.returns.quote, "'String");
  }
  for (const [name, method] of [["ToString", ".to-string"], ["Debug", ".debug"], ["Show", ".show"]]) {
    const trait = definition(baseline, name).declaration.quote;
    assert.equal(trait[0], "deftrait");
    assert.equal(trait[1], name);
    assert.equal(trait[2][0], method);
  }
});

test("native EDN preserves symbols, tags and raw schema quotes", () => {
  assert.deepEqual(readEdn(writeEdn(baseline)), baseline);
  const conversion = definition(baseline, "to-string");
  assert.ok(conversion.schema.quote.some(pair => pair[0] === ":where"));
  const reader = definition(baseline, "read-dir");
  assert.deepEqual(reader["runtime-arity"], { min: 1, max: 2 });
  const get = baseline["method-contracts"].find(row => row.receiver === ":: 'List 'Number" && row.name === ".get");
  assert.deepEqual(get.returns.quote, ["::", "'calcit.core/Option", "'Number"]);
  assert.equal(typeof conversion.schema.quote.find(pair => pair[0] === ":return")[1], "string",
    "quoted syntax leaves are strings; only EDN symbols outside quotes become symbol objects");
});

for (const [name, mutate] of [
  ["deletion", data => data.definitions.shift()],
  ["return type", data => definition(data, "to-string").schema.quote.find(pair => pair[0] === ":return")[1] = "'Dynamic"],
  ["argument type", data => definition(data, "to-string").schema.quote.find(pair => pair[0] === ":args")[1][1] = "'Dynamic"],
  ["generic bound", data => definition(data, "to-string").schema.quote = definition(data, "to-string").schema.quote.filter(pair => pair[0] !== ":where")],
  ["optional argument", data => definition(data, "read-dir")["runtime-arity"].min = 2],
  ["failure contract", data => definition(data, "parse-float").failure = "throw instead of Result"],
  ["receiver", data => data.methods[0].receiver = "'Dynamic"],
  ["method schema", data => data.methods[0].schema.quote[2][1] = "'Dynamic"],
  ["backend feature", data => data.methods[0].features.push("js-ffi")],
  ["specialized lookup result", data => data["method-contracts"].find(row => row.receiver === ":: 'List 'Number" && row.name === ".get").returns.quote = "'String"],
  ["scalar conversion result", data => data["method-contracts"].find(row => row.receiver === "'String" && row.name === ".to-string").returns.quote = "'Dynamic"],
  ["display trait method", data => definition(data, "Debug").declaration.quote[2][0] = ".to-string"],
  ["specialized callback relation", data => data["method-contracts"].find(row => row.name === ".fold").returns.quote = "'Number"],
]) {
  test(`rejects an unannounced ${name} change even when the baseline is regenerated`, () => {
    const changed = structuredClone(baseline);
    mutate(changed);
    assertPreserved(changed, changed);
    assert.throws(() => assertPreserved(baseline, changed));
  });
}

test("additions and internal implementation-path changes preserve the public contract", () => {
  const changed = structuredClone(baseline);
  changed.definitions.push({ name: { symbol: "calcit.core/new-api" } });
  changed.methods[0].provenance = { symbol: "calcit.core/new-private-implementation" };
  assertPreserved(baseline, changed);
});

test("schema map entry ordering is not a breaking signature change", () => {
  const changed = structuredClone(baseline);
  const conversion = definition(changed, "to-string");
  const schema = conversion.schema.quote;
  conversion.schema.quote = [schema[0], ...schema.slice(1).reverse()];
  const callback = changed["method-contracts"].find(row => row.name === ".fold").parameters[1].quote;
  callback[2] = [callback[2][0], ...callback[2].slice(1).reverse()];
  assertPreserved(baseline, changed);
});

test("function implementation bodies and argument variable names are not frozen", () => {
  const scope = { version: "test", scope: "test", definitions: [{ name: { symbol: "calcit.core/example" } }], methods: [] };
  const schema = ["{}", [":kind", ":fn"], [":args", ["[]", "'Number"]], [":return", "'Number"]];
  const before = collect(scope, () => ({ code: ["defn", "example", ["x"], ["+", "x", "1"]], schema }));
  const after = collect(scope, () => ({ code: ["defn", "example", ["value"], ["inc", "value"]], schema }));
  assertPreserved(before, after);
});

test("unproven dispatch or missing call syntax cannot be promoted into the baseline", () => {
  const scope = { definitions: [], methods: [{ receiver: "'Number", name: ".example" }] };
  for (const status of ["open", "ambiguous"]) {
    assert.throws(() => collect(scope, () => ({ methods: [{ name: ".example", status }] })), /requires proven dispatch/);
  }
  assert.throws(() => collect(scope, kind => kind === "type"
    ? { methods: [{ name: ".example", status: "proven", definition: "calcit.core/example" }] }
    : { code: ["defn", "example", ["x"], [",", "x"]], schema: "'Number" }), /requires resolved call type syntax/);
});

test("Git history rejects a baseline-only waiver in both PR and push workflows", async () => {
  const repository = await mkdtemp(join(tmpdir(), "calcit-core-contract-history-"));
  const git = (...args) => execFileSync("git", args, {
    cwd: repository, encoding: "utf8", stdio: "pipe",
    env: { ...process.env, GIT_AUTHOR_NAME: "Contract Test", GIT_AUTHOR_EMAIL: "test@example.invalid",
      GIT_COMMITTER_NAME: "Contract Test", GIT_COMMITTER_EMAIL: "test@example.invalid" },
  }).trim();
  try {
    git("init");
    const file = join(repository, baselinePath);
    await mkdir(dirname(file), { recursive: true });
    await writeFile(file, writeEdn(baseline));
    git("add", ".");
    git("-c", "commit.gpgsign=false", "commit", "-m", "original baseline");
    git("update-ref", "refs/remotes/origin/main", git("rev-parse", "HEAD"));
    const changed = structuredClone(baseline);
    changed.definitions.shift();
    await writeFile(file, writeEdn(changed));
    assert.throws(() => assertHistoryPreserved(changed, changed, { repository }));
    git("add", ".");
    git("-c", "commit.gpgsign=false", "commit", "-m", "attempted baseline waiver");
    assert.throws(() => assertHistoryPreserved(changed, changed, { repository, baseRef: "main" }));
    assert.throws(() => assertHistoryPreserved(changed, changed, { repository, eventName: "push" }));
  } finally {
    await rm(repository, { recursive: true, force: true });
  }
});
