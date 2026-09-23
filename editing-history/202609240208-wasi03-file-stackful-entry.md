# WASI 0.3 文件业务的命令入口选择

`#1267` 复用已有的 `FsPath .read-text/.write-text` 与 `Result` 语义，不为 Component 新增表层 API。WASI 0.3.1 的文件 `open-at` 是异步调用，因此包含这些文件操作的 command 必须使用 stackful `wasi:cli/run` 导出；这个选择不应取决于程序是否碰巧调用 `println`。原有实现只在 stdout/stderr 被识别时启用 stackful，文件程序如果不打印就会选错入口。

本次先识别已推断为 `FsPath` 的普通方法调用，以及直达 core 文件边界的调用，并让文件效果与标准输出共用一个 stackful command 决策。`[task-return]run` 与该决策绑定；stdout/stderr 的 stream imports 仍只在实际使用时注册。目录方法不在此切片中，不能被误认为已支持的文件能力。

这只是后续 host ABI lowering 的前置条件：`&fs-read-text` 和 `&fs-write-text` 在 Component 下仍返回 `E_WASI_COMMAND_CAPABILITY`。后续必须按同一 Manifest fixture 接上 preopen、异步 `open-at`、有界 stream 读写、资源关闭和真实 Wasmtime 49 正反例；不能凭入口选择通过就关闭 issue。

验证：聚焦 Rust 测试覆盖文件方法与目录方法的识别，原 Component capability 拒绝测试保持通过；`cargo fmt --check`、`cargo clippy -- -D warnings` 与 `git diff --check` 检查本切片。
