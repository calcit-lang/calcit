# Harden the WASI regression sandbox / 收窄 WASI 回归沙箱

- Disable persisted checkout credentials in the pull-request WASI regression
  job because no later step performs an authenticated Git operation.
- Preopen only the repository's `calcit/` fixture directory for Wasmtime. This
  preserves Snapshot-relative module lookup without exposing the checkout root
  or `.git` to the guest binary.

- WASI PR 回归 job 不再持久化 checkout credentials；后续步骤不需要执行认证的
  Git 操作。
- Wasmtime 只预打开仓库的 `calcit/` fixture 目录，在保留 Snapshot 相对模块
  查找能力的同时，不向 guest binary 暴露 checkout 根目录或 `.git`。

## Validation / 验证

- `bash scripts/test-wasi-preprocess.sh`
- `cargo fmt --check`
- `git diff --check`
