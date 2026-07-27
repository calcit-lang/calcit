# 2026-07-19 22:45 graph missing 分批筛选

## 修改概要

- 为 `cr docs graph missing` 增加 `--ns` namespace 前缀过滤。
- 增加 `--limit` 输出数量限制，便于按批次补充公共 API 的 `code_refs`。
- 在 `CalcitAgent.md` 和知识图谱 RFC 中补充该操作说明。
- 增加 namespace 过滤、数量限制和内部命名排除的单元测试。

## 验证

- `cargo fmt --all`
- `cargo test -q`
- `cargo clippy --all-targets -- -D warnings`
- `yarn compile`
- `cr docs graph missing --ns calcit.core --limit 5`
- `git diff --check`
