# Cursor focus、导航栈与结构化 clipboard

## 概要

- `cursor show` 基于 Cirru Parser 0.2.15 `focus_cirru_preview_with_options` 展示 definition 上下文，由 parser 直接保留 definition 签名并生成展示态 `CURSOR`。
- 精确锁定 `cirru_parser = 0.2.15`。原因是 `cargo install --path` 不保证沿用仓库 lockfile；若只使用兼容版本范围，本地测试与全局安装可能获得不同的 focus marker 语义。
- `.calcit-cursor.cirru` 升级到 schema v2，继续兼容读取 v1；新增普通导航 history、显式 `push/pop` stack 与保存 Cirru tree 的 clipboard。
- 新增 `cursor back/push/pop/copy/cut/paste/clipboard/clear-clipboard`。cut 后选择 parent，paste 后选择新节点，clipboard 保留以支持重复粘贴。
- 顶层 `--cursor-after none|summary|focus` 控制 mutation 后的 stderr 回显；默认 summary，不影响实际 cursor 维护。
- `edit cp/mv/split-def` 支持 `@cursor`；definition overwrite/rename/move/delete、namespace delete 与 transaction 提交后验证会更新或明确提示 cursor 状态。

## 准确性要点

- `edit mv` 按插入路径、源删除后的 index 漂移和 cursor 在源 subtree 内的相对路径做确定性迁移，不依赖可能多命中的 fingerprint 搜索。
- focus、full 和 node 都只构造 presentation tree；真实 snapshot、path、fingerprint 与 JSON `tree` 不包含 `CURSOR`。
- JSON `cursor.show` 分开返回真实 `tree` 和展示用 `preview_tree`，并报告 `exact`、`verified-at-path` 或 `relocated`。
- sidecar 不在 `.gitignore` 时只提示建议，不自动修改用户项目。

## 验证

- cursor 单元测试覆盖 v1→v2 兼容、history/stack/clipboard round-trip、focus 签名与 marker、cut/paste、前方增删、swap/unwrap，以及重复 fingerprint 下的确定性 move。
- 临时 Calcit snapshot 手工验证 `cursor show --view focus`、`--cursor-after focus`、back/push/pop、copy/cut/paste 和 `edit cp/mv/split-def/rename/mv-def`。
- 全局安装后的 `cr` 在 `respo-calcit-workflow` 临时副本中完成深层 cursor focus、cut/paste 原位恢复、`tree replace --path @cursor` 与 `cr js` 编译。
- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface`（12/12）
- `yarn check-all`
- `cr docs check-md` 验证 `docs/run/edit-tree.md` 与 `docs/CalcitAgent.md`
