# Bound the wide Struct cache / 限制宽 Struct 缓存

## English

- Keep all live indexes when the 256-entry cache is full and send additional
  definitions through the declaration-order linear fallback.
- Bound cache admission by both 65,536 aggregate fields and 8 MiB of copied
  field-name bytes per thread, in addition to the entry limit.
- Add regressions proving an over-budget definition is not cached, earlier live
  indexes remain present, and both aggregate resource limits reject overflow.

## 中文

- 256 项缓存已满时保留全部已有 live index，新增定义退回按声明顺序的线性查询。
- 除 entry 数外，每线程缓存准入同时限制为累计 65,536 个字段与 8 MiB 复制字段名。
- 增加回归，验证超预算定义不会进入缓存、已有 live index 不被清空，且两个累计资源
  上限都会拒绝溢出。
