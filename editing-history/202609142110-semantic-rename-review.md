# 语义重命名 review 修复

## 中文

- 修复 schema 引用扫描遗漏零参数 `TypeRef` enum variant 的问题，确保语义重命名在这类 schema 引用存在时继续 fail closed。
- 仅在 resolver usage trace 启用时构造 macro origin，避免普通编译路径为未使用的 provenance 付出开销。
- 增加零参数 enum variant 的定向单元测试，以及真实 Snapshot schema blocker 的原子性集成测试。
- staged validation 以稳定 warning identity 对比改写前后结果：保留已有 warning，但拒绝新 warning 或重复数量增加。

## English

- Scan zero-argument `TypeRef` enum variants so semantic rename continues to fail closed when schemas reference the target.
- Build macro-origin provenance only while resolver usage tracing is active, avoiding overhead during ordinary compilation.
- Add a focused unit test for zero-argument enum variants and an atomic Snapshot integration test for the schema blocker.
- Compare staged validation against stable pre-edit warning identities, preserving existing warnings while rejecting new warnings or increased multiplicity.
