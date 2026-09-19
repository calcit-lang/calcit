# redundant-do lint/fix 覆盖与 JS FFI 引导（#1216 阶段一）

- `redundant-do-v1` 扩展覆盖 `let[]` body（此前只识别 `defn`/`fn`/`let`/嵌套 `do`）；更新
  `src/bin/cli_handlers/fix.rs` 的 `collect_redundant_do_paths` 与对应 Rust 单测。
- 澄清"lint"机制：`calcit fix` 默认预览（不传 `--rule`/`--preset`）已包含 `redundant-do-v1`，
  不写回 Snapshot，每项 suggestion 带稳定 `diagnostic_code: FIX_REDUNDANT_DO`，CI 可解析并阻断。
  未新增默认预处理 warning：严格诊断下 `throw_on_warnings` 会把 warning 当错误，
  给常见代码风格加默认 warning 会破坏严格构建。
- 文档：`docs/run/fix.md` 更新覆盖范围与 lint 说明；`docs/features/js-interop.md` 新增
  「6.1 Remove redundant top-level do」，给出 JS FFI 适配器的 before/after 示例与 preview→apply 命令。
- 保留排除项：`if`/`case` 分支、调用参数、binding value、`defmacro` body、`quote`/`quasiquote`。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全绿；
  `cargo test --bin calcit redundant_do` 通过；docs check-md 69/69。
- 后续（记录在 #1216）：如需默认 lint/`--warn-redundant-do`，需设计为 opt-in，避免严格模式回归。
