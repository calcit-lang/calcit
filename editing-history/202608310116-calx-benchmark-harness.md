# Calcit→Calx 分阶段 benchmark harness

## 中文

本次修改开始 calx-vm #39 的证据阶段，不夹带性能优化。

- source-backed scalar correctness corpus 从三个扩展到五个 kernel，加入 polynomial 与随输入规模增长的 bounded simulation；所有 case 先通过 Calcit/Calx 差分检查。
- 新增 opt-in measured compile API，把 eligibility、typed lowering plan、program construction、strict validation/instruction lowering 与总耗时分开。现有 compile API 和 fallback/trap 行为保持不变。
- 新增 `calcit-calx-bench` 单 case runner。stdout 是单个版本化 JSON，独立报告 Calcit frontend、snapshot clone、编译阶段、参数/结果边界、VM setup、一次执行、复用 VM 的 hot execution，以及 program 结构指标。
- 新增 `yarn bench-calx-e2e` orchestration，运行 debug/release、五个 kernel 与多输入规模，保留 fresh-process 原始样本，使用 median 与 median absolute deviation，并只在采样点报告 crossover。
- typed buffer 在真实同质 buffer ABI 出现前明确标记为未测量；普通 persistent collection 不伪装为 typed buffer。WASM 仍是非阻塞参照。
- 默认基准是 informational，不向普通 CI 增加噪声敏感的绝对性能阈值。

验证要求包括五个 Calx correctness kernel、单 case JSON runner、quick debug/release suite、strict Clippy、全部 Rust tests、`yarn compile`、`yarn check-all` 与 Agent interface suite。

---

## English

This change starts the evidence phase of calx-vm #39 without mixing in performance optimizations.

- The source-backed scalar correctness corpus grows from three to five kernels with a polynomial and an input-scaled bounded simulation. Every case first passes Calcit/Calx differential correctness.
- An opt-in measured compile API separates eligibility, typed lowering planning, program construction, strict validation/instruction lowering, and total time. Existing compile APIs and fallback/trap behavior remain unchanged.
- The new `calcit-calx-bench` single-case runner emits one versioned JSON value on stdout. It reports the Calcit frontend, snapshot clone, compilation stages, argument/result boundaries, VM setup, one execution, reused-VM hot execution, and program structure metrics independently.
- The new `yarn bench-calx-e2e` orchestrator runs debug/release, five kernels, and multiple input sizes. It preserves fresh-process raw samples, uses the median plus median absolute deviation, and reports crossover only at sampled points.
- Typed buffers remain explicitly unmeasured until a real homogeneous buffer ABI exists; ordinary persistent collections are not presented as typed buffers. WASM remains a non-blocking reference.
- The benchmark is informational by default and adds no noise-sensitive absolute performance threshold to ordinary CI.

Required verification includes all five Calx correctness kernels, the single-case JSON runner, the quick debug/release suite, strict Clippy, the complete Rust test suite, `yarn compile`, `yarn check-all`, and the Agent interface suite.
