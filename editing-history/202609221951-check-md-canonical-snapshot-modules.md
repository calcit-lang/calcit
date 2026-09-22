# 文档检查依赖统一使用当前 Snapshot

- `docs check-md` 原先为读取 `:entries.default.modules`，独立解析原始 EDN，并允许旧 `%Expr` CodeEntry 先通过依赖发现；实际执行片段仍使用严格 Snapshot loader，因此该兼容分支不能让旧快照成功运行。
- 依赖发现现在通过正式 loader 和 active entry 读取模块列表，保留 CLI `--dep` 追加及去重；旧 `:configs` 与 `%Expr` 等历史结构从入口一致拒绝，迁移流程只由 `edit format` 的隔离入口负责。
- 边界测试覆盖正常模块合并、旧配置与旧 CodeEntry 的拒绝。此改动仅收敛 CLI 的主机侧加载路径，不改变 Calcit 表层语义，因此不增加重复的 definition `:tests`。
