# Calcit design narrative and real-time application model / Calcit 设计叙事与实时应用模型

## Summary / 概要

- Repositioned Calcit as its own typed functional language rather than a ClojureScript dialect.
- 将 Calcit 重新定位为独立的 typed functional language，不再以 ClojureScript dialect 作为主要身份。
- Rewrote the README, introduction, overview, and feature hub around nominal structs/enums, traits and methods, Option/Result, static analysis, typed FFI, canonical Cirru source, and deterministic state updates.
- README、introduction、overview 与 feature hub 改为围绕 nominal struct/enum、trait/method、Option/Result、静态分析、typed FFI、canonical Cirru source 与确定性状态更新组织。
- Converted the former Clojure comparison page into a historical influence and migration aid. Comparisons remain only where they prevent a concrete migration mistake.
- 原 Clojure 对比页改为历史影响与迁移提示；仅在防止具体迁移误判时保留比较。
- Added a real-time application model centered on Calcium Workflow, Respo, Recollect, typed WebSocket envelopes, revision/ack/resync, bounded async work, observability, and convergence testing.
- 新增以 Calcium Workflow、Respo、Recollect、typed WebSocket envelope、revision/ack/resync、有界异步、可观测性与收敛测试为中心的实时应用模型。
- Updated agent, syntax, Tag, macro, bundle, and structural-source wording so supporting docs follow the same design vocabulary.
- 更新 agent、syntax、Tag、macro、bundle 与 structural-source 文案，使辅助文档采用一致的设计词汇。
- Added the same language-positioning, Calcium architecture, method-first API, cross-project validation, and bilingual tracking policy to `AGENTS.md` so future work keeps the direction.
- 将语言定位、Calcium architecture、method-first API、跨项目验证及双语追踪规则写入 `AGENTS.md`，使后续工作持续遵循该方向。

## Policy / 约束

Future documentation should explain Calcit concepts on their own terms. Historical comparisons are migration aids, not design authorities. Language and ecosystem changes for web applications should preserve one coherent Calcium-style model: typed boundaries, serial deterministic business updates, pure projections, revisioned incremental synchronization, bounded effects, and observable convergence.

后续文档应直接解释 Calcit 自身概念；历史比较只用于迁移，不作为设计依据。面向 Web 应用的语言和生态改动应保持一致的 Calcium 模型：typed boundary、串行确定性业务更新、纯 projection、revisioned incremental synchronization、有界 effect 与可观测收敛。
