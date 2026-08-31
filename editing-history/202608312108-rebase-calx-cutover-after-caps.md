# Rebase Calx cutover after caps / 在 caps 迁出后重整 Calx 切换

## 中文

- 将 Calx benchmark 产品资产清理 rebase 到已合并 standalone caps cutover 的最新 `main`。
- README 冲突保留两个已拆分模块的最终 ownership，并继续明确 Calx lowering/cache/runtime/correctness 属于 core。
- 顺序增量修正为 binary targets `3 → 2`、direct dependencies 保持 `30`；最终 targets 仅有 `calcit` 与 `cr-wasm`。
- 删除 runner 目标不进入 `calcit` 链接路径；原始基线上的隔离 release 前后构建曾保持 `9,515,920` bytes 完全一致。

## English

- Rebased the Calx benchmark product-asset cleanup onto current `main` after the standalone caps cutover merged.
- Resolved the README overlap by retaining final ownership for both extracted modules while keeping Calx lowering/cache/runtime/correctness in core.
- Corrected the sequential delta to binary targets `3 → 2` with direct dependencies remaining `30`; only `calcit` and `cr-wasm` remain as final targets.
- The removed runner target is outside the `calcit` link path; a fresh release build after rebasing onto the caps cutover is byte-identical to that sequential baseline at `9,516,000` bytes.
