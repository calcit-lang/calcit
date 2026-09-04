# Agent lease heartbeat mirror recovery

UTC: 2026-09-04T19:40:45Z

## English

- Let `heartbeat` renew an owner-matching authoritative remote lease when the Issue state-label mirror is missing or stale.
- Keep closed and explicitly `agent:blocked` Issues non-renewable.
- Make `agent:blocked` dominant over conflicting claimable labels for both acquisition and renewal.
- Add shell regression coverage for mirror repair and both protected terminal states.

## 中文

- 当 Issue 状态标签镜像缺失或陈旧时，只要远端权威租约 owner 匹配，允许 `heartbeat` 续租并修复镜像。
- 已关闭或显式标记 `agent:blocked` 的 Issue 仍禁止续租。
- 即使可领取标签异常共存，`agent:blocked` 在领取与续租路径中都保持最高优先级。
- 增加 shell 回归测试，覆盖镜像修复以及两种受保护状态。
