import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFile, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const run = (...args) => execFileSync(binary, args, {
  encoding: "utf8", stdio: "pipe", timeout: 60000, maxBuffer: 16 * 1024 * 1024,
});
const traitSnapshot = resolve("calcit/test-traits.cirru");

// Legacy local `impl-traits` attachment and duplicate top-level impl negatives,
// rejected before execution or codegen.
export function traitRejectionCases() {
  const legacyAttachmentCases = [
    ["legacy-impl-traits", "let ((Person (impl-traits PlainPerson MyFooImpl))) (%{} Person (:name |Alice))", /E_LEGACY_DYNAMIC_STYLE/],
    ["legacy-struct-def-impl-traits", "&struct-def:impl-traits PlainPerson MyFooImpl", /E_LEGACY_DYNAMIC_STYLE/],
    ["legacy-enum-def-impl-traits", "&enum-def:impl-traits Demo0 MyBarImpl", /E_LEGACY_DYNAMIC_STYLE/],
    ["legacy-struct-impl-traits", "&struct:impl-traits (PlainPerson :name |Alice) MyFooImpl", /E_LEGACY_DYNAMIC_STYLE/],
    ["legacy-enum-impl-traits", "&enum:impl-traits (Demo0 :demo 1) MyBarImpl", /E_LEGACY_DYNAMIC_STYLE/],
  ];
  return legacyAttachmentCases;
}

async function checkTraitBoundaries() {
  run(traitSnapshot, "test", "test-traits.main/add-with-trait", "--require-match");
  run(traitSnapshot);

  const fixture = await mkdtemp(join(tmpdir(), "calcit-map-add-trait-"));
  try {
    await symlink(resolve("node_modules"), join(fixture, "node_modules"), "dir");
    const output = join(fixture, "js-out");
    run(traitSnapshot, "--emit-path", output, "js");
    const traits = await import(pathToFileURL(join(output, "test-traits.main.mjs")).href);
    traits.main_$x_();

    const snapshot = join(fixture, "calcit.cirru");
    await copyFile(traitSnapshot, snapshot);
    run(snapshot, "edit", "def", "test-traits.main/invalid-map-add-trait", "--input-format", "cirru", "--code",
      "quote $ defn invalid-map-add-trait ()\n  add-with-trait (&{} :a 1) (&{} :b 2)");
    let rejected = false;
    try {
      run(snapshot, "--init-fn", "test-traits.main/invalid-map-add-trait", "--reload-fn",
        "test-traits.main/invalid-map-add-trait", "--check-only");
    } catch (error) {
      const detail = `${error.stdout ?? ""}\n${error.stderr ?? ""}`;
      assert.match(detail, /E_GENERIC_WHERE_BOUND_MISMATCH.*trait Add/,
        "a Map must fail the generic Add bound, not an unrelated check");
      rejected = true;
    }
    assert.ok(rejected, "legacy Map .add must not satisfy the nominal Add trait bound");
    run(snapshot, "edit", "rm-def", "test-traits.main/invalid-map-add-trait");

    // These original source negatives now belong to compile rejection, not
    // attached runtime tests. Keep every target gate; host tests cover effects.
    run(snapshot, "edit", "def", "test-traits.main/PlainPerson", "--input-format", "cirru", "--code",
      "quote $ defstruct PlainPerson (:name String)");
    const sourceNegatives = traitRejectionCases();
    const setRuntimeBody = code => {
      // The private rejection entry returns Number 1, not the source routine's Unit.
      const edits = JSON.stringify([
        ["edit", "def", "test-traits.main/test-explicit-trait-call", "--overwrite", "--input-format", "cirru", "--code",
          `quote $ defn test-explicit-trait-call ()\n${code.split("\n").map(line => `  ${line}`).join("\n")}\n  , 1`],
        ["edit", "schema", "test-traits.main/test-explicit-trait-call", "--input-format", "json-ast", "--code",
          JSON.stringify(["::", "'Fn", ["{}", [":args", ["[]"]], [":return", "'Number"]]])],
      ]);
      const preview = JSON.parse(run(snapshot, "edit", "transaction", "--code", edits, "--dry-run", "--format", "json"));
      return run(snapshot, "edit", "transaction", "--code", edits, "--expect-revision", preview.scoped_revision, "--format", "edn");
    };
    const selection = ["--init-fn", "test-traits.main/test-explicit-trait-call",
      "--reload-fn", "test-traits.main/test-explicit-trait-call"];
    for (const [name, code, diagnostic] of sourceNegatives) {
      setRuntimeBody(code);
      for (const mode of [[], ["--check-only"], ["js"], ["wasm"], ["wasi"]]) {
        assert.throws(() => run(snapshot, ...selection, ...mode), error => {
          assert.match(`${error.stdout ?? ""}\n${error.stderr ?? ""}`, diagnostic);
          return true;
        }, `${name}: strict selection must reject before execution or codegen`);
      }
    }
    // Duplicate top-level impls on a statically coherent type are rejected at compile/codegen boundaries.
    run(snapshot, "edit", "def", "test-traits.main/DupPerson", "--input-format", "cirru", "--code",
      "quote $ defstruct DupPerson (:name String) (:impls MyFoo)");
    run(snapshot, "edit", "def", "test-traits.main/DupPersonImpl1", "--input-format", "cirru", "--code",
      "quote $ defimpl DupPerson MyFoo (.foo (fn (self) |a))");
    run(snapshot, "edit", "def", "test-traits.main/DupPersonImpl2", "--input-format", "cirru", "--code",
      "quote $ defimpl DupPerson MyFoo (.foo (fn (self) |b))");
    setRuntimeBody("&trait-call MyFoo :foo (DupPerson :name |Alice)");
    for (const mode of [[], ["--check-only"], ["js"], ["wasm"], ["wasi"]]) {
      assert.throws(() => run(snapshot, ...selection, ...mode), error => {
        assert.match(`${error.stdout ?? ""}\n${error.stderr ?? ""}`, /E_DUPLICATE_TRAIT_IMPL/);
        return true;
      }, "duplicate top-level defimpl must reject across every target");
    }
    for (const defName of ["DupPerson", "DupPersonImpl1", "DupPersonImpl2"]) {
      run(snapshot, "edit", "rm-def", `test-traits.main/${defName}`);
    }
    // Public deftrait accepts declarations, not a third default-body element.
    // Native's internal defaults field is not a user-facing source feature.
    setRuntimeBody("let\n    WithDefault $ deftrait WithDefault $ .render :fn $ fn (self) |default\n  , &unit");
    for (const mode of [[], ["--check-only"], ["js"], ["wasm"], ["wasi"]]) {
      assert.throws(() => run(snapshot, ...selection, ...mode), error => {
        assert.match(`${error.stdout ?? ""}\n${error.stderr ?? ""}`, /deftrait expects each method as \(method type\)/);
        return true;
      }, "an unsupported default body must fail macro validation on every target");
    }
    // Reuse runtime method validation before declaring a quoted signature checked.
    for (const [name, signature] of [
      ["direct-dynamic", "'Dynamic"],
      ["dynamic-argument", "(:: 'Fn ({} (:args ([] 'Number 'Dynamic)) (:return 'Number)))"],
      ["dynamic-result", "(:: 'Fn ({} (:args ([] 'Number)) (:return 'Dynamic)))"],
      ["nested-dynamic", "(:: 'Fn ({} (:args ([] (:: 'List 'Dynamic))) (:return 'Number)))"],
      ["named-dynamic-argument", "(:: 'Fn ({} (:args ([] (:: 'Option 'Dynamic))) (:return 'Number)))"],
      ["named-dynamic-result", "(:: 'Fn ({} (:args ([] 'Number)) (:return (:: 'Option 'Dynamic))))"],
      ["dynamic-callback", "(:: 'Fn ({} (:args ([] (:: 'Fn ({} (:args ([] 'Number)) (:return 'Dynamic))))) (:return 'Number)))"],
      ["dynamic-rest", "(:: 'Fn ({} (:args ([] 'Number)) (:rest 'Dynamic) (:return 'Number)))"],
    ]) {
      setRuntimeBody(`let\n    InvalidSignature $ deftrait InvalidSignature $ .value ${signature}\n  , &unit`);
      for (const mode of [[], ["--check-only"], ["js"], ["wasm"], ["wasi"]]) {
        assert.throws(() => run(snapshot, ...selection, ...mode), error => {
          const detail = `${error.stdout ?? ""}\n${error.stderr ?? ""}`;
          assert.match(detail, /preprocessing/);
          assert.doesNotMatch(detail, /Check passed/);
          assert.match(detail, /&trait::new does not allow Dynamic (?:in|inside) method signatures/);
          assert.match(detail, /at test-traits\.main\/test-explicit-trait-call @/);
          return true;
        }, `${name}: invalid method metadata must fail before execution or codegen`);
      }
    }
    // Computed metadata still reaches the same validator at native runtime.
    for (const signature of [
      ":: 'Fn $ {} (:args $ [] $ :: 'Option 'Dynamic) (:return 'Number)",
      ":: 'Fn $ {} (:args $ [] 'Number) (:return $ :: 'Option 'Dynamic)",
    ]) {
      setRuntimeBody(`let\n    signature $ fn ()\n      quote $ ${signature}\n  &trait::new :ComputedSignature $ [] $ [] :value (signature)`);
      assert.match(run(snapshot, ...selection, "--check-only"), /Check passed/);
      assert.throws(() => run(snapshot, ...selection), error => {
        const detail = `${error.stdout ?? ""}\n${error.stderr ?? ""}`;
        assert.doesNotMatch(detail, /preprocessing/);
        assert.match(detail, /&trait::new does not allow Dynamic inside method signatures/);
        return true;
      }, "computed named type arguments must be validated at native runtime");
    }
    setRuntimeBody("&unit");
    run(snapshot, "config", "set", "target", "native");
    run(snapshot, "edit", "def", "test-traits.main/RejectedSignature", "--input-format", "cirru", "--code",
      "quote $ deftrait RejectedSignature $ .value $ quote $ Fn Number Dynamic (-> Number)");
    for (const mode of [["--check-only", "--ns", "test-traits.main"], ["--check-only", "--all-defs"],
      ["fix", "--workflow", "strict", "--verify", "--format", "json"]]) {
      assert.throws(() => run(snapshot, ...selection, ...mode), error => {
        const detail = `${error.stdout ?? ""}\n${error.stderr ?? ""}`;
        assert.match(detail, /&trait::new does not allow Dynamic inside method signatures/);
        assert.match(detail, /test-traits\.main\/RejectedSignature/);
        return true;
      }, `a cold unreferenced trait declaration must fail ${mode.join(" ")}`);
    }
    console.log("Trait boundaries passed: native/JS semantics, duplicate/default rejection, and static signatures across compiler targets and cold strict checks");
  } finally {
    await rm(fixture, { recursive: true, force: true });
  }
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  await checkTraitBoundaries();
}
