import assert from "node:assert/strict";
import test from "node:test";
import { integerFromEnvironment } from "./bench-calx-settings.mjs";

test("uses the fallback only when the setting is absent", () => {
  assert.equal(integerFromEnvironment({}, "SAMPLES", 7, 1), 7);
});

test("accepts complete safe base-10 integer strings", () => {
  assert.equal(integerFromEnvironment({ SAMPLES: "12" }, "SAMPLES", 7, 1), 12);
  assert.equal(integerFromEnvironment({ SAMPLES: "+12" }, "SAMPLES", 7, 1), 12);
  assert.equal(integerFromEnvironment({ WARMUP: "0" }, "WARMUP", 2, 0), 0);
});

test("rejects partial, fractional, exponential, and padded values", () => {
  for (const raw of ["7junk", "1.5", "1e2", " 7", "7 ", ""]) {
    assert.throws(
      () => integerFromEnvironment({ SAMPLES: raw }, "SAMPLES", 7, 1),
      /SAMPLES must be an integer greater than or equal to 1/u,
    );
  }
});

test("rejects unsafe and below-minimum integers", () => {
  for (const raw of ["-1", "9007199254740992"]) {
    assert.throws(
      () => integerFromEnvironment({ WARMUP: raw }, "WARMUP", 2, 0),
      /WARMUP must be an integer greater than or equal to 0/u,
    );
  }
});
