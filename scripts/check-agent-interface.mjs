import { spawnSync } from "node:child_process";

const binary = process.env.CALCIT_AGENT_CR ?? "./target/debug/cr";

const scenarios = [
  {
    name: "static type methods",
    args: ["calcit/test.cirru", "query", "type", ":number", "--format", "json"],
    check(result) {
      if (result.schema_version !== 1 || result.command !== "query.type") {
        throw new Error("unexpected query.type envelope");
      }
      if (!result.data.methods.some((method) => method.name === ".ceil")) {
        throw new Error("query.type did not expose .ceil");
      }
    },
  },
  {
    name: "legacy any alias",
    args: ["calcit/test.cirru", "query", "type", ":any", "--format", "json"],
    check(result) {
      if (result.command !== "query.type" || result.data.canonical_type !== "'Dynamic") {
        throw new Error("query.type did not canonicalize legacy :any to 'Dynamic");
      }
      if (result.data.methods !== null || result.diagnostics[0]?.code !== "W_LEGACY_ANY_ALIAS") {
        throw new Error(":any did not preserve dynamic dispatch semantics and migration guidance");
      }
    },
  },
  {
    name: "builtin FFI context",
    args: [
      "calcit/test.cirru",
      "query",
      "context",
      "calcit.core/to-js-data",
      "--format",
      "json",
      "--budget",
      "1800",
    ],
    check(result) {
      if (result.schema_version !== 1 || result.command !== "query.context") {
        throw new Error("unexpected query.context envelope");
      }
      if (result.data.coverage !== "intentional-dynamic") {
        throw new Error("FFI context lost intentional dynamic classification");
      }
      if (result.data.examples.total !== 3 || !result.data.examples.items[2].tree) {
        throw new Error("builtin examples were not preserved as syntax trees");
      }
    },
  },
  {
    name: "project definition context",
    args: [
      "calcit/test.cirru",
      "query",
      "context",
      "app.main/main!",
      "--format",
      "json",
      "--budget",
      "1800",
    ],
    check(result) {
      if (result.data.id !== "app.main/main!") {
        throw new Error("project definition identity changed");
      }
      if (result.data.uri !== "calcit://definition/app.main/main!") {
        throw new Error("definition resource URI is missing");
      }
      if (!result.revision.startsWith("md5:") || result.data.code.root !== "code") {
        throw new Error("definition revision or Snapshot root path is missing");
      }
      if (!Array.isArray(result.data.dependencies.items) || !Array.isArray(result.data.usages.items)) {
        throw new Error("semantic relation collections are missing");
      }
    },
  },
  {
    name: "expression type evidence",
    args: [
      "calcit/test.cirru",
      "query",
      "type-at",
      "test-record.main/sum-point",
      "--path",
      "code@3.1",
      "--format",
      "json",
    ],
    check(result) {
      if (result.schema_version !== 1 || result.command !== "query.type-at") {
        throw new Error("unexpected query.type-at envelope");
      }
      if (result.data.inferred_type !== "'Number" || result.data.expected_type !== "'Number") {
        throw new Error("type-at lost inferred or expected type evidence");
      }
      if (!result.data.bindings.some((binding) => binding.name === "p")) {
        throw new Error("type-at did not expose the referenced typed binding");
      }
      if (result.diagnostics.length !== 0) {
        throw new Error("type-at reported diagnostics for a valid expression");
      }
    },
  },
  {
    name: "source-backed data type",
    args: [
      "calcit/test.cirru",
      "query",
      "type",
      "test-record.main/Person",
      "--format",
      "json",
    ],
    check(result) {
      if (result.data.resolved_from !== "definition inference") {
        throw new Error("query.type did not infer a source-backed defstruct");
      }
      if (!result.data.methods.some((method) => method.name === ".assoc")) {
        throw new Error("source-backed record methods are missing");
      }
    },
  },
  {
    name: "machine structural search",
    args: [
      "calcit/test.cirru",
      "query",
      "search",
      "defstruct",
      "--filter",
      "test-record.main/Person",
      "--parent-path",
      "--format",
      "json",
    ],
    check(result) {
      if (result.command !== "query.search" || result.data.summary.matches !== 1) {
        throw new Error("query.search JSON summary is incorrect");
      }
      const match = result.data.definitions[0]?.matches[0];
      if (match?.path !== "code@0" || match?.parent_path !== "code") {
        throw new Error("query.search did not expose stable edit paths");
      }
    },
  },
  {
    name: "staged edit transaction",
    args: [
      "calcit/test.cirru",
      "edit",
      "transaction",
      "--code",
      `[]
  [] |config |version |9.0.0
  []
    , |tree
    , |replace
    , |app.main/main!
    , |--path
    , |@48.1
    , |--code
    quote false`,
      "--dry-run",
      "--format",
      "json",
    ],
    check(result) {
      if (result.schema_version !== 1 || result.command !== "edit.transaction") {
        throw new Error("unexpected edit.transaction envelope");
      }
      if (!result.dry_run || !result.changed || result.operations.length !== 2) {
        throw new Error("edit.transaction did not preserve dry-run batch semantics");
      }
      if (!result.original_revision.startsWith("md5:") || !result.new_revision.startsWith("md5:")) {
        throw new Error("edit.transaction did not expose snapshot revisions");
      }
    },
  },
  {
    name: "machine value schema",
    args: [
      "calcit/test.cirru",
      "query",
      "schema",
      "app.main/*ref-demo",
      "--json",
    ],
    check(result) {
      if (
        result.command !== "query.schema" ||
        result.data.id !== "app.main/*ref-demo"
      ) {
        throw new Error("query.schema JSON envelope is incorrect");
      }
      if (result.data.canonical_schema !== ":: 'Ref 'Number") {
        throw new Error("parameterized value schema was not preserved");
      }
      if (JSON.stringify(result.data.tree) !== '["::","\'Ref","\'Number"]') {
        throw new Error("parameterized value schema tree is incorrect");
      }
    },
  },
  {
    name: "type coverage analysis",
    args: [
      "calcit/test.cirru",
      "analyze",
      "check-types",
      "--ns",
      "app.main",
      "--only",
      "none",
      "--format",
      "json",
    ],
    check(result) {
      if (result.command !== "analyze.check-types" || result.data.summary.definitions === 0) {
        throw new Error("type coverage result is incomplete");
      }
      if (!result.data.definitions.every((definition) => definition.coverage === "none")) {
        throw new Error("check-types --only filter was not preserved");
      }
      if (typeof result.data.summary.polymorphism?.generic_definitions !== "number") {
        throw new Error("check-types lost polymorphism evidence counts");
      }
    },
  },
  {
    name: "intentional dynamic analysis",
    args: [
      "calcit/test.cirru",
      "analyze",
      "weak-types",
      "--ns",
      "test-js.main",
      "--intent",
      "intentional-js-ffi",
      "--format",
      "json",
    ],
    check(result) {
      if (result.command !== "analyze.weak-types" || result.data.summary.hits === 0) {
        throw new Error("weak type result is incomplete");
      }
      const occurrences = result.data.definitions.flatMap((definition) => definition.occurrences);
      if (!occurrences.every((occurrence) => occurrence.intent === "intentional-js-ffi")) {
        throw new Error("weak-types --intent filter was not preserved");
      }
      if (!occurrences.every((occurrence) => typeof occurrence.suggestion === "string" && typeof occurrence.impact === "string")) {
        throw new Error("weak-type occurrences lost impact or actionable suggestions");
      }
    },
  },
  {
    name: "summary-only type coverage",
    args: [
      "calcit/test.cirru",
      "analyze",
      "check-types",
      "--ns",
      "test-record.main",
      "--summary-only",
      "--format",
      "json",
    ],
    check(result) {
      if (result.data.summary.definitions === 0) {
        throw new Error("summary-only coverage lost aggregate counts");
      }
      if (result.data.definitions.length !== 0 || result.data.filters.summary_only !== true) {
        throw new Error("summary-only coverage still emitted definition rows");
      }
    },
  },
];

const rows = [];
for (const scenario of scenarios) {
  const started = process.hrtime.bigint();
  const child = spawnSync(binary, scenario.args, {
    cwd: process.cwd(),
    encoding: "utf8",
    maxBuffer: 4 * 1024 * 1024,
  });
  const elapsedMs = Number(process.hrtime.bigint() - started) / 1e6;

  if (child.error) {
    throw child.error;
  }
  if (child.status !== 0) {
    throw new Error(`${scenario.name} failed (${child.status}):\n${child.stderr}`);
  }

  let parsed;
  try {
    parsed = JSON.parse(child.stdout);
  } catch (error) {
    throw new Error(`${scenario.name} polluted JSON stdout:\n${child.stdout}\n${error}`);
  }
  scenario.check(parsed);
  rows.push({
    scenario: scenario.name,
    milliseconds: elapsedMs.toFixed(2),
    stdoutBytes: Buffer.byteLength(child.stdout),
  });
}

console.log(`Agent interface smoke passed: ${rows.length}/${scenarios.length}`);
console.table(rows);
