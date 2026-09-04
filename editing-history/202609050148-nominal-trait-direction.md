# Preserve nominal trait matching direction

2026-09-05 01:48 CST

## English

- Keep actual-to-expected matching for legacy bare expected trait contracts.
- Permit reverse reference matching only when both traits carry qualified definition references, with a regression for that intentional reverse path.
- Prevent a bare actual placeholder from satisfying a runtime-identified trait by name alone.

## 中文

- 为兼容旧的 bare expected trait contract，保留 actual 到 expected 的方向匹配。
- 只有两侧都带 qualified definition reference 时才允许反向匹配，并用回归测试锁定这条有意保留的反向路径。
- 防止 bare actual placeholder 仅凭名称满足带 runtime identity 的 trait。
