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
