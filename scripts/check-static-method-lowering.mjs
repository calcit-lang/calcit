import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

function generatedReceiverBlocks(source, prefix) {
  const pattern = new RegExp(`let ${prefix}__\\d+ = [\\s\\S]*?\\n\\s*\\}\\)\\(\\);`, "g");
  return source.match(pattern) ?? [];
}

function exportedFunctionBlock(source, name) {
  const marker = `export function ${name}(`;
  const start = source.indexOf(marker);
  assert.notEqual(start, -1, `generated JavaScript must contain ${name}`);
  const next = source.indexOf("\nexport function ", start + marker.length);
  return source.slice(start, next === -1 ? source.length : next);
}

const mapOutput = readFileSync("js-out/test-map.main.mjs", "utf8");
assert.match(mapOutput, /\$clt\._\$n_map_\$o_keys\(dict\)/, "typed Map .keys must lower to &map:keys");
assert.doesNotMatch(
  mapOutput,
  /invoke_method\(\s*"keys"\s*,\s*dict\b/,
  "typed Map .keys must not retain dynamic dispatch",
);

const appOutput = readFileSync("js-out/app.main.mjs", "utf8");
assert.match(appOutput, /\$clt\._\$n_atom_\$o_deref\([^)]*\)/, "typed Ref .deref must lower to &atom:deref");

const structOutput = readFileSync("js-out/test-struct.main.mjs", "utf8");
assert.match(
  structOutput,
  /export function _\$n_lagopus0_\$o_show\(self\)/,
  "an anonymous nominal impl method must receive a stable &lagopus0:show callable",
);
assert.match(
  structOutput,
  /_\$n_lagopus0_\$o_rename\(l1t, "LagopusB"\)/,
  "typed source .rename must call the generated nominal callable directly",
);
assert.doesNotMatch(
  structOutput,
  /invoke_method\(\s*"show"\s*,\s*l1t\b/,
  "a statically known nominal .show call must not retain dynamic dispatch",
);

const typedOutput = readFileSync("js-out/test-types.main.mjs", "utf8");
const typedGetBlocks = generatedReceiverBlocks(typedOutput, "typed_get_receiver");
assert.ok(
  typedGetBlocks.some(
    (block) => /\$clt\._\$n_map_\$o_contains_\$q_\(/.test(block) && /\$clt\._\$n_map_\$o_get\(/.test(block),
  ),
  "typed Map .get must lower to direct contains/get primitives",
);
const typedFirstBlocks = generatedReceiverBlocks(typedOutput, "typed_first_receiver");
assert.ok(
  typedFirstBlocks.some(
    (block) => /\$clt\._\$n_list_\$o_empty_\$q_\(/.test(block) && /\$clt\._\$n_list_\$o_first\(/.test(block),
  ),
  "typed List .first must lower to direct empty/first primitives",
);
assert.ok(
  typedFirstBlocks.some(
    (block) => /\$clt\._\$n_str_\$o_empty_\$q_\(/.test(block) && /\$clt\._\$n_str_\$o_first\(/.test(block),
  ),
  "typed String .first must lower to direct empty/first primitives",
);
assert.doesNotMatch(
  typedOutput,
  /invoke_method\(\s*"(?:get|nth|first)"/,
  "typed Option-returning access must not retain dynamic method dispatch",
);

const inferenceOutput = readFileSync("js-out/test-types-inference.main.mjs", "utf8");
const inferredLastBlocks = generatedReceiverBlocks(inferenceOutput, "typed_last_receiver");
assert.ok(
  inferredLastBlocks.some(
    (block) => /\$clt\._\$n_list_\$o_empty_\$q_\(/.test(block) && /\$clt\._\$n_list_\$o_last\(/.test(block),
  ),
  "typed List last must lower to direct empty/last primitives",
);
for (const block of inferredLastBlocks) {
  assert.doesNotMatch(block, /invoke_method\(\s*"last"/, "a typed List last block must not retain dynamic dispatch");
}

const stringOutput = readFileSync("js-out/test-string.main.mjs", "utf8");
const stringLastBlocks = generatedReceiverBlocks(stringOutput, "typed_last_receiver");
assert.ok(
  stringLastBlocks.some(
    (block) => /\$clt\._\$n_str_\$o_count\(/.test(block) && /\$clt\._\$n_str_\$o_nth\(/.test(block),
  ),
  "typed String last must lower to direct count/nth primitives",
);
for (const block of stringLastBlocks) {
  assert.doesNotMatch(block, /invoke_method\(\s*"last"/, "a typed String last block must not retain dynamic dispatch");
}

const dynamicLastBlock = exportedFunctionBlock(inferenceOutput, "dynamic_last_compat");
assert.match(
  dynamicLastBlock,
  /invoke_method\(\s*"last"\s*,\s*value\b/,
  "an explicitly Dynamic last receiver must retain the compatibility dispatch path",
);
assert.doesNotMatch(dynamicLastBlock, /typed_last_receiver__/, "Dynamic last must not be guessed into a typed lowering");

const coreOutput = readFileSync("js-out/calcit.core.mjs", "utf8");
assert.match(coreOutput, /ref:\s*_\$n_core_ref_impls/, "generated JS must register the Ref fallback impl table");

console.log("static receiver-method lowering checks passed");
