# 2026-08-31 15:10 — freeze Calx harness extraction

## 中文

- 明确实验性 Calx benchmark harness 与 Calcit core lowering/correctness 的产品边界。
- 新增双语 extraction contract，冻结 schema v2、stdout/stderr、provenance、raw samples 和无绝对 CI 阈值的报告政策。
- 定义 revision-pinned internal adapter 的最小 session 能力，禁止外部 harness 直接访问可变 program globals。
- 新增 machine-readable bootstrap manifest，逐项标记 move、copy-with-provenance、stay-in-core 和 3 Rust + 4 Node tests/smoke 迁移矩阵。
- 增加 regression tests，验证 manifest 资产、tracking、所有权无误，并检查归档 suite 的工具链/版本/raw sample 可追溯性。

## English

- Separate the experimental Calx benchmark product from core lowering and correctness ownership.
- Freeze schema-v2 IO, provenance, raw-sample preservation, and informational-only threshold policy in a bilingual contract.
- Define a revision-pinned internal session adapter and forbid direct mutable-global access from the standalone harness.
- Add a machine-readable bootstrap manifest for moved, copied-with-provenance, and core-owned assets plus the 3 Rust + 4 Node test/smoke matrix.
- Add regression tests for manifest ownership/tracking and archived report provenance/raw samples.
