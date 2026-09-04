# Reconcile member-contract PR with its updated base / 协调成员契约 PR 与更新后的基线

## Context / 背景

The collection-member contract work in PR #624 is stacked on PR #622. After #622 incorporated the lexical FFI safety changes from `main`, overlapping tests and documentation needed an explicit reconciliation.

集合成员契约 PR #624 基于 PR #622。#622 合入 `main` 的词法 FFI 安全改动后，重叠的测试和文档需要显式协调。

## Resolution / 解决方案

- Retain #624's broader `get`/`includes?`/`contains?`/`assoc`/`dissoc` specialization and its focused fixtures.
- Inherit the updated FFI diagnostics, static-analysis documentation, and generated quality metadata from the rebased parent.
- Re-run type-fail, core-quality, and generated Dynamic-classification checks on the combined tree.

- 保留 #624 对 `get`/`includes?`/`contains?`/`assoc`/`dissoc` 的更完整专门化及其定向 fixture。
- 继承已更新基线中的 FFI diagnostics、静态分析文档和生成的质量元数据。
- 在合并后的树上重新运行 type-fail、core-quality 和生成 Dynamic 分类检查。
