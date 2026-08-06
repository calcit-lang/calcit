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
  const todoRecord = new runtimeA.CalcitRecord(todoName, [todoField], [""]);
  const todoStruct = new runtimeA.CalcitStruct(todoName, [todoField], [todoType]);
  const todoEnum = new runtimeA.CalcitEnum(new runtimeA.CalcitRecord(todoName, [todoField], [todoType]));
  assert.equal(todoRecord.toString(), "(%{} 'TodoState (:draft |))");
  assert.equal(todoStruct.toString(), "(%struct 'TodoState (:draft 'String))");
  assert.equal(todoEnum.toString(), "(%enum 'TodoState)");

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
  assertNominalNameIsSymbol(todoRecord, "record");
  assertNominalNameIsSymbol(todoStruct, "struct");
  assertNominalNameIsSymbol(todoEnum, "enum");
  assert.ok(formatter.hasBody(todoStruct), "struct formatter should expose field types");
  assert.ok(formatter.hasBody(todoEnum), "enum formatter should expose variants");
  assert.ok(embeddedObjects(formatter.body(todoStruct)).includes(todoType), "struct formatter should embed field types");
  assert.ok(embeddedObjects(formatter.body(todoEnum)).includes(todoType), "enum formatter should embed variant payload types");

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
