# WASI Cirru EDN 输出边界

- `format-cirru-edn` 的 WASM 专用拼接与字符串 leaf 编码在分配前检查 64 KiB 输出上限，避免长度溢出或无界单次输出分配。
- 通用字符串操作不受该 formatter 边界影响。
- WASI 回归会验证超限 trap，并把真实 Wasmtime stdout 中的每个 Cirru EDN 值交给 `cirru_edn` 重新解析。
