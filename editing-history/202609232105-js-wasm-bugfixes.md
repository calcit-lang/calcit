# 0.19.1 JS/WASM bugfixes / 0.19.1 JS/WASM 缺陷修复

2026-09-23 21:05 +0800

## 中文

- #1296 是已发布的 `@calcit/procs` 与编译器生成代码之间的契约缺口：`calcit.core/vals` 调用 `&map:vals`，JS runtime 缺失相应导出。补上同名导出，统一处理小 Map 与树 Map，保留重复值与 List 返回类型；现有 Calcit definition `:tests` 继续作为语言语义契约，增加 JS runtime 回归并实际运行生成的 `calcit.core/vals`。
- #1293 的根因是 WASM 顶层、List 与 Map 值相等使用不同实现，Enum/Option 被当作指针。保留现有表层 `=` API，用内部 `__rt_value_equal` 比较 Enum variant 与递归 payload，让 List 元素和 Map 值复用；Set 的结构搜索通过现有顶层路径继承 Enum 支持。拒绝非有效 heap header 的数值，避免扩大指针误判。Calcit `calcit.core/=` 新增 `:tests`，WASM fixture 验证正反例。
- 这些改动不启用 WASI 0.3.1，也不改变 Preview 1、HTTP 宿主或迁移工具。完成 PR 与 main CI 门禁后，再独立执行 0.19.1 纯版本号发布。

验证：`yarn compile`、`yarn check-js-runtime`、生成 JS 的 `vals` 冒烟、`calcit.core/=` 的 `:tests`、`yarn try-wasm`、`cargo clippy -- -D warnings`、`cargo test`、`yarn check-all`。额外的 `cargo clippy --all-targets -- -D warnings` 命中 main 既有的 `edn_parse.rs` 测试模块顺序告警，本 PR 不混入无关重排。

## English

- #1296 is a contract gap between published `@calcit/procs` and generated code: `calcit.core/vals` calls `&map:vals`, but the JS runtime omitted that export. Add it for both small and tree maps, preserving duplicate values and the List return type. Keep existing Calcit definition-attached tests as the language contract, add a JS runtime regression, and execute generated `calcit.core/vals`.
- #1293 comes from separate WASM equality paths for top-level values, List elements, and Map values; Enum/Option fell back to pointer identity. Preserve the surface `=` API and compare Enum variants plus recursive payloads in the internal `__rt_value_equal` helper, reused by List and Map. Set structural search inherits Enum support through the existing top-level path. Validate heap headers before interpreting numeric values as pointers. Add Calcit `calcit.core/=` tests and executable positive/negative WASM fixtures.
- Do not combine the patch release with the WASI 0.3.1 target switch, Preview 1 removal, HTTP host changes, or migration-tool design. After PR and main CI gates, make a separate version-only 0.19.1 release commit.

Verified with `yarn compile`, `yarn check-js-runtime`, generated JS `vals` smoke, `calcit.core/=` tests, `yarn try-wasm`, `cargo clippy -- -D warnings`, `cargo test`, and `yarn check-all`. Extra `cargo clippy --all-targets -- -D warnings` encounters an existing main-branch test-module-order warning in `edn_parse.rs`; this PR avoids unrelated reordering.
