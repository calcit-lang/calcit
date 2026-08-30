# 隐藏 Calx benchmark 原始 JSON diff

## 中文

Calx benchmark suite 会保存包含全部原始样本的较大 JSON。将 `benchmarks/calx/*.json` 标记为
`linguist-generated` 并禁用文本 diff，使 GitHub 默认折叠生成数据且不渲染巨型逐行差异。JSON 仍保留在
仓库中，可下载、解析和复现实验结论。

---

## English

The Calx benchmark suite preserves every raw sample in relatively large JSON reports. Mark
`benchmarks/calx/*.json` as `linguist-generated` and disable textual diffs so GitHub collapses generated data by
default and does not render a massive line-by-line diff. The JSON remains versioned, downloadable, parseable, and
available for reproducing the reported conclusions.
