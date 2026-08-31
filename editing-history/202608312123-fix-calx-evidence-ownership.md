# Fix Calx evidence ownership / 修正 Calx 证据归属

## 中文

- 人工引用扫描发现 `AGENTS.md` 仍把已删除的 `benchmarks/calx/*.json` 与 bootstrap manifest 当作 core source of truth。
- 改为 standalone `calcit-calx-bench` 拥有机器采样、raw reports、provenance、pins 与 product contract；core 只拥有 cache/runtime 语义、adapter 与 correctness。
- 明确禁止把 profiler 资产重新提交回 core，避免后续 Agent 逆转本次拆分。

## English

- A manual reference scan found that `AGENTS.md` still named deleted `benchmarks/calx/*.json` and the bootstrap manifest as core sources of truth.
- Moved machine sampling, raw reports, provenance, pins, and the product contract to standalone `calcit-calx-bench`; core retains only cache/runtime semantics, the adapter, and correctness.
- Explicitly prevents profiler assets from returning to core so future agents do not reverse this extraction.
