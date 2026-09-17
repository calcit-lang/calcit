# 增量动态方法分析的模块完整性

- `dynamic-methods --incremental` 在 active-entry module 加载失败时停止分析，不复用可能过期的入口诊断。
- 回归先建立模块与诊断缓存，再移除模块源码，验证增量与非增量路径都会失败。
- 闭包外 definition 修改后同时比较增量与无缓存报告，避免只验证 cache metadata 而漏掉陈旧诊断。
