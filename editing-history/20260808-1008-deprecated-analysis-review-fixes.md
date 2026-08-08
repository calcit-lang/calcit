# Deprecated Analysis Review Fixes

处理 record/tuple 迁移 PR 的未 resolve review：

- `analyze deprecated` 解析未限定 symbol 时先检查本地 namespace definition，避免 local `record?` shadowing 被误报为 `calcit.core/record?`。
- 遍历 list-valued callee，避免 IIFE 中的 deprecated call 漏报。
- JSON envelope 复用 static-analysis 的稳定 revision 算法，满足文档化 contract。
- WASM enum literal 与 enum operation 错误文案统一使用 Enum 术语。

验证：`cargo test deprecated_api`、`cr analyze deprecated --format json --summary-only`、`cargo fmt --check`。