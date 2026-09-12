# WASI 预开放文件系统第一切片

## 背景

`FsPath` 已在 Native 与生成的 JavaScript 中提供类型化的文本读写，但 WASI command 尚未连接 host preopen。旧的 String compatibility wrapper 还依赖 `try` 捕获异常，也不适合作为 WASM lowering 边界。

## 变更

- 新增内部 `&fs-read-text` 与 `&fs-write-text`，统一返回 nominal `Result`；`FsPath` 与兼容 String wrapper 直接复用这一边界。
- Native 使用系统 UTF-8 文件 API，JavaScript 继续使用显式 `__calcit_injections__`，宿主失败转换为 `Result :err`。
- WASI Preview 1 adapter 枚举 preopen，选择最长 guest 路径前缀，只把剩余相对路径传给 `path_open`。
- 拒绝绝对路径、NUL 与完整 `..` 分段；无 preopen、I/O 错误、非法 UTF-8、零进度或越界 partial I/O 均返回错误，不向 Calcit 暴露 descriptor。
- 文本读取限制为 4 MiB，避免不受限的 host 文件大小直接推动线性内存分配；目录读取仍留在后续切片。

## 验证

- Calcit definition-attached `:tests` 验证缺失路径保持 `Result :err`。
- Native core unit tests：257 passed。
- JavaScript runtime identity 覆盖成功读写、Unit payload 与 host failure。
- 确定性 Preview 1 假宿主覆盖最长 preopen、UTF-8 内容以及多次 partial read/write。
- 真实 Wasmtime 覆盖 named preopen 成功、无 preopen fail-closed、绝对路径、`..` 越界和非法 UTF-8。
- core WASM 对文件能力保持 `E_WASM_CAPABILITY` 拒绝。
