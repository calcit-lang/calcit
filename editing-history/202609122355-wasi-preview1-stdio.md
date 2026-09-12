# WASI Preview 1 标准输出 / WASI Preview 1 stdio

## 中文

- 为 WASI command target 登记真实类型的 `wasi_snapshot_preview1.fd_write` 导入，同时让通用宿主导入描述支持非 `f64` 参数与返回值。
- `println`、`eprintln` 与 `echo` 保持 Calcit 表层 API，通过私有运行时适配层分别写入 stdout/stderr；多参数使用空格连接并补换行。
- 写入按照 UTF-8 字节长度执行，并循环处理短写；WASI errno、零进展或无效的超长写入会 trap，避免产生被静默截断的输出。
- `calcit/test-wasi-command.cirru` 在定义级测试中保留返回值语义，`scripts/test-wasi-preprocess.sh` 使用 Wasmtime 捕获并核对 Unicode stdout 与数值 stderr。

验证：`cargo fmt --check`、`cargo test wasi_target_registers_the_preview1_fd_write_abi --lib`、`bash scripts/test-wasi-preprocess.sh`。

## English

- Registered the accurately typed `wasi_snapshot_preview1.fd_write` import for the WASI command target and generalized host-import declarations beyond the former all-`f64` signature.
- Kept `println`, `eprintln`, and `echo` as target-independent Calcit APIs, lowering them through a private runtime adapter to stdout or stderr with space-separated arguments and a trailing newline.
- Writes use UTF-8 byte lengths and retry short writes. WASI errors, zero progress, and invalid oversized writes trap instead of silently truncating output.
- `calcit/test-wasi-command.cirru` retains return-value semantics in definition-attached tests, while `scripts/test-wasi-preprocess.sh` captures and verifies Unicode stdout and numeric stderr under Wasmtime.

Verified with `cargo fmt --check`, `cargo test wasi_target_registers_the_preview1_fd_write_abi --lib`, and `bash scripts/test-wasi-preprocess.sh`.
