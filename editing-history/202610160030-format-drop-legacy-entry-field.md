# edit format 显式删除旧 entry 字段（#2052）

## 依据与设计

Cumulo Phlox workflow 的旧 Snapshot 在 named entry 中带有旧编辑器字段 `:storage-key`（以及 `:port`）。
`edit format` 的隔离迁移入口拒绝未知字段且不提供删除途径，而普通 loader 又要求先格式化，结构化流程形成循环。

新增可重复的 `edit format --drop-legacy-entry-field <entry>.<field>`：只删除点名的字段，
拒绝当前 entry 字段、不存在的 entry 或字段，并在输出中列出每个被删值；未点名的未知字段继续被拒绝。
未知字段报错一次列出全部字段并给出完整命令。不按项目硬编码 `storage-key`，不静默丢弃元数据，
也不修改运行时 loader。

## 退场条件

与顶层 `:configs` 的一次性迁移入口同时退场：已知下游的旧 compact Snapshot 都迁移后，在非 patch 版本删除。

## 验证

- `src/snapshot.rs` 单元测试覆盖报错提示、非法目标、部分删除仍拒绝、全部删除后迁移成功与报告值。
- CLI 冒烟：带 `:configs` 与 `entries.server` 旧字段的 Snapshot 报错一次列出两个字段，按提示命令完成格式化。
- `cargo test`、`yarn check-all`、`cargo clippy -- -D warnings`。
