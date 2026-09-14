# 退役公开发布的 `cr-wasm`

## 中文

- 从默认 Cargo 安装与 GitHub Release assets 中移除 `cr-wasm`，公开 WASM 入口收敛为 `calcit wasm` 与 `calcit wasi`。
- 将 WASI 自举预处理回归迁移到 `internal-wasi-preprocess-harness` feature 保护的 `calcit-wasi-preprocess-harness`，保留原有 wasm32-wasip1、原生 target validation 与共享预处理验证。
- 更新维护约束、WASM 验证说明、Agent 指南与 0.15.1 升级映射，明确内部 harness 不是用户 CLI。
- 验证：`cargo fmt --all -- --check`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test`、`cargo package --allow-dirty`、`bash scripts/check-docs-md.sh`、`bash scripts/test-wasi-preprocess.sh`。

## English

- Removed `cr-wasm` from default Cargo installation and GitHub Release assets, converging the public WASM surface on `calcit wasm` and `calcit wasi`.
- Moved the WASI bootstrap preprocessing regression to `calcit-wasi-preprocess-harness`, guarded by the `internal-wasi-preprocess-harness` feature, while retaining wasm32-wasip1, native target-validation, and shared preprocessing coverage.
- Updated maintainer constraints, WASM validation notes, Agent guidance, and the 0.15.1 command migration map to state that the internal harness is not a user CLI.
- Validation: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `cargo package --allow-dirty`, `bash scripts/check-docs-md.sh`, and `bash scripts/test-wasi-preprocess.sh`.
