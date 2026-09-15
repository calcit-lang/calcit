# 增加递归 Component List adapter

- 让类型推导得到的闭合 `List<T>` schema 直接进入同步 Component boundary，不增加新的类型声明、检测规则或 CLI 入口。
- 用一套递归 layout 与 lift/lower 规则覆盖 `List<Bool>`、`List<Number>`、`List<String>`、`List<Buffer>` 及嵌套 List；相同 item type 只生成一组 helper。
- 通过 Calcit definition `:tests` 覆盖列表语义，通过 Rust/Node 集成测试覆盖 import/export、空值、嵌套、非法 Bool、越界区间、长度计算与 memory growth。
- 更新 Component boundary、CLI 与 Agent 文档，明确 0.14.20 仍是 Number/String preview，List 属于 0.15.1 开发线。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test`、`yarn check-all`、文档检查与打包清单。
