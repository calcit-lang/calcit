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

const coreOutput = readFileSync("js-out/calcit.core.mjs", "utf8");
assert.match(coreOutput, /ref:\s*_\$n_core_ref_impls/, "generated JS must register the Ref fallback impl table");

console.log("static receiver-method lowering checks passed");
