# 迁移 test-lens/test-cond 到 definition :tests（#1210 阶段二）

- 从 `calcit/test.cirru` 移除 `./test-lens.cirru` 与 `./test-cond.cirru` 的 modules / ns `:require` /
  `main!` 调用；fixture 文件保留，手动 WASM 套件仍引用它们。
- 新增 core `:tests`：
  - `assoc-in`：`assocs-nested-maps-lists-and-nil-base`
  - `update-in`：`updates-collection-and-missing-paths`
  - `dissoc-in`：`dissocs-nested-maps-and-list-indexes`
  - `contains-in?`：`traverses-maps-lists-tags-and-enums`
  - `and`/`or`/`either`/`when`/`when-not`/`cond`/`case` 各一条行为测试
  - `Option`：`matches-payloads-and-rejects-arity-mismatch`，承载原 `test-cond` 的 `match` 形状断言。
    `match` 是内置语法、没有 def 归属，挂在 canonical enum `Option` 上，保证仍由 CI 的 core 单测执行。
- 分类结论：`test-macro`（macroexpand 形状属实现细节）留待后续阶段；`test-hygienic`、`test-algebra`
  在 fixture 内定义本地宏/trait，按 AGENTS 作为集成 fixture 保留。
- `cursor.rs`/`query.rs` 写死的 `app.main/main!` 尾部索引随两次移除从 45 同步到 43；已开 #1214 跟踪解耦。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全绿；
  core 单测 298 通过；core quality gate 无 diagnostics；`calcit calcit/test.cirru --compat-types`、
  `calcit/test-wasm-suite.cirru --check-only`、`node scripts/check-strict-default.mjs` 均通过。
