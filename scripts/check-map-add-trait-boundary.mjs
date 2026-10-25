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

// Importing this fixture factory must not execute CLI/host checks. The
// dedicated compatibility-policy script reuses exactly the same negatives.
export function traitRejectionCases() {
  const duplicateStruct = (impls, effect = false) => `let
    Person $ impl-traits Person0 ${impls}
    p $ %{} Person $ :name |Alice${effect ? "\n    counter $ ref 0" : ""}
  assert= true $ try
    do
      &trait-call MyFoo :foo p${effect ? " $ reset! counter $ + @counter 1" : ""}
      , false
    fn (error)
      hint-fn $ {}
        :args $ [] 'String
        :return 'Bool
      &str:includes? error |duplicate${effect ? "\n  assert= 1 @counter" : ""}`;
  const duplicateEnum = impls => `let
    Person $ impl-traits Demo0 ${impls}
    p $ %:: Person :demo 1
  assert= true $ try
    do (&trait-call MyBar :bar p) false
    fn (error)
      hint-fn $ {}
        :args $ [] 'String
        :return 'Bool
      &str:includes? error |duplicate`;
  const staleOrigin = `let
    make-trait $ fn ()
      hint-fn $ {}
        :args $ []
        :return 'Trait
      deftrait FreshShow $ .render :fn
    old-trait $ make-trait
    next-trait $ make-trait
    old-impl $ defimpl OldImpl old-trait $ .render
      fn (self) |old
    Person $ impl-traits Person0 old-impl
    p $ %{} Person $ :name |Alice
  assert= |old $ &trait-call old-trait :render p
  assert= true $ try
    do (&trait-call next-trait :render p) false
    fn (error)
      hint-fn $ {}
        :args $ [] 'String
        :return 'Bool
      &str:includes? error "|cannot find impl"`;
  return [
    ["duplicate-origin-runtime", duplicateStruct("MyFooImpl MyFooImpl2")],
    ["duplicate-origin-reversed", duplicateStruct("MyFooImpl2 MyFooImpl")],
    ["duplicate-origin-repeated", duplicateStruct("MyFooImpl MyFooImpl")],
    ["duplicate-enum-origin", duplicateEnum("MyBarImpl MyBarImpl2")],
    ["duplicate-enum-reversed", duplicateEnum("MyBarImpl2 MyBarImpl")],
    ["duplicate-arguments-once", duplicateStruct("MyFooImpl MyFooImpl2", true)],
    ["fresh-origin-rejects-stale-impl", staleOrigin, /E_UNPROVEN_TRAIT_CALL/],
  ];
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
      assert.match(detail, /W_GENERIC_WHERE_BOUND_MISMATCH.*trait Add/,
        "a Map must fail the generic Add bound, not an unrelated check");
      rejected = true;
    }
    assert.ok(rejected, "legacy Map .add must not satisfy the nominal Add trait bound");

    // These original source negatives now belong to compile rejection, not
    // attached runtime tests. Keep every target gate; host tests cover effects.
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
    for (const [name, code, diagnostic = /E_DUPLICATE_TRAIT_IMPL/] of sourceNegatives) {
      setRuntimeBody(code);
      for (const mode of [[], ["--check-only"], ["js"], ["wasm"], ["wasi"]]) {
        assert.throws(() => run(snapshot, ...selection, ...mode), error => {
          assert.match(`${error.stdout ?? ""}\n${error.stderr ?? ""}`, diagnostic);
          return true;
        }, `${name}: strict selection must reject before execution or codegen`);
      }
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
      "quote $ deftrait RejectedSignature $ .value $ :: 'Fn $ {} (:args $ [] 'Number 'Dynamic) (:return 'Number)");
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
