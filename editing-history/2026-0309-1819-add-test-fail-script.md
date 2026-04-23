# add test-fail script

- 在 `package.json` 中新增 `yarn test-fail`，用于日常单独执行 schema/type-fail 相关测试。
- 该命令当前映射到 `cargo test -q --bin cr type_fail_`，覆盖 `src/bin/cr.rs` 中新增的 fixture 测试。
- 更新 `calcit/type-fail/README.md`，补充 `yarn test-fail` 的使用说明。
