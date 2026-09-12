# 显式 WASM dependency export 的提取失败不可省略

## 问题

PR #1016 将目标 namespace 的 function extraction failure 改为 codegen error，但位于依赖 namespace 的 `DefWasmExport` 仍可能在第一遍被省略，导致输出模块缺少明确请求的 export。

## 修改

- 将 extraction failure 的拒绝条件统一为：definition 属于 `init_ns`，或其 preprocessed form 是 `DefWasmExport`。
- 普通、非显式 dependency definition 仍可在无法提取函数结构时省略；它没有外部 export 契约。
- 增加低层 Rust predicate 回归，分别覆盖目标 definition、显式 dependency export 与普通 dependency。

## 测试放置

该分支判断发生在 compiled-program function extraction/export bookkeeping 层，无法通过已经成功生成的 Calcit/WASM 用户语义构造，因此保留为小型 Rust 边界测试；主 WASM Calcit execution suite 继续验证端到端行为。

## 验证

- `cargo test --all-targets --quiet`
- `cargo clippy --all-targets -- -D warnings`
- `corepack yarn install --immutable`
- `corepack yarn procs-link`
- `corepack yarn check-all`
- `CR_WASM_BIN=./target/debug/cr-wasm bash scripts/test-wasm.sh`
