import { spawnSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import assert from "node:assert/strict";

const binary = process.env.CALCIT_AGENT_BIN ?? process.env.CALCIT_AGENT_CR ?? "./target/debug/calcit";

const scenarios = [
  {
    name: "complete definition metadata",
    args: ["calcit/test.cirru", "query", "def", "app.main/main!", "--raw", "--format", "json"],
    check(result) {
      if (result.schema_version !== 1 || result.command !== "query.def" || result.data.id !== "app.main/main!") {
        throw new Error("unexpected query.def envelope");
      }
      if (!Array.isArray(result.data.code) || result.data.ffi !== null || result.data.ffi_edn !== null || !result.revision.startsWith("md5:")) {
        throw new Error("query.def lost code, absent metadata, or revision");
      }
    },
  },
  {
    name: "builtin definition metadata",
    args: ["calcit/test.cirru", "query", "def", "calcit.core/to-js-data", "--format", "json"],
    check(result) {
      if (result.command !== "query.def" || !result.data.builtin || result.data.code !== null || result.data.ffi !== null) {
        throw new Error("query.def lost builtin metadata contract");
      }
      if (!result.data.tags.includes("js-ffi")) {
        throw new Error("query.def lost special-builtin semantic tags");
      }
    },
  },
  {
    name: "typed FFI Interface IR",
    args: ["calcit/test.cirru", "ffi", "export", "--json"],
    check(result) {
      if (result.schema_version !== 1 || result.command !== "ffi.export") {
        throw new Error("unexpected ffi.export envelope");
      }
      if (!result.interface_schema?.endsWith("ffi-interface-ir-v3.schema.json")) {
        throw new Error("ffi.export did not identify its Interface IR schema");
      }
      if (result.data.interface.version !== 3 || !Array.isArray(result.data.interface.declarations) || !result.revision.startsWith("md5:")) {
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
      "%none",
      "--exact",
      "--source",
      "project",
      "--filter",
      "app.main/query-search-node-kinds",
      "--parent-path",
      "--format",
      "json",
    ],
    check(result) {
      if (result.command !== "query.search" || result.data.summary.matches !== 2) {
        throw new Error("query.search JSON summary is incorrect");
      }
      const definition = result.data.definitions[0];
      if (definition?.source !== "project" || definition?.origin.package !== "app") {
        throw new Error("query.search did not expose the project definition origin");
      }
      const matches = definition.matches;
      if (matches[0]?.cursor_index !== 0 || matches[0]?.path !== "code@2.1" || matches[0]?.node_kind !== "leaf") {
        throw new Error("query.search did not identify the bare constructor leaf");
      }
      if (matches[1]?.cursor_index !== 1 || matches[1]?.path !== "code@2.2.0" || matches[1]?.node_kind !== "call") {
        throw new Error("query.search did not identify the invoked zero-argument constructor");
      }
      if (matches.some((match) => match.source !== "project" || match.origin.package !== "app")) {
        throw new Error("query.search matches lost their project origins");
      }
    },
  },
  {
    name: "core-only structural search",
    args: ["calcit/test.cirru", "query", "search", "%none", "--exact", "--source", "core", "--format", "json"],
    check(result) {
      const countedMatches = result.data.definitions.reduce((sum, definition) => sum + definition.match_count, 0);
      const wrongOrigin = result.data.definitions.some(
        (definition) => definition.source !== "core" || definition.origin.package !== "calcit" || definition.origin.module !== "builtin",
      );
      if (
        result.data.summary.matches === 0 ||
        result.data.summary.matches !== countedMatches ||
        result.data.summary.definitions !== result.data.definitions.length ||
        wrongOrigin
      ) {
        throw new Error("core-only query.search summary or origins are inconsistent");
      }
    },
  },
  {
    name: "dependency-only structural search",
    args: ["calcit/test.cirru", "query", "search", "%none", "--exact", "--source", "deps", "--format", "json"],
    check(result) {
      const countedMatches = result.data.definitions.reduce((sum, definition) => sum + definition.match_count, 0);
      const wrongOrigin = result.data.definitions.some(
        (definition) => definition.source !== "deps" || !definition.origin.module?.startsWith("./"),
      );
      if (
        result.data.summary.matches === 0 ||
        result.data.summary.matches !== countedMatches ||
        result.data.summary.definitions !== result.data.definitions.length ||
        wrongOrigin
      ) {
        throw new Error("dependency-only query.search summary or origins are inconsistent");
      }
    },
  },
  {
    name: "all-source structural search",
    args: ["calcit/test.cirru", "query", "search", "%none", "--exact", "--source", "all", "--format", "json"],
    check(result) {
      const countedMatches = result.data.definitions.reduce((sum, definition) => sum + definition.match_count, 0);
      const sources = new Set(result.data.definitions.map((definition) => definition.source));
      if (
        result.data.summary.matches === 0 ||
        result.data.summary.matches !== countedMatches ||
        result.data.summary.definitions !== result.data.definitions.length ||
        [...sources].sort().join(",") !== "core,deps,project"
      ) {
        throw new Error("all-source query.search summary or origins are inconsistent");
      }
    },
  },
  {
    name: "machine-readable config entries",
    args: ["calcit/type-fail/type-slot-entry-scope.cirru", "config", "show", "--format", "json"],
    check(result) {
      if (result.schema_version !== 1 || result.command !== "config.show" || result.diagnostics.length !== 0) {
        throw new Error("unexpected config.show envelope");
      }
      if (result.data.entries.map((entry) => entry.name).join(",") !== "default,server") {
        throw new Error("config.show entries are not deterministically ordered");
      }
      const server = result.data.entries[1];
      if (server.mode !== "native" || server.type_slots["dispatch-op"] !== "type-fail-type-slot-entry-scope.main/ServerOp") {
        throw new Error("config.show lost entry mode or type slots");
      }
      if (!result.revision.startsWith("md5:")) {
        throw new Error("config.show lost its Snapshot revision");
      }
    },
  },
  {
    name: "core library config inventory",
    args: ["src/cirru/calcit-core.cirru", "config", "show", "--format", "json"],
    check(result) {
      const entry = result.data?.entries?.[0];
      if (result.command !== "config.show" || result.data?.package !== "calcit" || entry?.name !== "default") {
        throw new Error("calcit-core automation could not read its config inventory");
      }
      if (entry.target !== null || entry.modules.length !== 0 || Object.keys(entry.type_slots).length !== 0) {
        throw new Error("calcit-core empty config fields did not round-trip");
      }
    },
  },
  {
    name: "machine-readable config modules",
    args: ["calcit/test.cirru", "config", "modules", "--format", "json"],
    check(result) {
      if (result.command !== "config.modules" || result.data.entry.name !== "default") {
        throw new Error("unexpected config.modules envelope");
      }
      if (result.data.modules.length !== result.data.entry.modules.length || result.data.modules.some((module) => module.status !== "loaded")) {
        throw new Error("config.modules lost resolved module status");
      }
    },
  },
  {
    name: "machine-readable config type slots",
    args: ["calcit/type-fail/type-slot-entry-scope.cirru", "config", "type-slots", "--entry", "server", "--format", "json"],
    check(result) {
      if (result.command !== "config.type-slots" || result.data.entry.name !== "server") {
        throw new Error("unexpected config.type-slots envelope");
      }
      if (result.data.type_slots["dispatch-op"] !== "type-fail-type-slot-entry-scope.main/ServerOp") {
        throw new Error("config.type-slots lost the selected binding");
      }
    },
  },
  {
    name: "missing config entry stays structured",
    args: ["calcit/test.cirru", "config", "show", "--entry", "missing", "--format", "json"],
    expectedStatus: 1,
    check(result) {
      if (result.command !== "config.show" || result.data !== null || result.diagnostics[0]?.code !== "E_CONFIG_ENTRY_NOT_FOUND") {
        throw new Error("missing config entry lost its structured diagnostic");
      }
    },
  },
  {
    name: "malformed config stays structured",
    args: ["package.json", "config", "show", "--format", "json"],
    expectedStatus: 1,
    check(result) {
      if (result.command !== "config.show" || result.data !== null || result.diagnostics[0]?.code !== "E_CONFIG_SNAPSHOT_INVALID") {
        throw new Error("malformed config lost its structured diagnostic");
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
    , |@40.1
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
    name: "compiler-guided fix preview",
    args: [
      "tests/fixtures/fix-command.cirru",
      "fix",
      "--ns",
      "fix-command.main",
      "--def",
      "fixable",
      "--rule",
      "removed-data-api-v1",
      "--format",
      "json",
    ],
    check(result) {
      if (result.schema_version !== 1 || result.command !== "fix" || result.data.mode !== "preview") {
        throw new Error("unexpected fix preview envelope");
      }
      const suggestion = result.data.suggestions[0];
      const rule = result.data.filters.expanded_rules?.[0];
      if (
        result.data.changed !== true ||
        suggestion?.rule_id !== "removed-data-api-v1" ||
        suggestion?.diagnostic_code !== "W_REMOVED_DATA_API" ||
        rule?.rule_id !== "removed-data-api-v1" ||
        rule?.diagnostic_code !== "W_REMOVED_DATA_API" ||
        rule?.evidence_source !== "current-diagnostic" ||
        rule?.lifecycle !== "current-semantics" ||
        rule?.source_version_required !== false
      ) {
        throw new Error("fix preview lost compiler evidence or deterministic rule identity");
      }
      if (
        suggestion.path !== "code@3.0" ||
        suggestion.applicability !== "machine-applicable" ||
        suggestion.original?.value !== "tuple-enum" ||
        suggestion.replacement?.value !== "enum-definition"
      ) {
        throw new Error("fix preview lost its source path or quoted AST replacement");
      }
    },
  },
  {
    name: "strict project workflow manifest",
    args: ["tests/fixtures/fix-command.cirru", "fix", "--workflow", "strict", "--format", "json"],
    check(result) {
      const workflow = result.data?.workflow;
      if (result.schema_version !== 1 || result.command !== "fix" || workflow?.workflow !== "strict-v1") {
        throw new Error("unexpected strict workflow envelope");
      }
      if (workflow.mode !== "preview" || workflow.status !== "planned" || workflow.safe_fixes?.preset !== "surface-latest-v2") {
        throw new Error("strict workflow lost its plan or safe preset identity");
      }
      if (!workflow.resume?.revision?.startsWith("md5:") || workflow.resume?.apply_command?.[0] !== "calcit") {
        throw new Error("strict workflow lost its resumable revision-bound command");
      }
      if (
        !Array.isArray(workflow.entries) ||
        !Array.isArray(workflow.review_required?.type_findings) ||
        !Array.isArray(workflow.retained_type_boundaries)
      ) {
        throw new Error("strict workflow lost project inventory fields");
      }
      if (workflow.verification?.commands?.[0]?.[0] !== "calcit" || workflow.verification?.external_commands?.length !== 0) {
        throw new Error("strict workflow verification commands are not explicit token lists");
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
      if (result.schema_version !== 8 || result.command !== "analyze.weak-types" || result.data.summary.hits === 0) {
        throw new Error("weak type result is incomplete");
      }
      const occurrences = result.data.definitions.flatMap((definition) => definition.occurrences);
      if (!occurrences.every((occurrence) => occurrence.intent === "intentional-js-ffi")) {
        throw new Error("weak-types --intent filter was not preserved");
      }
      if (!occurrences.every((occurrence) => typeof occurrence.path === "string" && typeof occurrence.detail === "string")) {
        throw new Error("weak-type occurrences lost source evidence");
      }
    },
  },
  {
    name: "FFI boundary evidence",
    args: [
      "tests/fixtures/ffi-boundary-evidence.cirru",
      "analyze",
      "weak-types",
      "--ffi-evidence",
      "--format",
      "json",
    ],
    check(result) {
      const boundaries = result.data?.evidence?.ffi_boundaries;
      if (result.schema_version !== 8 || !Array.isArray(boundaries) || boundaries.length === 0) {
        throw new Error("FFI evidence envelope is incomplete");
      }
      const boundary = boundaries.find((item) => item.definition === "ffi-evidence.main/query-host");
      if (boundary?.classification !== "mixed" || boundary?.target !== "browser") {
        throw new Error("FFI evidence lost boundary classification or target");
      }
      if (boundary.diagnostic_code !== "I_FFI_BOUNDARY_EVIDENCE") {
        throw new Error("FFI evidence lost its stable informational code");
      }
      if (!boundary.operations.some((item) => item.classification === "npm-import")) {
        throw new Error("FFI evidence lost npm import provenance");
      }
      if (boundary.helper_candidates?.[0]?.origin !== "dependency") {
        throw new Error("FFI evidence did not prefer an exact-schema dependency helper");
      }
      if (result.data.evidence.runtime_trust_inferred !== false) {
        throw new Error("FFI evidence must remain review-only");
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
    name: "strict bundled core public source check",
    args: [
      "calcit/test-wasi-command.cirru",
      "analyze",
      "check-public",
      "--ns",
      "calcit.core",
      "--ns",
      "calcit.test",
      "--ns",
      "calcit.internal",
      "--deps",
      "--summary-only",
      "--format",
      "json",
    ],
    expectedStatus: 0,
    check(result) {
      const summary = result.data?.summary;
      if (result.schema_version !== 1 || result.command !== "analyze.check-public") {
        throw new Error("bundled core check lost its structured envelope");
      }
      if (
        summary?.complete !== true ||
        summary?.passed !== true ||
        summary?.definitions_checked !== summary?.definitions_selected ||
        summary?.definitions_passed !== summary?.definitions_selected ||
        summary?.definitions_source_passed + summary?.definitions_intrinsic !== summary?.definitions_selected ||
        summary?.definitions_intrinsic === 0 ||
        result.diagnostics.length !== 0
      ) {
        throw new Error("bundled core source and intrinsic coverage is incomplete");
      }
      if (!result.data.checked_definition_ids.includes("calcit.core/unsafe-coerce") || result.data.definitions.length !== 0) {
        throw new Error("bundled core summary omitted checked definitions or included detailed rows");
      }
    },
  },
  {
    name: "target-aware public check failure",
    args: [
      "calcit/type-fail/js-nullish-dereference-strict.cirru",
      "analyze",
      "check-public",
      "--ns",
      "type-fail-js-nullish-dereference-strict.main",
      "--format",
      "json",
    ],
    expectedStatus: 1,
    check(result) {
      if (result.schema_version !== 1 || result.command !== "analyze.check-public") {
        throw new Error("unexpected analyze.check-public envelope");
      }
      if (result.data.target !== "node" || result.data.summary.complete !== true || result.data.summary.passed !== false) {
        throw new Error("check-public lost target, completeness, or failure state");
      }
      if (result.data.checked_definition_ids.length !== 2 || result.diagnostics.length === 0) {
        throw new Error("check-public omitted checked definition IDs or diagnostics");
      }
    },
  },
  {
    name: "strict public check failure stays structured",
    strictTypes: true,
    args: [
      "calcit/type-fail/js-nullish-dereference-strict.cirru",
      "analyze",
      "check-public",
      "--ns",
      "type-fail-js-nullish-dereference-strict.main",
      "--format",
      "json",
    ],
    expectedStatus: 1,
    check(result) {
      if (result.schema_version !== 1 || result.command !== "analyze.check-public") {
        throw new Error("strict check-public lost its structured envelope");
      }
      if (result.data.summary.complete !== false || result.data.summary.definitions_checked !== 0) {
        throw new Error("strict preflight failure reported partial public coverage");
      }
      if (result.diagnostics[0]?.code !== "E_PUBLIC_CHECK_STRICT_PREFLIGHT") {
        throw new Error("strict preflight failure lost its structured diagnostic");
      }
    },
  },
  {
    name: "keep-going strict check failure stays structured",
    defaultStrict: true,
    args: [
      "calcit/type-fail/js-nullish-dereference-strict.cirru",
      "--check-only",
      "--keep-going",
      "--format",
      "json",
    ],
    expectedStatus: 1,
    check(result) {
      if (result.schema_version !== 1 || result.command !== "check-only") {
        throw new Error("keep-going check lost its structured envelope");
      }
      if (result.data.summary.failed !== 1 || result.data.summary.blocked !== 0 || result.data.status !== "failed") {
        throw new Error("keep-going check lost confirmed and blocked status counts");
      }
      const failed = result.data.definitions.find((definition) => definition.status === "failed");
      if (failed?.diagnostics[0]?.code !== "E_JS_FFI_NULLABLE_DEREF" || !Array.isArray(failed.diagnostics[0].path)) {
        throw new Error("keep-going check lost stable code or source path");
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
  const fixtureArgs = scenario.defaultStrict
    ? scenario.args
    : scenario.strictTypes
      ? [scenario.args[0], "--strict-types", ...scenario.args.slice(1)]
      : scenario.args;
  const child = spawnSync(binary, fixtureArgs, {
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

// Keep EDN native while comparing its envelope semantics against JSON interoperability.
const normalizeEdnKeys = (value, reference) => {
  if (Array.isArray(value)) return value.map((child, index) => normalizeEdnKeys(child, reference?.[index]));
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, child]) => {
        const raw = key.startsWith(":") ? key.slice(1) : key;
        const normalized = reference && Object.hasOwn(reference, raw) ? raw : raw.replaceAll("-", "_");
        return [normalized, normalizeEdnKeys(child, reference?.[normalized])];
      }),
    );
  }
  return value;
};
const parseEdnRaw = (text, name) => {
  const parsed = spawnSync(binary, ["cirru", "parse-edn", text], { encoding: "utf8", maxBuffer: 4 * 1024 * 1024 });
  assert.equal(parsed.status, 0, `${name}: EDN output is not one parseable document:\n${text}\n${parsed.stderr}`);
  return JSON.parse(parsed.stdout);
};
const parseEdnEnvelope = (text, name, reference) => normalizeEdnKeys(parseEdnRaw(text, name), reference);
const querySnapshotBefore = readFileSync("calcit/test.cirru");
for (const { name, base, expectedStatus = 0, check } of [
  {
    name: "builtin type EDN",
    base: ["calcit/test.cirru", "query", "type", "'String"],
    check: (result) => assert.ok(result.data.methods.some((method) => method.name === ".contains?")),
  },
  {
    name: "variadic list method EDN",
    base: ["calcit/test.cirru", "query", "type", ":: 'List 'Number"],
    check: (result) => {
      const method = result.data.methods.find((item) => item.name === ".concat");
      assert.equal(method?.status, "proven");
      assert.deepEqual(method.parameter_types, []);
      assert.equal(method.rest_type, "list<number>");
      assert.equal(method.return_type, "list<number>");
      assert.equal(method.definition, "calcit.core/&list:concat");
    },
  },
  {
    name: "runtime-only context EDN",
    base: ["calcit/test.cirru", "query", "context", "calcit.core/&list:contains?", "--budget", "1800"],
    check: (result) => {
      assert.equal(result.data.code.cirru, "&runtime-implementation");
      assert.ok(result.diagnostics.some((item) => item.code === "I_SOURCE_BODY_UNAVAILABLE"));
    },
  },
  {
    name: "source definition EDN",
    base: ["calcit/test.cirru", "query", "def", "app.main/main!"],
    check: (result) => assert.equal(result.data.id, "app.main/main!"),
  },
  {
    name: "expression type EDN",
    base: ["calcit/test.cirru", "query", "type-at", "test-struct.main/sum-point", "--path", "code@3.1"],
    check: (result) => assert.equal(result.data.inferred_type, "'Number"),
  },
  {
    name: "source search EDN",
    base: ["calcit/test.cirru", "query", "search", "main!", "--filter", "app.main/main!"],
    check: (result) => assert.ok(result.data.summary.matches > 0),
  },
  {
    name: "type mismatch diagnostics EDN",
    base: [
      "calcit/type-fail/schema-call-arg-type-mismatch.cirru",
      "query",
      "type-at",
      "type-fail-schema-call-arg-type.main/main!",
      "--path",
      "code@3",
    ],
    check: (result) => assert.ok(result.diagnostics.some((item) => item.code === "W_FN_ARG_TYPE_MISMATCH")),
  },
  {
    name: "invalid type EDN failure",
    base: ["calcit/test.cirru", "query", "type", "not-a-type"],
    expectedStatus: 1,
    check: (result) => assert.equal(result.diagnostics[0].code, "E_QUERY_INVALID_TYPE"),
  },
  {
    name: "invalid expression path EDN failure",
    base: ["calcit/test.cirru", "query", "type-at", "test-struct.main/sum-point", "--path", "code@999"],
    expectedStatus: 1,
    check: (result) => {
      assert.equal(result.schema_version, 2);
      assert.equal(result.diagnostics[0].code, "E_QUERY_INVALID_PATH");
    },
  },
  {
    name: "missing definition EDN failure",
    base: ["calcit/test.cirru", "query", "context", "calcit.core/not-there"],
    expectedStatus: 1,
    check: (result) => assert.equal(result.diagnostics[0].code, "E_QUERY_TARGET_NOT_FOUND"),
  },
  {
    name: "invalid search path EDN failure",
    base: ["calcit/test.cirru", "query", "search", "main!", "--filter", "app.main/main!", "--start-path", "banana"],
    expectedStatus: 1,
    check: (result) => {
      assert.equal(result.command, "query.search");
      assert.equal(result.diagnostics[0].code, "E_QUERY_INVALID_PATH");
    },
  },
  {
    name: "invalid expression search EDN failure",
    base: ["calcit/test.cirru", "query", "search-expr", "foo ("],
    expectedStatus: 1,
    check: (result) => {
      assert.equal(result.command, "query.search-expr");
      assert.equal(result.diagnostics[0].code, "E_QUERY_FAILED");
    },
  },
  {
    name: "entry config EDN",
    base: ["calcit/test.cirru", "config", "show"],
    check: (result) => assert.equal(result.data.package, "app"),
  },
  {
    name: "query config EDN",
    base: ["calcit/test.cirru", "query", "config"],
    check: (result) => assert.equal(result.command, "config.show"),
  },
  {
    name: "entry modules EDN",
    base: ["calcit/test.cirru", "config", "modules"],
    check: (result) => assert.equal(result.data.entry.name, "default"),
  },
  {
    name: "entry type slots EDN",
    base: ["calcit/test.cirru", "config", "type-slots"],
    check: (result) => assert.equal(result.data.entry.name, "default"),
  },
  {
    name: "missing entry EDN failure",
    base: ["calcit/test.cirru", "config", "modules", "--entry", "not-there"],
    expectedStatus: 1,
    check: (result) => assert.equal(result.diagnostics[0].code, "E_CONFIG_ENTRY_NOT_FOUND"),
  },
]) {
  const json = spawnSync(binary, [...base, "--format", "json"], { encoding: "utf8", maxBuffer: 4 * 1024 * 1024 });
  const edn = spawnSync(binary, [...base, "--format", "edn"], { encoding: "utf8", maxBuffer: 4 * 1024 * 1024 });
  assert.equal(json.status, expectedStatus, `${name}: JSON failed:\n${json.stderr}`);
  assert.equal(edn.status, expectedStatus, `${name}: EDN failed:\n${edn.stderr}`);
  const jsonEnvelope = JSON.parse(json.stdout);
  const ednEnvelope = parseEdnEnvelope(edn.stdout, name, jsonEnvelope);
  assert.deepEqual(ednEnvelope, jsonEnvelope, `${name}: EDN and JSON semantics diverged`);
  check(ednEnvelope);
}
assert.deepEqual(readFileSync("calcit/test.cirru"), querySnapshotBefore, "query and config reads must not mutate the Snapshot");

const unicodeDefsChild = spawnSync(binary, ["calcit/test-wasi-command.cirru", "query", "defs", "app.main"], {
  cwd: process.cwd(),
  encoding: "utf8",
  maxBuffer: 4 * 1024 * 1024,
  env: { ...process.env, NO_COLOR: "1" },
});
assert.ifError(unicodeDefsChild.error);
assert.equal(unicodeDefsChild.status, 0, unicodeDefsChild.stderr);
assert.match(unicodeDefsChild.stdout, /用确定性的 WASI 假宿主验证时钟编号与纳秒到毫秒的换算。/);
assert.match(
  unicodeDefsChild.stdout,
  /filesystem-read-error\?.*泄漏\.\.\./,
  "long definition summaries must keep the ASCII ellipsis format",
);

const contractChild = spawnSync(binary, ["docs", "agents", "--contract"], {
  cwd: process.cwd(),
  encoding: "utf8",
  maxBuffer: 64 * 1024,
  env: { ...process.env, NO_COLOR: "1" },
});
assert.ifError(contractChild.error);
assert.equal(contractChild.status, 0, contractChild.stderr);
assert.match(contractChild.stdout, /Agent mutation contract: v1/);
assert.match(contractChild.stdout, /Agent contract digest: md5:[0-9a-f]{32}/);
assert.match(contractChild.stdout, /calcit docs read edit-tree\.md 'Atomic Transactions'/);
assert.ok(Buffer.byteLength(contractChild.stdout) < 5_000, "compact mutation contract should remain bounded");

const remoteLibsHelp = spawnSync(binary, ["docs", "remote-libs", "--help"], { encoding: "utf8" });
assert.ifError(remoteLibsHelp.error);
assert.equal(remoteLibsHelp.status, 0, remoteLibsHelp.stderr);
assert.match(remoteLibsHelp.stdout, /readme/);
assert.match(remoteLibsHelp.stdout, /search/);
const topLevelHelp = spawnSync(binary, ["--help"], { encoding: "utf8" });
assert.ifError(topLevelHelp.error);
assert.equal(topLevelHelp.status, 0, topLevelHelp.stderr);
assert.doesNotMatch(topLevelHelp.stdout, /^  libs\s/m);
const retiredLibs = spawnSync(binary, ["libs", "search", "web"], { encoding: "utf8" });
assert.ifError(retiredLibs.error);
assert.notEqual(retiredLibs.status, 0, "the retired top-level libs command must not reappear");
assert.match(retiredLibs.stderr, /Unrecognized argument: search/);

const stdinEval = spawnSync(binary, ["eval", "--stdin"], { encoding: "utf8", input: "range 3\n" });
assert.ifError(stdinEval.error);
assert.equal(stdinEval.status, 0, stdinEval.stderr);
assert.match(stdinEval.stdout, /\(\[\] 0 1 2\)/);
const dependencyEval = spawnSync(binary, [
  "eval", "--stdin", "--dep", "./calcit/util.cirru", "--dep", "./examples/wasi-http-client/calcit.cirru",
], {
  encoding: "utf8",
  input: "util.core/log-title |dependency-ready\ncomponent-wasm-async-import.starter/main!\n",
});
assert.ifError(dependencyEval.error);
assert.equal(dependencyEval.status, 0, dependencyEval.stderr);
assert.match(dependencyEval.stdout, /dependency-ready/);
assert.match(dependencyEval.stdout, /took .*: 0/);
for (const [args, input, message] of [
  [["eval"], "", /No snippet provided/],
  [["eval", "--stdin"], "", /No snippet read from stdin/],
  [["eval", "--stdin", "range 3"], "range 3", /Choose either a positional snippet/],
]) {
  const invalidEval = spawnSync(binary, args, { encoding: "utf8", input });
  assert.ifError(invalidEval.error);
  assert.equal(invalidEval.status, 1);
  assert.match(invalidEval.stderr, message);
}
const retiredExec = spawnSync(binary, ["exec", "--dep", "test.cirru"], { encoding: "utf8", input: "range 3" });
assert.ifError(retiredExec.error);
assert.equal(retiredExec.status, 1);
assert.match(retiredExec.stderr, /Unrecognized argument: --dep/);
assert.doesNotMatch(topLevelHelp.stdout, /^  exec\s/m);
const evalHelp = spawnSync(binary, ["eval", "--help"], { encoding: "utf8" });
assert.ifError(evalHelp.error);
assert.equal(evalHelp.status, 0, evalHelp.stderr);
assert.match(evalHelp.stdout, /--stdin/);
assert.match(topLevelHelp.stdout, /ir\s+diagnostic compiler output/);

// Real CLI round trips, including the legacy wire format and negative paths.
const fixtureDir = mkdtempSync(join(tmpdir(), "calcit-query-def-"));
try {
  const fixture = join(fixtureDir, "calcit.cirru");
  copyFileSync("calcit/test.cirru", fixture);
  const run = (...args) => {
    const child = spawnSync(binary, [fixture, ...args], { encoding: "utf8", maxBuffer: 4 * 1024 * 1024 });
    assert.ifError(child.error);
    return child;
  };
  const names = Array.from({ length: 300 }, (_, i) => `(:member-${i} ${JSON.stringify(`|宿主\\\"\n${i}`)})`).join(" ");
  const ffi = `{} (:backend :js) (:names $ {} ${names} (:foo_bar |first) (:foo-bar |second))`;
  const edit = run("edit", "ffi", "app.main/main!", "--code", ffi);
  assert.equal(edit.status, 0, edit.stderr);
  const query = (...flags) => run("query", "def", "app.main/main!", ...flags);
  const machine = query("--format", "json", "--json", "--raw");
  assert.equal(machine.status, 0, machine.stderr);
  const data = JSON.parse(machine.stdout).data;
  const nativeEdn = query("--format", "edn");
  assert.equal(nativeEdn.status, 0, nativeEdn.stderr);
  const nativeEnvelope = parseEdnRaw(nativeEdn.stdout, "definition FFI EDN");
  assert.deepEqual(nativeEnvelope[":data"][":ffi"], data.ffi, "EDN FFI must remain native rather than JSON-encoded metadata");
  const jsonEnvelope = JSON.parse(machine.stdout);
  const normalizedEnvelope = normalizeEdnKeys(nativeEnvelope, jsonEnvelope);
  normalizedEnvelope.data.ffi = data.ffi;
  assert.deepEqual(normalizedEnvelope, jsonEnvelope);
  assert.equal(Object.keys(data.ffi[":names"]).length, 302);
  assert.equal(data.ffi[":names"][":member-299"], "宿主\\\"\n299");
  assert.equal(data.ffi[":names"][":foo_bar"], "first");
  assert.equal(data.ffi[":names"][":foo-bar"], "second");
  const roundTrip = run("cirru", "parse-edn", data.ffi_edn);
  assert.equal(roundTrip.status, 0, roundTrip.stderr);
  assert.deepEqual(JSON.parse(roundTrip.stdout), data.ffi);
  const legacy = query("--raw", "--json");
  assert.equal(legacy.status, 0, legacy.stderr);
  const legacyJson = legacy.stdout.match(/\n## JSON\n\n(`{3,})json\n([\s\S]*?)\n\1\n?$/);
  assert.ok(legacyJson, "legacy --json output must append a fenced JSON block");
  assert.equal(JSON.parse(legacyJson[2]).ffi, data.ffi_edn);
  assert.ok(legacy.stdout.includes(data.ffi_edn), "raw human FFI must also be complete");
  assert.match(query().stdout, /FFI \(preview; use --raw/);
  for (const [args, structured] of [
    [["query", "def", "app.main/nonexistent-875", "--format", "json"], true],
    [["query", "def", "app.main/main!", "--format", "invalid"], false],
  ]) {
    const failed = run(...args);
    assert.notEqual(failed.status, 0);
    if (structured) {
      const envelope = JSON.parse(failed.stdout);
      assert.equal(envelope.data, null, "query failure must not emit partial success data");
      assert.equal(envelope.diagnostics[0].code, "E_QUERY_TARGET_NOT_FOUND");
    } else {
      assert.equal(failed.stdout, "", "unknown output format has no parseable structured contract");
    }
    assert.ok(failed.stderr.length > 0);
  }
  const inertEntry = run("edit", "def", "app.main/main!", "--overwrite", "--code", "quote $ defn main! () $ raise |query-must-not-run");
  assert.equal(inertEntry.status, 0, inertEntry.stderr);
  const beforeRead = readFileSync(fixture);
  const readOnlyQuery = run("query", "context", "app.main/main!", "--format", "edn");
  assert.equal(readOnlyQuery.status, 0, "queries must not execute the project's init function");
  assert.equal(parseEdnEnvelope(readOnlyQuery.stdout, "inert entry read").command, "query.context");
  assert.deepEqual(readFileSync(fixture), beforeRead, "query must leave the Snapshot unchanged");
} finally {
  rmSync(fixtureDir, { recursive: true, force: true });
}

for (const rule of ["tag-match-to-match-v1", "required-struct-field-v1"]) {
  const retired = spawnSync(
    binary,
    ["tests/fixtures/fix-command.cirru", "fix", "--rule", rule, "--format", "json"],
    { cwd: process.cwd(), encoding: "utf8", maxBuffer: 4 * 1024 * 1024 },
  );
  assert.ifError(retired.error);
  assert.notEqual(retired.status, 0, `${rule} must stay outside the 0.15 fix surface`);
  assert.match(retired.stderr, /Calcit 0\.14\.15/);
  assert.match(retired.stderr, /before upgrading/);
}

console.log(
  `Agent interface smoke passed: ${rows.length}/${scenarios.length}, plus retired migration, mutation contract, and definition protocol checks`,
);
console.table(rows);
