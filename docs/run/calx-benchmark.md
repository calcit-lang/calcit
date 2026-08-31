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

完整矩阵同时构建 debug/release runner，默认每个 case 丢弃 2 个进程样本、保留 7 个样本，并分别对缓存的
Calcit callable 与复用的 Calx VM 预热 20 次、测量 100 次：

```bash
yarn bench-calx-e2e
```

默认原始报告写到 `target/calx-bench/latest.json`。快速检查测量链路可用：

```bash
CALX_BENCH_QUICK=1 CALX_BENCH_SAMPLES=1 yarn bench-calx-e2e
```

仓库内首份完整、干净工作区 baseline 及其有限结论见
[`benchmarks/calx/README.md`](../../benchmarks/calx/README.md)。

以下环境变量可固定实验参数：

- `CALX_BENCH_SAMPLES`：保留的 fresh-process 样本数；
- `CALX_BENCH_PROCESS_WARMUP`：每个 case 丢弃的 fresh-process 样本数；
- `CALX_BENCH_VM_WARMUP`：cached Calcit callable 与复用的 Calx VM 在 hot call 前各自的预热次数；
- `CALX_BENCH_HOT_ITERATIONS`：每个进程内的 hot call 测量次数；
- `CALX_BENCH_OUTPUT`：相对仓库根目录或绝对 JSON 输出路径。

单 case runner 成功时，stdout 恰好输出一个 `calcit-calx-benchmark/2` JSON；失败写入 stderr 并以非零状态退出：

```bash
cargo run --release --bin calcit-calx-bench -- \
  --kernel range-sum --size 1000 --vm-warmup 20 --hot-iterations 100
```

### Compile profile 模式

CPU/allocation profiler 不能从单次约几十微秒的编译可靠分辨热点。显式传入
`--compile-profile-iterations` 后，runner 只安装并 preprocess source fixture 一次，再重复完整、未缓存的
eligibility → planning → program construction → validation/lowering 链。stdout 仍恰好输出一个 JSON，
schema 为 `calcit-calx-compile-profile/1`；默认 benchmark 模式和 schema 不变。

profile report 的 `compilationSucceeded` 只表示全部 warmup、stage、allocation 与未插桩编译都成功完成；
profile mode 不执行 kernel，因此不会输出 `correctness: true`。语义正确性仍由默认 benchmark 与 core
differential corpus 验证。

```bash
CARGO_PROFILE_RELEASE_STRIP=none \
  CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
  cargo build --release --bin calcit-calx-bench

samply record --save-only --main-thread-only --unstable-presymbolicate \
  --output target/calx-profile/compile-cpu.json.gz \
  ./target/release/calcit-calx-bench \
  --kernel affine \
  --compile-profile-warmup 1000 \
  --compile-profile-stage-iterations 10000 \
  --compile-profile-allocation-iterations 10000 \
  --compile-profile-iterations 100000
```

release profile 默认移除 debug info；上面的临时 Cargo override 只为 profiler 保留 line tables，不改变正式发布配置。
`stageTimingPerIteration` 来自单独的阶段计时窗口，`allocationsPerIteration` 来自只在指定窗口开启的
单线程 allocator counters。`requestedBytes` 是累计请求量，不是 peak RSS；reallocation 的新大小计入请求量，
旧 block 不重复计入显式 deallocation。无插桩的 `compilePerIterationNs` 才是外部 CPU profiler 的主负载。

原始 Samply/Instruments artifact 写在 `target/` 而不提交；仓库只保存工具版本、commit、硬件、命令、
迭代数、阶段摘要和可复查的 top stacks。profile mode 仍然执行完整 compile pipeline，因此它用于定位
cache candidate，不代表 cache-hit 成本。

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
- `runtime.nativeCallNs`：经 `run_program_with_docs` 完成 guard、entry lookup 与执行的一次 embedding-visible 调用；
- `runtime.cachedNativeResolutionNs`：从已 preprocess program 解析并缓存 Calcit callable 的一次性成本；
- `runtime.cachedNativeExecutionPerCallNs`：预先准备参数并复用 callable 后的平均调用耗时，仍包含函数 scope、参数绑定与 runner execution；
- `processWallNs`：Node 从 spawn 到进程退出的总墙钟时间，包含 OS process startup 与所有阶段。

suite 报告保留每个原始样本，并为每个字段给出 median 与 median absolute deviation。crossover 只表示
采样输入点中首次出现 ratio ≤ 1 的位置，不做曲线外推。`calxOneShotEndToEndVsLookupNativeRatio` 包含
frontend、snapshot、compile、boundary、VM setup 和执行；`calxHotVsLookupNativeRatio` 保留 embedding-visible
对比；`calxHotVsCachedNativeRatio` 才比较已经解析的两条重复执行路径。三个指标回答不同部署场景，不能互相替代。

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
seven, then warms both a cached Calcit callable and a reused Calx VM for 20 calls and measures 100 hot calls per
process:

```bash
yarn bench-calx-e2e
```

Raw output defaults to `target/calx-bench/latest.json`. Use this command for a fast harness smoke check:

```bash
CALX_BENCH_QUICK=1 CALX_BENCH_SAMPLES=1 yarn bench-calx-e2e
```

The first complete clean-worktree baseline and its bounded conclusions are recorded in
[`benchmarks/calx/README.md`](../../benchmarks/calx/README.md).

The experiment can be pinned with `CALX_BENCH_SAMPLES`, `CALX_BENCH_PROCESS_WARMUP`,
`CALX_BENCH_VM_WARMUP` (applied separately to the cached Calcit callable and reused Calx VM),
`CALX_BENCH_HOT_ITERATIONS`, and `CALX_BENCH_OUTPUT`.

On success, the single-case runner emits exactly one `calcit-calx-benchmark/2` JSON value on stdout. Failures are
reported on stderr with a nonzero exit status:

```bash
cargo run --release --bin calcit-calx-bench -- \
  --kernel range-sum --size 1000 --vm-warmup 20 --hot-iterations 100
```

### Compile profile mode

A CPU or allocation profiler cannot reliably distinguish hotspots from a single compilation lasting only tens of
microseconds. Passing `--compile-profile-iterations` installs and preprocesses the source fixture once, then repeats
the complete uncached eligibility → planning → program construction → validation/lowering pipeline. Stdout still
contains exactly one JSON value, using `calcit-calx-compile-profile/1`; the default benchmark mode and schema are
unchanged.

The profile report's `compilationSucceeded` field means only that every warmup, staged, allocation, and
uninstrumented compilation completed successfully. Profile mode does not execute the kernel and therefore does not
emit `correctness: true`; semantic correctness remains covered by the default benchmark and the core differential
corpus.

```bash
CARGO_PROFILE_RELEASE_STRIP=none \
  CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
  cargo build --release --bin calcit-calx-bench

samply record --save-only --main-thread-only --unstable-presymbolicate \
  --output target/calx-profile/compile-cpu.json.gz \
  ./target/release/calcit-calx-bench \
  --kernel affine \
  --compile-profile-warmup 1000 \
  --compile-profile-stage-iterations 10000 \
  --compile-profile-allocation-iterations 10000 \
  --compile-profile-iterations 100000
```

The release profile strips debug information by default. The temporary Cargo overrides above retain line tables for
the profiler without changing the shipping configuration. `stageTimingPerIteration` comes from a separate staged
timing window, while `allocationsPerIteration` comes from single-threaded allocator counters enabled only for the
requested window. `requestedBytes` is cumulative request volume, not peak RSS. A reallocation adds its new requested
size, while the replaced block is not counted again as an explicit deallocation. The uninstrumented
`compilePerIterationNs` remains the workload observed by the external CPU profiler.

Raw Samply/Instruments artifacts stay under `target/`; the repository records tool versions, commit, hardware,
commands, iteration counts, stage summaries, and reviewable top stacks. Profile mode still executes the complete
compile pipeline, so it locates cache candidates but does not model cache-hit cost.

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
- `runtime.nativeCallNs`: one embedding-visible call through `run_program_with_docs`, including its guard, entry lookup, and execution;
- `runtime.cachedNativeResolutionNs`: one-time resolution of the cached Calcit callable from the already-preprocessed program;
- `runtime.cachedNativeExecutionPerCallNs`: average call time with prebuilt arguments and a reused callable, while still timing function scope setup, argument binding, and runner execution;
- `processWallNs`: Node wall time from spawn through process exit, including OS startup and every measured phase.

The suite preserves every raw sample and reports the median plus median absolute deviation for every field. A
crossover is only the first sampled input whose ratio is at most one; it is not an extrapolation.
`calxOneShotEndToEndVsLookupNativeRatio` includes frontend, snapshot, compilation, boundaries, VM setup, and
execution. `calxHotVsLookupNativeRatio` preserves the embedding-visible comparison, while
`calxHotVsCachedNativeRatio` compares the two already-resolved repeated-execution paths. The three metrics model
different deployment scenarios and are not interchangeable.

The `program` section records function/import/syntax/instruction counts, diagnostic-report bytes, host-boundary
calls, and VM reuse. Peak memory and allocation hotspots require a platform profiler; they must not be inferred
from wall time. Publish Instruments, heaptrack, or equivalent profiles alongside the same JSON, git commit,
toolchain, and hardware metadata.
