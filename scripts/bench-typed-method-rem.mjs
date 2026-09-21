import { performance } from "node:perf_hooks";
import {
  loop_rem_direct,
  loop_rem_typed,
  rem_direct,
  rem_typed,
  test_rem_methods_$x_,
} from "../target/typed-method-rem-js/app.main.mjs";

const iterations = 500_000;

test_rem_methods_$x_();
const returnExpression = (fn) => fn.toString().match(/\breturn ([^\n]+)\n}/)?.[1];
const loweredMethod = returnExpression(rem_typed);
if (loweredMethod == null || loweredMethod !== returnExpression(rem_direct) || /invoke.method/i.test(loweredMethod)) {
  throw new Error(`typed .rem was not lowered to the same direct call as &number:rem: ${loweredMethod}`);
}
const typedResult = loop_rem_typed(iterations, 0);
const directResult = loop_rem_direct(iterations, 0);
if (typedResult !== directResult) {
  throw new Error(`typed method and direct proc results differ: ${typedResult}, ${directResult}`);
}

run("typed-method", () => loop_rem_typed(iterations, 0));
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
