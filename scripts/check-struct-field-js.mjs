import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdtemp, readFile, readdir, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const binary = resolve(process.env.CALCIT_BIN ?? "target/debug/calcit");
const nativeTrace = execFileSync(binary, ["calcit/test-wasm.cirru", "test", "--tag", "struct-field-order", "--require-match"], { encoding: "utf8" });
assert.deepEqual(nativeTrace.split(/\r?\n/).filter(line => line.startsWith("struct-order-")),
  ["struct-order-y", "struct-order-x", "struct-order-x", "struct-order-y"]);
const nativeFailure = execFileSync(binary, ["calcit/test-wasm.cirru", "test", "--tag", "struct-field-failure", "--require-match"], { encoding: "utf8" });
assert.deepEqual(nativeFailure.split(/\r?\n/).filter(line => line.startsWith("struct-failure-")), ["struct-failure-y"]);
execFileSync(binary, ["calcit/test-struct.cirru", "test", "--tag", "struct-shape", "--require-match"], { stdio: "pipe" });
execFileSync(binary, ["calcit/test-struct.cirru", "test", "--tag", "struct-field-names", "--require-match"], { stdio: "pipe" });
const typeQuery = JSON.parse(execFileSync(binary, ["calcit/test-wasm.cirru", "query", "type", "test-wasm.main/Point", "--format", "json"], { encoding: "utf8" }));
const fieldMethod = typeQuery.data.methods.find((method) => method.name === ".contains-field?");
assert.equal(fieldMethod?.status, "proven", "Agent method discovery must prove the Struct field contract");
assert.deepEqual(fieldMethod.parameter_types, ["tag"]);
assert.equal(fieldMethod.return_type, "bool");
const invalid = "let ((Point (defstruct Point (:x 'Number))) (point (%{} Point (:x 1)))) (point .contains-field? |x)";
const mismatch = spawnSync(binary, ["src/cirru/calcit-core.cirru", "eval", invalid], { encoding: "utf8" });
assert.notEqual(mismatch.status, 0, "String field names must fail strict type checking");
assert.match(mismatch.stderr, /W_METHOD_ARG_TYPE_MISMATCH/);

const output = await mkdtemp(join(tmpdir(), "calcit-struct-field-js-"));
try {
  await symlink(resolve("node_modules"), join(output, "node_modules"), "dir");
  execFileSync(binary, ["--emit-path", output, "calcit/test-wasm.cirru", "js"], { stdio: "pipe" });
  const compiled = await import(pathToFileURL(join(output, "test-wasm.main.mjs")).href);
  const trace = [];
  const originalLog = console.log;
  try {
    console.log = (...values) => trace.push(values.join(" "));
    assert.equal(compiled.test_struct_field_order(), 1);
    assert.equal(compiled.test_struct_field_order_forward(), 1);
    assert.deepEqual(trace, ["struct-order-y", "struct-order-x", "struct-order-x", "struct-order-y"]);
    trace.length = 0;
    assert.throws(() => compiled.test_struct_field_order_failure(0));
    assert.deepEqual(trace, ["struct-failure-y"]);
  } finally {
    console.log = originalLog;
  }
  assert.equal(compiled.test_struct_contains_field(), 1, "generated JS must preserve the Tag field predicate");
  const shapeOutput = join(output, "struct-shape");
  execFileSync(binary, ["calcit/test-struct.cirru",
    "--init-fn", "test-struct.main/test-struct-complete-construction",
    "--reload-fn", "test-struct.main/test-struct-complete-construction",
    "--emit-path", shapeOutput, "js"], { stdio: "pipe" });
  const shapeJs = await import(pathToFileURL(join(shapeOutput, "test-struct.main.mjs")).href);
  assert.equal(shapeJs.test_struct_complete_construction(), 1,
    "complete constructors must preserve aliases and dynamic prototypes");
  assert.equal(compiled.test_struct_nominal_equality(), 1, "generated JS must preserve definition identity and structural equality");
  assert.equal(compiled.test_struct_hash(), 1, "equal nested Struct values must have equal hashes");
  assert.equal(compiled.test_struct_map_key(), 1, "Map keys must preserve nominal Struct identity");
  assert.equal(compiled.test_struct_container_hash(), 1, "hashing must recurse through containers of nominal values");
  assert.equal(compiled.test_struct_layout_identity(), 1, "same-named definitions must retain their own field layout");
  assert.equal(compiled.test_struct_edn_identity(), 1, "typed EDN decoding must restore the requested definition identity");

  // Replay the actual attached expressions, including normal method calls,
  // imported schema aliases, two Db types and unrelated open history payloads.
  const appliedFixture = "tests/fixtures/applied-struct-evidence.cirru";
  execFileSync(binary, [appliedFixture, "--check-only", "--all-defs"], { stdio: "pipe" });
  execFileSync(process.execPath, ["scripts/run-core-tests.mjs", "--snapshot", appliedFixture,
    "--tag", "applied-struct-evidence", "--backend", "native,js"], {
    env: { ...process.env, CALCIT_BIN: binary }, stdio: "pipe",
  });
  execFileSync(process.execPath, ["scripts/run-core-tests.mjs", "--snapshot", appliedFixture,
    "--tag", "applied-struct-closed", "--backend", "native,js,wasm"], {
    env: { ...process.env, CALCIT_BIN: binary }, stdio: "pipe",
  });

  // A field write the checker leaves to runtime goes through the generated JS
  // matcher; each outcome follows the native `value_matches_type_annotation`.
  const writeOutput = join(output, "checked-field-write");
  execFileSync(binary, ["--emit-path", writeOutput, appliedFixture, "js"], { stdio: "pipe" });
  const procs = await import(pathToFileURL(resolve("lib/calcit.procs.mjs")).href);
  const boxes = await import(pathToFileURL(join(writeOutput, "app.checked-write.mjs")).href);
  const makeBox = (def, fields) =>
    procs._$n__PCT__$M_(def, ...Object.entries(fields).flatMap(([name, value]) => [procs.newTag(name), value]));
  const writeField = (box, name, value) => procs._$n_struct_$o_with(box, procs.newTag(name), value);
  const accepts = (box, name, value) => assert.doesNotThrow(() => writeField(box, name, value), `:${name} must accept ${procs.toString(value, true)}`);
  const rejects = (box, name, value) => assert.throws(() => writeField(box, name, value), /expects type/, `:${name} must reject ${procs.toString(value, true)}`);

  // Nil is `null` and Unit is `undefined`; only JsNullish admits both.
  const nilBox = makeBox(boxes.NilBox, { opt: null, none: null, host: null });
  accepts(nilBox, "opt", null);
  accepts(nilBox, "opt", 1);
  rejects(nilBox, "opt", undefined);
  rejects(nilBox, "opt", "1");
  accepts(nilBox, "none", null);
  rejects(nilBox, "none", undefined);
  accepts(nilBox, "host", null);
  accepts(nilBox, "host", undefined);
  accepts(nilBox, "host", 2);
  rejects(nilBox, "host", "2");

  // Numeric refinements check integrality and range in both spellings.
  const numBox = makeBox(boxes.NumBox, { small: 1, tiny: 1, wide: 1, single: 0.5 });
  accepts(numBox, "small", -128);
  rejects(numBox, "small", 300);
  rejects(numBox, "small", 1.5);
  rejects(numBox, "small", "1");
  accepts(numBox, "tiny", 127);
  rejects(numBox, "tiny", 128);
  accepts(numBox, "wide", 65535);
  rejects(numBox, "wide", -1);
  accepts(numBox, "single", 0.25);
  rejects(numBox, "single", 0.1);

  // calcit.core Struct and Enum references admit only values of that definition.
  const core = await import(pathToFileURL(join(writeOutput, "calcit.core.mjs")).href);
  const foreignEnums = await import(pathToFileURL(join(writeOutput, "app.foreign-nominal.mjs")).href);
  const { CalcitStructDef } = await import(pathToFileURL(resolve("lib/js-struct-def.mjs")).href);
  const { CalcitStructValue } = await import(pathToFileURL(resolve("lib/js-struct-value.mjs")).href);
  const { valueMatchesTypeForm } = await import(pathToFileURL(resolve("lib/js-type-form.mjs")).href);
  const { CalcitSymbol } = await import(pathToFileURL(resolve("lib/calcit-data.mjs")).href);
  const foreignOptionDef = new CalcitStructDef(procs.newTag("Option"), [], [], [], "foreign.schema/Option");
  const foreignOption = new CalcitStructValue(procs.newTag("Option"), [], [], foreignOptionDef);
  const variant = (def, tag, ...payload) => procs._PCT__$o__$o_(def, procs.newTag(tag), ...payload);
  const none = variant(core.Option, "none");
  const nominalBox = makeBox(boxes.CoreNominalBox, { maybe: none, qualified: none, outcome: variant(core.Result, "ok", 1) });
  accepts(nominalBox, "maybe", variant(core.Option, "some", 2));
  rejects(nominalBox, "maybe", 2);
  rejects(nominalBox, "maybe", null);
  rejects(nominalBox, "maybe", variant(core.Result, "ok", 2));
  accepts(nominalBox, "qualified", variant(core.Option, "some", 3));
  rejects(nominalBox, "qualified", 3);
  rejects(nominalBox, "maybe", foreignOption);
  rejects(nominalBox, "qualified", foreignOption);
  // Replay generated definitions, not hand-built identities: same names in
  // another namespace cannot satisfy a core Enum's nominal field contract.
  assert.equal(core.Option.prototype.structRef.definitionRef, "calcit.core/Option");
  assert.equal(core.Result.prototype.structRef.definitionRef, "calcit.core/Result");
  assert.equal(foreignEnums.Option.prototype.structRef.definitionRef, "app.foreign-nominal/Option");
  assert.equal(foreignEnums.Result.prototype.structRef.definitionRef, "app.foreign-nominal/Result");
  rejects(nominalBox, "maybe", variant(foreignEnums.Option, "some", 2));
  rejects(nominalBox, "qualified", variant(foreignEnums.Option, "some", 3));
  rejects(nominalBox, "outcome", variant(foreignEnums.Result, "ok", 1));
  const { bind_struct_definition } = await import(pathToFileURL(resolve("lib/js-struct-def.mjs")).href);
  const coreOptionAlias = bind_struct_definition(core.Option, "app.foreign-nominal/OptionAlias");
  assert.equal(coreOptionAlias, core.Option, "alias binding must preserve the original Enum identity");
  accepts(nominalBox, "maybe", variant(coreOptionAlias, "some", 4));
  assert.equal(core.Option.withImpls([]).prototype.structRef.definitionRef, "calcit.core/Option");
  assert.equal(foreignEnums.Option.withImpls([]).prototype.structRef.definitionRef, "app.foreign-nominal/Option");
  rejects(nominalBox, "qualified", variant(foreignEnums.Option.withImpls([]), "some", 5));
  const mapEntry = makeBox(core.MapEntry, { key: procs.newTag("key"), value: 1 });
  assert.equal(valueMatchesTypeForm(mapEntry, new CalcitSymbol("calcit.core/MapEntry")), true);
  assert.equal(valueMatchesTypeForm(mapEntry, new CalcitSymbol("MapEntry")), true);
  const foreignEntryDef = new CalcitStructDef(procs.newTag("MapEntry"), [], [], [], "foreign.schema/MapEntry");
  const foreignEntry = new CalcitStructValue(procs.newTag("MapEntry"), [], [], foreignEntryDef);
  assert.equal(valueMatchesTypeForm(foreignEntry, new CalcitSymbol("calcit.core/MapEntry")), false);
  assert.equal(valueMatchesTypeForm(foreignEntry, new CalcitSymbol("MapEntry")), false);
  accepts(nominalBox, "outcome", variant(core.Result, "err", "failed"));
  rejects(nominalBox, "outcome", "failed");

  const snapshot = join(output, "applied-struct-negative.cirru");
  const a = "model/ReelLike :base (data/DbA :value 1) :db (data/DbA :value 2) :records ([]) :merged? false";
  const b = "model/ReelLike :base (data/DbB :value |one) :db (data/DbB :value |two) :records ([]) :merged? false";
  const failures = [
    ["mixed-db", "model/ReelLike :base (data/DbA :value 1) :db (data/DbB :value |two) :records ([]) :merged? false"],
    ["wrong-updater", `model/step (${a}) append-b`],
    ["wrong-concrete-argument", `read-a $ ${b}`],
    ["wrong-local-alias", `let ((alias (${b}))) (read-a alias)`],
    ["wrong-nested-field", `read-nested-a $ model/OuterLike :reel $ ${b}`],
    ["wrong-method-update", `.assoc (${a}) :db $ data/DbB :value |wrong`],
    ["borrowed-return", b, ":: 'model/ReelLike 'data/DbA"],
    ["bare-cannot-prove-applied", "read-a reel", "'Number", "'model/ReelLike", a],
    ["open-cannot-prove-applied", "read-a reel", "'Number", "(:: 'model/ReelLike 'Dynamic)", a],
    ["unbound-cannot-prove-applied", "read-a reel", "'Number", "(:: 'model/ReelLike 'Db)", a, "'Db"],
    ["borrowed-generic-return", "reel", "(:: 'model/ReelLike 'Other)", "(:: 'model/ReelLike 'Db)", a, "'Db 'Other"],
    ["borrowed-generic-concrete-return", b, "(:: 'model/ReelLike 'Db)", "(:: 'model/ReelLike 'Db)", a, "'Db"],
  ];
  for (const [name, expression, returned = "'Unit", input, argument, generic] of failures) {
    await copyFile(appliedFixture, snapshot);
    const body = `quote $ defn invalid! (${input ? "reel" : ""})\n  ${expression}${returned === "'Unit" ? "\n  , &unit" : ""}`;
    const operations = [
      ["edit", "def", "app.applied-reader/invalid!", "--input-format", "cirru", "--code", body],
      ["edit", "schema", "app.applied-reader/invalid!", "--input-format", "cirru", "--code",
        `quote $ :: 'Fn $ {} (:args $ []${input ? ` ${input}` : ""}) (:return ${returned})${generic ? ` (:generics $ [] ${generic})` : ""}`],
      ["edit", "def", "app.applied-reader/main!", "--input-format", "cirru", "--code",
        `quote $ defn main! ()\n  invalid!${input ? ` $ ${argument}` : ""}\n  , &unit`],
      ["edit", "schema", "app.applied-reader/main!", "--input-format", "cirru", "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)"],
      ["config", "set", "init-fn", "app.applied-reader/main!"],
      ["config", "set", "reload-fn", "app.applied-reader/main!"],
    ];
    // This Node host consumes JSON explicitly; source remains quoted Cirru.
    const mutation = [snapshot, "edit", "transaction", "--code", JSON.stringify(operations), "--format", "json"];
    const before = await readFile(snapshot);
    const preview = JSON.parse(execFileSync(binary, [...mutation, "--dry-run"], { encoding: "utf8" }));
    assert.deepEqual(await readFile(snapshot), before);
    assert.match(preview.original_revision, /^md5:/);
    execFileSync(binary, [...mutation, "--expect-revision", preview.original_revision], { stdio: "pipe" });
    const original = await readFile(snapshot);
    const modes = name.startsWith("borrowed-generic-")
      ? [["fix", "--workflow", "strict", "--verify", "--format", "json"]]
      : [["--check-only"], ["js"]];
    for (const mode of modes) {
      const destination = join(output, name);
      const result = spawnSync(binary, ["--emit-path", destination, snapshot, ...mode], { encoding: "utf8" });
      assert.ifError(result.error);
      assert.equal(result.signal, null);
      assert.equal(result.status, 1, `${name} ${mode}\n${result.stdout}\n${result.stderr}`);
      let diagnostics = `${result.stdout}\n${result.stderr}`;
      if (mode[0] === "fix") {
        const report = JSON.parse(result.stdout);
        assert.ok(report.diagnostics.some(diagnostic => diagnostic.definition === "app.applied-reader/invalid!"
          && diagnostic.code === "E_ERASED_GENERIC_RELATION"), diagnostics);
      }
      if (mode[0] === "js" && result.stderr.includes("codegen blocked")) {
        assert.deepEqual(await readdir(destination), ["calcit.build-errors.mjs"]);
        diagnostics += await readFile(join(destination, "calcit.build-errors.mjs"), "utf8");
      }
      const expectedDiagnostic = name === "wrong-method-update"
        ? /struct update field `:db` expects type/
        : name === "borrowed-return"
          ? /W_FN_RETURN_TYPE_MISMATCH/
          : name.startsWith("borrowed-generic-")
            ? /E_ERASED_GENERIC_RELATION/
            : name === "open-cannot-prove-applied"
              ? /E_DYNAMIC_NOMINAL_ARGUMENT/
              : /W_FN_ARG_TYPE_MISMATCH/;
      assert.match(diagnostics, expectedDiagnostic, `${name} ${mode}`);
      assert.deepEqual(await readFile(snapshot), original);
    }
  }
  console.log("Applied Struct evidence: all-defs entry checking, 4 native/JS attached tests, JS runtime field-write checks, 1 shared WASM test, 10 native/JS strict rejections and 2 rigid generic return audits.");
} finally {
  await rm(output, { recursive: true, force: true });
}
