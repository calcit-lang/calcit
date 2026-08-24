import { performance } from "node:perf_hooks";
import { CalcitStructDef, CalcitStructValue, newTag } from "../lib/calcit.procs.mjs";

const fieldCount = 32;
const iterations = 5_000_000;
const fields = Array.from({ length: fieldCount }, (_, idx) => newTag(`field-${idx.toString().padStart(2, "0")}`));
const values = Array.from({ length: fieldCount }, (_, idx) => idx + 1);
const definition = new CalcitStructDef(newTag("StructIndexBenchmark"), fields, new Array(fieldCount).fill(null));
const value = new CalcitStructValue(definition.name, fields, values, definition);
const targetIndex = 27;
const targetField = fields[targetIndex];

const run = (label, read) => {
  let checksum = 0;
  for (let idx = 0; idx < 100_000; idx++) checksum += read();
  const started = performance.now();
  for (let idx = 0; idx < iterations; idx++) checksum += read();
  const elapsed = performance.now() - started;
  console.log(`${label}: ${elapsed.toFixed(2)}ms checksum=${checksum}`);
};

run("tag-lookup", () => value.getRequired(targetField));
run("indexed-with-check", () => value.nthAt(targetIndex, targetField));
