import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const quick = process.env.CALX_BENCH_QUICK === "1";
const samples = positiveInteger("CALX_BENCH_SAMPLES", quick ? 2 : 7);
const processWarmup = nonNegativeInteger("CALX_BENCH_PROCESS_WARMUP", quick ? 0 : 2);
const vmWarmup = nonNegativeInteger("CALX_BENCH_VM_WARMUP", quick ? 2 : 20);
const hotIterations = positiveInteger("CALX_BENCH_HOT_ITERATIONS", quick ? 5 : 100);
const outputPath = path.resolve(
  repoRoot,
  process.env.CALX_BENCH_OUTPUT ?? "target/calx-bench/latest.json",
);

const fullMatrix = [
  { kernel: "range-sum", sizes: [10, 100, 1000] },
  { kernel: "fibonacci", sizes: [5, 10, 20] },
  { kernel: "affine", sizes: [10, 1000] },
  { kernel: "polynomial", sizes: [10, 1000] },
  { kernel: "bounded-simulation", sizes: [10, 100, 1000] },
];
const matrix = quick
  ? fullMatrix.map(({ kernel, sizes }) => ({ kernel, sizes: [sizes[0]] }))
  : fullMatrix;

function integerFromEnvironment(name, fallback, minimum) {
  const raw = process.env[name];
  if (raw === undefined) return fallback;
  const parsed = Number.parseInt(raw, 10);
  if (!Number.isSafeInteger(parsed) || parsed < minimum) {
    throw new Error(`${name} must be an integer greater than or equal to ${minimum}`);
  }
  return parsed;
}

function positiveInteger(name, fallback) {
  return integerFromEnvironment(name, fallback, 1);
}

function nonNegativeInteger(name, fallback) {
  return integerFromEnvironment(name, fallback, 0);
}

function commandOutput(command, args) {
  return execFileSync(command, args, { cwd: repoRoot, encoding: "utf8" }).trim();
}

function build(profile) {
  const args = ["build", "--bin", "calcit-calx-bench"];
  if (profile === "release") args.push("--release");
  const result = spawnSync("cargo", args, { cwd: repoRoot, stdio: "inherit" });
  if (result.status !== 0) throw new Error(`failed to build ${profile} calcit-calx-bench`);
}

function runCase(binary, kernel, size) {
  const args = [
    "--kernel",
    kernel,
    "--size",
    String(size),
    "--vm-warmup",
    String(vmWarmup),
    "--hot-iterations",
    String(hotIterations),
  ];
  const started = process.hrtime.bigint();
  const result = spawnSync(binary, args, {
    cwd: repoRoot,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
  const processWallNs = Number(process.hrtime.bigint() - started);
  if (result.status !== 0) {
    throw new Error(`benchmark failed for ${kernel}/${size}\n${result.stdout}\n${result.stderr}`);
  }
  const report = JSON.parse(result.stdout);
  if (report.schema !== "calcit-calx-benchmark/1" || report.correctness !== true) {
    throw new Error(`invalid or unverified benchmark report for ${kernel}/${size}`);
  }
  return { processWallNs, report };
}

function median(values) {
  const ordered = [...values].sort((a, b) => a - b);
  const middle = Math.floor(ordered.length / 2);
  return ordered.length % 2 === 0
    ? Math.round((ordered[middle - 1] + ordered[middle]) / 2)
    : ordered[middle];
}

function medianAbsoluteDeviation(values) {
  const center = median(values);
  return median(values.map((value) => Math.abs(value - center)));
}

function selectMetrics(sample) {
  const { report } = sample;
  return {
    processWallNs: sample.processWallNs,
    fixtureInstallNs: report.fixtureInstallNs,
    calcitFrontendNs: report.calcitFrontendNs,
    snapshotCloneNs: report.snapshotCloneNs,
    eligibilityNs: report.compile.eligibilityNs,
    planningNs: report.compile.planningNs,
    programConstructionNs: report.compile.programConstructionNs,
    validationLoweringNs: report.compile.validationLoweringNs,
    calxCompileTotalNs: report.compile.totalNs,
    nativeCallNs: report.runtime.nativeCallNs,
    boundaryArgumentsNs: report.runtime.boundaryArgumentsNs,
    vmSetupNs: report.runtime.vmSetupNs,
    pureExecutionNs: report.runtime.pureExecutionNs,
    boundaryResultNs: report.runtime.boundaryResultNs,
    calxOneShotNs: report.runtime.calxOneShotNs,
    hotExecutionPerCallNs: report.runtime.hotExecutionPerCallNs,
  };
}

function aggregate(rawSamples) {
  const rows = rawSamples.map(selectMetrics);
  const names = Object.keys(rows[0]);
  const medians = Object.fromEntries(names.map((name) => [name, median(rows.map((row) => row[name]))]));
  const medianAbsoluteDeviations = Object.fromEntries(
    names.map((name) => [name, medianAbsoluteDeviation(rows.map((row) => row[name]))]),
  );
  const nativeEndToEndNs = medians.fixtureInstallNs + medians.calcitFrontendNs + medians.nativeCallNs;
  const calxEndToEndNs =
    medians.fixtureInstallNs +
    medians.calcitFrontendNs +
    medians.snapshotCloneNs +
    medians.calxCompileTotalNs +
    medians.calxOneShotNs;
  return {
    medians,
    medianAbsoluteDeviations,
    derived: {
      nativeEndToEndNs,
      calxEndToEndNs,
      hotVsNativeRatio: medians.hotExecutionPerCallNs / medians.nativeCallNs,
      oneShotEndToEndRatio: calxEndToEndNs / nativeEndToEndNs,
    },
  };
}

function crossover(cases, ratioName) {
  return cases.find((item) => item.aggregate.derived[ratioName] <= 1)?.size ?? null;
}

build("debug");
build("release");

const profiles = [];
for (const profile of ["debug", "release"]) {
  const binary = path.join(repoRoot, "target", profile, "calcit-calx-bench");
  const cases = [];
  for (const { kernel, sizes } of matrix) {
    for (const size of sizes) {
      for (let index = 0; index < processWarmup; index += 1) runCase(binary, kernel, size);
      const rawSamples = [];
      for (let index = 0; index < samples; index += 1) rawSamples.push(runCase(binary, kernel, size));
      cases.push({
        kernel,
        size,
        program: rawSamples[0].report.program,
        aggregate: aggregate(rawSamples),
        rawSamples,
      });
    }
  }
  const crossovers = matrix.map(({ kernel }) => {
    const kernelCases = cases.filter((item) => item.kernel === kernel).sort((a, b) => a.size - b.size);
    return {
      kernel,
      hotExecutionSize: crossover(kernelCases, "hotVsNativeRatio"),
      oneShotEndToEndSize: crossover(kernelCases, "oneShotEndToEndRatio"),
    };
  });
  profiles.push({ profile, cases, crossovers });
}

const cpu = os.cpus()[0];
const report = {
  schema: "calcit-calx-benchmark-suite/1",
  generatedAt: new Date().toISOString(),
  scope: {
    workload: "scalar-only",
    typedBufferStatus: "not-measured-no-typed-buffer-abi",
    wasmStatus: "not-measured-non-blocking-reference",
  },
  environment: {
    platform: os.platform(),
    release: os.release(),
    architecture: os.arch(),
    cpuModel: cpu?.model ?? "unknown",
    logicalCpuCount: os.cpus().length,
    totalMemoryBytes: os.totalmem(),
    rustc: commandOutput("rustc", ["-Vv"]),
    cargo: commandOutput("cargo", ["-V"]),
    node: process.version,
    gitCommit: commandOutput("git", ["rev-parse", "HEAD"]),
    gitDirty: commandOutput("git", ["status", "--porcelain"]).length > 0,
  },
  methodology: {
    processWarmup,
    samples,
    vmWarmup,
    hotIterations,
    noiseStatistic: "median-and-median-absolute-deviation",
    processWallMeaning: "Node spawn wall time including process startup and all measured phases",
    regressionPolicy: "informational-no-absolute-ci-threshold",
  },
  matrix,
  profiles,
};

mkdirSync(path.dirname(outputPath), { recursive: true });
writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(`Calx benchmark report: ${outputPath}`);
for (const { profile, crossovers } of profiles) {
  console.log(`${profile} sampled crossover points:`);
  for (const item of crossovers) {
    console.log(
      `  ${item.kernel}: hot=${item.hotExecutionSize ?? "none"}, one-shot=${item.oneShotEndToEndSize ?? "none"}`,
    );
  }
}
