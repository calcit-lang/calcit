# Review 后重新采集 Calx baseline

## 中文

PR #535 review 修复改变 one-shot pure-execution 的计时边界，并使 ordinary compile path 不再创建 timer。
因此在干净 commit `a1708055` 上重新运行完整 debug/release、五 kernel、13 输入点矩阵，替换旧原始 JSON。

- 报告记录 `gitDirty=false`，182 个保留样本均报告从 resolved Cargo.lock 读取的 calx_vm 0.3.0。
- 修正后的 release compile total 中位数范围约 38–72 μs。
- 采样 crossover 结论未改变：range-sum 与 bounded-simulation 在 100、Fibonacci 在 10 出现 one-shot crossover；fixed affine/polynomial 未出现。
- 原始 JSON 继续由 `.gitattributes` 标记为 generated/no-text-diff。

---

## English

PR #535 review fixes changed the one-shot pure-execution timing boundary and removed timer creation from ordinary
compile paths. The complete debug/release matrix of five kernels and 13 input points was therefore rerun from clean
commit `a1708055`, replacing the previous raw JSON.

- The report records `gitDirty=false`, and all 182 retained samples report calx_vm 0.3.0 from the resolved Cargo.lock.
- The corrected median release compile-total range is approximately 38–72 μs.
- Sampled crossover conclusions are unchanged: range-sum and bounded-simulation cross at 100, Fibonacci at 10, while fixed affine/polynomial do not reach a one-shot crossover.
- `.gitattributes` continues to mark the raw JSON as generated with textual diffs disabled.
