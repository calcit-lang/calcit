import { execSync } from "node:child_process";

const cases = [
  {
    name: "baseline-pass",
    command: "cargo run --bin cr -- calcit/test.cirru -1",
    expectedExit: 0,
    mustContain: [],
  },
  {
    name: "proc-type-warning-block",
    command: "cargo run --bin cr -- calcit/test-proc-type-warnings.cirru -1",
    expectedExit: 1,
    mustContain: ["Found 1 warnings, runner blocked"],
  },
  {
    name: "method-validation-error",
    command: "cargo run --bin cr -- calcit/test-method-validation.cirru -1",
    expectedExit: 1,
    mustContain: ["unknown method `.invalid-map-method`"],
  },
  {
    name: "ir-type-info-warning-block",
    command: "cargo run --bin cr -- calcit/test-ir-type-info.cirru -1",
    expectedExit: 1,
    mustContain: ["trying to call variable `inner` of non-function type number"],
  },
];

function runCase(item) {
  try {
    const output = execSync(item.command, { encoding: "utf8", stdio: "pipe" });
    if (item.expectedExit !== 0) {
      throw new Error(`expected exit ${item.expectedExit}, got 0`);
    }
    for (const token of item.mustContain) {
      if (!output.includes(token)) {
        throw new Error(`missing token: ${token}`);
      }
    }
    console.log(`[OK] ${item.name}`);
    return true;
  } catch (err) {
    const status = err?.status ?? 1;
    const output = `${err?.stdout ?? ""}${err?.stderr ?? ""}`;

    if (status !== item.expectedExit) {
      console.error(`[FAIL] ${item.name}: expected exit ${item.expectedExit}, got ${status}`);
      process.stderr.write(output);
      return false;
    }

    for (const token of item.mustContain) {
      if (!output.includes(token)) {
        console.error(`[FAIL] ${item.name}: missing token: ${token}`);
        process.stderr.write(output);
        return false;
      }
    }

    console.log(`[OK] ${item.name}`);
    return true;
  }
}

let passed = 0;
for (const item of cases) {
  if (runCase(item)) {
    passed += 1;
  }
}

if (passed !== cases.length) {
  console.error(`\nshift-left check failed: ${passed}/${cases.length} passed`);
  process.exit(1);
}

console.log(`\nshift-left check passed: ${passed}/${cases.length}`);
