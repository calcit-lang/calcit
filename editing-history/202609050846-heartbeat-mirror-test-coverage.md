# Complete heartbeat mirror test coverage / 补齐 heartbeat 镜像测试覆盖

## English

- Assert that heartbeat recovery restores the `agent-lease:<issue>` comment marker as well as the `agent:claimed` label.
- Exercise repair from a stale `agent:review` mirror while the authoritative remote lease remains valid.
- Preserve the existing blocked and closed Issue exception coverage.

## 中文

- 断言 heartbeat 修复会同时恢复 `agent-lease:<issue>` 评论标记和 `agent:claimed` 标签。
- 覆盖远端权威租约仍有效、Issue 镜像却停留在旧 `agent:review` 标签时的自动修复。
- 保留已有的 blocked 与 closed Issue 异常路径覆盖。
