# 增加 Component Bool adapter

- 让类型推导得到的 `Bool` schema 直接进入同步 Component boundary，不增加新的类型声明或命令入口。
- Canonical ABI 使用 `i32` 的 `0`/`1`；import/export adapter 在两个方向验证非规范值并 trap，Calcit 内部值表示不泄漏到边界。
- 在 Calcit definition `:tests` 覆盖 Bool 与 Bool/Number 混合语义，在 Rust/Node 集成测试覆盖真实 WASM 签名、往返和非法边界值。
- 更新 Component boundary、CLI 与 Agent 文档，保留 0.14.20 Number/String preview 的历史范围，并说明 0.15.1 开发线新增 Bool。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`、`yarn check-all`。
