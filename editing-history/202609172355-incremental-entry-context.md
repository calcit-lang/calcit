# Incremental entry context / 增量分析 entry 上下文

- Made static and incremental analysis honor the top-level selected entry before loading direct modules.
- Included the strict-mode default feature policy in the analysis context before cache lookup and reporting.
- Added `cache.input.entry` evidence and a multi-entry regression covering module reuse, policy invalidation, and cold/warm equivalence.
- 让静态与增量分析在加载 direct modules 前应用顶层选定的 entry。
- 在缓存查找和报告前将严格模式默认 feature policy 纳入分析上下文。
- 增加 `cache.input.entry` 证据及多 entry 回归，覆盖模块复用、policy 失效和冷热结果一致性。
