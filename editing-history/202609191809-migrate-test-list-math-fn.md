# 迁移 test-list/test-math/test-fn 到 definition :tests（#1210 阶段一）

- 目标：收缩 `calcit/test.cirru` 兼容套件。先迁移独有断言到 `src/cirru/calcit-core.cirru`
  的 definition `:tests`，确认覆盖后再从兼容套件移除模块。
- 新增 core 测试：
  - `calcit.core/identity`：`preserves-generic-schema-for-local-binding`（泛型 schema 在局部绑定后保留）
  - `calcit.core/&+`：`preserves-primitive-schema-in-local-binding`
  - `calcit.core/cos`：`satisfies-pythagorean-identity`（原 test-math 的 sin²+cos²）
  - `calcit.core/range`：`handles-negative-fractional-and-overflow`
  - `calcit.core/let[]`：`destructures-positional-and-rest-bindings`
  - `calcit.core/'`：`constructs-list-from-arguments`（`(' 1 2 3)`）
  - `calcit.core/list-match`：`branches-empty-and-head-tail`
  - `calcit.core/&doseq`：`applies-side-effects-per-item`
  - `calcit.core/get`：`supports-postfix-shorthand`（`.get` / `.any?`）
  - `calcit.core/&list:map-pair`：`supports-postfix-shorthand`
- 十六进制字面量 `0x10` / `0xf` 属于 reader 语义，按 AGENTS 的边界放到 Rust：
  `src/data/cirru.rs` 新增 `parses_hex_number_literal`。
- 从 `calcit/test.cirru` 移除 `./test-math.cirru`、`./test-fn.cirru`、`./test-list.cirru`
  的 modules / ns `:require` / `main!` 调用；三个 fixture 文件保留，因为手动 WASM 渐进套件
  （`scripts/test-wasm-suite*.sh`）仍消费它们，#1211 再决定该工具的接入或删除。
- 因 main! 少了 3 个调用，`cursor.rs` / `query.rs` 测试里写死的末节点索引从 `48` 更新为 `45`
  （该节点是 `(do true)`）。
- 暂不迁移 `test-recursion`（`hole-series` 是跨定义算法而非单 API 契约，且 loop 内 atom 副作用
  无 core 归属）与 `test-def-meta`（`&get-def-schema` 需要 `inside-eval:` 上下文，core 无该宏），
  原因记录在 #1210。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全绿；
  `calcit src/cirru/calcit-core.cirru --compat-types test --tag unit --require-match` 286 通过；
  core quality gate 无 diagnostics；`calcit calcit/test.cirru --compat-types` 通过；
  `calcit/test-wasm-suite.cirru --check-only` 通过；`node scripts/check-strict-default.mjs` 通过。
