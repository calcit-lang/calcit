import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const mode = process.argv[2];
if (mode !== "--write" && mode !== "--check") {
  throw new Error("usage: node scripts/core-dynamic-classification.mjs --write|--check");
}

const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const calcit = resolve(repository, "target/debug/calcit");
const outputPath = resolve(repository, "docs/core-dynamic-classification.md");
const result = spawnSync(
  calcit,
  ["src/cirru/calcit-core.cirru", "analyze", "weak-types", "--only", "schema-dynamic", "--format", "json"],
  { cwd: repository, encoding: "utf8" },
);
if (result.error) {
  throw new Error(`failed to run ${calcit}; build it first with cargo build --bin calcit: ${result.error.message}`);
}
if (result.status !== 0) {
  throw new Error(`core weak-type analysis failed:\n${result.stderr}\n${result.stdout}`);
}

const report = JSON.parse(result.stdout);
const occurrences = report.data.definitions.flatMap((definition) =>
  definition.occurrences.map((occurrence) => ({ definition: definition.id, ...occurrence })),
);
if (occurrences.length !== report.data.summary.hits) {
  throw new Error(`analysis summary reports ${report.data.summary.hits} hits, but emitted ${occurrences.length}`);
}

const compilerForms = new Set([
  "&",
  "&call-spread",
  "&data-to-code",
  "&extract-code-into-edn",
  "&let",
  "'",
  "~",
  "~@",
  "assert-type",
  "defmacro",
  "defn",
  "eval",
  "gensym",
  "hint-fn",
  "if",
  "macroexpand",
  "macroexpand-1",
  "macroexpand-all",
  "quasiquote",
  "quote",
  "recur",
  "try",
]);
const runtimeMetadata = new Set([
  "&core-enum-impls",
  "&core-enum-methods",
  "&core-fn-impls",
  "&core-fn-methods",
  "&core-list-impls",
  "&core-list-methods",
  "&core-map-impls",
  "&core-map-methods",
  "&core-number-impls",
  "&core-number-methods",
  "&core-scalar-impls",
  "&core-set-impls",
  "&core-set-methods",
  "&core-string-impls",
  "&core-string-methods",
  "&core-struct-impls",
  "&core-struct-methods",
  "&impl::new",
  "&trait::new",
  "MapEntryDecision",
  "Option",
  "OptionMethods",
  "Result",
  "ResultMethods",
]);
const runtimePolymorphic = new Set([
  "%::",
  "&=",
  "&buffer",
  "&compare",
  "&enum:params",
  "&format-ternary-tree",
  "&get-def-doc",
  "&get-def-schema",
  "&get-in",
  "&get-raw",
  "&hash",
  "&impl:get",
  "&impl:nth",
  "&list:flatten",
  "&list:foldl-shortcut",
  "&list:sort-by",
  "&str",
  "&struct:assoc",
  "&struct:contains?",
  "&struct:extend-as",
  "&struct:from-map",
  "&struct:get",
  "&struct:matches?",
  "&struct:to-map",
  "&struct:with",
  "&{}",
  "::",
  "?",
  "ffi:response",
  "ffi:task",
  "impl-traits",
  "unsafe-coerce",
  "with-type-slot",
]);
const openDataBoundaries = new Set([
  "data-definition-form",
  "data-definition-malformed-nesting?",
  "data-definition-where-form?",
  "decode-map-as",
  "json-parse",
  "parse-cirru-edn",
  "parse-cirru-list",
  "tagging-edn",
  "try-decode-map-as",
  "try-parse-cirru-edn",
  "try-parse-cirru-edn-as",
  "try-parse-cirru-list",
  "try-parse-json",
  "turn-symbol",
]);
const compilerSpecializedPositions = new Map([
  [
    "apply|schema.return",
    "Arbitrary callable arity is not representable in the schema; compatible homogeneous List calls recover the callable return type at preprocessing time, while unproved calls remain Dynamic.",
  ],
  [
    "first|schema.return.type-arg.0",
    "Supported sequence receivers recover the Option payload from List, String, or Enum evidence; the separate Dynamic receiver position remains in the migration queue until its capability is expressible.",
  ],
  [
    "last|schema.return.type-arg.0",
    "Supported sequence receivers recover the Option payload from List, String, or Enum evidence; the separate Dynamic receiver position remains in the migration queue until its capability is expressible.",
  ],
  [
    "nth|schema.return.type-arg.0",
    "Supported indexed receivers recover the Option payload from List, String, or Enum evidence; the separate Dynamic receiver position remains in the migration queue until its capability is expressible.",
  ],
  [
    "get|schema.args.1",
    "The receiver specializes the lookup key to the Map key or indexed Number contract; the separate Dynamic receiver position remains in the migration queue until its capability is expressible.",
  ],
  [
    "get|schema.return.type-arg.0",
    "The receiver specializes the Option payload to the Map value, List member, or String character contract; the separate Dynamic receiver position remains in the migration queue until its capability is expressible.",
  ],
  [
    "get-in|schema.return.type-arg.0",
    "A statically typed receiver plus a fully known literal Map/List path recovers the Option payload at preprocessing time; dynamic paths and Struct traversal remain on the reviewed fallback.",
  ],
  [
    "filter|schema.return",
    "A statically known List, Map, or Set receiver is rewritten to its typed core filter implementation, which preserves the receiver collection shape; the separate Dynamic receiver position remains in the migration queue.",
  ],
  [
    "includes?|schema.args.1",
    "The receiver specializes the searched value to the Map value, collection member, or String contract; the separate Dynamic receiver position remains in the migration queue until its capability is expressible.",
  ],
  [
    "update|schema.args.1",
    "The receiver specializes the update key or index and the updater callback contract; the separate Dynamic receiver position remains in the migration queue until its capability is expressible.",
  ],
  [
    "update|schema.return",
    "Preprocessing preserves the concrete collection or Struct receiver as the update result; the separate Dynamic receiver position remains in the migration queue until its capability is expressible.",
  ],
]);

function classify(entry) {
  const separator = entry.definition.indexOf("/");
  const namespace = entry.definition.slice(0, separator);
  const name = entry.definition.slice(separator + 1);
  const specializedRationale = namespace === "calcit.core" ? compilerSpecializedPositions.get(`${name}|${entry.path}`) : undefined;
  if (specializedRationale !== undefined) {
    return {
      owner: "compiler-specialized-contracts",
      decision: "retain-reviewed",
      rationale: specializedRationale,
    };
  }
  if (entry.intent === "intentional-macro-syntax") {
    return {
      owner: "macro-system",
      decision: "retain-reviewed",
      rationale: "Phase-aware macro syntax is intentionally open; the per-definition quality baseline prevents growth.",
    };
  }
  if (namespace === "calcit.internal" && name !== "normalize-trait-type") {
    return {
      owner: "runtime-internals",
      decision: "retain-reviewed",
      rationale: "Compiler-owned trait/impl metadata is not a public value contract; keep it visible and baseline-locked.",
    };
  }
  if (runtimeMetadata.has(name)) {
    return {
      owner: "runtime-internals",
      decision: "retain-reviewed",
      rationale: "This definition transports compiler/runtime nominal metadata rather than ordinary application data.",
    };
  }
  if (compilerForms.has(name)) {
    return {
      owner: "compiler-forms",
      decision: "retain-reviewed",
      rationale: "This compiler form consumes or produces code/syntax whose value shape is phase-dependent.",
    };
  }
  if (runtimePolymorphic.has(name)) {
    return {
      owner: "runtime-primitives",
      decision: "retain-reviewed",
      rationale: "The low-level runtime operation is deliberately representation-polymorphic; typed project APIs must not use it as an escape hatch.",
    };
  }
  if (openDataBoundaries.has(name)) {
    return {
      owner: "open-data-boundaries",
      decision: "retain-reviewed",
      rationale: "The parser/decoder boundary accepts open data; callers must decode or narrow before entering nominal business APIs.",
    };
  }
  return {
    owner: namespace === "calcit.test" ? "test-library" : "public-core-api",
    decision: "migrate",
    rationale:
      "This caller-visible contract still loses type evidence and is assigned to calcit#701 for an honest generic, trait-associated, recursive nominal, or reviewed-boundary decision.",
  };
}

const rows = occurrences.map((entry) => ({ ...entry, ...classify(entry) }));
for (const row of rows) {
  if (!row.owner || !row.decision || !row.rationale) {
    throw new Error(`incomplete classification for ${row.definition} ${row.path}`);
  }
  if (row.intent === "intentional-macro-syntax" && row.decision !== "retain-reviewed") {
    throw new Error(`intentional macro syntax must remain reviewed: ${row.definition} ${row.path}`);
  }
}
const classifiedSpecializedPositions = new Set(
  rows
    .filter((row) => row.owner === "compiler-specialized-contracts")
    .map((row) => `${row.definition.slice(row.definition.indexOf("/") + 1)}|${row.path}`),
);
for (const position of compilerSpecializedPositions.keys()) {
  if (!classifiedSpecializedPositions.has(position)) {
    throw new Error(`configured compiler-specialized position is absent from the current inventory: ${position}`);
  }
}

const counts = new Map();
for (const row of rows) {
  const key = `${row.decision}/${row.owner}`;
  counts.set(key, (counts.get(key) ?? 0) + 1);
}
const summaryRows = [...counts].sort(([a], [b]) => a.localeCompare(b));
const escapeCell = (value) => String(value).replaceAll("|", "\\|").replaceAll("\n", " ");
const lines = [
  "<!-- Generated by scripts/core-dynamic-classification.mjs; edit the classifier, not this table. -->",
  "# Bundled core Dynamic classification",
  "",
  `Source revision: \`${report.revision}\`. Inventory: **${rows.length}** schema-Dynamic positions across **${report.data.summary.definitions}** definitions.`,
  "",
  "Every position has an owning subsystem and an explicit migration decision. `retain-reviewed` is not an exemption: the existing per-definition quality baseline rejects growth. `migrate` is the ordered cleanup queue for caller-visible contracts.",
  "",
  "## Summary",
  "",
  "| Decision / owner | Positions |",
  "| --- | ---: |",
  ...summaryRows.map(([key, count]) => `| ${escapeCell(key)} | ${count} |`),
  "",
  "## Complete inventory",
  "",
  "| Definition | Schema path | Analysis intent | Owner | Decision | Rationale |",
  "| --- | --- | --- | --- | --- | --- |",
  ...rows.map(
    (row) =>
      `| \`${escapeCell(row.definition)}\` | \`${escapeCell(row.path)}\` | ${escapeCell(row.intent)} | ${escapeCell(row.owner)} | ${escapeCell(row.decision)} | ${escapeCell(row.rationale)} |`,
  ),
  "",
];
const generated = `${lines.join("\n")}\n`;

if (mode === "--write") {
  writeFileSync(outputPath, generated);
  console.log(`wrote ${outputPath} (${rows.length} positions)`);
} else {
  let current;
  try {
    current = readFileSync(outputPath, "utf8");
  } catch (error) {
    throw new Error(`missing ${outputPath}; run yarn generate-core-dynamic-classification`, { cause: error });
  }
  if (current !== generated) {
    throw new Error(
      "bundled core Dynamic classification is stale; review the changed positions and run yarn generate-core-dynamic-classification",
    );
  }
  console.log(`core Dynamic classification is current (${rows.length} positions)`);
}
