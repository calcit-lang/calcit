# 2026-08-31 16:34 — strengthen Calx harness contract regression

## 中文

- 根据 PR #560 review，把 bootstrap tracking 从宽松的字符串形状检查收紧为 #547/#557/#558/#559 精确映射。
- 对 `move` 与 `stayInCore` 资产集合做完整相等检查，防止静默遗漏或额外迁移 core correctness 资产。
- 固定归档报告的 debug/release profile、13 case × 7 samples × 2 profiles，以及 182 个 raw samples 总数。
- 验证归档主机身份、精确 Calcit commit/package version、resolved `calx-vm` version 和每个 sample 的 profile/OS/architecture。

## English

- Replace loose bootstrap tracking checks with exact #547/#557/#558/#559 mappings after the PR #560 review.
- Assert complete equality for the `move` and `stayInCore` asset sets so omissions or accidental correctness-asset migration cannot pass silently.
- Freeze the archived debug/release profiles, 13 cases × 7 samples × 2 profiles, and all 182 retained raw samples.
- Verify archived host identity, the exact Calcit commit/package version, the resolved `calx-vm` version, and each sample profile/OS/architecture.
