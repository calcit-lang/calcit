# split type-fail tests into separate file

- 将 `src/bin/cr.rs` 中与 `type-fail` fixture 相关的 helper 和测试拆分到独立文件 `src/bin/cr_type_fail_tests.rs`。
- 在 `src/bin/cr.rs` 顶部通过 `#[cfg(test)] mod cr_type_fail_tests;` 挂载测试模块。
- 保留 `cr.rs` 中的通用 schema/type 覆盖测试，减少主文件体积。
- 验证通过：
  - `cargo test -q --bin cr`
  - `yarn test-fail`
