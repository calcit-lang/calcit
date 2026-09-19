# 补齐流消费验收 / Close stream-consumer acceptance gaps

## 中文

- 让 Calcit `on-chunk` handler 校验三个不同大小的 Buffer 分片，并在 definition `:tests` 中同时覆盖接受预期分片与拒绝意外分片。
- 为 WASM 的 `&=` 增加无分配的 Buffer 字面量比较路径；比较前检查堆边界、magic 与 Buffer 类型标签，避免为校验分片引入临时分配或泄漏。
- 将真实 Wasmtime 生产者改为三个依次背压的分片，并重复调用以确认并发状态归零；Node 生命周期测试同时检查单一未完成 read、句柄释放、分配地址和内存页高水位稳定。

## English

- Make the Calcit `on-chunk` handler validate three differently sized Buffer chunks, with definition `:tests` covering both accepted expected chunks and a rejected unexpected chunk.
- Add an allocation-free Buffer-literal comparison path for WASM `&=`; validate heap bounds, magic, and the Buffer type tag before reading so chunk validation creates no temporary allocation or leak.
- Feed three independently backpressured chunks through the real Wasmtime producer and repeat calls until concurrent state is proven empty; also assert one outstanding read, released handles, a stable allocation address, and a stable memory-page high-water mark in the Node lifecycle test.
