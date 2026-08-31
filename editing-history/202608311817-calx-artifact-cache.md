# Calx revision-safe artifact cache / Calx revision-safe artifact 缓存

## 中文

- 将原先同时持有 validated program 与 host callbacks 的 kernel 拆成 immutable `CalxCompiledArtifact` 与每次请求重新构造的 `CalxPreparedKernel`。
- 新增 embedding-owned、容量有界、确定性 LRU 的 `CalxCompileCache`；缓存只保存 source-derived artifact，不保存 callback、capability state、VM、输入或 runtime state。
- cache hit 逐项核对 reachable definition 的结构化 code/schema，并额外核对实际使用的 Calcit host-import definition schema；同 contract 的 callback 热替换继续命中，但静态签名变化必须 miss。
- miss reason、eviction ledger、clear、capacity-zero、stale callback、entry/callee/schema/unrelated change 与 negative eligibility 均有回归覆盖。
- 增加迁移期 `calcit-calx-cache-profile/1` 单 JSON runner contract，分开报告 revision validation、binding attachment、fresh VM、reused VM、cached Calcit 与估算 cache bytes；该 runner/schema 后续随独立 harness 迁移。
- strict Calx boundary 继续只接受 concrete Number/Bool/Unit；本次没有增加 Nil、Dynamic、List coercion、VM pool 或自动 offload。

## English

- Split the former kernel, which combined a validated program with host callbacks, into an immutable `CalxCompiledArtifact` and a freshly attached `CalxPreparedKernel` per request.
- Added an embedding-owned, capacity-bounded, deterministic-LRU `CalxCompileCache`. Cached artifacts contain no callbacks, capability state, VM instances, inputs, or runtime state.
- Cache hits validate structural code/schema for every reachable definition and also recheck the schema of each Calcit definition actually used as a host import. A callback replacement with the same contract remains a hit, while a static-signature change must miss.
- Regression coverage includes miss reasons, the eviction ledger, clear, zero capacity, stale callbacks, entry/callee/schema/unrelated changes, and uncached negative eligibility.
- Added the transitional single-JSON `calcit-calx-cache-profile/1` runner contract, separating revision validation, binding attachment, fresh VM, reused VM, cached Calcit, and estimated cache bytes. This runner and schema migrate with the standalone harness.
- The strict Calx boundary remains limited to concrete Number/Bool/Unit values. This change adds no Nil, Dynamic, List coercion, VM pool, or automatic offload.
