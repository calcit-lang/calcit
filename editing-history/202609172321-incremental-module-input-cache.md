# Reuse incremental inputs by module / 按模块复用增量输入

- Replaced the all-or-nothing merged analysis input cache with schema v2 units for the main Snapshot and each direct module.
- Validated every module unit against its complete transitive source digests and module-resolution paths, reloading only invalid units.
- Added stable main/module hit, miss, and invalidation evidence without making cache state a type-correctness authority.
- Kept incomplete module loads from overwriting the last complete cache and preserved cold full analysis as the semantic reference.
- Covered main edits, direct and transitive module edits, resolution changes, interrupted module availability, corrupt caches, and cold/incremental equivalence.

- 将全量合并式 analysis input cache 升级为 schema v2，分别缓存主 Snapshot 和每个 direct module。
- 每个模块单元校验完整传递 source digest 与 module resolution，只重载失效单元。
- 增加稳定的 main/module hit、miss 与失效证据，但不让缓存承担类型正确性判断。
- 模块加载不完整时不覆盖上一份完整缓存，并保留无缓存冷分析作为语义基准。
- 覆盖主项目修改、direct/transitive module 修改、解析路径变化、模块暂时不可用、缓存损坏及冷/增量结果一致性。
