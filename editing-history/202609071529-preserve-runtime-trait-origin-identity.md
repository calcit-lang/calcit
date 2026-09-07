# Preserve runtime trait origin identity / 保留 runtime trait 来源身份

Issue: calcit-lang/calcit#844
Review: calcit-lang/calcit#900

## 中文

- 根据 CodeRabbit 对最新 head 的有效审查，修正 requires 稳定排序：machine key 同时包含 source-facing origin label 与 `runtime_id`，避免热重载前后的 trait 共享 `definition_ref` 时回退到输入顺序。
- strict 重复 impl 判定改为 `has_same_origin`，不再比较可能相同的展示标签；诊断仍保持面向源码的完整 origin。
- 新增两个 runtime trait 共享同一 `definition_ref`、但 `runtime_id` 不同的回归，验证两者不会被去重且 requires 重排不改变稳定顺序。

## English

- Addressed the valid CodeRabbit finding on the latest head by including `runtime_id` alongside the source-facing label in the stable requires ordering key, so reload generations sharing one `definition_ref` do not fall back to input order.
- Changed strict duplicate-impl classification to use `has_same_origin` instead of comparing rendered labels; diagnostics remain source-facing.
- Added a regression with two runtime traits sharing one `definition_ref` but carrying different `runtime_id` values, proving they remain distinct and reorder deterministically.

## Verification / 验证

- focused runtime-origin and strict duplicate-impl tests
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `git diff --check`
