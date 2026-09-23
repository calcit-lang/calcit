# WASI 0.3 文件导入的 Canonical ABI 基线

`#1267` 的业务程序仍只调用 `FsPath .read-text/.write-text`，内部 Component 则需要固定的 `wasi:filesystem@0.3.1` preopens/types 接口。本切片把 `get-directories`、descriptor drop、异步 `open-at`、read/write stream 与 future 的原始导入签名放在同一处，且只在识别到文件方法的命令中注册。stdout/stderr 仍按使用情况单独注册；不增加 Calcit 顶层命令、动态 FFI 或暴露资源句柄。

签名不能从函数名猜测：`open-at` 的异步参数采用间接内存记录和结果指针；`read-via-stream` 的 stream/future 位于结果 tuple 的槽位 0/1；`write-via-stream` 的输入 stream 占槽位 0，返回的 future 属于槽位 1。用 `calcit-bindgen 0.1.9` 所固定的 WASI 0.3.1 WIT 对一个最小 core module 实际进行 Component 包装，错误的 future 槽位会被验证器拒绝。再用真实业务 Snapshot 中的独立 `reload!` 入口编译 Component，验证接口存在且 Wasmtime 49 可加载运行。

这些验证只证明导入表与包装契约，不证明文件读取或写入已可用。当前 `&fs-read-text` / `&fs-write-text` 的 `E_WASI_COMMAND_CAPABILITY` 拒绝必须保留，直到 preopen 绑定、异步打开、有界 stream、资源清理和 Manifest 成功/失败路径在真实 Wasmtime 下全部通过。
