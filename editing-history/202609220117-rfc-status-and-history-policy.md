# 2026-09-22 01:17 RFC 状态与编辑历史策略

- 当前分层语义契约以 `RFCs/09-12-layered-semantics-and-agent-fixes-rfc.md` 为入口；RFC 索引与正文状态需要同步，避免把已实现或已退役方案当作新任务。
- `08-21-type-quality-ci-adoption-rfc.md` 依赖独立 quality baseline 门槛，已被 0.19 的默认严格诊断方向取代；保留全文作为历史设计记录，并在开头指向当前验收说明。
- 早期 WASM 三路径可行性评估不再是当前能力矩阵；实际能力由 `calcit wasm` / `calcit wasi` 的验证与现行文档决定。
- `editing-history/` 保留旧记录；今后只对语义设计、复杂故障修复和跨模块契约变化新增决策记录，机械提交由 commit/PR 追踪。
- 验证：Markdown 链接与文档检查、RFC 状态核对、`git diff --check`。
