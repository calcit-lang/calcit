# Verify final Calx cutover release / 验证 Calx 切换后的最终 release

## 中文

- 在已包含 standalone caps cutover 的最新 main 基线上重新构建 `cargo build --release --bin calcit`。
- 最终 binary 为 9,516,000 bytes，与 caps cutover 后基线精确一致，确认删除独立 benchmark runner 不改变 runtime payload。
- rebase 后 strict clippy、621/621 library tests、`yarn check-all` 的 FFI/Agent/core/native 阶段以及 GitHub clean-runner CI 均通过；本机完整 Rust docs tests 仅受已知用户级旧 docs symlink 污染影响。

## English

- Rebuilt `cargo build --release --bin calcit` on current main after the standalone caps cutover.
- The final binary is exactly 9,516,000 bytes, matching the post-caps baseline and confirming that removing the separate benchmark runner does not change runtime payload.
- Post-rebase strict clippy, 621/621 library tests, the FFI/Agent/core/native stages of `yarn check-all`, and clean-runner GitHub CI pass; only the known user-level stale docs symlink affects the complete local Rust docs test run.
