# 类型安全的 Option 查询终点

- 盘点本地 Calcit 生态中的 `unwrap` / `unwrap-or` 使用，确认最大重复模式是查询后立即提供默认值。
- 新增 `get-or`、`get-in-or`、`get-env-or`、`first-or`、`last-or`、`nth-or` 六个 core 宏；宏展开到内部 `option:unwrap-or`，保留 Option 泛型检查和跨后端语义。
- 保持原查询 API 始终返回 `Option<T>`，分支处理继续使用 `if-let` / `match`，不引入隐式解包或 Dynamic fallback。
- 增加正常、缺失和 fallback 类型不兼容测试，并更新诊断、升级文档、常见模式与 RFC 索引。
- 验证中发现 `option:fold` 未强制两个回调共享返回类型，记录为 GitHub issue #388；因此本版不承诺惰性 fallback。
