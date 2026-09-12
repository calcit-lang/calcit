# 明确时钟能力仅属于 WASI

- 能力矩阵明确区分 WASI Preview 1 command 与 core WASM module，避免把 `clock_time_get` 支持误读为所有 WASM target 都可用。
- `unix-time-ms` 与 `cpu-time` 在 `calcit wasi` 下可用；`calcit wasm` 仍以稳定的 `E_WASM_CAPABILITY` 拒绝这两个宿主能力。
