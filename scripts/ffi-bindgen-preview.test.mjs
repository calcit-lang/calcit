import assert from "node:assert/strict";
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { generatePreview, writePreview } from "./ffi-bindgen-preview.mjs";

const fixtureRoot = path.join("tests", "fixtures", "ffi-bindgen-preview");

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

test("generates deterministic Rust, Calcit, TypeScript, and WIT previews", async () => {
  const document = await readJson(path.join(fixtureRoot, "md5-interface.json"));
  const first = await mkdtemp(path.join(tmpdir(), "calcit-ffi-bindgen-first-"));
  const second = await mkdtemp(path.join(tmpdir(), "calcit-ffi-bindgen-second-"));
  const firstManifest = await writePreview(document, first);
  const secondManifest = await writePreview(document, second);

  assert.deepEqual(firstManifest, secondManifest);
  for (const { path: relativePath } of firstManifest.generated) {
    const [firstContent, secondContent, expectedContent] = await Promise.all([
      readFile(path.join(first, relativePath), "utf8"),
      readFile(path.join(second, relativePath), "utf8"),
      readFile(path.join(fixtureRoot, "expected", relativePath), "utf8"),
    ]);
    assert.equal(firstContent, secondContent);
    assert.equal(firstContent, expectedContent);
  }
  assert.deepEqual(await readJson(path.join(first, "manifest.json")), await readJson(path.join(fixtureRoot, "expected", "manifest.json")));
});

test("refuses unsupported definitions instead of generating Dynamic fallbacks", async () => {
  const document = await readJson(path.join(fixtureRoot, "md5-interface.json"));
  document.definitions[0].status = "unsupported";
  document.definitions[0].signature = null;
  document.definitions[0].diagnostic_codes = ["E_FFI_IR_UNSUPPORTED_TYPE"];
  assert.throws(() => generatePreview(document), /E_FFI_IR_UNSUPPORTED_TYPE/u);
});

test("preview rejects a missing composite declaration", async () => {
  const document = await readJson(path.join(fixtureRoot, "md5-interface.json"));
  document.definitions[0].signature.result = {
    kind: "struct",
    id: "demo/Resource",
    arguments: [],
  };
  assert.throws(() => generatePreview(document), /missing Interface IR declaration demo\/Resource/u);
});

test("v1 consumers fail explicitly after the versioned declaration upgrade", async () => {
  const document = await readJson(path.join(fixtureRoot, "md5-interface.json"));
  document.version = 1;
  assert.throws(() => generatePreview(document), /expected an Interface IR v2 document/u);
});

test("WIT preview rejects Unit parameters instead of emitting an invalid null type", async () => {
  const document = await readJson(path.join(fixtureRoot, "md5-interface.json"));
  document.definitions[0].signature.parameters[0].type = { kind: "unit" };
  assert.throws(() => generatePreview(document), /parameter 0 uses Unit/u);
});

test("generates Struct, Enum, Option, and Result declarations from Interface IR v2", async () => {
  const document = await readJson(path.join(fixtureRoot, "composite-interface.json"));

  const generated = generatePreview(document);
  const rust = generated.get("demo-types.ffi.rs");
  const typescript = generated.get("demo-types.d.ts");
  const wit = generated.get("demo-types.wit");
  assert.match(rust, /pub struct DemoSchemaPerson/u);
  assert.match(rust, /pub enum DemoSchemaOutcome/u);
  assert.match(rust, /nickname: Option<String>/u);
  assert.match(typescript, /export interface DemoSchemaPerson/u);
  assert.match(typescript, /readonly tag: "ok"/u);
  assert.match(wit, /record demo-schema-person/u);
  assert.match(wit, /variant demo-schema-outcome/u);
  assert.match(wit, /roundtrip: func\(arg0: demo-schema-person\) -> result<demo-schema-outcome, string>/u);
});

test("WIT preview uses canonical Result forms when Unit omits payloads", async () => {
  const fixture = await readJson(path.join(fixtureRoot, "composite-interface.json"));

  const unitError = structuredClone(fixture);
  unitError.definitions[0].signature.result.error = { kind: "unit" };
  assert.match(generatePreview(unitError).get("demo-types.wit"), /-> result<demo-schema-outcome>;/u);

  const unitOk = structuredClone(fixture);
  unitOk.definitions[0].signature.result.ok = { kind: "unit" };
  assert.match(generatePreview(unitOk).get("demo-types.wit"), /-> result<_, string>;/u);

  const unitBoth = structuredClone(fixture);
  unitBoth.definitions[0].signature.result.ok = { kind: "unit" };
  unitBoth.definitions[0].signature.result.error = { kind: "unit" };
  assert.match(generatePreview(unitBoth).get("demo-types.wit"), /-> result;/u);
});

test("preview rejects generated definition identifiers that collide across namespaces", async () => {
  const document = await readJson(path.join(fixtureRoot, "md5-interface.json"));
  const duplicate = structuredClone(document.definitions[0]);
  duplicate.id = "demo.hash/md5";
  duplicate.namespace = "demo.hash";
  duplicate.lowering.symbol = "demo_md5";
  document.definitions.push(duplicate);

  assert.throws(
    () => generatePreview(document),
    /generated Rust\/TypeScript definition identifier "md5" collides between calcit\.std\.hash\/md5 and demo\.hash\/md5/u,
  );
});

test("preview rejects generated struct-field and enum-variant collisions", async () => {
  const fixture = await readJson(path.join(fixtureRoot, "composite-interface.json"));

  const structCollision = structuredClone(fixture);
  structCollision.declarations[0].fields.push(
    { name: "foo-bar", type: { kind: "string" } },
    { name: "foo_bar", type: { kind: "string" } },
  );
  assert.throws(
    () => generatePreview(structCollision),
    /generated Rust\/TypeScript field in demo\.schema\/Person identifier "foo_bar" collides/u,
  );

  const enumCollision = structuredClone(fixture);
  enumCollision.declarations[1].variants.push(
    { name: "retry-now", payload: [] },
    { name: "retry_now", payload: [] },
  );
  assert.throws(
    () => generatePreview(enumCollision),
    /generated Rust variant in demo\.schema\/Outcome identifier "RetryNow" collides/u,
  );
});
