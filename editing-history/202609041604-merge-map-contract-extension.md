# Merge concurrent map-contract extension / 合并并发的 map 契约扩展

## Context / 背景

While the stacked predicate-contract branch was being updated, its remote head advanced with the independent collection map-contract extension. The update was fetched before retrying the rejected push.

在更新串联的谓词契约分支时，远端 head 同时加入了独立的集合 map 契约扩展。重试被拒绝的推送前已拉取该更新。

## Resolution / 解决方案

- Merge the concurrent remote head without overwriting it.
- Preserve the predicate-contract work and lexical FFI fixture coverage alongside the map-contract changes.
- Re-run type-fail, generated Dynamic-classification, and quality-baseline checks on the combined revision.

- 合入并发远端 head，不覆盖其改动。
- 在 map 契约改动旁保留 predicate 契约工作与词法 FFI fixture 覆盖。
- 在组合 revision 上重新运行 type-fail、生成的 Dynamic 分类和 quality-baseline 检查。
