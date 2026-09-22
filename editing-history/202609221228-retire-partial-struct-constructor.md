# 退役部分 Struct 构造入口

- 移除 `%{}?` macro、底层 `&%{}?` proc、Rust 和 JavaScript runtime 实现，以及只验证旧行为的 Calcit 兼容测试。
- 项目源码在严格与兼容模式下使用旧写法时，统一给出 `E_PARTIAL_STRUCT_NIL_FILL`，指向完整 `%{}` 构造和显式 `Option<T>` / `%none`。
- 更新 0.19 升级、Struct、类型与 Agent 文档；不能自动猜测遗漏字段的业务默认值，因此不添加自动 fix。
- 新行为只有“拒绝旧语法”，无法写成通过的 Calcit `:tests`；诊断保留在 Rust 预处理边界测试，JS 导出移除由 runtime 边界检查验证。现有 Struct 正向语义仍由 Calcit 测试覆盖。
- 验证：`cargo test --lib --quiet` 837/837 通过，`yarn check-all`、`cargo fmt --all -- --check`、`cargo clippy -- -D warnings` 通过。额外运行的 `cargo clippy --all-targets -- -D warnings` 被现有 `src/codegen/emit_wasm/edn_parse.rs` 测试模块后仍有 helper 的 lint 阻断，与本次改动无关。
