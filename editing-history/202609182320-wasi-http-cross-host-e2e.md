# Verify buffered WASI HTTP across hosts / 跨宿主验证 buffered WASI HTTP

## 中文

- 为同一份 Calcit typed HTTP contract 增加 JS core host 与 Wasmtime packaged Component 两条真实本机 HTTP 集成测试。
- 同时覆盖显式 origin capability 拒绝和 response size 上限，错误保持为闭合 `HttpError`，不伪造成功响应。
- JS 宿主拒绝非法 URL，并关闭自动 redirect，避免从已授权 origin 跳转到未授权网络目标。
- 在 Calcit 定义上补充 success、capability denied、response-too-large 的 `:tests`，让用户先从 Calcit 语义 review。
- 文档固定现有命令链、Cirru EDN 默认 contract、宿主显式授权和当前不支持范围，不增加 CLI 入口。

## English

- Add real local HTTP integration through both a JavaScript core host and a Wasmtime packaged Component for the same typed Calcit contract.
- Cover explicit origin-capability denial and response-size bounds while preserving closed `HttpError` results instead of fabricating success.
- Make the JavaScript host reject malformed URLs and disable automatic redirects so an allowed origin cannot reach an ungranted network target.
- Add definition-attached Calcit tests for success, capability denial, and response overflow so review starts from Calcit semantics.
- Document the existing command chain, Cirru EDN default contract, explicit host grants, and current limitations without adding a CLI entry point.

## Verification / 验证

- `target/debug/calcit tests/fixtures/component-wasm-async-import.cirru test --tag wasm --require-match`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `yarn check-all`
- `yarn check-agent-interface`
