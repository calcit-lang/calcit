# 迁移 test-map/test-string 到 definition :tests（#1210 阶段四，堆叠于 #1218）

- 新增 core `:tests` 补齐 test-map/test-string 的独有断言：
  - `&map:destruct`：`exposes-pairs-and-folds`
  - `&hash`：`ignores-insertion-order`
  - `get`：`reads-tag-keys-via-postfix`（`dict.:a`）
  - `filter-map-kv`：`skips-empty-and-propagates-failure`
  - `&str`：`converts-values-to-strings`
- 其余断言经核对已由 core 既有测试覆盖（merge/assoc/dissoc/keys/vals、bit-and/or/xor/not 与
  `&number:display-by`、format/slice/replace/escape/get-char-code、find-index/starts-with?/ends-with?/
  strip-prefix/strip-suffix、format-to-lisp/format-to-cirru 等）。
- 从 `calcit/test.cirru` 移除 `test-map`、`test-string`（fixture 保留给手动 WASM 套件）。
- `cursor.rs`/`query.rs` 尾部索引随两次移除同步 `42 → 40`。
- 未迁移：`{}`/`{,}` 的 macroexpand 形状属编译器内部，暂不迁入 Calcit 测试。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全绿；
  core 单测 304 通过；quality gate 无 diagnostics；`calcit/test.cirru --compat-types`、
  `calcit/test-wasm-suite.cirru --check-only`、`node scripts/check-strict-default.mjs` 通过。
