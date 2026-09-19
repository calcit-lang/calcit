# 迁移 test-set 到 definition :tests（#1210 阶段三）

- `test-set` 只有一条断言 `.add (#{} 1 2) 3`；在 `calcit.core/include` 新增
  `adds-via-postfix-method` 覆盖 set 的后缀 `.add` 方法，然后从 `calcit/test.cirru` 移除
  `./test-set.cirru`（fixture 保留给手动 WASM 套件）。
- `cursor.rs`/`query.rs` 写死的 `app.main/main!` 尾部索引随移除同步 `43 → 42`。
- 本阶段同时完成规划整理：
  - 新建 #1216（fn/let body 顶层 `do` 的 lint 与 fix 规则）与 #1217（JS FFI 方法调用体验与文档引导）。
  - 新建 milestone `0.18.0 — Code quality automation and JS FFI ergonomics`，纳入 #1216/#1217；
    原 WASI sockets milestone 重命名为 `0.19.0`；#1214 归入 `0.17.1` 测试整理。
- 说明：`test-map` 仍含 `&map:destruct`/`&hash`/`.:` 等尚未覆盖断言，本阶段未移除，留待后续阶段。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全绿；
  core 单测 299 通过；quality gate 无 diagnostics；`calcit/test.cirru --compat-types`、
  `calcit/test-wasm-suite.cirru --check-only` 通过。
