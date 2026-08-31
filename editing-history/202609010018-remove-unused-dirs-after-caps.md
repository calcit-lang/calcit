# Remove unused dirs after caps cutover / Caps 切换后删除未使用的 dirs

## 中文

PR #567 删除了 core 内的 caps binary 与源码，但 `dirs = "6.0.0"` 仍留在 Calcit 的
non-Wasm direct dependencies。全仓搜索确认原唯一调用 `dirs::home_dir()` 已随
`src/bin/calcit_deps.rs` 删除，当前 `src/` 与 `tests/` 不再消费该 crate。

本次 follow-up 删除 `dirs` direct dependency，并通过 `cargo update --workspace` 清理
`dirs`、`dirs-sys`、`option-ext`、`redox_users`、`libredox`、`thiserror 2` 与对应 proc macro。
这不改变 Snapshot/module loading、独立 caps 或 Calx 语义。

## English

PR #567 removed the core caps binary and sources, but `dirs = "6.0.0"` remained in
Calcit's non-Wasm direct dependencies. A repository-wide search confirms that its sole
`dirs::home_dir()` call disappeared with `src/bin/calcit_deps.rs`; current `src/` and
`tests/` no longer consume the crate.

This follow-up removes the direct dependency and lets `cargo update --workspace` prune
`dirs`, `dirs-sys`, `option-ext`, `redox_users`, `libredox`, `thiserror 2`, and its proc
macro. Snapshot/module loading, standalone caps, and Calx semantics are unchanged.
