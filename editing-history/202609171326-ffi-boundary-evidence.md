# 收敛 FFI 边界迁移证据

- 在现有 `analyze weak-types` 增加显式 `--ffi-evidence`，按 operation 报告 browser、Node、npm、WebGPU 与未知 host 边界，不新增顶层命令。
- 复用同一 collector 为 `fix --workflow strict` 提供静态 caller、nullable/unsafe path、exact-schema helper 与 review-only trait/adapter 候选。
- 增加 Cirru EDN 结构化输出和稳定信息代码 `I_FFI_BOUNDARY_EVIDENCE`；不推断运行时 trust，不自动收窄 Dynamic 或选择业务语义。
- 增加 CLI 集成 fixture 与 Agent interface smoke，覆盖 JSON/EDN、summary-only 和 strict workflow 复用。

## English

- Add opt-in `--ffi-evidence` to the existing `analyze weak-types` surface, classifying browser, Node, npm, WebGPU, and unknown-host operations without adding a top-level command.
- Reuse one collector in `fix --workflow strict` for static callers, nullable/unsafe paths, exact-schema helpers, and review-only trait/adapter candidates.
- Add Cirru EDN output and stable informational code `I_FFI_BOUNDARY_EVIDENCE`; never infer runtime trust, narrow `Dynamic`, or choose business semantics.
- Cover JSON/EDN, summary-only output, strict-workflow reuse, and the Agent interface with CLI integration fixtures.
