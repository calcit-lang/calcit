# FFI Interface IR v3 lifecycle metadata / FFI Interface IR v3 生命周期元数据

## 中文

- 将 `calcit ffi export --json` 的权威 Interface IR 从 v2 升级为 v3，并新增对应 JSON Schema。
- 导出并校验 WSS 异步 stream 的 callback、事件类型、协作式取消和任务所有权，以及 regex opaque resource 的构造器/方法所有权元数据。
- 生命周期定义在适配器与 conformance vectors 就绪前保持明确的 `unsupported` 诊断；资源消费所有权也会被拒绝，避免生成不安全的 ABI。
- 更新 Agent interface 检查和用户文档，并覆盖 stream、resource 及拒绝消费输入的单元测试。

## English

- Upgraded the authoritative Interface IR emitted by `calcit ffi export --json` from v2 to v3 and added its JSON Schema.
- Export and validate WSS async-stream callback/event/cancellation/task ownership metadata and regex opaque-resource constructor/method ownership metadata.
- Keep lifecycle definitions explicitly `unsupported` until adapters and conformance vectors are available; consuming resource ownership is also rejected to avoid an unsafe ABI.
- Updated the Agent interface check and user documentation, with unit coverage for stream, resource, and consuming-input rejection paths.
