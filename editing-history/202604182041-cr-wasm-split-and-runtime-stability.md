# 2026-04-18 20:41 CR WASM Split and Runtime Stability

## 关键修改

- 将 `cr wasm` 拆分为独立二进制 `cr-wasm`，避免主 `cr` 命令体积和职责继续膨胀。
- `calcit` 的 WASM 相关验证脚本和 CI 调整为优先走 `cr-wasm`。
- 修复 `emit_wasm` 中 map/set/tuple 相关回归点，恢复 `yarn check-all` 的可通过性。
- 修复 `recollect` 在 Rust 运行时下的 `patch-map` 路径，避免本地变量解析异常导致测试阻断。
- 将 `recollect` 的 wasm 测试分为核心 smoke（阻断）与扩展 probes（非阻断），保证 wasm 独立验证但不影响 rust/js 主链路。
- 版本提升到 patch：`0.12.22`。

## 经验与注意事项

- 对运行时快照文件（`calcit.cirru` / legacy `compact.cirru`）的修改必须通过 `cr edit`/`cr tree` 完成，避免文本直接编辑破坏结构。
- 在多项目联动（calcit + recollect）场景下，发布前验证至少应覆盖：
  - calcit: `yarn check-all`
  - recollect: `yarn test:cr`、`yarn test:js`、`yarn test:wasm`
- macOS 上遇到 `iconv` 链接问题时，需带 `SDKROOT` 执行构建命令进行最终回归确认。
