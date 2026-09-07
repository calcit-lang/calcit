# Reserve WASI preprocessing stack / 为 WASI 预处理预留栈空间

- Reproduced calcit#763 with the archived 0.13.77-era Snapshot and Wasmtime
  18.0.3: native preprocessing completed, while the default 1 MiB wasm-ld
  shadow stack underflowed and trapped as an out-of-bounds memory access.
- Splitting the largest `preprocess_list_call` frame only moved the trap into
  another recursive evaluator path. The failure belongs to the standalone
  compiler's aggregate recursive workload, not one malformed source function.
- Set the `cr-wasm` WASI link stack to 4 MiB in `build.rs`. The resulting
  `__stack_pointer` starts at 4194304 and the unchanged recursive preprocessing
  path completes under Wasmtime.
- Added a minimized, canonically formatted legacy Snapshot plus a native/WASI
  parity script and a pinned CI job using Wasmtime 18.0.3.

- 使用归档的 0.13.77-era Snapshot 和 Wasmtime 18.0.3 复现 calcit#763：native
  预处理成功，而 wasm-ld 默认 1 MiB shadow stack 下溢并触发越界内存 trap。
- 拆分最大的 `preprocess_list_call` 栈帧只会把 trap 移到另一个递归求值路径，
  因此根因是独立编译器整体递归工作负载，而不是单个异常源码函数。
- 在 `build.rs` 中把 `cr-wasm` 的 WASI 链接栈设为 4 MiB；生成模块的
  `__stack_pointer` 初值为 4194304，原递归预处理路径可在 Wasmtime 完成。
- 添加最小化并规范格式化的旧 Snapshot、native/WASI parity 脚本，以及固定
  Wasmtime 18.0.3 的 CI job。

## Validation / 验证

- `bash scripts/test-wasi-preprocess.sh`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test` (730 + 308 + 23 passed)
- `yarn check-all`
