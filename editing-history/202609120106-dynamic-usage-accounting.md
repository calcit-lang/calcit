# 对齐 Dynamic 使用量、弱类型与质量预算

## 背景

启动提示、`analyze weak-types` 与 `analyze quality` 原本分别显示合理但口径不同的 Dynamic 数字。用户能够看到差异，却无法从一个结构化结果判断变化来自未解决债务、已审阅边界重新分类，还是分析分母变化。

## 修改

- 将 `analyze.weak-types` JSON 协议升级到 v6，在 `data.summary.dynamic_usage` 中提供与启动提示同范围的分子、分母、比例、intent 分类及对账状态。
- 启动提示改为指向可精确复现自身口径的 `weak-types --only schema-dynamic,code-dynamic --summary-only --format json` 命令。
- human quality 输出在 `schemaDynamic` 与 `unresolved` 不同时解释两项预算的分类策略。
- 中文文档明确 Snapshot scope、递归位置规则、依赖和运行时生成 definition 的排除规则，以及 intentional 边界仍计入 Dynamic 使用率的原则。
- 更新协议消费者与回归测试；同一 fixture 明确覆盖启动 Dynamic 分子、unresolved weak-type 命中和 quality 预算可以不同且仍保持一致分析。

## 验证

- `cargo test --bin calcit analysis_json_envelopes_preserve_filters_and_definition_paths`
- `cargo test --bin calcit human_report_explains_distinct_dynamic_budgets`
- `node scripts/check-agent-interface.mjs`
- 在 `calcit/test.cirru` 上运行 v6 summary，确认 23 个 Dynamic 位置被 22 个 unresolved 与 1 个 intentional JS FFI 完整对账，分母为 56，`reconciled` 为 true。
