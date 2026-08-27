# Async FFI response review fixes

## 中文

- 将每个 host table 的 context 绑定到对应 task handle，阻止不同任务或模块意外复用 host context；
- 在调用 dylib resolver 前原子 claim response，保证并发 resolve/reject 只会进入模块一次，模块返回错误时也会释放 capability；
- 使用 ordered deadline index、response lookup 与 owner index 代替每轮 drain 和 task 结束时的全 registry clone/scan；
- 每个任务最多允许 1024 个并发未完成 response，避免只 open 不 enqueue 导致无界增长；
- response 在队列等待期间过期时只 reject/skip 当前 request，不再把整个长期 Server 标记失败并清空后续事件；
- 修正文档中的 `REQUIRES_RESPONSE` 术语，并补充 host table 生命周期、并发与超时语义。

## English

- bound each host-table context to its task handle so another task or module cannot accidentally reuse it;
- atomically claim a response before entering the dylib resolver, ensuring concurrent resolve/reject attempts invoke the module at most once and release the capability even when the module reports an error;
- replaced full-registry clones/scans during drains and task completion with an ordered deadline index, response lookup, and owner index;
- capped concurrent outstanding responses at 1024 per task to prevent unbounded open-without-enqueue growth;
- reject and skip a request that expires in the queue without failing the long-lived Server or purging later events;
- aligned documentation with `REQUIRES_RESPONSE` terminology and clarified host-table lifetime, concurrency, and timeout semantics.

## Validation

- `cargo fmt --all`;
- `cargo clippy --all-targets -- -D warnings`;
- `cargo test -- --test-threads=1` (855 Rust tests passed);
- `yarn compile`;
- `yarn check-agent-interface` (17/17);
- `yarn check-all` (Calcit core 221/221 plus JS, IR, WASM, and benchmark checks);
- rebuilt the standalone C dylib and verified pthread Server request → Calcit
  callback → response resolver → explicit `&unit` completion with no runtime
  errors;
- verified an idle Server task capability can still be cancelled and reaches
  explicit `&unit` completion with no runtime errors.
