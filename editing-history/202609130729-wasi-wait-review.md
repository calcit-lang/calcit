# 收紧 JavaScript 同步等待注入

时间：2026-09-13 07:29 +0800

## 背景

PR #1043 的 review 指出，JavaScript `wait_ms` injection 的返回类型为 `unknown`，若宿主误传异步函数，
runtime 会在 Promise 真正完成前返回 `%ok`，随后 rejection 既不能进入 Calcit `Result`，也可能成为未处理拒绝。

## 调整

- 检查 injection 返回值是否为 Promise/thenable；发现异步结果时立即返回同步契约错误。
- 为已返回的 thenable 安装 rejection handler，避免错误被遗留为宿主级 unhandled rejection。
- 增加 rejecting async injection 回归，并等待一个事件循环阶段证明 rejection 已被消费。
- 文档明确 JavaScript 同步 wait injection 不接受 Promise/thenable。

## 验证

- `yarn compile`
- `yarn check-js-runtime`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- `yarn check-all`
