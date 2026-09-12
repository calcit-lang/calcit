# WASI 进程退出语义 / WASI process exit semantics

## 中文

- 将 `quit!` 的可移植退出码约束统一为 `0..255` 的整数；原生与 JavaScript 后端拒绝小数、非有限数和越界值。
- WASI command target 引入 Preview 1 `proc_exit`，把 `quit!` 显式 lowering 为宿主进程退出；core WASM 仍保持无进程退出能力并 trap。
- 使用同一个 Calcit command fixture 验证原生、JavaScript 与 WASI 均返回状态码 `7`，让用户可见语义由 Calcit definition 定义，脚本只检查宿主边界。

验证：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test -q`、`yarn compile`、`bash scripts/test-js-command.sh`、`bash scripts/test-wasi-preprocess.sh`。

## English

- Unified the portable `quit!` contract around integer exit codes in `0..255`; native and JavaScript backends reject fractional, non-finite, and out-of-range values.
- Added Preview 1 `proc_exit` to the WASI command target and lower `quit!` to host process termination. Core WASM retains no process-exit capability and traps.
- Reused one Calcit command fixture to verify exit status `7` across native, JavaScript, and WASI. The Calcit definition expresses the user-facing behavior while scripts only check host boundaries.

Verified with `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test -q`, `yarn compile`, `bash scripts/test-js-command.sh`, and `bash scripts/test-wasi-preprocess.sh`.
