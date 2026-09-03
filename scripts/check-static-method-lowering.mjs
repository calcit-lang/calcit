import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

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
assert.match(
  typedOutput,
  /typed_get_receiver__\d+[\s\S]*?\$clt\._\$n_map_\$o_contains_\$q_\([\s\S]*?\$clt\._\$n_map_\$o_get\(/,
  "typed Map .get must lower to direct contains/get primitives",
);
assert.match(
  typedOutput,
  /typed_first_receiver__\d+[\s\S]*?\$clt\._\$n_list_\$o_empty_\$q_\([\s\S]*?\$clt\._\$n_list_\$o_first\(/,
  "typed List .first must lower to direct empty/first primitives",
);
assert.match(
  typedOutput,
  /typed_first_receiver__\d+[\s\S]*?\$clt\._\$n_str_\$o_empty_\$q_\([\s\S]*?\$clt\._\$n_str_\$o_first\(/,
  "typed String .first must lower to direct empty/first primitives",
);
assert.doesNotMatch(
  typedOutput,
  /invoke_method\(\s*"(?:get|nth|first)"/,
  "typed Option-returning access must not retain dynamic method dispatch",
);

const inferenceOutput = readFileSync("js-out/test-types-inference.main.mjs", "utf8");
assert.match(
  inferenceOutput,
  /typed_last_receiver__\d+[\s\S]*?\$clt\._\$n_list_\$o_empty_\$q_\([\s\S]*?\$clt\._\$n_list_\$o_last\(/,
  "typed List last must lower to direct empty/last primitives",
);
assert.match(
  inferenceOutput,
  /typed_last_receiver__\d+[\s\S]*?\$clt\._\$n_str_\$o_count\([\s\S]*?\$clt\._\$n_str_\$o_nth\(/,
  "typed String last must lower to direct count/nth primitives",
);
assert.doesNotMatch(inferenceOutput, /invoke_method\(\s*"last"/, "typed last must not retain dynamic method dispatch");

const coreOutput = readFileSync("js-out/calcit.core.mjs", "utf8");
assert.match(coreOutput, /ref:\s*_\$n_core_ref_impls/, "generated JS must register the Ref fallback impl table");

console.log("static receiver-method lowering checks passed");
