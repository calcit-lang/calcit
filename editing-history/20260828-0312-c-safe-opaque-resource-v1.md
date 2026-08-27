# C-safe opaque resource v1

- 在同步 buffer v1 上增加保留的 `CalcitFfiResourceV1` token，以 little-endian `u64` handle 和 generation 表示模块资源。
- 宿主将 token 水合为自动生命周期的 `AnyRef`，固定创建资源的 dylib，并在最后一个引用释放时 exactly-once 调用模块 C ABI release。
- 调用前递归校验资源归属；拒绝 wrong-module、普通 AnyRef、伪造 token 和畸形 token。
- 覆盖嵌套资源、并发 clone/drop、错误边界和协议格式测试，并为 `--trace-ffi` 增加资源创建与释放事件。
- 新增中英双语协议文档，并从 FFI bindings 与升级手册链接。

