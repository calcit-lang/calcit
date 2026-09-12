# 统一安全随机字节能力

- 在 core 增加 `secure-random-bytes`，以 `Result<Buffer,String>` 统一 Native、JavaScript 与 WASI command 的公开语义，并限制单次请求为 `0..65536` 字节。
- Native 使用系统 CSPRNG，生成的 JavaScript 使用 Web Crypto，WASI command 通过 Preview 1 `random_get` 填充由 Calcit 管理的 Buffer。
- core WASM 不伪造随机数，继续用稳定的 `E_WASM_CAPABILITY` 拒绝缺少宿主协议的调用。
- 主要语义测试定义在 Calcit Snapshot 的 `:tests` 中；脚本只负责跨后端运行、固定字节验证和宿主错误注入。
