# 区分预处理策略失效原因

## 背景

增量严格检查在 closure 未变化、仅严格类型或动态方法 warning 策略变化时会正确冷启动，但此前统一报告为 dependency closure 变化。

## 调整

- 在 core revision 与 closure revision 比较之后单独比较预处理策略。
- 策略变化时报告 `preprocessing-policy-changed`。
- 增加 CLI 回归测试，验证动态方法 warning 策略切换不会被误报为 closure 变化。

## 验证

- `cargo fmt`
- `cargo test --test incremental_analysis_cli incremental_check_only_reports_policy_invalidation_separately`
- `cargo clippy --all-targets -- -D warnings`
