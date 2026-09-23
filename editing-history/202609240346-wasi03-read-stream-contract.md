# WASI 0.3 文件 stream 读取契约

沿 `#1267` 的文件路径，在真实 Wasmtime 49 WASI 0.3 host 中，用已完成的内部 `open-at` helper 打开具名 preopen 下的文件，再验证 `descriptor.read-via-stream` 返回 stream/future 两个 handle。测试实际读出 `hello` 的 5 个字节，下一次读取收到 EOF，并在关闭 stream 后读取 future 的成功结果，最后归还 stream、future 和文件 descriptor。

完整文件 imports 包含同步 `future.read`，Wasmtime 执行时除 `component-model-async-stackful` 外，还必须显式启用 `component-model-more-async-builtins`；缺少后者时 Component 在解析阶段失败。此真实 host 测试纳入 CI，避免仅靠打包成功误判文件读取可用。

本切片确认低层 stream/future 契约；尚未实现有界多次读取、超限拒绝、UTF-8 验证、错误字符串释放和 Calcit `Result` 映射。`&fs-read-text` 仍明确拒绝，不能关闭 `#1267`。
