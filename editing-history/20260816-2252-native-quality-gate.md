# 2026-08-16 原生静态质量门禁

- 新增 `cr analyze quality`，一次聚合 type coverage、unresolved Dynamic、nil/Optional 迁移债务和 deprecated calls，并按零目标或 baseline 返回 CI 可用的失败退出码。
- 兼容业务项目已有的八项扁平 JSON baseline；`--write-baseline` 生成带 scope 和 definition 级预算的原生格式，防止跨 definition 的清债掩盖新增回归。
- human/JSON 报告统一提供 limit、delta 和 regression；失败 JSON 继续保证 stdout 为单个 envelope。
- 在 `respo-markdown.calcit` 与 `calcit-theme.calcit` 的现有 baseline 上完成真实项目回归；提交前运行 `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test`、`yarn compile`、`yarn check-agent-interface` 和 `yarn check-all`。
