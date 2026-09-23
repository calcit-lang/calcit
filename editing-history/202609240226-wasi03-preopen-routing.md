# WASI 0.3 文件路径的 preopen 匹配边界

`#1267` 的 guest path 来自具名 `FsPath` 字符串，而 WASI 0.3 文件打开必须使用 host 提供的 preopen descriptor 和相对路径。不能把任意 guest path 直接交给 `open-at`，也不能把字符串检查当作符号链接权限检查。

本切片增加内部 WASM 路由辅助函数：输入 Calcit 路径字符串与一个 WIT preopen 名称，返回匹配根之后的 UTF-8 字节偏移，失败返回 `-1`。路径必须非空、相对、长度有界，不含 NUL 或完整 `..` 分量；具名根只在完整路径分量匹配且后面确有文件名时成立。`.` 与 `/` preopen 代表整个相对 guest 路径。这样后续可以在多个 preopen 中选择最长匹配根，不因 `workspace2` 误命中 `workspace`。真正的符号链接越界仍由 WASI host 的 `open-at` capability 语义拒绝。

辅助函数已经编入识别到文件操作的命令模块，但尚未连接 `get-directories` 或 `open-at`；当前文件调用继续明确拒绝。WASM 实际执行测试覆盖具名根、根目录、Unicode 字节偏移、前缀碰撞、空路径、绝对路径、NUL、`..` 与普通双点文件名。现有 Component CLI/Wasmtime 49 回归保持通过。下一步是枚举并释放 preopen 资源，保留选中的 descriptor，再执行异步打开。
