# WASI 0.3 文件打开的内部 runtime lowering

在已验证真实 host 的 `open-at` ABI 后，把测试中的手写调用序列抽成文件命令可复用的 WASM runtime helper。它先选择具名 preopen，再为异步 Canonical ABI 分配间接参数和返回区，调用 `descriptor.open-at`，等待 subtask 完成，并按所有权归还 waitable set、subtask、父目录 descriptor 和临时内存。成功时只把新打开文件的 descriptor 交给调用方；不存在文件、未授权路径和 `..` 路径返回失败，不借出资源。

WIT `error-code::other(option<string>)` 含可拥有的字符串，错误分支在释放返回区前回收该字符串。直接路径使用不跟随符号链接的 path flag；安全边界仍以 host 的 descriptor capability 为准，不以字符串检查代替。

此 helper 已编入包含文件操作的 WASI 0.3 command，且真实 Wasmtime 49 测试直接调用它；生成的 Calcit Component smoke 和 clippy 通过。目前 Calcit `&fs-read-text`/`&fs-write-text` 仍明确拒绝，尚需 stream 数据路径、Calcit Result 映射及 Manifest 业务验收，不能据此关闭 `#1267`。
