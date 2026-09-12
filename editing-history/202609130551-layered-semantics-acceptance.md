# 收口分层语义 RFC

时间：2026-09-13 05:51 +0800

## 背景

#997 已经产出分层语义 RFC，并被 `AGENTS.md`、类型指南、Agent 文档以及后续 source fix/WASM 工作引用，
但 RFC 仍标记为 Draft，验收要求中的代表性语义例子也没有逐项连接到仓库内的可执行证据。

## 调整

- 将 RFC 状态更新为 Accepted。
- 增加普通 typed value、开放集合与 `Option<Dynamic>`、macro、JS FFI、WASM typed 子集的证据矩阵。
- 区分 Calcit definition 所表达的语义契约与脚本承担的 backend/ABI 验证职责。
- 明确 backend 不共同支持的能力通过 capability boundary 或稳定 unsupported diagnostic 表达，不靠表层语义退化补齐。

## 验证

- 文档检查通过 65 个文件、331 个代码块。
- core definition tests 通过 257 项，并启用 `--require-match`。
- native 主测试与生成 JS 后的 Node.js 主测试通过。
- WASM definition tests 通过 3 项，生成模块的 Node.js 实际执行与 fail-closed 检查通过。
