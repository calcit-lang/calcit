import { performance } from "node:perf_hooks";
import {
  loop_rem_direct,
  loop_rem_dynamic,
  loop_rem_typed,
  test_rem_methods_$x_,
} from "../target/typed-method-rem-js/app.main.mjs";

const iterations = 500_000;

test_rem_methods_$x_();
const typedResult = loop_rem_typed(iterations, 0);
const dynamicResult = loop_rem_dynamic(iterations, 0);
const directResult = loop_rem_direct(iterations, 0);
if (typedResult !== directResult || dynamicResult !== directResult) {
  throw new Error(`typed/dynamic/direct remainder results differ: ${typedResult}, ${dynamicResult}, ${directResult}`);
}

run("typed-method", () => loop_rem_typed(iterations, 0));
run("dynamic-method", () => loop_rem_dynamic(iterations, 0));
run("direct-proc", () => loop_rem_direct(iterations, 0));

/** Warm a generated-JS benchmark, then print the median of five measured samples. */
function run(label, task) {
  task();
  const samples = [];
  let result;
  for (let index = 0; index < 5; index += 1) {
    const start = performance.now();
    result = task();
    samples.push(performance.now() - start);
  }
  samples.sort((a, b) => a - b);
  console.log(`${label}: median=${samples[2].toFixed(2)}ms result=${result}`);
}
