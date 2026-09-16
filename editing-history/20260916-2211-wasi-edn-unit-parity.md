# WASI Cirru EDN Unit 语义对齐

Cirru EDN 的 `nil` 与 Calcit 的 `Unit` 是不同类型，native typed decoder 会明确拒绝把任何 EDN 值解码成 Unit。WASM 标量 parser 因此不能复用 nil 分支生成 Unit。

本次调整让闭合 Unit shape 仍可通过安全 API 编译，但解析始终返回稳定 `Result :err`；Calcit definition test 与真实 Wasmtime fixture 同时覆盖该行为，避免后端语义漂移。
