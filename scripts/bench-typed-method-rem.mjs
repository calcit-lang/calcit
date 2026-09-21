import { performance } from "node:perf_hooks";
import {
  loop_rem_direct,
  test_rem_methods_$x_,
} from "../target/typed-method-rem-js/app.main.mjs";

const iterations = 500_000;

test_rem_methods_$x_();
run("strict-number-rem", () => loop_rem_direct(iterations, 0));

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
