# Revision-safe Calx program artifact cache / revision-safe Calx 程序产物缓存

## English

- Extended the existing embedding-owned cache invariants from one-entry kernels to the versioned whole-program compilation unit.
- Kept cache keys callback-free while covering program ABI, selected entry, host target, lifecycle root roles/definitions, and typed import declarations.
- Added structural stamps for every reachable compiled definition and imported Calcit schema; unrelated changes remain hits, while root/callee/schema/missing-dependency changes rebuild or fail atomically.
- Reattached current typed callbacks on every request and kept `CalxProgramInstance`, VM globals, platform handles, connections, and capability state outside cached artifacts.
- Added deterministic LRU, capacity-zero, clear, counters, miss reasons, artifact-size gauges, and positive/negative regression coverage without changing the kernel compatibility API.
- Isolated an existing effects-graph fixture with the shared program-state test guard so the expanded cache suite stays deterministic under parallel `cargo test` execution.

## 中文

- 将已有 embedding-owned 单入口 kernel cache 的不变量扩展到有版本的 whole-program compilation unit。
- cache key 不含 callback identity，但覆盖 program ABI、selected entry、host target、lifecycle root role/definition 与 typed import declarations。
- 为全部 reachable compiled definitions 与 imported Calcit schema 增加结构 stamp；无关修改继续命中，root/callee/schema/依赖缺失则重建或原子失败。
- 每次请求重新挂载当前 typed callbacks；`CalxProgramInstance`、VM globals、平台 handles、连接与 capability state 均不进入缓存 artifact。
- 增加确定性 LRU、capacity-zero、clear、计数、miss reasons、artifact-size gauges 及正反回归，不改变 kernel compatibility API。
- 为既有 effects-graph fixture 补上共享 program-state 测试守卫，保证扩展后的 cache suite 在并行 `cargo test` 下仍具确定性。
