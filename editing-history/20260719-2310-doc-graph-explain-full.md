# 2026-07-19 23:10 graph explain 定义摘要

## 修改概要

- 为 `cr docs graph explain` 增加 `--full`，输出 Calcit 定义的 doc 和 examples 可用性。
- 缓存定义记录保存 doc 文本，并将 cache schema 升级为 5，确保旧缓存自动重建。
- 更新 command echo、`CalcitAgent.md` 和 graph 验证案例。

## 验证

- `cargo fmt --all`
- `cargo test -q`
- `cargo clippy --all-targets -- -D warnings`
- `yarn compile`
- `cr docs graph explain calcit.core/nth --full`
- `git diff --check`
