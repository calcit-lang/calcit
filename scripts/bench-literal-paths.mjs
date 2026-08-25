import { performance } from "node:perf_hooks";
import { newTag, _$n__$M_ as map } from "../target/literal-path-js/calcit.core.mjs";
import {
  loop_read,
  loop_write,
  read_dynamic,
  read_typed,
  write_dynamic,
  write_typed,
} from "../target/literal-path-js/bench-literal-paths.main.mjs";

const iterations = 100_000;
const a = newTag("a");
const b = newTag("b");
const data = map(a, map(b, 2));

run("typed-read", () => loop_read(read_typed, iterations, data, 0));
run("dynamic-read", () => loop_read(read_dynamic, iterations, data, 0));
run("typed-write", () => loop_write(write_typed, iterations, data));
run("dynamic-write", () => loop_write(write_dynamic, iterations, data));

function run(label, task) {
  task();
  const samples = [];
  let result;
  for (let idx = 0; idx < 5; idx += 1) {
    const started = performance.now();
    result = task();
    samples.push(performance.now() - started);
  }
  samples.sort((a, b) => a - b);
  console.log(`${label}: median=${samples[2].toFixed(2)}ms result=${String(result)}`);
}
