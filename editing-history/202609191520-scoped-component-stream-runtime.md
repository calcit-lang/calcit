# 交付作用域化 Component 字节流消费 / Deliver scoped Component byte-stream consumption

## 中文

- 在现有 `calcit wasm --boundary component` 路径内增加严格受限的 `ReadableByteStream` 消费形式：异步导出只能直接调用一次 `consume-readable-byte-stream`，并使用编译期常量限制总字节数与单次 chunk。
- 生成 stackless Canonical ABI 状态机，保持至多一个未完成 read；覆盖背压恢复、EOF、总量超限、handler 提前停止、主动取消、exactly-once stream drop 与 task return/cancel。
- 将 `StreamConsumeError` 作为内建、可导出的 nominal enum，并保持 Cirru EDN 为默认 Interface IR；没有新增命令入口。
- 语义示例放在 Calcit definition `:tests`；Rust/Node/Wasmtime 测试只验证 Canonical ABI、生命周期、内存与真实慢生产者恢复。

## English

- Add a strictly scoped `ReadableByteStream` consumer form under the existing `calcit wasm --boundary component` path: an async export may only delegate directly to one `consume-readable-byte-stream` call with compile-time total and chunk limits.
- Generate a stackless Canonical ABI state machine with at most one outstanding read, covering backpressure resume, EOF, total-limit failure, handler early stop, active cancellation, exactly-once stream drop, and task return/cancel.
- Export `StreamConsumeError` as a built-in nominal enum while keeping Cirru EDN as the default Interface IR and adding no command surface.
- Keep user-observable handler semantics in Calcit definition `:tests`; use Rust, Node, and Wasmtime only for Canonical ABI, lifecycle, memory, and a real delayed producer.
