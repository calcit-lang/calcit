# 严格 F64Buffer Calcit→Calx kernel / Strict F64Buffer Calcit-to-Calx kernel

## 中文

`calx_vm 0.4.0` 发布后，Calcit 将稳定依赖升级到该版本，并在 runtime/type annotation 中加入独立的
immutable `F64Buffer`。该值明确不同于 byte `Buffer`、persistent `List`、Nil 和 Dynamic；它不能进入
JSON/data-shape serialization 路径。

使用 `F64Buffer` 的 strict adapter 采用 ABI `calcit-calx-kernel/2`（既有纯 scalar kernel 保持 `/1`）：
只接受 typed `F64Buffer`、Number、Bool 和 Unit，`&f64-buffer:len`、`&f64:to-i64-index`、
`&f64-buffer:get` 分别 lower 为 Calx 的三条已冻结 instruction。非法 index 与 bounds 由 VM trap；只有
eligibility 阶段可以整体 fallback，VM 开始后不自动重跑 Calcit。

source-backed dot-product fixture 使用同一 CalcIt source 做 native/Calx differential，并固定 generated
program golden；其负向路径拒绝 Nil、List、byte Buffer 和越界。机器相关的 allocation/ownership/copy
分段及 crossover 报告仍由 standalone `calcit-calx-bench` 维护，core 不重新引入 benchmark runner。

## English

After `calx_vm 0.4.0` was published, Calcit upgrades its stable dependency and adds a distinct immutable
`F64Buffer` runtime value and type annotation. It is explicitly different from byte `Buffer`, persistent
`List`, Nil, and Dynamic, and it cannot enter JSON or data-shape serialization paths.

The strict adapter for a kernel using `F64Buffer` uses ABI `calcit-calx-kernel/2` (existing scalar-only
kernels retain `/1`): only typed `F64Buffer`, Number, Bool, and Unit are accepted. `&f64-buffer:len`,
`&f64:to-i64-index`, and `&f64-buffer:get` lower to the three frozen Calx instructions. Invalid indexes
and bounds trap in the VM; only eligibility may choose whole-kernel fallback, and Calcit never reruns
automatically after VM execution begins.

The source-backed dot-product fixture differentially executes the same Calcit source natively and in Calx
and fixes the generated program in a golden. Its negative paths reject Nil, List, byte Buffer, and bounds
errors. Machine-dependent allocation/ownership/copy stages and crossover reporting remain in standalone
`calcit-calx-bench`; core does not reintroduce a benchmark runner.
