# Intern FFI resource leases

- 根据 review 修复重复 resource token 产生独立 lease、提前释放和重复释放的问题。
- 宿主按 `(module, handle, generation)` 保存 weak intern entry；同一响应及跨响应 alias 复用同一 lease。
- 最后一个强引用析构后 exactly-once release，并清理失效的 weak entry。
- 增加同响应重复 token 与跨响应重复 token 的回归测试，并同步协议文档。

