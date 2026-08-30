# Calcit→Calx scalar baseline

## 中文

首份完整 baseline 是 [`20260831-macos-arm64.json`](./20260831-macos-arm64.json)。采集环境为 Apple M1 Pro
8 logical CPUs、macOS arm64、Rust 1.97.1，代码 commit 为
`6863811956d23b6834fb9e336f0cc25a4844f607`，采集开始时 `gitDirty=false`。每个 case 丢弃 2 个 fresh-process
预热样本，保留 7 个原始样本；hot call 使用 20 次 VM 预热和 100 次测量。报告同时保存 debug/release，
使用 median 与 median absolute deviation。

release 采样点的有限结果：

| Kernel | 采样规模 | 首个 hot ≤ native | 首个 one-shot end-to-end ≤ native |
| --- | --- | --- | --- |
| range-sum | 10, 100, 1000 | 10 | 100 |
| fibonacci | 5, 10, 20 | 5 | 10 |
| affine | 10, 1000 | 10 | 未出现 |
| polynomial | 10, 1000 | 10 | 未出现 |
| bounded-simulation | 10, 100, 1000 | 10 | 100 |

在 release 样本中，Calx compile total 的中位数约为 42–68 μs。规模增长的 range-sum、Fibonacci 和
bounded-simulation 能摊薄 frontend/compile/setup 成本；固定深度 affine/polynomial 在所测输入点没有
one-shot crossover。这支持下一步研究 compile cache 与 VM reuse，但不证明所有 Calcit 代码适合 Calx。

hot 对比尤其需要谨慎：Calx 侧复用已经实例化的 VM，而当前 Calcit baseline 每次通过
`run_program_with_docs` 完成入口查找与调用。它衡量的是 embedding 可见的重复调用路径，不是纯粹的 VM opcode
dispatch 对 runner opcode dispatch。后续选择策略若依赖该数据，必须先增加等价的 cached Calcit callable
baseline。

本报告仍缺少 typed-buffer workload、平台 profiler 的 peak RSS/allocation hotspot、WASM 同 kernel 参照，
以及跨机器重复采样。因此 calx-vm #39 保持打开；不能根据这一个环境承诺固定倍数或自动 offload 阈值。

---

## English

The first complete baseline is [`20260831-macos-arm64.json`](./20260831-macos-arm64.json). It was collected on an
Apple M1 Pro with 8 logical CPUs, macOS arm64, and Rust 1.97.1, at commit
`6863811956d23b6834fb9e336f0cc25a4844f607` with `gitDirty=false`. Each case discards two fresh-process warm-up
samples and preserves seven raw samples. Hot calls use 20 VM warm-ups and 100 measured calls. The report contains
both debug and release profiles and uses the median plus median absolute deviation.

Bounded results at the sampled release points:

| Kernel | Sampled sizes | First hot ≤ native | First one-shot end-to-end ≤ native |
| --- | --- | --- | --- |
| range-sum | 10, 100, 1000 | 10 | 100 |
| fibonacci | 5, 10, 20 | 5 | 10 |
| affine | 10, 1000 | 10 | none |
| polynomial | 10, 1000 | 10 | none |
| bounded-simulation | 10, 100, 1000 | 10 | 100 |

Median Calx compile total is approximately 42–68 μs in the release samples. The input-scaled range-sum,
Fibonacci, and bounded-simulation cases amortize frontend, compile, and setup costs; fixed-depth affine and
polynomial do not reach a one-shot crossover at their sampled points. This supports investigating compile caching
and VM reuse next, but it does not show that arbitrary Calcit code belongs on Calx.

The hot comparison needs particular care. Calx reuses an instantiated VM, while the current Calcit baseline calls
through `run_program_with_docs`, including entry lookup, every time. This measures embedding-visible repeated-call
paths, not an isolated comparison of VM opcode dispatch with runner opcode dispatch. Any selection policy based on
this evidence first needs an equivalent cached Calcit callable baseline.

The report still lacks a typed-buffer workload, platform-profiler peak RSS/allocation hotspots, a same-kernel WASM
reference, and cross-machine repetition. calx-vm #39 therefore remains open; this one environment cannot justify a
fixed multiplier or an automatic offload threshold.
