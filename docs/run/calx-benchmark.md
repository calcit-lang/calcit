# Calcit→Calx benchmark methodology

## 中文

这套基准用于回答一个有限问题：对已经通过 correctness corpus 的 typed scalar kernel，Calcit frontend、
Calx 编译、严格边界、VM 建立和执行全部计入后，哪些调用模式可能受益。它不把一个微基准推广为语言级
性能结论，也不设置会因机器噪声阻塞普通 CI 的绝对阈值。

### 当前范围

source-backed corpus 包含五个 kernel：

- `range-sum`：线性 tail recur 与 accumulation；
- `fibonacci`：direct recursion 与分支；
- `affine`：fixed-arity helper call graph；
- `polynomial`：固定深度数值表达式；
- `bounded-simulation`：随输入规模增长的数值状态迭代。

所有 case 都先执行同一 typed preprocessed source 的 Calcit/Calx 差分检查。当前报告明确标记
`scalar-only`。Calcit persistent List/Map/Set 不会冒充 typed buffer；在 Calcit/Calx ABI 出现真实同质
buffer 前，typed-buffer 结果保持 `not-measured-no-typed-buffer-abi`。WASM 是非阻塞参照，本阶段不制造
无法稳定复现的对比数字。

### 复现命令

完整矩阵同时构建 debug/release runner，默认每个 case 丢弃 2 个进程样本、保留 7 个样本，在复用 VM
中预热 20 次并测量 100 次：

```bash
yarn bench-calx-e2e
```

默认原始报告写到 `target/calx-bench/latest.json`。快速检查测量链路可用：

```bash
CALX_BENCH_QUICK=1 CALX_BENCH_SAMPLES=1 yarn bench-calx-e2e
```

以下环境变量可固定实验参数：

- `CALX_BENCH_SAMPLES`：保留的 fresh-process 样本数；
- `CALX_BENCH_PROCESS_WARMUP`：每个 case 丢弃的 fresh-process 样本数；
- `CALX_BENCH_VM_WARMUP`：hot call 前复用同一 VM 的预热次数；
- `CALX_BENCH_HOT_ITERATIONS`：每个进程内的 hot call 测量次数；
- `CALX_BENCH_OUTPUT`：相对仓库根目录或绝对 JSON 输出路径。

单 case runner 的 stdout 始终是一个 `calcit-calx-benchmark/1` JSON，可独立排错：

```bash
cargo run --release --bin calcit-calx-bench -- \
  --kernel range-sum --size 1000 --vm-warmup 20 --hot-iterations 100
```

### 阶段定义

- `fixtureInstallNs`：解析并安装固定 source fixture；
- `calcitFrontendNs`：Calcit preprocessing、symbol resolution 与 type-backed compilation；
- `snapshotCloneNs`：克隆已经 preprocess 的 compiled snapshot，不隐式编译无关 definition；
- `compile.eligibilityNs`：完整 reachable call-closure eligibility；
- `compile.planningNs`：从 typed expressions 建立 lowering plan；
- `compile.programConstructionNs`：声明 imports/functions、emission 与 `ProgramBuilder::build`；
- `compile.validationLoweringNs`：strict validation 与 Calx instruction lowering；
- `runtime.boundaryArgumentsNs` / `boundaryResultNs`：Calcit↔Calx scalar conversion；
- `runtime.vmSetupNs`：从 immutable validated program 建立 VM；
- `runtime.pureExecutionNs`：一次 `run_typed`，包含 strict runtime argument validation；
- `runtime.hotExecutionPerCallNs`：预先准备参数并复用 VM frames/stack 后的平均调用耗时；
- `runtime.nativeCallNs`：同一已 preprocess Calcit definition 的 native runner 调用；
- `processWallNs`：Node 从 spawn 到进程退出的总墙钟时间，包含 OS process startup 与所有阶段。

suite 报告保留每个原始样本，并为每个字段给出 median 与 median absolute deviation。crossover 只表示
采样输入点中首次出现 ratio ≤ 1 的位置，不做曲线外推。`oneShotEndToEndRatio` 包含 frontend、snapshot、
compile、boundary、VM setup 和执行；`hotVsNativeRatio` 比较复用 VM 的 Calx 调用与已 preprocess 的 Calcit
调用。两者回答不同部署场景，不能互相替代。

`program` 同时记录 function/import/syntax/instruction 数、diagnostic report 字节数、host-boundary 次数和
VM reuse 状态。峰值内存与 allocation hotspot 需要平台 profiler；不能从墙钟时间猜测，采集时应把
Instruments、heaptrack 或等价 profile 与同一 JSON、git commit、工具链和硬件信息一起发布。

---

## English

This benchmark answers one bounded question: for typed scalar kernels that already pass the correctness corpus,
which call patterns may benefit after Calcit frontend work, Calx compilation, strict boundaries, VM setup, and
execution are all included? It does not generalize one microbenchmark into a language-wide performance claim, and
it defines no noise-sensitive absolute threshold for ordinary CI.

### Current scope

The source-backed corpus contains five kernels:

- `range-sum`: linear tail recursion and accumulation;
- `fibonacci`: direct recursion and branching;
- `affine`: a fixed-arity helper call graph;
- `polynomial`: a fixed-depth numeric expression;
- `bounded-simulation`: numeric state iteration that grows with input size.

Every case first checks Calcit/Calx differential correctness from the same typed preprocessed source. Reports are
explicitly marked `scalar-only`. Calcit persistent List/Map/Set values are never presented as typed buffers; until a
real homogeneous buffer exists in the Calcit/Calx ABI, typed-buffer status remains
`not-measured-no-typed-buffer-abi`. WASM is a non-blocking reference and this stage does not manufacture an unstable
comparison number.

### Reproduction

The full matrix builds both debug and release runners. By default it discards two fresh-process samples, retains
seven, warms a reused VM for 20 calls, and measures 100 hot calls per process:

```bash
yarn bench-calx-e2e
```

Raw output defaults to `target/calx-bench/latest.json`. Use this command for a fast harness smoke check:

```bash
CALX_BENCH_QUICK=1 CALX_BENCH_SAMPLES=1 yarn bench-calx-e2e
```

The experiment can be pinned with `CALX_BENCH_SAMPLES`, `CALX_BENCH_PROCESS_WARMUP`,
`CALX_BENCH_VM_WARMUP`, `CALX_BENCH_HOT_ITERATIONS`, and `CALX_BENCH_OUTPUT`.

The single-case runner always emits one `calcit-calx-benchmark/1` JSON value on stdout:

```bash
cargo run --release --bin calcit-calx-bench -- \
  --kernel range-sum --size 1000 --vm-warmup 20 --hot-iterations 100
```

### Phase definitions

- `fixtureInstallNs`: parse and install the fixed source fixture;
- `calcitFrontendNs`: Calcit preprocessing, symbol resolution, and type-backed compilation;
- `snapshotCloneNs`: clone only already-preprocessed compiled definitions;
- `compile.eligibilityNs`: complete reachable-call-closure eligibility;
- `compile.planningNs`: build the lowering plan from typed expressions;
- `compile.programConstructionNs`: declare and emit imports/functions, then run `ProgramBuilder::build`;
- `compile.validationLoweringNs`: strict validation and Calx instruction lowering;
- `runtime.boundaryArgumentsNs` / `boundaryResultNs`: Calcit↔Calx scalar conversion;
- `runtime.vmSetupNs`: instantiate a VM from the immutable validated program;
- `runtime.pureExecutionNs`: one `run_typed`, including strict runtime argument validation;
- `runtime.hotExecutionPerCallNs`: average call time with prebuilt arguments and reused VM frames/stack;
- `runtime.nativeCallNs`: invoke the same already-preprocessed definition in the native Calcit runner;
- `processWallNs`: Node wall time from spawn through process exit, including OS startup and every measured phase.

The suite preserves every raw sample and reports the median plus median absolute deviation for every field. A
crossover is only the first sampled input whose ratio is at most one; it is not an extrapolation. The
`oneShotEndToEndRatio` includes frontend, snapshot, compilation, boundaries, VM setup, and execution, while
`hotVsNativeRatio` compares a reused Calx VM call with an already-preprocessed Calcit call. They model different
deployment scenarios and are not interchangeable.

The `program` section records function/import/syntax/instruction counts, diagnostic-report bytes, host-boundary
calls, and VM reuse. Peak memory and allocation hotspots require a platform profiler; they must not be inferred
from wall time. Publish Instruments, heaptrack, or equivalent profiles alongside the same JSON, git commit,
toolchain, and hardware metadata.
