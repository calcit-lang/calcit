# WASI 0.3 文件 preopen 的选择与资源归还

延续 `#1267`，在文件命令的 WASM runtime 中接入 `get-directories`：枚举 host 授予的 preopen，按最长的完整路径前缀选择 descriptor，并把相对 UTF-8 路径交给后续打开步骤。路径无效、没有匹配目录或 preopen 数量超过 64 时，不授予文件能力。

WIT 返回的目录名称和列表由 guest 的 `cabi_free` 释放；未选中的 descriptor 当场调用 `[resource-drop]descriptor`，选中的 descriptor 留给后续 `open-at` 和清理步骤。测试覆盖多个重叠目录、拒绝 `..`、空列表、超限列表和每一项资源归还。另用 `calcit-bindgen` 打包为真实 Component，在 Wasmtime 49 的 WASI 0.3 host 上验证具名目录映射能命中、未授权路径被拒绝，并纳入 CI。

此切片只解决 preopen 路由及资源所有权的前半段，尚未调用异步 `open-at`，也没有放开 Calcit 的 `&fs-read-text`/`&fs-write-text`。必须继续实现打开、读写、边界和错误路径测试后，才能关闭 `#1267`。
