# 隔离非法字段 warning 测试状态

## 中文

- 为 `warns_on_invalid_method_field_access` 持有共享 preprocess test-state guard，避免并行测试修改 JS FFI feature policy 时产生偶发的 `E_JS_FFI_FEATURE_REQUIRED`。
- 该测试继续验证非法 Struct field warning；不改变类型推导、capability policy 或运行时行为。
- 测试保留在 Rust，因为它验证的是全局测试状态隔离这一并发 invariant，而非用户可观察的 Calcit 语义。
- 验证：目标测试、连续多轮默认并行 `cargo test --lib`、`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`。

## English

- Held the shared preprocess test-state guard in `warns_on_invalid_method_field_access` so parallel JS FFI feature-policy tests cannot cause intermittent `E_JS_FFI_FEATURE_REQUIRED` failures.
- The test continues to verify the invalid Struct-field warning; type inference, capability policy, and runtime behavior are unchanged.
- The test remains in Rust because it covers a concurrent global-test-state invariant rather than user-observable Calcit semantics.
- Validation: the focused test, repeated default-parallel `cargo test --lib` runs, `cargo fmt --all -- --check`, and `cargo clippy --all-targets -- -D warnings`.
