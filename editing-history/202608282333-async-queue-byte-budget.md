# Async queue byte budget and terminal reserve

## 中文

- native async queue 现在可同时限制事件数和累计 payload bytes，避免大量合法但体积较大的事件在 1024 个 event slots 内消耗过多内存。
- CLI runtime 使用 64 MiB 总字节预算，并为 `Complete`/`Fail` 预留 16 个 event slots 和 64 KiB；普通 Stream/Server emit 不能侵占这些预留资源。
- event admission 成功后才分配 sequence；event/byte budget 失败都返回稳定的 `QUEUE_FULL`，不会误占 terminal 或 sequence。
- coalescing 在字节压力下也能替换同一 Stream 的旧 emit，并精确维护 queued-byte accounting；drain/purge 同样回收字节账本。
- `--trace-ffi` 的 enqueue accepted/rejected 事件现在包含当前 queued events/bytes，便于定位 slow host 或 producer pressure。
- 修复 async protocol 文档仍描述 legacy Rust callback fallback 的过时内容。

## English

- The native async queue can now bound both event count and aggregate payload bytes, preventing a queue of individually valid events from consuming excessive memory.
- The CLI runtime uses a 64 MiB byte budget and reserves 16 event slots plus 64 KiB for `Complete`/`Fail`; ordinary Stream/Server emits cannot consume those reserves.
- Event sequences are allocated only after admission. Event-count and byte-budget failures both map to stable `QUEUE_FULL` without claiming a sequence or terminal event.
- Coalescing can replace an older Stream emit under byte pressure while keeping queued-byte accounting exact; drain and purge release the same accounting.
- Accepted and rejected `--trace-ffi` enqueue events now include current queued event/byte counters for diagnosing slow hosts and producer pressure.
- Removed stale async protocol documentation that still described the deleted legacy Rust callback fallback.
