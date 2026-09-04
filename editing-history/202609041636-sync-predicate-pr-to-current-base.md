# Sync predicate-contract PR to the current base / 同步谓词契约 PR 到当前基线

## Context / 背景

The collection base advanced with an integrated member-contract commit while the predicate-contract branch was being updated. Git reported overlaps because the top branch already carried equivalent member-contract work.

集合基线在更新谓词契约分支时加入了已集成的成员契约提交。由于顶层分支已经带有等价的成员契约改动，Git 报告了重叠。

## Resolution / 解决方案

- Retain the top branch's equivalent and extended collection, map, predicate, and lexical FFI coverage.
- Record the new base as a merge parent so the PR can merge against the current branch graph.
- Verify the complete type-fail subset after resolving the overlap.

- 保留顶层分支中等价且扩展的集合、map、predicate 与词法 FFI 覆盖。
- 将新基线记录为 merge parent，使 PR 可针对当前分支图合并。
- 解决重叠后验证完整 type-fail 子集。
