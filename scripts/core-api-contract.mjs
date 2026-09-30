import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parse_cirru_edn } from "../lib/calcit.procs.mjs";
import { CalcitSymbol, CalcitTag, newTag } from "../lib/calcit-data.mjs";
import { CalcitMap, CalcitSliceMap } from "../lib/js-map.mjs";
import { CalcitSliceList } from "../lib/js-list.mjs";
import { CalcitCirruQuote, format_cirru_edn } from "../lib/js-cirru.mjs";

export const baselinePath = "docs/data/core-api-0.28-baseline.edn";
const scopePath = "docs/data/core-api-0.28-scope.edn";

// JSON is only the Node subprocess bridge; the versioned artifact is native EDN.
export function encode(value) {
  if (value?.quote) return new CalcitCirruQuote(value.quote);
  if (value?.symbol) return new CalcitSymbol(value.symbol);
  if (Array.isArray(value)) return new CalcitSliceList(value.map(encode));
  if (value !== null && typeof value === "object") {
    return new CalcitMap(null).assoc(...Object.entries(value).flatMap(([key, item]) => [newTag(key), encode(item)]));
  }
  return value;
}

export function decode(value) {
  if (value instanceof CalcitCirruQuote) return { quote: value.value };
  if (value instanceof CalcitSymbol) return { symbol: value.value };
  if (value instanceof CalcitTag) return `:${value.value}`;
  if (value instanceof CalcitMap || value instanceof CalcitSliceMap) {
    return Object.fromEntries(value.pairs().map(([key, item]) => [key.value, decode(item)]));
  }
  if (value?.toArray) return value.toArray().map(decode);
  return value;
}

export const readEdn = text => decode(parse_cirru_edn(text));
export const writeEdn = value => `${format_cirru_edn(encode(value))}\n`;

function declaration(code) {
  if (!Array.isArray(code)) return null;
  if (code[0] === "def") return declaration(code[2]);
  if (code[0] === "impl-traits") return declaration(code[1]);
  if (["defenum", "defstruct", "deftrait"].includes(code[0])) return { quote: code };
  return null;
}

function sourceArity(code) {
  if (!Array.isArray(code) || !["defn", "defmacro"].includes(code[0])) return null;
  const args = code[2];
  assert.ok(Array.isArray(args), "source callable must declare arguments");
  let min = 0, max = 0, optional = false;
  for (const arg of args) {
    assert.equal(typeof arg, "string", "source arity requires ordinary named arguments");
    if (arg === "?") optional = true;
    else if (arg === "&") return { min, max: null };
    else { if (!optional) min++; max++; }
  }
  return { min, max };
}

export function collect(scope, query) {
  const cache = new Map();
  const receivers = new Map();
  const definition = name => {
    if (!cache.has(name)) cache.set(name, query("def", name));
    const data = cache.get(name);
    assert.ok(data?.code !== undefined, `definition missing: ${name}`);
    return {
      schema: data.schema == null ? null : { quote: data.schema },
      declaration: declaration(data.code),
      "runtime-arity": data.runtime_arity ?? sourceArity(data.code),
    };
  };
  const methods = scope.methods.map(item => {
    if (!receivers.has(item.receiver)) receivers.set(item.receiver, query("type", item.receiver));
    const data = receivers.get(item.receiver);
    const method = data.methods.find(method => method.name === item.name);
    assert.equal(method?.status, "proven", `${item.receiver} ${item.name} requires proven dispatch`);
    assert.notEqual(method.role, "compatibility", `${item.name} is not the preferred spelling`);
    assert.ok(method.definition, `${item.name} requires schema provenance`);
    const evidence = definition(method.definition);
    assert.ok(evidence.schema, `${item.name} requires declared generic/receiver evidence`);
    return {
      ...item,
      schema: evidence.schema,
      "runtime-arity": evidence["runtime-arity"],
      features: method.features,
      provenance: { symbol: method.definition },
    };
  });
  return {
    version: scope.version,
    scope: scope.scope,
    definitions: scope.definitions.map(item => ({ ...item, ...definition(item.name.symbol) })),
    methods,
    "method-contracts": scope.methods.map(item => {
      const data = receivers.get(item.receiver);
      const method = data.methods.find(method => method.name === item.name);
      assert.ok(method.call_types, `${item.receiver} ${item.name} requires resolved call type syntax`);
      return {
        receiver: item.receiver,
        name: item.name,
        parameters: method.call_types.parameters.map(quote => ({ quote })),
        rest: method.call_types.rest == null ? null : { quote: method.call_types.rest },
        returns: { quote: method.call_types.returns },
      };
    }),
  };
}

// Implementation paths and display strings are not public signatures.
function contract(row) {
  const { provenance, ...publicContract } = row;
  // EDN map entry order is not part of a type signature; argument lists are.
  const normalize = node => {
    if (!Array.isArray(node)) return node;
    const children = node.map(normalize);
    if (children[0] === "{}") return ["{}", ...children.slice(1).sort((a, b) => JSON.stringify(a[0]).localeCompare(JSON.stringify(b[0])))];
    return children;
  };
  if (publicContract.schema) publicContract.schema = { quote: normalize(publicContract.schema.quote) };
  if (publicContract.parameters) publicContract.parameters = publicContract.parameters.map(item => ({ quote: normalize(item.quote) }));
  if (publicContract.rest) publicContract.rest = { quote: normalize(publicContract.rest.quote) };
  if (publicContract.returns) publicContract.returns = { quote: normalize(publicContract.returns.quote) };
  return publicContract;
}

export function assertPreserved(previous, current) {
  for (const family of ["definitions", "methods", "method-contracts"]) {
    for (const old of previous[family] ?? []) {
      const row = (current[family] ?? []).find(row => family === "definitions"
        ? row.name.symbol === old.name.symbol
        : row.receiver === old.receiver && row.name === old.name);
      assert.ok(row, `removed frozen ${family}: ${JSON.stringify(old.name)}`);
      assert.deepEqual(contract(row), contract(old), `changed frozen ${family}: ${JSON.stringify(old.name)}`);
    }
  }
}

export function assertHistoryPreserved(baseline, current, { repository = process.cwd(), baseRef, eventName } = {}) {
  const git = args => execFileSync("git", args, { cwd: repository, encoding: "utf8" }).trim();
  const revisions = ["HEAD"];
  if (baseRef) revisions.push(git(["merge-base", "HEAD", `origin/${baseRef}`]));
  else if (eventName === "push") revisions.push("HEAD^1");
  for (const revision of revisions) {
    if (git(["ls-tree", revision, "--", baselinePath])) {
      const previous = readEdn(git(["show", `${revision}:${baselinePath}`]));
      assertPreserved(previous, baseline);
      assertPreserved(previous, current);
    }
  }
}

export function check() {
  const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
  const scope = readEdn(readFileSync(scopePath, "utf8"));
  const query = (kind, target) => {
    const response = JSON.parse(execFileSync(binary, ["src/cirru/calcit-core.cirru", "query", kind, target, "--format", "json"], {
      encoding: "utf8", stdio: "pipe", timeout: 60000, maxBuffer: 16 * 1024 * 1024,
    }));
    assert.deepEqual(response.diagnostics, [], `query diagnostics: ${target}`);
    return response.data;
  };
  const current = collect(scope, query);
  if (process.argv.includes("--export")) {
    process.stdout.write(writeEdn(current));
    return;
  }
  const baseline = readEdn(readFileSync(baselinePath, "utf8"));
  assertPreserved(baseline, current);
  // Compare the committed baseline too: editing the working copy is not a waiver.
  assertHistoryPreserved(baseline, current, { baseRef: process.env.GITHUB_BASE_REF, eventName: process.env.GITHUB_EVENT_NAME });
  console.log("Reviewed core contracts preserve native schemas, arity and specialized method call types");
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) check();
