# Calx compile profile runner / Calx 编译 profile runner

## 中文

- 为 `calcit-calx-bench` 增加显式 compile profile 模式，在一次 source fixture 安装和 preprocessing 后重复完整未缓存的 eligibility、planning、program construction 与 validation/lowering 链路。
- 分离三类测量：无阶段计时插桩的 profiler 负载、聚合的分阶段墙钟时间、仅在指定窗口启用的单线程 allocator 计数。
- allocator 报告 allocation/reallocation/deallocation 次数、请求字节数与显式释放字节数；它只用于定位分配压力，不把累计请求字节误称为峰值驻留内存。
- 默认 benchmark 模式与 `calcit-calx-benchmark/2` schema 保持不变；profile 模式使用独立的 `calcit-calx-compile-profile/1` schema，并继续保证 stdout 只有一个 JSON。
- 增加小规模回归测试，验证完整编译重复执行、stage timing 与 allocation 统计均可用。

## English

- Added an explicit compile-profile mode to `calcit-calx-bench`. It installs and preprocesses the source fixture once, then repeats the complete uncached eligibility, planning, program-construction, and validation/lowering pipeline.
- Separated three measurements: a profiler workload without per-stage timing instrumentation, aggregate per-stage wall time, and single-threaded allocator counters enabled only for a bounded window.
- The allocator report records allocation, reallocation, and deallocation calls, requested bytes, and explicitly deallocated bytes. It locates allocation pressure and does not mislabel cumulative requested bytes as peak resident memory.
- Kept the default benchmark mode and `calcit-calx-benchmark/2` schema unchanged. Profile mode uses the separate `calcit-calx-compile-profile/1` schema and still emits exactly one JSON value on stdout.
- Added a bounded regression test that exercises complete repeated compilation and verifies stage-timing and allocation evidence.
