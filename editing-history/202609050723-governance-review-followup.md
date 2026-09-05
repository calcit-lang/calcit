# 2026-09-05 07:23 UTC — Governance review follow-up / 协作治理审查修正

## 中文

- 将直接发布例外限制为版本字段及其 lockfile metadata；依赖集合或依赖版本变化必须走普通 PR。
- 发布前记录 `VERIFIED_SHA`，逐个确认 required workflow 在该 SHA 上完成且成功，并验证 annotated tag peel 后仍指向同一 commit。
- 删除只描述旧 lease、镜像修复和 lease 调度的编辑历史，避免其被误当成当前操作指南；退役事实保留在本次治理记录中。
- 补记开放 Issue 状态迁移、旧标签删除和远程锁 ref 清理的实际证据。

## English

- Restrict the direct-release exception to version fields and their lockfile metadata; dependency-set or dependency-version changes require a normal PR.
- Record `VERIFIED_SHA`, verify every required workflow completes successfully for that SHA, and ensure the annotated tag peels to the same commit.
- Remove editing histories that only described the old lease, mirror repair, and lease-based scheduling so they cannot be mistaken for current instructions; retain the retirement fact in this governance record.
- Record evidence for the open-Issue status migration, obsolete-label deletion, and remote lock-ref cleanup.
