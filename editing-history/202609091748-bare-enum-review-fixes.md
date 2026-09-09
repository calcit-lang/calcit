# Bare enum review fixes / 裸枚举评审修复

## English

- Updated expected-value descent for the indexed match representation emitted
  by preprocessing nominal enums; empty table slots are ignored and actual
  branch bodies are checked.
- Added a strict regression with a bare `%none` in an indexed `match` branch.
- Reused `resolve_to_fn` instead of cloning a full annotation, documented the
  new helpers, and clarified the top-level Cirru invocation fixture.

## 中文

- 适配名义枚举预处理产生的 indexed match 表示；忽略空 slot，并继续检查实际
  branch body 的预期值。
- 新增 indexed `match` 分支中裸 `%none` 的严格模式回归。
- 改用 `resolve_to_fn`，避免复制完整类型 annotation；同时补充新 helper 文档，
  并解释顶层 Cirru 调用测试的语义。

## Validation / 验证

- `cargo test --bin calcit enum_constructor -- --nocapture`
- `cargo clippy --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`
