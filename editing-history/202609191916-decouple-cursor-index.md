# 解耦 cursor/query 测试与 test.cirru 尾部索引（#1214）

- 根因：每次从 `calcit/test.cirru` 增删 `main!` 调用，`cursor.rs`/`query.rs` 里写死的尾节点索引
  （阶段一至四经历了 48→45→44→43→42→41→40）都要手工同步，易漏。
- `TestProject::from_fixture()` 现在解析复制出的 fixture snapshot，计算 `app.main/main!` 最后一个子节点
  的索引，存为 `main_tail: usize`。
- `cursor.rs`/`query.rs` 的所有相关断言改用 `fixture.main_tail` 或
  `format!("@{}.1", fixture.main_tail)`，不再依赖固定数字。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全绿；
  `calcit/test.cirru --compat-types` 通过。此后增删 fixture 的 `main!` 调用无需再改 cursor/query 测试。
