import { performance } from "node:perf_hooks";
import { newTag } from "../lib/calcit.procs.mjs";

const variantCount = 16;
const iterations = 5_000_000;
const tags = Array.from({ length: variantCount }, (_, idx) => newTag(`enum-match-${idx.toString().padStart(2, "0")}`));
const target = tags.at(-1);

const ifBody = tags.map((tag, idx) => `${idx === 0 ? "if" : "else if"} (tag === tags[${idx}]) return ${idx};`).join("\n");
const switchBody = tags.map((tag, idx) => `case ${tag.idx}: return ${idx};`).join("\n");
const linearDispatch = new Function("tag", "tags", `${ifBody}\nreturn -1;`);
const switchDispatch = new Function("tag", `switch (tag.idx) {${switchBody}} return -1;`);

run("if-else-tag-chain", () => linearDispatch(target, tags));
run("integer-tag-switch", () => switchDispatch(target));

function run(label, dispatch) {
  let checksum = 0;
  for (let idx = 0; idx < 100_000; idx += 1) checksum += dispatch();
  const started = performance.now();
  for (let idx = 0; idx < iterations; idx += 1) checksum += dispatch();
  const elapsed = performance.now() - started;
  console.log(`${label}: ${elapsed.toFixed(2)}ms checksum=${checksum}`);
}
