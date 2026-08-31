# Calx cached callable 公平基线

## 中文

为 calcit#548 与 calx-vm#39 增加公平的 Calcit→Calx repeated-call 计量边界：

- 保留 `run_program_with_docs` 的 embedding-visible lookup-call 指标；
- 从已经 preprocess 的 program 解析并缓存同一个 `CalcitFn`，直接通过 `runner::run_fn` 建立 cached-native baseline；
- cached Calcit 与 reused Calx VM 使用相同 warm-up/iteration 数，并在计时前准备每次调用的输入；
- Calcit cached 路径仍计入函数 scope 建立、参数绑定与 runner execution，不把它缩减成空 dispatch；
- 单 case 与 suite schema 升级到 edition 2，分别报告 lookup-native、cached-native 与 Calx hot ratio/crossover；
- correctness gate 同时验证 lookup Calcit、cached Calcit 与 Calx 结果，缺失 callable 明确失败；
- 这一步只修正 benchmark 证据，不引入 production cache、VM pool、自动 offload，也不改变 Nil/Dynamic strict boundary。

快速 debug/release matrix 已验证 schema、correctness 与三类 crossover 输出。完整 clean-worktree baseline 在首个实现提交后重新采集。

---

## English

Add a fair repeated-call boundary for calcit#548 and calx-vm#39:

- Preserve the embedding-visible lookup-call metric through `run_program_with_docs`.
- Resolve and cache the same `CalcitFn` from the already-preprocessed program, then call it directly through `runner::run_fn` for the cached-native baseline.
- Use equal warm-up and iteration counts for cached Calcit and a reused Calx VM, with per-call inputs prepared before timing.
- Keep function-scope setup, argument binding, and runner execution inside the cached Calcit timing boundary instead of reducing it to an empty dispatch.
- Bump the single-case and suite schemas to edition 2 and report lookup-native, cached-native, and Calx hot ratios/crossovers separately.
- Gate correctness across lookup Calcit, cached Calcit, and Calx, with an explicit failure for missing cached callables.
- This slice corrects benchmark evidence only. It does not add a production cache, VM pool, automatic offload, or any relaxation of the strict Nil/Dynamic boundary.

The quick debug/release matrix verifies the schema, correctness gate, and three crossover views. The complete clean-worktree baseline is recollected after the first implementation commit.
