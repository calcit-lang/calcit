---
title: "Standalone Calx benchmark harness"
summary: "Locate the independent Calx benchmark runner, methodology, reports, and the narrow adapter retained in Calcit core."
scope: "core"
kind: "reference"
category: "run"
aliases:
  - "Calx benchmark"
  - "calcit-calx-bench"
  - "benchmark methodology"
id: core/run/calx-benchmark
related:
  - core/run/calx-target
---

# Standalone Calx benchmark harness / 独立 Calx benchmark 工具

## 中文

机器相关的 Calx runner、采样策略、raw report archive 与跨机器比较现由
[`calcit-lang/calcit-calx-bench`](https://github.com/calcit-lang/calcit-calx-bench) 独立维护。
运行命令、schema、warm-up/sample 规则、provenance 和当前报告见该仓库的
[README](https://github.com/calcit-lang/calcit-calx-bench#readme) 与
[`docs/methodology.md`](https://github.com/calcit-lang/calcit-calx-bench/blob/main/docs/methodology.md)。

Calcit core 只保留：

- Calx eligibility、lowering、runtime、trap/fallback 与 differential correctness；
- revision-safe artifact cache 及其稳定 correctness tests；
- `calcit::codegen::calx::benchmark_session` 窄 adapter。standalone runner 必须固定精确 Calcit revision，
  校验 adapter edition，并且不能绕过它访问 compiler mutable internals。

性能报告不是 runtime correctness gate，也不能直接转化为自动 offload policy。任何结论必须携带
Calcit commit/dirty state、Calx VM version、workload hash、host/toolchain 和完整样本。

## English

Machine-dependent runners, sampling policy, raw reports, and cross-machine comparison now belong to
[`calcit-lang/calcit-calx-bench`](https://github.com/calcit-lang/calcit-calx-bench). Its
[README](https://github.com/calcit-lang/calcit-calx-bench#readme) and
[`docs/methodology.md`](https://github.com/calcit-lang/calcit-calx-bench/blob/main/docs/methodology.md)
own commands, schemas, warm-up/sample rules, provenance, and current reports.

Calcit core retains Calx eligibility, lowering, runtime, trap/fallback and differential correctness,
the revision-safe artifact cache, and the narrow `calcit::codegen::calx::benchmark_session` adapter.
The standalone runner pins an exact Calcit revision, validates the adapter edition, and must not bypass
the adapter to reach mutable compiler internals. Performance reports are not runtime correctness gates or
automatic offload policy; conclusions require complete source, workload, host, and toolchain provenance.
