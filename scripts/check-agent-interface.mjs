import { spawnSync } from "node:child_process";

const binary = process.env.CALCIT_AGENT_BIN ?? process.env.CALCIT_AGENT_CR ?? "./target/debug/calcit";

const scenarios = [
  {
    name: "typed FFI Interface IR",
    args: ["calcit/test.cirru", "ffi", "export", "--json"],
    check(result) {
      if (result.schema_version !== 1 || result.command !== "ffi.export") {
        throw new Error("unexpected ffi.export envelope");
      }
      if (!result.interface_schema?.endsWith("ffi-interface-ir-v2.schema.json")) {
        throw new Error("ffi.export did not identify its Interface IR schema");
      }
      if (result.data.interface.version !== 2 || !Array.isArray(result.data.interface.declarations) || !result.revision.startsWith("md5:")) {
        throw new Error("ffi.export lost its versioned deterministic interface");
      }
      if (!Array.isArray(result.data.interface.definitions) || !Array.isArray(result.diagnostics)) {
        throw new Error("ffi.export omitted inventory or diagnostics");
      }
    },
  },
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
      "test-struct.main/sum-point",
      "--path",
      "code@3.1",
      "--format",
      "json",
    ],
    check(result) {
      if (result.schema_version !== 2 || result.command !== "query.type-at") {
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
      "test-struct.main/Person",
      "--format",
      "json",
    ],
    check(result) {
      if (result.data.resolved_from !== "definition inference") {
        throw new Error("query.type did not infer a source-backed defstruct");
      }
      if (!result.data.methods.some((method) => method.name === ".assoc")) {
        throw new Error("source-backed struct methods are missing");
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
      "test-struct.main/Person",
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
  [] |edit |doc |app.main/main! "|Updated by transaction"
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
      if (result.schema_version !== 5 || result.command !== "analyze.weak-types" || result.data.summary.hits === 0) {
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
      "test-struct.main",
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
  {
    name: "dynamic method summary",
    args: [
      "calcit/test-method-errors.cirru",
      "analyze",
      "dynamic-methods",
      "--summary-only",
      "--format",
      "json",
    ],
    check(result) {
      if (result.schema_version !== 1 || result.command !== "analyze.dynamic-methods") {
        throw new Error("unexpected analyze.dynamic-methods envelope");
      }
      if (result.data.summary.findings !== 2 || result.data.findings.length !== 0) {
        throw new Error("dynamic-methods summary did not preserve aggregate-only output");
      }
      if (result.data.summary.passed !== true || !result.revision.startsWith("md5:")) {
        throw new Error("dynamic-methods summary lost policy or revision metadata");
      }
    },
  },
  {
    name: "dynamic method policy failure",
    args: [
      "calcit/test-method-errors.cirru",
      "analyze",
      "dynamic-methods",
      "--max",
      "1",
      "--format",
      "json",
    ],
    expectedStatus: 1,
    check(result) {
      if (result.data.summary.findings !== 2 || result.data.summary.passed !== false) {
        throw new Error("dynamic-methods policy did not fail above its limit");
      }
      if (result.data.findings.length !== 2) {
        throw new Error("dynamic-methods policy omitted fixture finding rows");
      }
      if (result.data.findings.some((finding) => !finding.code?.startsWith("P_DYNAMIC_"))) {
        throw new Error("dynamic-methods report included unrelated warnings");
      }
      if (result.diagnostics[0]?.code !== "E_DYNAMIC_METHOD_POLICY") {
        throw new Error("dynamic-methods policy failure lost its structured diagnostic");
      }
    },
  },
  {
    name: "dynamic method project scope",
    args: [
      "calcit/test-dynamic-method-scope.cirru",
      "analyze",
      "dynamic-methods",
      "--summary-only",
      "--format",
      "json",
    ],
    check(result) {
      if (result.data.summary.findings !== 0 || result.data.filters.include_dependencies !== false) {
        throw new Error("dynamic-methods default scope leaked dependency findings");
      }
    },
  },
  {
    name: "dynamic method dependency scope",
    args: [
      "calcit/test-dynamic-method-scope.cirru",
      "analyze",
      "dynamic-methods",
      "--deps",
      "--summary-only",
      "--format",
      "json",
    ],
    check(result) {
      if (result.data.summary.findings !== 2 || result.data.filters.include_dependencies !== true) {
        throw new Error("dynamic-methods --deps lost reachable module findings");
      }
    },
  },
  {
    name: "static quality gate failure",
    args: [
      "calcit/test.cirru",
      "analyze",
      "quality",
      "--ns",
      "app.main",
      "--format",
      "json",
    ],
    expectedStatus: 1,
    check(result) {
      if (result.schema_version !== 2 || result.command !== "analyze.quality") {
        throw new Error("unexpected analyze.quality envelope");
      }
      if (result.data.passed !== false || result.data.mode !== "strict-zero") {
        throw new Error("quality gate did not preserve strict failure semantics");
      }
      if (result.data.violations.length === 0 || result.diagnostics[0]?.code !== "E_STATIC_QUALITY_REGRESSION") {
        throw new Error("quality gate failure lost regressions or structured diagnostics");
      }
      if (typeof result.data.metrics.unsafeCoerce !== "number") {
        throw new Error("quality gate v2 lost the unsafeCoerce metric");
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
  const expectedStatus = scenario.expectedStatus ?? 0;
  if (child.status !== expectedStatus) {
    throw new Error(`${scenario.name} returned ${child.status}, expected ${expectedStatus}:\n${child.stderr}`);
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
