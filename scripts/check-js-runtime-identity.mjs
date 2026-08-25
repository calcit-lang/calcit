import assert from "node:assert/strict";
import { cp, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const fixtureRoot = await mkdtemp(join(tmpdir(), "calcit-js-runtime-identity-"));

try {
  const runtimeAPath = join(fixtureRoot, "runtime-a", "lib");
  const runtimeBPath = join(fixtureRoot, "runtime-b", "lib");
  await cp(new URL("../lib", import.meta.url), runtimeAPath, { recursive: true });
  await cp(new URL("../lib", import.meta.url), runtimeBPath, { recursive: true });
  await symlink(resolve("node_modules"), join(fixtureRoot, "node_modules"), "dir");

  const runtimeA = await import(pathToFileURL(join(runtimeAPath, "calcit.procs.mjs")).href);
  const runtimeB = await import(pathToFileURL(join(runtimeBPath, "calcit.procs.mjs")).href);
  assert.equal(runtimeA.get_env, runtimeA._$n_get_env, "legacy get_env export should delegate to the raw proc");
  assert.equal(runtimeA.get_env("CALCIT_MISSING_ENV_FOR_RUNTIME_TEST", "fallback"), "fallback");
  assert.equal(
    runtimeA._$n_str_$o_replace("a&a&", "&", "&amp;"),
    "a&amp;a&amp;",
    "string replacement must finish when the replacement contains the pattern"
  );
  assert.equal(
    runtimeA._$n_str_$o_replace("a.b", ".", "$&"),
    "a$&b",
    "string replacement must treat patterns and replacement text literally"
  );
  assert.equal(runtimeA._$n_str_$o_replace("ab", "", "-"), "-a-b-", "empty string patterns should replace each boundary");

  const todoName = runtimeA.newTag("TodoState");
  const todoField = runtimeA.newTag("draft");
  const todoType = new runtimeA.CalcitSymbol("String");
  const todoRecord = new runtimeA.CalcitStructValue(todoName, [todoField], [""]);
  const todoStruct = new runtimeA.CalcitStructDef(todoName, [todoField], [todoType]);
  const todoEnum = new runtimeA.CalcitEnumDef(new runtimeA.CalcitStructValue(todoName, [todoField], [todoType]));
  const todoEnumValue = new runtimeA.CalcitEnumValue(todoField, [""], todoEnum);
  const anonymousEnumValue = new runtimeA.CalcitEnumValue(todoField, [""]);
  assert.equal(todoRecord.toString(), "(%{} 'TodoState (:draft |))");
  assert.equal(todoStruct.toString(), "(%struct-def 'TodoState (:draft 'String))");
  assert.equal(todoEnum.toString(), "(%enum-def 'TodoState)");
  assert.equal(todoEnumValue.toString(), "(%:: 'TodoState :draft |)");
  assert.equal(anonymousEnumValue.toString(), "(%:: _ :draft |)");

  assert.equal(runtimeA.type_of(null).value, "nil", "nil must keep its own runtime type");
  assert.equal(runtimeA.type_of(undefined).value, "unit", "&unit must keep its own runtime type");
  assert.equal(runtimeA.nil_$q_(null), true);
  assert.equal(runtimeA.nil_$q_(undefined), false, "&unit must not satisfy nil?");
  assert.equal(runtimeA._$n__$e_(null, undefined), false, "nil and &unit must not compare equal");
  assert.notEqual(runtimeA.hashFunction(null), runtimeA.hashFunction(undefined), "nil and &unit need distinct hashes");
  assert.equal(runtimeA.toString(null, true), "nil");
  assert.equal(runtimeA.toString(undefined, true), "&unit");
  assert.throws(() => runtimeA.json_stringify(undefined), /cannot encode value: &unit/);
  assert.throws(() => runtimeA.to_cirru_edn(undefined), /cannot encode &unit/);
  const nilShape = { version: 2, root: 0, fingerprint: "runtime-nil-shape", nodes: [{ kind: "nil" }] };
  assert.equal(runtimeA.parse_cirru_edn_as("do nil", nilShape), null, "typed EDN decoding must preserve the Nil node");
  assert.throws(
    () => runtimeA.parse_cirru_edn_as("do |value", nilShape),
    /expected Nil, got string/,
    "typed EDN decoding must reject non-Nil input for the Nil node"
  );

  const effectRef = runtimeA.atom(1);
  const watchKey = runtimeA.newTag("runtime-unit-check");
  const watchCalls = [];
  assert.equal(
    runtimeA.add_watch(effectRef, watchKey, (next, previous) => watchCalls.push([next, previous])),
    undefined,
    "add-watch must return &unit"
  );
  assert.equal(runtimeA.reset_$x_(effectRef, 2), 2, "reset! must return the written value");
  assert.deepEqual(watchCalls, [[2, 1]], "reset! must still notify watchers");
  assert.equal(runtimeA.remove_watch(effectRef, watchKey), undefined, "remove-watch must return &unit");
  const validatedEnum = new runtimeA.CalcitEnumDef(
    new runtimeA.CalcitStructValue(todoName, [todoField], [new runtimeA.CalcitSliceList([todoType])])
  );
  const validatedEnumValue = new runtimeA.CalcitEnumValue(todoField, ["ready"], validatedEnum);
  assert.equal(runtimeA._$n_enum_$o_validate(validatedEnumValue, todoField), undefined, "enum validation must return &unit");
  assert.equal(runtimeA.timeout_call(0, () => {}), undefined, "timeout-call must return &unit");

  const anonymousEnumCode = runtimeA.format_cirru_edn(anonymousEnumValue);
  const parsedAnonymousEnum = runtimeA.parse_cirru_edn(anonymousEnumCode, null);
  assert.equal(parsedAnonymousEnum.tag, todoField, "anonymous enum tags should stay interned after a Cirru EDN round-trip");
  assert.equal(
    parsedAnonymousEnum.tag === todoField ? "matched-draft" : "unmatched",
    "matched-draft",
    "a parsed enum should enter the same identity-based branch as a compiled match"
  );

  const enumOptions = new runtimeA.CalcitSliceMap([todoName, todoEnum]);
  const namedEnumCode = runtimeA.format_cirru_edn(todoEnumValue);
  const parsedNamedEnum = runtimeA.parse_cirru_edn(namedEnumCode, enumOptions);
  assert.equal(parsedNamedEnum.tag, todoField, "named enum tags should stay interned after a Cirru EDN round-trip");
  assert.equal(parsedNamedEnum.enumPrototype, todoEnum, "named enum round-trips should restore the provided prototype");
  assert.throws(
    () => runtimeA._$n_struct_$o_get(todoRecord, runtimeA.newTag("missing")),
    /does not define field :missing/,
    "struct field lookup must reject a missing field instead of returning nil"
  );

  const lateField = runtimeA.newTag("zz-layout-field");
  const earlyField = runtimeA.newTag("aa-layout-field");
  assert.ok(lateField.idx < earlyField.idx, "fixture must register fields in reverse lexical order");
  assert.ok(
    runtimeA.compareTagNames(runtimeA.newTag("\ue000"), runtimeA.newTag("𐀀")) < 0,
    "field ordering must match Rust Unicode scalar ordering rather than UTF-16 code-unit ordering"
  );
  const layoutDef = runtimeA.defstruct(
    runtimeA.newTag("LayoutProbe"),
    new runtimeA.CalcitSliceList([lateField, todoType]),
    new runtimeA.CalcitSliceList([earlyField, todoType])
  );
  assert.deepEqual(
    layoutDef.fields.map((field) => field.value),
    ["aa-layout-field", "zz-layout-field"],
    "Struct field layout must be lexical rather than tag-registration order"
  );
  const legacyLayoutValue = new runtimeA.CalcitStructValue(
    layoutDef.name,
    [lateField, earlyField],
    ["late", "early"],
    { name: layoutDef.name, fields: [lateField, earlyField], fieldTypes: [todoType, todoType], impls: [] }
  );
  assert.deepEqual(legacyLayoutValue.fields, [earlyField, lateField], "legacy Struct metadata should be canonicalized");
  assert.deepEqual(legacyLayoutValue.values, ["early", "late"], "canonicalization must preserve field/value alignment");
  const layoutValue = new runtimeA.CalcitStructValue(layoutDef.name, layoutDef.fields, ["early", "late"], layoutDef);
  assert.equal(layoutValue.nthAt(0, earlyField), "early", "indexed Struct reads should use the stable layout");
  assert.equal(layoutValue.assocAt(1, lateField, "updated").values[1], "updated");
  assert.deepEqual(layoutValue.withAt(0, earlyField, "a", 1, lateField, "z").values, ["a", "z"]);
  assert.throws(() => layoutValue.nthAt(0, lateField), /expects field :aa-layout-field/);
  assert.throws(() => layoutValue.assocAt(-1, earlyField, "bad"), /non-negative integer index/);
  assert.throws(() => layoutValue.nthAt(0.5, earlyField), /non-negative integer index/);
  assert.throws(() => layoutValue.assocAt(0, lateField, "bad"), /expects field :aa-layout-field/);
  assert.throws(() => layoutValue.withAt(0, lateField, "bad"), /expects field :aa-layout-field/);
  assert.throws(() => layoutValue.withAt(), /index\/tag\/value triples/);
  const parsedReverseLayout = runtimeA.parse_cirru_edn(
    "%{} 'LayoutProbe\n  :zz-layout-field |late\n  :aa-layout-field |early",
    null
  );
  assert.deepEqual(parsedReverseLayout.fields, [earlyField, lateField], "EDN Struct fields should be canonicalized");
  assert.deepEqual(parsedReverseLayout.values, ["early", "late"], "EDN Struct values must follow canonicalized fields");

  runtimeA.load_console_formatter_$x_();
  const formatter = globalThis.devtoolsFormatters.at(-1);
  const embeddedObjects = (node, found = []) => {
    if (Array.isArray(node)) {
      if (node[0] === "object" && node[1]?.object != null) found.push(node[1].object);
      for (const item of node) embeddedObjects(item, found);
    } else if (node != null && typeof node === "object") {
      for (const value of Object.values(node)) embeddedObjects(value, found);
    }
    return found;
  };
  const assertNominalNameIsSymbol = (value, kind) => {
    const name = embeddedObjects(formatter.header(value)).find((item) => item?.value === "TodoState");
    assert.ok(name instanceof runtimeA.CalcitSymbol, `${kind} formatter should render its name as a symbol`);
  };
  assertNominalNameIsSymbol(todoRecord, "struct value");
  assertNominalNameIsSymbol(todoStruct, "struct definition");
  assertNominalNameIsSymbol(todoEnum, "enum definition");
  assertNominalNameIsSymbol(todoEnumValue, "enum value");
  const anonymousEnumName = embeddedObjects(formatter.header(anonymousEnumValue)).find((item) => item?.value === "_");
  assert.ok(anonymousEnumName instanceof runtimeA.CalcitSymbol, "anonymous enum formatter should render `_` as a symbol");
  assert.ok(formatter.hasBody(todoStruct), "struct definition formatter should expose field types");
  assert.ok(formatter.hasBody(todoEnum), "enum definition formatter should expose variants");
  assert.ok(embeddedObjects(formatter.body(todoStruct)).includes(todoType), "struct definition formatter should embed field types");
  assert.ok(embeddedObjects(formatter.body(todoEnum)).includes(todoType), "enum definition formatter should embed variant payload types");
  const assertFormatterRendersNestedUnit = (value, kind) => {
    assert.ok(JSON.stringify(formatter.body(value)).includes("&unit"), `${kind} formatter should render nested &unit values`);
  };
  assertFormatterRendersNestedUnit(new runtimeA.CalcitSliceList([undefined]), "list");
  assertFormatterRendersNestedUnit(new runtimeA.CalcitSet([undefined]), "set");
  assertFormatterRendersNestedUnit(new runtimeA.CalcitSliceMap([todoField, undefined]), "map");
  assertFormatterRendersNestedUnit(new runtimeA.CalcitStructValue(todoName, [todoField], [undefined]), "struct");
  assert.ok(
    !JSON.stringify(formatter.body(new runtimeA.CalcitSliceList([null]))).includes("&unit"),
    "nested nil must remain distinct from &unit"
  );

  const foreignField = runtimeA.newTag("show");
  const method = () => "demo";
  const foreignImpl = new runtimeA.CalcitImpl(runtimeA.newTag("ForeignImpl"), [foreignField], [method], null);

  assert.ok(
    foreignImpl instanceof runtimeB.CalcitImpl,
    "CalcitImpl values from another runtime module instance should remain recognizable"
  );

  const brand = Symbol.for("@calcit/procs/CalcitImpl");
  const inheritedBrand = Object.assign(Object.create({ [brand]: true }), {
    name: runtimeA.newTag("InheritedImpl"),
    origin: null,
    fields: [],
    values: [],
    cachedHash: null,
  });
  const malformedBrand = {
    [brand]: true,
    name: runtimeA.newTag("MalformedImpl"),
    fields: [],
    values: [],
  };
  assert.ok(!(inheritedBrand instanceof runtimeB.CalcitImpl), "inherited impl brands must not be accepted");
  assert.ok(!(malformedBrand instanceof runtimeB.CalcitImpl), "branded objects without impl fields must not be accepted");

  const clonedImpl = runtimeB._$n_impl_$o__$o_new(runtimeB.newTag("ClonedImpl"), foreignImpl);
  assert.ok(clonedImpl instanceof runtimeB.CalcitImpl);
  assert.ok(clonedImpl.fields[0] instanceof runtimeB.CalcitTag);
  assert.equal(clonedImpl.fields[0].value, "show");
  assert.deepEqual(clonedImpl.values, [method]);

  console.log("JS runtime identity check passed");
} finally {
  await rm(fixtureRoot, { recursive: true, force: true });
}
