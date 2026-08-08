# PR Review 与 CI 修复

针对 record/tuple 到 struct/enum 迁移 PR 的 review 与 CI 失败做最小修复：

- Snapshot round-trip 测试改为断言 canonical 的 `'CodeEntry` 输出，保留 Fn/Macro schema 验证。
- `dump_code` 的运行时 enum/struct 值 metadata `:kind` 恢复为兼容的 `:tuple`/`:record`；定义 metadata 继续使用 `:enum`/`:struct`。
- 修正 enum、struct 与 WASM 布局的遗留文案和注释偏移量。
- 修复此前迁移记录中分裂的 Markdown code span。

验证：snapshot 精确测试、`cargo test --lib`、`cargo fmt --check`、`cargo clippy -- -D warnings`。