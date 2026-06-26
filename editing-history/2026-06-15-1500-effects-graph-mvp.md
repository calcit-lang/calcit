# `cr analyze effects-graph` Phase A MVP

## 概要

实验性只读分析命令，从入口定义递归分解 State / Transform / Effect 三维信息。

## 实现

- 新模块 `src/effects_graph.rs`：`EffectsGraphAnalyzer` 遍历 `PROGRAM_CODE_DATA`，复用 call-tree 可达性模式。
- Tag 来源：`load_core_snapshot()` 的 `calcit.core` `:tags` + `RegisteredProcDescriptor.tags`。
- CLI：`cr analyze effects-graph [--root ns/def] [--format tree|json] [--detail summary|full|minimal] ...`
- 测试夹具：`calcit/test-effects-graph.cirru`

## 路径修正

RFC 文件名统一为 `RFCs/06-15-effects-graph-rfc.md`（原 `06-10-...` 已手动纠正）。

## 已知限制（Phase B）

- `mermaid` 输出未实现
- `--infer-missing` 未实现
- `detail full|minimal` 暂未差异化输出
- Respo 专用规则（`render!`、`d!`）仅靠 `!` 启发式

## 输出格式（2026-06-15 更新）

- **默认 `--format sketch`**：聚合 birdview 文本（State / Lifecycle / Effects 通道 / Data flow）
- 自动 `--ns-prefix` 推断为 `app.`（从入口 ns 首段）
- 默认 `--max-depth 2`；库代码不展开
- `--format mermaid` / `tree` 仍可用
