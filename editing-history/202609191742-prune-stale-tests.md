# 清理过期测试并缓存测试用 core snapshot

- 删除 7 个自 2026-01/02 起无任何入口引用的 dynamic 时代 fixture：`calcit/test-invalid-tag.cirru`、
  `test-ir-type-info`、`test-method-validation`、`test-nested-types`、`test-optimize`、
  `test-proc-type-warnings`、`test-sum-types`。逐个全仓（含隐藏目录）按文件名与 namespace 搜索确认无引用，
  且它们连 `--check-only` 都因 `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA` 失败。
- 用 `calcit tree delete` + `edit rm-def`（先 `cargo build` 得到与 main 一致的 0.17.0 CLI）删除
  `calcit/test-types.cirru` 中已注释/未调用的 `test-method-type-errors`、`test-proc-type-warnings`
  及其调用注释。
- 修正 `calcit/type-fail/README.md` 文档漂移：移除指向不存在的 `type-slot-bind-unknown.cirru` /
  `type-slot-bind-duplicate.cirru` 的条目，并把测试路径从 `src/bin/calcit.rs` 改为
  `src/bin/cr.rs` 的 `cr_type_fail_tests` / `src/bin/cr_tests/type_fail.rs`。
- Rust 测试提速：在 `src/bin/cr.rs` 增加 `#[cfg(test)] cached_core_snapshot()`（`OnceLock`），
  `type_fail.rs`、`cirru_suite.rs` 与 `cr.rs` 测试不再重复 rmp 解码 core snapshot，改为 clone。
  `cr_type_fail_tests` 本地从 19.0s 降到 12.9s；bin 单测整体 21.8s。
- 后续分阶段任务记录为 #1210（迁移 `test-*.cirru` 独有断言到 `:tests`）与 #1211（审查未引用的测试工具）。
- 未删除最近仍在维护的 wasm 渐进套件脚本，避免误删手动工具。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（全绿）、
  `calcit src/cirru/calcit-core.cirru --compat-types test --tag unit --require-match`（276 通过）、
  `calcit calcit/test.cirru --compat-types`、`node scripts/check-strict-default.mjs`、docs check-md 69/69。
