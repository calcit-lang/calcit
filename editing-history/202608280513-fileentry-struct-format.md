# FileEntry struct support in format migration

- 修复 0.13.54 format-only migration loader 只接受 `FileEntry` map、不能读取 struct 的回归。
- 统一 formatter 与普通严格 loader 对现代 `FileEntry` map/struct 的接受范围，不扩大 direct-quote runtime 兼容边界。
- 增加与 Calcit 0.13.51 schema migration 输出一致的 `%FileEntry` struct 回归 fixture。
- 缺陷由 `calcit-paint` 的真实 direct quote → strict macro schema → current formatter 升级链发现并记录为 Issue #493。
