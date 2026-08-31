# Calcit→Calx scalar baseline

## 中文

当前完整 schema v2 baseline 是 [`20260831-macos-arm64.json`](./20260831-macos-arm64.json)。采集环境为 Apple M1 Pro
8 logical CPUs、macOS arm64、Rust 1.97.1，代码 commit 为
`88bb5a2250ba65b0e35c4d1809e6d49a14c61623`，采集开始时 `gitDirty=false`。每个 case 丢弃 2 个
fresh-process 预热样本，保留 7 个原始样本；cached Calcit callable 与 reused Calx VM 都使用 20 次预热和
100 次测量。报告同时保存 debug/release，使用 median 与 median absolute deviation，共保留 182 个原始样本。

release 采样点的有限结果：

| Kernel | 采样规模 | 首个 Calx hot ≤ lookup native | 首个 Calx hot ≤ cached native | 首个 one-shot end-to-end ≤ lookup native |
| --- | --- | --- | --- | --- |
| range-sum | 10, 100, 1000 | 10 | 10 | 100 |
| fibonacci | 5, 10, 20 | 5 | 5 | 10 |
| affine | 10, 1000 | 10 | 10 | 未出现 |
| polynomial | 10, 1000 | 10 | 10 | 未出现 |
| bounded-simulation | 10, 100, 1000 | 10 | 10 | 1000 |

在 release 样本中，Calx compile total 的中位数约为 41–78 μs。规模增长的 range-sum、Fibonacci 和
bounded-simulation 能摊薄 frontend/compile/setup 成本；固定深度 affine/polynomial 在所测输入点没有
one-shot crossover。bounded-simulation 的采样 one-shot crossover 从旧基线的 100 移到 1000，说明阈值只能
解释为当前样本点，不能固定写进选择策略。

公平的 cached-callable 对比消除了 Calcit 每次 entry lookup 的不对称，但仍保留函数 scope、参数绑定和真实
runner execution。release 下 Calx hot / cached Calcit ratio 为 0.106–0.392，即在这些 scalar kernels 和
输入点上约快 2.6–9.5 倍。旧的 lookup-native ratio 为 0.009–0.150，确实夸大了微型 kernel 的差距；不过
cached 对比没有消除 Calx 的有限收益。证据支持下一步先用 profile 建立 compile/program cache issue；当前
compile 成本远大于 VM setup，尚不足以优先实现 VM pooling，也不证明任意 Calcit 代码适合 Calx。

本报告仍缺少 typed-buffer workload、平台 profiler 的 peak RSS/allocation hotspot、WASM 同 kernel 参照，
以及跨机器重复采样。因此 calx-vm #39 保持打开；不能根据这一个环境承诺固定倍数或自动 offload 阈值。

编译阶段 profile 见 [`20260831-compile-profile-macos-arm64.json`](./20260831-compile-profile-macos-arm64.json)。
它从干净提交 `ef04017f0bc60e2d8f6341599f00d426a11ed360` 采集五个 kernel 的分阶段时间与 allocator counters，
并在 bounded-simulation 上保留 Samply profile 哈希和 top stacks。program construction 占约 47–54%，
主要 CPU/分配路径为 `emit_expression`、`source_origin` 与 builder emission。这个证据把下一阶段收敛为
revision-safe validated-artifact cache；详细边界见
[compile cache design](../../docs/run/calx-compile-cache.md)。

---

## English

The current complete schema-v2 baseline is [`20260831-macos-arm64.json`](./20260831-macos-arm64.json). It was collected on an
Apple M1 Pro with 8 logical CPUs, macOS arm64, and Rust 1.97.1, at commit
`88bb5a2250ba65b0e35c4d1809e6d49a14c61623` with `gitDirty=false`. Each case discards two fresh-process warm-up
samples and preserves seven raw samples. Both the cached Calcit callable and reused Calx VM use 20 warm-ups and
100 measured calls. The report contains debug and release profiles, uses the median plus median absolute deviation,
and preserves 182 raw samples.

Bounded results at the sampled release points:

| Kernel | Sampled sizes | First Calx hot ≤ lookup native | First Calx hot ≤ cached native | First one-shot end-to-end ≤ lookup native |
| --- | --- | --- | --- | --- |
| range-sum | 10, 100, 1000 | 10 | 10 | 100 |
| fibonacci | 5, 10, 20 | 5 | 5 | 10 |
| affine | 10, 1000 | 10 | 10 | none |
| polynomial | 10, 1000 | 10 | 10 | none |
| bounded-simulation | 10, 100, 1000 | 10 | 10 | 1000 |

Median Calx compile total is approximately 41–78 μs in the release samples. The input-scaled range-sum,
Fibonacci, and bounded-simulation cases amortize frontend, compile, and setup costs; fixed-depth affine and
polynomial do not reach a one-shot crossover at their sampled points. The sampled bounded-simulation one-shot
crossover moves from 100 in the old baseline to 1000, demonstrating that a sampled threshold must not be frozen
into a selection policy.

The fair cached-callable comparison removes repeated Calcit entry lookup while retaining function-scope setup,
argument binding, and real runner execution. The release Calx-hot/cached-Calcit ratio is 0.106–0.392, or about
2.6–9.5 times faster for these scalar kernels and sampled inputs. The old lookup-native ratio of 0.009–0.150 did
overstate the tiny-kernel gap, but the cached comparison does not eliminate the bounded Calx gain. This evidence
supports filing a profile-backed compile/program-cache issue next. Compilation remains much larger than VM setup,
so it does not yet justify prioritizing VM pooling or claiming that arbitrary Calcit code belongs on Calx.

The report still lacks a typed-buffer workload, platform-profiler peak RSS/allocation hotspots, a same-kernel WASM
reference, and cross-machine repetition. calx-vm #39 therefore remains open; this one environment cannot justify a
fixed multiplier or an automatic offload threshold.

The compile-stage profile is recorded in
[`20260831-compile-profile-macos-arm64.json`](./20260831-compile-profile-macos-arm64.json). It captures staged timings
and allocator counters for all five kernels from clean commit `ef04017f0bc60e2d8f6341599f00d426a11ed360`, plus
the Samply profile hashes and selected stacks for bounded-simulation. Program construction accounts for roughly
47–54%, with `emit_expression`, `source_origin`, and builder emission dominating CPU and allocation paths. This
evidence narrows the next slice to a revision-safe validated-artifact cache; see the
[compile cache design](../../docs/run/calx-compile-cache.md) for the boundary.
