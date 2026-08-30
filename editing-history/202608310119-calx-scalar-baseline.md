# Calx scalar baseline 数据

## 中文

在 review-fixed benchmark harness commit `a1708055` 的干净工作区上采集首份完整 Calcit→Calx scalar baseline。

- 环境为 Apple M1 Pro、macOS arm64、Rust 1.97.1；JSON 固定 git commit、dirty 状态、工具链、硬件、warm-up、sample 与噪声统计。
- debug/release 共覆盖五个 kernel、13 个输入点，每点保留 7 个 fresh-process 原始样本。
- release 样本中，随规模增长的 range-sum、Fibonacci、bounded-simulation 出现采样 one-shot crossover；固定 affine/polynomial 未出现。
- hot 结果明确限定为“复用 Calx VM 对比每次经 Calcit entry lookup 的调用路径”，不能解释成纯 interpreter dispatch 倍数。
- 数据支持下一阶段分别建立 cached Calcit callable baseline，再评估 compile cache 与 VM reuse；尚不支持自动 selection policy。
- typed buffer、peak RSS/allocation profile、WASM 参照和跨机器重复仍未完成，因此 #39 保持打开。

---

## English

The first complete Calcit-to-Calx scalar baseline was collected from a clean worktree at review-fixed benchmark-harness commit `a1708055`.

- The environment is an Apple M1 Pro on macOS arm64 with Rust 1.97.1. The JSON pins the git commit, dirty state, toolchain, hardware, warm-up, sample, and noise methodology.
- Debug and release cover five kernels and 13 input points, preserving seven fresh-process raw samples per point.
- In release samples, the input-scaled range-sum, Fibonacci, and bounded-simulation cases reach sampled one-shot crossovers; fixed affine and polynomial do not.
- Hot results are explicitly scoped to a reused Calx VM versus a Calcit call path that performs entry lookup each time. They are not an isolated interpreter-dispatch multiplier.
- The evidence supports adding an equivalent baseline that uses a cached Calcit callable before evaluating compile caching and VM reuse. It does not yet support an automatic selection policy.
- Typed buffers, peak RSS/allocation profiles, a WASM reference, and cross-machine repetition remain incomplete, so #39 stays open.
