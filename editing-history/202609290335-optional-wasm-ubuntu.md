# 可选 WASM 编译与 Ubuntu 22.04 资产

关联 #1471、#1472。

## 决策

低资源 Ubuntu 22.04 x86_64 主机可能无法完成 Calcit 源码编译。保留现有默认构建的 WASM/WASI 功能，同时将其独立为默认开启的 Cargo `wasm` feature；`--no-default-features` 真正排除 `wasm-encoder`、正常依赖链中的 `calcit-bindgen` 及 WASM codegen 模块，不影响 native/JS 与 Agent CLI。关闭后请求 `calcit wasm` / `calcit wasi` 必须明确报错，不能静默回退。内部 WASI harness 依赖 `wasm` feature，防止无意义的组合。

Release 工作流固定在 Ubuntu 22.04 x86_64 上，先构建并保存常规二进制，再用 `--no-default-features` 构建独立的 `calcit-ubuntu22.04-x86_64-no-wasm`。两个资产都进入同一 SHA-256 manifest；发布前在目标 runner 上检查文件、动态依赖、版本及 native/JS 基本行为。PR 的 CI 也要在同一 runner 上验证轻量构建和 WASM/WASI 禁用提示。

## 边界

该资产依赖 Ubuntu 22.04 用户态，不宣称静态链接或兼容更旧的 glibc。低资源构建的目标是减少本机编译开销，而不是减少运行时所有内存消耗。正常 `cargo test` 的 dev-dependencies 包含 Wasmtime，不能用它衡量轻量生产构建；使用 `cargo build --no-default-features --bin calcit` 和正常依赖树验证。

## 验证

本地验证：默认功能的 `cargo test --locked -q`、`cargo clippy --all-targets -- -D warnings`、`yarn compile` 和 `yarn check-all` 全部通过；无 WASM 的 `cargo check --locked --no-default-features --bin calcit`、`cargo build --locked --no-default-features --bin calcit`、838 个库测试、392 个 CLI 测试及 `cargo clippy --no-default-features --bin calcit -- -D warnings` 通过。无 WASM 二进制的 native/JS smoke 通过，`wasm`/`wasi` 均明确报禁用错误。正常依赖树从 226 行降为 158 行，后者不包含 `wasm-encoder`、`calcit-bindgen`、`wasmtime`。Release manifest 单元测试、Markdown 检查与 `cargo fmt --check` 通过。

Ubuntu 22.04 x86_64 的 release profile 构建与 smoke 尚待 PR 的同平台 GitHub Actions 验证；本机 macOS 的验证不能替代该验收。
