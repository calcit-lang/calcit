# 2026-09-22 00:48 退役旧式 namespace 与 Snapshot 展示兼容

- 当前 namespace 使用 `ns`；加载器对旧式 `:ns` 提供包含命名空间和 `:require` 保留提示的稳定错误。
- 通过 `calcit edit imports` 将主仓库的 hygienic 测试快照迁移为 `ns`，不直接编辑 Snapshot 文本。
- 错误展示只对当前 `quote` 代码生成 Cirru 预览，移除仅用于早期 `%Expr` / `%Leaf` 结构的递归转换及其测试，保留 `CodeEntry` 中 quoted code 的预览。
- Caltrop 的两份旧快照仍使用 `:configs`，当前加载器在到达 `:ns` 前已拒绝；整体升级需独立处理，不应把保留 `:ns` 视为它们可运行的证据。
