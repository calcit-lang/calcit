# 2026-02-15 diagnostics 合并记录

本文件合并以下 diagnostics 相关记录，避免碎片化：

- 2026-0215-1011-preprocess-diagnostic-v1.md
- 2026-0215-1026-cli-diag-json.md
- 2026-0215-1104-diag-expected-actual-action-let.md
- 2026-0215-1110-eval-diag-fallbacks.md
- 2026-0215-1116-macro-diagnostic-protocol.md
- 2026-0215-1134-macro-diag-expansion-if-defn-assert.md
- 2026-0215-1152-runtime-fallback-tag-match-arity.md
- 2026-0215-1202-warning-arity-fallback-compat.md
- 2026-0215-1211-warning-learning-fields.md
- 2026-0215-1238-convergence-help-docs-diag.md
- 2026-0215-1306-text-diagnostics-convergence-fix.md
- 2026-0215-1332-diagnostics-simplification-example-driven.md

## 合并后的结论（当前保留）

- 已移除 `--diag json` 路径，CLI 统一使用文本诊断输出。
- warning 维持最小字段与文本展示，不再维护 JSON 结构演进。
- error 维持 call stack + examples 辅助定位，避免复杂规则映射。
- 文本模式保持简洁：优先 message/hint/location/stack，便于直接修复。

## 本轮简化（减法）

- 移除 warning 侧复杂增强字段：`fingerprint/convergence/help/learning`。
- 移除 error 侧复杂增强字段：`fingerprint/convergence/help/learning` 与大段规则映射。
- 将 error 的 expected/actual/action 处理改为“hint 优先 + 通用 message 提取 + 最小 fallback”。

## 设计原则（后续继续遵循）

- 诊断能力优先复用编译器已有信息（hint、message、stack、examples）。
- 尽量减少按错误码硬编码规则，避免语言层复杂度膨胀。
- LLM 修复入口尽量通用化：`query examples` + 最小可执行动作。
